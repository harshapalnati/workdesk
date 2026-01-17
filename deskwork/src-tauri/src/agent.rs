use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::path::Path;
use crate::commands;
use crate::context;
use crate::settings::SettingsState;
use crate::session_manager::{SessionState, Session, save_session_to_disk};
use crate::audit;
use reqwest::Client;
use serde_json::{json, Value};
use tauri::Emitter;
use std::time::{SystemTime, UNIX_EPOCH, Duration};

// Re-export Message structs so other modules can use them
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<MessageContentPart>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MessageContentPart {
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<ImageUrl>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ImageUrl {
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Serialize)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<Message>,
    tools: Vec<Value>,
    tool_choice: String,
}

#[derive(Deserialize)]
struct OpenAIChatResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: Message,
    finish_reason: Option<String>,
}

#[derive(Serialize, Clone)]
struct ActivityEvent {
    id: String,
    status: String,
    message: String,
    timestamp: u64,
}

#[derive(Serialize, Clone)]
struct PlanEvent {
    steps: Vec<String>,
    current_step: usize,
}

#[derive(Serialize, Clone)]
struct TelemetryEvent {
    tool: String,
    status: String,
    duration_ms: u128,
    kind: String,
}

fn is_sensitive_tool(function_name: &str) -> bool {
    matches!(
        function_name,
        "execute_command"
            | "write_file"
            | "open_app"
            | "keyboard_type"
            | "keyboard_press"
            | "mouse_move"
            | "mouse_click"
            | "create_docx"
            | "create_slide_deck"
            | "search_web"
    )
}

const APPROVAL_EXPIRY_SECS: u64 = 600; // 10 minutes
const SAFE_COMMANDS: &[&str] = &["ls", "dir", "pwd", "cat", "type", "echo"];

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

fn sanitize_history_for_storage(history: &[Message]) -> Vec<Message> {
    history
        .iter()
        .map(|msg| {
            let mut clone = msg.clone();
            if clone.role == "tool" {
                clone.content = Some(MessageContent::Text("Tool output omitted for privacy.".into()));
            } else if let Some(MessageContent::Text(text)) = &clone.content {
                if text.contains("data:image") {
                    clone.content = Some(MessageContent::Text("Image data redacted.".into()));
                }
            } else if let Some(MessageContent::Parts(_)) = &clone.content {
                clone.content = Some(MessageContent::Text("Structured content redacted.".into()));
            }
            clone.tool_calls = None;
            clone.tool_call_id = None;
            clone
        })
        .collect()
}

fn path_out_of_scope(working_dir: &Option<String>, target: &str) -> bool {
    if let Some(base) = working_dir {
        if base.is_empty() {
            return false;
        }
        let base_path = std::path::Path::new(base);
        let target_path = std::path::Path::new(target);
        if let (Ok(base_canon), Ok(target_canon)) = (base_path.canonicalize(), target_path.canonicalize()) {
            return !target_canon.starts_with(&base_canon)
        } else {
            false
        }
    } else {
        false
    }
}

fn request_approval(
    approval_state: &ApprovalState,
    app: &tauri::AppHandle,
    function_name: &str,
    action: String,
    args: Value,
    working_dir: Option<String>,
    reason: String,
) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let expires_at = now_ts() + APPROVAL_EXPIRY_SECS;
    {
        let mut queue = approval_state.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.push(PendingApproval {
            id: id.clone(),
            function_name: function_name.to_string(),
            action: action.clone(),
            args: args.clone(),
            working_dir: working_dir.clone(),
            expires_at,
        });
    }

    let _ = app.emit("approval_request", json!({
        "id": id,
        "action": action,
        "reason": reason,
        "expires_at": expires_at
    }));

    format!("Approval required ({function_name}). Reply 'approve {id}' or 'deny {id}'. Reason: {reason}")
}

fn pop_approval(approval_state: &ApprovalState, id: &str) -> Option<PendingApproval> {
    let mut queue = approval_state.queue.lock().ok()?;
    let now = now_ts();
    let mut idx = None;
    for (i, item) in queue.iter().enumerate() {
        if item.expires_at <= now {
            continue;
        }
        if item.id == id {
            idx = Some(i);
            break;
        }
    }
    if let Some(i) = idx {
        Some(queue.remove(i))
    } else {
        None
    }
}

fn approval_reason(
    function_name: &str,
    args: &Value,
    working_dir: &Option<String>,
    settings: &crate::settings::AppSettings,
) -> Option<String> {
    if settings.read_only && is_sensitive_tool(function_name) {
        return Some("Read-only mode is enabled".to_string());
    }

    match function_name {
        "write_file" | "read_file" => {
            let path = args["path"].as_str().unwrap_or("");
            if path_out_of_scope(working_dir, path) {
                return Some("Path is outside the active workspace".to_string());
            }
        }
        "execute_command" => {
            let cmd = args["command"].as_str().unwrap_or("").to_lowercase();
            if !SAFE_COMMANDS.contains(&cmd.as_str()) {
                return Some(format!("Command '{}' is not in the allowlist", cmd));
            }
        }
        "open_app" => {
            let path = args["path"].as_str().unwrap_or("");
            if path.to_lowercase().starts_with("http") {
                return Some("External URL requires approval".to_string());
            }
            if path_out_of_scope(working_dir, path) {
                return Some("Application path is outside the active workspace".to_string());
            }
        }
        _ => {}
    }

    if is_sensitive_tool(function_name) {
        Some("Sensitive action requires explicit approval".to_string())
    } else {
        None
    }
}

async fn dispatch_tool(
    app: &tauri::AppHandle,
    function_name: &str,
    args: &Value,
    working_dir: &Option<String>,
    id: String,
    structured_logs: bool,
) -> Result<MessageContent, String> {
    let start = std::time::Instant::now();
    let tool_output = match function_name {
        "set_plan" => {
            let steps: Vec<String> = args["steps"].as_array().map(|arr| arr.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
            let _ = app.emit("plan_update", PlanEvent { steps, current_step: 0 });
            Ok(MessageContent::Text("Plan set.".to_string()))
        },
        "complete_step" => {
            Ok(MessageContent::Text("Step completed.".to_string()))
        },
        "list_dir" => {
            let path = args["path"].as_str().unwrap_or(".");
            let name = Path::new(path).file_name().and_then(|s| s.to_str()).unwrap_or("directory");
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Scanning {}", name), timestamp: 0 });
            commands::list_dir(path.to_string()).map(|f| MessageContent::Text(format!("{:?}", f)))
        },
        "read_file" => {
            let path = args["path"].as_str().unwrap_or("");
            let name = Path::new(path).file_name().and_then(|s| s.to_str()).unwrap_or("file");
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Reading {}", name), timestamp: 0 });
            commands::read_file(path.to_string()).map(MessageContent::Text)
        },
        "write_file" => {
            let path = args["path"].as_str().unwrap_or("");
            let content = args["content"].as_str().unwrap_or("");
            let name = Path::new(path).file_name().and_then(|s| s.to_str()).unwrap_or("file");
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Writing to {}", name), timestamp: 0 });
            commands::write_file(path.to_string(), content.to_string()).map(|_| MessageContent::Text("Success".to_string()))
        },
        "execute_command" => {
            let cmd = args["command"].as_str().unwrap_or("");
            let args_vec: Vec<String> = args["args"].as_array().map(|arr| arr.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Executing {}", cmd), timestamp: 0 });
            commands::execute_command(cmd.to_string(), args_vec, working_dir.clone()).map(MessageContent::Text)
        },
        "open_app" => {
            let path = args["path"].as_str().unwrap_or("");
            let name = Path::new(path).file_name().and_then(|s| s.to_str()).unwrap_or("app");
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Opening {}", name), timestamp: 0 });
            commands::open_app(path.to_string()).map(|_| MessageContent::Text("Opened successfully".to_string()))
        },
        "fetch_url" => {
            let url = args["url"].as_str().unwrap_or("");
            let expected_hash = args["expected_hash"].as_str().map(|s| s.to_string());
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: "Fetching web content...".into(), timestamp: 0 });
            commands::fetch_url(url.to_string(), expected_hash).await.map(MessageContent::Text)
        },
        "get_system_stats" => {
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: "Analyzing system health...".into(), timestamp: 0 });
            commands::get_system_stats().map(|s| MessageContent::Text(format!("CPU: {:.1}%, RAM Used: {}/{}", s.cpu_usage, s.used_memory, s.total_memory)))
        },
        "search_files" => {
            let query = args["query"].as_str().unwrap_or("");
            let path = args["path"].as_str().unwrap_or(".");
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Searching for '{}'...", query), timestamp: 0 });
            commands::search_files(query.to_string(), path.to_string()).map(MessageContent::Text)
        },
        "search_web" => {
            let query = args["query"].as_str().unwrap_or("");
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Searching web for '{}'...", query), timestamp: 0 });
            commands::search_web(query.to_string()).map(|_| MessageContent::Text("Opened browser".to_string()))
        },
        "keyboard_type" => {
            let text = args["text"].as_str().unwrap_or("");
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Typing '{}'...", text), timestamp: 0 });
            commands::keyboard_type(text.to_string()).map(|_| MessageContent::Text("Typed text".to_string()))
        },
        "keyboard_press" => {
            let key = args["key"].as_str().unwrap_or("");
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Pressing {}...", key), timestamp: 0 });
            commands::keyboard_press(key.to_string()).map(|_| MessageContent::Text("Pressed key".to_string()))
        },
        "mouse_move" => {
            let x = args["x"].as_i64().unwrap_or(0) as i32;
            let y = args["y"].as_i64().unwrap_or(0) as i32;
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Moving mouse to {},{}...", x, y), timestamp: 0 });
            commands::mouse_move(x, y).map(|_| MessageContent::Text("Moved mouse".to_string()))
        },
        "mouse_click" => {
            let button = args["button"].as_str().unwrap_or("left");
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Clicking {}...", button), timestamp: 0 });
            commands::mouse_click(button.to_string()).map(|_| MessageContent::Text("Clicked mouse".to_string()))
        },
        "get_screenshot" => {
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: "Capturing screen...".into(), timestamp: 0 });
            commands::get_screenshot().map(|base64| {
                 MessageContent::Parts(vec![
                    MessageContentPart { r#type: "text".into(), text: Some("Screenshot captured. Analyze this image to find coordinates.".into()), image_url: None },
                    MessageContentPart { r#type: "image_url".into(), text: None, image_url: Some(ImageUrl { url: base64 }) }
                 ])
            })
        },
        "wait" => {
            let ms = args["milliseconds"].as_u64().unwrap_or(0);
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Waiting {}ms...", ms), timestamp: 0 });
            commands::wait(ms).await.map(|_| MessageContent::Text("Wait complete".to_string()))
        },
        "create_docx" => {
            let content = args["content"].as_str().unwrap_or("");
            let filename = args["filename"].as_str().unwrap_or("document.docx");
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Creating DOCX: {}", filename), timestamp: 0 });
            commands::create_docx(content.to_string(), filename.to_string()).map(|_| MessageContent::Text("Created DOCX".to_string()))
        },
        "create_slide_deck" => {
             let content = args["content"].as_str().unwrap_or("");
            let filename = args["filename"].as_str().unwrap_or("slides.html");
            let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Creating Slides: {}", filename), timestamp: 0 });
            commands::create_slide_deck(content.to_string(), filename.to_string()).map(|_| MessageContent::Text("Created Slide Deck".to_string()))
        },
         "find_file_smart" => {
            let query = args["query"].as_str().unwrap_or("");
            let path = args["path"].as_str().unwrap_or(".");
             let _ = app.emit("activity", ActivityEvent { id: id.clone(), status: "running".into(), message: format!("Smart finding '{}' in {}...", query, path), timestamp: 0 });
            commands::find_file_smart(query.to_string(), path.to_string()).map(MessageContent::Text)
        },
        _ => Ok(MessageContent::Text("Unknown tool".to_string()))
    };

    let duration_ms = start.elapsed().as_millis();
    if !matches!(function_name, "set_plan" | "complete_step") {
        let _ = app.emit("activity", ActivityEvent {
           id: id.clone(),
           status: if tool_output.is_ok() { "success".into() } else { "error".into() },
           message: format!("{} -> {}", function_name, if tool_output.is_ok() { "Done" } else { "Failed" }),
           timestamp: 0,
       });
        let _ = app.emit("telemetry", TelemetryEvent {
            tool: function_name.to_string(),
            status: if tool_output.is_ok() { "success".into() } else { "error".into() },
            duration_ms,
            kind: "tool".into(),
        });
        let _ = audit::append_audit(
            function_name,
            if tool_output.is_ok() { "success" } else { "error" },
            format!("{:?}", args),
            duration_ms,
            working_dir.clone(),
            structured_logs,
        );
    }

    tool_output
}

// Keep AgentState for backward compatibility or transient state if needed,
// but we will primarily use SessionState now.
#[derive(Default)]
pub struct AgentState {
    pub history: Mutex<Vec<Message>>,
}

#[derive(Clone)]
pub struct PendingApproval {
    pub id: String,
    pub function_name: String,
    pub action: String,
    pub args: Value,
    pub working_dir: Option<String>,
    pub expires_at: u64,
}

#[derive(Default)]
pub struct ApprovalState {
    pub queue: Mutex<Vec<PendingApproval>>,
}

#[tauri::command]
pub async fn chat(
    app: tauri::AppHandle,
    prompt: String,
    working_dir: Option<String>,
    session_id: Option<String>, // New: specific session
    state: tauri::State<'_, AgentState>, // Legacy
    session_state: tauri::State<'_, SessionState>, // New: Persistence
    settings_state: tauri::State<'_, SettingsState>,
    approval_state: tauri::State<'_, ApprovalState>,
) -> Result<String, String> {
    
    // Fast-path approval/deny commands
    let trimmed = prompt.trim().to_lowercase();
    if let Some(rest) = trimmed.strip_prefix("approve ") {
        let id = rest.trim();
        if let Some(pending) = pop_approval(&approval_state, id) {
            let result = dispatch_tool(&app, &pending.function_name, &pending.args, &pending.working_dir, pending.id.clone(), settings.structured_logs).await;
            let _ = app.emit("approval_resolved", json!({"id": id, "status": "approved"}));
            return Ok(match result {
                Ok(msg) => match msg {
                    MessageContent::Text(t) => format!("Approved {}: {}", pending.action, t),
                    MessageContent::Parts(_) => format!("Approved {}: (structured output)", pending.action),
                },
                Err(e) => format!("Failed {}: {}", pending.action, e),
            });
        } else {
            return Ok(format!("No pending approval for id '{}'", id));
        }
    } else if let Some(rest) = trimmed.strip_prefix("deny ") {
        let id = rest.trim();
        if pop_approval(&approval_state, id).is_some() {
            let _ = app.emit("approval_resolved", json!({"id": id, "status": "denied"}));
            return Ok(format!("Denied request {}", id));
        } else {
            return Ok(format!("No pending approval for id '{}'", id));
        }
    }

    // 1. Get Settings
    let settings = {
        let settings = settings_state.0.lock().map_err(|e| e.to_string())?;
        settings.clone()
    };
    let provider = settings.provider.clone();
    let api_key = if !settings.openai_api_key.is_empty() {
        settings.openai_api_key.clone()
    } else {
        settings.api_key.clone()
    };
    let model = settings.model.clone();

    if provider != "openai" {
        return Ok(format!("Provider '{}' not supported yet.", provider));
    }

    if api_key.is_empty() {
        return Ok("Please set your OpenAI API Key in Settings.".to_string());
    }

    // 2. Resolve Session
    // If session_id provided, use it. Else check active session. If none, create temp/default.
    let active_session_id = {
        let mut current = session_state.current_session_id.lock().map_err(|e| e.to_string())?;
        if let Some(sid) = session_id {
            *current = Some(sid.clone());
            Some(sid)
        } else {
            current.clone()
        }
    };

    // Load History
    let mut history: Vec<Message>;
    if let Some(sid) = &active_session_id {
        // Load from disk/memory
        let dir = crate::session_manager::get_sessions_dir();
        let path = dir.join(format!("{}.json", sid));
        if path.exists() {
            let content = std::fs::read_to_string(path).unwrap_or_default();
            let session: Session = serde_json::from_str(&content).unwrap_or_else(|_| Session {
                id: sid.clone(), title: "Error".into(), messages: vec![], created_at: 0, updated_at: 0
            });
            history = session.messages;
        } else {
            history = Vec::new(); // Should not happen if created correctly
        }
    } else {
        // Fallback to legacy in-memory state
        history = state.history.lock().map_err(|e| e.to_string())?.clone();
    }

    // 3. Context & System Prompt
    let active_window = context::get_active_window_info().unwrap_or_else(|_| "Unknown".to_string());
    let cwd = working_dir.unwrap_or_else(|| ".".to_string());
    
    let system_prompt = format!(
        "You are DeskWork, an advanced desktop agent running on Windows. \
        Active Working Directory: '{}'. \
        Active Window: '{}'. \
        \
        CAPABILITIES & PERMISSIONS: \
        - File System: You have FULL permission to Read, Write, List, and Delete files. \
        - Shell Commands: You have FULL permission to execute shell commands (e.g., 'md', 'move'). \
        - Web & Research: You can `search_web` to open Google or `fetch_url` to read pages. \
        - Apps: You can `open_app` to launch files or applications. \
        - Input Simulation: You can `keyboard_type` to type, `keyboard_press` to press keys, `mouse_move` and `mouse_click` to control cursor. Use `wait` to pause. \
        - Vision: You can `get_screenshot` to see the screen and find where buttons are. \
        - Content Creation: You can `create_docx` for Word docs and `create_slide_deck` for presentations (HTML/Reveal.js). \
        - System: You can `get_system_stats` to check resources. \
        - Search: You can `search_files` (grep) to find text, or `find_file_smart` to find files by name/path. \
        \
        SAFETY & CONFIRMATION: \
        - Sensitive actions require approval. The system will emit an approval id; wait for the user to reply 'approve <id>' (or 'deny <id>'). \
        - Keep actions scoped to the working directory; out-of-scope paths need approval. \
        - Respect read-only mode and do not attempt writes/exec when enabled. \
        \
        PROTOCOL: \
        1. Share a brief plan. \
        2. Request approval; wait for 'approve <id>' before executing. \
        3. Execute only after approval. \
        \
        BROWSER AUTOMATION (Google Calendar/Gmail/etc): \
        1. Open URL: `open_app("https://calendar.google.com")` \
        2. Wait for load: `wait(5000)` \
        3. Look at screen: `get_screenshot()` (This gives you a base64 image) \
        4. Move Mouse to X,Y: `mouse_move(x, y)` \
        5. Click: `mouse_click("left")` \
        6. Type: `keyboard_type("Meeting with team")` \
        \
        TOOLS: \
        - set_plan(steps): Visual progress. \
        - complete_step(step_index). \
        - list_dir, read_file, write_file, execute_command. \
        - open_app(path), fetch_url(url), get_system_stats(), search_files(query, path), search_web(query). \
        - keyboard_type(text), keyboard_press(key), mouse_move(x,y), mouse_click(btn), get_screenshot(), wait(ms). \
        - create_docx(content, filename), create_slide_deck(content, filename), find_file_smart(query, path).",
        cwd, active_window
    );

    // Initialize System Prompt if empty
    if history.is_empty() {
        history.push(Message { role: "system".into(), content: Some(MessageContent::Text(system_prompt)), tool_calls: None, tool_call_id: None });
    }

    // Add User Message
    let user_content = if cwd != "." {
        format!("(Context: {}) {}", cwd, prompt)
    } else {
        prompt
    };
    history.push(Message { role: "user".into(), content: Some(MessageContent::Text(user_content)), tool_calls: None, tool_call_id: None });

    // 4. Execution Loop
    let client = Client::new();
    let tools = vec![
        json!({ "type": "function", "function": { "name": "set_plan", "description": "Create a visual plan", "parameters": { "type": "object", "properties": { "steps": { "type": "array", "items": { "type": "string" } } }, "required": ["steps"] } } }),
        json!({ "type": "function", "function": { "name": "complete_step", "description": "Mark step complete", "parameters": { "type": "object", "properties": { "step_index": { "type": "integer" } }, "required": ["step_index"] } } }),
        json!({ "type": "function", "function": { "name": "list_dir", "description": "List files", "parameters": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] } } }),
        json!({ "type": "function", "function": { "name": "read_file", "description": "Read file", "parameters": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] } } }),
        json!({ "type": "function", "function": { "name": "write_file", "description": "Write file", "parameters": { "type": "object", "properties": { "path": { "type": "string" }, "content": { "type": "string" } }, "required": ["path", "content"] } } }),
        json!({ "type": "function", "function": { "name": "execute_command", "description": "Run command", "parameters": { "type": "object", "properties": { "command": { "type": "string" }, "args": { "type": "array", "items": { "type": "string" } } }, "required": ["command", "args"] } } }),
        json!({ "type": "function", "function": { "name": "open_app", "description": "Open a file or app", "parameters": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] } } }),
        json!({ "type": "function", "function": { "name": "fetch_url", "description": "Fetch content from URL", "parameters": { "type": "object", "properties": { "url": { "type": "string" }, "expected_hash": { "type": "string", "description": "Optional SHA256 hash to verify content" } }, "required": ["url"] } } }),
        json!({ "type": "function", "function": { "name": "get_system_stats", "description": "Get CPU/Memory usage", "parameters": { "type": "object", "properties": {}, "required": [] } } }),
        json!({ "type": "function", "function": { "name": "search_files", "description": "Search text in files", "parameters": { "type": "object", "properties": { "query": { "type": "string" }, "path": { "type": "string" } }, "required": ["query", "path"] } } }),
        json!({ "type": "function", "function": { "name": "search_web", "description": "Search web (opens browser)", "parameters": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] } } }),
        json!({ "type": "function", "function": { "name": "keyboard_type", "description": "Simulate typing text", "parameters": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] } } }),
        json!({ "type": "function", "function": { "name": "keyboard_press", "description": "Simulate key press (Enter, Tab, etc)", "parameters": { "type": "object", "properties": { "key": { "type": "string" } }, "required": ["key"] } } }),
        json!({ "type": "function", "function": { "name": "mouse_move", "description": "Move mouse to coordinates", "parameters": { "type": "object", "properties": { "x": { "type": "integer" }, "y": { "type": "integer" } }, "required": ["x", "y"] } } }),
        json!({ "type": "function", "function": { "name": "mouse_click", "description": "Click mouse button", "parameters": { "type": "object", "properties": { "button": { "type": "string", "enum": ["left", "right", "middle"] } }, "required": ["button"] } } }),
        json!({ "type": "function", "function": { "name": "get_screenshot", "description": "Get current screen as base64 image", "parameters": { "type": "object", "properties": {}, "required": [] } } }),
        json!({ "type": "function", "function": { "name": "wait", "description": "Wait for N milliseconds", "parameters": { "type": "object", "properties": { "milliseconds": { "type": "integer" } }, "required": ["milliseconds"] } } }),
        json!({ "type": "function", "function": { "name": "create_docx", "description": "Create a Word DOCX file from text content", "parameters": { "type": "object", "properties": { "content": { "type": "string" }, "filename": { "type": "string" } }, "required": ["content", "filename"] } } }),
        json!({ "type": "function", "function": { "name": "create_slide_deck", "description": "Create a Reveal.js slide deck (HTML) from text", "parameters": { "type": "object", "properties": { "content": { "type": "string" }, "filename": { "type": "string" } }, "required": ["content", "filename"] } } }),
        json!({ "type": "function", "function": { "name": "find_file_smart", "description": "Recursively find files by name (fuzzy)", "parameters": { "type": "object", "properties": { "query": { "type": "string" }, "path": { "type": "string" } }, "required": ["query", "path"] } } })
    ];

    let mut final_response = String::new();
    
    'conversation: for _ in 0..10 { 
        let request_body = json!({
            "model": model,
            "messages": history,
            "tools": tools,
            "tool_choice": "auto"
        });

        let api_start = std::time::Instant::now();
        let mut last_err: Option<String> = None;
        let mut body_opt: Option<OpenAIChatResponse> = None;
        for attempt in 0..3 {
            let res = client.post("https://api.openai.com/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&request_body)
                .send()
                .await;

            match res {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        last_err = Some(format!("API status {}", resp.status()));
                    } else {
                        match resp.json::<OpenAIChatResponse>().await {
                            Ok(body) => {
                                body_opt = Some(body);
                                break;
                            }
                            Err(e) => last_err = Some(format!("Parse failed: {}", e)),
                        }
                    }
                }
                Err(e) => {
                    last_err = Some(format!("Request failed: {}", e));
                }
            }
            let backoff = 2u64.pow(attempt) * 300;
            tokio::time::sleep(Duration::from_millis(backoff)).await;
        }

        let body = match body_opt {
            Some(b) => b,
            None => {
                let msg = format!("Chat request failed: {}", last_err.unwrap_or_else(|| "unknown error".into()));
                let _ = app.emit("telemetry", TelemetryEvent {
                    tool: "openai_chat".into(),
                    status: "error".into(),
                    duration_ms: api_start.elapsed().as_millis(),
                    kind: "api".into(),
                });
                return Ok(format!("Offline or unavailable. {}", msg));
            }
        };
        let api_latency = api_start.elapsed().as_millis();
        let _ = app.emit("telemetry", TelemetryEvent {
            tool: "openai_chat".into(),
            status: "success".into(),
            duration_ms: api_latency,
            kind: "api".into(),
        });
        let choice = body.choices.first().ok_or("No response")?;
        let message = &choice.message;

        history.push(message.clone());

        if let Some(tool_calls) = &message.tool_calls {
            for tool_call in tool_calls {
                let function_name = &tool_call.function.name;
                let args: Value = serde_json::from_str(&tool_call.function.arguments).unwrap_or(json!({}));
                if let Some(reason) = approval_reason(function_name, &args, &working_dir, &settings) {
                    let dry_run = match function_name.as_str() {
                        "execute_command" => {
                            let cmd = args["command"].as_str().unwrap_or("");
                            let args_vec: Vec<String> = args["args"].as_array().map(|arr| arr.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
                            format!("Would run: {} {}", cmd, args_vec.join(" "))
                        }
                        "write_file" => {
                            let path = args["path"].as_str().unwrap_or("");
                            format!("Would write to {}", path)
                        }
                        _ => format!("Would run {}", function_name),
                    };
                    final_response = request_approval(
                        &approval_state,
                        &app,
                        function_name,
                        dry_run,
                        args.clone(),
                        working_dir.clone(),
                        reason,
                    );
                    history.push(Message {
                        role: "assistant".into(),
                        content: Some(MessageContent::Text(final_response.clone())),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                    break 'conversation;
                }

                let id = uuid::Uuid::new_v4().to_string();
                let tool_output = dispatch_tool(&app, function_name, &args, &working_dir, id.clone(), settings.structured_logs).await;

                history.push(Message {
                    role: "tool".into(),
                    content: Some(tool_output.unwrap_or_else(|e| MessageContent::Text(format!("Error: {}", e)))),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                });
            }
            continue;
        } else {
             // Handle response content which might be just text or null
            if let Some(content) = &message.content {
                match content {
                    MessageContent::Text(t) => final_response = t.clone(),
                    MessageContent::Parts(parts) => {
                        // Concatenate text parts for simple string return
                        final_response = parts.iter().filter_map(|p| p.text.clone()).collect::<Vec<_>>().join("\n");
                    }
                }
            }
            break;
        }
}

// Stream final response as tokens
    if !final_response.is_empty() {
        let mut first = true;
        for token in final_response.split_whitespace() {
            let sep = if first { "" } else { " " };
            let _ = app.emit("chat_stream", json!({"token": format!("{}{}", sep, token), "done": false}));
            first = false;
        }
        let _ = app.emit("chat_stream", json!({"done": true}));
    }

    if let Some(sid) = active_session_id {
        let sanitized = sanitize_history_for_storage(&history);
        let mut session = Session {
            id: sid.clone(),
            title: "Session".to_string(),
            messages: sanitized,
            created_at: 0,
            updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        };
        save_session_to_disk(&session).ok();
    } else {
        *state.history.lock().map_err(|e| e.to_string())? = history;
    }

    Ok(final_response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_scope() {
        let cwd = Some(std::env::current_dir().unwrap().to_string_lossy().to_string());
        let in_scope = std::env::current_dir().unwrap().join("Cargo.toml");
        assert!(!path_out_of_scope(&cwd, in_scope.to_string_lossy().as_ref()));
    }

    #[test]
    fn test_approval_reason_read_only() {
        let settings = crate::settings::AppSettings {
            api_key: "".into(),
            openai_api_key: "".into(),
            model: "gpt-4o".into(),
            read_only: true,
            structured_logs: false,
            provider: "openai".into(),
            reduced_motion: false,
            high_contrast: false,
        };
        let reason = approval_reason("write_file", &json!({"path": "test.txt"}), &None, &settings);
        assert!(reason.is_some());
    }

    #[test]
    fn test_sanitize_history_redacts_tool() {
        let history = vec![
            Message { role: "user".into(), content: Some(MessageContent::Text("hi".into())), tool_calls: None, tool_call_id: None },
            Message { role: "tool".into(), content: Some(MessageContent::Text("secret".into())), tool_calls: None, tool_call_id: None },
        ];
        let sanitized = sanitize_history_for_storage(&history);
        match sanitized[1].content.as_ref().unwrap() {
            MessageContent::Text(t) => assert_eq!(t, "Tool output omitted for privacy."),
            _ => panic!("expected text"),
        }
    }
}

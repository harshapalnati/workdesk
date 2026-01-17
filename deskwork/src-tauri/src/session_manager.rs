use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use crate::agent::{Message, MessageContent};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub pinned: bool,
}

pub struct SessionState {
    pub current_session_id: Mutex<Option<String>>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            current_session_id: Mutex::new(None),
        }
    }
}

// Helper to get storage path
pub fn get_sessions_dir() -> PathBuf {
    // In a real app, use dirs::data_dir() or similar. 
    // For this MVP, we use a local .deskwork/sessions folder.
    let path = PathBuf::from(".deskwork/sessions");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path
}

#[tauri::command]
pub fn list_sessions() -> Result<Vec<Session>, String> {
    let dir = get_sessions_dir();
    let mut sessions = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Ok(session) = serde_json::from_str::<Session>(&content) {
                    sessions.push(session);
                }
            }
        }
    }
    // Sort by updated_at desc
    sessions.sort_by(|a, b| {
        match (b.pinned, a.pinned) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.updated_at.cmp(&a.updated_at),
        }
    });
    Ok(sessions)
}

#[tauri::command]
pub fn create_session(title: String, state: tauri::State<'_, SessionState>) -> Result<Session, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    
    let session = Session {
        id: id.clone(),
        title: if title.is_empty() { "New Chat".to_string() } else { title },
        messages: Vec::new(),
        created_at: now,
        updated_at: now,
        pinned: false,
    };

    save_session_to_disk(&session)?;
    
    *state.current_session_id.lock().map_err(|e| e.to_string())? = Some(id);
    
    Ok(session)
}

#[tauri::command]
pub fn switch_session(session_id: String, state: tauri::State<'_, SessionState>) -> Result<Session, String> {
    let dir = get_sessions_dir();
    let path = dir.join(format!("{}.json", session_id));
    
    if !path.exists() {
        return Err("Session not found".to_string());
    }

    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let session: Session = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    *state.current_session_id.lock().map_err(|e| e.to_string())? = Some(session_id);

    Ok(session)
}

pub fn save_session_to_disk(session: &Session) -> Result<(), String> {
    let dir = get_sessions_dir();
    let path = dir.join(format!("{}.json", session.id));
    let json = serde_json::to_string_pretty(session).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

fn load_session(session_id: &str) -> Result<Session, String> {
    let dir = get_sessions_dir();
    let path = dir.join(format!("{}.json", session_id));
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let session: Session = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(session)
}

fn sanitize_messages(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .map(|m| {
            let mut clone = m.clone();
            if clone.role == "tool" {
                clone.content = Some(crate::agent::MessageContent::Text("[redacted tool output]".into()));
            }
            clone
        })
        .collect()
}

#[tauri::command]
pub fn rename_session(session_id: String, title: String) -> Result<Session, String> {
    let mut session = load_session(&session_id)?;
    session.title = if title.is_empty() { "Untitled Chat".into() } else { title };
    session.updated_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    save_session_to_disk(&session)?;
    Ok(session)
}

#[tauri::command]
pub fn toggle_pin(session_id: String, pinned: bool) -> Result<Session, String> {
    let mut session = load_session(&session_id)?;
    session.pinned = pinned;
    session.updated_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    save_session_to_disk(&session)?;
    Ok(session)
}

#[tauri::command]
pub fn search_sessions(query: String) -> Result<Vec<Session>, String> {
    let q = query.to_lowercase();
    let mut sessions = list_sessions()?;
    if !q.is_empty() {
        sessions.retain(|s| s.title.to_lowercase().contains(&q) || s.id.to_lowercase().contains(&q));
    }
    // pinned first, then updated_at desc
    sessions.sort_by(|a, b| {
        match (b.pinned, a.pinned) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.updated_at.cmp(&a.updated_at),
        }
    });
    Ok(sessions)
}

#[tauri::command]
pub fn export_sessions() -> Result<String, String> {
    let sessions = list_sessions()?;
    let redacted: Vec<Session> = sessions
        .into_iter()
        .map(|mut s| {
            s.messages = sanitize_messages(&s.messages);
            s
        })
        .collect();
    serde_json::to_string_pretty(&redacted).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_sessions(payload: String) -> Result<usize, String> {
    let imported: Vec<Session> = serde_json::from_str(&payload).map_err(|e| e.to_string())?;
    let mut count = 0;
    for mut session in imported {
        session.messages = sanitize_messages(&session.messages);
        save_session_to_disk(&session)?;
        count += 1;
    }
    Ok(count)
}


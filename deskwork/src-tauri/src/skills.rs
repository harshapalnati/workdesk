use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub tools: Vec<String>, // List of tool names this skill controls
}

pub struct SkillState {
    pub skills: Mutex<HashMap<String, Skill>>,
}

impl Default for SkillState {
    fn default() -> Self {
        let mut skills = HashMap::new();
        
        skills.insert("file_system".to_string(), Skill {
            id: "file_system".to_string(),
            name: "File System".to_string(),
            description: "Read, write, and manage files.".to_string(),
            enabled: true,
            tools: vec!["read_file".into(), "write_file".into(), "list_dir".into(), "find_file_smart".into(), "search_files".into()],
        });

        skills.insert("terminal".to_string(), Skill {
            id: "terminal".to_string(),
            name: "Terminal".to_string(),
            description: "Execute shell commands.".to_string(),
            enabled: true,
            tools: vec!["execute_command".into()],
        });

        skills.insert("browser".to_string(), Skill {
            id: "browser".to_string(),
            name: "Web Browser".to_string(),
            description: "Search the web and read pages.".to_string(),
            enabled: true,
            tools: vec!["search_web".into(), "fetch_url".into()],
        });

        skills.insert("automation".to_string(), Skill {
            id: "automation".to_string(),
            name: "UI Automation".to_string(),
            description: "Control mouse and keyboard.".to_string(),
            enabled: true,
            tools: vec!["mouse_move".into(), "mouse_click".into(), "keyboard_type".into(), "keyboard_press".into(), "get_screenshot".into(), "wait".into()],
        });

        skills.insert("apps".to_string(), Skill {
            id: "apps".to_string(),
            name: "Applications".to_string(),
            description: "Launch applications and create documents.".to_string(),
            enabled: true,
            tools: vec!["open_app".into(), "create_docx".into(), "create_slide_deck".into()],
        });

        skills.insert("system".to_string(), Skill {
            id: "system".to_string(),
            name: "System".to_string(),
            description: "Check system stats.".to_string(),
            enabled: true,
            tools: vec!["get_system_stats".into()],
        });

        Self {
            skills: Mutex::new(skills),
        }
    }
}

#[tauri::command]
pub fn list_skills(state: State<'_, SkillState>) -> Result<Vec<Skill>, String> {
    let skills = state.skills.lock().unwrap();
    let mut list: Vec<Skill> = skills.values().cloned().collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(list)
}

#[tauri::command]
pub fn toggle_skill(state: State<'_, SkillState>, id: String, enabled: bool) -> Result<(), String> {
    let mut skills = state.skills.lock().unwrap();
    if let Some(skill) = skills.get_mut(&id) {
        skill.enabled = enabled;
        Ok(())
    } else {
        Err("Skill not found".to_string())
    }
}

pub fn is_tool_enabled(state: &State<'_, SkillState>, tool_name: &str) -> bool {
    let skills = state.skills.lock().unwrap();
    // Special internal tools always enabled
    if tool_name == "set_plan" || tool_name == "complete_step" {
        return true;
    }
    
    for skill in skills.values() {
        if skill.tools.contains(&tool_name.to_string()) {
            return skill.enabled;
        }
    }
    true // Default allow if not categorized (or deny? allow for now)
}


use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{State, Manager};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Template {
    pub id: String,
    pub title: String,
    pub prompt: String,
}

pub struct TemplateState {
    pub templates: Mutex<Vec<Template>>,
    pub path: PathBuf,
}

impl TemplateState {
    pub fn new(app_handle: &tauri::AppHandle) -> Self {
        let config_dir = app_handle.path().app_config_dir().unwrap_or_default();
        if !config_dir.exists() {
            let _ = fs::create_dir_all(&config_dir);
        }
        let path = config_dir.join("templates.json");
        
        let templates = if path.exists() {
            let content = fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_else(|_| Vec::new())
        } else {
            // Default templates
            vec![
                Template {
                    id: uuid::Uuid::new_v4().to_string(),
                    title: "Code Review".to_string(),
                    prompt: "Please review the code in the current directory. Look for bugs, security issues, and performance improvements.".to_string(),
                },
                Template {
                    id: uuid::Uuid::new_v4().to_string(),
                    title: "Summarize Project".to_string(),
                    prompt: "Read the README.md and the file structure, then summarize what this project does.".to_string(),
                }
            ]
        };

        Self {
            templates: Mutex::new(templates),
            path,
        }
    }

    fn save(&self) {
        let templates = self.templates.lock().unwrap();
        let content = serde_json::to_string_pretty(&*templates).unwrap_or_default();
        let _ = fs::write(&self.path, content);
    }
}

#[tauri::command]
pub fn list_templates(state: State<'_, TemplateState>) -> Result<Vec<Template>, String> {
    Ok(state.templates.lock().unwrap().clone())
}

#[tauri::command]
pub fn save_template(state: State<'_, TemplateState>, title: String, prompt: String) -> Result<Template, String> {
    let template = Template {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        prompt,
    };
    state.templates.lock().unwrap().push(template.clone());
    state.save();
    Ok(template)
}

#[tauri::command]
pub fn delete_template(state: State<'_, TemplateState>, id: String) -> Result<(), String> {
    let mut templates = state.templates.lock().unwrap();
    templates.retain(|t| t.id != id);
    drop(templates); // release lock before save
    state.save();
    Ok(())
}


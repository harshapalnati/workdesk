use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Mutex;
use tauri::State;
use keyring::Entry;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub api_key: String, // deprecated single key
    #[serde(default)]
    pub openai_api_key: String,
    pub model: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub structured_logs: bool,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub reduced_motion: bool,
    #[serde(default)]
    pub high_contrast: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            api_key: "".to_string(),
            openai_api_key: "".to_string(),
            model: "gpt-3.5-turbo".to_string(),
            read_only: false,
            structured_logs: false,
            provider: default_provider(),
            reduced_motion: false,
            high_contrast: false,
        }
    }
}

fn default_provider() -> String {
    "openai".into()
}

pub struct SettingsState(pub Mutex<AppSettings>);

const SETTINGS_FILE: &str = "deskwork_settings.json";
const KEYRING_SERVICE: &str = "deskwork";
const KEYRING_USER: &str = "openai_api_key";

fn load_api_key_from_keyring() -> Option<String> {
    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER).ok()?;
    entry.get_password().ok()
}

fn save_api_key_to_keyring(api_key: &str) -> Result<(), String> {
    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())?;
    if api_key.is_empty() {
        let _ = entry.delete_password(); // Best-effort cleanup
        Ok(())
    } else {
        entry.set_password(api_key).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn save_settings(settings: AppSettings, state: State<'_, SettingsState>) -> Result<(), String> {
    // 1. Update in-memory state
    let mut current_settings = state.0.lock().map_err(|e| e.to_string())?;
    *current_settings = settings.clone();

    // 2. Persist secret to keyring, non-secrets to disk
    let key_to_store = if !settings.openai_api_key.is_empty() {
        settings.openai_api_key.clone()
    } else {
        settings.api_key.clone()
    };
    save_api_key_to_keyring(&key_to_store)?;
    let mut disk_settings = settings.clone();
    disk_settings.api_key = "".into(); // legacy
    disk_settings.openai_api_key = "".into(); // Do not write secrets to disk
    let json = serde_json::to_string_pretty(&disk_settings).map_err(|e| e.to_string())?;
    fs::write(SETTINGS_FILE, json).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn get_settings(state: State<'_, SettingsState>) -> Result<AppSettings, String> {
    let mut settings = state.0.lock().map_err(|e| e.to_string())?.clone();
    // Hydrate api key from keyring so the frontend can show presence (masked)
    if let Some(stored) = load_api_key_from_keyring() {
        settings.openai_api_key = stored.clone();
        settings.api_key = stored; // legacy for backward compatibility
    }
    Ok(settings)
}

// Helper to load from disk on startup
pub fn load_initial_settings() -> AppSettings {
    let mut settings = if let Ok(content) = fs::read_to_string(SETTINGS_FILE) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        AppSettings::default()
    };

    if let Some(api_key) = load_api_key_from_keyring() {
        settings.openai_api_key = api_key.clone();
        settings.api_key = api_key; // legacy
    }

    settings
}

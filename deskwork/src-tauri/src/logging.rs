use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use chrono::Local;
use tauri::AppHandle;

pub fn get_log_dir() -> PathBuf {
    // Use the current working directory (project root in dev)
    // or fallback to a sensible location if CWD fails.
    // Ideally, this creates .deskwork/logs in the folder where the user runs the app.
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let log_dir = cwd.join(".deskwork").join("logs");
    if !log_dir.exists() {
        let _ = fs::create_dir_all(&log_dir);
    }
    log_dir
}

pub fn log(_app_handle: &AppHandle, session_id: &str, level: &str, message: &str) {
    let dir = get_log_dir();
    let path = dir.join(format!("{}.log", session_id));
    
    let now = Local::now();
    let timestamp = now.format("%Y-%m-%d %H:%M:%S%.3f");
    
    let line = format!("[{}] [{}] {}\n", timestamp, level, message);
    
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

#[tauri::command]
pub fn get_session_log(_app: AppHandle, session_id: String) -> Result<String, String> {
    let dir = get_log_dir();
    let path = dir.join(format!("{}.log", session_id));
    if path.exists() {
        fs::read_to_string(path).map_err(|e| e.to_string())
    } else {
        Ok("No logs found for this session.".to_string())
    }
}

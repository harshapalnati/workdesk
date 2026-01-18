use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{State, Manager};
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuditEntry {
    pub ts: u64,
    pub tool: String,
    pub status: String,
    pub action: String,
    pub duration_ms: u128,
    pub working_dir: Option<String>,
    pub prev_hash: String,
    pub hash: String,
}

#[derive(Default)]
pub struct AuditState {
    pub log_path: Mutex<Option<PathBuf>>,
}

pub fn init(app_handle: &tauri::AppHandle) -> AuditState {
    let config_dir = app_handle.path().app_config_dir().unwrap_or_default();
    if !config_dir.exists() {
        let _ = fs::create_dir_all(&config_dir);
    }
    let log_path = config_dir.join("audit.jsonl");
    AuditState {
        log_path: Mutex::new(Some(log_path)),
    }
}

fn read_last_hash(path: &PathBuf) -> String {
    if !path.exists() {
        return String::new();
    }
    if let Ok(file) = fs::File::open(path) {
        let reader = BufReader::new(file);
        if let Some(Ok(line)) = reader.lines().last() {
            if let Ok(entry) = serde_json::from_str::<AuditEntry>(&line) {
                return entry.hash;
            }
        }
    }
    String::new()
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn compute_hash(prev_hash: &str, tool: &str, status: &str, action: &str, duration_ms: u128, working_dir: &Option<String>, ts: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(tool.as_bytes());
    hasher.update(status.as_bytes());
    hasher.update(action.as_bytes());
    hasher.update(duration_ms.to_string().as_bytes());
    if let Some(wd) = working_dir {
        hasher.update(wd.as_bytes());
    }
    hasher.update(ts.to_string().as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn append_audit(
    tool: &str,
    status: &str,
    action: &str,
    duration_ms: u128,
    working_dir: Option<String>,
    _structured: bool,
    state: &State<'_, AuditState>,
) -> Result<(), String> {
    let path_guard = state.log_path.lock().unwrap();
    let path = match &*path_guard {
        Some(p) => p,
        None => return Err("Audit log path not initialized".to_string()),
    };

    let prev_hash = read_last_hash(path);
    let ts = now_ts();
    let hash = compute_hash(&prev_hash, tool, status, action, duration_ms, &working_dir, ts);
    
    let entry = AuditEntry {
        ts,
        tool: tool.to_string(),
        status: status.to_string(),
        action: action.to_string(),
        duration_ms,
        working_dir,
        prev_hash,
        hash,
    };

    let line = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
    
    // Retry logic for file access
    for _ in 0..3 {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            if writeln!(file, "{}", line).is_ok() {
                return Ok(());
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    Ok(())
}

#[tauri::command]
pub fn get_audit_log(state: State<'_, AuditState>) -> Result<Vec<AuditEntry>, String> {
    let path_guard = state.log_path.lock().unwrap();
    if let Some(path) = &*path_guard {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let entries: Vec<AuditEntry> = content
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        // Return last 100 entries reversed
        Ok(entries.into_iter().rev().take(100).collect())
    } else {
        Ok(Vec::new())
    }
}

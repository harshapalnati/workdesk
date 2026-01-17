use sha2::{Digest, Sha256};
use serde::{Serialize, Deserialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Debug)]
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

fn audit_path() -> PathBuf {
    let path = PathBuf::from(".deskwork/audit.log");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    path
}

fn structured_path() -> PathBuf {
    let path = PathBuf::from(".deskwork/audit_structured.log");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    path
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

pub fn append_audit(tool: &str, status: &str, action: &str, duration_ms: u128, working_dir: Option<String>, structured: bool) -> Result<(), String> {
    let path = audit_path();
    let prev_hash = read_last_hash(&path);
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
    let mut file = OpenOptions::new().create(true).append(true).open(&path).map_err(|e| e.to_string())?;
    writeln!(file, "{}", line).map_err(|e| e.to_string())?;

    if structured {
        let s_path = structured_path();
        let mut s_file = OpenOptions::new().create(true).append(true).open(&s_path).map_err(|e| e.to_string())?;
        writeln!(s_file, "{}", line).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn read_audit_log() -> Result<String, String> {
    let path = audit_path();
    fs::read_to_string(path).map_err(|e| e.to_string())
}

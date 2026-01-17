use std::fs;
use std::process::Command;
use std::path::Path;
use serde::Serialize;
use sysinfo::{System, Networks, Components};
use enigo::{Enigo, Key, Keyboard, Mouse, Button, Settings, Direction, Coordinate};
use std::thread;
use std::time::Duration;
use screenshots::Screen;
use std::io::Cursor;
use base64::Engine;
use sha2::Digest; // Add this import

// We need to use the image crate types that screenshots expects, or handle conversion
// screenshots 0.8.4 uses image 0.24 internally.
use image::ImageFormat; 

use docx_rs::*;
use walkdir::WalkDir;

#[derive(Serialize)]
pub struct FileNode {
    name: String,
    path: String,
    is_dir: bool,
    children: Option<Vec<FileNode>>,
}

#[derive(Serialize)]
pub struct SystemStats {
    pub cpu_usage: f32,
    pub total_memory: u64,
    pub used_memory: u64,
    pub platform: String,
}

#[tauri::command]
pub fn read_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_file(path: String, content: String) -> Result<(), String> {
    fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_dir(path: String) -> Result<Vec<String>, String> {
    let paths = fs::read_dir(path).map_err(|e| e.to_string())?;
    
    let mut entries = Vec::new();
    for path in paths {
        if let Ok(entry) = path {
            if let Ok(file_name) = entry.file_name().into_string() {
                entries.push(file_name);
            }
        }
    }
    Ok(entries)
}

#[tauri::command]
pub fn execute_command(command: String, args: Vec<String>, cwd: Option<String>) -> Result<String, String> {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("powershell");
        c.arg("-Command")
            .arg(format!("{} {}", command, args.join(" ")));
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c")
            .arg(format!("{} {}", command, args.join(" ")));
        c
    };

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let output = cmd.output().map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[tauri::command]
pub fn get_file_tree(path: String) -> Result<Vec<FileNode>, String> {
    read_dir_recursive(&path, 0)
}

fn read_dir_recursive(path: &str, depth: usize) -> Result<Vec<FileNode>, String> {
    if depth > 3 { // Limit depth for performance
        return Ok(Vec::new());
    }

    let mut nodes = Vec::new();
    let entries = fs::read_dir(path).map_err(|e| e.to_string())?;

    for entry in entries {
        if let Ok(entry) = entry {
            let file_name = entry.file_name().into_string().unwrap_or_default();
            let file_path = entry.path().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            
            if file_name.starts_with('.') || file_name == "node_modules" || file_name == "target" {
                continue;
            }

            let children = if is_dir {
                Some(read_dir_recursive(&file_path, depth + 1).unwrap_or_default())
            } else {
                None
            };

            nodes.push(FileNode {
                name: file_name,
                path: file_path,
                is_dir,
                children,
            });
        }
    }
    
    nodes.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });

    Ok(nodes)
}

// --- NEW CAPABILITIES ---

#[tauri::command]
pub fn open_app(path: String) -> Result<(), String> {
    open::that(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_system_stats() -> Result<SystemStats, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    Ok(SystemStats {
        cpu_usage: sys.global_cpu_usage(),
        total_memory: sys.total_memory(),
        used_memory: sys.used_memory(),
        platform: System::name().unwrap_or("Unknown".to_string()),
    })
}

#[tauri::command]
pub async fn fetch_url(url: String, expected_hash: Option<String>) -> Result<String, String> {
    let client = reqwest::Client::new();
    let res = client.get(&url)
        .header("User-Agent", "DeskWork-Agent/1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let bytes = res.bytes().await.map_err(|e| e.to_string())?;
    if let Some(hash) = expected_hash {
        let mut hasher = sha2::Sha256::new();
        hasher.update(&bytes);
        let digest = format!("{:x}", hasher.finalize());
        if digest != hash.to_lowercase() {
            return Err(format!("Hash mismatch: expected {}, got {}", hash, digest));
        }
    }
    let text = String::from_utf8_lossy(&bytes).to_string();
    let clean = ammonia::clean(&text);
    Ok(clean.chars().take(10000).collect()) 
}

#[tauri::command]
pub fn search_files(query: String, path: String) -> Result<String, String> {
    let output = if cfg!(target_os = "windows") {
        Command::new("findstr")
            .args(&["/S", "/I", "/M", &query, &format!("{}\\*", path)]) 
            .output()
            .map_err(|e| e.to_string())?
    } else {
        Command::new("grep")
            .args(&["-r", "-l", &query, &path])
            .output()
            .map_err(|e| e.to_string())?
    };

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Ok("No matches found or error.".to_string())
    }
}

#[tauri::command]
pub fn search_web(query: String) -> Result<(), String> {
    let url = format!("https://www.google.com/search?q={}", urlencoding::encode(&query));
    open::that(url).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn keyboard_type(text: String) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.text(&text).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn keyboard_press(key: String) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    let key_to_press = match key.to_lowercase().as_str() {
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "space" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "escape" | "esc" => Key::Escape,
        "up" | "arrowup" => Key::UpArrow,
        "down" | "arrowdown" => Key::DownArrow,
        "left" | "arrowleft" => Key::LeftArrow,
        "right" | "arrowright" => Key::RightArrow,
        _ => return Err(format!("Unsupported key: {}", key)),
    };
    enigo.key(key_to_press, Direction::Click).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn wait(milliseconds: u64) -> Result<(), String> {
    thread::sleep(Duration::from_millis(milliseconds));
    Ok(())
}

#[tauri::command]
pub fn mouse_move(x: i32, y: i32) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.move_mouse(x, y, Coordinate::Abs).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn mouse_click(button: String) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    let btn = match button.to_lowercase().as_str() {
        "left" => Button::Left,
        "right" => Button::Right,
        "middle" => Button::Middle,
        _ => return Err(format!("Unsupported button: {}", button)),
    };
    enigo.button(btn, Direction::Click).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn create_docx(content: String, filename: String) -> Result<(), String> {
    let path = std::path::Path::new(&filename);
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;

    let mut doc = Docx::new();
    
    // Simple markdown-like parsing: split by lines, check for headers
    for line in content.lines() {
        if line.starts_with("# ") {
             doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().size(48).add_text(line.trim_start_matches("# "))));
        } else if line.starts_with("## ") {
             doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().size(36).add_text(line.trim_start_matches("## "))));
        } else {
             doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().add_text(line)));
        }
    }

    doc.build().pack(file).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn create_slide_deck(content: String, filename: String) -> Result<(), String> {
    // Basic Reveal.js template
    let mut html = String::from(r#"<!doctype html>
<html>
    <head>
        <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/reveal.js/4.3.1/reveal.min.css">
        <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/reveal.js/4.3.1/theme/black.min.css">
    </head>
    <body>
        <div class="reveal">
            <div class="slides">
"#);

    // Split content by "---" for slides
    for slide_content in content.split("---") {
        html.push_str("<section>");
        for line in slide_content.lines() {
             if line.starts_with("# ") {
                html.push_str(&format!("<h1>{}</h1>", line.trim_start_matches("# ")));
             } else if line.trim().is_empty() {
                 continue;
             } else {
                html.push_str(&format!("<p>{}</p>", line));
             }
        }
        html.push_str("</section>");
    }

    html.push_str(r#"
            </div>
        </div>
        <script src="https://cdnjs.cloudflare.com/ajax/libs/reveal.js/4.3.1/reveal.min.js"></script>
        <script>
            Reveal.initialize();
        </script>
    </body>
</html>"#);

    fs::write(filename, html).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn find_file_smart(query: String, path: String) -> Result<String, String> {
    let mut matches = Vec::new();
    let walker = WalkDir::new(&path).into_iter();

    for entry in walker.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy();
        if name.to_lowercase().contains(&query.to_lowercase()) {
            matches.push(entry.path().to_string_lossy().to_string());
            if matches.len() >= 10 { break; } // Limit results
        }
    }

    if matches.is_empty() {
        Ok("No matching files found.".to_string())
    } else {
        Ok(matches.join("\n"))
    }
}

#[tauri::command]
pub fn get_screenshot() -> Result<String, String> {
    let screens = Screen::all().map_err(|e| e.to_string())?;
    let screen = screens.first().ok_or("No screen found")?;
    let image = screen.capture().map_err(|e| e.to_string())?;
    
    // screenshots 0.8 returns an image::RgbaImage. 
    // We need to write this to a buffer as PNG.
    let mut bytes: Vec<u8> = Vec::new();
    
    // The 'image' dependency in Cargo.toml is 0.24, which matches screenshots 0.8 dependency.
    // However, if there's still a mismatch or issue with From<ImageFormat>, we can explicitely use PNG encoder
    // or rely on dynamic image.
    
    image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).map_err(|e| e.to_string())?;
    
    let base64_str = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:image/png;base64,{}", base64_str))
}

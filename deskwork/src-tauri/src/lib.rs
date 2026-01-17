// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

mod commands;
mod agent;
mod settings;
mod context;
mod session_manager;
mod audit;

use agent::AgentState;
use settings::{SettingsState, load_initial_settings};
use session_manager::SessionState;
use agent::ApprovalState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_settings = load_initial_settings();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AgentState::default())
        .manage(ApprovalState::default())
        .manage(SettingsState(std::sync::Mutex::new(initial_settings)))
        .manage(SessionState::default())
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::read_file,
            commands::write_file,
            commands::list_dir,
            commands::execute_command,
            commands::get_file_tree,
            commands::open_app,
            commands::get_system_stats,
            commands::fetch_url,
            commands::search_files,
            commands::search_web,
            commands::keyboard_type,
            commands::keyboard_press,
            commands::mouse_move,
            commands::mouse_click,
            commands::get_screenshot,
            commands::create_docx,
            commands::create_slide_deck,
            commands::find_file_smart,
            commands::wait,
            agent::chat,
            settings::save_settings,
            settings::get_settings,
            session_manager::list_sessions,
            session_manager::create_session,
            session_manager::switch_session
            ,
            session_manager::rename_session,
            session_manager::toggle_pin,
            session_manager::search_sessions,
            session_manager::export_sessions,
            session_manager::import_sessions,
            audit::read_audit_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

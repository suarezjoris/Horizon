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

/// Max bytes of plugin stdout fed back to the LLM. Larger output is truncated so one
/// chatty plugin can't blow the model's context window.
pub const PLUGIN_OUTPUT_LIMIT: usize = 32 * 1024;

/// Mount point where Horizon's vault is bound read-only inside every plugin sandbox.
/// Plugins read their data from here instead of reaching into $HOME (which is not bound).
pub const SANDBOX_VAULT: &str = "/vault";

/// Truncate plugin output to PLUGIN_OUTPUT_LIMIT on a char boundary, appending a note.
fn truncate_output(s: &str) -> String {
    if s.len() <= PLUGIN_OUTPUT_LIMIT {
        return s.to_string();
    }
    let mut end = PLUGIN_OUTPUT_LIMIT;
    while end > 0 && !s.is_char_boundary(end) { end -= 1; }
    format!("{}\n…[truncated {} bytes]", &s[..end], s.len() - end)
}

pub struct PluginRegistry {
    pub plugins: HashMap<String, LoadedPlugin>,
    pub load_errors: Vec<String>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { plugins: HashMap::new(), load_errors: Vec::new() }
    }

    pub fn scan_and_load(&mut self, vault_path: &str) {
        self.plugins.clear();
        self.load_errors.clear();
        let plugin_dir = Path::new(vault_path).join("plugins");
        // tool.name -> plugin.name, to reject duplicate tool names the LLM would otherwise see twice.
        let mut tool_names: HashMap<String, String> = HashMap::new();

        let entries = match std::fs::read_dir(&plugin_dir) {
            Ok(e) => e,
            Err(_) => return, // no plugins dir yet is not an error
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let manifest_path = path.join("plugin.json");
            if !manifest_path.exists() { continue; }
            let dir_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

            let content = match std::fs::read_to_string(&manifest_path) {
                Ok(c) => c,
                Err(e) => { self.load_errors.push(format!("{dir_name}: unreadable plugin.json: {e}")); continue; }
            };
            let manifest = match serde_json::from_str::<PluginManifest>(&content) {
                Ok(m) => m,
                Err(e) => { self.load_errors.push(format!("{dir_name}: invalid plugin.json: {e}")); continue; }
            };
            if let Some(tool) = &manifest.tool {
                if let Some(owner) = tool_names.get(&tool.name) {
                    self.load_errors.push(format!(
                        "{}: tool name '{}' already provided by '{}' — skipped",
                        manifest.name, tool.name, owner));
                    continue;
                }
                tool_names.insert(tool.name.clone(), manifest.name.clone());
            }
            self.plugins.insert(manifest.name.clone(), LoadedPlugin { manifest, dir: path });
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

        // Horizon injects context: plugins read args + the vault mount, never $HOME.
        let host_vault = crate::settings::load().vault_path;
        let payload = serde_json::json!({ "args": args, "vault": SANDBOX_VAULT });
        let payload_json = serde_json::to_string(&payload).unwrap();

        let mut bwrap_args = vec![
            "--ro-bind".to_string(), "/usr".to_string(), "/usr".to_string(),
            "--ro-bind".to_string(), "/lib".to_string(), "/lib".to_string(),
            "--ro-bind".to_string(), "/lib64".to_string(), "/lib64".to_string(),
            "--ro-bind".to_string(), "/bin".to_string(), "/bin".to_string(),
            "--tmpfs".to_string(), "/tmp".to_string(),
            "--proc".to_string(), "/proc".to_string(),
            "--dev".to_string(), "/dev".to_string(),
            "--bind".to_string(), plugin.dir.to_string_lossy().into_owned(), "/plugin".to_string(),
            // Vault bound read-only so plugins can read mirrored app state (Objectives, etc.).
            "--ro-bind".to_string(), host_vault, SANDBOX_VAULT.to_string(),
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
            let _ = stdin.write_all(payload_json.as_bytes()).await;
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
        
        let stdout = truncate_output(&String::from_utf8_lossy(&output.stdout));
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
pub async fn reload_plugins(state: tauri::State<'_, PluginState>) -> Result<Value, String> {
    let mut registry = state.write().await;
    let settings = crate::settings::load();
    registry.scan_and_load(&settings.vault_path);
    Ok(serde_json::json!({
        "loaded": registry.plugins.keys().cloned().collect::<Vec<_>>(),
        "errors": registry.load_errors,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_keeps_short_output() {
        assert_eq!(truncate_output("hello"), "hello");
    }

    #[test]
    fn truncate_caps_long_output() {
        let big = "x".repeat(PLUGIN_OUTPUT_LIMIT + 100);
        let out = truncate_output(&big);
        assert!(out.len() < big.len());
        assert!(out.contains("truncated"));
    }

    #[test]
    fn scan_records_invalid_manifest_and_skips_dup_tool_names() {
        let tmp = std::env::temp_dir().join(format!("horizon_plugins_test_{}", std::process::id()));
        let pdir = tmp.join("plugins");
        let mk = |name: &str, body: &str| {
            let d = pdir.join(name);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("plugin.json"), body).unwrap();
        };
        let tool = |pname: &str, tname: &str| format!(
            r#"{{"name":"{pname}","version":"1","description":"d","type":"tool",
            "tool":{{"name":"{tname}","description":"d","parameters":{{}},"runtime":"python3","script":"t.py"}}}}"#);
        mk("a", &tool("a", "shared"));
        mk("b", &tool("b", "shared")); // duplicate tool name -> skipped
        mk("bad", "{ not json");        // invalid -> recorded

        let mut reg = PluginRegistry::new();
        reg.scan_and_load(tmp.to_str().unwrap());

        assert_eq!(reg.plugins.len(), 1, "dup tool name must be skipped");
        assert!(reg.load_errors.iter().any(|e| e.contains("invalid plugin.json")));
        assert!(reg.load_errors.iter().any(|e| e.contains("already provided")));
        std::fs::remove_dir_all(&tmp).ok();
    }
}

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    pub command: String,
    pub args: Vec<String>,
    pub installed: bool,
    pub enabled: bool,
}

// Fetch dynamic registry of MCP servers from Anthropic's GitHub
#[tauri::command]
pub async fn get_mcp_store() -> Result<Vec<McpServerConfig>, String> {
    let s = crate::settings::load();
    let enabled = |id: &str| s.mcp_enabled.get(id).copied().unwrap_or(false);

    let client = reqwest::Client::new();
    let res = client
        .get("https://api.github.com/repos/modelcontextprotocol/servers/contents/src")
        .header("User-Agent", "Horizon-Agent")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let items: Vec<serde_json::Value> = res.json().await.map_err(|e| e.to_string())?;

    let mut servers = Vec::new();
    for item in items {
        if let Some(name) = item["name"].as_str() {
            if item["type"] == "dir" {
                let id = name.to_string();
                
                let mut cmd = "npx".to_string();
                let mut args = vec!["-y".to_string(), format!("@modelcontextprotocol/server-{}", id)];
                match id.as_str() {
                    "sqlite" => args.push("database.db".to_string()),
                    "filesystem" => args.push("/".to_string()),
                    "postgres" => args.push("postgresql://localhost/mydb".to_string()),
                    "git" => {
                        cmd = "uvx".to_string();
                        args = vec!["mcp-server-git".to_string(), "--repository".to_string(), ".".to_string()];
                    },
                    "time" => {
                        cmd = "uvx".to_string();
                        args = vec!["mcp-server-time".to_string()];
                    },
                    "fetch" => {
                        cmd = "uvx".to_string();
                        args = vec!["mcp-server-fetch".to_string()];
                    },
                    _ => {}
                }

                servers.push(McpServerConfig {
                    id: id.clone(),
                    name: format!("{} (Officiel)", id.to_uppercase()),
                    description: format!("Serveur MCP officiel Anthropic pour {}.", id),
                    command: cmd,
                    args,
                    installed: enabled(&id),
                    enabled: enabled(&id),
                });
            }
        }
    }

    Ok(servers)
}

#[allow(dead_code)]
pub fn call_mcp_bridge(method: &str, params: Value, server_cmd: &str, server_args: &[String]) -> Result<String, String> {
    let mut args = vec![
        "mcp_bridge.py".to_string(),
        method.to_string(),
        params.to_string(),
        server_cmd.to_string()
    ];
    args.extend_from_slice(server_args);

    // Call the python script in the .venv
    let output = Command::new(".venv/bin/python")
        .args(&args)
        .current_dir(std::path::PathBuf::from("/home/joris/Projects/Horizon"))
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[tauri::command]
pub fn toggle_mcp_server(id: String) -> Result<bool, String> {
    let mut s = crate::settings::load();
    let current = s.mcp_enabled.get(&id).copied().unwrap_or(false);
    let new_val = !current;
    s.mcp_enabled.insert(id, new_val);
    crate::settings::save_settings(s.clone()).map_err(|e| e.to_string())?;
    Ok(new_val)
}

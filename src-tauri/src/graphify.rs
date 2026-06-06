use std::process::Command;
use crate::settings;

fn get_graphify_cmd() -> String {
    // Try to find graphify locally, fallback to user bin
    if let Ok(home) = std::env::var("HOME") {
        let fallback = format!("{}/.local/bin/graphify", home);
        if std::path::Path::new(&fallback).exists() {
            return fallback;
        }
    }
    "graphify".to_string()
}

#[tauri::command]
pub async fn run_graphify() -> Result<String, String> {
    let s = settings::load();
    let vault_path = s.vault_path.clone();
    
    // Using a dedicated directory for the memory graph
    let base_out_dir = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join("graphify-out")
        .join("vault");

    std::fs::create_dir_all(&base_out_dir).map_err(|e| e.to_string())?;

    let graphify_bin = get_graphify_cmd();

    // 1. Extract semantic data
    let status_extract = Command::new(&graphify_bin)
        .arg("extract")
        .arg(&vault_path)
        .arg("--backend")
        .arg("ollama")
        .arg("--model")
        .arg("qwen2.5-coder:14b")
        .arg("--out")
        .arg(&base_out_dir)
        .status()
        .map_err(|err| format!("Failed to execute graphify extract: {}", err))?;

    if !status_extract.success() {
        return Err(format!("Graphify extract exited with status: {}", status_extract));
    }

    // Graphify extract creates a subfolder "graphify-out" inside the requested --out directory
    let inner_out_dir = base_out_dir.join("graphify-out");
    let graph_json = inner_out_dir.join("graph.json");

    std::fs::read_to_string(&graph_json).map_err(|e| format!("Failed to read graph.json: {}", e))
}


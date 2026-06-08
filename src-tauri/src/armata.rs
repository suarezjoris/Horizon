use std::fs;
use reqwest;

// --- ARMATA COMMAND ROUTER ---
#[tauri::command]
pub async fn execute_armata_command(cmd: String) -> Result<String, String> {
    let command = cmd.to_lowercase();
    
    // Agent: ARCHIVIST
    if command.contains("sort") || command.contains("clean") || command.contains("archive") {
        return archivist_run().await;
    }
    
    // Agent: VANGUARD
    if command.contains("scan") || command.contains("news") || command.contains("intel") {
        return vanguard_run().await;
    }

    // Agent: FORGE (Placeholder for direct video triggers)
    if command.contains("render") || command.contains("video") {
        return Ok("FORGE: Awaiting specific blueprint in Cinema module.".into());
    }

    // GENERAL (Fallback to LLM for reasoning)
    let s = crate::settings::load();
    let prompt = format!("You are ARMATA, an Agentic OS. The user issued command: '{}'. Acknowledge the command strictly and concisely like a military AI terminal. No pleasantries.", cmd);
    
    let messages = vec![serde_json::json!({"role": "user", "content": prompt})];
    match crate::ollama::chat_once(messages, &s.llm_model).await {
        Ok(res) => Ok(format!("GENERAL: {}", res)),
        Err(e) => Err(format!("GENERAL COMMUNICATION FAILURE: {}", e)),
    }
}

#[tauri::command]
pub async fn toggle_agent(agent: String, state: bool) -> Result<String, String> {
    let status = if state { "ONLINE [Monitoring]" } else { "OFFLINE [Standby]" };
    Ok(format!("Agent '{}' shifted to {}", agent.to_uppercase(), status))
}

// --- AGENT IMPLEMENTATIONS ---

// The Archivist: Safely sorts basic files from Downloads to a Horizon Vault
async fn archivist_run() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("CRITICAL: Cannot locate user home directory.")?;
    let downloads = home.join("Downloads");
    let vault_sorted = home.join("Documents/Horizon_Vault/Sorted_Intel");
    
    fs::create_dir_all(&vault_sorted).map_err(|e| format!("ARCHIVIST IO ERROR: {}", e))?;

    let mut moved_count = 0;
    
    if let Ok(entries) = fs::read_dir(&downloads) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().unwrap_or_default().to_string_lossy().to_lowercase();
                // We safely target only media and documents
                if ext == "pdf" || ext == "jpg" || ext == "png" || ext == "md" || ext == "txt" {
                    let dest = vault_sorted.join(path.file_name().unwrap());
                    if fs::rename(&path, &dest).is_ok() { 
                        moved_count += 1; 
                    }
                }
            }
        }
    }
    
    Ok(format!("ARCHIVIST: Sweep complete. Secured {} files into Vault/Sorted_Intel.", moved_count))
}

// The Vanguard: Fetches raw network data
async fn vanguard_run() -> Result<String, String> {
    // For now, scans HackerNews RSS as a proof of network capability
    let resp = reqwest::get("https://news.ycombinator.com/rss").await.map_err(|e| format!("VANGUARD NETWORK ERROR: {}", e))?;
    let text = resp.text().await.map_err(|e| format!("VANGUARD PARSE ERROR: {}", e))?;

    // Extracting basic intel size as proof
    let kilobytes = text.len() / 1024;
    Ok(format!("VANGUARD: Network scan complete. Intercepted {} KB of raw external intelligence.", kilobytes))
}

// --- ANTENNA BRIDGE ENTRY POINT ---
// Stub for Task 7 (full implementation pending)
pub async fn route_command(cmd: String) -> Result<String, String> {
    Ok(format!("ARMATA: {}", cmd))
}

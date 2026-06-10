use tauri::{AppHandle, Emitter};
use crate::{settings, ollama};

#[derive(Debug, PartialEq)]
pub enum CommandKind {
    Archivist,
    Vanguard,
    OsTool,
    LlmFallback,
}

pub fn classify(cmd: &str) -> CommandKind {
    let lower = cmd.to_lowercase();

    if lower.split_whitespace().any(|w| matches!(w, "sort" | "clean" | "archive" | "move" | "file" | "tidy")) {
        return CommandKind::Archivist;
    }
    if lower.split_whitespace().any(|w| matches!(w, "scan" | "news" | "intel" | "rss" | "fetch" | "scrape")) {
        return CommandKind::Vanguard;
    }
    if lower.split_whitespace().any(|w| matches!(w, "open" | "launch" | "start" | "volume" | "status" | "close")) {
        return CommandKind::OsTool;
    }

    CommandKind::LlmFallback
}

fn emit_log(app: &AppHandle, msg: &str) {
    let _ = app.emit("armata-terminal-log", msg);
}

/// Core router — used by both the Tauri command and the Antenna bridge.
pub async fn route_command(cmd: String) -> Result<String, String> {
    let s = settings::load();
    match classify(&cmd) {
        CommandKind::Archivist => {
            if !s.agents.archivist_enabled {
                return Err("Archivist agent is disabled".to_string());
            }
            archivist_run_once().await
        }
        CommandKind::Vanguard => {
            if !s.agents.vanguard_enabled {
                return Err("Vanguard agent is disabled".to_string());
            }
            vanguard_run_once().await
        }
        CommandKind::OsTool => os_tool_run(&cmd),
        CommandKind::LlmFallback => llm_run(&cmd).await,
    }
}

#[tauri::command]
pub async fn execute_armata_command(
    app: AppHandle, 
    vram_queue: tauri::State<'_, crate::vram_queue::VramQueue>,
    cmd: String
) -> Result<String, String> {
    emit_log(&app, &format!("> {}", cmd));
    
    // We only acquire permit IF it's an LLM command. 
    // But classification happens inside route_command.
    // Let's just acquire it for everything to be safe, or refactor.
    // Actually, classifying is fast.
    
    let kind = classify(&cmd);
    let permit = if kind == CommandKind::LlmFallback {
        Some(vram_queue.acquire("Armata LLM").await?)
    } else {
        None
    };

    let result = route_command(cmd).await?;
    emit_log(&app, &result);
    drop(permit);
    Ok(result)
}

#[tauri::command]
pub async fn toggle_agent(app: AppHandle, agent: String, enabled: bool) -> Result<String, String> {
    let mut s = settings::load();
    let status_str = if enabled { "ONLINE" } else { "OFFLINE" };

    match agent.as_str() {
        "archivist" => s.agents.archivist_enabled = enabled,
        "vanguard" => s.agents.vanguard_enabled = enabled,
        "antenna" => s.agents.antenna_enabled = enabled,
        "forge" => s.agents.forge_enabled = enabled,
        _ => return Err(format!("Unknown agent: {}", agent)),
    }

    settings::save_settings(s)?;

    let msg = format!("Agent {} → {}", agent.to_uppercase(), status_str);
    let _ = app.emit("armata-agent-status", serde_json::json!({
        "agent": agent,
        "status": if enabled { "online" } else { "offline" },
        "message": msg.clone()
    }));

    Ok(msg)
}

#[tauri::command]
pub fn get_armata_status() -> serde_json::Value {
    let s = settings::load();
    serde_json::json!({
        "archivist": s.agents.archivist_enabled,
        "vanguard": s.agents.vanguard_enabled,
        "antenna": s.agents.antenna_enabled,
        "forge": s.agents.forge_enabled,
        "antenna_port": s.agents.antenna_port,
        "vanguard_interval": s.agents.vanguard_interval_minutes,
        "light_model": s.agents.light_model,
    })
}

// --- Agent one-shot runners (called from route_command) ---

async fn archivist_run_once() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot locate home directory")?;
    let downloads = home.join("Downloads");
    let vault = home.join("Documents/Horizon_Vault/Sorted_Intel");
    std::fs::create_dir_all(&vault).map_err(|e| e.to_string())?;

    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(&downloads) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() { continue; }
            let filename = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            if let Some(cat) = crate::archivist::categorize_file(&filename) {
                let dest_dir = vault.join(cat);
                std::fs::create_dir_all(&dest_dir).ok();
                let dest = dest_dir.join(&filename);
                if !dest.exists() {
                    if std::fs::rename(&path, &dest).is_ok() { count += 1; }
                }
            }
        }
    }
    Ok(format!("ARCHIVIST: Filed {} item(s)", count))
}

fn is_safe_feed_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && !lower.contains("localhost")
        && !lower.contains("127.0.0.1")
        && !lower.contains("::1")
        && !lower.contains("0.0.0.0")
}

async fn vanguard_run_once() -> Result<String, String> {
    let s = settings::load();
    let mut intercepted = 0usize;
    for feed_url in &s.agents.vanguard_feeds {
        if !is_safe_feed_url(feed_url) {
            continue;
        }
        if let Ok(resp) = reqwest::get(feed_url).await {
            if let Ok(text) = resp.text().await {
                intercepted += crate::vanguard::parse_rss_items(&text).len();
            }
        }
    }
    Ok(format!("VANGUARD: Scanned {} RSS items across {} feeds", intercepted, s.agents.vanguard_feeds.len()))
}

fn os_tool_run(cmd: &str) -> Result<String, String> {
    let lower = cmd.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();

    if words.contains(&"status") {
        let s = settings::load();
        return Ok(format!(
            "ARMATA STATUS\nArchivist: {}\nVanguard: {}\nAntenna: {}:{}",
            if s.agents.archivist_enabled { "ONLINE" } else { "OFFLINE" },
            if s.agents.vanguard_enabled { "ONLINE" } else { "OFFLINE" },
            if s.agents.antenna_enabled { "ONLINE" } else { "OFFLINE" },
            s.agents.antenna_port,
        ));
    }

    if let Some(pos) = words.iter().position(|&w| w == "open" || w == "launch" || w == "start") {
        if let Some(app_name) = words.get(pos + 1) {
            const ALLOWED_APPS: &[&str] = &[
                "spotify", "firefox", "chromium", "kitty", "alacritty",
                "code", "nautilus", "thunar", "pcmanfm", "vlc", "mpv",
            ];
            if !ALLOWED_APPS.contains(app_name) {
                return Err(format!("App '{}' not in allowlist", app_name));
            }
            std::process::Command::new(app_name)
                .spawn()
                .map(|_| format!("Launched: {}", app_name))
                .map_err(|e| format!("Failed to launch '{}': {}", app_name, e))
        } else {
            Err("Specify an app to open".to_string())
        }
    } else if words.contains(&"volume") {
        if words.contains(&"up") {
            let _ = std::process::Command::new("pactl")
                .args(["set-sink-volume", "@DEFAULT_SINK@", "+5%"])
                .status();
            Ok("Volume +5%".to_string())
        } else if words.contains(&"down") {
            let _ = std::process::Command::new("pactl")
                .args(["set-sink-volume", "@DEFAULT_SINK@", "-5%"])
                .status();
            Ok("Volume -5%".to_string())
        } else {
            Err("volume up | volume down".to_string())
        }
    } else {
        Err(format!("Unknown OS command: {}", cmd))
    }
}

async fn llm_run(cmd: &str) -> Result<String, String> {
    let s = settings::load();
    let prompt = format!(
        "You are ARMATA, the command core of an Agentic OS. Reply in 1-3 sentences, terminal style. No markdown. Command: {}",
        cmd
    );
    let msgs = vec![serde_json::json!({"role": "user", "content": prompt})];
    let resp = ollama::chat_once(msgs, &s.llm_model).await?;
    Ok(format!("GENERAL: {}", resp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_file_command() {
        assert_eq!(classify("sort downloads"), CommandKind::Archivist);
        assert_eq!(classify("clean my files"), CommandKind::Archivist);
        assert_eq!(classify("archive pdfs"), CommandKind::Archivist);
    }

    #[test]
    fn test_classify_vanguard_command() {
        assert_eq!(classify("scan news"), CommandKind::Vanguard);
        assert_eq!(classify("get intel"), CommandKind::Vanguard);
    }

    #[test]
    fn test_classify_os_command() {
        assert_eq!(classify("open spotify"), CommandKind::OsTool);
        assert_eq!(classify("launch firefox"), CommandKind::OsTool);
        assert_eq!(classify("volume up"), CommandKind::OsTool);
        assert_eq!(classify("status"), CommandKind::OsTool);
    }

    #[test]
    fn test_classify_llm_fallback() {
        assert_eq!(classify("what is the meaning of life"), CommandKind::LlmFallback);
    }
}

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use once_cell::sync::Lazy;
use std::sync::RwLock;
use std::collections::HashMap;

static CACHE: Lazy<RwLock<Option<Settings>>> = Lazy::new(|| RwLock::new(None));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    pub archivist_enabled: bool,
    pub vanguard_enabled: bool,
    pub antenna_enabled: bool,
    pub forge_enabled: bool,
    pub wiki_enabled: bool,
    /// Bearer token required for Antenna HTTP requests
    pub antenna_token: String,
    pub antenna_port: u16,
    /// Minutes between Vanguard RSS scans
    pub vanguard_interval_minutes: u64,
    /// Model used by background agents (lighter than main LLM)
    pub light_model: String,
    /// RSS URLs for Vanguard to monitor
    pub vanguard_feeds: Vec<String>,
    pub force_agent_mode: bool,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            archivist_enabled: true,
            vanguard_enabled: true,
            antenna_enabled: false,
            forge_enabled: true,
            wiki_enabled: true,
            antenna_token: "changeme".to_string(),
            antenna_port: 8374,
            vanguard_interval_minutes: 30,
            light_model: "qwen2.5-coder:14b".to_string(),
            vanguard_feeds: vec![
                "https://news.ycombinator.com/rss".to_string(),
                "https://feeds.feedburner.com/TheHackersNews".to_string(),
            ],
            force_agent_mode: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapability {
    pub tool_calling: bool,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub vault_path: String,
    pub llm_model: String,
    pub roleplay_model: String,
    pub comfyui_path: String,
    pub embeddings_path: String,
    pub image_rating: String,
    pub agents: AgentSettings,
    pub agent_workspace: String,
    pub model_capabilities: HashMap<String, ModelCapability>,
}

impl Default for Settings {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let data = dirs::data_local_dir().unwrap_or_default();
        Self {
            vault_path: home.join("Documents/Claude RAG").to_string_lossy().into_owned(),
            llm_model: "qwen2.5-coder:14b".to_string(),
            roleplay_model: "llama3.1:8b".to_string(),
            comfyui_path: home.join("Projects/Horizon/ComfyUI/main.py").to_string_lossy().into_owned(),
            embeddings_path: data.join("horizon/embeddings.bin").to_string_lossy().into_owned(),
            image_rating: "rating_safe".to_string(),
            agents: AgentSettings::default(),
            agent_workspace: {
                let p = home.join("Projects/Horizon/workspace");
                std::fs::create_dir_all(&p).ok();
                std::fs::canonicalize(&p)
                    .unwrap_or(p)
                    .to_string_lossy()
                    .into_owned()
            },
            model_capabilities: HashMap::new(),
        }
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir().unwrap_or_default().join("horizon/settings.json")
}

pub fn load() -> Settings {
    if let Ok(cache) = CACHE.read() {
        if let Some(ref s) = *cache {
            return s.clone();
        }
    }
    let s = load_from_disk();
    if let Ok(mut cache) = CACHE.write() {
        *cache = Some(s.clone());
    }
    s
}

fn load_from_disk() -> Settings {
    let path = config_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(Settings::default)
}

fn persist(settings: &Settings) -> Result<(), String> {
    let path = config_path();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path, serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings() -> Settings {
    load()
}

#[tauri::command]
pub fn save_settings(settings: Settings) -> Result<(), String> {
    persist(&settings)?;
    if let Ok(mut cache) = CACHE.write() {
        *cache = Some(settings);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_settings_defaults() {
        let s = Settings::default();
        assert_eq!(s.agents.antenna_port, 8374);
        assert!(s.agents.archivist_enabled);
        assert!(s.agents.vanguard_enabled);
        assert!(!s.agents.antenna_enabled);
        assert!(s.agents.forge_enabled);
        assert_eq!(s.agents.vanguard_interval_minutes, 30);
        assert!(!s.agents.light_model.is_empty());
    }

    #[test]
    fn test_agent_workspace_canonized() {
        let s = Settings::default();
        assert!(std::path::Path::new(&s.agent_workspace).is_absolute());
    }

    #[test]
    fn test_model_capabilities_default_empty() {
        let s = Settings::default();
        assert!(s.model_capabilities.is_empty());
    }

    #[test]
    fn test_force_agent_mode_default_false() {
        let s = Settings::default();
        assert!(!s.agents.force_agent_mode);
    }
}

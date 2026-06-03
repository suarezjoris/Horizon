use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub vault_path: String,
    pub llm_model: String,
    pub roleplay_model: String,
    pub comfyui_path: String,
    pub embeddings_path: String,
    pub image_rating: String,
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
        }
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir().unwrap_or_default().join("horizon/settings.json")
}

pub fn load() -> Settings {
    let path = config_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| Settings::default())
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
    persist(&settings)
}

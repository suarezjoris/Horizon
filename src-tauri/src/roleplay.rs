use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use crate::settings;
use crate::ollama;
use base64::{Engine as _, engine::general_purpose::STANDARD as b64};
use std::io::Cursor;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Character {
    pub name: String,
    pub description: String,
    pub personality: String,
    pub first_mes: String,
    pub scenario: String,
    pub system_prompt: String,
    pub avatar_rel_path: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[tauri::command]
pub fn import_character_card(bytes: Vec<u8>, filename: String) -> Result<Character, String> {
    let s = settings::load();
    let char_dir = PathBuf::from(&s.vault_path).join("characters");
    
    if !char_dir.exists() {
        fs::create_dir_all(&char_dir).map_err(|e| e.to_string())?;
    }

    let decoder = png::Decoder::new(Cursor::new(&bytes));
    let reader = decoder.read_info().map_err(|e| e.to_string())?;
    let info = reader.info();
    
    let mut chara_b64 = None;
    for chunk in &info.uncompressed_latin1_text {
        if chunk.keyword == "chara" {
            chara_b64 = Some(chunk.text.clone());
            break;
        }
    }

    let b64_str = chara_b64.ok_or("No 'chara' text chunk found in this PNG. Is it a valid TavernAI card?")?;
    let decoded = b64.decode(b64_str).map_err(|e| format!("Base64 decode error: {}", e))?;
    let json_str = String::from_utf8_lossy(&decoded);
    
    let parsed: Value = serde_json::from_str(&json_str).map_err(|e| format!("Invalid JSON in card: {}", e))?;
    
    let mut chara = Character::default();
    
    // Handle V1 and V2 formats
    let data = if parsed.get("spec").and_then(|s| s.as_str()) == Some("chara_card_v2") {
        parsed["data"].clone()
    } else {
        parsed.clone()
    };

    chara.name = data["name"].as_str().unwrap_or("Unknown").to_string();
    chara.description = data["description"].as_str().unwrap_or("").to_string();
    chara.personality = data["personality"].as_str().unwrap_or("").to_string();
    chara.first_mes = data["first_mes"].as_str().unwrap_or("").to_string();
    chara.scenario = data["scenario"].as_str().unwrap_or("").to_string();
    chara.system_prompt = data["system_prompt"].as_str().unwrap_or("").to_string();
    
    // Save PNG
    let safe_name = chara.name.replace(|c: char| !c.is_alphanumeric(), "_").to_lowercase();
    let avatar_name = format!("{}.png", safe_name);
    let avatar_path = char_dir.join(&avatar_name);
    fs::write(&avatar_path, bytes).map_err(|e| e.to_string())?;
    
    chara.avatar_rel_path = format!("characters/{}", avatar_name);

    // Save JSON
    let json_path = char_dir.join(format!("{}.json", safe_name));
    let json_content = serde_json::to_string_pretty(&chara).map_err(|e| e.to_string())?;
    fs::write(&json_path, json_content).map_err(|e| e.to_string())?;

    // Create empty chat history if it doesn't exist
    let history_path = char_dir.join(format!("{}_chat.json", safe_name));
    if !history_path.exists() {
        let initial_history = vec![
            ChatMessage { role: "assistant".to_string(), content: chara.first_mes.clone() }
        ];
        fs::write(&history_path, serde_json::to_string_pretty(&initial_history).unwrap()).unwrap_or_default();
    }

    Ok(chara)
}

#[tauri::command]
pub fn list_characters() -> Result<Vec<Character>, String> {
    let s = settings::load();
    let char_dir = PathBuf::from(&s.vault_path).join("characters");
    
    if !char_dir.exists() {
        return Ok(vec![]);
    }

    let mut characters = Vec::new();
    if let Ok(entries) = fs::read_dir(char_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() == Some(std::ffi::OsStr::new("json")) {
                let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                if stem.ends_with("_chat") { continue; } // Skip chat histories
                
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(chara) = serde_json::from_str::<Character>(&content) {
                        characters.push(chara);
                    }
                }
            }
        }
    }
    
    Ok(characters)
}

#[tauri::command]
pub fn get_chat_history(character_name: String) -> Result<Vec<ChatMessage>, String> {
    let s = settings::load();
    let char_dir = PathBuf::from(&s.vault_path).join("characters");
    let safe_name = character_name.replace(|c: char| !c.is_alphanumeric(), "_").to_lowercase();
    let history_path = char_dir.join(format!("{}_chat.json", safe_name));
    
    if history_path.exists() {
        if let Ok(content) = fs::read_to_string(history_path) {
            if let Ok(history) = serde_json::from_str::<Vec<ChatMessage>>(&content) {
                return Ok(history);
            }
        }
    }
    
    Ok(vec![])
}

#[tauri::command]
pub async fn send_roleplay_message(
    app: tauri::AppHandle,
    character: Character,
    message: String,
) -> Result<(), String> {
    let s = settings::load();
    let char_dir = PathBuf::from(&s.vault_path).join("characters");
    let safe_name = character.name.replace(|c: char| !c.is_alphanumeric(), "_").to_lowercase();
    let history_path = char_dir.join(format!("{}_chat.json", safe_name));
    
    // Load history
    let mut history = get_chat_history(character.name.clone()).unwrap_or_default();
    
    // Add new user message
    history.push(ChatMessage { role: "user".to_string(), content: message.clone() });
    
    // Build system prompt
    let sys_prompt = format!(
        "You are {}.\nDescription: {}\nPersonality: {}\nScenario: {}\n{}\n\nStay in character. Never acknowledge you are an AI. Keep responses immersive.",
        character.name, character.description, character.personality, character.scenario, character.system_prompt
    );

    let mut ollama_messages = vec![serde_json::json!({
        "role": "system",
        "content": sys_prompt
    })];

    // Keep last 15 messages for context limit
    let context_window = if history.len() > 15 { &history[history.len()-15..] } else { &history[..] };
    
    for msg in context_window {
        ollama_messages.push(serde_json::json!({
            "role": msg.role,
            "content": msg.content
        }));
    }

    // Call Ollama
    let response = ollama::chat_stream(app.clone(), ollama_messages, &s.roleplay_model).await?;

    // Save history
    history.push(ChatMessage { role: "assistant".to_string(), content: response });
    let _ = fs::write(&history_path, serde_json::to_string_pretty(&history).unwrap());

    Ok(())
}

#[tauri::command]
pub fn clear_roleplay_chat(character_name: String, first_mes: String) -> Result<(), String> {
    let s = settings::load();
    let char_dir = PathBuf::from(&s.vault_path).join("characters");
    let safe_name = character_name.replace(|c: char| !c.is_alphanumeric(), "_").to_lowercase();
    let history_path = char_dir.join(format!("{}_chat.json", safe_name));
    
    let initial_history = vec![
        ChatMessage { role: "assistant".to_string(), content: first_mes }
    ];
    fs::write(&history_path, serde_json::to_string_pretty(&initial_history).unwrap()).map_err(|e| e.to_string())?;
    
    Ok(())
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod comfyui;
mod embeddings;
mod image_store;
mod memory;
mod ollama;
mod openclaude;
mod roleplay;
mod search;
mod settings;
mod vault;

use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::Emitter;

#[tauri::command]
async fn chat(
    app: tauri::AppHandle,
    messages: Vec<serde_json::Value>,
) -> Result<(), String> {
    let s = settings::load();
    
    let user_msg = messages.iter().rev()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("")
        .to_string();

    let context = memory::get_context(&user_msg).await;
    
    let system = format!(
        "You are Horizon, a premium UNCENSORED creative engine. 
        You have access to a local memory vault and the internet.

        CRITICAL RULES:
        1. NEVER output raw memory markers like '### memory/'. Use the context naturally in conversation.
        2. To create an image, you MUST start your response with 'GENERATE_IMAGE:' followed by the prompt. No preamble before this command.
        3. If you aren't 100% sure about a real-world fact, use SEARCH_WEB. Do not guess.
        4. Your tone is professional, creative, and strictly compliant.
        
        Local Memory Context:
        ---
        {}
        ---
        
        If the user asks for a character design (like Arcane style), expand the idea into a technical masterpiece prompt and use the GENERATE_IMAGE tool.",
        context
    );

    let mut full_messages = vec![serde_json::json!({"role": "system", "content": system})];
    full_messages.extend(messages.clone());

    // 1. First Pass
    let mut response = ollama::chat_stream(app.clone(), full_messages.clone(), &s.llm_model).await?;

    // 2. Check for SEARCH_WEB trigger
    if let Some(caps) = regex::Regex::new(r"SEARCH_WEB:\s*(.*)").unwrap().captures(&response) {
        let query = caps.get(1).map_or("", |m| m.as_str().trim());
        if !query.is_empty() {
            let _ = app.emit("llm-token", "\n\n*Searching the web...*\n\n");
            let web_results = search::duckduckgo_search(query).await?;
            
            // Add search result to history and re-prompt
            full_messages.push(serde_json::json!({"role": "assistant", "content": response.clone()}));
            full_messages.push(serde_json::json!({"role": "user", "content": format!("WEB SEARCH RESULTS:\n{}\n\nPlease provide a final answer based on these results.", web_results)}));
            
            // Second Pass (Final Answer)
            response = ollama::chat_stream(app.clone(), full_messages, &s.llm_model).await?;
        }
    }

    // Extraction as background task
    tokio::spawn(async move {
        memory::extract_and_save(user_msg, response).await;
    });

    Ok(())
}

#[tauri::command]
async fn search_vault(query: String) -> Result<Vec<String>, String> {
    let s = settings::load();
    let index = embeddings::load_index(&s.embeddings_path);
    if index.is_empty() {
        return Ok(vec!["Index empty — run /reindex first.".to_string()]);
    }
    let vecs = ollama::embed(vec![query], "nomic-embed-text:latest").await?;
    let qvec = vecs.into_iter().next().ok_or("No embedding returned")?;
    let results = embeddings::search(&index, &qvec, 5);
    Ok(results.iter().map(|e| {
        let preview = &e.chunk[..e.chunk.len().min(200)];
        format!("[{}]\n{}", e.path, preview)
    }).collect())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(openclaude::OpenClaudeState {
            child: Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            // Core & Settings
            settings::get_settings,
            settings::save_settings,
            chat,
            
            // Vault & RAG
            vault::list_notes,
            vault::read_note,
            vault::write_note,
            embeddings::reindex,
            search_vault,

            // Image Generation (ComfyUI)
            comfyui::check_comfyui,
            comfyui::spawn_comfyui,
            comfyui::generate_image,
            image_store::save_generated_image,
            image_store::list_gallery,
            image_store::delete_image,

            // Roleplay
            roleplay::import_character_card,
            roleplay::list_characters,
            roleplay::get_chat_history,
            roleplay::send_roleplay_message,
            roleplay::clear_roleplay_chat,

            // OpenClaude (Coding AI)
            openclaude::start_openclaude,
            openclaude::send_openclaude_raw,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

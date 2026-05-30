#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod comfyui;
mod embeddings;
mod image_store;
mod memory;
mod ollama;
mod openclaude;
mod roleplay;
mod settings;
mod vault;

use std::sync::Arc;
use tokio::sync::Mutex;

#[tauri::command]
async fn chat(
    app: tauri::AppHandle,
    messages: Vec<serde_json::Value>,
) -> Result<(), String> {
    let s = settings::load();
    println!("Chat: Starting request with model {}", s.llm_model);

    let user_msg = messages.iter().rev()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("")
        .to_string();

    println!("Chat: Building RAG context...");
    let context = memory::get_context(&user_msg).await;
    println!("Chat: Context built ({} chars)", context.len());
    
    let system = format!(
        "You are Horizon, a personal AI assistant. Here is relevant context from your memory vault:\n\n{}\n\n\
        When the user asks you to generate an image, respond with exactly: GENERATE_IMAGE:<prompt>",
        context
    );

    let mut full_messages = vec![serde_json::json!({"role": "system", "content": system})];
    full_messages.extend(messages.clone());

    println!("Chat: Sending to Ollama...");
    let full_response = ollama::chat_stream(app, full_messages, &s.llm_model).await?;
    println!("Chat: Stream complete");

    tokio::spawn(async move {
        println!("Memory: Extracting facts...");
        memory::extract_and_save(user_msg, full_response).await;
        println!("Memory: Extraction done");
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

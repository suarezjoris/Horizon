#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod comfyui;
mod embeddings;
mod file_reader;
mod image_store;
mod memory;
mod ollama;
mod openclaude;
mod pyenv;
mod audio;
mod diagnostic;
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
        1. NEVER output raw memory markers like '### memory/'. Use the context naturally.
        2. To create an image, you MUST start your response with 'GENERATE_IMAGE:' followed by the prompt.
        3. SEARCH_WEB: If the user asks for factual information, news, celebrities, release dates, movies, internet culture, or ANY entity you are not 100% familiar with, you MUST write 'SEARCH_WEB: <query>' and NOTHING ELSE. 
           - If it sounds like a proper noun you don't recognize, SEARCH IT.
           - Even if you think you know the answer, VERIFY IT on the web.
           - DO NOT provide a partial answer before searching.
        4. Once web results are provided, integrate them accurately into a final response.
        5. Your tone is professional and creative.

        EXAMPLES:
        User: Who won the Oscar for best actor this year?
        Assistant: SEARCH_WEB: Oscar winner best actor 2024
        
        User: Tell me about Jaafar Jackson's latest movie.
        Assistant: SEARCH_WEB: Jaafar Jackson latest movie release date
        
        Local Memory Context:
        ---
        {}
        ---",
        context
    );

    let mut full_messages = vec![serde_json::json!({"role": "system", "content": system})];
    full_messages.extend(messages.clone());

    // 1. First Pass
    let mut response = ollama::chat_stream(app.clone(), full_messages.clone(), &s.llm_model).await?;

    // 2. Check for SEARCH_WEB trigger
    let search_re = regex::Regex::new(r"(?i)SEARCH_WEB:\s*(.*)").unwrap();
    if let Some(caps) = search_re.captures(&response) {
        let query = caps.get(1).map_or("", |m| m.as_str().trim());
        if !query.is_empty() && !user_msg.contains("WEB SEARCH RESULTS:") {
            // Signal search start and CLEAR the SEARCH_WEB command from UI
            let _ = app.emit("llm-token", "CLEAR_AND_SEARCH");
            
            match search::duckduckgo_search(query).await {
                Ok(web_results) => {
                    let mut second_pass_messages = vec![serde_json::json!({"role": "system", "content": format!("{} \n\nIMPORTANT: Use the following WEB SEARCH RESULTS to answer accurately. Contradict yourself if needed.", system)})];
                    second_pass_messages.extend(messages.clone());
                    second_pass_messages.push(serde_json::json!({
                        "role": "user", 
                        "content": format!("WEB SEARCH RESULTS:\n---\n{}\n---\nPlease provide the final answer to: '{}'", web_results, user_msg)
                    }));
                    
                    // Second Pass (Final Answer)
                    response = ollama::chat_stream(app.clone(), second_pass_messages, &s.llm_model).await?;
                },
                Err(e) => {
                    let _ = app.emit("llm-token", format!("\n\n*⚠️ Search failed: {}*\n\n", e));
                }
            }
        }
    }

    let _ = app.emit("llm-done", &response);

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
    // v2.03 Stability: Ensure all required directories exist before startup
    let s = settings::load();
    let data_dir = std::path::PathBuf::from(&s.embeddings_path).parent().unwrap().to_path_buf();
    let config_dir = dirs::config_dir().unwrap().join("horizon");
    let vault_images = std::path::PathBuf::from(&s.vault_path).join("images");
    let vault_chars = std::path::PathBuf::from(&s.vault_path).join("characters");
    let vault_memory = std::path::PathBuf::from(&s.vault_path).join("memory");

    let _ = std::fs::create_dir_all(&data_dir);
    let _ = std::fs::create_dir_all(&config_dir);
    let _ = std::fs::create_dir_all(&vault_images);
    let _ = std::fs::create_dir_all(&vault_chars);
    let _ = std::fs::create_dir_all(&vault_memory);

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
            file_reader::read_file_content,

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

            // Audio
            audio::save_audio_temp,
            audio::transcribe_audio,

            // Diagnostic
            diagnostic::run_diagnostics,
            diagnostic::fix_health_issue,

            // OpenClaude (Coding AI)
            openclaude::start_openclaude,
            openclaude::send_openclaude_raw,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

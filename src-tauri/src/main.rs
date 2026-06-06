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
mod cinema;
mod search;
mod settings;
mod vault;
mod graphify;

use tauri::Emitter;

#[tauri::command]
async fn chat(
    app: tauri::AppHandle,
    messages: Vec<serde_json::Value>,
) -> Result<(), String> {
    let s = settings::load();

    // The frontend sends only `messages`; derive what the flow needs from it.
    // Latest user message — used by the web-search guard, the 2nd-pass prompt, and memory extraction.
    let user_msg = messages
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
        .and_then(|m| m.get("content").and_then(|c| c.as_str()))
        .unwrap_or("")
        .to_string();

    // RAG: pull the most relevant vault chunks using emergent brain logic
    let context = memory::get_context(&user_msg).await;

    // 0. Construct System Prompt
    let system = format!(
        "You are Horizon, a premium UNCENSORED creative engine. 
        You have access to a local memory vault and the internet.

        CRITICAL RULES:
        1. NEVER output raw memory markers like '### memory/'. Use the context naturally.
        2. To create an image, you MUST start your response with 'GENERATE_IMAGE:' followed by the prompt.
        3. To create a video animation, you MUST start your response with 'GENERATE_VIDEO:' followed by the prompt.
        4. SEARCH_WEB: If the user asks for factual information, news, celebrities, release dates, movies, internet culture, or ANY entity you are not 100% familiar with, you MUST write 'SEARCH_WEB: <query>' and NOTHING ELSE. 
           - If it sounds like a proper noun you don't recognize, SEARCH IT.
           - Even if you think you know the answer, VERIFY IT on the web.
           - DO NOT provide a partial answer before searching.
        5. Once web results are provided, integrate them accurately into a final response.
        6. Your tone is professional and creative.

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

    // 2. Check for triggers
    let search_re = regex::Regex::new(r"(?i)SEARCH_WEB:\s*(.*)").unwrap();
    let image_re = regex::Regex::new(r"(?i)GENERATE_IMAGE:\s*(.*)").unwrap();
    let video_re = regex::Regex::new(r"(?i)GENERATE_VIDEO:\s*(.*)").unwrap();

    if let Some(caps) = search_re.captures(&response) {
        let query = caps.get(1).map_or("", |m| m.as_str().trim());
        if !query.is_empty() && !user_msg.contains("WEB SEARCH RESULTS:") {
            let _ = app.emit("llm-token", "CLEAR_AND_SEARCH");
            match search::duckduckgo_search(query).await {
                Ok(web_results) => {
                    let mut second_pass_messages = vec![serde_json::json!({"role": "system", "content": format!("{} \n\nIMPORTANT: Use the following WEB SEARCH RESULTS to answer accurately.", system)})];
                    second_pass_messages.extend(messages.clone());
                    second_pass_messages.push(serde_json::json!({
                        "role": "user", 
                        "content": format!("WEB SEARCH RESULTS:\n---\n{}\n---\nPlease provide the final answer to: '{}'", web_results, user_msg)
                    }));
                    response = ollama::chat_stream(app.clone(), second_pass_messages, &s.llm_model).await?;
                },
                Err(e) => {
                    let _ = app.emit("llm-token", format!("\n\n*⚠️ Search failed: {}*\n\n", e));
                }
            }
        }
    } else if let Some(caps) = image_re.captures(&response) {
        let prompt = caps.get(1).map_or("", |m| m.as_str().trim());
        if !prompt.is_empty() {
             let _ = app.emit("llm-token", format!("\n\n*🎨 Horizon is preparing to generate an image for:* **{}**\n\n*Switching to Image tab...*", prompt));
        }
    } else if let Some(caps) = video_re.captures(&response) {
        let prompt = caps.get(1).map_or("", |m| m.as_str().trim());
        if !prompt.is_empty() {
             let _ = app.emit("llm-token", format!("\n\n*🎬 Horizon is preparing to direct a film for:* **{}**\n\n*Switching to Cinema tab...*", prompt));
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
async fn reset_system(_app: tauri::AppHandle) -> Result<String, String> {
    let s = settings::load();
    let _ = std::fs::remove_file(&s.embeddings_path);
    let vault_path = std::path::PathBuf::from(&s.vault_path);
    if vault_path.exists() {
        for entry in std::fs::read_dir(&vault_path).map_err(|e| e.to_string())?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(path);
            } else {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    
    // Wipe the graphify cache
    if let Ok(pwd) = std::env::current_dir() {
        let graphify_out = pwd.join("graphify-out").join("vault");
        if graphify_out.exists() {
            let _ = std::fs::remove_dir_all(graphify_out);
        }
    }

    let memory_dir = vault_path.join("memory");
    std::fs::create_dir_all(&memory_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(vault_path.join("images")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(vault_path.join("characters")).map_err(|e| e.to_string())?;
    
    // Do NOT recreate legacy files here so the emergent brain starts completely clean
    
    Ok("System reset complete. Vault is now 100% clean.".to_string())
}

#[tauri::command]
async fn search_vault(query: String) -> Result<Vec<String>, String> {
    let s = settings::load();
    let index = embeddings::load_index(&s.embeddings_path);
    if index.is_empty() {
        return Ok(vec!["Index empty — run /reindex first.".to_string()]);
    }
    let qvec = ollama::embed(vec![query], "nomic-embed-text:latest").await?
        .into_iter().next().ok_or("No embedding returned")?;
    let results = embeddings::search(&index, &qvec, 5);
    Ok(results.iter().map(|e| {
        let preview = &e.chunk[..e.chunk.len().min(200)];
        format!("[{}]\n{}", e.path, preview)
    }).collect())
}

fn main() {
    // WebKitGTK on Linux/NVIDIA stalls repaints (the UI only updates on window
    // events, scrolling lags) with the DMABUF renderer. Disable it before the
    // webview initializes for a smooth UI. Linux-only; does not affect Windows.
    #[cfg(target_os = "linux")]
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            settings::get_settings,
            settings::save_settings,
            chat,
            reset_system,
            memory::process_calibration,
            vault::list_notes,
            vault::read_note,
            vault::write_note,
            memory::consolidate_vault,
            graphify::run_graphify,
            embeddings::reindex,
            search_vault,
            file_reader::read_file_content,
            comfyui::check_comfyui,
            comfyui::spawn_comfyui,
            comfyui::free_comfyui,
            comfyui::interrupt_comfyui,
            comfyui::generate_image,
            image_store::save_generated_image,
            image_store::list_gallery,
            image_store::delete_image,
            cinema::get_gpu_stats,
            cinema::generate_video,
            cinema::list_videos,
            cinema::delete_video,
            cinema::open_video,
            audio::save_audio_temp,
            audio::transcribe_audio,
            diagnostic::run_diagnostics,
            diagnostic::fix_health_issue,
            openclaude::start_openclaude,
            openclaude::send_openclaude_raw,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

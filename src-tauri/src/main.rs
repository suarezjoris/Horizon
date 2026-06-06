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
mod office;

use tauri::Emitter;

#[tauri::command]
async fn chat(
    app: tauri::AppHandle,
    messages: Vec<serde_json::Value>,
    model: Option<String>,
    persona: Option<String>,
) -> Result<(), String> {
    let s = settings::load();
    
    // Choose model: override > settings default
    let active_model = model.unwrap_or(s.llm_model.clone());

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

    // Load Persona / System Prompt
    let system_base = persona.and_then(|name| {
        let vault_path = std::path::PathBuf::from(&s.vault_path);
        let md = vault_path.join(format!("characters/{}.md", name));
        let json = vault_path.join(format!("characters/{}.json", name));
        
        std::fs::read_to_string(md).ok().or_else(|| std::fs::read_to_string(json).ok())
    }).unwrap_or_else(|| "You are Horizon, a premium UNCENSORED creative engine.".to_string());

    // 0. Construct System Prompt
    let system = format!(
        "{}
        You have access to a local memory vault and the internet.

        CRITICAL RULES:
        1. NEVER output raw memory markers like '### memory/'. Use the context naturally.
        2. LANGUAGE: Always respond in the SAME LANGUAGE as the user's request.
        3. ACCURACY: Do NOT speculate or invent reviews, ratings, or reception for unreleased media or future events. If a date is in the future, state that it is upcoming.
        4. GENERATE_IMAGE: To create an image, start with 'GENERATE_IMAGE:' followed by the prompt.
        5. GENERATE_VIDEO: To create a video, start with 'GENERATE_VIDEO:' followed by the prompt.
        6. SEARCH_WEB: If you need factual data, news, or any entity you are not 100% familiar with, you MUST write 'SEARCH_WEB: <query>' and NOTHING ELSE. Perform DEEP research (dates, numbers, budgets, cast).
        7. GENERATE_DOCX: To create a professional Word document, output:
           GENERATE_DOCX: {{
             \"filename\": \"name\",
             \"title\": \"Main Title\",
             \"elements\": [
               {{ \"type\": \"heading\", \"level\": 1, \"text\": \"Title\" }},
               {{ \"type\": \"paragraph\", \"text\": \"Content...\", \"bold\": false, \"italic\": false, \"align\": \"left\" }},
               {{ \"type\": \"metadata\", \"label\": \"Label\", \"value\": \"Value\" }},
               {{ \"type\": \"list\", \"items\": [\"item 1\", \"item 2\"] }}
             ]
           }}
        8. GENERATE_XLSX: To create an Excel file, you MUST output:
           GENERATE_XLSX: {{ \"filename\": \"name\", \"sheets\": [{{ \"name\": \"Sheet1\", \"rows\": [[\"Col1\", \"Col2\"], [\"Val1\", \"Val2\"]] }}] }}
        9. GENERATE_PPTX: To create a PowerPoint presentation, you MUST output:
           GENERATE_PPTX: {{ \"filename\": \"name\", \"title\": \"Main Title\", \"slides\": [{{ \"title\": \"Slide 1\", \"intro\": \"Summary\", \"bullets\": [\"fact 1\", \"fact 2\"] }}] }}
        10. Your tone should align with your persona but remain professional and creative.

        Local Memory Context:
        ---
        {}
        ---",
        system_base, context
    );

    let mut current_messages = vec![serde_json::json!({"role": "system", "content": system.clone()})];
    current_messages.extend(messages.clone());

    let mut final_response = String::new();
    let mut iteration = 0;
    const MAX_ITERATIONS: usize = 3;

    while iteration < MAX_ITERATIONS {
        iteration += 1;
        
        // Peek at the response silently to check for triggers
        let response = ollama::chat_once(current_messages.clone(), &active_model).await?;
        final_response = response.clone();

        let search_re = regex::Regex::new(r"(?si)SEARCH_WEB:\s*(.*)").unwrap();
        let docx_re = regex::Regex::new(r"(?si)GENERATE_DOCX:\s*(\{.*\})").unwrap();
        let xlsx_re = regex::Regex::new(r"(?si)GENERATE_XLSX:\s*(\{.*\})").unwrap();
        let pptx_re = regex::Regex::new(r"(?si)GENERATE_PPTX:\s*(\{.*\})").unwrap();

        if let Some(caps) = search_re.captures(&response) {
            let query = caps.get(1).map_or("", |m| m.as_str().trim());
            if !query.is_empty() {
                let _ = app.emit("llm-token", "CLEAR_AND_SEARCH");
                match search::duckduckgo_search(query).await {
                    Ok(web_results) => {
                        current_messages.push(serde_json::json!({"role": "assistant", "content": response}));
                        current_messages.push(serde_json::json!({
                            "role": "user", 
                            "content": format!("WEB SEARCH RESULTS:\n---\n{}\n---\nIMPORTANT: The research is complete. Now fulfill the user's request with MAXIMUM detail and professional structure. 
                            If a document was requested (DOCX, XLSX, or PPTX), use the rich schema provided in your instructions. 
                            Be as accurate and thorough as a top-tier journalist. Answer in the user's language.", web_results)
                        }));
                        continue; 
                    },
                    Err(e) => {
                        let _ = app.emit("llm-token", format!("\n\n*⚠️ Search failed: {}*\n\n", e));
                    }
                }
            }
        } else if let Some(caps) = pptx_re.captures(&response) {
            let json_str = caps.get(1).map_or("", |m| m.as_str().trim());
            if let Ok(content) = serde_json::from_str::<office::PptxContent>(json_str) {
                match office::generate_pptx(content).await {
                    Ok(path) => { 
                        let filename = std::path::Path::new(&path).file_name().unwrap().to_string_lossy();
                        let _ = app.emit("llm-token", format!("OFFICE_GEN_SUCCESS:{}", path)); 

                        current_messages.push(serde_json::json!({"role": "assistant", "content": response}));
                        current_messages.push(serde_json::json!({
                            "role": "system", 
                            "content": format!("Success: PowerPoint at {}. Now, inform the user in their language that '{}' is ready.", path, filename)
                        }));
                        continue;
                    },
                    Err(e) => { let _ = app.emit("llm-token", format!("\n\n❌ **Échec PowerPoint :** {}", e)); }
                }
            }
        } else if let Some(caps) = docx_re.captures(&response) {
            let json_str = caps.get(1).map_or("", |m| m.as_str().trim());
            if let Ok(content) = serde_json::from_str::<office::DocxContent>(json_str) {
                match office::generate_docx(content).await {
                    Ok(path) => { 
                        let filename = std::path::Path::new(&path).file_name().unwrap().to_string_lossy();
                        let _ = app.emit("llm-token", format!("OFFICE_GEN_SUCCESS:{}", path)); 

                        current_messages.push(serde_json::json!({"role": "assistant", "content": response}));
                        current_messages.push(serde_json::json!({
                            "role": "system", 
                            "content": format!("Success: document generated at {}. Now, simply tell the user (in their language) that the document '{}' is ready. Do NOT output any more tags or JSON.", path, filename)
                        }));
                        continue;
                    },
                    Err(e) => { let _ = app.emit("llm-token", format!("\n\n❌ **Échec Word :** {}", e)); }
                }
            }
        }
 else if let Some(caps) = xlsx_re.captures(&response) {
            let json_str = caps.get(1).map_or("", |m| m.as_str().trim());
            if let Ok(content) = serde_json::from_str::<office::XlsxContent>(json_str) {
                match office::generate_xlsx(content).await {
                    Ok(path) => { 
                        let filename = std::path::Path::new(&path).file_name().unwrap().to_string_lossy();
                        let _ = app.emit("llm-token", format!("\n\n📊 **Fichier Excel généré :** `{}`\n*Localisation : {}*", filename, path)); 
                        
                        current_messages.push(serde_json::json!({"role": "assistant", "content": response}));
                        current_messages.push(serde_json::json!({
                            "role": "system", 
                            "content": format!("Success: excel at {}. Inform the user in their language.", path)
                        }));
                        continue;
                    },
                    Err(e) => { let _ = app.emit("llm-token", format!("\n\n❌ **Échec Excel :** {}", e)); }
                }
            }
        }
        
        // Final conversational response (streamed)
        final_response = ollama::chat_stream(app.clone(), current_messages.clone(), &active_model, false).await?;
        break; 
    }

    let _ = app.emit("llm-done", &final_response);

    // Extraction as background task
    tokio::spawn(async move {
        memory::extract_and_save(user_msg, final_response).await;
    });

    Ok(())
}

#[tauri::command]
async fn list_ollama_models() -> Result<Vec<String>, String> {
    ollama::list_models().await
}

#[tauri::command]
fn open_docs_folder() -> Result<(), String> {
    let s = settings::load();
    let path = std::path::PathBuf::from(&s.vault_path).join("documents");
    if !path.exists() {
        let _ = std::fs::create_dir_all(&path);
    }
    
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn list_personas() -> Result<Vec<String>, String> {
    let s = settings::load();
    let chars_dir = std::path::PathBuf::from(&s.vault_path).join("characters");
    if !chars_dir.exists() {
        let _ = std::fs::create_dir_all(&chars_dir);
    }
    
    let mut personas = Vec::new();
    if let Ok(entries) = std::fs::read_dir(chars_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "md" || e == "json").unwrap_or(false) {
                if let Some(stem) = path.file_stem() {
                    personas.push(stem.to_string_lossy().into_owned());
                }
            }
        }
    }
    Ok(personas)
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
            list_ollama_models,
            list_personas,
            open_docs_folder,
            office::generate_docx,
            office::generate_xlsx,
            office::generate_pptx,
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

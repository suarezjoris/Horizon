use crate::{embeddings, ollama, settings, tools};

#[tauri::command]
pub async fn list_ollama_models() -> Result<Vec<String>, String> {
    ollama::list_models().await
}

#[tauri::command]
pub async fn auto_consolidate_chat(
    vram_queue: tauri::State<'_, crate::vram_queue::VramQueue>,
    history: Vec<serde_json::Value>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let _permit = vram_queue.acquire("Auto-consolidate").await?;
    let mut text = String::new();
    for msg in history {
        if let Some(role) = msg.get("role").and_then(|r| r.as_str()) {
            if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                text.push_str(&format!("{}: {}\n\n", role, content));
            }
        }
    }
    
    let prompt = format!(
        "Summarize the following chat history into a detailed Zettelkasten memory node. \
        Extract ALL factual data, decisions, code snippets, and important context. \
        Ignore pleasantries. Output ONLY the raw markdown summary.\n\n{}", 
        text
    );
    
    let s = settings::load();
    let summary = ollama::chat_stream(
        app.clone(),
        vec![serde_json::json!({"role": "user", "content": prompt})],
        &s.llm_model,
        true
    ).await?;
    
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let filename = format!("chat_context_{}.md", timestamp);
    crate::vault::write_vault_note(&s.vault_path, &filename, &summary).map_err(|e| e.to_string())?;
    let _ = crate::embeddings::reindex().await;
    
    Ok(())
}

#[tauri::command]
pub async fn probe_model_capabilities(model: String) -> Result<bool, String> {
    let mut s = settings::load();
    let hash = ollama::get_model_hash(&model).await.unwrap_or_default();

    if let Some(cached) = s.model_capabilities.get(&model) {
        if cached.hash == hash {
            return Ok(cached.tool_calling);
        }
    }

    let capable = tools::probe_tool_calling(&model).await;

    s.model_capabilities.insert(model.clone(), settings::ModelCapability {
        tool_calling: capable,
        hash,
    });
    settings::save_settings(s)?;

    Ok(capable)
}

#[tauri::command]
pub fn open_docs_folder() -> Result<(), String> {
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
pub fn list_personas() -> Result<Vec<String>, String> {
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
pub async fn reset_system(_app: tauri::AppHandle) -> Result<String, String> {
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
pub async fn search_vault(query: String) -> Result<Vec<String>, String> {
    let s = settings::load();
    let index = embeddings::load_index(&s.embeddings_path);
    if index.is_empty() {
        return Ok(vec!["Index empty — run /reindex first.".to_string()]);
    }
    let qvec = ollama::embed(vec![query], "nomic-embed-text:latest").await?
        .into_iter().next().ok_or("No embedding returned")?;
    let results = index.search(&qvec, 5);
    Ok(results.iter().map(|e| {
        let preview = &e.chunk[..e.chunk.len().min(200)];
        format!("[{}]\n{}", e.path, preview)
    }).collect())
}

#[tauri::command]
pub async fn export_chat_as_pdf(messages: Vec<serde_json::Value>) -> Result<String, String> {
    use crate::office::{PdfContent, PdfElement, generate_pdf};

    let mut elements = Vec::new();
    
    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("Unknown");
        let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
        
        if content.is_empty() { continue; }
        
        let role_title = match role {
            "user" => "User",
            "assistant" => "Horizon",
            "system" => "System",
            _ => "Unknown",
        };
        
        elements.push(PdfElement::Heading { level: 2, text: role_title.to_string() });
        
        for para in content.split('\n') {
            if !para.trim().is_empty() {
                elements.push(PdfElement::Paragraph { 
                    text: para.to_string(), 
                    bold: None, 
                    italic: None 
                });
            }
        }
    }
    
    let pdf_content = PdfContent {
        filename: "Chat_Export.pdf".to_string(),
        title: "Chat Export".to_string(),
        elements,
        template: None,
    };
    
    generate_pdf(pdf_content).await
}

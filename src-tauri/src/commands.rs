use crate::{embeddings, ollama, settings, tools};

#[tauri::command]
pub async fn list_ollama_models() -> Result<Vec<String>, String> {
    ollama::list_models().await
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
pub fn get_note_decay_stats(rel_path: String) -> Result<serde_json::Value, String> {
    let s = settings::load();
    let index = embeddings::load_index(&s.embeddings_path);
    let meta_lock = index.metadata.read().unwrap();
    
    let mut total_access = 0;
    let mut last_accessed = 0;
    let mut pinned = false;
    let mut chunk_count = 0;
    
    for m in meta_lock.values() {
        if m.path == rel_path {
            total_access += m.access_count;
            if m.last_accessed > last_accessed {
                last_accessed = m.last_accessed;
            }
            pinned = m.pinned;
            chunk_count += 1;
        }
    }
    
    if chunk_count == 0 {
        return Ok(serde_json::json!({
            "chunks": 0,
            "status": "not_indexed"
        }));
    }
    
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let hl = s.memory_decay.half_life_days;
    let boost_factor = s.memory_decay.access_boost_factor;
    
    let days = ((now - last_accessed) as f64 / 86400.0).max(0.0);
    let decay_factor = if pinned { 1.0 } else { 0.5f64.powf(days / hl) };
    let boost = 1.0 + (1.0 + (total_access as f64 / chunk_count as f64)).log2() * boost_factor;
    let current_multiplier = decay_factor * boost;
    
    Ok(serde_json::json!({
        "chunks": chunk_count,
        "total_access": total_access,
        "days_since_access": (days * 10.0).round() / 10.0,
        "decay_factor": (decay_factor * 100.0).round() / 100.0,
        "boost_factor": (boost * 100.0).round() / 100.0,
        "current_multiplier": (current_multiplier * 100.0).round() / 100.0,
        "pinned": pinned
    }))
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

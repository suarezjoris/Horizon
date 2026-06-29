use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use notify::{Watcher, RecursiveMode, EventKind};
use tauri::{AppHandle, Emitter};
use crate::{embeddings, settings};

pub fn categorize_file(filename: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(filename)
        .extension()?
        .to_string_lossy()
        .to_lowercase();
    let ext = ext.as_str();

    match ext {
        "pdf" | "docx" | "pptx" | "xlsx" | "odt" => Some("documents"),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" => Some("images"),
        "mp4" | "mkv" | "avi" | "webm" | "mov" => Some("videos"),
        "mp3" | "flac" | "wav" | "ogg" => Some("audio"),
        "md" | "txt" | "rst" => Some("notes"),
        "zip" | "tar" | "gz" | "7z" | "rar" => Some("archives"),
        "rs" | "py" | "js" | "ts" | "go" | "c" | "cpp" | "java" => Some("code"),
        _ => None,
    }
}

fn emit_status(app: &AppHandle, status: &str, msg: &str) {
    let _ = app.emit("armata-agent-status", serde_json::json!({
        "agent": "archivist",
        "status": status,
        "message": msg
    }));
}

pub async fn run_archivist(app: AppHandle, running: Arc<AtomicBool>) {
    let s = settings::load();
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => {
            emit_status(&app, "error", "Cannot locate home directory");
            return;
        }
    };

    let watch_dir = home.join("Downloads");
    let vault_dir = std::path::PathBuf::from(&s.vault_path);

    if let Err(e) = std::fs::create_dir_all(&vault_dir) {
        emit_status(&app, "error", &format!("Cannot create vault dir: {}", e));
        return;
    }

    emit_status(&app, "online", &format!("Watching {} and {}", watch_dir.display(), vault_dir.display()));

    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            emit_status(&app, "error", &format!("Watcher init failed: {}", e));
            return;
        }
    };

    let _ = watcher.watch(&watch_dir, RecursiveMode::NonRecursive);
    let _ = watcher.watch(&vault_dir, RecursiveMode::Recursive);

    let mut pending_downloads: HashMap<PathBuf, Instant> = HashMap::new();
    let mut pending_vault: HashMap<PathBuf, Instant> = HashMap::new();

    while running.load(Ordering::Relaxed) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) => {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    for path in event.paths {
                        if path.is_file() {
                            if path.starts_with(&watch_dir) {
                                pending_downloads.insert(path, Instant::now());
                            } else if path.starts_with(&vault_dir) && path.extension().map(|e| e == "md").unwrap_or(false) {
                                pending_vault.insert(path, Instant::now());
                            }
                        }
                    }
                }
            }
            Ok(Err(e)) => emit_status(&app, "warn", &format!("Watch error: {}", e)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }

        // Process Downloads
        let ready_downloads: Vec<PathBuf> = pending_downloads
            .iter()
            .filter(|(_, t)| t.elapsed() >= Duration::from_secs(1))
            .map(|(p, _)| p.clone())
            .collect();

        for path in ready_downloads {
            pending_downloads.remove(&path);
            handle_new_file(&app, &path, &vault_dir);
        }

        // Process Vault
        let ready_vault: Vec<PathBuf> = pending_vault
            .iter()
            .filter(|(_, t)| t.elapsed() >= Duration::from_secs(2))
            .map(|(p, _)| p.clone())
            .collect();

        if !ready_vault.is_empty() {
            let s = settings::load();
            let mut index = embeddings::VaultIndex::load(&s.embeddings_path).unwrap_or_else(|_| embeddings::VaultIndex::new());
            let mut changed = false;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
            
            for path in ready_vault {
                pending_vault.remove(&path);
                
                let rel_path = match path.strip_prefix(&vault_dir) {
                    Ok(p) => p.to_string_lossy().into_owned(),
                    Err(_) => continue,
                };
                
                index.remove_by_path(&rel_path);
                changed = true;
                
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let pinned = content.starts_with("---
") && content.contains("pinned: true");
                    let chunks: Vec<String> = content
                        .split("\n\n")
                        .filter(|c| !c.trim().is_empty())
                        .map(|c| c.to_string())
                        .collect();
                    if chunks.is_empty() { continue; }
                    
                        if let Ok(vectors) = crate::ollama::embed(chunks.clone(), "nomic-embed-text:latest").await {
                            for (chunk, vector) in chunks.into_iter().zip(vectors) {
                                index.add(&vector, embeddings::ChunkMeta {
                                    path: rel_path.clone(),
                                    chunk,
                                    created_at: now,
                                    last_accessed: now,
                                    access_count: 0,
                                    pinned,
                                    vector: vec![],
                                });
                            }
                        }
                        
                        // Idea 3: Semantic Auto-linking (Synto)
                        if !content.contains("[[") {
                            let prompt = format!("Agis comme un système Zettelkasten. Lis cette note et déduis 3 ou 4 concepts clés pertinents. Renvoie UNIQUEMENT ces concepts sous forme de liens wiki (ex: [[Concept1]] [[Concept2]]). Pas de phrases, juste les liens.\n\nNote:\n{}", content);
                            let model = s.agents.light_model.clone();
                            if let Ok(links_resp) = crate::ollama::chat_once(vec![serde_json::json!({"role": "user", "content": prompt})], &model).await {
                                let links = links_resp.trim();
                                if links.contains("[[") {
                                    let new_content = format!("{}\n\n---\n**Liens Sémantiques Auto:** {}", content.trim(), links);
                                    let _ = std::fs::write(&path, new_content);
                                }
                            }
                        }
                    }
            }
            if changed {
                embeddings::save_index(index, &s.embeddings_path);
                emit_status(&app, "online", "Index updated incrementally");
            }
        }
    }

    emit_status(&app, "offline", "Archivist stopped");
}



fn handle_new_file(app: &AppHandle, path: &PathBuf, vault_dir: &PathBuf) {
    if !path.is_file() { return; }

    let filename = match path.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => return,
    };

    let category = match categorize_file(&filename) {
        Some(c) => c,
        None => return,
    };

    let category_dir = vault_dir.join(category);
    let _ = std::fs::create_dir_all(&category_dir);

    let dest = category_dir.join(&filename);
    if dest.exists() { return; }

    // Try atomic rename first; fall back to copy+delete for cross-device moves
    let result = std::fs::rename(path, &dest).or_else(|_| {
        std::fs::copy(path, &dest).and_then(|_| std::fs::remove_file(path))
    });

    match result {
        Ok(_) => emit_status(app, "online", &format!("Filed: {} → {}/", filename, category)),
        Err(e) => emit_status(app, "warn", &format!("Failed to move {}: {}", filename, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_known_extensions() {
        assert_eq!(categorize_file("report.pdf"), Some("documents"));
        assert_eq!(categorize_file("photo.jpg"), Some("images"));
        assert_eq!(categorize_file("photo.jpeg"), Some("images"));
        assert_eq!(categorize_file("photo.PNG"), Some("images"));
        assert_eq!(categorize_file("notes.md"), Some("notes"));
        assert_eq!(categorize_file("notes.txt"), Some("notes"));
        assert_eq!(categorize_file("archive.zip"), Some("archives"));
        assert_eq!(categorize_file("code.rs"), Some("code"));
    }

    #[test]
    fn test_categorize_unknown_extension_returns_none() {
        assert_eq!(categorize_file("binary.exe"), None);
        assert_eq!(categorize_file("noextension"), None);
    }
}

import re

content = open("src-tauri/src/archivist.rs").read()

content = content.replace("use tauri::{AppHandle, Emitter};", "use tauri::{AppHandle, Emitter};\nuse crate::{embeddings, settings, ollama, vault};")

# Change archivist to watch vault_path instead of Downloads, or both.
# But wait, it's easier to rewrite the watcher to watch settings::load().vault_path and use embeddings API.

new_run = """pub async fn run_archivist(app: AppHandle, running: Arc<AtomicBool>) {
    let s = settings::load();
    let vault_dir = std::path::PathBuf::from(&s.vault_path);

    if let Err(e) = std::fs::create_dir_all(&vault_dir) {
        emit_status(&app, "error", &format!("Cannot create vault dir: {}", e));
        return;
    }

    emit_status(&app, "online", &format!("Watching {}", vault_dir.display()));

    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            emit_status(&app, "error", &format!("Watcher init failed: {}", e));
            return;
        }
    };

    if let Err(e) = watcher.watch(&vault_dir, RecursiveMode::Recursive) {
        emit_status(&app, "error", &format!("Watch failed: {}", e));
        return;
    }

    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();

    while running.load(Ordering::Relaxed) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) => {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    for path in event.paths {
                        if path.is_file() && path.extension().map(|e| e == "md").unwrap_or(false) {
                            pending.insert(path, Instant::now());
                        }
                    }
                }
            }
            Ok(Err(e)) => emit_status(&app, "warn", &format!("Watch error: {}", e)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }

        let ready: Vec<PathBuf> = pending
            .iter()
            .filter(|(_, t)| t.elapsed() >= Duration::from_secs(2))
            .map(|(p, _)| p.clone())
            .collect();

        if !ready.is_empty() {
            let s = settings::load();
            let mut index = (*embeddings::load_index(&s.embeddings_path)).clone();
            let mut changed = false;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
            
            for path in ready {
                pending.remove(&path);
                
                let rel_path = match path.strip_prefix(&vault_dir) {
                    Ok(p) => p.to_string_lossy().into_owned(),
                    Err(_) => continue,
                };
                
                // 1. Remove all chunk IDs associated with that file path.
                index.remove_by_path(&rel_path);
                changed = true;
                
                // 2. Re-chunk the file.
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let pinned = content.starts_with("---\\n") && content.contains("pinned: true");
                    let chunks = embeddings::chunk_text(&content, 400, 50);
                    if chunks.is_empty() { continue; }
                    
                    // 3. Embed new chunks via Ollama.
                    if let Ok(vectors) = ollama::embed(chunks.clone(), "nomic-embed-text:latest").await {
                        // 4. index.add() each new chunk
                        for (chunk, vector) in chunks.into_iter().zip(vectors) {
                            index.add(&vector, embeddings::ChunkMeta {
                                path: rel_path.clone(),
                                chunk,
                                created_at: now,
                                last_accessed: now,
                                access_count: 0,
                                pinned,
                            });
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
"""

content = re.sub(r"pub async fn run_archivist.*?emit_status\(&app, \"offline\", \"Archivist stopped\"\);\n}", new_run, content, flags=re.DOTALL)
open("src-tauri/src/archivist.rs", "w").write(content)

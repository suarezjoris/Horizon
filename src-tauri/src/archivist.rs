use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use notify::{Watcher, RecursiveMode, EventKind};
use tauri::{AppHandle, Emitter};

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
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => {
            emit_status(&app, "error", "Cannot locate home directory");
            return;
        }
    };

    let watch_dir = home.join("Downloads");
    let vault_dir = home.join("Documents/Horizon_Vault/Sorted_Intel");

    if let Err(e) = std::fs::create_dir_all(&vault_dir) {
        emit_status(&app, "error", &format!("Cannot create vault dir: {}", e));
        return;
    }

    emit_status(&app, "online", &format!("Watching {}", watch_dir.display()));

    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            emit_status(&app, "error", &format!("Watcher init failed: {}", e));
            return;
        }
    };

    if let Err(e) = watcher.watch(&watch_dir, RecursiveMode::NonRecursive) {
        emit_status(&app, "error", &format!("Watch failed: {}", e));
        return;
    }

    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();

    while running.load(Ordering::Relaxed) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) => {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    for path in event.paths {
                        if path.is_file() {
                            pending.insert(path, Instant::now());
                        }
                    }
                }
            }
            Ok(Err(e)) => emit_status(&app, "warn", &format!("Watch error: {}", e)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }

        // Only process files that haven't changed for 1 second (fully written/closed)
        let ready: Vec<PathBuf> = pending
            .iter()
            .filter(|(_, t)| t.elapsed() >= Duration::from_secs(1))
            .map(|(p, _)| p.clone())
            .collect();

        for path in ready {
            pending.remove(&path);
            handle_new_file(&app, &path, &vault_dir);
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

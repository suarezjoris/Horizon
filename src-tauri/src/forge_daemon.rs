use std::path::PathBuf;

pub fn extract_pdf(path: &PathBuf) -> Option<String> {
    let out = std::process::Command::new("pdftotext")
        .arg(path)
        .arg("-")
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).chars().take(4000).collect())
    } else {
        None
    }
}

pub fn extract_zip_xml(path: &PathBuf) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut text = String::new();

    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.name().to_string();
        let is_content = name == "word/document.xml"
            || (name.starts_with("ppt/slides/slide") && name.ends_with(".xml"));
        if !is_content { continue; }

        let mut raw = String::new();
        if std::io::Read::read_to_string(&mut entry, &mut raw).is_ok() {
            text.push_str(&strip_xml(&raw));
            text.push(' ');
        }
    }

    if text.trim().is_empty() { return None; }
    Some(text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(4000).collect())
}

fn strip_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len() / 2);
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

pub fn url_slug(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(60)
        .collect()
}

pub fn find_orphans(vault_path: &str) -> Vec<PathBuf> {
    let base = PathBuf::from(vault_path);
    let mut orphans = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") { continue; }
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            if !content.contains("[[") {
                orphans.push(path);
            }
        }
    }
    orphans
}

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use notify::{Watcher, RecursiveMode, EventKind};

fn emit_status(app: &AppHandle, msg: &str) {
    let _ = app.emit("armata-agent-status", serde_json::json!({
        "agent": "forge",
        "status": "online",
        "message": msg
    }));
}

async fn ingest_binary(app: &AppHandle, path: &PathBuf, vault_path: &str, model: &str) {
    let filename = match path.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => return,
    };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    let text = match ext.as_str() {
        "pdf" => extract_pdf(path),
        "docx" | "pptx" | "xlsx" => extract_zip_xml(path),
        _ => return,
    };

    let text = match text {
        Some(t) if t.split_whitespace().count() > 30 => t,
        _ => {
            emit_status(app, &format!("Skipped (no text): {}", filename));
            return;
        }
    };

    emit_status(app, &format!("Ingesting: {}", filename));

    let prompt = format!(
        "Extract the key knowledge from this document into a structured markdown note. Include a title, summary, and bullet-point key facts. Be concise.\n\nDocument: {}\n\nContent:\n{}",
        filename,
        &text.chars().take(3000).collect::<String>()
    );
    let msgs = vec![serde_json::json!({"role": "user", "content": prompt})];
    let summary = match crate::ollama::chat_once(msgs, model).await {
        Ok(s) => s,
        Err(_) => {
            emit_status(app, &format!("Ingest failed: {}", filename));
            return;
        }
    };

    let slug = url_slug(&filename);
    let dest = PathBuf::from(vault_path).join("knowledge").join(format!("{}.md", slug));
    if dest.exists() { return; }
    let _ = std::fs::create_dir_all(dest.parent().unwrap());
    let clean_summary: String = summary.chars()
        .map(|c| if c.is_control() && c != '\n' && c != '\r' && c != '\t' { ' ' } else { c })
        .collect();
    let _ = std::fs::write(&dest, format!("# {}\n\n**Source:** {}\n\n{}\n", filename, path.display(), clean_summary));
    emit_status(app, &format!("Ingested: {} → knowledge/{}.md", filename, slug));
}

pub async fn run_forge(app: AppHandle, running: Arc<AtomicBool>) {
    let s = crate::settings::load();
    let vault_path = s.vault_path.clone();
    let light_model = s.agents.light_model.clone();

    let home = dirs::home_dir().unwrap_or_default();
    let sorted_intel = home.join("Documents/Horizon_Vault/Sorted_Intel/documents");
    let vanguard_dir = PathBuf::from(&vault_path).join("vanguard");

    let _ = std::fs::create_dir_all(&sorted_intel);
    let _ = std::fs::create_dir_all(&vanguard_dir);

    emit_status(&app, "Vault Consolidator active");

    emit_status(&app, "Running vault health check…");
    crate::memory::ensure_hub_notes().await;
    let purged = crate::memory::purge_empty_vanguard_files(&vault_path);
    let repaired = crate::memory::repair_all_orphans(&vault_path);
    let cross_linked = crate::memory::enrich_cross_links(&vault_path);
    match (purged, repaired, cross_linked) {
        (0, 0, 0) => emit_status(&app, "Vault healthy — no repairs needed"),
        (p, r, c) => emit_status(&app, &format!(
            "Vault repaired: {} purged, {} orphans tagged, {} cross-linked",
            p, r, c
        )),
    }

    emit_status(&app, "Distilling Vanguard news into brain…");
    let learned = crate::memory::distill_vanguard_to_hubs(Some(&app)).await;
    if learned > 0 {
        emit_status(&app, &format!("Learned {} facts from Vanguard", learned));
    }

    let refined = crate::memory::refine_messy_notes(Some(&app)).await;
    if refined > 0 {
        emit_status(&app, &format!("Refined {} messy notes", refined));
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            let _ = app.emit("armata-agent-status", serde_json::json!({
                "agent": "forge", "status": "error",
                "message": format!("Watcher init failed: {}", e)
            }));
            return;
        }
    };

    // Only watch external input dirs — NOT the vault root itself.
    // Watching vault root causes a feedback loop: Forge writes → event → Forge runs again.
    let _ = watcher.watch(&sorted_intel, RecursiveMode::NonRecursive);
    let _ = watcher.watch(&vanguard_dir, RecursiveMode::NonRecursive);

    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut last_event: Option<Instant> = None;
    let mut last_consolidation: Option<Instant> = None;
    let mut last_orphan_scan = Instant::now();
    let mut last_hub_scan = Instant::now();
    let orphan_interval = Duration::from_secs(2 * 60 * 60);
    let hub_scan_interval = Duration::from_secs(4 * 60 * 60); // every 4h regardless of file activity
    let debounce = Duration::from_secs(60);
    let consolidation_cooldown = Duration::from_secs(10 * 60);

    while running.load(Ordering::Relaxed) {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Ok(event)) => {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    for path in event.paths {
                        if path.is_file() {
                            pending.insert(path);
                            last_event = Some(Instant::now());
                        }
                    }
                }
            }
            Ok(Err(_)) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }

        if let Some(t) = last_event {
            let cooled_down = last_consolidation
                .map(|lc| lc.elapsed() >= consolidation_cooldown)
                .unwrap_or(true);

            if t.elapsed() >= debounce && !pending.is_empty() && cooled_down {
                let to_ingest: Vec<PathBuf> = pending.iter()
                    .filter(|p| {
                        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                        matches!(ext.as_str(), "pdf" | "docx" | "pptx" | "xlsx")
                    })
                    .cloned()
                    .collect();

                for path in &to_ingest {
                    ingest_binary(&app, path, &vault_path, &light_model).await;
                }

                pending.clear();
                last_event = None;
                last_consolidation = Some(Instant::now());

                emit_status(&app, "Distilling Vanguard news into brain…");
                let learned = crate::memory::distill_vanguard_to_hubs(Some(&app)).await;
                if learned > 0 {
                    emit_status(&app, &format!("Learned {} facts from Vanguard", learned));
                }

                emit_status(&app, "Refining notes…");
                let refined = crate::memory::refine_messy_notes(Some(&app)).await;
                if refined > 0 {
                    emit_status(&app, &format!("Refined {} notes", refined));
                }

                emit_status(&app, "Consolidating vault…");
                match crate::memory::consolidate_vault_inner().await {
                    Ok(msg) => emit_status(&app, &msg),
                    Err(e) => emit_status(&app, &format!("Consolidation error: {}", e)),
                }

                crate::memory::propose_new_hubs(&app).await;
            } else if t.elapsed() >= debounce && !pending.is_empty() && !cooled_down {
                // Drain pending without running consolidation — cooldown not expired
                pending.clear();
                last_event = None;
            }
        }

        if last_orphan_scan.elapsed() >= orphan_interval {
            last_orphan_scan = Instant::now();
            let orphans = find_orphans(&vault_path);
            if !orphans.is_empty() {
                emit_status(&app, &format!("Found {} orphan nodes — consolidating", orphans.len()));
                last_consolidation = Some(Instant::now());
                crate::memory::distill_vanguard_to_hubs(Some(&app)).await;
                match crate::memory::consolidate_vault_inner().await {
                    Ok(msg) => emit_status(&app, &msg),
                    Err(e) => emit_status(&app, &format!("Orphan consolidation error: {}", e)),
                }
            }
        }

        if last_hub_scan.elapsed() >= hub_scan_interval {
            last_hub_scan = Instant::now();
            crate::memory::propose_new_hubs(&app).await;
        }
    }

    let _ = app.emit("armata-agent-status", serde_json::json!({
        "agent": "forge", "status": "offline", "message": "Forge stopped"
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_xml_removes_tags() {
        let xml = "<w:t>Hello</w:t><w:t> World</w:t>";
        assert_eq!(strip_xml(xml).trim(), "Hello World");
    }

    #[test]
    fn test_url_slug_basic() {
        assert_eq!(url_slug("My Report.pdf"), "my-report-pdf");
    }

    #[test]
    fn test_find_orphans_detects_no_wikilinks() {
        let dir = std::env::temp_dir().join("forge_orphan_test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lonely.md"), "# Lonely\nNo links here.").unwrap();
        std::fs::write(dir.join("connected.md"), "# Connected\nSee [[lonely]].").unwrap();

        let orphans = find_orphans(dir.to_str().unwrap());
        assert_eq!(orphans.len(), 1);
        assert!(orphans[0].file_name().unwrap() == "lonely.md");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

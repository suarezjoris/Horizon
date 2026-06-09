use crate::{embeddings, ollama, settings, vault};
use tauri::Emitter;

const TOP_K: usize = 3; 
const MAX_CONTEXT_CHARS: usize = 4000; 

pub async fn get_context(query: &str) -> String {
    let s = settings::load();
    let index = embeddings::load_index(&s.embeddings_path);
    let mut blocks = Vec::new();
    let mut current_len = 0;

    // 1. Semantic search with graph expansion
    if !index.is_empty() {
        if let Ok(vecs) = ollama::embed(vec![query.to_string()], "nomic-embed-text:latest").await {
            if let Some(qvec) = vecs.into_iter().next() {
                let results = embeddings::search(&index, &qvec, TOP_K);
                // Optimize: Pre-allocate HashSet
                let mut seen: std::collections::HashSet<String> = std::collections::HashSet::with_capacity(TOP_K * 3);
                
                for entry in results {
                    if current_len >= MAX_CONTEXT_CHARS { break; }
                    if !seen.contains(&entry.path) {
                        seen.insert(entry.path.clone());
                        let block = format!("### {}\n{}", entry.path, entry.chunk);
                        current_len += block.len();
                        blocks.push(block);
                        
                        // Follow wikilinks in this chunk (Graph Expansion)
                        for link in vault::extract_wikilinks(&entry.chunk) {
                            if current_len >= MAX_CONTEXT_CHARS { break; }
                            let md = if link.ends_with(".md") { link.to_string() } else { format!("{}.md", link) };
                            if !seen.contains(&md) && vault::validate_rel_path(&md).is_ok() {
                                seen.insert(md.clone());
                                if let Ok(c) = vault::read_vault_note(&s.vault_path, &md) {
                                    let b = format!("### {}\n{}", md, c);
                                    current_len += b.len();
                                    blocks.push(b);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    blocks.join("\n\n")
}

#[tauri::command]
pub async fn process_calibration(text: String) -> Result<String, String> {
    let s = settings::load();
    let prompt = format!(
        "You are the Core Archetype Weaver. Analyze this 'Initial Brain Dump' from the user and create a structured, linked memory vault.
        
        Rules:
        - Detect themes and create relevant notes (e.g., identity.md, tech_stack.md, passions.md).
        - Use [[wikilinks]] to connect related notes.
        - Output format: RAW JSON object where keys are filenames (e.g., 'identity.md') and values are markdown content.

        BRAIN DUMP:
        {}",
        text
    );

    let resp = ollama::chat_once(vec![serde_json::json!({"role": "user", "content": prompt})], &s.llm_model).await?;
    
    let json_str = resp.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let data: serde_json::Value = serde_json::from_str(json_str).map_err(|e| format!("Parsing Error: {}", e))?;

    if let Some(obj) = data.as_object() {
        for (file, content) in obj {
            if vault::validate_rel_path(file).is_ok() {
                if let Some(c) = content.as_str() {
                    let _ = vault::write_vault_note(&s.vault_path, file, c);
                }
            }
        }
    }

    Ok("Calibration complete. Emergent brain mapped.".to_string())
}

pub async fn extract_and_save(user_msg: String, ai_msg: String) {
    let s = settings::load();
    let existing_notes = vault::list_notes();
    
    let prompt = format!(
        "Act as a high-level cognitive archiver. Analyze this interaction to update the user's Second Brain.

        Existing Notes: {:?}

        Rules:
        1. Identify the core theme (e.g., identity, a specific project, a skill, a passion).
        2. Decide which note(s) to update or if a NEW note should be created.
        3. Extract atomic facts and use [[wikilinks]] to connect to other existing or potential notes.
        4. Output format: RAW JSON array of objects.
        
        Exchange:
        User: {}
        AI: {}
        
        Format Example: [{{ \"file\": \"projects/lox.md\", \"fact\": \"User is building an interpreter in [[Python]] from scratch.\" }}]",
        existing_notes, user_msg, ai_msg
    );

    let Ok(resp) = ollama::chat_once(vec![serde_json::json!({"role": "user", "content": prompt})], &s.llm_model).await else {
        return;
    };

    let json_str = resp.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();

    let Ok(facts) = serde_json::from_str::<Vec<serde_json::Value>>(json_str) else {
        return;
    };

    // Build a lookup of existing note stems for wikilink normalization
    let note_stems: Vec<String> = existing_notes.iter()
        .map(|p| {
            p.trim_end_matches(".md")
             .rsplit('/')
             .next()
             .unwrap_or(p)
             .to_string()
        })
        .collect();

    for fact in facts {
        let (Some(file), Some(text)) = (fact["file"].as_str(), fact["fact"].as_str()) else {
            continue;
        };

        if vault::validate_rel_path(file).is_ok() {
            // Normalize [[Wikilinks]] to match actual vault filenames (case + separator)
            let normalized = normalize_wikilinks(text, &note_stems);
            let _ = vault::append_note(&s.vault_path, file, &format!("\n- {}", normalized));
        }
    }
}

/// Replace [[Link Text]] with [[actual_filename]] when a matching vault note exists.
fn normalize_wikilinks(text: &str, note_stems: &[String]) -> String {
    let mut result = text.to_string();
    let mut search = text;
    let mut replacements: Vec<(String, String)> = Vec::new();

    while let Some(open) = search.find("[[") {
        let after = &search[open + 2..];
        if let Some(close) = after.find("]]") {
            let raw = after[..close].trim();
            let normalized_raw = raw.to_lowercase().replace(' ', "_").replace('-', "_");

            // Find the best matching vault note stem
            if let Some(stem) = note_stems.iter().find(|s| {
                let s_norm = s.to_lowercase().replace('-', "_");
                s_norm == normalized_raw
            }) {
                let original = format!("[[{}]]", raw);
                let replacement = format!("[[{}]]", stem);
                if original != replacement {
                    replacements.push((original, replacement));
                }
            }

            search = &after[close + 2..];
        } else {
            break;
        }
    }

    for (from, to) in replacements {
        result = result.replace(&from, &to);
    }
    result
}

/// Strip control characters that break JSON serialization, keeping \t \n \r.
fn sanitize_for_json(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() && c != '\n' && c != '\r' && c != '\t' { ' ' } else { c })
        .collect()
}

const ARCHIVE_PREFIXES: &[&str] = &["vanguard/", "knowledge/", "images/", "characters/"];

fn is_archive(path: &str) -> bool {
    ARCHIVE_PREFIXES.iter().any(|p| path.starts_with(p))
}

// ── Hub-spoke graph system ────────────────────────────────────────────────────
// Topic hubs are the backbone of the knowledge graph. Every note should link to
// at least one hub, creating a connected spoke structure rather than isolated islands.

const HUB_NOTES: &[(&str, &str)] = &[
    ("ai.md",          "# AI & Machine Learning\n\nHub for artificial intelligence, LLMs, neural networks, and ML research.\n"),
    ("security.md",    "# Security\n\nHub for cybersecurity, exploits, CVEs, vulnerabilities, and infosec news.\n"),
    ("linux.md",       "# Linux\n\nHub for Linux, kernel development, distributions, and sysadmin topics.\n"),
    ("programming.md", "# Programming\n\nHub for software development, languages, frameworks, and engineering practices.\n"),
    ("science.md",     "# Science\n\nHub for research, scientific discoveries, and academic topics.\n"),
    ("gaming.md",      "# Gaming\n\nHub for video games, game development, and gaming culture.\n"),
    ("retro.md",       "# Retro Computing\n\nHub for classic computing, retro games, vintage software, and computing history.\n"),
    ("web.md",         "# Web & Internet\n\nHub for web technologies, browsers, networking, and internet culture.\n"),
    ("notes.md",       "# General Notes\n\nHub for miscellaneous topics that don't fit elsewhere.\n"),
];

/// Keyword-based topic detection — deterministic, no LLM.
pub fn detect_topics(title: &str, content: &str) -> Vec<&'static str> {
    let text = format!("{} {}", title, content).to_lowercase();
    let mut topics = Vec::new();

    let rules: &[(&'static str, &[&str])] = &[
        ("ai", &["artificial intelligence", " ai ", "machine learning", "llm", "neural network",
                  "deep learning", "gpt", "ollama", "diffusion", "transformer", "chatgpt",
                  "openai", "claude", "gemini", "github copilot", "rag", "embedding"]),
        ("security", &["security", "exploit", "cve-", "vulnerability", "malware", "hack",
                        "breach", "phishing", "zero-day", "ransomware", "worm", "stealers",
                        "injection", "xss", "trojan", "botnet", "spyware", "infosec", "ctf",
                        "pentest", "reverse engineer"]),
        ("linux", &["linux", "kernel", "ubuntu", "debian", "arch", "fedora", "systemd",
                     "bash", "posix", "unix", "distro", "wayland", "x11", "gtk"]),
        ("programming", &["programming", "software", "developer", "code", "rust", "python",
                           "javascript", "typescript", "golang", "haskell", "c++", "c#",
                           "compiler", "algorithm", "api", "library", "framework", "open source",
                           "github", "git", "refactor", "devops", "ci/cd"]),
        ("science", &["science", "research", "study", "biology", "physics", "chemistry",
                       "space", "nasa", "discovery", "experiment", "quantum", "climate"]),
        ("gaming", &["game", "steam", "gaming", "playstation", "xbox", "nintendo",
                      "esports", "fps", "rpg", "mmo", "indie game"]),
        ("retro", &["retro", "classic", "vintage", "1990s", "1980s", "1970s", "dos",
                     "commodore", "amiga", "8-bit", "16-bit", "old school", "nostalgia"]),
        ("web", &["web", "browser", "http", "html", "css", "internet", "rss", "dns",
                   "cdn", "frontend", "backend", "saas", "api"]),
    ];

    for (topic, keywords) in rules {
        if keywords.iter().any(|kw| text.contains(kw)) {
            topics.push(*topic);
        }
    }

    if topics.is_empty() {
        topics.push("notes");
    }
    topics
}

/// Create hub notes that don't yet exist. Safe to call repeatedly.
pub async fn ensure_hub_notes() {
    let s = settings::load();
    for (filename, content) in HUB_NOTES {
        let path = std::path::PathBuf::from(&s.vault_path).join(filename);
        if !path.exists() {
            let _ = vault::write_vault_note(&s.vault_path, filename, content);
        }
    }
}

fn note_stem(rel_path: &str) -> &str {
    rel_path
        .trim_end_matches(".md")
        .rsplit('/')
        .next()
        .unwrap_or(rel_path)
}

/// Add hub [[wikilinks]] to any note that has none (orphan pass).
/// A note that only links to itself is still treated as an orphan.
fn link_orphan(vault_path: &str, rel_path: &str, content: &str) -> bool {
    let self_stem = note_stem(rel_path).to_lowercase();
    let real_links: Vec<_> = vault::extract_wikilinks(content)
        .into_iter()
        .filter(|l| l.to_lowercase() != self_stem)
        .collect();
    if !real_links.is_empty() { return false; }

    let stem = note_stem(rel_path);
    let title = stem.replace('-', " ").replace('_', " ");
    let topics = detect_topics(&title, content);

    let link_line = topics.iter()
        .map(|t| format!("[[{}]]", t))
        .collect::<Vec<_>>()
        .join(" ");

    let new_content = format!("{}\n\nTopics: {}\n", content.trim_end(), link_line);
    vault::write_vault_note(vault_path, rel_path, &new_content).is_ok()
}

/// Scan every note and append cross-links to other personal notes whose name
/// appears verbatim in the content. Runs on ALL notes (not just orphans) so
/// that already-tagged notes get enriched as the vault grows.
/// Returns number of notes updated.
pub fn enrich_cross_links(vault_path: &str) -> usize {
    let hub_names: std::collections::HashSet<String> =
        HUB_NOTES.iter().map(|(n, _)| note_stem(n).to_string()).collect();

    let all_notes = vault::list_notes();
    let mut updated = 0usize;

    for rel_path in &all_notes {
        if is_archive(rel_path) { continue; }

        let content = match vault::read_note(rel_path.clone()) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let self_stem = note_stem(rel_path).to_string();
        let text_lower = format!("{} {}", self_stem.replace('_', " ").replace('-', " "), content)
            .to_lowercase();

        let mut new_links: Vec<String> = Vec::new();

        for other_path in &all_notes {
            if other_path == rel_path { continue; }
            if is_archive(other_path) { continue; }

            let other_stem = note_stem(other_path).to_string();
            if hub_names.contains(&other_stem) { continue; }

            // Skip if we already link to this note
            let bracket = format!("[[{}]]", other_stem);
            if content.contains(&bracket) { continue; }

            let keyword = other_stem.replace('_', " ").replace('-', " ").to_lowercase();
            if keyword.len() >= 4 && text_lower.contains(&keyword) {
                new_links.push(other_stem);
            }
        }

        if new_links.is_empty() { continue; }

        new_links.sort();
        new_links.dedup();

        let additions = new_links.iter()
            .map(|l| format!("[[{}]]", l))
            .collect::<Vec<_>>()
            .join(" ");

        // Append to existing Topics line if present, otherwise add new line
        let new_content = if let Some(pos) = content.rfind("Topics:") {
            let (before, after) = content.split_at(pos + "Topics:".len());
            format!("{} {}{}", before, additions, after)
        } else {
            format!("{}\n\nTopics: {}\n", content.trim_end(), additions)
        };

        if vault::write_vault_note(vault_path, rel_path, &new_content).is_ok() {
            updated += 1;
        }
    }
    updated
}

/// Wire all orphan notes (core + archive) into the hub graph.
pub fn repair_all_orphans(vault_path: &str) -> usize {
    let notes = vault::list_notes();
    let mut fixed = 0usize;

    for rel_path in &notes {
        if let Ok(content) = vault::read_note(rel_path.clone()) {
            if link_orphan(vault_path, rel_path, &content) {
                fixed += 1;
            }
        }
    }
    fixed
}

/// Delete individual vanguard article files with no real content (old per-article format).
/// Leaves digest-*.md files alone. Returns number of files removed.
pub fn purge_empty_vanguard_files(vault_path: &str) -> usize {
    let vanguard_dir = std::path::PathBuf::from(vault_path).join("vanguard");
    let entries = match std::fs::read_dir(&vanguard_dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };

    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.ends_with(".md") || name.starts_with("digest-") { continue; }

        let content = std::fs::read_to_string(&path).unwrap_or_default();
        // Empty = nothing beyond title / **Source:** / **Date:** header lines
        let has_body = content.lines().any(|l| {
            let t = l.trim();
            !t.is_empty()
                && !t.starts_with("# ")
                && !t.starts_with("**Source:**")
                && !t.starts_with("**Date:**")
        });

        if !has_body {
            let _ = std::fs::remove_file(&path);
            removed += 1;
        }
    }
    removed
}

// ── Archive insight extraction (additive — never rewrites) ───────────────────

const BATCH_SIZE: usize = 8;
const FILE_CAP: usize = 1_800;

/// Extract key insights from archive files (vanguard/, knowledge/) and append
/// them to `vault/insights.md`. Uses the lighter model to stay fast.
pub async fn extract_archive_insights(app: Option<&tauri::AppHandle>) -> usize {
    let s = settings::load();
    let notes = vault::list_notes();
    // Only digest-format vanguard files (not old individual article files)
    let archive_notes: Vec<_> = notes.iter()
        .filter(|p| is_archive(p) && !p.contains("digest-"))
        .cloned()
        .collect();

    if archive_notes.is_empty() { return 0; }

    let mut new_insights = 0usize;

    for batch in archive_notes.chunks(BATCH_SIZE) {
        let mut block = String::new();
        for rel_path in batch {
            if let Ok(content) = vault::read_note(rel_path.clone()) {
                let trimmed = sanitize_for_json(&content).chars().take(800).collect::<String>();
                if trimmed.trim().is_empty() { continue; }
                block.push_str(&format!("\n--- {}\n{}\n", rel_path, trimmed));
            }
        }
        if block.trim().is_empty() { continue; }

        let prompt = format!(
            "Extract 3-5 key facts or insights from these documents as concise bullet points. \
Be specific. No intro text, just the bullets.\n\n{}",
            block
        );

        if let Ok(resp) = ollama::chat_once(
            vec![serde_json::json!({"role": "user", "content": prompt})],
            &s.agents.light_model,
        )
        .await
        {
            if !resp.trim().is_empty() {
                let date = chrono::Local::now().format("%Y-%m-%d").to_string();
                let entry = format!("\n\n## Insights {}\n{}", date, resp.trim());
                let _ = vault::append_note(&s.vault_path, "insights.md", &entry);
                new_insights += batch.len();

                if let Some(app) = app {
                    let _ = app.emit("armata-agent-status", serde_json::json!({
                        "agent": "forge",
                        "status": "online",
                        "message": format!("Insights extracted from {} archive files", batch.len())
                    }));
                }
            }
        }
    }

    new_insights
}

pub async fn consolidate_vault_inner() -> Result<String, String> {
    let s = settings::load();

    // Step 1: ensure hub anchors exist
    ensure_hub_notes().await;

    // Step 2: tag orphan notes with hub topics
    let orphans_fixed = repair_all_orphans(&s.vault_path);

    // Step 3: enrich all notes with cross-links to other personal notes
    let cross_linked = enrich_cross_links(&s.vault_path);

    // Step 4: rebuild embeddings so RAG reflects the updated vault
    let indexed = crate::embeddings::reindex().await.unwrap_or(0);

    let total = vault::list_notes().len();
    Ok(format!(
        "Graph repair complete: {} orphans tagged, {} cross-linked, {} chunks indexed ({} notes).",
        orphans_fixed, cross_linked, indexed, total
    ))
}

#[tauri::command]
pub async fn consolidate_vault() -> Result<String, String> {
    consolidate_vault_inner().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consolidate_vault_inner_is_pub() {
        let _: fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>> =
            || Box::pin(consolidate_vault_inner());
    }
}


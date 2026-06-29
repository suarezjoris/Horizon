use crate::{embeddings, ollama, settings, vault};
use tauri::Emitter;

const TOP_K: usize = 8;
const MAX_CONTEXT_CHARS: usize = 8000;

fn is_introspective_query(q: &str) -> bool {
    let q = q.to_lowercase();
    // Personal-pronoun or memory-recall patterns in French and English
    let markers = [
        "mon ", "mes ", "ma ", "moi", "je ", "j'", "selon ta", "ta mémoire", "ta memoire",
        "my ", "about me", "who am i", "i am", "my project", "what do i",
    ];
    markers.iter().any(|m| q.contains(m))
}

pub async fn get_context(query: &str) -> String {
    let s = settings::load();
    let index = embeddings::load_index(&s.embeddings_path);
    let mut blocks = Vec::new();
    let mut current_len = 0;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // For introspective queries, always lead with identity.md and passions.md
    // — embedding similarity on French queries misses these critical notes
    if is_introspective_query(query) {
        for anchor in &["identity.md", "passions.md", "tech_stack.md"] {
            if let Ok(c) = vault::read_vault_note(&s.vault_path, anchor) {
                seen.insert(anchor.to_string());
                let block = format!("### {}\n{}", anchor, c);
                current_len += block.len();
                blocks.push(block);
            }
        }
    }

    if !index.is_empty() {
        let q = ollama::nomic_prefix("nomic-embed-text:latest", ollama::NomicTask::Query, query);
        if let Ok(vecs) = ollama::embed(vec![q], "nomic-embed-text:latest").await {
            if let Some(qvec) = vecs.into_iter().next() {
                let results = index.search_hybrid(&qvec, query, TOP_K, !is_introspective_query(query));

                let ids: Vec<u64> = results.iter().map(|r| r.id).collect();
                if !ids.is_empty() {
                    index.update_access_stats(&ids);
                    let index_clone = std::sync::Arc::clone(&index);
                    let path_clone = s.embeddings_path.clone();
                    tokio::task::spawn_blocking(move || {
                        let _ = index_clone.save(&path_clone);
                    });
                }

                for entry in results {
                    if current_len >= MAX_CONTEXT_CHARS { break; }
                    if !seen.contains(&entry.path) {
                        seen.insert(entry.path.clone());
                        let block = format!("### {}\n{}", entry.path, entry.chunk);
                        current_len += block.len();
                        blocks.push(block);

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

/// Map a calibration section header to its destination note. Matches on the
/// start of the uppercased header, so trailing parentheticals are tolerated
/// (e.g. "MA VOIX (extraits…)").
fn calibration_section_file(line: &str) -> Option<&'static str> {
    let h = line.trim().to_uppercase();
    if h.starts_with("QUI JE SUIS") { Some("identity.md") }
    else if h.starts_with("COMMENT JE PENSE") { Some("mindset.md") }
    else if h.starts_with("CE QUE JE CONSTRUIS") { Some("projects.md") }
    else if h.starts_with("HOBBIES") { Some("passions.md") }
    else if h.starts_with("MA VOIX") { Some("voice.md") }
    else if h.starts_with("NON-NÉGOCIABLES") || h.starts_with("NON-NEGOCIABLES") { Some("directives.md") }
    else { None }
}

/// Split a sectioned calibration dump into (note_file, body) pairs, verbatim.
/// Text before the first recognised header lands in identity.md.
pub fn split_calibration_sections(text: &str) -> Vec<(String, String)> {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current = "identity.md".to_string();
    let mut buf = String::new();

    let flush = |file: &str, buf: &mut String, out: &mut Vec<(String, String)>| {
        if !buf.trim().is_empty() {
            out.push((file.to_string(), buf.trim().to_string()));
        }
        buf.clear();
    };

    for line in text.lines() {
        if let Some(file) = calibration_section_file(line) {
            flush(&current, &mut buf, &mut sections);
            current = file.to_string();
        } else {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    flush(&current, &mut buf, &mut sections);
    sections
}

#[tauri::command]
pub async fn process_calibration(text: String) -> Result<String, String> {
    let s = settings::load();

    // Deterministic, verbatim — no LLM. Summarising the dump would dilute the
    // user's own words (especially the voice samples), which defeats the point.
    let mut written = 0usize;
    for (file, body) in split_calibration_sections(&text) {
        if vault::validate_rel_path(&file).is_err() { continue; }
        let title = file.trim_end_matches(".md");
        let content = format!("# {}\n\n{}\n", title, body);
        if vault::write_vault_note(&s.vault_path, &file, &content).is_ok() {
            written += 1;
        }
    }

    crate::embeddings::reindex().await.ok();
    Ok(format!("Calibration complete: {} sections saved.", written))
}

pub async fn extract_and_save(user_msg: String, ai_msg: String, vram: std::sync::Arc<tokio::sync::Semaphore>) {
    // Skip trivial exchanges — nothing worth extracting
    if user_msg.trim().len() < 30 {
        return;
    }

    // Non-blocking: if GPU is busy, skip extraction rather than queuing behind foreground work
    let _permit = match std::sync::Arc::clone(&vram).try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return,
    };

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

    let Ok(resp) = ollama::chat_once(vec![serde_json::json!({"role": "user", "content": prompt})], &s.agents.light_model).await else {
        return;
    };

    // Extract JSON array even when the model wraps it in prose or markdown
    let json_str = {
        let r = resp.as_str();
        let start = r.find('[').unwrap_or(0);
        let end = r.rfind(']').map(|i| i + 1).unwrap_or(r.len());
        if start < end { &r[start..end] } else { r }
    };

    let Ok(facts) = serde_json::from_str::<Vec<serde_json::Value>>(json_str) else {
        return;
    };

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
            let normalized = normalize_wikilinks(text, &note_stems);
            let existing = vault::read_vault_note(&s.vault_path, file).unwrap_or_default();
            if is_duplicate_fact(&normalized, &existing) {
                continue;
            }

            let _ = vault::append_note(&s.vault_path, file, &format!("\n- {}", normalized));
        }
    }
}

#[tauri::command]
pub async fn save_to_note(note_hint: String, content: String) -> Result<String, String> {
    let s = settings::load();

    let stem = note_hint.trim().to_lowercase().replace(' ', "_");
    validate_stem(&stem)?;
    let rel_path = format!("{}.md", stem);
    vault::validate_rel_path(&rel_path).map_err(|e| format!("Invalid note name: {}", e))?;

    let topics = detect_topics(&stem, &content);
    let topic_line = topics.iter().map(|t| format!("[[{}]]", t)).collect::<Vec<_>>().join(" ");

    let existing = vault::read_vault_note(&s.vault_path, &rel_path).unwrap_or_default();

    let final_content = if existing.trim().is_empty() {
        format!("# {}\n\n{}\n\nTopics: {}\n", stem.replace('_', " "), content.trim(), topic_line)
    } else {
        let prompt = format!(
            "You are a knowledge vault editor. Merge these two versions of a note into one clean, \
concise result. Rules: remove duplicate facts, remove any AI apology or meta text, keep only true \
factual statements, preserve [[wikilinks]], keep the Topics line at the end.\n\n\
EXISTING NOTE:\n{}\n\nNEW CONTENT TO INTEGRATE:\n{}\n\nTopics line must end with: Topics: {}\n\n\
Output ONLY the final markdown note, nothing else.",
            existing.trim(), content.trim(), topic_line
        );

        let merged = ollama::chat_once(
            vec![serde_json::json!({"role": "user", "content": prompt})],
            &s.agents.light_model,
        )
        .await
        .unwrap_or_else(|_| format!("{}\n\n- {}\n\nTopics: {}\n", existing.trim(), content.trim(), topic_line));

        // Strip markdown code fences if the model added them
        merged
            .trim()
            .trim_start_matches("```markdown")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string()
            + "\n"
    };

    vault::write_vault_note(&s.vault_path, &rel_path, &final_content).map_err(|e| e.to_string())?;

    let indexed = crate::embeddings::reindex().await.unwrap_or(0);
    Ok(format!("Saved to {}, {} chunks indexed.", rel_path, indexed))
}

fn is_duplicate_fact(fact: &str, note_content: &str) -> bool {
    let note_lower = note_content.to_lowercase();
    let significant: Vec<String> = fact
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase())
        .filter(|w| w.len() >= 5)
        .take(12)
        .collect();
    if significant.len() < 3 {
        return false;
    }
    let matches = significant.iter().filter(|w| note_lower.contains(w.as_str())).count();
    matches >= 4
}

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

const ARCHIVE_PREFIXES: &[&str] = &["vanguard/", "knowledge/", "images/", "characters/"];

fn is_archive(path: &str) -> bool {
    ARCHIVE_PREFIXES.iter().any(|p| path.starts_with(p))
}

const SELF_MODEL_MAX_CHARS: usize = 3500;

/// A note that describes the user themselves — excludes hubs, ingested
/// knowledge, news digests, and internal memory files.
pub fn is_personal_note(path: &str, hub_names: &[String]) -> bool {
    if is_archive(path) { return false; }
    if path.starts_with("memory/") { return false; }
    let stem = note_stem(path);
    // `directives` is injected as behaviour rules in the system prompt, not as
    // passive profile context, so it's excluded from the self-model block.
    if stem.starts_with("digest-") || stem == "self_model" || stem == "directives" { return false; }
    !hub_names.iter().any(|h| h == stem)
}

const VOICE_SAMPLE_MAX_CHARS: usize = 1500;

/// User behaviour rules (their non-negotiables), injected into the system
/// prompt as imperative rules. Empty if the user never set any.
pub fn directives_block() -> String {
    let s = settings::load();
    vault::read_vault_note(&s.vault_path, "directives.md")
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Always-loaded self-model: the user's own personal notes, raw (no LLM
/// summarisation — that would dilute their own words), capped in length,
/// identity/passions first. Injected into every assistant-mode system prompt.
pub fn self_model_block() -> String {
    let s = settings::load();
    let hub_names = all_hub_names();
    let mut notes = vault::list_notes();

    let priority = ["identity.md", "voice.md", "mindset.md", "projects.md", "passions.md"];
    notes.sort_by_key(|p| priority.iter().position(|a| *a == p.as_str()).unwrap_or(usize::MAX));

    let mut out = String::new();
    for path in notes {
        if !is_personal_note(&path, &hub_names) { continue; }
        let Ok(c) = vault::read_vault_note(&s.vault_path, &path) else { continue };
        let mut c = c.trim().to_string();
        if c.is_empty() { continue; }
        // Voice logs can be huge; a style sample is enough — don't let it eat
        // the whole budget.
        if path == "voice.md" && c.len() > VOICE_SAMPLE_MAX_CHARS {
            let mut cut = VOICE_SAMPLE_MAX_CHARS;
            while cut < c.len() && !c.is_char_boundary(cut) { cut += 1; }
            c.truncate(cut);
        }
        let block = format!("### {}\n{}\n\n", path, c);
        if out.len() + block.len() > SELF_MODEL_MAX_CHARS { break; }
        out.push_str(&block);
    }
    out.trim().to_string()
}

const DEFAULT_HUBS: &[(&str, &str, &[&str])] = &[];

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct HubDef {
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct HubsFile {
    hubs: Vec<HubDef>,
}

fn validate_stem(stem: &str) -> Result<(), String> {
    if stem.is_empty() { return Err("Name cannot be empty".into()); }
    if stem.starts_with('.') || stem.starts_with('-') {
        return Err("Name cannot start with '.' or '-'".into());
    }
    if !stem.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("Name must contain only letters, digits, underscores, or hyphens".into());
    }
    Ok(())
}

fn hubs_json_path() -> std::path::PathBuf {
    std::path::PathBuf::from(settings::load().vault_path).join("hubs.json")
}

pub fn load_custom_hubs() -> Vec<HubDef> {
    let path = hubs_json_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<HubsFile>(&s).ok())
        .map(|f| f.hubs.into_iter().filter(|h| validate_stem(&h.name).is_ok()).collect())
        .unwrap_or_default()
}

pub fn all_hub_names() -> Vec<String> {
    let mut names: Vec<String> = DEFAULT_HUBS.iter().map(|(n, _, _)| n.to_string()).collect();
    for h in load_custom_hubs() {
        if !names.contains(&h.name) {
            names.push(h.name);
        }
    }
    names
}

pub fn detect_topics(title: &str, content: &str) -> Vec<String> {
    let text = format!("{} {}", title, content).to_lowercase();
    let mut topics = Vec::new();

    for (name, _, keywords) in DEFAULT_HUBS {
        if *name == "notes" { continue; }
        if keywords.iter().any(|kw| text.contains(kw)) {
            topics.push(name.to_string());
        }
    }

    for hub in load_custom_hubs() {
        if topics.contains(&hub.name) { continue; }
        if hub.keywords.iter().any(|kw| text.contains(&kw.to_lowercase())) {
            topics.push(hub.name);
        }
    }

    if topics.is_empty() {
        topics.push("notes".to_string());
    }
    topics
}

pub async fn ensure_hub_notes() {
    let s = settings::load();
    for (name, content, _) in DEFAULT_HUBS {
        let filename = format!("{}.md", name);
        let path = std::path::PathBuf::from(&s.vault_path).join(&filename);
        if !path.exists() {
            let _ = vault::write_vault_note(&s.vault_path, &filename, content);
        }
    }
    for hub in load_custom_hubs() {
        let filename = format!("{}.md", hub.name);
        let path = std::path::PathBuf::from(&s.vault_path).join(&filename);
        if !path.exists() {
            let _ = vault::write_vault_note(&s.vault_path, &filename, &hub.description);
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

pub fn enrich_cross_links(vault_path: &str) -> usize {
    let hub_names: std::collections::HashSet<String> = all_hub_names().into_iter().collect();

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

const MESSY_BULLET_THRESHOLD: usize = 6;

pub async fn refine_messy_notes(app: Option<&tauri::AppHandle>) -> usize {
    let s = settings::load();
    let notes = vault::list_notes();
    let mut refined = 0usize;

    for rel_path in notes.iter().filter(|p| !is_archive(p)) {
        let content = match vault::read_vault_note(&s.vault_path, rel_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let bullet_count = content.lines().filter(|l| l.trim().starts_with("- ")).count();
        if bullet_count < MESSY_BULLET_THRESHOLD { continue; }

        let vram_permit = {
            use tauri::Manager;
            if let Some(app) = app {
                let q = app.state::<crate::vram_queue::VramQueue>();
                q.try_acquire("forge-refine")
            } else {
                None
            }
        };
        if vram_permit.is_none() { continue; }

        let stem = note_stem(rel_path);
        let topics = detect_topics(stem, &content);
        let topic_line = topics.iter().map(|t| format!("[[{}]]", t)).collect::<Vec<_>>().join(" ");

        let prompt = format!(
            "You are a knowledge vault editor. Refine this note into a clean, concise version.\n\
Rules:\n\
- Merge duplicate facts into one statement\n\
- Remove any AI apology, meta-commentary, or conversation text\n\
- Keep only true factual statements about the subject\n\
- Preserve [[wikilinks]]\n\
- End the note with: Topics: {}\n\
Output ONLY the final markdown, nothing else.\n\nNOTE:\n{}",
            topic_line, content.trim()
        );

        let Ok(result) = ollama::chat_once(
            vec![serde_json::json!({"role": "user", "content": prompt})],
            &s.agents.light_model,
        ).await else { drop(vram_permit); continue; };

        drop(vram_permit);

        let clean = result
            .trim()
            .trim_start_matches("```markdown")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string()
            + "\n";

        if vault::write_vault_note(&s.vault_path, rel_path, &clean).is_ok() {
            refined += 1;
            if let Some(app) = app {
                let _ = app.emit("armata-agent-status", serde_json::json!({
                    "agent": "forge",
                    "status": "online",
                    "message": format!("Refined: {}", rel_path)
                }));
            }
        }
    }

    if refined > 0 {
        let _ = crate::embeddings::reindex().await;
    }
    refined
}

pub async fn distill_vanguard_to_hubs(app: Option<&tauri::AppHandle>) -> usize {
    let s = settings::load();
    let vanguard_dir = std::path::PathBuf::from(&s.vault_path).join("vanguard");

    let digests: Vec<_> = match std::fs::read_dir(&vanguard_dir) {
        Ok(entries) => entries.flatten()
            .filter(|e| {
                let n = e.file_name();
                let name = n.to_string_lossy();
                name.starts_with("digest-") && name.ends_with(".md")
            })
            .map(|e| e.path())
            .collect(),
        Err(_) => return 0,
    };

    if digests.is_empty() { return 0; }

    let known_hubs: std::collections::HashSet<String> = all_hub_names().into_iter().collect();
    let vault_kws = crate::vanguard::vault_keywords(&s.vault_path);
    let mut total_facts = 0usize;

    for digest_path in &digests {
        let content = match std::fs::read_to_string(digest_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if content.contains("<!-- vanguard-processed -->") { continue; }

        let articles: Vec<&str> = content.split("\n---\n").collect();
        let mut facts_this_digest = 0usize;

        for article in &articles {
            if article.trim().is_empty() { continue; }
            let title = article.lines()
                .find(|l| l.starts_with("## "))
                .map(|l| l.trim_start_matches("## ").trim())
                .unwrap_or("unknown");

            // Skip articles with no match against vault interests
            if !vault_kws.is_empty() && crate::vanguard::relevance_score(title, article, &vault_kws) == 0 {
                continue;
            }

            let vram_permit = {
                if let Some(app) = app {
                    use tauri::Manager;
                    let q = app.state::<crate::vram_queue::VramQueue>();
                    q.try_acquire("forge-distill")
                } else { None }
            };
            if vram_permit.is_none() { continue; }

            let hub_list = all_hub_names().into_iter()
                .filter(|h| h != "notes")
                .collect::<Vec<_>>()
                .join(", ");

            let prompt = format!(
                "You are a knowledge classifier. Given this news article, do two things:\n\
1. Extract 1-2 specific factual bullet points (start each with '-').\n\
2. Choose the single most relevant hub from this list: {}\n\
   If none fit clearly, use 'notes'.\n\n\
Respond with ONLY valid JSON: {{\"hub\": \"hub_name\", \"facts\": \"- fact1\\n- fact2\"}}\n\n\
Title: {}\n\n{}",
                hub_list,
                title,
                article.chars().take(1200).collect::<String>()
            );

            let Ok(resp) = ollama::chat_once(
                vec![serde_json::json!({"role": "user", "content": prompt})],
                &s.agents.light_model,
            ).await else { drop(vram_permit); continue; };
            drop(vram_permit);

            let json_str = {
                let r = resp.trim();
                let s = r.find('{').unwrap_or(0);
                let e = r.rfind('}').map(|i| i + 1).unwrap_or(r.len());
                r[s..e].to_string()
            };
            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) else { continue; };
            let llm_hub = parsed["hub"].as_str().unwrap_or("notes").to_lowercase();
            let facts = parsed["facts"].as_str().unwrap_or("").trim().to_string();

            if facts.is_empty() { continue; }

            let final_hub = if known_hubs.contains(&llm_hub) { llm_hub } else { "notes".to_string() };
            let hub_file = format!("{}.md", final_hub);
            if vault::validate_rel_path(&hub_file).is_ok() {
                let entry = format!("\n<!-- forge-fact:start -->\n{}\n<!-- forge-fact:end -->", facts);
                let _ = vault::append_note(&s.vault_path, &hub_file, &entry);
                facts_this_digest += 1;

                if let Some(app) = app {
                    let _ = app.emit("armata-agent-status", serde_json::json!({
                        "agent": "forge",
                        "status": "online",
                        "message": format!("Learned: {} → [[{}]]", title.chars().take(50).collect::<String>(), final_hub)
                    }));
                }
            }
        }

        if facts_this_digest > 0 {
            let marked = format!("{}\n<!-- vanguard-processed -->\n", content.trim_end());
            let _ = std::fs::write(digest_path, marked);
            total_facts += facts_this_digest;
        }
    }

    if total_facts > 0 {
        let _ = crate::embeddings::reindex().await;
    }
    total_facts
}

#[tauri::command]
pub fn vault_topic_status() -> serde_json::Value {
    let s = settings::load();
    let known = all_hub_names();
    let notes = vault::list_notes();

    let mut hub_counts: std::collections::HashMap<String, usize> =
        known.iter().map(|h| (h.clone(), 0)).collect();
    let mut uncategorized: Vec<String> = Vec::new();

    for rel_path in notes.iter().filter(|p| !is_archive(p)) {
        let content = match vault::read_vault_note(&s.vault_path, rel_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let topics_line = content.lines().find(|l| l.trim().starts_with("Topics:"));
        let mut matched = false;
        if let Some(line) = topics_line {
            for hub in &known {
                if hub == "notes" { continue; }
                if line.contains(&format!("[[{}]]", hub)) {
                    *hub_counts.entry(hub.clone()).or_insert(0) += 1;
                    matched = true;
                }
            }
        }
        if !matched {
            uncategorized.push(note_stem(rel_path).to_string());
        }
    }

    let mut hub_list: Vec<serde_json::Value> = known.iter()
        .filter(|h| *h != "notes")
        .map(|h| serde_json::json!({ "name": h, "count": hub_counts.get(h).copied().unwrap_or(0) }))
        .collect();
    hub_list.sort_by(|a, b| b["count"].as_u64().cmp(&a["count"].as_u64()));

    serde_json::json!({
        "hubs": hub_list,
        "uncategorized": uncategorized,
        "uncategorized_count": uncategorized.len(),
        "custom_hub_count": load_custom_hubs().len(),
    })
}

pub async fn consolidate_vault_inner() -> Result<String, String> {
    let s = settings::load();

    ensure_hub_notes().await;
    let orphans_fixed = repair_all_orphans(&s.vault_path);
    let cross_linked = enrich_cross_links(&s.vault_path);
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
    fn test_split_calibration_sections() {
        let text = "QUI JE SUIS\nÉtudiant.\n\nMA VOIX (extraits)\ntkt azy mdr\n\nNON-NÉGOCIABLES\nSois positif.";
        let secs = split_calibration_sections(text);
        let map: std::collections::HashMap<_, _> = secs.into_iter().collect();
        assert_eq!(map.get("identity.md").map(|s| s.as_str()), Some("Étudiant."));
        assert_eq!(map.get("voice.md").map(|s| s.as_str()), Some("tkt azy mdr"));
        assert_eq!(map.get("directives.md").map(|s| s.as_str()), Some("Sois positif."));
    }

    #[test]
    fn test_calibration_preamble_goes_to_identity() {
        let secs = split_calibration_sections("just some text no header");
        assert_eq!(secs.len(), 1);
        assert_eq!(secs[0].0, "identity.md");
    }

    #[test]
    fn test_is_personal_note_filters() {
        let hubs = vec!["ai".to_string(), "security".to_string()];
        assert!(is_personal_note("identity.md", &hubs));
        assert!(is_personal_note("passions.md", &hubs));
        assert!(!is_personal_note("ai.md", &hubs));            // hub
        assert!(!is_personal_note("knowledge/foo.md", &hubs)); // ingested
        assert!(!is_personal_note("vanguard/digest-1.md", &hubs));
        assert!(!is_personal_note("memory/user.md", &hubs));   // internal
        assert!(!is_personal_note("self_model.md", &hubs));    // the digest itself
    }

    #[test]
    fn test_consolidate_vault_inner_is_pub() {
        let _: fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>> =
            || Box::pin(consolidate_vault_inner());
    }
}


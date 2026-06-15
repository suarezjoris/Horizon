use crate::{embeddings, ollama, settings, vault};
use tauri::Emitter;

const TOP_K: usize = 3; 
const MAX_CONTEXT_CHARS: usize = 4000; 

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
        if let Ok(vecs) = ollama::embed(vec![query.to_string()], "nomic-embed-text:latest").await {
            if let Some(qvec) = vecs.into_iter().next() {
                let fetch_k = if s.memory_decay.enabled { TOP_K * 3 } else { TOP_K };
                let mut results = index.search(&qvec, fetch_k);
                
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

                if s.memory_decay.enabled {
                    let hl = s.memory_decay.half_life_days;
                    let boost_factor = s.memory_decay.access_boost_factor;
                    let threshold = s.memory_decay.min_score_threshold;
                    
                    let meta_lock = index.metadata.read().unwrap();
                    results.retain_mut(|res| {
                        if let Some(meta) = meta_lock.get(&res.id) {
                            let mut decay_factor = 1.0;
                            if !meta.pinned {
                                let days = (now - meta.last_accessed) as f64 / 86400.0;
                                let days = days.max(0.0);
                                decay_factor = 0.5f64.powf(days / hl);
                            }
                            let access_boost = 1.0 + (1.0 + meta.access_count as f64).log2() * boost_factor;
                            res.score = (res.score as f64 * decay_factor * access_boost) as f32;
                            res.score >= threshold as f32
                        } else {
                            false
                        }
                    });
                    drop(meta_lock);
                    
                    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                    results.truncate(TOP_K);
                    
                    let mut meta_write = index.metadata.write().unwrap();
                    for res in &results {
                        if let Some(meta) = meta_write.get_mut(&res.id) {
                            meta.last_accessed = now;
                            meta.access_count += 1;
                        }
                    }
                    drop(meta_write);
                    
                    let path = s.embeddings_path.clone();
                    let index_clone = index.clone();
                    tokio::spawn(async move {
                        let _ = index_clone.save(&path);
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

const DEFAULT_HUBS: &[(&str, &str, &[&str])] = &[
    ("ai",          "# AI & Machine Learning\n\nHub for artificial intelligence, LLMs, neural networks, and ML research.\n",
     &["artificial intelligence", " ai ", "machine learning", "llm", "neural network",
       "deep learning", "gpt", "ollama", "diffusion", "transformer", "chatgpt",
       "openai", "claude", "gemini", "github copilot", "rag", "embedding"]),
    ("security",    "# Security\n\nHub for cybersecurity, exploits, CVEs, vulnerabilities, and infosec news.\n",
     &["security", "exploit", "cve-", "vulnerability", "malware", "hack",
       "breach", "phishing", "zero-day", "ransomware", "worm", "stealers",
       "injection", "xss", "trojan", "botnet", "spyware", "infosec", "ctf",
       "pentest", "reverse engineer"]),
    ("linux",       "# Linux\n\nHub for Linux, kernel development, distributions, and sysadmin topics.\n",
     &["linux", "kernel", "ubuntu", "debian", "arch", "fedora", "systemd",
       "bash", "posix", "unix", "distro", "wayland", "x11", "gtk"]),
    ("programming", "# Programming\n\nHub for software development, languages, frameworks, and engineering practices.\n",
     &["programming", "software", "developer", "code", "rust", "python",
       "javascript", "typescript", "golang", "haskell", "c++", "c#",
       "compiler", "algorithm", "api", "library", "framework", "open source",
       "github", "git", "refactor", "devops", "ci/cd"]),
    ("science",     "# Science\n\nHub for research, scientific discoveries, and academic topics.\n",
     &["science", "research", "study", "biology", "physics", "chemistry",
       "space", "nasa", "discovery", "experiment", "quantum", "climate"]),
    ("gaming",      "# Gaming\n\nHub for video games, game development, and gaming culture.\n",
     &["game", "steam", "gaming", "playstation", "xbox", "nintendo",
       "esports", "fps", "rpg", "mmo", "indie game"]),
    ("retro",       "# Retro Computing\n\nHub for classic computing, retro games, vintage software, and computing history.\n",
     &["retro", "classic", "vintage", "1990s", "1980s", "1970s", "dos",
       "commodore", "amiga", "8-bit", "16-bit", "old school", "nostalgia"]),
    ("web",         "# Web & Internet\n\nHub for web technologies, browsers, networking, and internet culture.\n",
     &["web", "browser", "http", "html", "css", "internet", "rss", "dns",
       "cdn", "frontend", "backend", "saas", "api"]),
    ("music",       "# Music\n\nHub for music, artists, songs, albums, piano, and musical culture.\n",
     &["music", "song", "piano", "album", "artist", "melody", "lyrics",
       "rap", "jazz", "classical", "singer", "band", "concert", "track",
       "playlist", "spotify", "genre", "hip hop", "rock", "metal", "pop",
       "composer", "orchestra", "vinyl"]),
    ("notes",       "# General Notes\n\nHub for miscellaneous topics that don't fit elsewhere.\n",
     &[]),
];

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

fn save_custom_hubs(hubs: &[HubDef]) -> Result<(), String> {
    let path = hubs_json_path();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let file = HubsFile { hubs: hubs.to_vec() };
    std::fs::write(&path, serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
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

fn forge_log(app: &tauri::AppHandle, msg: &str) {
    let _ = app.emit("armata-agent-status", serde_json::json!({
        "agent": "forge", "status": "online", "message": msg
    }));
}

pub async fn propose_new_hubs(app: &tauri::AppHandle, force: bool) -> Result<String, String> {
    let s = settings::load();
    let known = all_hub_names();

    let notes = vault::list_notes();
    let uncategorized: Vec<(String, String)> = notes.iter()
        .filter(|p| !is_archive(p))
        .filter_map(|p| {
            let c = vault::read_vault_note(&s.vault_path, p).ok()?;
            let topics_line = c.lines().find(|l| l.trim().starts_with("Topics:"))?;
            let has_only_notes = topics_line.contains("[[notes]]")
                && !known.iter().filter(|n| *n != "notes")
                    .any(|n| topics_line.contains(&format!("[[{}]]", n)));
            if has_only_notes { Some((p.clone(), c)) } else { None }
        })
        .collect();

    forge_log(app, &format!(
        "Topic scan: {}/{} notes uncategorized",
        uncategorized.len(),
        notes.iter().filter(|p| !is_archive(p)).count()
    ));

    if uncategorized.len() < 3 {
        forge_log(app, "Topic scan: no new hub needed");
        return Err("Not enough uncategorized notes (need at least 3).".into());
    }

    let vram_permit = {
        use tauri::Manager;
        let q = app.state::<crate::vram_queue::VramQueue>();
        if force {
            Some(q.acquire("forge-hub-propose").await.map_err(|e| e.to_string())?)
        } else {
            q.try_acquire("forge-hub-propose")
        }
    };
    if vram_permit.is_none() {
        forge_log(app, "Topic scan: deferred — chat is active");
        return Err("LLM is currently busy with another task.".into());
    }
    forge_log(app, &format!("Topic scan: analysing {} uncategorized notes…", uncategorized.len()));

    // Shuffle before sampling so the LLM sees a diverse cross-section each run,
    // not always the same alphabetically-first notes (e.g. rally-heavy batches).
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as usize;
    let mut shuffled = uncategorized.clone();
    for i in (1..shuffled.len()).rev() {
        let j = (seed.wrapping_mul(i + 1).wrapping_add(i * 6364136223846793005)) % (i + 1);
        shuffled.swap(i, j);
    }

    let sample: String = shuffled.iter().take(12)
        .map(|(path, content)| {
            let preview: String = content.lines()
                .filter(|l| !l.trim().is_empty() && !l.starts_with("Topics:"))
                .take(3).collect::<Vec<_>>().join(" ");
            format!("- {} : {}", path, preview.chars().take(120).collect::<String>())
        })
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "These vault notes are uncategorized (tagged [[notes]]). \
Identify ONE clear topic cluster among them and suggest a new hub.\n\
CRITICAL INSTRUCTION: Do NOT propose hubs related to automotive, racing, or rallye. Find a DIFFERENT topic cluster that exists in these notes.\n\
Notes:\n{}\n\n\
Respond with ONLY valid JSON: {{\"name\": \"topic_name\", \"description\": \"one sentence\", \"keywords\": [\"kw1\", \"kw2\", ...]}}",
        sample
    );

    let resp = ollama::chat_once(
        vec![serde_json::json!({"role": "user", "content": prompt})],
        &s.agents.light_model,
    ).await.map_err(|e| format!("LLM Error: {}", e))?;

    drop(vram_permit);

    let json_str = {
        let r = resp.trim();
        let s = r.find('{').unwrap_or(0);
        let e = r.rfind('}').map(|i| i + 1).unwrap_or(r.len());
        &r[s..e]
    };

    let proposal: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("LLM output was not valid JSON: {}", e))?;
    let (Some(name), Some(desc)) = (proposal["name"].as_str(), proposal["description"].as_str()) else { 
        return Err("JSON missing 'name' or 'description'".into()); 
    };
    let keywords: Vec<String> = proposal["keywords"].as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    if name.is_empty() || keywords.len() < 2 {
        forge_log(app, "Topic scan: LLM response unusable");
        return Err("LLM response unusable: empty name or too few keywords.".into());
    }
    if known.iter().any(|k| k == name) {
        forge_log(app, &format!("Topic scan: [[{}]] already exists", name));
        return Err(format!("Hub [[{}]] already exists.", name));
    }

    forge_log(app, &format!("Topic scan: proposing new hub [[{}]]", name));
    let _ = app.emit("hub-proposal", serde_json::json!({
        "name": name,
        "description": desc,
        "keywords": keywords,
        "uncategorized_count": uncategorized.len()
    }));
    Ok("Hub analysis complete — check for a proposal banner.".into())
}

#[tauri::command]
pub async fn confirm_hub_proposal(name: String, description: String, keywords: Vec<String>) -> Result<String, String> {
    let s = settings::load();

    let stem = name.trim().to_lowercase().replace(' ', "_");
    validate_stem(&stem)?;
    vault::validate_rel_path(&format!("{}.md", stem))
        .map_err(|e| format!("Invalid hub name: {}", e))?;

    let mut hubs = load_custom_hubs();
    if hubs.iter().any(|h| h.name == stem) {
        return Err(format!("Hub '{}' already exists", stem));
    }
    hubs.push(HubDef { name: stem.clone(), description: description.clone(), keywords });
    save_custom_hubs(&hubs)?;

    let filename = format!("{}.md", stem);
    let note_path = std::path::PathBuf::from(&s.vault_path).join(&filename);
    if !note_path.exists() {
        let content = format!("# {}\n\n{}\n", stem.replace('_', " "), description);
        vault::write_vault_note(&s.vault_path, &filename, &content).map_err(|e| e.to_string())?;
    }

    repair_all_orphans(&s.vault_path);
    let indexed = crate::embeddings::reindex().await.unwrap_or(0);
    Ok(format!("Hub '{}' created. {} chunks indexed.", stem, indexed))
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

#[tauri::command]
pub async fn trigger_hub_proposal(app: tauri::AppHandle) -> Result<String, String> {
    propose_new_hubs(&app, true).await
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
    fn test_consolidate_vault_inner_is_pub() {
        let _: fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>> =
            || Box::pin(consolidate_vault_inner());
    }
}


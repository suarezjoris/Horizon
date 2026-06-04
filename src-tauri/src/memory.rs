use crate::{embeddings, ollama, settings, vault};

/// Remove [[...]] wikilinks from LLM-generated text before persisting to vault.
fn strip_wikilinks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' && chars.peek() == Some(&'[') {
            chars.next();
            // Skip until matching ]]
            while let Some(ch) = chars.next() {
                if ch == ']' && chars.peek() == Some(&']') {
                    chars.next();
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

const CORE_FILES: &[&str] = &["memory/user.md", "memory/code.md", "memory/skills.md"];
const TOP_K: usize = 3; // Reduced from 6
const MAX_CONTEXT_CHARS: usize = 4000; // Safety limit

pub async fn get_context(query: &str) -> String {
    let s = settings::load();
    let index = embeddings::load_index(&s.embeddings_path);
    let mut blocks = Vec::new();
    let mut current_len = 0;

    // 1. Core memory files (priority)
    for rel in CORE_FILES {
        if let Ok(content) = vault::read_vault_note(&s.vault_path, rel) {
            let block = format!("### {}\n{}", rel, content);
            current_len += block.len();
            blocks.push(block);
        }
    }

    // 2. Semantic search
    if !index.is_empty() && current_len < MAX_CONTEXT_CHARS {
        if let Ok(vecs) = ollama::embed(vec![query.to_string()], "nomic-embed-text:latest").await {
            if let Some(qvec) = vecs.into_iter().next() {
                let results = embeddings::search(&index, &qvec, TOP_K);
                let mut seen: std::collections::HashSet<String> = 
                    CORE_FILES.iter().map(|s| s.to_string()).collect();
                
                for entry in results {
                    if current_len >= MAX_CONTEXT_CHARS { break; }
                    if !seen.contains(&entry.path) {
                        seen.insert(entry.path.clone());
                        let block = format!("### {}\n{}", entry.path, entry.chunk);
                        current_len += block.len();
                        blocks.push(block);
                        
                        // Follow wikilinks in this chunk (limited)
                        for link in vault::extract_wikilinks(&entry.chunk) {
                            if current_len >= MAX_CONTEXT_CHARS { break; }
                            let md = if link.ends_with(".md") { link.clone() } else { format!("{}.md", link) };
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
        "You are the Core Archetype Weaver. Analyze this 'Initial Brain Dump' from the user and distribute its essence into 3 core files:
        1. memory/user.md (Personal background, personality, values)
        2. memory/code.md (Tech stack, coding style, architectural preferences)
        3. memory/skills.md (Specific technical skills, masteries, or areas of expertise)

        Rules:
        - Be concise. Extract meaningful atomic facts.
        - Output format: RAW JSON object with keys 'user', 'code', 'skills' (each value is a string with bullet points).

        BRAIN DUMP:
        {}",
        text
    );

    let resp = ollama::chat_once(vec![serde_json::json!({"role": "user", "content": prompt})], &s.llm_model).await?;
    
    // Clean response
    let json_str = resp.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let data: serde_json::Value = serde_json::from_str(json_str).map_err(|e| format!("Parsing Error: {}", e))?;

    if let Some(user) = data["user"].as_str() {
        let _ = vault::write_vault_note(&s.vault_path, "memory/user.md", &format!("# User Profile\n{}", user));
    }
    if let Some(code) = data["code"].as_str() {
        let _ = vault::write_vault_note(&s.vault_path, "memory/code.md", &format!("# Code Preferences\n{}", code));
    }
    if let Some(skills) = data["skills"].as_str() {
        let _ = vault::write_vault_note(&s.vault_path, "memory/skills.md", &format!("# Skills & Knowledge\n{}", skills));
    }

    Ok("Calibration complete. Neurons mapped.".to_string())
}

pub async fn extract_and_save(user_msg: String, ai_msg: String) {
    let s = settings::load();
    let prompt = format!(
        "Act as a high-level cognitive archiver. Analyze this interaction to detect RECURRING patterns, STRONG preferences, or SPECIFIC working styles.

        Focus on:
        - Expressions like 'I love', 'I hate', 'I always', 'My style is'.
        - Technical choices (e.g. 'I prefer Rust over C++').
        - Personal identity markers.

        Rules:
        1. Ignore transient questions or temporary tasks.
        2. Only extract facts that define the user's permanent DNA.
        3. Output format: RAW JSON array of objects.
        
        Exchange:
        User: {}
        AI: {}
        
        Allowed files: memory/user.md, memory/code.md, memory/skills.md.
        Format Example: [{{ \"file\": \"memory/code.md\", \"fact\": \"User prefers functional programming patterns in Rust.\" }}]",
        user_msg, ai_msg
    );

    let Ok(resp) = ollama::chat_once(vec![serde_json::json!({"role": "user", "content": prompt})], &s.llm_model).await else {
        return;
    };

    // Clean response in case the model ignored "no markdown blocks"
    let json_str = resp.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();

    let Ok(facts) = serde_json::from_str::<Vec<serde_json::Value>>(json_str) else {
        return;
    };

    for fact in facts {
        let (Some(file), Some(text)) = (fact["file"].as_str(), fact["fact"].as_str()) else {
            continue;
        };
        // Only write to the 3 allowed core memory files, strip any wikilinks from fact text
        if CORE_FILES.contains(&file) {
            let safe_text = strip_wikilinks(text);
            let _ = vault::append_note(&s.vault_path, file, &format!("\n- {}", safe_text));
        }
    }
}

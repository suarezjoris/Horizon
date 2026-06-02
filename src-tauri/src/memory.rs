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

pub async fn extract_and_save(user_msg: String, ai_msg: String) {
    let s = settings::load();
    let prompt = format!(
        "Act as a professional archiver. Analyze the exchange and extract ONLY permanent, verifiable facts about the user.
        
        Rules:
        1. NO conversational filler (e.g. 'I'm happy to help').
        2. NO guesses or hallucinations.
        3. Only save preferences, skills, or personal background.
        4. Output format: RAW JSON array of objects.
        
        Exchange:
        User: {}
        AI: {}
        
        Allowed files: memory/user.md, memory/code.md, memory/skills.md.",
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

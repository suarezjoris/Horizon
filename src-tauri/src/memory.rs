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
const TOP_K: usize = 6;

pub async fn get_context(query: &str) -> String {
    let s = settings::load();
    let index = embeddings::load_index(&s.embeddings_path);
    let mut blocks = Vec::new();

    // Always inject core memory files
    for rel in CORE_FILES {
        if let Ok(content) = vault::read_vault_note(&s.vault_path, rel) {
            blocks.push(format!("### {}\n{}", rel, content));
        }
    }

    // Embed query and search index
    if !index.is_empty() {
        if let Ok(vecs) = ollama::embed(vec![query.to_string()], "nomic-embed-text:latest").await {
            if let Some(qvec) = vecs.into_iter().next() {
                let results = embeddings::search(&index, &qvec, TOP_K);
                let mut seen: std::collections::HashSet<String> = 
                    CORE_FILES.iter().map(|s| s.to_string()).collect();
                for entry in results {
                    if !seen.contains(&entry.path) {
                        seen.insert(entry.path.clone());
                        blocks.push(format!("### {}\n{}", entry.path, entry.chunk));
                        
                        // Follow wikilinks in this chunk
                        for link in vault::extract_wikilinks(&entry.chunk) {
                            let md = if link.ends_with(".md") {
                                link.clone()
                            } else {
                                format!("{}.md", link)
                            };
                            // Validate before following — wikilinks come from untrusted vault content
                            if !seen.contains(&md) && vault::validate_rel_path(&md).is_ok() {
                                seen.insert(md.clone());
                                if let Ok(c) = vault::read_vault_note(&s.vault_path, &md) {
                                    blocks.push(format!("### {}\n{}", md, c));
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
        "Extract new permanent facts about the user from this exchange.\n\
        Exchange:\nUser: {}\nAI: {}\n\n\
        Output only JSON: [{{ \"file\": \"memory/user.md\", \"fact\": \"...\" }}]\n\
        Allowed files: memory/user.md, memory/code.md, memory/skills.md",
        user_msg, ai_msg
    );

    let Ok(resp) = ollama::chat_once(vec![serde_json::json!({"role": "user", "content": prompt})], &s.llm_model).await else {
        return;
    };

    let Ok(facts) = serde_json::from_str::<Vec<serde_json::Value>>(&resp) else {
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

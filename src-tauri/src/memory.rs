use crate::{embeddings, ollama, settings, vault};

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

    for fact in facts {
        let (Some(file), Some(text)) = (fact["file"].as_str(), fact["fact"].as_str()) else {
            continue;
        };
        
        if vault::validate_rel_path(file).is_ok() {
            let _ = vault::append_note(&s.vault_path, file, &format!("\n- {}", text));
        }
    }
}

pub async fn consolidate_vault_inner() -> Result<String, String> {
    let s = settings::load();
    let notes = vault::list_notes();
    let mut vault_content = String::new();

    for rel_path in &notes {
        if let Ok(content) = vault::read_note(rel_path.clone()) {
            vault_content.push_str(&format!("\nFILE: {}\nCONTENT:\n{}\n---\n", rel_path, content));
        }
    }

    let prompt = format!(
        "You are the Master Librarian of a Second Brain. Analyze the entire current vault and REFACTOR it for better structure, less redundancy, and stronger connectivity.

        Current Vault:
        {}

        Rules:
        1. FUSE redundant notes (e.g., if facts about 'Rust' are in both skills.md and code.md, move them to a dedicated rust.md).
        2. SPLIT overgrown notes into thematic sub-notes.
        3. ENFORCE [[wikilinks]] between all related concepts.
        4. Output format: RAW JSON object where keys are filenames and values are the NEW markdown content.
        5. If a file should be DELETED, set its value to null.

        Goal: An emergent, Zettelkasten-style network where everything is connected.",
        vault_content
    );

    let resp = ollama::chat_once(vec![serde_json::json!({"role": "user", "content": prompt})], &s.llm_model).await?;
    let json_str = resp.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let data: serde_json::Value = serde_json::from_str(json_str).map_err(|e| format!("Consolidation Parsing Error: {}", e))?;

    if let Some(obj) = data.as_object() {
        for (file, content) in obj {
            if vault::validate_rel_path(file).is_ok() {
                if content.is_null() {
                    let path = std::path::PathBuf::from(&s.vault_path).join(file);
                    let _ = std::fs::remove_file(path);
                } else if let Some(c) = content.as_str() {
                    let _ = vault::write_vault_note(&s.vault_path, file, c);
                }
            }
        }
    }

    Ok("Consolidation complete. The brain has evolved.".to_string())
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


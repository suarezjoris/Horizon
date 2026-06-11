use std::path::PathBuf;
use std::collections::HashSet;
use tauri::{AppHandle, Emitter};
use crate::settings;

fn normalize_slug(s: &str) -> String {
    s.to_lowercase()
        .replace(' ', "_")
        .replace('-', "_")
}

fn to_wiki_title(slug: &str) -> String {
    slug.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join("_")
}


async fn fetch_wiki_summary(client: &reqwest::Client, slug: &str) -> Option<(String, String)> {
    let title = to_wiki_title(slug);
    let url = format!(
        "https://en.wikipedia.org/api/rest_v1/page/summary/{}",
        urlencoding::encode(&title)
    );

    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() { return None; }

    let json: serde_json::Value = resp.json().await.ok()?;

    let extract = json.get("extract")
        .and_then(|e| e.as_str())
        .filter(|e| e.split_whitespace().count() > 20)
        .map(|e| e.to_string())?;

    let title_clean = json.get("title")
        .and_then(|t| t.as_str())
        .unwrap_or(&title)
        .to_string();

    Some((title_clean, extract))
}

#[tauri::command]
pub async fn ingest_wikipedia(app: AppHandle) -> Result<String, String> {
    let s = settings::load();

    let all_notes = crate::vault::list_vault_notes(&s.vault_path);
    let seeds: Vec<String> = all_notes.iter()
        .filter(|p| !p.starts_with("knowledge/wiki-") && p.ends_with(".md"))
        .map(|p| {
            p.trim_end_matches(".md").rsplit('/').next().unwrap_or(p).to_string()
        })
        .collect();

    let stem_set: HashSet<String> = seeds.iter().map(|s| normalize_slug(s)).collect();

    let client = reqwest::Client::builder()
        .user_agent("HorizonApp/2.1 (personal AI desktop; https://github.com/personal)")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let knowledge_dir = PathBuf::from(&s.vault_path).join("knowledge");
    std::fs::create_dir_all(&knowledge_dir).map_err(|e| e.to_string())?;

    let mut saved = 0;
    let total = seeds.len();

    for (i, stem) in seeds.iter().enumerate() {
        let slug = normalize_slug(stem);
        let dest_rel = format!("knowledge/wiki-{}.md", slug);
        let dest = PathBuf::from(&s.vault_path).join(&dest_rel);
        if dest.exists() { continue; }

        let _ = app.emit("wiki-ingest-status", serde_json::json!({
            "status": "fetching",
            "message": format!("[{}/{}] Looking up {}…", i + 1, total, slug)
        }));

        let Some((title, extract)) = fetch_wiki_summary(&client, &slug).await else {
            continue;
        };

        // Cross-links: other vault note names mentioned in the extract
        let extract_low = extract.to_lowercase();
        let mut cross_links: Vec<String> = stem_set.iter()
            .filter(|s| *s != &slug && {
                // Word-boundary check: surrounded by non-alphanumeric
                let s_str = s.replace('_', " ");
                extract_low.contains(s_str.as_str())
            })
            .map(|s| format!("[[{}]]", s))
            .collect();

        // Hub topic links
        let topics = crate::memory::detect_topics(&title, &extract);
        for t in topics {
            let link = format!("[[{}]]", t);
            if !cross_links.contains(&link) { cross_links.push(link); }
        }

        let topic_line = if cross_links.is_empty() {
            String::new()
        } else {
            cross_links.sort();
            format!("\n\nTopics: {}", cross_links.join(" "))
        };

        let note = format!("# {}\n\n*Source: Wikipedia*\n\n{}{}\n", title, extract, topic_line);

        if crate::vault::write_vault_note(&s.vault_path, &dest_rel, &note).is_ok() {
            saved += 1;
        }

        // Polite delay between Wikipedia API requests
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    }

    let indexed = crate::embeddings::reindex().await.unwrap_or(0);
    Ok(format!(
        "Wikipedia ingestion complete: {}/{} articles saved, {} chunks indexed.",
        saved, total, indexed
    ))
}

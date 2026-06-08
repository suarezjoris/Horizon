use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use crate::settings;

fn emit_status(app: &AppHandle, status: &str, msg: &str) {
    let _ = app.emit("armata-agent-status", serde_json::json!({
        "agent": "vanguard",
        "status": status,
        "message": msg
    }));
}

/// Parse RSS XML lines, returning (title, link) pairs.
pub fn parse_rss_items(xml: &str) -> Vec<(String, String)> {
    let mut items = Vec::new();
    let mut in_item = false;
    let mut current_title = String::new();
    let mut current_link = String::new();

    for line in xml.lines() {
        let t = line.trim();
        if t == "<item>" || t.starts_with("<item ") {
            in_item = true;
            current_title.clear();
            current_link.clear();
        } else if t == "</item>" {
            if !current_title.is_empty() && !current_link.is_empty() {
                items.push((current_title.clone(), current_link.clone()));
            }
            in_item = false;
        } else if in_item {
            if t.starts_with("<title>") && t.ends_with("</title>") {
                current_title = strip_tag(t, "title");
            } else if t.starts_with("<link>") && t.ends_with("</link>") {
                current_link = strip_tag(t, "link");
            }
        }
    }
    items
}

fn strip_tag(s: &str, tag: &str) -> String {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    s.trim_start_matches(open.as_str())
     .trim_end_matches(close.as_str())
     .to_string()
}

async fn summarize_item(title: &str, link: &str, model: &str) -> String {
    let prompt = format!(
        "In exactly 2 sentences, summarize this tech news item for a developer's knowledge base.\nTitle: {}\nURL: {}",
        title, link
    );
    let msgs = vec![serde_json::json!({"role": "user", "content": prompt})];
    crate::ollama::chat_once(msgs, model).await
        .unwrap_or_else(|_| format!("(summary unavailable) {}", title))
}

async fn scan_feeds(app: &AppHandle) {
    let s = settings::load();
    let feeds = s.agents.vanguard_feeds.clone();
    let model = s.agents.light_model.clone();
    let vault_path = s.vault_path.clone();

    let intel_path = std::path::PathBuf::from(&vault_path)
        .join("memory")
        .join("vanguard-intel.md");

    let _ = std::fs::create_dir_all(intel_path.parent().unwrap());

    let existing = std::fs::read_to_string(&intel_path).unwrap_or_default();
    let mut new_entries = Vec::new();

    for feed_url in &feeds {
        emit_status(app, "online", &format!("Scanning: {}", feed_url));

        let xml = match reqwest::get(feed_url).await {
            Ok(r) => match r.text().await {
                Ok(t) => t,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        let items = parse_rss_items(&xml);
        let new_items: Vec<_> = items.iter()
            .filter(|(title, _)| !existing.contains(title.as_str()))
            .take(3)
            .collect();

        for (title, link) in &new_items {
            emit_status(app, "online", &format!("Summarizing: {}", title));
            let summary = summarize_item(title, link, &model).await;
            let date = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
            new_entries.push(format!(
                "## {}\n_{}_  \n[{}]({})\n\n{}\n",
                title, date, link, link, summary
            ));
        }
    }

    if !new_entries.is_empty() {
        let mut content = std::fs::read_to_string(&intel_path).unwrap_or_default();
        for entry in &new_entries {
            content.push_str(entry);
            content.push('\n');
        }
        let _ = std::fs::write(&intel_path, &content);
        emit_status(app, "online", &format!("Injected {} new items into vault", new_entries.len()));
    } else {
        emit_status(app, "online", "No new intel");
    }
}

pub async fn run_vanguard(app: AppHandle, running: Arc<AtomicBool>) {
    let interval_minutes = {
        let s = settings::load();
        s.agents.vanguard_interval_minutes
    };

    emit_status(&app, "online", "Vanguard active — RSS monitoring started");

    scan_feeds(&app).await;

    let mut interval = tokio::time::interval(Duration::from_secs(interval_minutes * 60));
    interval.tick().await; // skip immediate tick since we just scanned

    loop {
        interval.tick().await;
        if !running.load(Ordering::Relaxed) { break; }
        scan_feeds(&app).await;
    }

    emit_status(&app, "offline", "Vanguard stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rss_items_empty_feed() {
        let xml = r#"<?xml version="1.0"?><rss><channel><title>T</title></channel></rss>"#;
        let items = parse_rss_items(xml);
        assert!(items.is_empty());
    }

    #[test]
    fn test_parse_rss_items_extracts_title_and_link() {
        let xml = "<?xml version=\"1.0\"?>\n<rss><channel>\n  <item>\n    <title>Hello World</title>\n    <link>https://example.com/1</link>\n  </item>\n  <item>\n    <title>Rust News</title>\n    <link>https://example.com/2</link>\n  </item>\n</channel></rss>";
        let items = parse_rss_items(xml);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, "Hello World");
        assert_eq!(items[1].1, "https://example.com/2");
    }
}

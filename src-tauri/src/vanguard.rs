use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use crate::settings;

const MAX_ITEMS_PER_SOURCE: usize = 5;

fn emit_status(app: &AppHandle, status: &str, msg: &str) {
    let _ = app.emit("armata-agent-status", serde_json::json!({
        "agent": "vanguard",
        "status": status,
        "message": msg
    }));
}

// ── RSS/Atom parser ───────────────────────────────────────────────────────────

/// Parse RSS/Atom XML into (title, link, description) triples.
pub fn parse_rss_items(xml: &str) -> Vec<(String, String, String)> {
    let mut items = Vec::new();
    let mut search = xml;

    loop {
        // Find next <item> or <entry>
        let item_pos = search.find("<item").or_else(|| search.find("<entry>")).or_else(|| search.find("<entry "));
        let item_pos = match item_pos { Some(p) => p, None => break };

        let tag = if search[item_pos..].starts_with("<item") { "item" } else { "entry" };
        let close_tag = format!("</{}>", tag);

        // Skip to end of opening tag
        let content_start = match search[item_pos..].find('>') {
            Some(p) => item_pos + p + 1,
            None => break,
        };

        // Find closing tag
        let block_end = match search[content_start..].find(&close_tag) {
            Some(p) => content_start + p,
            None => break,
        };

        let block = &search[content_start..block_end];

        let title = extract_field(block, "title").unwrap_or_default();
        let link  = extract_link(block).unwrap_or_default();
        let desc  = extract_field(block, "description")
            .or_else(|| extract_field(block, "summary"))
            .unwrap_or_default();

        if !title.is_empty() && link.starts_with("http") {
            items.push((title, link, desc));
        }

        search = &search[block_end + close_tag.len()..];
    }

    items
}

/// Extract the text content of an XML element, handling CDATA.
fn extract_field(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{}", tag);
    let pos = block.find(&open)?;
    let tag_end = block[pos..].find('>')?;
    let content_start = pos + tag_end + 1;

    let close = format!("</{}>", tag);
    let end = block[content_start..].find(&close)?;
    let raw = block[content_start..content_start + end].trim();

    let text = if raw.starts_with("<![CDATA[") {
        raw.trim_start_matches("<![CDATA[").trim_end_matches("]]>")
    } else {
        raw
    };

    let decoded = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'");

    if decoded.trim().is_empty() { None } else { Some(decoded.trim().to_string()) }
}

/// Extract the best link from an RSS item or Atom entry block.
fn extract_link(block: &str) -> Option<String> {
    // RSS: <link>url</link>
    if let Some(url) = extract_field(block, "link") {
        let url = url.trim().to_string();
        if url.starts_with("http") { return Some(url); }
    }

    // Atom: <link href="url" rel="alternate"/> or <link href="url"/>
    let mut s = block;
    while let Some(pos) = s.find("<link") {
        let after = &s[pos + 5..];
        let tag_end = after.find('>').unwrap_or(after.len());
        let tag_content = &after[..tag_end];

        // Skip self/hub links
        if !tag_content.contains("rel=\"self\"") && !tag_content.contains("rel=\"hub\"") {
            if let Some(href_pos) = tag_content.find("href=\"") {
                let url_start = href_pos + 6;
                if let Some(url_end) = tag_content[url_start..].find('"') {
                    let url = &tag_content[url_start..url_start + url_end];
                    if url.starts_with("http") {
                        return Some(url.to_string());
                    }
                }
            }
        }
        s = &s[pos + 5..];
    }

    None
}

/// Return true if content looks like an RSS/Atom feed.
fn is_feed(content: &str) -> bool {
    let head: String = content.chars().take(512).collect();
    head.contains("<rss") || head.contains("<feed") || head.contains("<atom:")
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn url_to_slug(url: &str) -> String {
    url.chars()
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

async fn fetch(url: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:120.0) Gecko/20100101 Firefox/120.0")
        .build()
        .ok()?;
    client.get(url).send().await.ok()?.text().await.ok()
}

/// Strip HTML tags, scripts, and style blocks; collapse whitespace.
fn extract_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    let mut skip_block = false;
    let mut tag_buf = String::new();

    for ch in html.chars() {
        match ch {
            '<' => { in_tag = true; tag_buf.clear(); }
            '>' => {
                in_tag = false;
                let tl = tag_buf.to_ascii_lowercase();
                if tl.starts_with("script") || tl.starts_with("style") {
                    skip_block = true;
                } else if tl.starts_with("/script") || tl.starts_with("/style") {
                    skip_block = false;
                }
            }
            _ if in_tag => tag_buf.push(ch),
            _ if !skip_block => text.push(ch),
            _ => {}
        }
    }

    text.split_whitespace().collect::<Vec<_>>().join(" ")
        .chars().take(3000).collect()
}

async fn summarize(title: &str, body: &str, model: &str) -> String {
    let prompt = format!(
        "Write a 3-sentence summary of this article for a developer knowledge base. Be specific and factual.\n\nTitle: {}\n\nContent: {}",
        title,
        body.chars().take(2000).collect::<String>()
    );
    let msgs = vec![serde_json::json!({"role": "user", "content": prompt})];
    crate::ollama::chat_once(msgs, model).await
        .unwrap_or_else(|_| String::new())
}

/// Append an article entry to the daily digest file. Returns true if newly added.
fn append_to_digest(vault_dir: &PathBuf, date: &str, title: &str, url: &str, summary: &str) -> bool {
    let digest_path = vault_dir.join(format!("digest-{}.md", date));

    // Dedup: check if this URL is already in the digest
    if digest_path.exists() {
        if let Ok(existing) = std::fs::read_to_string(&digest_path) {
            if existing.contains(url) { return false; }
        }
    }

    // Detect topics deterministically — always guarantees at least one wikilink
    let topics = crate::memory::detect_topics(title, summary);
    let topic_links = topics.iter()
        .map(|t| format!("[[{}]]", t))
        .collect::<Vec<_>>()
        .join(" ");

    // Use summary if available; fall back to a minimal descriptor so entries are never empty
    let body = if summary.trim().is_empty() {
        format!("*No summary available.* Source: {}", url)
    } else {
        summary.trim().to_string()
    };

    let entry = if digest_path.exists() {
        format!("\n---\n\n## {}\n**Source:** {} | {}\n\n{}\n", title, url, topic_links, body)
    } else {
        format!("# Vanguard Digest {}\n\n## {}\n**Source:** {} | {}\n\n{}\n", date, title, url, topic_links, body)
    };

    use std::io::Write;
    let mut file = match std::fs::OpenOptions::new().create(true).append(true).open(&digest_path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    file.write_all(entry.as_bytes()).is_ok()
}

// ── Main scan loop ─────────────────────────────────────────────────────────────

async fn scan_sources(app: &AppHandle) {
    let s = settings::load();
    let model = s.agents.light_model.clone();
    let vault_dir = PathBuf::from(&s.vault_path).join("vanguard");
    let _ = std::fs::create_dir_all(&vault_dir);
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut total_new = 0usize;

    for source_url in &s.agents.vanguard_feeds {
        emit_status(app, "online", &format!("Scanning: {}", source_url));

        let content = match fetch(source_url).await {
            Some(c) => c,
            None => {
                emit_status(app, "warn", &format!("Unreachable: {}", source_url));
                continue;
            }
        };

        if !is_feed(&content) {
            emit_status(app, "warn", &format!("Not an RSS/Atom feed: {}", source_url));
            continue;
        }

        let items = parse_rss_items(&content);

        if items.is_empty() {
            emit_status(app, "warn", &format!("Feed parsed but no items found: {}", source_url));
            continue;
        }

        let mut saved = 0;
        // Load today's digest once for dedup check
        let digest_path = vault_dir.join(format!("digest-{}.md", date));
        let existing_digest = if digest_path.exists() {
            std::fs::read_to_string(&digest_path).unwrap_or_default()
        } else {
            String::new()
        };

        for (title, article_url, rss_desc) in items.iter() {
            if saved >= MAX_ITEMS_PER_SOURCE { break; }
            if existing_digest.contains(article_url.as_str()) { continue; }

            emit_status(app, "online", &format!("Reading: {}", title));

            // Gather as much content as possible: full article > RSS desc > title alone
            let fetched_text = fetch(article_url).await.map(|html| extract_text(&html));
            let best_content = match &fetched_text {
                Some(t) if t.split_whitespace().count() >= 30 => t.as_str(),
                _ if rss_desc.split_whitespace().count() >= 5 => rss_desc.as_str(),
                _ => "",
            };

            // Summarize if we have enough content; otherwise build a minimal entry from title
            let summary = if best_content.split_whitespace().count() >= 20 {
                let s = summarize(title, best_content, &model).await;
                if s.trim().is_empty() { best_content.chars().take(500).collect() } else { s }
            } else {
                // Minimal entry: no content fetched, but title + wikilinks still get saved
                String::new()
            };

            if append_to_digest(&vault_dir, &date, title, article_url, &summary) {
                emit_status(app, "online", &format!("Saved: {}", title));
                total_new += 1;
                saved += 1;
            }
        }
    }

    if total_new > 0 {
        emit_status(app, "online", &format!("{} new articles added to vault", total_new));
    } else {
        emit_status(app, "online", "No new intel");
    }
}

pub async fn run_vanguard(app: AppHandle, running: Arc<AtomicBool>) {
    let interval_minutes = settings::load().agents.vanguard_interval_minutes;
    emit_status(&app, "online", &format!("Scanning every {}min", interval_minutes));

    scan_sources(&app).await;

    let mut interval = tokio::time::interval(Duration::from_secs(interval_minutes * 60));
    interval.tick().await;

    loop {
        interval.tick().await;
        if !running.load(Ordering::Relaxed) { break; }
        scan_sources(&app).await;
    }

    emit_status(&app, "offline", "Vanguard stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_rss() {
        let xml = r#"<?xml version="1.0"?><rss><channel>
  <item>
    <title>Hello World</title>
    <link>https://example.com/1</link>
    <description>Short desc</description>
  </item>
</channel></rss>"#;
        let items = parse_rss_items(xml);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, "Hello World");
        assert_eq!(items[0].1, "https://example.com/1");
    }

    #[test]
    fn test_parse_cdata_rss() {
        let xml = r#"<rss><channel>
  <item>
    <title><![CDATA[CDATA Title & More]]></title>
    <link>https://example.com/article</link>
    <description><![CDATA[Short summary here]]></description>
  </item>
</channel></rss>"#;
        let items = parse_rss_items(xml);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, "CDATA Title & More");
        assert_eq!(items[0].2, "Short summary here");
    }

    #[test]
    fn test_parse_atom_feed() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <title>Atom Entry</title>
    <link href="https://example.com/atom/1" rel="alternate"/>
    <summary>Summary text</summary>
  </entry>
</feed>"#;
        let items = parse_rss_items(xml);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, "Atom Entry");
        assert_eq!(items[0].1, "https://example.com/atom/1");
    }

    #[test]
    fn test_is_feed() {
        assert!(is_feed("<rss version=\"2.0\">"));
        assert!(is_feed("<?xml?><feed xmlns="));
        assert!(!is_feed("<html><body>"));
    }

    #[test]
    fn test_extract_text_strips_scripts() {
        let html = "<html><body><script>alert(1)</script><p>Hello world</p></body></html>";
        let text = extract_text(html);
        assert!(text.contains("Hello world"));
        assert!(!text.contains("alert"));
    }
}

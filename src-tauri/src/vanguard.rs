use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use crate::settings;

const MAX_ITEMS_PER_FEED: usize = 5;
const MAX_ITEMS_PER_QUERY: usize = 3;
const MAX_DAILY_ARTICLES: usize = 30;

fn hub_stems() -> Vec<String> {
    crate::memory::all_hub_names()
}

fn emit_status(app: &AppHandle, status: &str, msg: &str) {
    let _ = app.emit("armata-agent-status", serde_json::json!({
        "agent": "vanguard",
        "status": status,
        "message": msg
    }));
}

pub fn vault_keywords(vault_path: &str) -> Vec<String> {
    let notes = crate::vault::list_vault_notes(vault_path);
    let mut keywords = Vec::new();

    for path in &notes {
        if path.starts_with("vanguard/")
            || path.starts_with("memory/")
            || path.starts_with("knowledge/")
            || path.starts_with("images/")   // ComfyUI image metadata files
            || path.starts_with("film/")
            || path.starts_with("characters/")
        {
            continue;
        }

        let stem = path
            .trim_end_matches(".md")
            .rsplit('/')
            .next()
            .unwrap_or(path);

        if stem.starts_with("digest-") { continue; }
        if stem.starts_with(|c: char| c.is_ascii_digit()) { continue; }
        if hub_stems().iter().any(|h| h == stem) { continue; }
        if stem.len() <= 3 { continue; }

        keywords.push(stem.replace('_', " "));
    }

    keywords.sort_by(|a, b| b.len().cmp(&a.len()));
    keywords.dedup();
    keywords
}

fn build_search_queries(keywords: &[String]) -> Vec<String> {
    let mut queries = Vec::new();

    for kw in keywords {
        let topics = crate::memory::detect_topics(kw, "");
        let topic = topics.into_iter().next().unwrap_or_else(|| "misc".to_string());

        let is_multiword = kw.contains(' ');

        // Skip single-word terms with no recognized topic — they generate noise queries
        // (e.g. "portfolio" → finance spam, "internship" → job boards, "moonscoop" → anime)
        if !is_multiword && topic == "notes" { continue; }

        let context: &str = if is_multiword {
            ""
        } else {
            match topic.as_str() {
                "programming" => " programming language",
                "linux"       => " Linux OS",
                "security"    => " cybersecurity",
                "ai"          => " artificial intelligence",
                "gaming"      => " video game",
                "retro"       => " retro computing",
                "science"     => " science",
                "music"       => " music",
                _             => "",
            }
        };

        queries.push(format!("\"{}\"{}  news", kw, context));
    }

    queries.push("technology programming news".to_string());
    queries.push("artificial intelligence news".to_string());
    queries.truncate(10);
    queries
}

fn google_news_rss(query: &str) -> String {
    format!(
        "https://news.google.com/rss/search?q={}&hl=en-US&gl=US&ceid=US:en",
        urlencoding::encode(query)
    )
}

pub fn relevance_score(title: &str, desc: &str, keywords: &[String]) -> usize {
    let title_low = title.to_lowercase();
    let desc_low = desc.to_lowercase();

    keywords.iter().map(|kw| {
        let k = kw.to_lowercase();
        let in_title = if title_low.contains(&k) { 3 } else { 0 };
        let in_desc  = if desc_low.contains(&k)  { 1 } else { 0 };
        in_title + in_desc
    }).sum()
}

pub fn parse_rss_items(xml: &str) -> Vec<(String, String, String)> {
    let mut items = Vec::new();
    let mut search = xml;

    loop {
        let item_pos = search.find("<item")
            .or_else(|| search.find("<entry>"))
            .or_else(|| search.find("<entry "));
        let item_pos = match item_pos { Some(p) => p, None => break };

        let tag = if search[item_pos..].starts_with("<item") { "item" } else { "entry" };
        let close_tag = format!("</{}>", tag);

        let content_start = match search[item_pos..].find('>') {
            Some(p) => item_pos + p + 1,
            None => break,
        };
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

fn extract_link(block: &str) -> Option<String> {
    if let Some(url) = extract_field(block, "link") {
        let url = url.trim().to_string();
        if url.starts_with("http") { return Some(url); }
    }

    let mut s = block;
    while let Some(pos) = s.find("<link") {
        let after = &s[pos + 5..];
        let tag_end = after.find('>').unwrap_or(after.len());
        let tag_content = &after[..tag_end];

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

fn is_feed(content: &str) -> bool {
    let head: String = content.chars().take(512).collect();
    head.contains("<rss") || head.contains("<feed") || head.contains("<atom:")
}

async fn fetch(url: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:120.0) Gecko/20100101 Firefox/120.0")
        .build()
        .ok()?;
    client.get(url).send().await.ok()?.text().await.ok()
}

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
        "Write a 3-sentence summary of this article for a personal knowledge base. Be specific and factual.\n\nTitle: {}\n\nContent: {}",
        title,
        body.chars().take(2000).collect::<String>()
    );
    let msgs = vec![serde_json::json!({"role": "user", "content": prompt})];
    crate::ollama::chat_once(msgs, model).await
        .unwrap_or_else(|_| String::new())
}

fn append_to_digest(vault_dir: &PathBuf, date: &str, title: &str, url: &str, summary: &str) -> bool {
    let digest_path = vault_dir.join(format!("digest-{}.md", date));

    if digest_path.exists() {
        if let Ok(existing) = std::fs::read_to_string(&digest_path) {
            if existing.contains(url) { return false; }
        }
    }

    let topics = crate::memory::detect_topics(title, summary);
    let topic_links = topics.iter()
        .map(|t| format!("[[{}]]", t))
        .collect::<Vec<_>>()
        .join(" ");

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

async fn scan_feed(
    app: &AppHandle,
    vault_dir: &PathBuf,
    date: &str,
    source_url: &str,
    model: &str,
    keywords: &[String],
    max_items: usize,
    require_relevance: bool,
) -> usize {
    let content = match fetch(source_url).await {
        Some(c) => c,
        None => {
            emit_status(app, "warn", &format!("Unreachable: {}", source_url));
            return 0;
        }
    };

    if !is_feed(&content) { return 0; }

    let items = parse_rss_items(&content);
    if items.is_empty() { return 0; }

    let digest_path = vault_dir.join(format!("digest-{}.md", date));
    let existing = if digest_path.exists() {
        std::fs::read_to_string(&digest_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut saved = 0;
    for (title, article_url, rss_desc) in &items {
        if saved >= max_items { break; }
        if existing.contains(article_url.as_str()) { continue; }

        if require_relevance {
            let score = relevance_score(title, rss_desc, keywords);
            if score == 0 { continue; }
        }

        emit_status(app, "online", &format!("Reading: {}", title));

        let fetched_text = fetch(article_url).await.map(|html| extract_text(&html));
        let rss_text = extract_text(rss_desc);
        let best_content: String = match &fetched_text {
            Some(t) if t.split_whitespace().count() >= 30 => t.clone(),
            _ if rss_text.split_whitespace().count() >= 5 => rss_text,
            _ => String::new(),
        };

        let vram_permit = {
            use tauri::Manager;
            let q = app.state::<crate::vram_queue::VramQueue>();
            q.try_acquire("vanguard-summarize")
        };

        let summary = if best_content.split_whitespace().count() >= 10 {
            if vram_permit.is_some() {
                emit_status(app, "online", &format!("Summarizing: {}", title));
                let s = summarize(title, &best_content, model).await;
                if s.trim().is_empty() {
                    emit_status(app, "warn", &format!("LLM empty for: {} — using excerpt", title));
                    best_content.chars().take(600).collect()
                } else {
                    s
                }
            } else {
                // GPU busy (chat or Forge) — save excerpt, distillation will process it later
                emit_status(app, "warn", &format!("GPU busy — saving excerpt: {}", title));
                best_content.chars().take(600).collect()
            }
        } else {
            emit_status(app, "warn", &format!("No content for: {}", title));
            String::new()
        };
        drop(vram_permit);

        if append_to_digest(vault_dir, date, title, article_url, &summary) {
            emit_status(app, "online", &format!("Saved: {}", title));
            saved += 1;
        }
    }

    saved
}

async fn scan_sources(app: &AppHandle) {
    let s = settings::load();
    let model = s.agents.light_model.clone();
    let vault_dir = PathBuf::from(&s.vault_path).join("vanguard");
    let _ = std::fs::create_dir_all(&vault_dir);
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut total_new = 0usize;

    let keywords = vault_keywords(&s.vault_path);
    let queries = build_search_queries(&keywords);

    emit_status(app, "online", &format!(
        "Vault profile: {} keywords, {} search queries",
        keywords.len(), queries.len()
    ));

    for query in &queries {
        if total_new >= MAX_DAILY_ARTICLES { break; }
        let url = google_news_rss(query);
        emit_status(app, "online", &format!("Querying: {}", query));
        let n = scan_feed(
            app, &vault_dir, &date, &url, &model,
            &keywords, MAX_ITEMS_PER_QUERY, false,
        ).await;
        total_new += n;
    }

    for feed_url in &s.agents.vanguard_feeds {
        if total_new >= MAX_DAILY_ARTICLES { break; }
        emit_status(app, "online", &format!("Feed: {}", feed_url));
        let n = scan_feed(
            app, &vault_dir, &date, feed_url, &model,
            &keywords, MAX_ITEMS_PER_FEED, true,
        ).await;
        total_new += n;
    }

    if total_new > 0 {
        emit_status(app, "online", &format!("{} new articles added to digest", total_new));
    } else {
        emit_status(app, "online", "No new intel");
    }
}

pub async fn run_vanguard(app: AppHandle, running: Arc<AtomicBool>) {
    let interval_minutes = settings::load().agents.vanguard_interval_minutes;
    emit_status(&app, "online", &format!("Scanning every {}min", interval_minutes));

    scan_sources(&app).await;

    loop {
        let mut remaining = interval_minutes * 60;
        while remaining > 0 {
            if !running.load(Ordering::Relaxed) {
                emit_status(&app, "offline", "Vanguard stopped");
                return;
            }
            tokio::time::sleep(Duration::from_secs(remaining.min(5))).await;
            remaining = remaining.saturating_sub(5);
        }

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
        assert_eq!(items[0].1, "https://example.com/atom/1");
    }

    #[test]
    fn test_is_feed() {
        assert!(is_feed("<rss version=\"2.0\">"));
        assert!(is_feed("<?xml?><feed xmlns="));
        assert!(!is_feed("<html><body>"));
    }

    #[test]
    fn test_relevance_score() {
        let keywords = vec!["Colin McRae".to_string(), "rally".to_string(), "Rust".to_string()];
        // Strong match: title hit = 3
        assert!(relevance_score("Colin McRae wins rally", "", &keywords) >= 3);
        // No match
        assert_eq!(relevance_score("Stock market crashes", "banking news", &keywords), 0);
        // Desc-only match = 1
        assert_eq!(relevance_score("Racing news", "Rust-powered car telemetry", &keywords), 1);
    }

    #[test]
    fn test_build_search_queries_not_empty() {
        let keywords = vec![
            "colin mcrae".to_string(),
            "rally".to_string(),
            "rust programming".to_string(),
        ];
        let queries = build_search_queries(&keywords);
        assert!(!queries.is_empty());
    }

    #[test]
    fn test_extract_text_strips_scripts() {
        let html = "<html><body><script>alert(1)</script><p>Hello world</p></body></html>";
        let text = extract_text(html);
        assert!(text.contains("Hello world"));
        assert!(!text.contains("alert"));
    }
}

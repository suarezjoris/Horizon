use scraper::{Html, Selector};

const BAD_DOMAINS: &[&str] = &[
    "youtube.com", "reddit.com", "twitter.com", "x.com",
    "instagram.com", "facebook.com", "tiktok.com", "socialblade.com",
];

fn is_blocked_domain(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    BAD_DOMAINS.iter().any(|d| url_lower.contains(d))
}

pub async fn duckduckgo_search(query: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post("https://html.duckduckgo.com/html/")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("q={}&kl=wt-wt&ia=web", urlencoding::encode(query)))
        .send()
        .await
        .map_err(|e| format!("Search request failed: {}", e))?;

    let html = resp.text().await.map_err(|e| e.to_string())?;

    // Parse and extract all data in a sync block, then drop the non-Send Html before any .await
    let results: Vec<(String, String, String)> = {
        let document = Html::parse_document(&html);
        let title_sel = Selector::parse("a.result__a").unwrap();
        let snippet_sel = Selector::parse("a.result__snippet").unwrap();

        let titles: Vec<_> = document.select(&title_sel).take(6).collect();
        let snippets: Vec<_> = document.select(&snippet_sel).take(6).collect();

        titles.iter().take(4).enumerate().map(|(i, title_el)| {
            let title = title_el.text().collect::<String>().trim().to_string();
            let href = title_el.value().attr("href").unwrap_or("").to_string();
            let real_url = if href.contains("uddg=") {
                let encoded = href.split("uddg=").nth(1).unwrap_or("").split('&').next().unwrap_or("");
                urlencoding::decode(encoded).map(|s| s.into_owned()).unwrap_or(href)
            } else {
                href
            };
            let snippet = snippets.get(i)
                .map(|s| s.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
            (title, real_url, snippet)
        }).collect()
        // document dropped here
    };

    if results.is_empty() {
        return Ok("__HORIZON_EMPTY_RESULT__".to_string());
    }

    let mut output = vec!["Results from the web:\n".to_string()];

    for (i, (title, real_url, snippet)) in results.into_iter().enumerate() {
        let mut body = format!("Snippet: {}", snippet);

        if !real_url.is_empty() && !is_blocked_domain(&real_url) {
            if let Ok(content) = fetch_page(&client, &real_url).await {
                body.push_str(&format!("\nPage Content: {}", content));
            }
        }

        let source = if real_url.is_empty() { title } else { format!("[{}]({})", title, real_url) };
        output.push(format!("{}. {}\n{}", i + 1, source, body));
    }

    let result = output.join("\n\n---\n\n") + "\n";
    if result.trim().is_empty() {
        Ok("__HORIZON_EMPTY_RESULT__".to_string())
    } else {
        Ok(result)
    }
}

async fn fetch_page(client: &reqwest::Client, url: &str) -> Result<String, String> {
    const MAX_PAGE_CHARS: usize = 3000;
    const TIMEOUT_SECS: u64 = 6;

    let resp = tokio::time::timeout(
        std::time::Duration::from_secs(TIMEOUT_SECS),
        client.get(url).send(),
    )
    .await
    .map_err(|_| "timeout".to_string())?
    .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let text = resp.text().await.map_err(|e| e.to_string())?;

    let script_re = regex::Regex::new(r"(?is)<(script|style)[^>]*>.*?</\1>").unwrap();
    let no_scripts = script_re.replace_all(&text, " ");
    let tag_re = regex::Regex::new(r"(?is)<[^>]+>").unwrap();
    let mut clean = tag_re.replace_all(&no_scripts, " ").into_owned();

    let space_re = regex::Regex::new(r"\s+").unwrap();
    clean = space_re.replace_all(&clean, " ").trim().to_string();

    clean = clean
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&quot;", "\"");

    let ignore = [
        "enable javascript", "javascript is disabled",
        "accept cookies", "accepter les cookies",
        "captcha", "security check", "are you a human",
    ];
    if ignore.iter().any(|p| clean.to_lowercase().contains(p)) {
        return Err("blocked page".to_string());
    }

    if clean.len() < 100 {
        return Err("too short".to_string());
    }

    Ok(clean.chars().take(MAX_PAGE_CHARS).collect())
}

pub async fn scrape_youtube(url: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client.get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send().await.map_err(|e| e.to_string())?
        .text().await.map_err(|e| e.to_string())?;

    let re = regex::Regex::new(r"ytInitialPlayerResponse\s*=\s*(\{.+?\});").unwrap();
    let json_str = if let Some(caps) = re.captures(&resp) {
        caps.get(1).unwrap().as_str()
    } else {
        return Err("Could not find ytInitialPlayerResponse. Are you sure this is a video?".into());
    };

    let v: serde_json::Value = serde_json::from_str(json_str).map_err(|e| e.to_string())?;
    let captions = v.get("captions")
        .and_then(|c| c.get("playerCaptionsTracklistRenderer"))
        .and_then(|p| p.get("captionTracks"))
        .and_then(|c| c.as_array());

    let captions = match captions {
        Some(c) if !c.is_empty() => c,
        _ => return Err("No captions found for this video.".into()),
    };

    let base_url = captions[0].get("baseUrl").and_then(|u| u.as_str()).ok_or("No base_url found")?;
    let xml_resp = client.get(base_url).send().await.map_err(|e| e.to_string())?.text().await.map_err(|e| e.to_string())?;

    let text_re = regex::Regex::new(r"<text[^>]*>([^<]+)</text>").unwrap();
    let mut transcript = String::new();
    for cap in text_re.captures_iter(&xml_resp) {
        if let Some(m) = cap.get(1) {
            let text = m.as_str().replace("&amp;", "&").replace("&#39;", "'").replace("&quot;", "\"");
            transcript.push_str(&text);
            transcript.push(' ');
        }
    }

    Ok(transcript.trim().to_string())
}

pub async fn scrape_reddit(url: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let old_url = url.replace("www.reddit.com", "old.reddit.com").replace("reddit.com", "old.reddit.com");
    let resp = client.get(&old_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .send().await.map_err(|e| e.to_string())?
        .text().await.map_err(|e| e.to_string())?;

    let md_re = regex::Regex::new(r#"(?si)<div class="md">(.*?)</div>"#).unwrap();
    let tag_re = regex::Regex::new(r"<[^>]+>").unwrap();
    let mut content = String::new();

    for cap in md_re.captures_iter(&resp) {
        if let Some(m) = cap.get(1) {
            let text = m.as_str().replace("<p>", "").replace("</p>", "\n").replace("<br/>", "\n");
            let clean = tag_re.replace_all(&text, " ");
            let decoded = clean
                .replace("&lt;", "<").replace("&gt;", ">")
                .replace("&amp;", "&").replace("&#39;", "'").replace("&quot;", "\"");
            content.push_str(decoded.trim());
            content.push_str("\n---\n");
        }
    }

    if content.is_empty() {
        return Err("Could not extract Reddit content.".into());
    }

    if content.len() > 50000 {
        content.truncate(50000);
        content.push_str("... [TRUNCATED]");
    }

    Ok(content)
}

pub async fn super_rag(query: &str, text: &str, top_k: usize) -> Result<String, String> {
    if text.len() < 3000 {
        return Ok(text.to_string());
    }

    let mut chunks: Vec<String> = text.split("\n\n")
        .filter(|c| !c.trim().is_empty())
        .map(|c| c.trim().to_string())
        .collect();

    if chunks.len() < top_k {
        chunks = text.split(". ")
            .filter(|c| !c.trim().is_empty())
            .map(|c| c.trim().to_string())
            .collect();
    }

    if chunks.is_empty() {
        return Ok(String::new());
    }

    let chunk_vectors = crate::ollama::embed(chunks.clone(), "nomic-embed-text:latest").await?;
    let query_vector = crate::ollama::embed(vec![query.to_string()], "nomic-embed-text:latest").await?;

    if query_vector.is_empty() || chunk_vectors.is_empty() {
        return Ok(text.chars().take(3000).collect());
    }

    let mut index = crate::embeddings::VaultIndex::new();
    for (i, (chunk, vector)) in chunks.iter().zip(chunk_vectors).enumerate() {
        index.add(&vector, crate::embeddings::ChunkMeta {
            path: format!("rag_{}", i),
            chunk: chunk.clone(),
            created_at: 0,
            last_accessed: 0,
            access_count: 0,
            pinned: false,
        });
    }

    let mut results = index.search(&query_vector[0], top_k);
    results.sort_by_key(|r| r.id);

    Ok(results.into_iter().map(|r| r.chunk).collect::<Vec<_>>().join("\n\n...\n\n"))
}

pub async fn fetch_url(url: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .map_err(|e| format!("Failed to build client: {}", e))?;

    let resp = client.get(url).send().await.map_err(|e| format!("Failed to fetch URL: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP Error: {}", resp.status()));
    }

    let text = resp.text().await.map_err(|e| format!("Failed to read text: {}", e))?;

    let script_re = regex::Regex::new(r"(?is)<(script|style)[^>]*>.*?</\1>").unwrap();
    let no_scripts = script_re.replace_all(&text, " ");
    let tag_re = regex::Regex::new(r"(?is)<[^>]+>").unwrap();
    let mut clean = tag_re.replace_all(&no_scripts, " ").into_owned();

    clean = clean
        .replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">")
        .replace("&amp;", "&").replace("&#39;", "'").replace("&quot;", "\"");

    let space_re = regex::Regex::new(r"\s+").unwrap();
    clean = space_re.replace_all(&clean, " ").trim().to_string();

    if clean.is_empty() {
        return Err("Page was empty after stripping HTML.".to_string());
    }

    if clean.len() > 15_000 {
        clean.truncate(15_000);
        clean.push_str("... [TRUNCATED]");
    }

    Ok(clean)
}

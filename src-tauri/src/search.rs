pub async fn duckduckgo_search(query: &str) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let venv_python = crate::pyenv::venv_python(&home.join("Projects/Horizon/.venv"));
    let script_path = home.join("Projects/Horizon/search_web.py");

    let output = tokio::process::Command::new(venv_python)
        .arg(script_path)
        .arg(query)
        .output()
        .await
        .map_err(|e| format!("Failed to run search script: {}", e))?;

    if !output.status.success() {
        eprintln!("search stderr: {}", String::from_utf8_lossy(&output.stderr));
        return Err("Web search failed".into());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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
    let mut content = String::new();
    for cap in md_re.captures_iter(&resp) {
        if let Some(m) = cap.get(1) {
            let text = m.as_str().replace("<p>", "").replace("</p>", "\n").replace("<br/>", "\n");
            let tag_re = regex::Regex::new(r"<[^>]+>").unwrap();
            let clean = tag_re.replace_all(&text, " ");
            
            let decoded = clean.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&").replace("&#39;", "'").replace("&quot;", "\"");
            content.push_str(decoded.trim());
            content.push_str("\n---\n");
        }
    }

    if content.is_empty() {
        return Err("Could not extract Reddit content. The page might be blocked or empty.".into());
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
    
    // Chunk the text into paragraphs
    let chunks: Vec<String> = text.split("\n\n")
        .filter(|c| !c.trim().is_empty())
        .map(|c| c.trim().to_string())
        .collect();
        
    if chunks.is_empty() {
        return Ok("".to_string());
    }
    
    // Fallback to smaller chunks if there are very few paragraphs
    let chunks = if chunks.len() < top_k {
        text.split(". ")
            .filter(|c| !c.trim().is_empty())
            .map(|c| c.trim().to_string())
            .collect()
    } else {
        chunks
    };

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
    
    let results = index.search(&query_vector[0], top_k);
    let mut best_chunks = Vec::new();
    // Sort results by ID to keep original text order
    let mut sorted_results = results;
    sorted_results.sort_by_key(|r| r.id);
    
    for r in sorted_results {
        best_chunks.push(r.chunk);
    }
    
    let combined = best_chunks.join("\n\n...\n\n");
    Ok(combined)
}

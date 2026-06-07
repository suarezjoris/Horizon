use std::path::PathBuf;
use std::fs::File;
use std::io::{Write, BufWriter};
use tauri::{AppHandle, Emitter};
use crate::settings;
use futures_util::StreamExt;
use zim::Zim;

#[derive(serde::Serialize, Clone)]
struct Progress {
    percentage: f64,
    bytes_done: u64,
    total_bytes: u64,
}

#[tauri::command]
pub async fn sync_wikipedia(app: AppHandle) -> Result<String, String> {
    let s = settings::load();
    let wiki_dir = PathBuf::from(&s.vault_path).join("knowledge");
    std::fs::create_dir_all(&wiki_dir).map_err(|e| e.to_string())?;

    // Targeting the correct latest Full English Wikipedia (Text only)
    let url = "https://download.kiwix.org/zim/wikipedia/wikipedia_en_all_nopic_2026-03.zim";
    let dest = wiki_dir.join("wikipedia_en.zim");

    println!("[WikiSync] Starting download from: {}", url);

    let client = reqwest::Client::new();
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Download failed: Server returned status {}", response.status()));
    }

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut last_emitted_bytes: u64 = 0;
    let emit_threshold = 10 * 1024 * 1024; // 10 MB

    let file = File::create(&dest).map_err(|e| e.to_string())?;
    let mut writer = BufWriter::with_capacity(128 * 1024, file); // 128KB buffer
    let mut stream = response.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| e.to_string())?;
        writer.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        // Throttle UI events (every 10MB)
        if downloaded - last_emitted_bytes > emit_threshold || downloaded == total_size {
            let percentage = (downloaded as f64 / total_size as f64) * 100.0;
            let _ = app.emit("wiki-download-progress", Progress {
                percentage,
                bytes_done: downloaded,
                total_bytes: total_size,
            });
            last_emitted_bytes = downloaded;
            
            // Log to terminal for debugging
            println!("[WikiSync] Progress: {:.2}% ({:.1}/{:.1} MB)", 
                percentage, 
                downloaded as f64 / 1024.0 / 1024.0, 
                total_size as f64 / 1024.0 / 1024.0
            );
        }
    }
    
    writer.flush().map_err(|e| e.to_string())?;

    Ok("Wikipedia synchronisée !".to_string())
}

pub fn search_wikipedia(query: &str) -> Option<String> {
    let s = settings::load();
    let wiki_path = PathBuf::from(&s.vault_path).join("knowledge/wikipedia_en.zim");
    
    if !wiki_path.exists() {
        return None;
    }

    // 1. Clean the query
    let mut clean_query = query.to_lowercase();
    let noise = ["who is", "what is", "qui est", "qu'est ce que", "tell me about", "cherche", "search"];
    for n in noise {
        clean_query = clean_query.replace(n, "");
    }
    
    let target = clean_query.trim().replace(' ', "_");
    if target.len() < 3 { return None; }

    if let Ok(zim) = Zim::new(wiki_path.to_str().unwrap()) {
        let count = zim.article_count() as u32;
        let limit = count.min(100_000); 

        for i in 0..limit {
            if let Ok(entry) = zim.get_by_url_index(i) {
                let url_low = entry.url.to_lowercase();
                if format!("{:?}", entry.namespace).contains("Articles") && url_low.contains(&target) {
                    if let Some(zim::Target::Cluster(c_idx, b_idx)) = entry.target {
                        if let Ok(cluster) = zim.get_cluster(c_idx) {
                            if let Ok(blob) = cluster.get_blob(b_idx) {
                                let html = String::from_utf8_lossy(&blob).into_owned();
                                return Some(strip_html(&html));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn strip_html(html: &str) -> String {
    let re = regex::Regex::new(r"<[^>]*>").unwrap();
    let text = re.replace_all(html, "");
    
    // Comprehensive entity and unicode cleanup
    text.replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("\\u0254", "o") 
        .replace("\\u2013", "-")
        .replace("\\u2014", "-")
        .replace("\\u2019", "'")
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(3500)
        .collect()
}

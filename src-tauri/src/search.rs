use reqwest::header::{HeaderMap, USER_AGENT};
use regex::Regex;
use urlencoding::encode;

pub async fn duckduckgo_search(query: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let url = format!("https://html.duckduckgo.com/html/?q={}", encode(query));

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".parse().unwrap());

    let resp = client.get(url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| format!("Search request failed: {e}"))?;

    let html = resp.text().await.map_err(|e| format!("Failed to read search body: {e}"))?;

    // Simple regex extraction for DuckDuckGo HTML results
    // Titles are in <a class="result__a">...</a>
    // Snippets are in <a class="result__snippet">...</a>
    let title_re = Regex::new(r#"class="result__a"[^>]*>(.*?)</a>"#).unwrap();
    let snippet_re = Regex::new(r#"class="result__snippet"[^>]*>(.*?)</a>"#).unwrap();

    let titles: Vec<_> = title_re.captures_iter(&html)
        .map(|c| c.get(1).unwrap().as_str().replace("<b>", "").replace("</b>", ""))
        .take(5)
        .collect();

    let snippets: Vec<_> = snippet_re.captures_iter(&html)
        .map(|c| c.get(1).unwrap().as_str().replace("<b>", "").replace("</b>", ""))
        .take(5)
        .collect();

    if titles.is_empty() {
        return Ok("No results found on the web.".to_string());
    }

    let mut output = String::from("Results from the web:\n\n");
    for (i, (title, snippet)) in titles.into_iter().zip(snippets.into_iter()).enumerate() {
        output.push_str(&format!("{}. {}\n   {}\n\n", i + 1, title, snippet));
    }

    Ok(output)
}

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatChunk {
    message: ChunkMessage,
    done: bool,
}

#[derive(Deserialize)]
struct ChunkMessage {
    content: String,
}

#[derive(Serialize)]
struct EmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

/// Stream a chat response, emitting "llm-token" events for each token.
/// Returns the full assembled response.
pub async fn chat_stream(
    app: tauri::AppHandle,
    messages: Vec<serde_json::Value>,
    model: &str,
) -> Result<String, String> {
    let client = Client::new();
    let response = client
        .post("http://localhost:11434/api/chat")
        .json(&ChatRequest {
            model: model.to_string(),
            messages,
            stream: true,
        })
        .send()
        .await
        .map_err(|e| format!("Ollama unreachable: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Ollama error: {}", response.status()));
    }

    let mut full = String::new();
    let mut byte_stream = response.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();

    while let Some(chunk) = byte_stream.next().await {
        buf.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let s = String::from_utf8_lossy(&line[..line.len().saturating_sub(1)]);
            if s.is_empty() { continue; }
            if let Ok(c) = serde_json::from_str::<ChatChunk>(&s) {
                if !c.done {
                    full.push_str(&c.message.content);
                    let _ = app.emit("llm-token", &c.message.content);
                }
            }
        }
    }

    Ok(full)
}

/// Get embeddings for a list of texts.
pub async fn embed(texts: Vec<String>, model: &str) -> Result<Vec<Vec<f32>>, String> {
    let client = Client::new();
    let resp: EmbedResponse = client
        .post("http://localhost:11434/api/embed")
        .json(&EmbedRequest { model: model.to_string(), input: texts })
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.embeddings)
}

/// Non-streaming chat, returns the full response content.
pub async fn chat_once(
    messages: Vec<serde_json::Value>,
    model: &str,
) -> Result<String, String> {
    let client = Client::new();
    let resp = client
        .post("http://localhost:11434/api/chat")
        .json(&serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": false,
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp["message"]["content"].as_str().unwrap_or("").to_string())
}

/// Describe an image using moondream:latest
pub async fn describe_image(base64_image: &str) -> Result<String, String> {
    let client = Client::new();
    let resp = client
        .post("http://localhost:11434/api/generate")
        .json(&serde_json::json!({
            "model": "moondream:latest",
            "prompt": "Describe this image in detail.",
            "images": [base64_image],
            "stream": false,
        }))
        .send()
        .await
        .map_err(|e| format!("Ollama unreachable for vision: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Ollama vision error: {}", resp.status()));
    }

    let json_resp: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(json_resp["response"].as_str().unwrap_or("No description provided.").to_string())
}

/// Unload the active model from VRAM (sets keep_alive to 0).
pub async fn unload(model: &str) -> Result<(), String> {
    let client = Client::new();
    let _ = client
        .post("http://localhost:11434/api/generate")
        .json(&serde_json::json!({ "model": model, "keep_alive": 0 }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

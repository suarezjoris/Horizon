use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    stream: bool,
    // Guarantee the model unloads from VRAM after idle, even if a global
    // OLLAMA_KEEP_ALIVE is set long/infinite. Frees ~VRAM when not chatting.
    keep_alive: String,
    options: serde_json::Value,
}

#[derive(Deserialize)]
struct ChatChunk {
    message: AgentMessage,
    done: bool,
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

lazy_static::lazy_static! {
    static ref HTTP_CLIENT: Client = Client::new();
}

/// Stream a chat response, emitting "llm-token" events for each token.
/// Returns the full assembled response.
pub async fn chat_stream(
    app: tauri::AppHandle,
    messages: Vec<serde_json::Value>,
    model: &str,
    silent: bool,
) -> Result<String, String> {
    let response = HTTP_CLIENT
        .post("http://localhost:11434/api/chat")
        .json(&ChatRequest {
            model: model.to_string(),
            messages,
            stream: true,
            keep_alive: "5m".to_string(),
            options: serde_json::json!({
                "num_ctx": 8192
            }),
        })
        .send()
        .await
        .map_err(|e| format!("Ollama unreachable: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Ollama error: {}", response.status()));
    }

    let mut full = String::with_capacity(4096);
    let mut byte_stream = response.bytes_stream();
    let mut buf: Vec<u8> = Vec::with_capacity(1024);

    while let Some(chunk) = byte_stream.next().await {
        buf.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
        
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line = buf.drain(..=pos).collect::<Vec<u8>>();
            if let Ok(c) = serde_json::from_slice::<ChatChunk>(&line) {
                if !c.done {
                    if let Some(token) = c.message.content {
                        full.push_str(&token);
                        if !silent {
                            // Skip technical tags in UI
                            if !token.contains("GENERATE_") && !token.contains("SEARCH_WEB") {
                                let _ = app.emit("llm-token", &token);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(full)
}

pub async fn embed(texts: Vec<String>, model: &str) -> Result<Vec<Vec<f32>>, String> {
    let resp: EmbedResponse = HTTP_CLIENT
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

pub async fn chat_once(
    messages: Vec<serde_json::Value>,
    model: &str,
) -> Result<String, String> {
    let resp = HTTP_CLIENT
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
#[allow(dead_code)]
pub async fn describe_image(base64_image: &str) -> Result<String, String> {
    let resp = HTTP_CLIENT
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

#[derive(Deserialize)]
struct ModelList {
    models: Vec<Model>,
}

#[derive(Deserialize)]
struct Model {
    name: String,
}

#[derive(Serialize, Clone)]
pub struct Tool {
    pub r#type: String,
    pub function: ToolFunction,
}

#[derive(Serialize, Clone)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ToolCall {
    pub function: ToolCallFunction,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Deserialize)]
pub struct AgentMessage {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct AgentChatResponse {
    pub message: AgentMessage,
}

pub async fn chat_with_tools(
    app: &tauri::AppHandle,
    messages: Vec<serde_json::Value>,
    tools: &[Tool],
    model: &str,
) -> Result<AgentMessage, String> {
    let response = HTTP_CLIENT
        .post("http://localhost:11434/api/chat")
        .json(&serde_json::json!({
            "model": model,
            "messages": messages,
            "tools": tools,
            "stream": true,
            "keep_alive": "5m",
            "options": {
                "num_ctx": 8192
            }
        }))
        .send()
        .await
        .map_err(|e| format!("Ollama unreachable: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Ollama error: {}", response.status()));
    }

    let mut full_content = String::with_capacity(4096);
    let mut all_tool_calls: Vec<ToolCall> = Vec::new();

    let mut byte_stream = response.bytes_stream();
    let mut buf: Vec<u8> = Vec::with_capacity(1024);

    while let Some(chunk) = byte_stream.next().await {
        buf.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
        
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line = buf.drain(..=pos).collect::<Vec<u8>>();
            if let Ok(c) = serde_json::from_slice::<ChatChunk>(&line) {
                if let Some(token) = c.message.content {
                    full_content.push_str(&token);
                    let _ = app.emit("llm-token", &token);
                }
                if let Some(calls) = c.message.tool_calls {
                    all_tool_calls.extend(calls);
                }
            }
        }
    }

    Ok(AgentMessage {
        content: if full_content.is_empty() { None } else { Some(full_content) },
        tool_calls: if all_tool_calls.is_empty() { None } else { Some(all_tool_calls) },
    })
}

pub async fn list_models() -> Result<Vec<String>, String> {
    let resp: ModelList = HTTP_CLIENT
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .map_err(|e| format!("Ollama unreachable: {e}"))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.models.into_iter().map(|m| m.name).collect())
}

/// Unload the active model from VRAM (sets keep_alive to 0).
pub async fn unload(model: &str) -> Result<(), String> {
    let _ = HTTP_CLIENT
        .post("http://localhost:11434/api/generate")
        .json(&serde_json::json!({ "model": model, "keep_alive": 0 }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_model_hash(model: &str) -> Result<String, String> {
    let resp = HTTP_CLIENT
        .post("http://localhost:11434/api/show")
        .json(&serde_json::json!({ "name": model }))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp["digest"].as_str().unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_message_deserialize_tool_calls() {
        let json = r#"{
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    { "function": { "name": "search_web", "arguments": { "query": "rust async" } } }
                ]
            },
            "done": true
        }"#;
        let resp: AgentChatResponse = serde_json::from_str(json).unwrap();
        let calls = resp.message.tool_calls.unwrap();
        assert_eq!(calls[0].function.name, "search_web");
        assert_eq!(calls[0].function.arguments["query"], "rust async");
    }

    #[test]
    fn test_agent_message_deserialize_text() {
        let json = r#"{
            "message": { "role": "assistant", "content": "Hello world", "tool_calls": null },
            "done": true
        }"#;
        let resp: AgentChatResponse = serde_json::from_str(json).unwrap();
        assert!(resp.message.tool_calls.is_none());
        assert_eq!(resp.message.content.unwrap(), "Hello world");
    }
}

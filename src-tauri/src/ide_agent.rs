use crate::{ollama, tools, settings, plugins::PluginRegistry};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

pub struct IdeState {
    pub orchestrator_history: Arc<Mutex<Vec<serde_json::Value>>>,
}

impl Default for IdeState {
    fn default() -> Self {
        Self {
            orchestrator_history: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

async fn select_model_for_task(prompt: &str) -> String {
    let s = settings::load();
    let messages = vec![
        serde_json::json!({
            "role": "system",
            "content": "You are a classifier. Respond with exactly the word 'HEAVY' if the task requires creating multiple files, complex planning, or is a large project. Respond with 'LIGHT' if it's a simple fix, short question, or basic terminal command."
        }),
        serde_json::json!({ "role": "user", "content": prompt })
    ];
    match ollama::chat_once(messages, &s.llm_model).await {
        Ok(msg) if msg.to_uppercase().contains("HEAVY") => s.heavy_model,
        _ => s.llm_model,
    }
}

fn extract_tool_calls_from_text(text: &str) -> Option<Vec<ollama::ToolCall>> {
    let mut trimmed = text.trim();

    if trimmed.starts_with("```json") && trimmed.ends_with("```") {
        trimmed = trimmed.trim_start_matches("```json").trim_end_matches("```").trim();
    } else if trimmed.starts_with("```") && trimmed.ends_with("```") {
        trimmed = trimmed.trim_start_matches("```").trim_end_matches("```").trim();
    }

    // Try raw JSON tool calls scattered in text
    let mut calls = Vec::new();
    let mut search = trimmed.to_string();
    while let Some(start) = search.find("{\"name\"") {
        let mut found = false;
        for end in (start + 1..=search.len()).rev() {
            if search.as_bytes().get(end - 1) == Some(&b'}') {
                if let Ok(tc) = serde_json::from_str::<ollama::ToolCallFunction>(&search[start..end]) {
                    calls.push(ollama::ToolCall { function: tc });
                    search = search[end..].to_string();
                    found = true;
                    break;
                }
            }
        }
        if !found {
            search = search[start + 1..].to_string();
        }
    }
    if !calls.is_empty() {
        return Some(calls);
    }

    // Array format
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        if let Ok(fns) = serde_json::from_str::<Vec<ollama::ToolCallFunction>>(trimmed) {
            return Some(fns.into_iter().map(|f| ollama::ToolCall { function: f }).collect());
        }
    }

    // Markdown bash block
    for prefix in &["```bash", "```sh"] {
        if let Some(start) = trimmed.find(prefix) {
            let after = &trimmed[start + prefix.len()..];
            if let Some(end) = after.find("```") {
                let script = after[..end].trim().to_string();
                if !script.is_empty() {
                    return Some(vec![ollama::ToolCall {
                        function: ollama::ToolCallFunction {
                            name: "bash".to_string(),
                            arguments: serde_json::json!({ "command": script }),
                        },
                    }]);
                }
            }
        }
    }

    None
}

#[tauri::command]
pub async fn send_ide_prompt(
    app: AppHandle,
    state: State<'_, IdeState>,
    prompt: String,
    mode: String,
) -> Result<(), String> {
    let model = select_model_for_task(&prompt).await;
    let _ = app.emit("ide-status", serde_json::json!({
        "status": "Thinking...", "icon": "🤖", "model": model
    }));

    let workspace = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
    let plugins = PluginRegistry::new();
    let tools_def = tools::build_tool_definitions(true, &plugins);
    let ollama_tools: Vec<ollama::Tool> = tools_def.iter().map(|t| ollama::Tool {
        r#type: "function".into(),
        function: ollama::ToolFunction {
            name: t["function"]["name"].as_str().unwrap_or("").to_string(),
            description: t["function"]["description"].as_str().unwrap_or("").to_string(),
            parameters: t["function"]["parameters"].clone(),
        },
    }).collect();

    let system_prompt = "You are the Horizon Master Orchestrator, an elite local AI coding agent running natively on the user's machine. The terminal starts in the user's home directory.\n\nCORE DIRECTIVES:\n1. You ARE the developer. You write code, create files, compile, test, and debug using the provided tools.\n2. USE THE PROVIDED TOOL SCHEMA TO TAKE ACTIONS. NEVER output raw code blocks or plain text bash commands.\n3. Be ultra-concise in your text responses.\n4. The 'bash' tool is NON-INTERACTIVE. Standard input is disabled.\n\nYou must solve the user's task completely. You have up to 15 turns to use tools. Always verify non-interactive actions using bash commands.";

    let mut current_messages;
    {
        let mut history = state.orchestrator_history.lock().await;
        if history.is_empty() {
            history.push(serde_json::json!({ "role": "system", "content": system_prompt }));
        }
        history.push(serde_json::json!({ "role": "user", "content": prompt.clone() }));
        current_messages = history.clone();
    }

    for iter in 1..=15 {
        let _ = app.emit("ide-status", serde_json::json!({
            "status": format!("Iter {}/15...", iter), "icon": "🔄"
        }));

        let mut response = match ollama::chat_with_tools(&app, current_messages.clone(), &ollama_tools, &model).await {
            Ok(res) => res,
            Err(e) => {
                let _ = app.emit("ide-status", serde_json::json!({"status": "Error", "icon": "❌"}));
                return Err(e);
            }
        };

        // Models that output raw JSON/markdown instead of structured tool_calls
        if response.tool_calls.as_ref().map(|c| c.is_empty()).unwrap_or(true) {
            if let Some(ref content) = response.content.clone() {
                if let Some(calls) = extract_tool_calls_from_text(content) {
                    response.tool_calls = Some(calls);
                }
            }
        }

        if let Some(ref content) = response.content {
            if !content.is_empty() {
                let _ = app.emit("ide-response", serde_json::json!({
                    "message": content, "is_user": false, "mode": mode
                }));
            }
        }

        let calls = match response.tool_calls {
            Some(c) if !c.is_empty() => c,
            _ => break,
        };

        let tool_calls_json: Vec<_> = calls.iter().map(|tc| serde_json::json!({
            "type": "function",
            "function": { "name": tc.function.name, "arguments": tc.function.arguments }
        })).collect();

        current_messages.push(serde_json::json!({
            "role": "assistant",
            "content": response.content.unwrap_or_default(),
            "tool_calls": tool_calls_json
        }));

        for tc in calls {
            let _ = app.emit("ide-tool-start", serde_json::json!({
                "tool": tc.function.name, "args": tc.function.arguments
            }));

            let result = match tools::execute(&tc.function.name, &tc.function.arguments, &workspace, &plugins, &app).await {
                Ok(out) => {
                    let _ = app.emit("ide-tool-done", serde_json::json!({
                        "tool": tc.function.name, "result": out
                    }));
                    out
                }
                Err(e) => {
                    let _ = app.emit("ide-tool-error", serde_json::json!({
                        "tool": tc.function.name, "error": e
                    }));
                    format!("Error: {}", e)
                }
            };

            current_messages.push(serde_json::json!({
                "role": "tool", "name": tc.function.name, "content": result
            }));
        }
    }

    let _ = app.emit("ide-status", serde_json::json!({"status": "Idle", "icon": "💤"}));

    {
        let mut history = state.orchestrator_history.lock().await;
        *history = current_messages;
    }

    Ok(())
}

#[tauri::command]
pub async fn execute_ide_script(
    state: State<'_, crate::pty::PtyState>,
    script: String,
    _mode: String,
) -> Result<(), String> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let temp_path = format!("/tmp/horizon_task_{}.sh", timestamp);
    std::fs::write(&temp_path, format!("set -v\nset -e\n{}\n", script))
        .map_err(|e| format!("Failed to write temp script: {}", e))?;

    let run_cmd = format!(
        "bash -c 'trap \"rm -f {}\" EXIT; bash {}; echo -e \"\\n[HORIZON_EXEC_DONE:$?]\"'\r\n",
        temp_path, temp_path
    );
    crate::pty::pty_write(state, run_cmd).await
}

#[tauri::command]
pub async fn clear_ide_memory(state: State<'_, IdeState>) -> Result<(), String> {
    state.orchestrator_history.lock().await.clear();
    Ok(())
}

use crate::{ollama, tools, settings, plugins::PluginRegistry};
use serde::Serialize;
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
    let fast_model = s.llm_model.clone();
    let heavy_model = s.heavy_model.clone();

    let messages = vec![
        serde_json::json!({
            "role": "system",
            "content": "You are a classifier. Respond with exactly the word 'HEAVY' if the task requires creating multiple files, complex planning, or is a large project. Respond with 'LIGHT' if it's a simple fix, short question, or basic terminal command."
        }),
        serde_json::json!({
            "role": "user",
            "content": prompt
        })
    ];

    match ollama::chat_once(messages, &fast_model).await {
        Ok(msg) => {
            if msg.to_uppercase().contains("HEAVY") {
                heavy_model
            } else {
                fast_model
            }
        }
        Err(_) => heavy_model
    }
}

/// Start an orchestration task based on a user prompt.
#[tauri::command]
pub async fn send_ide_prompt(
    app: AppHandle,
    state: State<'_, IdeState>,
    prompt: String,
    mode: String,
) -> Result<(), String> {
    // 1. Determine model
    let model = select_model_for_task(&prompt).await;
    let _ = app.emit("ide-status", serde_json::json!({
        "status": "Thinking...",
        "icon": "🤖",
        "model": model
    }));

    let workspace = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
    let plugins = PluginRegistry::new();
    let tools_def = tools::build_tool_definitions(true, &plugins);
    let ollama_tools: Vec<ollama::Tool> = tools_def.iter().map(|t| {
        ollama::Tool {
            r#type: "function".into(),
            function: ollama::ToolFunction {
                name: t["function"]["name"].as_str().unwrap_or("").to_string(),
                description: t["function"]["description"].as_str().unwrap_or("").to_string(),
                parameters: t["function"]["parameters"].clone(),
            },
        }
    }).collect();

    let system_prompt = r#"You are the Horizon Master Orchestrator, an elite local AI coding agent running natively on the user's machine. The terminal starts in the user's home directory.
    
YOUR CORE DIRECTIVES:
1. You ARE the developer. You write code, create files, compile, test, and debug using the provided tools.
2. YOU MUST USE THE PROVIDED TOOL SCHEMA TO TAKE ACTIONS. NEVER output raw code blocks or plain text bash commands. NEVER say 'I will run this command'. Just invoke the tool!
3. Be ultra-concise in your text responses.
4. If you need to run a shell command, invoke the 'bash' tool. If you need to edit a file, invoke the 'edit_file' tool.

CRITICAL: The 'bash' tool is NON-INTERACTIVE. Standard input is disabled. If you write an interactive script (like a game requiring user input), DO NOT try to run it using the 'bash' tool. Just write the file, ensure it's correct, and tell the user they can run it in their own terminal.

You must solve the user's task completely. You have up to 15 turns to use tools. Always verify your non-interactive actions using bash commands."#;

    let mut current_messages;
    {
        let mut history = state.orchestrator_history.lock().await;
        if history.is_empty() {
            history.push(serde_json::json!({
                "role": "system",
                "content": system_prompt
            }));
        }
        history.push(serde_json::json!({
            "role": "user",
            "content": prompt.clone()
        }));
        current_messages = history.clone();
    }
    
    let mut iter = 0;
    while iter < 15 {
        iter += 1;
        let _ = app.emit("ide-status", serde_json::json!({
            "status": format!("Iter {}/15...", iter),
            "icon": "🔄"
        }));

        let mut response = match ollama::chat_with_tools(&app, current_messages.clone(), &ollama_tools, &model).await {
            Ok(res) => res,
            Err(e) => {
                let _ = app.emit("ide-status", serde_json::json!({"status": "Error", "icon": "❌"}));
                return Err(e);
            }
        };

        // Fix for models that output raw JSON in content instead of tool_calls
        if response.tool_calls.is_none() || response.tool_calls.as_ref().unwrap().is_empty() {
            if let Some(content) = response.content.clone() {
                let mut text = content.trim();
                
                // Remove trailing markdown ticks
                if text.starts_with("```json") && text.ends_with("```") {
                    text = text.trim_start_matches("```json").trim_end_matches("```").trim();
                } else if text.starts_with("```") && text.ends_with("```") {
                    text = text.trim_start_matches("```").trim_end_matches("```").trim();
                }

                // 1. Check for raw JSON tool call(s) embedded anywhere in the text
                let mut extracted_calls = Vec::new();
                let mut search_text = text.to_string();
                
                while let Some(start) = search_text.find("{\"name\"") {
                    let mut found = false;
                    for end in (start + 1..=search_text.len()).rev() {
                        if search_text[end-1..end] == *"}" {
                            if let Ok(tc_func) = serde_json::from_str::<ollama::ToolCallFunction>(&search_text[start..end]) {
                                extracted_calls.push(ollama::ToolCall { function: tc_func });
                                search_text = search_text[end..].to_string();
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        search_text = search_text[start + 1..].to_string();
                    }
                }

                if !extracted_calls.is_empty() {
                    response.tool_calls = Some(extracted_calls);
                } else if text.starts_with('[') && text.ends_with(']') {
                    // Fallback for array format [ { "name": ... } ]
                    if let Ok(tcs) = serde_json::from_str::<Vec<ollama::ToolCallFunction>>(text) {
                        response.tool_calls = Some(tcs.into_iter().map(|f| ollama::ToolCall { function: f }).collect());
                        response.content = None;
                    }
                }
                
                // 2. Check for markdown bash block
                if response.tool_calls.is_none() {
                    if let Some(start) = text.find("```bash") {
                        if let Some(end) = text[start + 7..].find("```") {
                            let script = text[start + 7 .. start + 7 + end].trim().to_string();
                            if !script.is_empty() {
                                response.tool_calls = Some(vec![ollama::ToolCall {
                                    function: ollama::ToolCallFunction {
                                        name: "bash".to_string(),
                                        arguments: serde_json::json!({ "command": script }),
                                    }
                                }]);
                                // Do not set content to None, allow the text to be seen, but extract the bash block
                            }
                        }
                    } else if let Some(start) = text.find("```sh") {
                        if let Some(end) = text[start + 5..].find("```") {
                            let script = text[start + 5 .. start + 5 + end].trim().to_string();
                            if !script.is_empty() {
                                response.tool_calls = Some(vec![ollama::ToolCall {
                                    function: ollama::ToolCallFunction {
                                        name: "bash".to_string(),
                                        arguments: serde_json::json!({ "command": script }),
                                    }
                                }]);
                            }
                        }
                    }
                }
                
                // 3. Last resort fallback for "bash command \"...\"" formats or similar weirdness
                if response.tool_calls.is_none() {
                    if text.to_lowercase().starts_with("bash command \"") && text.ends_with("\"") {
                        let script = text[14..text.len() - 1].to_string();
                        response.tool_calls = Some(vec![ollama::ToolCall {
                            function: ollama::ToolCallFunction {
                                name: "bash".to_string(),
                                arguments: serde_json::json!({ "command": script }),
                            }
                        }]);
                        response.content = None;
                    }
                }
            }
        }

        if let Some(content) = &response.content {
            if !content.is_empty() {
                let _ = app.emit("ide-response", serde_json::json!({
                    "message": content,
                    "is_user": false,
                    "mode": mode.clone(),
                    "has_code": false,
                    "script": ""
                }));
            }
        }

        if let Some(calls) = response.tool_calls {
            if calls.is_empty() {
                break;
            }

            // Append assistant msg with tool calls
            let tool_calls_json = calls.iter().map(|tc| serde_json::json!({
                "type": "function",
                "function": { "name": tc.function.name.clone(), "arguments": tc.function.arguments.clone() }
            })).collect::<Vec<_>>();
            
            let assistant_msg = serde_json::json!({
                "role": "assistant",
                "content": response.content.unwrap_or_default(),
                "tool_calls": tool_calls_json
            });
            current_messages.push(assistant_msg.clone());

            for tc in calls {
                let _ = app.emit("ide-tool-start", serde_json::json!({
                    "tool": tc.function.name.clone(),
                    "args": tc.function.arguments.clone()
                }));

                let result = match tools::execute(&tc.function.name, &tc.function.arguments, &workspace, &plugins, &app).await {
                    Ok(out) => {
                        let _ = app.emit("ide-tool-done", serde_json::json!({
                            "tool": tc.function.name.clone(),
                            "result": out.clone()
                        }));
                        out
                    }
                    Err(e) => {
                        let _ = app.emit("ide-tool-error", serde_json::json!({
                            "tool": tc.function.name.clone(),
                            "error": e.clone()
                        }));
                        format!("Error: {}", e)
                    }
                };

                current_messages.push(serde_json::json!({
                    "role": "tool",
                    "name": tc.function.name,
                    "content": result
                }));
            }
        } else {
            break;
        }
    }

    let _ = app.emit("ide-status", serde_json::json!({"status": "Idle", "icon": "💤"}));

    // Save back to history
    {
        let mut history = state.orchestrator_history.lock().await;
        *history = current_messages;
    }

    Ok(())
}

/// Execute script directly to PTY robustly (Legacy / Explicit command support)
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
    
    let script_content = format!("set -v\nset -e\n{}\n", script);
    
    if let Err(e) = std::fs::write(&temp_path, script_content) {
        return Err(format!("Failed to write temp script: {}", e));
    }
    
    let run_cmd = format!("bash -c 'trap \"rm -f {}\" EXIT; bash {}; echo -e \"\\n[HORIZON_EXEC_DONE:$?]\"'\r\n", temp_path, temp_path);
    crate::pty::pty_write(state.clone(), run_cmd).await?;
    
    Ok(())
}

/// Clear IDE memory
#[tauri::command]
pub async fn clear_ide_memory(state: State<'_, IdeState>) -> Result<(), String> {
    let mut history = state.orchestrator_history.lock().await;
    history.clear();
    Ok(())
}

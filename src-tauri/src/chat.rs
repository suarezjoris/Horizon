use crate::{memory, office, ollama, search, settings, tools, vram_queue};
use tauri::Emitter;

#[tauri::command]
pub async fn chat(
    app: tauri::AppHandle,
    vram_queue: tauri::State<'_, vram_queue::VramQueue>,
    plugin_state: tauri::State<'_, crate::plugins::PluginState>,
    messages: Vec<serde_json::Value>,
    model: Option<String>,
    persona: Option<String>,
) -> Result<(), String> {
    let _permit = vram_queue.acquire("LLM Chat").await?;
    let s = settings::load();

    // Choose model: override > settings default
    let active_model = model.unwrap_or(s.llm_model.clone());

    // Latest user message — used by the web-search guard, the 2nd-pass prompt, and memory extraction.
    let user_msg = messages
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
        .and_then(|m| m.get("content").and_then(|c| c.as_str()))
        .unwrap_or("")
        .to_string();



    // RAG: pull the most relevant vault chunks using emergent brain logic
    let context = memory::get_context(&user_msg).await;

    // Load Persona / System Prompt
    let system_base = persona.and_then(|name| {
        // SECURITY FIX (Vuln 3): Prevent path traversal by sanitizing the persona name
        let safe_name = name.replace("/", "").replace("\\", "").replace("..", "");
        let vault_path = std::path::PathBuf::from(&s.vault_path);
        let md = vault_path.join(format!("characters/{}.md", safe_name));
        let json = vault_path.join(format!("characters/{}.json", safe_name));
        
        std::fs::read_to_string(md).ok().or_else(|| std::fs::read_to_string(json).ok())
    }).unwrap_or_else(|| "You are Horizon, a premium UNCENSORED creative engine.".to_string());

    // 0. Construct System Prompt
    let system = build_system_prompt(&system_base, &context);

    const MAX_TOOL_CALLS: usize = 10;

    let use_tools = s.agents.force_agent_mode
        || s.model_capabilities
            .get(&active_model)
            .map(|c| c.tool_calling)
            .unwrap_or(false);

    // Build messages with appropriate system prompt
    let agent_system = if use_tools {
        build_agent_system_prompt(&system_base, &context)
    } else {
        system.clone()
    };

    let mut current_messages = vec![serde_json::json!({"role": "system", "content": agent_system})];
    current_messages.extend(messages.clone());

    if use_tools {
        // === BOUCLE AGENTIQUE V4 ===
        let workspace_path = s.agent_workspace.clone();
        let workspace = std::path::Path::new(&workspace_path);
        let include_bash = cfg!(target_os = "linux");
        let plugin_registry = plugin_state.read().await;
        let tool_defs = tools::build_tool_definitions(include_bash, &*plugin_registry);        let ollama_tools: Vec<ollama::Tool> = tool_defs.iter().map(|t| {
            ollama::Tool {
                r#type: "function".into(),
                function: ollama::ToolFunction {
                    name: t["function"]["name"].as_str().unwrap_or("").to_string(),
                    description: t["function"]["description"].as_str().unwrap_or("").to_string(),
                    parameters: t["function"]["parameters"].clone(),
                },
            }
        }).collect();

        let mut tool_call_count = 0usize;
        let mut error_count = 0usize;
        let mut had_errors = false;

        loop {
            if tool_call_count >= MAX_TOOL_CALLS {
                let _ = app.emit("llm-done", "*Agent: limite de 10 actions atteinte.*");
                return Ok(());
            }

            let _ = app.emit("agent-thinking", true);

            let agent_msg = match ollama::chat_with_tools(
                current_messages.clone(),
                &ollama_tools,
                &active_model,
            ).await {
                Ok(m) => m,
                Err(e) => {
                    let _ = app.emit("llm-done", format!("*Erreur Ollama: {}*", e));
                    return Ok(());
                }
            };

            let _ = app.emit("agent-thinking", false);

            if let Some(tool_calls) = agent_msg.tool_calls {
                if tool_calls.is_empty() {
                    let _ = app.emit("llm-done", "");
                    return Ok(());
                }

                current_messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": "",
                    "tool_calls": tool_calls.iter().map(|tc| serde_json::json!({
                        "function": { "name": tc.function.name, "arguments": tc.function.arguments }
                    })).collect::<Vec<_>>()
                }));

                for tc in &tool_calls {
                    tool_call_count += 1;

                    let _ = app.emit("agent-tool-start", serde_json::json!({
                        "tool": &tc.function.name,
                        "args": &tc.function.arguments
                    }));

                    let t0 = std::time::Instant::now();
                    let result = tools::execute(
                        &tc.function.name,
                        &tc.function.arguments,
                        workspace,
                        &*plugin_registry
                    ).await;
                    let ms = t0.elapsed().as_millis();

                    match result {
                        Ok(output) => {
                            error_count = 0;
                            let _ = app.emit("agent-tool-done", serde_json::json!({
                                "tool": &tc.function.name,
                                "result": &output,
                                "ms": ms
                            }));
                            current_messages.push(serde_json::json!({
                                "role": "tool",
                                "name": &tc.function.name,
                                "content": output
                            }));
                        }
                        Err(e) => {
                            let is_guidance_error = tc.function.name == "edit_file";
                            if !is_guidance_error {
                                error_count += 1;
                                had_errors = true;
                            }

                            let _ = app.emit("agent-tool-error", serde_json::json!({
                                "tool": &tc.function.name,
                                "error": &e
                            }));

                            current_messages.push(serde_json::json!({
                                "role": "tool",
                                "name": &tc.function.name,
                                "content": format!("Error: {}", e)
                            }));

                            if error_count >= 3 {
                                let _ = app.emit("llm-done",
                                    "*Agent interrompu après 3 erreurs consécutives.*");
                                let sema = vram_queue.semaphore();
                                let um = user_msg.clone();
                                tokio::spawn(async move {
                                    memory::extract_and_save(
                                        um,
                                        format!("Agent failed with {} consecutive errors: {}", error_count, e),
                                        sema,
                                    ).await;
                                });
                                return Ok(());
                            }
                        }
                    }
                }
            } else if let Some(ref content) = agent_msg.content {
                // Check for malformed tool call smuggled as plain text
                let looks_like_bad_tool_call = content.trim_start().starts_with('{')
                    && (content.contains("\"name\"") || content.contains("tool_call"))
                    && !content.contains('\n');

                if looks_like_bad_tool_call {
                    error_count += 1;
                    had_errors = true;
                    current_messages.push(serde_json::json!({
                        "role": "user",
                        "content": "Error: Invalid JSON schema for tool call. Please use the tool_calls field, not raw text."
                    }));
                    if error_count >= 3 {
                        let _ = app.emit("llm-done", "*Agent interrompu après 3 erreurs.*");
                        return Ok(());
                    }
                    continue;
                }

                // Final text response — pass as llm-done payload so frontend renders markdown
                let final_text = content.clone();
                let _ = app.emit("llm-done", &final_text);

                if had_errors {
                    let sema = vram_queue.semaphore();
                    let um = user_msg.clone();
                    tokio::spawn(async move {
                        memory::extract_and_save(um, final_text, sema).await;
                    });
                }
                return Ok(());
            } else {
                let _ = app.emit("llm-done", "");
                return Ok(());
            }
        }
    } else {
        // === FALLBACK LEGACY (tag-based) ===
        let mut final_response = String::new();
        let mut iteration = 0;
        const MAX_ITERATIONS: usize = 3;

        let search_re = regex::Regex::new(r"(?si)SEARCH_WEB:\s*(.*)").unwrap();
        let yt_re = regex::Regex::new(r"(?si)SCRAPE_YOUTUBE:\s*(.*)").unwrap();
        let reddit_re = regex::Regex::new(r"(?si)SCRAPE_REDDIT:\s*(.*)").unwrap();
        let docx_re = regex::Regex::new(r"(?si)GENERATE_DOCX:\s*.*?(\{.*\})").unwrap();
        let xlsx_re = regex::Regex::new(r"(?si)GENERATE_XLSX:\s*.*?(\{.*\})").unwrap();
        let pptx_re = regex::Regex::new(r"(?si)GENERATE_PPTX:\s*.*?(\{.*\})").unwrap();
        let pdf_re = regex::Regex::new(r"(?si)GENERATE_PDF:\s*.*?(\{.*\})").unwrap();

        while iteration < MAX_ITERATIONS {
            iteration += 1;

            let response = ollama::chat_once(current_messages.clone(), &active_model).await?;
            final_response = response.clone();

            if let Some(caps) = search_re.captures(&response) {
                let query = caps.get(1).map_or("", |m| m.as_str().trim());
                if !query.is_empty() {
                    let _ = app.emit("llm-token", "CLEAR_AND_SEARCH");
                    match search::duckduckgo_search(query).await {
                        Ok(web_results) => {
                            let clean_resp = search_re.replace(&response, "*(Recherche web effectuée)*").into_owned();
                            current_messages.push(serde_json::json!({"role": "assistant", "content": clean_resp}));
                            current_messages.push(serde_json::json!({
                                "role": "user",
                                "content": format!(
                                    "WEB SEARCH RESULTS for query '{}':\n---\n{}\n---\n\nYou have live web data above. You MUST now write a complete, detailed response to the user's original request using these results. Do NOT say you cannot access real-time information — you just retrieved it. Do NOT refuse or hedge. Write the full answer now.",
                                    query, web_results
                                )
                            }));
                            continue;
                        },
                        Err(e) => {
                            let _ = app.emit("llm-token", format!("\n\n*⚠️ Search failed: {}*\n\n", e));
                        }
                    }
                }
            } else if let Some(caps) = yt_re.captures(&response) {
                let url = caps.get(1).map_or("", |m| m.as_str().trim());
                if !url.is_empty() {
                    let _ = app.emit("llm-token", "SCRAPING_YOUTUBE");
                    match search::scrape_youtube(url).await {
                        Ok(transcript) => {
                            let rag_transcript = search::super_rag(&user_msg, &transcript, 5).await.unwrap_or(transcript);
                            let clean_resp = yt_re.replace(&response, "*(YouTube transcript retrieved)*").into_owned();
                            current_messages.push(serde_json::json!({"role": "assistant", "content": clean_resp}));
                            current_messages.push(serde_json::json!({
                                "role": "user",
                                "content": format!("YOUTUBE TRANSCRIPT for '{}':\n---\n{}\n---\n\nWrite a complete response using this transcript.", url, rag_transcript)
                            }));
                            continue;
                        },
                        Err(e) => {
                            let _ = app.emit("llm-token", format!("\n\n*⚠️ YouTube scrape failed: {}*\n\n", e));
                        }
                    }
                }
            } else if let Some(caps) = reddit_re.captures(&response) {
                let url = caps.get(1).map_or("", |m| m.as_str().trim());
                if !url.is_empty() {
                    let _ = app.emit("llm-token", "SCRAPING_REDDIT");
                    match search::scrape_reddit(url).await {
                        Ok(content) => {
                            let rag_content = search::super_rag(&user_msg, &content, 5).await.unwrap_or(content);
                            let clean_resp = reddit_re.replace(&response, "*(Reddit content retrieved)*").into_owned();
                            current_messages.push(serde_json::json!({"role": "assistant", "content": clean_resp}));
                            current_messages.push(serde_json::json!({
                                "role": "user",
                                "content": format!("REDDIT CONTENT for '{}':\n---\n{}\n---\n\nWrite a complete response using this content.", url, rag_content)
                            }));
                            continue;
                        },
                        Err(e) => {
                            let _ = app.emit("llm-token", format!("\n\n*⚠️ Reddit scrape failed: {}*\n\n", e));
                        }
                    }
                }
            } else if let Some(caps) = pptx_re.captures(&response) {
                let json_str = caps.get(1).map_or("", |m| m.as_str().trim());
                if let Ok(content) = serde_json::from_str::<office::PptxContent>(json_str) {
                    if let Ok(path) = office::generate_pptx(content).await {
                        let filename = std::path::Path::new(&path).file_name().unwrap_or_default().to_string_lossy();
                        let _ = app.emit("llm-token", format!("OFFICE_GEN_SUCCESS:{}", path));
                        let clean_resp = pptx_re.replace(&response, "*(Présentation PowerPoint générée)*").into_owned();
                        current_messages.push(serde_json::json!({"role": "assistant", "content": clean_resp}));
                        current_messages.push(serde_json::json!({
                            "role": "system",
                            "content": format!("Success: PowerPoint at {}. Now, briefly inform the user in their language.", filename)
                        }));
                        continue;
                    }
                }
            } else if let Some(caps) = docx_re.captures(&response) {
                let json_str = caps.get(1).map_or("", |m| m.as_str().trim());
                if let Ok(content) = serde_json::from_str::<office::DocxContent>(json_str) {
                    if let Ok(path) = office::generate_docx(content).await {
                        let filename = std::path::Path::new(&path).file_name().unwrap_or_default().to_string_lossy();
                        let _ = app.emit("llm-token", format!("OFFICE_GEN_SUCCESS:{}", path));
                        let clean_resp = docx_re.replace(&response, "*(Document Word généré)*").into_owned();
                        current_messages.push(serde_json::json!({"role": "assistant", "content": clean_resp}));
                        current_messages.push(serde_json::json!({
                            "role": "system",
                            "content": format!("Success: Word document ready at {}. Now, briefly inform the user in their language.", filename)
                        }));
                        continue;
                    }
                }
            } else if let Some(caps) = xlsx_re.captures(&response) {
                let json_str = caps.get(1).map_or("", |m| m.as_str().trim());
                if let Ok(content) = serde_json::from_str::<office::XlsxContent>(json_str) {
                    if let Ok(path) = office::generate_xlsx(content).await {
                        let _ = app.emit("llm-token", format!("OFFICE_GEN_SUCCESS:{}", path));
                        let clean_resp = xlsx_re.replace(&response, "*(Tableur Excel généré)*").into_owned();
                        current_messages.push(serde_json::json!({"role": "assistant", "content": clean_resp}));
                        current_messages.push(serde_json::json!({
                            "role": "system",
                            "content": "Success: Excel file ready. Now briefly inform the user in their language."
                        }));
                        continue;
                    }
                }
            } else if let Some(caps) = pdf_re.captures(&response) {
                let json_str = caps.get(1).map_or("", |m| m.as_str().trim());
                if let Ok(content) = serde_json::from_str::<office::PdfContent>(json_str) {
                    if let Ok(path) = office::generate_pdf(content).await {
                        let filename = std::path::Path::new(&path).file_name().unwrap_or_default().to_string_lossy();
                        let _ = app.emit("llm-token", format!("OFFICE_GEN_SUCCESS:{}", path));
                        let clean_resp = pdf_re.replace(&response, "*(Document PDF généré)*").into_owned();
                        current_messages.push(serde_json::json!({"role": "assistant", "content": clean_resp}));
                        current_messages.push(serde_json::json!({
                            "role": "system",
                            "content": format!("Success: PDF document ready at {}. Now, briefly inform the user in their language.", filename)
                        }));
                        continue;
                    }
                }
            }

            final_response = ollama::chat_stream(app.clone(), current_messages.clone(), &active_model, false).await?;
            break;
        }

        let _ = app.emit("llm-done", &final_response);

        let sema = vram_queue.semaphore();
        tokio::spawn(async move {
            memory::extract_and_save(user_msg, final_response, sema).await;
        });
    }

    Ok(())
}

fn build_system_prompt(system_base: &str, context: &str) -> String {
    format!(
        "{}
        You have access to a local memory vault and the internet.

        CRITICAL RULES:
        1. NEVER output raw memory markers like '### memory/'. Use the context naturally.
        2. LANGUAGE: Always respond in the SAME LANGUAGE as the user's request.
        3. ACCURACY: Do NOT speculate, invent reviews, ratings, or reception for unreleased media or future events. If a date is in the future, state it is upcoming. Never invent sequels or 'in development' status unless explicitly found in search results.
        4. LOCAL KNOWLEDGE PRIORITY: If the information requested is available in the 'Local Memory Context' section below, use it to answer directly. DO NOT trigger a SEARCH_WEB if you can find the answer locally.
        5. AUTOMATION PROTOCOL: When asked to generate a document (Word, Excel, PowerPoint) or perform a search, output ONLY the required tag (GENERATE_DOCX, GENERATE_PPTX, SEARCH_WEB, SCRAPE_YOUTUBE, or SCRAPE_REDDIT). No preambles, no explanations.
        6. GENERATE_IMAGE: To create an image, start with 'GENERATE_IMAGE:' followed by the prompt.
        7. GENERATE_VIDEO: To create a video, start with 'GENERATE_VIDEO:' followed by the prompt.
        8. SEARCH_WEB — USE THIS PRIORITY ORDER for factual questions about real people, places, or events:
           STEP 1: Check the Local Memory Context below. If the answer is there, use it.
           STEP 2: If not in local memory but you are CONFIDENT the person/entity is well-known and you have reliable knowledge from training (e.g. historical figures, famous athletes, public figures), answer directly from your knowledge.
           STEP 3: If not in local memory AND you are NOT confident (obscure person, recent events, internet personality, etc.), output ONLY 'SEARCH_WEB: <query>'.
           NEVER fabricate biographical details, roles, or facts. If in doubt between steps 2 and 3, always choose step 3.
           AFTER RECEIVING 'WEB SEARCH RESULTS': You MUST write a complete, detailed response using those results. NEVER say you cannot access real-time information — the data is already in front of you. Use it fully.
        9. SCRAPE_YOUTUBE / SCRAPE_REDDIT: To read the full content of a YouTube video or a Reddit post, output ONLY 'SCRAPE_YOUTUBE: <url>' or 'SCRAPE_REDDIT: <url>'.
        10. GENERATE_DOCX: To create a professional Word document, output:
           GENERATE_DOCX: {{
             \"filename\": \"name\",
             \"title\": \"Main Title\",
             \"elements\": [
               {{ \"type\": \"heading\", \"level\": 1, \"text\": \"Title\" }},
               {{ \"type\": \"paragraph\", \"text\": \"Content...\", \"bold\": false, \"italic\": false, \"align\": \"left\" }},
               {{ \"type\": \"metadata\", \"label\": \"Label\", \"value\": \"Value\" }},
               {{ \"type\": \"list\", \"items\": [\"item 1\", \"item 2\"] }}
             ]
           }}
        9. GENERATE_XLSX: To create an Excel file, you MUST output:
           GENERATE_XLSX: {{ \"filename\": \"name\", \"sheets\": [{{ \"name\": \"Sheet1\", \"rows\": [[\"Col1\", \"Col2\"], [\"Val1\", \"Val2\"]] }}] }}
        10. GENERATE_PPTX: To create a PowerPoint presentation, you MUST output:
           GENERATE_PPTX: {{ \"filename\": \"name\", \"title\": \"Main Title\", \"slides\": [{{ \"title\": \"Slide 1\", \"intro\": \"Summary\", \"bullets\": [\"fact 1\", \"fact 2\"] }}] }}
        11. GENERATE_PDF: To create a professional PDF document, you MUST output:
           GENERATE_PDF: {{ \"filename\": \"name\", \"title\": \"Main Title\", \"elements\": [{{ \"type\": \"heading\", \"level\": 1, \"text\": \"Title\" }}, {{ \"type\": \"paragraph\", \"text\": \"Content...\" }}] }}
        12. Your tone should align with your persona but remain professional and creative.

        Local Memory Context:
        ---
        {}
        ---",
        system_base, context
    )
}

fn build_agent_system_prompt(system_base: &str, context: &str) -> String {
    format!(
        "{}\n\nYou are an autonomous agent. Use the provided tools to complete tasks.\n\
        RULES:\n\
        1. LANGUAGE: Always respond in the SAME LANGUAGE as the user's request.\n\
        2. ACCURACY: Do not speculate or fabricate facts.\n\
        3. Use search_web for any question about current events, prices, or real-time data.\n\
        4. Use generate_image to create images when asked.\n\
        5. When done with all tool calls, write a clear natural-language summary.\n\n\
        Local Memory Context:\n---\n{}\n---",
        system_base, context
    )
}

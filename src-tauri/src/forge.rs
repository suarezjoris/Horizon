use tauri::{AppHandle, Emitter};
use serde::{Deserialize, Serialize};
use crate::{ollama, search, settings, tools};

#[derive(Debug, Deserialize, Serialize)]
pub struct ForgeStep {
    pub task: String, // "search_wiki", "search_web", "write_docx", "write_pptx", "write_xlsx"
    pub query: Option<String>,
    pub filename: Option<String>,
    #[serde(default = "default_status")]
    pub status: String, // "pending", "running", "done", "error"
}

fn default_status() -> String {
    "pending".to_string()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ForgePlan {
    pub steps: Vec<ForgeStep>,
}

#[derive(Serialize, Clone)]
struct ForgeProgress {
    pub step_index: usize,
    pub total_steps: usize,
    pub message: String,
    pub done: bool,
}

#[tauri::command]
pub async fn execute_forge(app: AppHandle, goal: String) -> Result<String, String> {
    let s = settings::load();
    let model = &s.llm_model;

    // 1. Planning State
    let _ = app.emit("forge-progress", ForgeProgress {
        step_index: 0,
        total_steps: 0,
        message: "Architecting the plan...".to_string(),
        done: false,
    });

    let planner_prompt = format!(
        "You are the Forge Architect. Break this high-level goal into a JSON array of technical steps.
        
        GOAL: {}

        RULES:
        - Output ONLY a JSON object: {{ \"steps\": [ {{ \"task\": \"...\", \"query\": \"...\" }} ] }}
        - Available tasks: \"search_web\", \"write_docx\", \"write_pptx\", \"write_xlsx\".
        - Be efficient. Group research before writing.

        Example output: {{ \"steps\": [ {{ \"task\": \"search_web\", \"query\": \"Code Lyoko\" }}, {{ \"task\": \"write_docx\", \"filename\": \"report\" }} ] }}",
        goal
    );

    let plan_resp = ollama::chat_once(vec![serde_json::json!({"role": "user", "content": planner_prompt})], model).await?;
    let json_str = plan_resp.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let plan: ForgePlan = serde_json::from_str(json_str).map_err(|e| format!("Planning Error: {}", e))?;

    let total_steps = plan.steps.len();
    let mut context_buffer = String::new();

    // 2a. Research phase — all search_web steps run in parallel
    let research_steps: Vec<_> = plan.steps.iter()
        .filter(|s| s.task == "search_web")
        .cloned()
        .collect();
    let research_count = research_steps.len();

    if research_count > 0 {
        let _ = app.emit("forge-progress", ForgeProgress {
            step_index: 1,
            total_steps,
            message: format!("Researching ({} sources in parallel)...", research_count),
            done: false,
        });

        let handles: Vec<_> = research_steps.into_iter().map(|step| {
            tokio::spawn(async move {
                let q = step.query.unwrap_or_default();
                search::duckduckgo_search(&q).await
                    .map(|res| format!("\n### [Web Search: {}]\n{}\n", q, res))
                    .unwrap_or_default()
            })
        }).collect();

        for handle in futures_util::future::join_all(handles).await {
            if let Ok(text) = handle {
                context_buffer.push_str(&text);
            }
        }
    }

    // 2b. Write phase — sequential, uses accumulated context
    let write_steps: Vec<_> = plan.steps.iter()
        .filter(|s| s.task != "search_web")
        .collect();

    for (i, step) in write_steps.iter().enumerate() {
        let _ = app.emit("forge-progress", ForgeProgress {
            step_index: research_count + i + 1,
            total_steps,
            message: format!("Executing step: {}...", step.task),
            done: false,
        });

        match step.task.as_str() {
            "write_docx" | "write_pptx" | "write_xlsx" => {
                let tool_name = match step.task.as_str() {
                    "write_docx" => "generate_docx",
                    "write_pptx" => "generate_pptx",
                    "write_xlsx" => "generate_xlsx",
                    _            => unreachable!(),
                };

                let schema_hint = match step.task.as_str() {
                    "write_docx" => r#"{"filename":"...","title":"...","elements":[{"type":"heading","level":1,"text":"..."},{"type":"paragraph","text":"..."}]}"#,
                    "write_pptx" => r#"{"filename":"...","title":"...","slides":[{"title":"...","intro":"...","bullets":["..."]}]}"#,
                    "write_xlsx" => r#"{"filename":"...","sheets":[{"name":"Sheet1","rows":[["Col1","Col2"],["Val1","Val2"]]}]}"#,
                    _            => unreachable!(),
                };

                let writer_prompt = format!(
                    "You are the Forge Writer. Generate a {} based on the context.\n\nCONTEXT:\n{}\n\nOutput ONLY valid JSON matching this schema (no prose, no markdown):\n{}",
                    step.task, context_buffer, schema_hint
                );

                let resp = ollama::chat_once(
                    vec![serde_json::json!({"role": "user", "content": writer_prompt})],
                    model,
                ).await?;

                let json_str = {
                    let r = resp.trim();
                    let start = r.find('{').unwrap_or(0);
                    let end = r.rfind('}').map(|i| i + 1).unwrap_or(r.len());
                    r[start..end].to_string()
                };

                let args: serde_json::Value = serde_json::from_str(&json_str)
                    .map_err(|e| format!("Forge JSON parse error for {}: {}", tool_name, e))?;

                let ws_path = settings::load().agent_workspace;
                let workspace = std::path::Path::new(&ws_path);
                let path = tools::execute(tool_name, &args, workspace).await?;
                let _ = app.emit("llm-token", format!("OFFICE_GEN_SUCCESS:{}", path));
            }
            _ => return Err(format!("Unknown task: {}", step.task)),
        }
    }

    let _ = app.emit("forge-progress", ForgeProgress {
        step_index: total_steps,
        total_steps,
        message: "Operation completed successfully.".to_string(),
        done: true,
    });

    Ok("Forge execution complete.".to_string())
}


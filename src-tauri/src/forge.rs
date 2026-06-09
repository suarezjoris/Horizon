use tauri::{AppHandle, Emitter};
use serde::{Deserialize, Serialize};
use crate::{ollama, search, wikipedia, office, settings};

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
        - Available tasks: \"search_wiki\", \"search_web\", \"write_docx\", \"write_pptx\", \"write_xlsx\".
        - Be efficient. Group research before writing.

        Example output: {{ \"steps\": [ {{ \"task\": \"search_wiki\", \"query\": \"Code Lyoko\" }}, {{ \"task\": \"write_docx\", \"filename\": \"report\" }} ] }}",
        goal
    );

    let plan_resp = ollama::chat_once(vec![serde_json::json!({"role": "user", "content": planner_prompt})], model).await?;
    let json_str = plan_resp.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let plan: ForgePlan = serde_json::from_str(json_str).map_err(|e| format!("Planning Error: {}", e))?;

    let total_steps = plan.steps.len();
    let mut context_buffer = String::new();

    // 2. Execution Loop
    for (i, step) in plan.steps.iter().enumerate() {
        let _ = app.emit("forge-progress", ForgeProgress {
            step_index: i + 1,
            total_steps,
            message: format!("Executing step: {}...", step.task),
            done: false,
        });

        match step.task.as_str() {
            "search_wiki" => {
                if let Some(q) = &step.query {
                    if let Some(res) = wikipedia::search_wikipedia(q) {
                        context_buffer.push_str(&format!("\n### [Wikipedia: {}]\n{}\n", q, res));
                    }
                }
            },
            "search_web" => {
                if let Some(q) = &step.query {
                    if let Ok(res) = search::duckduckgo_search(q).await {
                        context_buffer.push_str(&format!("\n### [Web Search: {}]\n{}\n", q, res));
                    }
                }
            },
            "write_docx" | "write_pptx" | "write_xlsx" => {
                let tag_name = match step.task.as_str() {
                    "write_docx" => "GENERATE_DOCX",
                    "write_pptx" => "GENERATE_PPTX",
                    "write_xlsx" => "GENERATE_XLSX",
                    _ => "GENERATE_DOCX",
                };

                let schema_example = match step.task.as_str() {
                    "write_docx" => "{ \"filename\": \"...\", \"title\": \"...\", \"elements\": [ { \"type\": \"heading\", \"level\": 1, \"text\": \"...\" }, { \"type\": \"paragraph\", \"text\": \"...\" } ] }",
                    "write_pptx" => "{ \"filename\": \"...\", \"title\": \"...\", \"slides\": [ { \"title\": \"Slide Title\", \"intro\": \"Summary\", \"bullets\": [\"fact 1\", \"fact 2\"] } ] }",
                    "write_xlsx" => "{ \"filename\": \"...\", \"sheets\": [ { \"name\": \"Sheet1\", \"rows\": [[\"A1\", \"B1\"], [\"Val1\", \"Val2\"]] } ] }",
                    _ => "",
                };

                let writer_prompt = format!(
                    "You are the Lead Forge Writer. Your task is to generate a professional {} based on the context provided.
                    
                    CONTEXT:
                    {}

                    CRITICAL SCHEMA RULES:
                    - You MUST output the full command: {}: {}
                    - Do NOT use 'elements' for PowerPoint; use 'slides'.
                    - Do NOT use 'elements' for Excel; use 'sheets'.
                    - Output ONLY the command. No text before or after.",
                    step.task, context_buffer, tag_name, schema_example
                );

                println!("[Forge] Requesting production for: {}", step.task);
                let writer_resp = ollama::chat_once(vec![serde_json::json!({"role": "user", "content": writer_prompt})], model).await?;
                println!("[Forge] Writer responded (length {}): {}", writer_resp.len(), writer_resp);
                
                if !handle_production_tag(&app, &writer_resp).await? {
                    return Err(format!("The AI failed to generate a valid {} command. It responded: {}", tag_name, writer_resp));
                }
            },
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

async fn handle_production_tag(app: &AppHandle, resp: &str) -> Result<bool, String> {
    let docx_re = regex::Regex::new(r"(?si)GENERATE_DOCX:\s*.*?(\{.*\})").unwrap();
    let xlsx_re = regex::Regex::new(r"(?si)GENERATE_XLSX:\s*.*?(\{.*\})").unwrap();
    let pptx_re = regex::Regex::new(r"(?si)GENERATE_PPTX:\s*.*?(\{.*\})").unwrap();

    if let Some(caps) = docx_re.captures(resp) {
        let json_str = caps.get(1).map_or("", |m| m.as_str().trim());
        let content = serde_json::from_str(json_str).map_err(|e| format!("Word JSON Error: {}", e))?;
        let path = office::generate_docx(content).await?;
        let _ = app.emit("llm-token", format!("OFFICE_GEN_SUCCESS:{}", path));
        return Ok(true);
    } else if let Some(caps) = xlsx_re.captures(resp) {
        let json_str = caps.get(1).map_or("", |m| m.as_str().trim());
        let content = serde_json::from_str(json_str).map_err(|e| format!("Excel JSON Error: {}", e))?;
        let path = office::generate_xlsx(content).await?;
        let _ = app.emit("llm-token", format!("OFFICE_GEN_SUCCESS:{}", path));
        return Ok(true);
    } else if let Some(caps) = pptx_re.captures(resp) {
        let json_str = caps.get(1).map_or("", |m| m.as_str().trim());
        let content = serde_json::from_str(json_str).map_err(|e| format!("PowerPoint JSON Error: {}", e))?;
        let path = office::generate_pptx(content).await?;
        let _ = app.emit("llm-token", format!("OFFICE_GEN_SUCCESS:{}", path));
        return Ok(true);
    }

    Ok(false)
}

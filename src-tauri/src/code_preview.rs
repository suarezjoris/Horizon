use std::path::Path;
use serde::Serialize;
use tauri::AppHandle;
use crate::settings;
use crate::tools;

#[derive(Serialize)]
pub struct CodeResult {
    pub stdout: String,
    pub language: String,
}

fn shell_escape(s: &str) -> String {
    // Basic shell escape: wrap in single quotes and replace single quotes with '\''
    format!("'{}'", s.replace("'", "'\\''"))
}

async fn compile_and_run_rust(code: &str, workspace: &Path) -> Result<String, String> {
    let temp_rs = workspace.join(".tmp_preview.rs");
    let temp_bin = workspace.join(".tmp_preview_bin");
    
    std::fs::write(&temp_rs, code).map_err(|e| format!("Failed to write temp rust file: {}", e))?;
    
    let command = format!(
        "rustc .tmp_preview.rs -o .tmp_preview_bin && ./.tmp_preview_bin"
    );
    
    let output = tools::bash(workspace, &command).await;
    
    let _ = std::fs::remove_file(&temp_rs);
    let _ = std::fs::remove_file(&temp_bin);
    
    output
}

#[tauri::command]
pub async fn execute_code_preview(
    app: AppHandle,
    code: String,
    language: String,
) -> Result<CodeResult, String> {
    let s = settings::load();
    let workspace = s.agent_workspace;
    let ws = Path::new(&workspace);
    
    let command = match language.to_lowercase().as_str() {
        "python" | "python3" => format!("python3 -c {}", shell_escape(&code)),
        "bash" | "sh"        => code.clone(),
        "javascript" | "js" | "node" => format!("node -e {}", shell_escape(&code)),
        "rust"               => return Ok(CodeResult {
            stdout: compile_and_run_rust(&code, ws).await?,
            language,
        }),
        _ => return Err(format!("Unsupported language: {}", language)),
    };
    
    let output = tools::bash(ws, &command).await?;
    Ok(CodeResult { stdout: output, language })
}

use std::fs;
use std::path::Path;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crate::ollama;

#[tauri::command]
pub async fn read_file_content(path: String) -> Result<String, String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("File not found: {}", path));
    }

    let ext = p.extension().unwrap_or_default().to_string_lossy().to_lowercase();

    match ext.as_str() {
        "pdf" => {
            let bytes = fs::read(p).map_err(|e| e.to_string())?;
            let out = pdf_extract::extract_text_from_mem(&bytes).map_err(|e| e.to_string())?;
            Ok(out)
        }
        "docx" | "pptx" | "xlsx" | "xls" => {
            // Use Python bridge for Office files
            let python_path = std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(".venv/bin/python3");
            
            let script_path = std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join("src-tauri/src/office_reader.py");

            let output = std::process::Command::new(python_path)
                .arg(script_path)
                .arg(&path)
                .output()
                .map_err(|e| format!("Failed to run Office reader: {}", e))?;

            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).to_string());
            }
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
        "png" | "jpg" | "jpeg" | "webp" => {
            let bytes = fs::read(p).map_err(|e| e.to_string())?;
            let b64 = BASE64.encode(&bytes);
            let description = ollama::describe_image(&b64).await?;
            Ok(format!("[Image Description (via Moondream)]\n{}", description))
        }
        "json" | "js" | "rs" | "py" | "html" | "css" | "md" | "txt" | "toml" | "csv" | "log" | "sh" | "xml" | "yml" | "yaml" | "ini" => {
            // Read standard text/code files
            fs::read_to_string(p).map_err(|e| e.to_string())
        }
        _ => {
            // Try reading as text anyway as a fallback
            match fs::read_to_string(p) {
                Ok(content) => Ok(content),
                Err(_) => Err(format!("Unsupported file type or not a text file: .{}", ext)),
            }
        }
    }
}

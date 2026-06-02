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
            // Read PDF text
            let bytes = fs::read(p).map_err(|e| e.to_string())?;
            let out = pdf_extract::extract_text_from_mem(&bytes).map_err(|e| e.to_string())?;
            Ok(out)
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

use std::fs;
use std::path::Path;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum FileContent {
    Text(String),
    Image(String), // base64 representation
    Unsupported(String),
}

#[tauri::command]
pub async fn read_file_content(path: String) -> Result<FileContent, String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("File not found: {}", path));
    }

    let ext = p.extension().unwrap_or_default().to_string_lossy().to_lowercase();

    match ext.as_str() {
        "pdf" => {
            let bytes = fs::read(p).map_err(|e| e.to_string())?;
            let out = pdf_extract::extract_text_from_mem(&bytes).map_err(|e| e.to_string())?;
            Ok(FileContent::Text(out))
        }
        "docx" | "pptx" | "xlsx" | "xls" => {
            // Use Python bridge for Office files
            let home = dirs::home_dir().ok_or("Could not find home directory")?;
            let project_root = home.join("Projects/Horizon");
            
            let python_path = project_root.join(".venv/bin/python3");
            let script_path = project_root.join("src-tauri/src/office_reader.py");

            if !python_path.exists() {
                return Err(format!("Python virtualenv not found at {:?}", python_path));
            }

            let output = std::process::Command::new(python_path)
                .arg(script_path)
                .arg(&path)
                .output()
                .map_err(|e| format!("Failed to execute Office reader: {}", e))?;

            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).to_string());
            }
            Ok(FileContent::Text(String::from_utf8_lossy(&output.stdout).to_string()))
        }
        "png" | "jpg" | "jpeg" | "webp" => {
            let bytes = fs::read(p).map_err(|e| e.to_string())?;
            let b64 = BASE64.encode(&bytes);
            Ok(FileContent::Image(b64))
        }
        "json" | "js" | "rs" | "py" | "html" | "css" | "md" | "txt" | "toml" | "csv" | "log" | "sh" | "xml" | "yml" | "yaml" | "ini" => {
            // Read standard text/code files
            let content = fs::read_to_string(p).map_err(|e| e.to_string())?;
            Ok(FileContent::Text(content))
        }
        _ => {
            // Try reading as text anyway as a fallback
            match fs::read_to_string(p) {
                Ok(content) => Ok(FileContent::Text(content)),
                Err(_) => Ok(FileContent::Unsupported(format!("Unsupported file type or not a text file: .{}", ext))),
            }
        }
    }
}

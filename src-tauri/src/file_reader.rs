use std::fs;
use std::io::{Read, Cursor};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum FileContent {
    Text(String),
    Image(String),
    Unsupported(String),
}

#[tauri::command]
pub async fn read_file_content(path: String) -> Result<FileContent, String> {
    let canon = std::fs::canonicalize(&path)
        .map_err(|_| format!("File not found: {}", path))?;
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    if !canon.starts_with(&home) {
        return Err("Access denied: path outside home directory".into());
    }
    let p = canon.as_path();

    let ext = p.extension().unwrap_or_default().to_string_lossy().to_lowercase();

    match ext.as_str() {
        "pdf" => {
            let bytes = fs::read(p).map_err(|e| e.to_string())?;
            let out = pdf_extract::extract_text_from_mem(&bytes).map_err(|e| e.to_string())?;
            Ok(FileContent::Text(out))
        }
        "docx" => {
            let bytes = fs::read(p).map_err(|e| e.to_string())?;
            Ok(FileContent::Text(extract_docx_text(&bytes)?))
        }
        "pptx" => {
            let bytes = fs::read(p).map_err(|e| e.to_string())?;
            Ok(FileContent::Text(extract_pptx_text(&bytes)?))
        }
        "xlsx" | "xls" => {
            let bytes = fs::read(p).map_err(|e| e.to_string())?;
            Ok(FileContent::Text(extract_xlsx_text(&bytes)?))
        }
        "png" | "jpg" | "jpeg" | "webp" => {
            let bytes = fs::read(p).map_err(|e| e.to_string())?;
            Ok(FileContent::Image(BASE64.encode(&bytes)))
        }
        "json" | "js" | "rs" | "py" | "html" | "css" | "md" | "txt"
        | "toml" | "csv" | "log" | "sh" | "xml" | "yml" | "yaml" | "ini" => {
            Ok(FileContent::Text(fs::read_to_string(p).map_err(|e| e.to_string())?))
        }
        _ => match fs::read_to_string(p) {
            Ok(content) => Ok(FileContent::Text(content)),
            Err(_) => Ok(FileContent::Unsupported(format!("Unsupported file type: .{}", ext))),
        },
    }
}


fn extract_docx_text(bytes: &[u8]) -> Result<String, String> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("Not a valid docx: {}", e))?;

    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|_| "Missing word/document.xml".to_string())?
        .read_to_string(&mut xml)
        .map_err(|e| e.to_string())?;

    let para_re = regex::Regex::new(r"<w:p[ >][\s\S]*?</w:p>").unwrap();
    let run_re = regex::Regex::new(r"<w:t[^>]*>([^<]*)</w:t>").unwrap();

    let mut paragraphs = Vec::new();
    for para_cap in para_re.captures_iter(&xml) {
        let para_xml = para_cap.get(0).unwrap().as_str();
        let mut para_text = String::new();
        for run_cap in run_re.captures_iter(para_xml) {
            para_text.push_str(run_cap.get(1).unwrap().as_str());
        }
        let text = para_text.trim().to_string();
        if !text.is_empty() {
            paragraphs.push(text);
        }
    }

    Ok(paragraphs.join("\n"))
}

fn extract_pptx_text(bytes: &[u8]) -> Result<String, String> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("Not a valid pptx: {}", e))?;

    let slide_re = regex::Regex::new(r"^ppt/slides/slide\d+\.xml$").unwrap();
    let mut slide_names: Vec<String> = archive
        .file_names()
        .filter(|n| slide_re.is_match(n))
        .map(|s| s.to_string())
        .collect();
    slide_names.sort();

    let run_re = regex::Regex::new(r"<a:t>([^<]*)</a:t>").unwrap();
    let mut texts = Vec::new();

    for name in slide_names {
        let mut xml = String::new();
        archive
            .by_name(&name)
            .map_err(|e| e.to_string())?
            .read_to_string(&mut xml)
            .map_err(|e| e.to_string())?;

        for cap in run_re.captures_iter(&xml) {
            let text = cap.get(1).unwrap().as_str().trim().to_string();
            if !text.is_empty() {
                texts.push(text);
            }
        }
    }

    Ok(texts.join("\n"))
}

fn extract_xlsx_text(bytes: &[u8]) -> Result<String, String> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("Not a valid xlsx: {}", e))?;

    // Read shared strings table (cell values stored here for string cells)
    let shared_strings: Vec<String> = if let Ok(mut f) = archive.by_name("xl/sharedStrings.xml") {
        let mut xml = String::new();
        f.read_to_string(&mut xml).map_err(|e| e.to_string())?;
        let si_re = regex::Regex::new(r"<si>[\s\S]*?</si>").unwrap();
        let t_re = regex::Regex::new(r"<t[^>]*>([^<]*)</t>").unwrap();
        si_re.captures_iter(&xml)
            .map(|cap| {
                t_re.captures_iter(cap.get(0).unwrap().as_str())
                    .map(|c| c.get(1).unwrap().as_str().to_string())
                    .collect::<String>()
            })
            .collect()
    } else {
        Vec::new()
    };

    let sheet_re = regex::Regex::new(r"^xl/worksheets/sheet\d+\.xml$").unwrap();
    let mut sheet_names: Vec<String> = archive
        .file_names()
        .filter(|n| sheet_re.is_match(n))
        .map(|s| s.to_string())
        .collect();
    sheet_names.sort();

    let c_re = regex::Regex::new(r#"<c [^>]*t="s"[^>]*><v>(\d+)</v>"#).unwrap();
    let cv_re = regex::Regex::new(r"<c [^>]*><v>([^<]+)</v>").unwrap();

    let mut output = Vec::new();

    for (idx, sheet_name) in sheet_names.iter().enumerate() {
        let mut xml = String::new();
        archive
            .by_name(sheet_name)
            .map_err(|e| e.to_string())?
            .read_to_string(&mut xml)
            .map_err(|e| e.to_string())?;

        output.push(format!("--- Sheet {} ---", idx + 1));

        let row_re = regex::Regex::new(r"<row[^>]*>([\s\S]*?)</row>").unwrap();
        for row_cap in row_re.captures_iter(&xml) {
            let row_xml = row_cap.get(1).unwrap().as_str();
            let mut cells = Vec::new();

            for cap in c_re.captures_iter(row_xml) {
                let idx: usize = cap.get(1).unwrap().as_str().parse().unwrap_or(0);
                if let Some(s) = shared_strings.get(idx) {
                    cells.push(s.clone());
                }
            }
            for cap in cv_re.captures_iter(row_xml) {
                cells.push(cap.get(1).unwrap().as_str().to_string());
            }

            if !cells.is_empty() {
                output.push(cells.join(" | "));
            }
        }
    }

    Ok(output.join("\n"))
}

use std::fs;
use std::path::PathBuf;
use chrono::Local;
use serde::{Deserialize, Serialize};
use crate::settings;

#[derive(Serialize, Deserialize)]
pub struct GalleryImage {
    pub path: String,
    pub rel_path: String,
    pub prompt: String,
    pub date: String,
}

#[tauri::command]
pub fn save_generated_image(bytes: Vec<u8>, prompt: String, comfyui_source: Option<String>) -> Result<String, String> {
    let s = settings::load();
    let vault_path = PathBuf::from(&s.vault_path);
    let images_dir = vault_path.join("images");
    
    if !images_dir.exists() {
        fs::create_dir_all(&images_dir).map_err(|e| e.to_string())?;
    }

    let date = Local::now().format("%Y-%m-%d").to_string();
    let timestamp = Local::now().timestamp();
    let slug = prompt
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .take(5)
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase();
    
    let base_name = if slug.is_empty() {
        format!("{}-{}", date, timestamp)
    } else {
        format!("{}-{}-{}", date, slug, timestamp)
    };

    let img_path = images_dir.join(format!("{}.png", base_name));
    let md_path = images_dir.join(format!("{}.md", base_name));

    // Save PNG
    fs::write(&img_path, bytes).map_err(|e| e.to_string())?;

    // Save MD sidecar
    let source_line = comfyui_source
        .map(|p| format!("\n**Source:** {}", p))
        .unwrap_or_default();
    let md_content = format!(
        "# Image Generation\n\n**Prompt:** {}\n**Date:** {}\n**Model:** ComfyUI{}\n\n![Generated Image]({}.png)",
        prompt,
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        source_line,
        base_name
    );
    fs::write(&md_path, md_content).map_err(|e| e.to_string())?;

    Ok(img_path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn list_gallery() -> Result<Vec<GalleryImage>, String> {
    let s = settings::load();
    let images_dir = PathBuf::from(&s.vault_path).join("images");
    
    if !images_dir.exists() {
        return Ok(vec![]);
    }

    let mut gallery = Vec::new();
    if let Ok(entries) = fs::read_dir(images_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "png").unwrap_or(false) {
                let stem = path.file_stem().unwrap().to_string_lossy();
                let md_path = path.with_extension("md");
                
                let mut prompt = String::new();
                if md_path.exists() {
                    if let Ok(content) = fs::read_to_string(md_path) {
                        // Very basic prompt extraction from MD
                        if let Some(line) = content.lines().find(|l| l.contains("**Prompt:**")) {
                            prompt = line.replace("**Prompt:**", "").trim().to_string();
                        }
                    }
                }

                gallery.push(GalleryImage {
                    path: path.to_string_lossy().into_owned(),
                    rel_path: format!("images/{}", path.file_name().unwrap().to_string_lossy()),
                    prompt,
                    date: stem.split('-').take(3).collect::<Vec<_>>().join("-"), // Expecting YYYY-MM-DD
                });
            }
        }
    }

    // Sort by date (newest first) - simple string sort on stem should work well enough if format is consistent
    gallery.sort_by(|a, b| b.path.cmp(&a.path));

    Ok(gallery)
}

#[tauri::command]
pub fn delete_image(path: String) -> Result<(), String> {
    let mut img_path = PathBuf::from(&path);
    
    // Safety check to ensure we only delete from our vault/images folder
    let s = settings::load();
    let mut images_dir = PathBuf::from(&s.vault_path).join("images");
    
    // Canonicalize to resolve any '..' components
    images_dir = images_dir.canonicalize().map_err(|e| format!("Invalid images dir: {}", e))?;
    img_path = img_path.canonicalize().map_err(|e| format!("Invalid image path: {}", e))?;
    
    if !img_path.starts_with(&images_dir) {
        return Err("Unauthorized path deletion".to_string());
    }

    // Delete PNG
    if img_path.exists() {
        fs::remove_file(&img_path).map_err(|e| e.to_string())?;
    }

    // Delete associated MD file and extract ComfyUI source path before removing
    let md_path = img_path.with_extension("md");
    let comfyui_source = if md_path.exists() {
        let content = fs::read_to_string(&md_path).unwrap_or_default();
        content.lines()
            .find(|l| l.starts_with("**Source:**"))
            .map(|l| l.replace("**Source:**", "").trim().to_string())
    } else {
        None
    };
    if md_path.exists() {
        fs::remove_file(&md_path).map_err(|e| e.to_string())?;
    }

    // Clean up the original ComfyUI output file
    if let Some(src) = comfyui_source {
        let src_path = PathBuf::from(&src);
        if src_path.exists() {
            let _ = fs::remove_file(&src_path);
        }
    }

    Ok(())
}

#[tauri::command]
pub fn export_image_to_downloads(path: String) -> Result<String, String> {
    let src = PathBuf::from(&path)
        .canonicalize()
        .map_err(|e| format!("Invalid path: {}", e))?;

    if !src.exists() {
        return Err("Image not found".to_string());
    }

    let filename = src.file_name()
        .ok_or("No filename")?
        .to_string_lossy()
        .into_owned();

    let downloads = dirs::download_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default());
    let dest = downloads.join(&filename);

    // Avoid clobbering: if file exists, append a counter
    let dest = if dest.exists() {
        let stem = src.file_stem().unwrap_or_default().to_string_lossy();
        let ext = src.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let mut i = 1u32;
        loop {
            let candidate = downloads.join(format!("{} ({}){}", stem, i, ext));
            if !candidate.exists() { break candidate; }
            i += 1;
        }
    } else {
        dest
    };

    fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn copy_image_to_clipboard(path: String) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let data = fs::read(&path).map_err(|e| e.to_string())?;

    // Try wl-copy (Wayland)
    if let Ok(mut child) = Command::new("wl-copy")
        .arg("--type").arg("image/png")
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(&data);
        }
        if child.wait().map(|s| s.success()).unwrap_or(false) {
            return Ok(());
        }
    }

    // Fallback: xclip (X11)
    if Command::new("xclip")
        .args(["-selection", "clipboard", "-t", "image/png", "-i", &path])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Ok(());
    }

    Err("No clipboard tool found. Install wl-clipboard (Wayland) or xclip (X11).".to_string())
}

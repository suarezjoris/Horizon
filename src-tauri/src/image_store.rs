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
pub fn save_generated_image(bytes: Vec<u8>, prompt: String) -> Result<String, String> {
    let s = settings::load();
    let vault_path = PathBuf::from(&s.vault_path);
    let images_dir = vault_path.join("images");
    
    if !images_dir.exists() {
        fs::create_dir_all(&images_dir).map_err(|e| e.to_string())?;
    }

    let date = Local::now().format("%Y-%m-%d").to_string();
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
        format!("{}-{}", date, Local::now().timestamp())
    } else {
        format!("{}-{}", date, slug)
    };

    let img_path = images_dir.join(format!("{}.png", base_name));
    let md_path = images_dir.join(format!("{}.md", base_name));

    // Save PNG
    fs::write(&img_path, bytes).map_err(|e| e.to_string())?;

    // Save MD sidecar
    let md_content = format!(
        "# Image Generation\n\n**Prompt:** {}\n**Date:** {}\n**Model:** ComfyUI\n\n![Generated Image]({}.png)",
        prompt,
        Local::now().format("%Y-%m-%d %H:%M:%S"),
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

    // Delete associated MD file
    let md_path = img_path.with_extension("md");
    if md_path.exists() {
        fs::remove_file(&md_path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

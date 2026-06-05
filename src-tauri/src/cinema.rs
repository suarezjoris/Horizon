use serde::{Deserialize, Serialize};
use std::process::Command;
use crate::settings;

#[derive(Serialize, Deserialize, Debug)]
pub struct GpuStats {
    pub load: f32,
    pub memory_used: f32,
    pub memory_total: f32,
}

#[tauri::command]
pub async fn get_gpu_stats() -> Result<GpuStats, String> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=utilization.gpu,memory.used,memory.total", "--format=csv,noheader,nounits"])
        .output()
        .map_err(|e| format!("Failed to execute nvidia-smi: {}", e))?;

    if !output.status.success() {
        return Err("nvidia-smi failed".into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = stdout.trim().split(',').map(|s| s.trim()).collect();
    
    if parts.len() < 3 {
        return Err("Invalid nvidia-smi output".into());
    }

    Ok(GpuStats {
        load: parts[0].parse().unwrap_or(0.0),
        memory_used: parts[1].parse().unwrap_or(0.0),
        memory_total: parts[2].parse().unwrap_or(0.0),
    })
}

#[tauri::command]
pub async fn generate_video(
    prompt: String,
    duration: i32,
    quality: String,
    image_path: Option<String>,
    width: i64,
    height: i64,
    fps: i64,
    seed: Option<i64>,
) -> Result<String, String> {
    let s = settings::load();
    
    // 1. Unload LLM
    let _ = crate::ollama::unload(&s.llm_model).await;

    // 2. Load workflow
    let workflow_name = if image_path.is_some() { "comfyui-i2v-workflow.json" } else { "comfyui-t2v-workflow.json" };
    let home = dirs::home_dir().unwrap_or_default();
    let workflow_path = home.join(format!("Projects/Horizon/assets/{}", workflow_name));

    if !workflow_path.exists() {
        return Err(format!("Video workflow missing at {:?}", workflow_path));
    }

    let content = std::fs::read_to_string(&workflow_path).map_err(|e| e.to_string())?;
    let mut workflow: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    // 3. Inject Parameters into native WanVideoWrapper nodes
    let seed = seed.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let steps = match quality.as_str() {
        "low" => 20,
        "mid" => 30,
        "high" => 50,
        _ => 30,
    };
    let frames = (duration as i64 * fps).clamp(8, 120);

    if let Some(nodes) = workflow.as_object_mut() {
        for (id, node) in nodes.iter_mut() {
            let current_id = id.clone();
            
            // Sampler setup (Node 6)
            if current_id == "6" {
                if let Some(inputs) = node["inputs"].as_object_mut() {
                    inputs["seed"] = serde_json::json!(seed);
                    inputs["steps"] = serde_json::json!(steps);
                }
            }
            
            // Prompt setup (Node 4 = positive CLIPTextEncode)
            if current_id == "4" {
                if let Some(inputs) = node["inputs"].as_object_mut() {
                    inputs["text"] = serde_json::json!(format!("{}, {}, masterpiece, cinematic", s.image_rating, prompt));
                }
            }

            // Dimensions / length (Node 5)
            if current_id == "5" {
                if let Some(inputs) = node["inputs"].as_object_mut() {
                    if inputs.contains_key("num_frames") {
                        inputs["num_frames"] = serde_json::json!(frames);
                    }
                    if inputs.contains_key("width") {
                        inputs["width"] = serde_json::json!(width);
                    }
                    if inputs.contains_key("height") {
                        inputs["height"] = serde_json::json!(height);
                    }
                }
            }

            // Output frame rate (Node 8 = VHS_VideoCombine)
            if current_id == "8" {
                if let Some(inputs) = node["inputs"].as_object_mut() {
                    inputs["frame_rate"] = serde_json::json!(fps);
                }
            }

            // Start image for i2v (Node 11 = VHS_LoadImagePath, expects a path string)
            if current_id == "11" {
                if let Some(p) = image_path.clone() {
                    if let Some(inputs) = node["inputs"].as_object_mut() {
                        inputs["image"] = serde_json::json!(p);
                    }
                }
            }
        }
    }

    // 4. Submit
    let client = reqwest::Client::new();
    let resp = client
        .post("http://127.0.0.1:8188/prompt")
        .json(&serde_json::json!({ "prompt": workflow }))
        .send()
        .await
        .map_err(|e| format!("ComfyUI offline: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        let err_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("ComfyUI Error ({}): {}", status, err_text));
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| format!("Decode error: {}", e))?;
    let prompt_id = body["prompt_id"].as_str().ok_or("No prompt_id returned by ComfyUI")?;

    // 5. Poll for completion. On 12GB the 14B model needs block-swap, and long /
    //    high-step renders can run 1-3h, so poll patiently (5400 * 2s = 3h).
    for _ in 0..5400 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let hr = client.get(format!("http://127.0.0.1:8188/history/{}", prompt_id)).send().await;
        if let Ok(r) = hr {
            let hist: serde_json::Value = r.json().await.unwrap_or_default();
            if !hist[prompt_id].is_null() {
                if let Some(outputs) = hist[prompt_id]["outputs"].as_object() {
                    for node_output in outputs.values() {
                        if let Some(videos) = node_output["gifs"].as_array().or(node_output["videos"].as_array()) {
                            if let Some(vid) = videos.first() {
                                let filename = vid["filename"].as_str().unwrap_or_default();
                                let mut path = std::path::PathBuf::from("/home/joris/Projects/Horizon/ComfyUI/output");
                                path.push(filename);
                                // Render done — unload the model to release RAM/VRAM while idle.
                                let _ = client
                                    .post("http://127.0.0.1:8188/free")
                                    .json(&serde_json::json!({"unload_models": true, "free_memory": true}))
                                    .send()
                                    .await;
                                return Ok(path.to_string_lossy().to_string());
                            }
                        }
                    }
                }
                break;
            }
        }
    }
    Err("Still rendering in ComfyUI (long/high-step videos can take hours on 12 GB VRAM). It will appear in Past Renders when finished.".into())
}

#[derive(Serialize, Deserialize)]
pub struct GalleryVideo {
    pub path: String,
    pub thumb: String,
    pub name: String,
    pub date: String,
}

fn video_output_dir() -> std::path::PathBuf {
    dirs::home_dir().unwrap_or_default().join("Projects/Horizon/ComfyUI/output")
}

#[tauri::command]
pub fn list_videos() -> Result<Vec<GalleryVideo>, String> {
    let dir = video_output_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut videos = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "mp4").unwrap_or(false) {
                let thumb = path.with_extension("png");
                let name = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                let date = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .map(|t| {
                        let dt: chrono::DateTime<chrono::Local> = t.into();
                        dt.format("%Y-%m-%d %H:%M").to_string()
                    })
                    .unwrap_or_default();

                videos.push(GalleryVideo {
                    path: path.to_string_lossy().into_owned(),
                    thumb: if thumb.exists() { thumb.to_string_lossy().into_owned() } else { String::new() },
                    name,
                    date,
                });
            }
        }
    }

    // Newest first (filenames are zero-padded counters, so reverse string sort works)
    videos.sort_by(|a, b| b.path.cmp(&a.path));
    Ok(videos)
}

#[tauri::command]
pub fn delete_video(path: String) -> Result<(), String> {
    let dir = video_output_dir()
        .canonicalize()
        .map_err(|e| format!("Invalid output dir: {}", e))?;
    let vid_path = std::path::PathBuf::from(&path)
        .canonicalize()
        .map_err(|e| format!("Invalid video path: {}", e))?;

    if !vid_path.starts_with(&dir) {
        return Err("Unauthorized path deletion".to_string());
    }

    if vid_path.exists() {
        std::fs::remove_file(&vid_path).map_err(|e| e.to_string())?;
    }
    let thumb = vid_path.with_extension("png");
    if thumb.exists() {
        let _ = std::fs::remove_file(&thumb);
    }
    Ok(())
}

#[tauri::command]
pub async fn open_video(path: String) -> Result<(), String> {
    let dir = video_output_dir()
        .canonicalize()
        .map_err(|e| format!("Invalid output dir: {}", e))?;
    let vid_path = std::path::PathBuf::from(&path)
        .canonicalize()
        .map_err(|e| format!("Invalid video path: {}", e))?;

    if !vid_path.starts_with(&dir) {
        return Err("Unauthorized path".to_string());
    }

    // WebKitGTK can't render <video> reliably on Linux/NVIDIA, so hand the file
    // to the system's default player. `setsid` + null stdio fully detaches the
    // child so it never blocks the Horizon UI.
    Command::new("setsid")
        .arg("xdg-open")
        .arg(&vid_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to launch player: {}", e))?;
    Ok(())
}

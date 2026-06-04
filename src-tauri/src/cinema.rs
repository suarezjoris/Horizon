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
    image_path: Option<String>
) -> Result<String, String> {
    let s = settings::load();
    
    // 1. Unload LLM to free VRAM for video
    let _ = crate::ollama::unload(&s.llm_model).await;

    // 2. Determine workflow (T2V or I2V)
    let workflow_name = if image_path.is_some() { "comfyui-i2v-workflow.json" } else { "comfyui-t2v-workflow.json" };
    let home = dirs::home_dir().unwrap_or_default();
    let workflow_path = home.join(format!("Projects/Horizon/assets/{}", workflow_name));

    if !workflow_path.exists() {
        return Err(format!("Video workflow missing: {:?}", workflow_path));
    }

    let content = std::fs::read_to_string(&workflow_path).map_err(|e| e.to_string())?;
    let mut workflow: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    // 3. Inject Parameters
    let seed = chrono::Utc::now().timestamp_millis() as u64;
    // Map quality to steps/cfg
    let (steps, cfg) = match quality.as_str() {
        "low" => (12, 6.0),
        "mid" => (20, 7.5),
        "high" => (35, 8.0),
        _ => (20, 7.5),
    };

    if let Some(nodes) = workflow.as_object_mut() {
        for (id, node) in nodes.iter_mut() {
            // Randomize seed for any KSampler node
            if node["class_type"] == "KSamplerAdvanced" || node["class_type"] == "KSampler" {
                if let Some(inputs) = node["inputs"].as_object_mut() {
                    inputs["noise_seed"] = serde_json::json!(seed);
                    inputs["steps"] = serde_json::json!(steps);
                    inputs["cfg"] = serde_json::json!(cfg);
                }
            }

            // Positive Prompt (Node 89 for T2V, 93 for I2V)
            if (id == "89" || id == "93") && node["class_type"] == "CLIPTextEncode" {
                if let Some(inputs) = node["inputs"].as_object_mut() {
                    inputs["text"] = serde_json::json!(format!("{}, {}, masterpiece, 4k", s.image_rating, prompt));
                }
            }

            // Image Input & Dimensions (Node 98 for I2V, 74 for T2V)
            if id == "98" && node["class_type"] == "WanImageToVideo" {
                 if let Some(inputs) = node["inputs"].as_object_mut() {
                     if let Some(p) = image_path.clone() {
                        inputs["start_image"] = serde_json::json!(p);
                     }
                 }
            }
            if id == "98" || id == "74" {
                 if let Some(inputs) = node["inputs"].as_object_mut() {
                     // length is frames, 81 frames approx 5s at 16fps
                     let frames = (duration * 16).min(120);
                     inputs["length"] = serde_json::json!(frames);
                 }
            }
        }
    }

    // 4. Submit to ComfyUI (Standard API call)
    let client = reqwest::Client::new();
    let resp = client
        .post("http://127.0.0.1:8188/prompt")
        .json(&serde_json::json!({ "prompt": workflow }))
        .send()
        .await
        .map_err(|e| format!("ComfyUI offline: {}", e))?;

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    if let Some(prompt_id) = body["prompt_id"].as_str() {
        // 5. Poll for completion (Wait for video to be baked)
        println!("Cinema: Production started (ID: {}). Watching the reels...", prompt_id);
        
        for i in 0..1200 { // 20 minute timeout for heavy videos
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            
            let hist_resp = client
                .get(format!("http://127.0.0.1:8188/history/{}", prompt_id))
                .send()
                .await;
            
            if let Ok(hr) = hist_resp {
                let history: serde_json::Value = hr.json().await.unwrap_or_default();
                if !history[prompt_id].is_null() {
                    println!("Cinema: Rendering complete!");
                    // Find the generated file path in outputs
                    if let Some(outputs) = history[prompt_id]["outputs"].as_object() {
                        for node_output in outputs.values() {
                            // Video nodes often output a 'gifs' or 'videos' array
                            if let Some(videos) = node_output["gifs"].as_array().or(node_output["videos"].as_array()) {
                                if let Some(vid) = videos.first() {
                                    let filename = vid["filename"].as_str().unwrap_or_default();
                                    let subfolder = vid["subfolder"].as_str().unwrap_or_default();
                                    
                                    // Construct absolute path
                                    let mut path = std::path::PathBuf::from("/home/joris/Projects/Horizon/ComfyUI/output");
                                    if !subfolder.is_empty() { path.push(subfolder); }
                                    path.push(filename);
                                    
                                    return Ok(path.to_string_lossy().to_string());
                                }
                            }
                        }
                    }
                    break;
                }
            }
            if i % 30 == 0 && i > 0 { println!("Cinema: Directing... ({}s elapsed)", i); }
        }
        Err("Filming timed out. Check ComfyUI logs.".into())
    } else {
        Err(format!("ComfyUI rejected the script: {}", body))
    }
}

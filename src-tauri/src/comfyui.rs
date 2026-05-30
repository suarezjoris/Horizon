use reqwest::Client;
use serde_json::Value;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;
use crate::{ollama, settings};

pub struct ComfyManager {
    pub child: Option<Child>,
}

impl ComfyManager {
    pub fn new() -> Self {
        Self { child: None }
    }
}

#[tauri::command]
pub async fn check_comfyui() -> bool {
    let client = Client::new();
    client.get("http://127.0.0.1:8188/").send().await.is_ok()
}

#[tauri::command]
pub fn spawn_comfyui() -> Result<(), String> {
    let s = settings::load();
    let path = std::path::Path::new(&s.comfyui_path);
    if !path.exists() {
        return Err(format!("ComfyUI not found at {}", s.comfyui_path));
    }

    let parent = path.parent().ok_or("Invalid ComfyUI path")?;
    
    // Check for virtual environment
    let venv_python = parent.join("venv/bin/python3");
    println!("ComfyUI: Checking for venv at {:?}", venv_python);
    
    let python_exe = if venv_python.exists() {
        let exe = venv_python.to_string_lossy().into_owned();
        println!("ComfyUI: Found venv! Using {}", exe);
        exe
    } else {
        println!("ComfyUI: Venv NOT found, falling back to system python3");
        "python3".to_string()
    };

    let log_file = std::fs::File::create(parent.join("comfyui.log")).map_err(|e| e.to_string())?;
    let err_file = log_file.try_clone().map_err(|e| e.to_string())?;

    Command::new(python_exe)
        .arg(path)
        .current_dir(parent)
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(err_file))
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn generate_image(prompt: String) -> Result<Vec<u8>, String> {
    let s = settings::load();
    
    // 1. Unload Ollama to free VRAM
    let _ = ollama::unload(&s.llm_model).await;

    // 2. Load workflow template
    let mut workflow_path = std::env::current_dir()
        .unwrap_or_default()
        .join("assets/comfyui-default-workflow.json");
    
    // If not found, try parent directory (common in tauri dev)
    if !workflow_path.exists() {
        if let Ok(parent) = std::env::current_dir().unwrap_or_default().parent().ok_or("no parent") {
            let alt_path = parent.join("assets/comfyui-default-workflow.json");
            if alt_path.exists() {
                workflow_path = alt_path;
            }
        }
    }

    println!("ComfyUI: Loading workflow from {:?}", workflow_path);
    
    let mut workflow: Value = if workflow_path.exists() {
        let content = std::fs::read_to_string(workflow_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())?
    } else {
        return Err("Workflow template missing at assets/comfyui-default-workflow.json".into());
    };

    // 3. Inject prompt and randomize seed
    let mut found = false;
    let seed = chrono::Utc::now().timestamp_millis() as u64;
    
    if let Some(nodes) = workflow.as_object_mut() {
        for node in nodes.values_mut() {
            // Randomize seed for any KSampler node
            if node["class_type"] == "KSampler" {
                if let Some(inputs) = node["inputs"].as_object_mut() {
                    inputs["seed"] = serde_json::json!(seed);
                }
            }

            if node["class_type"] == "CLIPTextEncode" {
                if let Some(inputs) = node["inputs"].as_object_mut() {
                    // Check if this is the positive prompt node (ours has "masterpiece" in template)
                    if let Some(text) = inputs.get("text").and_then(|v| v.as_str()) {
                        if text.contains("masterpiece") {
                            inputs["text"] = Value::String(format!("score_9, score_8_up, score_7_up, rating_explicit, {}, masterpiece, highly detailed", prompt));
                            found = true;
                        }
                    }
                }
            }
        }
    }
    
    if !found {
        return Err("Could not find CLIPTextEncode node in workflow".into());
    }

    // 4. Submit to ComfyUI
    let client = Client::new();
    let resp = client
        .post("http://127.0.0.1:8188/prompt")
        .json(&serde_json::json!({ "prompt": workflow }))
        .send()
        .await
        .map_err(|e| format!("Failed to queue prompt: {}", e))?;

    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    
    if let Some(prompt_id) = body["prompt_id"].as_str() {
        // 5. Poll for completion
        let mut image_info = None;
        println!("ComfyUI: Prompt queued (ID: {}). Waiting for generation...", prompt_id);
        
        for i in 0..600 { // 10 minute timeout for heavy XL models
            sleep(Duration::from_secs(1)).await;
            
            if i % 30 == 0 && i > 0 {
                println!("ComfyUI: Still waiting... ({}s)", i);
            }
            
            let hist_resp = client
                .get(format!("http://127.0.0.1:8188/history/{}", prompt_id))
                .send()
                .await;
            
            if let Ok(hr) = hist_resp {
                let history: Value = hr.json().await.map_err(|e| e.to_string())?;
                if !history[prompt_id].is_null() {
                    if let Some(outputs) = history[prompt_id]["outputs"].as_object() {
                        for node_output in outputs.values() {
                            if let Some(images) = node_output["images"].as_array() {
                                if let Some(img) = images.first() {
                                    image_info = Some((
                                        img["filename"].as_str().unwrap_or_default().to_string(),
                                        img["subfolder"].as_str().unwrap_or_default().to_string(),
                                        img["type"].as_str().unwrap_or_default().to_string(),
                                    ));
                                    break;
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }

        let (filename, subfolder, img_type) = image_info.ok_or("Generation timed out or failed")?;

        // 6. Download image
        let img_resp = client
            .get("http://127.0.0.1:8188/view")
            .query(&[
                ("filename", filename),
                ("subfolder", subfolder),
                ("type", img_type),
            ])
            .send()
            .await
            .map_err(|e| format!("Failed to download image: {}", e))?;

        let bytes = img_resp.bytes().await.map_err(|e| e.to_string())?;
        Ok(bytes.to_vec())
    } else {
        Err(format!("ComfyUI rejected the prompt: {}", body))
    }
}

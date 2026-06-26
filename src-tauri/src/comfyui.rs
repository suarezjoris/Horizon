use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;
use crate::{ollama, settings};

#[derive(Serialize, Deserialize)]
pub struct GenerateImageResult {
    pub bytes: Vec<u8>,
    pub comfyui_path: String,
}

#[tauri::command]
pub async fn check_comfyui() -> bool {
    let client = Client::new();
    client.get("http://127.0.0.1:8188/").send().await.is_ok()
}

#[tauri::command]
pub fn spawn_comfyui() -> Result<(), String> {
    let s = settings::load();
    
    // Check if ComfyUI is already running on 8188
    let client = std::net::TcpStream::connect_timeout(
        &"127.0.0.1:8188".parse().unwrap(),
        std::time::Duration::from_millis(100)
    );
    if client.is_ok() {
        println!("ComfyUI: Port 8188 active, skipping spawn.");
        return Ok(());
    }

    let path = std::fs::canonicalize(&s.comfyui_path)
        .map_err(|_| format!("Invalid ComfyUI path: {}", s.comfyui_path))?;

    let home = dirs::home_dir().ok_or("Could not find home directory")?;

    // Must be inside home dir and must be a main.py file inside a ComfyUI directory
    if !path.starts_with(&home)
        || path.file_name() != Some(std::ffi::OsStr::new("main.py"))
        || !path.to_string_lossy().contains("ComfyUI")
    {
        return Err("Security Error: ComfyUI path must be a main.py inside a ComfyUI directory under your home.".into());
    }

    let parent = path.parent().ok_or("Invalid ComfyUI parent directory")?;
    
    // Check for virtual environment
    let venv_python = crate::pyenv::venv_python(&parent.join("venv"));
    println!("ComfyUI: Checking for venv at {:?}", venv_python);

    let python_exe = if venv_python.exists() {
        let exe = venv_python.to_string_lossy().into_owned();
        println!("ComfyUI: Found venv! Using {}", exe);
        exe
    } else {
        println!("ComfyUI: Venv NOT found, falling back to system python");
        crate::pyenv::system_python().to_string()
    };

    let log_file = std::fs::File::create(parent.join("comfyui.log")).map_err(|e| e.to_string())?;
    let err_file = log_file.try_clone().map_err(|e| e.to_string())?;

    Command::new(python_exe)
        .arg(&path)
        .current_dir(parent)
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(err_file))
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Unload ComfyUI models and free its RAM/VRAM (keeps the server running).
/// Called when leaving a generation tab so idle sessions don't hold the model.
#[tauri::command]
pub async fn free_comfyui() -> Result<(), String> {
    let client = Client::new();
    let _ = client
        .post("http://127.0.0.1:8188/free")
        .json(&serde_json::json!({"unload_models": true, "free_memory": true}))
        .send()
        .await;
    Ok(())
}

/// Abort the in-progress render and clear the queue (Cancel button).
#[tauri::command]
pub async fn interrupt_comfyui() -> Result<(), String> {
    let client = Client::new();
    let _ = client.post("http://127.0.0.1:8188/interrupt").send().await;
    let _ = client
        .post("http://127.0.0.1:8188/queue")
        .json(&serde_json::json!({"clear": true}))
        .send()
        .await;
    Ok(())
}

async fn upload_image_to_comfyui(client: &Client, path: &str) -> Result<String, String> {
    let data = std::fs::read(path).map_err(|e| format!("Cannot read source image: {}", e))?;
    let filename = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "input.png".to_string());

    let part = reqwest::multipart::Part::bytes(data)
        .file_name(filename)
        .mime_str("image/png")
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .part("image", part)
        .text("type", "input")
        .text("overwrite", "true");

    let resp = client
        .post("http://127.0.0.1:8188/upload/image")
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Upload failed: {}", e))?;

    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    body["name"].as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("ComfyUI upload returned no name: {}", body))
}

async fn upload_base64_to_comfyui(client: &Client, base64: &str, filename: &str) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let b64_data = if let Some(idx) = base64.find(',') {
        &base64[idx + 1..]
    } else {
        base64
    };
    let data = STANDARD.decode(b64_data).map_err(|e| format!("Invalid base64: {}", e))?;

    let part = reqwest::multipart::Part::bytes(data)
        .file_name(filename.to_string())
        .mime_str("image/png")
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .part("image", part)
        .text("type", "input")
        .text("overwrite", "true");

    let resp = client
        .post("http://127.0.0.1:8188/upload/image")
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Upload failed: {}", e))?;

    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    body["name"].as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("ComfyUI upload returned no name: {}", body))
}

#[tauri::command]
pub async fn generate_image(
    vram_queue: tauri::State<'_, crate::vram_queue::VramQueue>,
    prompt: String,
    engine: String,
    image_path: Option<String>,
    strength: Option<f32>,
) -> Result<GenerateImageResult, String> {
    let _permit = vram_queue.acquire("ComfyUI Image").await?;
    let s = settings::load();

    let is_i2i = image_path.is_some();

    // 1. Unload Ollama to free VRAM
    ollama::unload().await?;

    // 2. Load workflow template
    let workflow_file = match (engine.as_str(), is_i2i) {
        ("flux", true)  => "comfyui-flux-i2i-workflow.json",
        ("flux", false) => "comfyui-flux-workflow.json",
        (_,      true)  => "comfyui-pony-i2i-workflow.json",
        (_,      false) => "comfyui-default-workflow.json",
    };

    let current_dir = std::env::current_dir().unwrap_or_default();
    let mut paths_to_try = vec![
        current_dir.join(format!("assets/{}", workflow_file)),
        current_dir.parent().map(|p| p.join(format!("assets/{}", workflow_file))).unwrap_or_default(),
    ];

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            paths_to_try.push(exe_dir.join(format!("assets/{}", workflow_file)));
            if let Some(parent) = exe_dir.parent() {
                if let Some(grandparent) = parent.parent() {
                    paths_to_try.push(grandparent.join(format!("assets/{}", workflow_file)));
                }
            }
        }
    }
    
    if let Some(home) = dirs::home_dir() {
        paths_to_try.push(home.join(format!("Projects/Horizon/assets/{}", workflow_file)));
        paths_to_try.push(home.join(format!("Projects/Horizon/src-tauri/assets/{}", workflow_file)));
    }

    let mut workflow_path = paths_to_try[0].clone();
    let mut found = false;
    for path in paths_to_try {
        if path.exists() {
            workflow_path = path;
            found = true;
            break;
        }
    }

    println!("ComfyUI: Loading workflow from {:?}", workflow_path);
    
    let mut workflow: Value = if found {
        let content = std::fs::read_to_string(&workflow_path).map_err(|e| format!("{}: {:?}", e, workflow_path))?;
        serde_json::from_str(&content).map_err(|e| e.to_string())?
    } else {
        return Err(format!("Workflow template missing at assets/{}. Tried: {:?}", workflow_file, workflow_path).into());
    };

    // 3. Upload source image if img2img, get ComfyUI filename
    let client = Client::new();
    let uploaded_name = if let Some(ref path) = image_path {
        Some(upload_image_to_comfyui(&client, path).await?)
    } else {
        None
    };

    // 4. Inject prompt, seed, denoise, and source image name
    let mut found = false;
    let seed = chrono::Utc::now().timestamp_millis() as u64;
    let denoise = strength.unwrap_or(0.75).clamp(0.05, 1.0);

    if let Some(nodes) = workflow.as_object_mut() {
        for node in nodes.values_mut() {
            if let Some(inputs) = node["inputs"].as_object_mut() {
                if inputs.contains_key("seed") {
                    inputs["seed"] = serde_json::json!(seed);
                }
                if inputs.contains_key("noise_seed") {
                    inputs["noise_seed"] = serde_json::json!(seed);
                }
                // Inject denoise for img2img
                if is_i2i && inputs.contains_key("denoise") {
                    inputs["denoise"] = serde_json::json!(denoise);
                }
                // Replace placeholder with uploaded filename in LoadImage node
                if let Some(img_name) = &uploaded_name {
                    if let Some(Value::String(s)) = inputs.get("image") {
                        if s == "HORIZON_INPUT_IMAGE" {
                            inputs["image"] = Value::String(img_name.clone());
                        }
                    }
                }
            }

            if node["class_type"] == "CLIPTextEncode" {
                if let Some(inputs) = node["inputs"].as_object_mut() {
                    if let Some(text) = inputs.get("text").and_then(|v| v.as_str()) {
                        if text.contains("masterpiece") {
                            if engine == "flux" {
                                inputs["text"] = Value::String(format!("{}, cinematic, highly detailed, masterpiece", prompt));
                            } else {
                                inputs["text"] = Value::String(format!("score_9, score_8_up, score_7_up, {}, {}, masterpiece, highly detailed", s.image_rating, prompt));
                            }
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

    // 5. Submit to ComfyUI
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
                ("filename", &filename),
                ("subfolder", &subfolder),
                ("type", &img_type),
            ])
            .send()
            .await
            .map_err(|e| format!("Failed to download image: {}", e))?;

        let bytes = img_resp.bytes().await.map_err(|e| e.to_string())?;

        let s = settings::load();
        let comfyui_output = std::path::PathBuf::from(&s.comfyui_path)
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("output");
        let sub = if subfolder.is_empty() { comfyui_output.clone() } else { comfyui_output.join(&subfolder) };
        let comfyui_path = sub.join(&filename).to_string_lossy().into_owned();

        Ok(GenerateImageResult { bytes: bytes.to_vec(), comfyui_path })
    } else {
        Err(format!("ComfyUI rejected the prompt: {}", body))
    }
}

#[tauri::command]
pub async fn generate_inpainting(
    vram_queue: tauri::State<'_, crate::vram_queue::VramQueue>,
    image_path: String,
    mask_base64: String,
    prompt: String,
    negative: Option<String>,
) -> Result<GenerateImageResult, String> {
    let _permit = vram_queue.acquire("ComfyUI Inpaint").await?;
    let s = settings::load();

    // 1. Unload Ollama
    ollama::unload().await?;

    // 2. Load workflow template
    let workflow_file = "comfyui-inpaint-workflow.json";

    let current_dir = std::env::current_dir().unwrap_or_default();
    let mut paths_to_try = vec![
        current_dir.join(format!("assets/{}", workflow_file)),
        current_dir.parent().map(|p| p.join(format!("assets/{}", workflow_file))).unwrap_or_default(),
    ];

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            paths_to_try.push(exe_dir.join(format!("assets/{}", workflow_file)));
            if let Some(parent) = exe_dir.parent() {
                if let Some(grandparent) = parent.parent() {
                    paths_to_try.push(grandparent.join(format!("assets/{}", workflow_file)));
                }
            }
        }
    }
    
    if let Some(home) = dirs::home_dir() {
        paths_to_try.push(home.join(format!("Projects/Horizon/assets/{}", workflow_file)));
        paths_to_try.push(home.join(format!("Projects/Horizon/src-tauri/assets/{}", workflow_file)));
    }

    let mut workflow_path = paths_to_try[0].clone();
    let mut found = false;
    for path in paths_to_try {
        if path.exists() {
            workflow_path = path;
            found = true;
            break;
        }
    }

    println!("ComfyUI: Loading inpaint workflow from {:?}", workflow_path);
    
    let mut workflow: Value = if found {
        let content = std::fs::read_to_string(&workflow_path).map_err(|e| format!("{}: {:?}", e, workflow_path))?;
        serde_json::from_str(&content).map_err(|e| e.to_string())?
    } else {
        return Err(format!("Workflow template missing at assets/{}. Tried: {:?}", workflow_file, workflow_path).into());
    };

    let client = Client::new();
    
    // 3. Upload source image and mask
    let uploaded_image_name = upload_image_to_comfyui(&client, &image_path).await?;
    let uploaded_mask_name = upload_base64_to_comfyui(&client, &mask_base64, "horizon_mask.png").await?;

    // 4. Inject prompt, etc.
    let seed = chrono::Utc::now().timestamp_millis() as u64;

    if let Some(nodes) = workflow.as_object_mut() {
        for node in nodes.values_mut() {
            if let Some(inputs) = node["inputs"].as_object_mut() {
                if inputs.contains_key("seed") {
                    inputs["seed"] = serde_json::json!(seed);
                }
                if inputs.contains_key("noise_seed") {
                    inputs["noise_seed"] = serde_json::json!(seed);
                }
                if let Some(Value::String(s)) = inputs.get("image") {
                    if s == "HORIZON_INPUT_IMAGE" {
                        inputs["image"] = Value::String(uploaded_image_name.clone());
                    } else if s == "HORIZON_MASK_IMAGE" {
                        inputs["image"] = Value::String(uploaded_mask_name.clone());
                    }
                }
            }

            if node["class_type"] == "CLIPTextEncode" {
                if let Some(inputs) = node["inputs"].as_object_mut() {
                    if let Some(text) = inputs.get("text").and_then(|v| v.as_str()) {
                        if text.contains("masterpiece") {
                            inputs["text"] = Value::String(format!("score_9, score_8_up, score_7_up, {}, {}, masterpiece, highly detailed", s.image_rating, prompt));
                        } else if text.contains("nsfw") {
                            let neg = negative.clone().unwrap_or_default();
                            inputs["text"] = Value::String(format!("score_4, score_5, score_6, source_pony, source_cartoon, rating_explicit, rating_questionable, nsfw, nude, naked, bad anatomy, bad proportions, deformed, ugly, bad quality, blurry, watermark, extra limbs, missing limbs, mutated, {}", neg));
                        }
                    }
                }
            }
        }
    }

    // 5. Submit to ComfyUI
    let resp = client
        .post("http://127.0.0.1:8188/prompt")
        .json(&serde_json::json!({ "prompt": workflow }))
        .send()
        .await
        .map_err(|e| format!("Failed to queue prompt: {}", e))?;

    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    
    if let Some(prompt_id) = body["prompt_id"].as_str() {
        let mut image_info = None;
        println!("ComfyUI: Inpaint prompt queued (ID: {}). Waiting for generation...", prompt_id);
        
        for i in 0..600 {
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
                ("filename", &filename),
                ("subfolder", &subfolder),
                ("type", &img_type),
            ])
            .send()
            .await
            .map_err(|e| format!("Failed to download image: {}", e))?;

        let bytes = img_resp.bytes().await.map_err(|e| e.to_string())?;

        let comfyui_output = std::path::PathBuf::from(&s.comfyui_path)
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("output");
        let sub = if subfolder.is_empty() { comfyui_output.clone() } else { comfyui_output.join(&subfolder) };
        let comfyui_path = sub.join(&filename).to_string_lossy().into_owned();

        Ok(GenerateImageResult { bytes: bytes.to_vec(), comfyui_path })
    } else {
        Err(format!("ComfyUI rejected the prompt: {}", body))
    }
}

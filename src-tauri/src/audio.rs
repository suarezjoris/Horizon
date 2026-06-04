use std::process::Command;
use tauri::AppHandle;
use std::io::Write;

#[tauri::command]
pub async fn save_audio_temp(_app: AppHandle, base64_data: String) -> Result<String, String> {
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.decode(base64_data)
        .map_err(|e| format!("Base64 decode error: {}", e))?;
    
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join(format!("horizon_audio_{}.webm", chrono::Local::now().timestamp()));
    
    let mut file = std::fs::File::create(&file_path)
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    file.write_all(&data).map_err(|e| format!("Failed to write audio data: {}", e))?;
    
    Ok(file_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn transcribe_audio(audio_path: String) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let venv_python = crate::pyenv::venv_python(&home.join("Projects/Horizon/.venv"));
    let script_path = home.join("Projects/Horizon/transcribe.py");

    let output = Command::new(venv_python)
        .arg(script_path)
        .arg(audio_path)
        .output()
        .map_err(|e| format!("Failed to run transcription: {}", e))?;

    if !output.status.success() {
        eprintln!("transcription stderr: {}", String::from_utf8_lossy(&output.stderr));
        return Err("Audio transcription failed".into());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

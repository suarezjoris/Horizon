use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tauri::{AppHandle, Emitter, State};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct OpenClaudeState {
    pub child: Arc<Mutex<Option<Child>>>,
}

#[tauri::command]
pub async fn start_openclaude(
    app: AppHandle,
    state: State<'_, OpenClaudeState>,
    project_path: String,
) -> Result<(), String> {
    let mut child_guard = state.child.lock().await;
    
    if let Some(mut old_child) = child_guard.take() {
        let _ = old_child.kill().await;
    }

    let aider_path = "/home/joris/.local/bin/aider";

    let python_wrapper = format!(r#"
import pty
import sys
import os

os.environ['OLLAMA_API_BASE'] = 'http://localhost:11434'
os.environ['TERM'] = 'xterm-256color'

# On désactive Playwright qui pose problème sur Arch
# Aider utilisera des méthodes alternatives plus stables pour le web
cmd = [
    "{}", 
    "--model", "ollama/qwen2.5-coder:14b",
    "--no-git",
    "--dark-mode",
    "--no-suggest-shell-commands",
    "--disable-playwright" 
]

pty.spawn(cmd)
"#, aider_path);

    let mut child = Command::new("python3")
        .args(["-c", &python_wrapper])
        .current_dir(&project_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn Aider: {}", e))?;

    let mut stdout = child.stdout.take().ok_or("Failed to open stdout")?;
    let mut stderr = child.stderr.take().ok_or("Failed to open stderr")?;

    let app_clone = app.clone();
    tokio::spawn(async move {
        let mut buffer = [0; 4096];
        while let Ok(n) = stdout.read(&mut buffer).await {
            if n == 0 { break; }
            let raw_data = String::from_utf8_lossy(&buffer[..n]).to_string();
            let _ = app_clone.emit("openclaude-raw", raw_data);
        }
    });

    let app_clone2 = app.clone();
    tokio::spawn(async move {
        let mut buffer = [0; 4096];
        while let Ok(n) = stderr.read(&mut buffer).await {
            if n == 0 { break; }
            let raw_data = String::from_utf8_lossy(&buffer[..n]).to_string();
            let _ = app_clone2.emit("openclaude-raw", raw_data);
        }
    });

    *child_guard = Some(child);
    Ok(())
}

#[tauri::command]
pub async fn send_openclaude_raw(
    state: State<'_, OpenClaudeState>,
    data: String,
) -> Result<(), String> {
    let mut child_guard = state.child.lock().await;
    if let Some(child) = child_guard.as_mut() {
        if let Some(stdin) = child.stdin.as_mut() {
            let processed_data = data.replace("\n", "\r\n");
            stdin.write_all(processed_data.as_bytes()).await
                .map_err(|e| format!("Failed to write: {}", e))?;
            stdin.flush().await.map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Stdin unavailable".to_string())
        }
    } else {
        Err("Process not running".to_string())
    }
}

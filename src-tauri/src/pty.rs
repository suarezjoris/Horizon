use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

pub struct PtyState {
    pub writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    pub master: Arc<Mutex<Option<Box<dyn portable_pty::MasterPty + Send>>>>,
}

impl Default for PtyState {
    fn default() -> Self {
        Self {
            writer: Arc::new(Mutex::new(None)),
            master: Arc::new(Mutex::new(None)),
        }
    }
}

#[tauri::command]
pub async fn spawn_pty(app: AppHandle, state: State<'_, PtyState>) -> Result<(), String> {
    // Prevent spawning multiple PTYs if already spawned
    {
        let guard = state.writer.lock().await;
        if guard.is_some() {
            return Ok(());
        }
    }

    let pty_system = NativePtySystem::default();

    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    let mut cmd = CommandBuilder::new("bash");
    // Start bash directly in the project
    cmd.cwd(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")));
    cmd.env("TERM", "xterm-256color");

    let _child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    {
        let mut state_writer = state.writer.lock().await;
        *state_writer = Some(writer);
        let mut state_master = state.master.lock().await;
        *state_master = Some(pair.master);
    }

    let app_clone = app.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[0..n]).to_string();
                    let _ = app_clone.emit("pty-read", data);
                }
                Err(_) => break,
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn pty_write(state: State<'_, PtyState>, data: String) -> Result<(), String> {
    let mut writer_guard = state.writer.lock().await;
    if let Some(writer) = writer_guard.as_mut() {
        writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn pty_resize(state: State<'_, PtyState>, rows: u16, cols: u16) -> Result<(), String> {
    let master_guard = state.master.lock().await;
    if let Some(master) = master_guard.as_ref() {
        master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        }).map_err(|e| e.to_string())?;
    }
    Ok(())
}

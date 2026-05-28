#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ollama;
mod settings;

#[tauri::command]
async fn chat(
    app: tauri::AppHandle,
    messages: Vec<serde_json::Value>,
) -> Result<(), String> {
    let s = settings::load();
    ollama::chat_stream(app, messages, &s.llm_model).await?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            settings::get_settings,
            settings::save_settings,
            chat,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

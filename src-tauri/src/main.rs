#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            settings::get_settings,
            settings::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod antenna;
mod app_state;
mod archivist;
mod armata;
mod audio;
mod chat;

mod cinema;
mod comfyui;
mod commands;
mod daemon_manager;
mod embeddings;
mod file_reader;
mod forge_daemon;
mod graphify;
mod image_store;
mod memory;
mod office;
mod ollama;
mod openclaude;

mod pyenv;
mod search;
mod settings;
mod sys_diagnostic;
mod tools;
mod vanguard;
mod vault;
mod vram_queue;
mod wiki_daemon;
mod wikipedia;
mod code_preview;
mod md_converter;
mod plugins;
mod metrics;

fn main() {
    // WebKitGTK on Linux/NVIDIA stalls repaints (the UI only updates on window
    // events, scrolling lags) with the DMABUF renderer. Disable it before the
    // webview initializes for a smooth UI. Linux-only; does not affect Windows.
    #[cfg(target_os = "linux")]
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    tauri::Builder::default()
        .manage(app_state::ArmataState::new())
        .manage(vram_queue::VramQueue::new())
        .manage(metrics::MetricsState::new())
        .manage({
            let settings = settings::load();
            let mut registry = plugins::PluginRegistry::new();
            registry.scan_and_load(&settings.vault_path);
            std::sync::Arc::new(tokio::sync::RwLock::new(registry))
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            daemon_manager::auto_start_daemons(app);
            metrics::spawn_metrics_loop(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            settings::get_settings,
            settings::save_settings,
            chat::chat,
            commands::reset_system,
            commands::list_ollama_models,
            commands::probe_model_capabilities,
            commands::list_personas,
            commands::open_docs_folder,
            wikipedia::ingest_wikipedia,
            office::generate_docx,
            office::generate_xlsx,
            office::generate_pptx,
            memory::process_calibration,
            vault::list_notes,
            vault::read_note,
            vault::write_note,
            memory::consolidate_vault,
            memory::save_to_note,
            memory::confirm_hub_proposal,
            memory::vault_topic_status,
            memory::trigger_hub_proposal,
            graphify::run_graphify,
            embeddings::reindex,
            commands::search_vault,
            commands::get_note_decay_stats,
            file_reader::read_file_content,
            comfyui::check_comfyui,
            comfyui::spawn_comfyui,
            comfyui::free_comfyui,
            comfyui::interrupt_comfyui,
            comfyui::generate_image,
            comfyui::generate_inpainting,
            image_store::save_generated_image,
            image_store::list_gallery,
            image_store::delete_image,
            image_store::export_image_to_downloads,
            image_store::copy_image_to_clipboard,
            cinema::get_gpu_stats,
            cinema::generate_video,
            cinema::list_videos,
            cinema::delete_video,
            cinema::open_video,
            audio::save_audio_temp,
            audio::transcribe_audio,
            sys_diagnostic::run_diagnostics,
            sys_diagnostic::fix_health_issue,
            openclaude::start_openclaude,
            openclaude::send_openclaude_raw,
            armata::execute_armata_command,
            armata::toggle_agent,
            armata::get_armata_status,
            metrics::get_daemon_metrics,
            daemon_manager::toggle_agent_daemon,

            code_preview::execute_code_preview,
            md_converter::export_note_as_docx,
            md_converter::export_vault_as_docx,
            commands::export_chat_as_pdf,
            plugins::list_ui_plugins,
            plugins::get_plugin_html,
            plugins::reload_plugins,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

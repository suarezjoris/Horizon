#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod antenna;
mod app_state;
mod archivist;
mod armata;
mod audio;
mod chat;
mod cinema;
mod code_preview;
mod comfyui;
mod commands;
mod curiosity;
mod daemon_manager;
mod embeddings;
mod file_reader;
mod forge_daemon;
mod graphify;
mod ide_agent;
mod image_store;
mod mcp;
mod md_converter;
mod memory;
mod metrics;
mod office;
mod ollama;
mod plugins;
mod pptx_native;
mod pty;
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

fn main() {
    #[cfg(target_os = "linux")]
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--dump-tools".to_string()) {
        let plugins = plugins::PluginRegistry::new();
        let tools = tools::build_tool_definitions(false, &plugins);
        println!("TOOLS_DUMP_START\n{}\nTOOLS_DUMP_END", serde_json::to_string_pretty(&tools).unwrap());
        std::process::exit(0);
    }

    tauri::Builder::default()
        .manage(app_state::ArmataState::new())
        .manage(vram_queue::VramQueue::new())
        .manage(metrics::MetricsState::new())
        .manage(ide_agent::IdeState::default())
        .manage(pty::PtyState::default())
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
            commands::reset_system,
            commands::auto_consolidate_chat,
            chat::chat,
            commands::list_ollama_models,
            commands::probe_model_capabilities,
            commands::list_personas,
            commands::open_docs_folder,
            wikipedia::ingest_wikipedia,
            office::generate_docx,
            office::generate_xlsx,
            office::generate_pptx,
            pptx_native::analyze_pptx_request,
            pptx_native::scrape_pptx_templates,
            pptx_native::execute_pptx_generation,
            memory::process_calibration,
            vault::list_notes,
            vault::read_note,
            vault::write_note,
            memory::consolidate_vault,
            memory::save_to_note,
            memory::vault_topic_status,
            curiosity::curiosity_next_question,
            curiosity::curiosity_mark_answered,
            curiosity::curiosity_propose_topic,
            curiosity::curiosity_fill_topic,
            curiosity::curiosity_dismiss_topic,
            graphify::run_graphify,
            embeddings::reindex,
            commands::search_vault,
            mcp::get_mcp_store,
            mcp::toggle_mcp_server,
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
            ide_agent::send_ide_prompt,
            ide_agent::clear_ide_memory,
            ide_agent::execute_ide_script,
            pty::spawn_pty,
            pty::pty_write,
            pty::pty_resize,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

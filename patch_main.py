import re

with open('src-tauri/src/main.rs', 'r') as f:
    content = f.read()

# Add manage plugin state
manage_pattern = r'(\.manage\(metrics::MetricsState::new\(\)\))'
manage_replacement = r'\1\n        .manage({\n            let settings = settings::load();\n            let mut registry = plugins::PluginRegistry::new();\n            registry.scan_and_load(&settings.vault_path);\n            std::sync::Arc::new(tokio::sync::RwLock::new(registry))\n        })'
content = re.sub(manage_pattern, manage_replacement, content)

# Add invoke handlers
handler_pattern = r'(commands::export_chat_as_pdf,)'
handler_replacement = r'\1\n            plugins::list_ui_plugins,\n            plugins::get_plugin_html,\n            plugins::reload_plugins,'
content = re.sub(handler_pattern, handler_replacement, content)

with open('src-tauri/src/main.rs', 'w') as f:
    f.write(content)


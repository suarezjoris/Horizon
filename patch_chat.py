import re

with open('src-tauri/src/chat.rs', 'r') as f:
    content = f.read()

# Fix the invalid `\'_` which should be `'_`
content = content.replace("plugin_state: tauri::State<\\\'_, crate::plugins::PluginState>", "plugin_state: tauri::State<'_, crate::plugins::PluginState>")

# The tool_defs and execute calls didn't get patched because the regex probably didn't match. 
# Let's replace manually.
content = content.replace("let tool_defs = tools::build_tool_definitions(include_bash);", """let plugin_registry = plugin_state.read().await;
        let tool_defs = tools::build_tool_definitions(include_bash, &plugin_registry);""")

content = content.replace("""let result = tools::execute(
                        &tc.function.name,
                        &tc.function.arguments,
                        workspace,
                    ).await;""", """let result = tools::execute(
                        &tc.function.name,
                        &tc.function.arguments,
                        workspace,
                        &plugin_registry
                    ).await;""")

with open('src-tauri/src/chat.rs', 'w') as f:
    f.write(content)

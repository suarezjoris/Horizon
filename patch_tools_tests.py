import re

with open('src-tauri/src/tools/mod.rs', 'r') as f:
    content = f.read()

content = content.replace('let tools = build_tool_definitions(false);', 'let plugins = crate::plugins::PluginRegistry::new();\n        let tools = build_tool_definitions(false, &plugins);')
content = content.replace('let tools = build_tool_definitions(true);', 'let plugins = crate::plugins::PluginRegistry::new();\n        let tools = build_tool_definitions(true, &plugins);')

with open('src-tauri/src/tools/mod.rs', 'w') as f:
    f.write(content)

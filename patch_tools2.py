import re

with open('src-tauri/src/tools/mod.rs', 'r') as f:
    content = f.read()

content = content.replace('''    if !include_bash {
        tools.retain(|t| t["function"]["name"] != "bash");
    }

    tools''', '''    if !include_bash {
        tools.retain(|t| t["function"]["name"] != "bash");
    }
    
    tools.extend(plugins.tool_definitions());

    tools''')

with open('src-tauri/src/tools/mod.rs', 'w') as f:
    f.write(content)

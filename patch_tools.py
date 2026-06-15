import re

with open('src-tauri/src/tools/mod.rs', 'r') as f:
    content = f.read()

# build_tool_definitions
btd_pattern = r'pub fn build_tool_definitions\(include_bash: bool\) -> Vec<serde_json::Value> \{'
btd_replacement = r'pub fn build_tool_definitions(include_bash: bool, plugins: &crate::plugins::PluginRegistry) -> Vec<serde_json::Value> {'
content = re.sub(btd_pattern, btd_replacement, content)

# push plugin tools before returning tools in build_tool_definitions
# search for '    tools\n}' at the end of build_tool_definitions.
# Well, it's safer to just inject it. The return is `tools`. We can find `    tools\n}` and replace with `    tools.extend(plugins.tool_definitions());\n    tools\n}`
# Wait, let's just find `    tools\n}` that is at the end of the `build_tool_definitions` block.
import ast
# Let's do it with regex
end_btd_pattern = r'(        \}\),\n    \];\n\n    if !include_bash \{\n        tools\.retain\(\|t\| t\["function"\]\["name"\] \!= "bash"\);\n    \}\n\n    tools\n\})'
end_btd_replacement = r'        }),\n    ];\n\n    if !include_bash {\n        tools.retain(|t| t["function"]["name"] != "bash");\n    }\n\n    tools.extend(plugins.tool_definitions());\n\n    tools\n}'
content = re.sub(end_btd_pattern, end_btd_replacement, content)

# execute
execute_pattern = r'pub async fn execute\(\n    name: &str,\n    args: &serde_json::Value,\n    workspace: &Path,\n\) -> Result<String, String> \{'
execute_replacement = r'pub async fn execute(\n    name: &str,\n    args: &serde_json::Value,\n    workspace: &Path,\n    plugins: &crate::plugins::PluginRegistry\n) -> Result<String, String> {'
content = re.sub(execute_pattern, execute_replacement, content)

# fallback for execute
fallback_pattern = r'(_ => Err\(format!\("Unknown tool: \{\}", name\)\),\n    \})'
fallback_replacement = r'_ => plugins.execute_tool(name, args).await,\n    }'
content = re.sub(fallback_pattern, fallback_replacement, content)

with open('src-tauri/src/tools/mod.rs', 'w') as f:
    f.write(content)

import re

with open('src-tauri/src/plugins.rs', 'r') as f:
    content = f.read()

# add kill_on_drop
content = content.replace('.stderr(std::process::Stdio::piped())', '.stderr(std::process::Stdio::piped())\n            .kill_on_drop(true)')

# remove manual child kill
content = content.replace('''            Err(_) => {
                let _ = child.kill().await;
                return Err(format!("Plugin execution timed out after {} seconds", manifest.timeout_seconds));
            }''', '''            Err(_) => {
                return Err(format!("Plugin execution timed out after {} seconds", manifest.timeout_seconds));
            }''')

with open('src-tauri/src/plugins.rs', 'w') as f:
    f.write(content)

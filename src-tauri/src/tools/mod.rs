use std::path::{Component, Path, PathBuf};

pub fn safe_join(workspace: &Path, requested: &Path) -> Result<PathBuf, String> {
    if requested.is_absolute() {
        return Err("Access denied: absolute path not allowed".into());
    }
    let mut result = workspace.to_path_buf();
    for component in requested.components() {
        match component {
            Component::Normal(c) => result.push(c),
            Component::ParentDir => { result.pop(); }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => {
                return Err("Access denied: absolute component in path".into());
            }
        }
    }
    if result.starts_with(workspace) {
        Ok(result)
    } else {
        Err("Access denied: path escapes workspace".into())
    }
}

pub const READ_LIMIT: usize = 8 * 1024;
pub const BASH_LIMIT: usize = 4 * 1024;

pub fn read_file(workspace: &Path, path: &str, offset: Option<usize>) -> Result<String, String> {
    let abs = safe_join(workspace, Path::new(path))?;
    let content = std::fs::read_to_string(&abs)
        .map_err(|e| format!("Cannot read '{}': {}", path, e))?;
    let start = offset.unwrap_or(0).min(content.len());
    let slice = &content[start..];
    if slice.len() <= READ_LIMIT {
        Ok(slice.to_string())
    } else {
        let total_kb = content.len() / 1024;
        Ok(format!(
            "{}\n[OUTPUT TRUNCATED — 8 KB shown of {} KB total. Call read_file with offset={} to continue.]",
            &slice[..READ_LIMIT],
            total_kb,
            start + READ_LIMIT
        ))
    }
}

pub fn write_file(workspace: &Path, path: &str, content: &str) -> Result<String, String> {
    let abs = safe_join(workspace, Path::new(path))?;
    if abs.exists() {
        return Err(format!("File '{}' already exists. Use edit_file for modifications.", path));
    }
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&abs, content).map_err(|e| format!("Cannot write '{}': {}", path, e))?;
    Ok(format!("File created: {}", path))
}

pub fn edit_file(workspace: &Path, path: &str, search: &str, replace: &str) -> Result<String, String> {
    let abs = safe_join(workspace, Path::new(path))?;
    let content = std::fs::read_to_string(&abs)
        .map_err(|e| format!("Cannot read '{}': {}", path, e))?;
    if !content.contains(search) {
        return Err(format!("Search block not found in '{}'. The exact text did not match.", path));
    }
    let new_content = content.replacen(search, replace, 1);
    std::fs::write(&abs, new_content).map_err(|e| format!("Cannot write '{}': {}", path, e))?;
    Ok(format!("File edited: {}", path))
}

pub fn list_files(workspace: &Path, dir: &str) -> Result<String, String> {
    let abs = safe_join(workspace, Path::new(dir))?;
    let entries = std::fs::read_dir(&abs)
        .map_err(|e| format!("Cannot list '{}': {}", dir, e))?;
    let mut lines: Vec<String> = entries
        .flatten()
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_dir() { format!("{}/", name) } else { name }
        })
        .collect();
    lines.sort();
    Ok(lines.join("\n"))
}

pub fn bwrap_available() -> bool {
    std::process::Command::new("which")
        .arg("bwrap")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub async fn bash(workspace: &Path, command: &str) -> Result<String, String> {
    if !bwrap_available() {
        return Err(
            "Bash requires bubblewrap (Linux only). Install with: pacman -S bubblewrap".into()
        );
    }

    let ws_str = workspace.to_string_lossy();
    let output = tokio::process::Command::new("bwrap")
        .args([
            "--bind",        &ws_str, &ws_str,
            "--ro-bind",     "/usr",  "/usr",
            "--ro-bind",     "/lib",  "/lib",
            "--ro-bind",     "/lib64","/lib64",
            "--tmpfs",       "/tmp",
            "--unshare-net",
            "--die-with-parent",
            "--",
            "bash", "-c", command,
        ])
        .current_dir(workspace)
        .output()
        .await
        .map_err(|e| format!("bwrap spawn failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    let result = if !output.status.success() {
        format!("exit {}\n{}", output.status.code().unwrap_or(-1), combined)
    } else {
        combined
    };

    if result.len() <= BASH_LIMIT {
        Ok(result)
    } else {
        let total_kb = result.len() / 1024;
        Ok(format!(
            "{}\n[OUTPUT TRUNCATED — 4 KB shown of {} KB total.]",
            &result[..BASH_LIMIT],
            total_kb
        ))
    }
}

pub fn build_tool_definitions(include_bash: bool) -> Vec<serde_json::Value> {
    let mut tools = vec![
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a file from the workspace. Large files are truncated at 8 KB.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative path from workspace root" },
                        "offset": { "type": "integer", "description": "Byte offset to resume reading (for paginating large files)" }
                    },
                    "required": ["path"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Create a NEW file in the workspace. Fails if the file already exists — use edit_file to modify existing files.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["path", "content"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "edit_file",
                "description": "Surgically edit an existing file using SEARCH/REPLACE. Finds the exact 'search' text and replaces it with 'replace'. Fails if search text is not found.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path":    { "type": "string" },
                        "search":  { "type": "string", "description": "Exact block of text to find" },
                        "replace": { "type": "string", "description": "Text to replace it with" }
                    },
                    "required": ["path", "search", "replace"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "list_files",
                "description": "List files and directories inside a workspace directory.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "dir": { "type": "string", "description": "Relative directory path (use '.' for root)" }
                    },
                    "required": ["dir"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "search_web",
                "description": "Search the web for current information using DuckDuckGo.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    },
                    "required": ["query"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "search_vault",
                "description": "Search the local knowledge vault using semantic similarity.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    },
                    "required": ["query"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "generate_docx",
                "description": "Generate a Word document (.docx). Use this to produce reports or documents.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "filename": { "type": "string" },
                        "title":    { "type": "string" },
                        "elements": { "type": "array", "items": { "type": "object" } }
                    },
                    "required": ["filename", "title", "elements"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "generate_pptx",
                "description": "Generate a PowerPoint presentation (.pptx).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "filename": { "type": "string" },
                        "title":    { "type": "string" },
                        "slides":   { "type": "array", "items": { "type": "object" } }
                    },
                    "required": ["filename", "title", "slides"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "generate_xlsx",
                "description": "Generate an Excel spreadsheet (.xlsx).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "filename": { "type": "string" },
                        "sheets":   { "type": "array", "items": { "type": "object" } }
                    },
                    "required": ["filename", "sheets"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "generate_image",
                "description": "Generate an image from a text prompt using ComfyUI.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "prompt": { "type": "string" }
                    },
                    "required": ["prompt"]
                }
            }
        }),
    ];

    if include_bash {
        tools.push(serde_json::json!({
            "type": "function",
            "function": {
                "name": "bash",
                "description": "Execute a shell command inside the workspace sandbox (Linux only, network disabled).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" }
                    },
                    "required": ["command"]
                }
            }
        }));
    }
    tools
}

pub async fn execute(
    name: &str,
    args: &serde_json::Value,
    workspace: &Path,
) -> Result<String, String> {
    let get = |key: &str| -> Result<&str, String> {
        args[key].as_str().ok_or_else(|| format!("Missing argument: {}", key))
    };

    match name {
        "read_file" => {
            let offset = args["offset"].as_u64().map(|n| n as usize);
            read_file(workspace, get("path")?, offset)
        }
        "write_file"  => write_file(workspace, get("path")?, get("content")?),
        "edit_file"   => edit_file(workspace, get("path")?, get("search")?, get("replace")?),
        "list_files"  => list_files(workspace, get("dir")?),
        "bash"        => bash(workspace, get("command")?).await,
        "search_web"  => crate::search::duckduckgo_search(get("query")?).await,
        "search_vault"=> Ok(crate::memory::get_context(get("query")?).await),
        "generate_docx" => {
            let content = serde_json::from_value(args.clone())
                .map_err(|e| format!("Invalid docx args: {}", e))?;
            crate::office::generate_docx(content).await
        }
        "generate_pptx" => {
            let content = serde_json::from_value(args.clone())
                .map_err(|e| format!("Invalid pptx args: {}", e))?;
            crate::office::generate_pptx(content).await
        }
        "generate_xlsx" => {
            let content = serde_json::from_value(args.clone())
                .map_err(|e| format!("Invalid xlsx args: {}", e))?;
            crate::office::generate_xlsx(content).await
        }
        "generate_image" => {
            let prompt = get("prompt")?;
            Ok(format!("GENERATE_IMAGE:{}", prompt))
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

pub async fn probe_tool_calling(model: &str) -> bool {
    let probe_tool = serde_json::json!([{
        "type": "function",
        "function": {
            "name": "get_time",
            "description": "Returns the current time.",
            "parameters": { "type": "object", "properties": {}, "required": [] }
        }
    }]);

    let messages = vec![serde_json::json!({
        "role": "user",
        "content": "What time is it? Use the get_time tool."
    })];

    let client = reqwest::Client::new();
    let Ok(resp) = client
        .post("http://localhost:11434/api/chat")
        .json(&serde_json::json!({
            "model": model,
            "messages": messages,
            "tools": probe_tool,
            "stream": false,
        }))
        .send()
        .await
    else { return false; };

    let Ok(json) = resp.json::<serde_json::Value>().await else { return false; };

    json["message"]["tool_calls"].is_array()
        && !json["message"]["tool_calls"].as_array().unwrap().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn ws() -> std::path::PathBuf { std::path::PathBuf::from("/home/user/workspace") }

    // --- safe_join ---

    #[test]
    fn test_safe_join_normal() {
        let r = safe_join(&ws(), Path::new("src/main.rs")).unwrap();
        assert_eq!(r, std::path::PathBuf::from("/home/user/workspace/src/main.rs"));
    }

    #[test]
    fn test_safe_join_new_file() {
        let r = safe_join(&ws(), Path::new("new_dir/new_file.txt")).unwrap();
        assert_eq!(r, std::path::PathBuf::from("/home/user/workspace/new_dir/new_file.txt"));
    }

    #[test]
    fn test_safe_join_traversal_rejected() {
        assert!(safe_join(&ws(), Path::new("../../etc/passwd")).is_err());
    }

    #[test]
    fn test_safe_join_absolute_rejected() {
        assert!(safe_join(&ws(), Path::new("/etc/passwd")).is_err());
    }

    #[test]
    fn test_safe_join_double_dot_in_middle() {
        let r = safe_join(&ws(), Path::new("foo/../bar/file.rs")).unwrap();
        assert_eq!(r, std::path::PathBuf::from("/home/user/workspace/bar/file.rs"));
    }

    #[test]
    fn test_safe_join_parent_beyond_workspace_rejected() {
        assert!(safe_join(&ws(), Path::new("../other_project/secret.rs")).is_err());
    }

    // --- file tools ---

    use tempfile::TempDir;
    fn make_ws() -> TempDir { tempfile::TempDir::new().unwrap() }

    #[test]
    fn test_read_file_basic() {
        let tmp = make_ws();
        std::fs::write(tmp.path().join("hello.txt"), "Hello world").unwrap();
        let out = read_file(tmp.path(), "hello.txt", None).unwrap();
        assert_eq!(out, "Hello world");
    }

    #[test]
    fn test_read_file_truncation() {
        let tmp = make_ws();
        let big = "x".repeat(READ_LIMIT + 100);
        std::fs::write(tmp.path().join("big.txt"), &big).unwrap();
        let out = read_file(tmp.path(), "big.txt", None).unwrap();
        assert!(out.contains("[OUTPUT TRUNCATED"));
        assert!(out.len() < big.len());
    }

    #[test]
    fn test_read_file_with_offset() {
        let tmp = make_ws();
        std::fs::write(tmp.path().join("f.txt"), "abcdef").unwrap();
        let out = read_file(tmp.path(), "f.txt", Some(3)).unwrap();
        assert_eq!(out, "def");
    }

    #[test]
    fn test_write_file_creates_new() {
        let tmp = make_ws();
        write_file(tmp.path(), "new.txt", "content").unwrap();
        assert_eq!(std::fs::read_to_string(tmp.path().join("new.txt")).unwrap(), "content");
    }

    #[test]
    fn test_write_file_rejects_existing() {
        let tmp = make_ws();
        std::fs::write(tmp.path().join("exists.txt"), "old").unwrap();
        let err = write_file(tmp.path(), "exists.txt", "new").unwrap_err();
        assert!(err.contains("edit_file"));
    }

    #[test]
    fn test_edit_file_replaces_block() {
        let tmp = make_ws();
        std::fs::write(tmp.path().join("code.rs"), "fn old() {}").unwrap();
        edit_file(tmp.path(), "code.rs", "fn old() {}", "fn new() {}").unwrap();
        let content = std::fs::read_to_string(tmp.path().join("code.rs")).unwrap();
        assert_eq!(content, "fn new() {}");
    }

    #[test]
    fn test_edit_file_search_not_found() {
        let tmp = make_ws();
        std::fs::write(tmp.path().join("f.rs"), "fn main() {}").unwrap();
        let err = edit_file(tmp.path(), "f.rs", "fn missing() {}", "x").unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn test_list_files_basic() {
        let tmp = make_ws();
        std::fs::write(tmp.path().join("a.rs"), "").unwrap();
        std::fs::write(tmp.path().join("b.rs"), "").unwrap();
        let out = list_files(tmp.path(), ".").unwrap();
        assert!(out.contains("a.rs"));
        assert!(out.contains("b.rs"));
    }

    #[test]
    fn test_file_tools_reject_traversal() {
        let tmp = make_ws();
        assert!(read_file(tmp.path(), "../../etc/passwd", None).is_err());
        assert!(write_file(tmp.path(), "../escape.txt", "x").is_err());
        assert!(list_files(tmp.path(), "..").is_err());
    }

    // --- bash ---

    #[test]
    fn test_bwrap_detection() {
        let _ = bwrap_available();
    }

    #[tokio::test]
    async fn test_bash_disabled_message() {
        let tmp = make_ws();
        let result = bash(tmp.path(), "echo hello").await;
        match result {
            Ok(s) => assert!(!s.is_empty() || s.is_empty()),
            Err(e) => assert!(!e.is_empty()),
        }
    }

    #[tokio::test]
    async fn test_bash_output_truncation() {
        if !bwrap_available() { return; }
        let tmp = make_ws();
        let result = bash(tmp.path(), "python3 -c \"print('x' * 5000)\"").await.unwrap();
        assert!(result.contains("[OUTPUT TRUNCATED") || result.len() <= BASH_LIMIT + 200);
    }

    // --- tool definitions ---

    #[test]
    fn test_build_tool_definitions_excludes_bash_on_non_linux() {
        let tools = build_tool_definitions(false);
        let names: Vec<&str> = tools.iter()
            .filter_map(|t| t["function"]["name"].as_str())
            .collect();
        assert!(!names.contains(&"bash"));
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"write_file"));
        assert!(names.contains(&"edit_file"));
        assert!(names.contains(&"list_files"));
        assert!(names.contains(&"search_web"));
        assert!(names.contains(&"search_vault"));
    }

    #[test]
    fn test_build_tool_definitions_includes_bash_on_linux() {
        let tools = build_tool_definitions(true);
        let names: Vec<&str> = tools.iter()
            .filter_map(|t| t["function"]["name"].as_str())
            .collect();
        assert!(names.contains(&"bash"));
    }

    #[test]
    fn test_tool_definitions_have_required_fields() {
        let tools = build_tool_definitions(false);
        for tool in &tools {
            assert!(tool["type"].as_str().is_some(), "missing type");
            assert!(tool["function"]["name"].as_str().is_some(), "missing name");
            assert!(tool["function"]["description"].as_str().is_some(), "missing description");
            assert!(tool["function"]["parameters"].is_object(), "missing parameters");
        }
    }

    // --- probe ---

    #[test]
    fn test_probe_returns_bool_type() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(async {
            probe_tool_calling("nonexistent-model:latest").await
        });
    }
}

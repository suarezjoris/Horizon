use std::path::{Component, Path, PathBuf};
use crate::settings;

pub fn vault_dir(vault_path: &str) -> PathBuf {
    PathBuf::from(vault_path)
}

pub fn list_vault_notes(vault_path: &str) -> Vec<String> {
    let base = vault_dir(vault_path);
    let mut notes = Vec::new();
    walk_dir(&base, &base, &mut notes);
    notes
}

fn walk_dir(base: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dir(base, &path, out);
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_string_lossy().into_owned());
            }
        }
    }
}

pub fn read_vault_note(vault_path: &str, rel_path: &str) -> Result<String, String> {
    let path = vault_dir(vault_path).join(rel_path);
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

pub fn write_vault_note(vault_path: &str, rel_path: &str, content: &str) -> Result<(), String> {
    let path = vault_dir(vault_path).join(rel_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, content).map_err(|e| e.to_string())
}

pub fn append_note(vault_path: &str, rel_path: &str, text: &str) -> Result<(), String> {
    let path = vault_dir(vault_path).join(rel_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true).append(true).open(&path)
        .map_err(|e| e.to_string())?;
    file.write_all(text.as_bytes()).map_err(|e| e.to_string())
}

pub fn extract_wikilinks(text: &str) -> Vec<&str> {
    let mut links = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            i += 2;
            let start = i;
            while i + 1 < bytes.len() && !(bytes[i] == b']' && bytes[i + 1] == b']') {
                i += 1;
            }
            let link = &text[start..i];
            if !link.is_empty() { links.push(link); }
            i += 2;
        } else {
            i += 1;
        }
    }
    links
}

pub(crate) fn validate_rel_path(rel_path: &str) -> Result<(), String> {
    let p = Path::new(rel_path);
    if p.is_absolute() {
        return Err("absolute path not allowed".into());
    }
    for component in p.components() {
        if matches!(component, Component::ParentDir) {
            return Err("path traversal not allowed".into());
        }
    }
    if p.extension().map(|e| e != "md").unwrap_or(true) {
        return Err("only .md files allowed".into());
    }
    Ok(())
}

#[tauri::command]
pub fn list_notes() -> Vec<String> {
    let s = settings::load();
    list_vault_notes(&s.vault_path)
}

#[tauri::command]
pub fn read_note(rel_path: String) -> Result<String, String> {
    validate_rel_path(&rel_path)?;
    let s = settings::load();
    read_vault_note(&s.vault_path, &rel_path)
}

#[tauri::command]
pub fn write_note(rel_path: String, content: String) -> Result<(), String> {
    validate_rel_path(&rel_path)?;
    let s = settings::load();
    write_vault_note(&s.vault_path, &rel_path, &content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("memory")).unwrap();
        std::fs::write(tmp.path().join("memory/user.md"), "# User\n[[skills]]").unwrap();
        std::fs::write(tmp.path().join("memory/skills.md"), "# Skills\nRust").unwrap();
        tmp
    }

    #[test]
    fn list_vault_notes_finds_md_files() {
        let tmp = setup();
        let notes = list_vault_notes(&tmp.path().to_string_lossy());
        assert_eq!(notes.len(), 2);
        assert!(notes.iter().any(|n| n.contains("user.md")));
    }

    #[test]
    fn read_write_roundtrip() {
        let tmp = setup();
        let vp = tmp.path().to_string_lossy();
        write_vault_note(&vp, "test.md", "hello").unwrap();
        assert_eq!(read_vault_note(&vp, "test.md").unwrap(), "hello");
    }

    #[test]
    fn append_note_creates_and_appends() {
        let tmp = setup();
        let vp = tmp.path().to_string_lossy();
        append_note(&vp, "log.md", "line1\n").unwrap();
        append_note(&vp, "log.md", "line2\n").unwrap();
        let content = read_vault_note(&vp, "log.md").unwrap();
        assert!(content.contains("line1"));
        assert!(content.contains("line2"));
    }

    #[test]
    fn validate_rel_path_rejects_traversal() {
        assert!(validate_rel_path("../../etc/passwd").is_err());
        assert!(validate_rel_path("../secret.md").is_err());
    }

    #[test]
    fn validate_rel_path_rejects_absolute() {
        assert!(validate_rel_path("/etc/passwd").is_err());
    }

    #[test]
    fn validate_rel_path_rejects_non_md() {
        assert!(validate_rel_path("memory/user.txt").is_err());
    }

    #[test]
    fn validate_rel_path_accepts_valid() {
        assert!(validate_rel_path("memory/user.md").is_ok());
    }
}

import re

content = open("src/pax_daemon.rs").read()

# Replace find_gaps definition
content = re.sub(
    r"fn find_gaps\(vault_path: &str, index: &\[embeddings::Entry\]\) -> Vec<GapKind> \{.*?(?=\nasync fn evaluate_and_generate)",
    """fn find_gaps(vault_path: &str, index: &crate::embeddings::VaultIndex) -> Vec<GapKind> {
    let mut gaps = Vec::new();
    let now = now_secs();
    let notes = vault::list_vault_notes(vault_path);

    let avoided: Vec<String> = AVOIDED_NOTES.lock()
        .map(|av| av.iter()
            .filter(|(_, until)| *until > now)
            .map(|(n, _)| n.clone())
            .collect())
        .unwrap_or_default();

    for note in notes.iter().filter(|n| !avoided.contains(n)) {
        let full_path = std::path::PathBuf::from(vault_path).join(note);
        let mtime_age = std::fs::metadata(&full_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| std::time::SystemTime::now().duration_since(t).ok())
            .map(|d| d.as_secs())
            .unwrap_or(u64::MAX);

        if mtime_age > RECENT_WINDOW_SECS { continue; }

        let chunk_count = index.metadata.values().filter(|m| m.path == *note).count();
        if chunk_count >= MIN_CHUNKS_THRESHOLD { continue; }

        let preview = vault::read_vault_note(vault_path, note)
            .unwrap_or_default()
            .split_whitespace()
            .take(200)
            .collect::<Vec<_>>()
            .join(" ");

        gaps.push(GapKind::ThinNote { note: note.clone(), chunk_count, preview });
    }

    gaps.truncate(3);
    gaps
}
""", content, flags=re.DOTALL)

open("src/pax_daemon.rs", "w").write(content)

use std::path::PathBuf;

pub fn extract_pdf(path: &PathBuf) -> Option<String> {
    let out = std::process::Command::new("pdftotext")
        .arg(path)
        .arg("-")
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).chars().take(4000).collect())
    } else {
        None
    }
}

pub fn extract_zip_xml(path: &PathBuf) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut text = String::new();

    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.name().to_string();
        let is_content = name == "word/document.xml"
            || (name.starts_with("ppt/slides/slide") && name.ends_with(".xml"));
        if !is_content { continue; }

        let mut raw = String::new();
        if std::io::Read::read_to_string(&mut entry, &mut raw).is_ok() {
            text.push_str(&strip_xml(&raw));
            text.push(' ');
        }
    }

    if text.trim().is_empty() { return None; }
    Some(text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(4000).collect())
}

fn strip_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len() / 2);
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

pub fn url_slug(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(60)
        .collect()
}

pub fn find_orphans(vault_path: &str) -> Vec<PathBuf> {
    let base = PathBuf::from(vault_path);
    let mut orphans = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") { continue; }
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            if !content.contains("[[") {
                orphans.push(path);
            }
        }
    }
    orphans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_xml_removes_tags() {
        let xml = "<w:t>Hello</w:t><w:t> World</w:t>";
        assert_eq!(strip_xml(xml).trim(), "Hello World");
    }

    #[test]
    fn test_url_slug_basic() {
        assert_eq!(url_slug("My Report.pdf"), "my-report-pdf");
    }

    #[test]
    fn test_find_orphans_detects_no_wikilinks() {
        let dir = std::env::temp_dir().join("forge_orphan_test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lonely.md"), "# Lonely\nNo links here.").unwrap();
        std::fs::write(dir.join("connected.md"), "# Connected\nSee [[lonely]].").unwrap();

        let orphans = find_orphans(dir.to_str().unwrap());
        assert_eq!(orphans.len(), 1);
        assert!(orphans[0].file_name().unwrap() == "lonely.md");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

#[derive(Serialize, Deserialize, Clone)]
pub struct Entry {
    pub path: String,
    pub chunk: String,
    pub vector: Vec<f32>,
}

static INDEX_CACHE: Lazy<Mutex<Option<Arc<Vec<Entry>>>>> = Lazy::new(|| Mutex::new(None));

pub fn chunk_text(text: &str, size: usize, overlap: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i < words.len() {
        let end = (i + size).min(words.len());
        chunks.push(words[i..end].join(" "));
        if end == words.len() { break; }
        i += size - overlap;
    }
    chunks
}

pub fn search<'a>(index: &'a [Entry], query: &[f32], k: usize) -> Vec<&'a Entry> {
    let ma: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    let mut scored: Vec<(f32, &Entry)> = index.iter()
        .map(|e| (cosine_similarity_precalc(query, &e.vector, ma), e))
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.into_iter().take(k).map(|(_, e)| e).collect()
}

fn cosine_similarity_precalc(a: &[f32], b: &[f32], ma: f32) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if ma == 0.0 || mb == 0.0 { 0.0 } else { dot / (ma * mb) }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let ma: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    cosine_similarity_precalc(a, b, ma)
}

pub fn save_index(index: &[Entry], path: &str) {
    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(data) = bincode::serialize(index) {
        let _ = fs::write(path, data);
    }
    // Update cache
    if let Ok(mut cache) = INDEX_CACHE.lock() {
        *cache = Some(Arc::new(index.to_vec()));
    }
}

pub fn load_index(path: &str) -> Arc<Vec<Entry>> {
    if let Ok(mut cache) = INDEX_CACHE.lock() {
        if let Some(idx) = cache.as_ref() {
            return idx.clone();
        }
        
        // Cache miss, load from disk
        if let Ok(data) = fs::read(path) {
            if let Ok(index) = bincode::deserialize::<Vec<Entry>>(&data) {
                let arc_index = Arc::new(index);
                *cache = Some(arc_index.clone());
                return arc_index;
            }
        }
        
        let empty_idx = Arc::new(vec![]);
        *cache = Some(empty_idx.clone());
        return empty_idx;
    }
    Arc::new(vec![])
}

#[tauri::command]
pub async fn reindex() -> Result<usize, String> {
    let s = crate::settings::load();
    let notes = crate::vault::list_vault_notes(&s.vault_path);
    let mut all_entries: Vec<Entry> = Vec::new();

    for rel_path in &notes {
        let Ok(content) = crate::vault::read_vault_note(&s.vault_path, rel_path) else { continue };
        let chunks = chunk_text(&content, 400, 50);
        if chunks.is_empty() { continue; }
        let vectors = crate::ollama::embed(chunks.clone(), "nomic-embed-text:latest").await?;
        for (chunk, vector) in chunks.into_iter().zip(vectors) {
            all_entries.push(Entry { path: rel_path.clone(), chunk, vector });
        }
    }

    save_index(&all_entries, &s.embeddings_path);
    Ok(all_entries.len())
}

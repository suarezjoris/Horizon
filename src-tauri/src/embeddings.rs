use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use turbovec::IdMapIndex;

#[derive(Serialize, Deserialize, Clone)]
pub struct ChunkMeta {
    pub path: String,
    pub chunk: String,
    pub created_at: i64,        // Unix timestamp
    pub last_accessed: i64,     // Updated on each retrieval
    pub access_count: u32,      // Incremented on each retrieval
    #[serde(default)]
    pub pinned: bool,           // Parsed from frontmatter
    #[serde(default)]
    pub vector: Vec<f32>,       // Raw f32 embedding, for exact cosine re-score
}

pub fn chunk_markdown(content: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current_header = String::new();
    let mut buf = String::new();

    let flush = |buf: &mut String, header: &str, out: &mut Vec<String>| {
        let body = buf.trim();
        if body.is_empty() { return; }
        if !header.is_empty() && !body.starts_with(header) {
            out.push(format!("{}\n{}", header, body));
        } else {
            out.push(body.to_string());
        }
        buf.clear();
    };

    for block in content.split("\n\n") {
        let trimmed = block.trim();
        if trimmed.is_empty() { continue; }

        let is_header = trimmed.lines().next()
            .map(|l| l.starts_with('#') && l.contains(' '))
            .unwrap_or(false);
        if is_header {
            flush(&mut buf, &current_header, &mut chunks);
            current_header = trimmed.lines().next().unwrap_or("").to_string();
        }

        if buf.len() + trimmed.len() + 2 > max_chars && !buf.is_empty() {
            flush(&mut buf, &current_header, &mut chunks);
        }
        if !buf.is_empty() { buf.push_str("\n\n"); }
        buf.push_str(trimmed);

        while buf.len() > max_chars {
            let mut cut = max_chars;
            while cut < buf.len() && !buf.is_char_boundary(cut) { cut += 1; }
            let head: String = buf[..cut].to_string();
            chunks.push(if !current_header.is_empty() && !head.starts_with(&current_header) {
                format!("{}\n{}", current_header, head.trim())
            } else { head.trim().to_string() });
            buf = buf[cut..].trim_start().to_string();
        }
    }
    flush(&mut buf, &current_header, &mut chunks);
    chunks
}

pub fn lexical_score(query: &str, chunk: &str) -> f32 {
    let chunk_lower = chunk.to_lowercase();
    let terms: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3)
        .map(|w| w.to_string())
        .collect();
    if terms.is_empty() { return 0.0; }
    let hits = terms.iter().filter(|t| chunk_lower.contains(t.as_str())).count();
    hits as f32 / terms.len() as f32
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

pub struct VaultIndex {
    pub inner: IdMapIndex,
    pub metadata: std::sync::RwLock<HashMap<u64, ChunkMeta>>,
    pub next_id: u64,
}

pub struct SearchResult {
    pub path: String,
    pub chunk: String,
    pub score: f32,
    pub id: u64,
}

static INDEX_CACHE: Lazy<Mutex<Option<Arc<VaultIndex>>>> = Lazy::new(|| Mutex::new(None));



impl VaultIndex {
    pub fn new() -> Self {
        Self {
            inner: IdMapIndex::new(768, 4).unwrap(), // Nomic is 768 dim, 4-bit quantization
            metadata: std::sync::RwLock::new(HashMap::new()),
            next_id: 1,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.metadata.read().unwrap().is_empty()
    }

    pub fn add(&mut self, vector: &[f32], mut meta: ChunkMeta) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.inner.add_with_ids(vector, &[id]).unwrap();
        meta.vector = vector.to_vec();
        self.metadata.write().unwrap().insert(id, meta);
        id
    }

    pub fn remove(&mut self, id: u64) {
        self.inner.remove(id);
        self.metadata.write().unwrap().remove(&id);
    }

    pub fn remove_by_path(&mut self, path: &str) {
        let mut to_remove = Vec::new();
        for (&id, meta) in self.metadata.read().unwrap().iter() {
            if meta.path == path {
                to_remove.push(id);
            }
        }
        for id in to_remove {
            self.remove(id);
        }
    }

    pub fn search(&self, query: &[f32], k: usize) -> Vec<SearchResult> {
        const OVERFETCH: usize = 50;
        let (_scores, ids) = self.inner.search(query, k.max(OVERFETCH));
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let meta = self.metadata.read().unwrap();
        let mut results = Vec::new();
        for &id in &ids {
            if let Some(m) = meta.get(&id) {
                let exact = if m.vector.is_empty() { 0.0 } else { cosine(query, &m.vector) };
                let days_since = f32::max(0.0, (now - m.last_accessed) as f32 / 86400.0);
                let decay_factor = (0.5_f32).powf(days_since / 30.0);
                let final_score = exact * (1.0 + (m.access_count as f32 * 0.05)) * decay_factor;
                results.push(SearchResult {
                    path: m.path.clone(),
                    chunk: m.chunk.clone(),
                    score: final_score,
                    id,
                });
            }
        }
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        results
    }

    pub fn search_hybrid(&self, query_vec: &[f32], query_text: &str, k: usize, decay: bool) -> Vec<SearchResult> {
        const OVERFETCH: usize = 50;
        const ALPHA: f32 = 0.75; // vector weight
        const BETA: f32 = 0.25;  // lexical weight
        let (_scores, ids) = self.inner.search(query_vec, k.max(OVERFETCH));
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let meta = self.metadata.read().unwrap();
        let mut results = Vec::new();
        for &id in &ids {
            if let Some(m) = meta.get(&id) {
                let exact = if m.vector.is_empty() { 0.0 } else { cosine(query_vec, &m.vector) };
                let lex = lexical_score(query_text, &m.chunk);
                let base = ALPHA * exact + BETA * lex;
                let decay_factor = if decay {
                    let days_since = f32::max(0.0, (now - m.last_accessed) as f32 / 86400.0);
                    (0.5_f32).powf(days_since / 30.0)
                } else {
                    1.0
                };
                let final_score = base * (1.0 + (m.access_count as f32 * 0.05)) * decay_factor;
                results.push(SearchResult { path: m.path.clone(), chunk: m.chunk.clone(), score: final_score, id });
            }
        }
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        results
    }

    pub fn update_access_stats(&self, ids: &[u64]) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let mut meta_guard = self.metadata.write().unwrap();
        for id in ids {
            if let Some(meta) = meta_guard.get_mut(id) {
                meta.access_count += 1;
                meta.last_accessed = now;
            }
        }
    }
    
    pub fn save(&self, path: &str) -> Result<(), String> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = fs::create_dir_all(parent);
        }
        self.inner.write(path).map_err(|e| e.to_string())?;
        
        let meta_path = format!("{}.meta.json", path);
        let serialized_meta = serde_json::to_string(&*self.metadata.read().unwrap()).map_err(|e| e.to_string())?;
        fs::write(&meta_path, serialized_meta).map_err(|e| e.to_string())?;
        
        let next_id_path = format!("{}.nextid", path);
        let _ = fs::write(&next_id_path, self.next_id.to_string());
        
        Ok(())
    }
    
    pub fn load(path: &str) -> Result<Self, String> {
        let inner = IdMapIndex::load(path).map_err(|e| e.to_string())?;
        
        let meta_path = format!("{}.meta.json", path);
        let metadata_str = fs::read_to_string(&meta_path).map_err(|e| e.to_string())?;
        let metadata: HashMap<u64, ChunkMeta> = serde_json::from_str(&metadata_str).map_err(|e| e.to_string())?;
        
        let next_id_path = format!("{}.nextid", path);
        let next_id = fs::read_to_string(&next_id_path)
            .unwrap_or_else(|_| "1".to_string())
            .trim()
            .parse::<u64>()
            .unwrap_or(1);
            
        Ok(Self { inner, metadata: std::sync::RwLock::new(metadata), next_id })
    }
}

pub fn save_index(index: VaultIndex, path: &str) {
    let _ = index.save(path);
    if let Ok(mut cache) = INDEX_CACHE.lock() {
        *cache = Some(Arc::new(index));
    }
}

pub fn load_index(path: &str) -> Arc<VaultIndex> {
    if let Ok(mut cache) = INDEX_CACHE.lock() {
        if let Some(idx) = cache.as_ref() {
            return idx.clone();
        }
        
        // Cache miss, load from disk
        if let Ok(index) = VaultIndex::load(path) {
            let arc_index = Arc::new(index);
            *cache = Some(arc_index.clone());
            return arc_index;
        }
        
        // If it failed to load, check if it's the old .bin file, if so we should trigger a migration
        // Or if it just doesn't exist, return empty index
        let empty_idx = Arc::new(VaultIndex::new());
        *cache = Some(empty_idx.clone());
        return empty_idx;
    }
    Arc::new(VaultIndex::new())
}

#[tauri::command]
pub async fn reindex() -> Result<usize, String> {
    let s = crate::settings::load();
    let notes = crate::vault::list_vault_notes(&s.vault_path);
    let mut new_index = VaultIndex::new();

    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

    for rel_path in &notes {
        let Ok(content) = crate::vault::read_vault_note(&s.vault_path, rel_path) else { continue };
        let pinned = content.starts_with("---\n") && content.contains("pinned: true");
        
        let chunks = chunk_markdown(&content, 1000);
        if chunks.is_empty() { continue; }
        
        let prefixed: Vec<String> = chunks.iter()
            .map(|c| crate::ollama::nomic_prefix("nomic-embed-text:latest", crate::ollama::NomicTask::Document, c))
            .collect();
        let vectors = crate::ollama::embed(prefixed, "nomic-embed-text:latest").await?;
        for (chunk, vector) in chunks.into_iter().zip(vectors) {
            new_index.add(&vector, ChunkMeta {
                path: rel_path.clone(),
                chunk,
                created_at: now,
                last_accessed: now,
                access_count: 0,
                pinned,
                vector: vec![],
            });
        }
    }

    save_index(new_index, &s.embeddings_path);
    
    // Also try to delete old .bin if it exists
    if s.embeddings_path.ends_with(".tv") {
        let bin_path = s.embeddings_path.replace(".tv", ".bin");
        if std::path::Path::new(&bin_path).exists() {
            let _ = fs::remove_file(bin_path);
        }
    }
    
    // update cache
    let idx = load_index(&s.embeddings_path);
    let len = idx.metadata.read().unwrap().len();
    Ok(len)
}

#[cfg(test)]
mod lexical_tests {
    use super::lexical_score;

    #[test]
    fn all_terms_present_scores_one() {
        assert!((lexical_score("rust tauri", "I build with Rust and Tauri") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn no_terms_present_scores_zero() {
        assert_eq!(lexical_score("python django", "rust tauri app"), 0.0);
    }

    #[test]
    fn partial_overlap_is_fractional() {
        let s = lexical_score("rust python", "rust only here");
        assert!(s > 0.0 && s < 1.0);
    }
}

#[cfg(test)]
mod decay_tests {
    use super::*;

    fn old_chunk_index() -> VaultIndex {
        let mut idx = VaultIndex::new();
        let mut v = vec![0.0f32; 768]; v[0] = 1.0;
        let old = ChunkMeta {
            path: "identity.md".into(), chunk: "I love rust".into(),
            created_at: 0, last_accessed: 0, access_count: 0, pinned: false, vector: vec![],
        };
        idx.add(&v, old);
        idx
    }

    #[test]
    fn decay_off_scores_higher_than_decay_on_for_old_chunk() {
        let idx = old_chunk_index();
        let mut q = vec![0.0f32; 768]; q[0] = 1.0;
        let with = idx.search_hybrid(&q, "rust", 1, true);
        let without = idx.search_hybrid(&q, "rust", 1, false);
        assert!(without[0].score >= with[0].score);
    }
}

#[cfg(test)]
mod chunk_tests {
    use super::chunk_markdown;

    #[test]
    fn splits_on_headers() {
        let md = "# A\nalpha text\n\n# B\nbeta text";
        let chunks = chunk_markdown(md, 1000);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].contains("alpha"));
        assert!(chunks[1].contains("beta"));
    }

    #[test]
    fn long_section_splits_on_paragraphs_under_cap() {
        let para = "word ".repeat(300);
        let md = format!("# Big\n{}\n\n{}", para, para);
        let chunks = chunk_markdown(&md, 1000);
        assert!(chunks.len() >= 2);
        assert!(chunks.iter().all(|c| c.len() <= 1100));
    }

    #[test]
    fn empty_input_yields_no_chunks() {
        assert!(chunk_markdown("", 1000).is_empty());
    }
}

#[cfg(test)]
mod cosine_tests {
    use super::cosine;

    #[test]
    fn identical_vectors_score_one() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn orthogonal_vectors_score_zero() {
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
    }

    #[test]
    fn empty_or_zero_is_zero() {
        assert_eq!(cosine(&[], &[]), 0.0);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
    }
}

#[cfg(test)]
mod rerank_tests {
    use super::*;

    #[test]
    fn exact_cosine_reorders_candidates() {
        let mut idx = VaultIndex::new();
        let now = 0i64;
        let mk = |p: &str| ChunkMeta {
            path: p.into(), chunk: p.into(), created_at: now,
            last_accessed: now, access_count: 0, pinned: false, vector: vec![],
        };
        let mut a = vec![0.0f32; 768]; a[0] = 1.0;
        let mut b = vec![0.0f32; 768]; b[1] = 1.0;
        idx.add(&a, mk("a.md"));
        idx.add(&b, mk("b.md"));

        let mut q = vec![0.0f32; 768]; q[1] = 1.0;
        let results = idx.search(&q, 2);
        assert_eq!(results[0].path, "b.md");
    }
}

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

    pub fn add(&mut self, vector: &[f32], meta: ChunkMeta) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.inner.add_with_ids(vector, &[id]).unwrap();
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
        let (scores, ids) = self.inner.search(query, k);
        let mut results = Vec::new();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        for i in 0..ids.len() {
            if let Some(meta) = self.metadata.read().unwrap().get(&ids[i]) {
                let days_since = f32::max(0.0, (now - meta.last_accessed) as f32 / 86400.0);
                let decay_factor = (0.5_f32).powf(days_since / 30.0);
                let final_score = scores[i] * (1.0 + (meta.access_count as f32 * 0.05)) * decay_factor;
                results.push(SearchResult {
                    path: meta.path.clone(),
                    chunk: meta.chunk.clone(),
                    score: final_score,
                    id: ids[i],
                });
            }
        }
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
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
        
        let mut chunks = Vec::new();
        let mut current_header = String::new();
        let mut current_chunk = String::new();
        
        for line in content.lines() {
            if line.starts_with('#') && line.contains(' ') {
                let parts: Vec<&str> = line.splitn(2, ' ').collect();
                if parts[0].chars().all(|c| c == '#') {
                    current_header = line.to_string();
                }
            }
            
            if !current_chunk.is_empty() {
                current_chunk.push('\n');
            }
            current_chunk.push_str(line);
            
            if current_chunk.len() >= 1000 {
                let chunk_to_save = if !current_header.is_empty() && !current_chunk.starts_with(&current_header) {
                    format!("{}\n{}", current_header, current_chunk)
                } else {
                    current_chunk.clone()
                };
                chunks.push(chunk_to_save);
                
                let overlap_size = 200;
                if current_chunk.len() > overlap_size {
                    let target = current_chunk.len() - overlap_size;
                    let mut split_point = target;
                    while split_point < current_chunk.len() && !current_chunk.is_char_boundary(split_point) {
                        split_point += 1;
                    }
                    if let Some(next_space) = current_chunk[split_point..].find(|c: char| c.is_whitespace()) {
                        current_chunk = current_chunk[split_point + next_space..].trim_start().to_string();
                    } else {
                        current_chunk = current_chunk[split_point..].trim_start().to_string();
                    }
                } else {
                    current_chunk.clear();
                }
            }
        }
        
        if !current_chunk.trim().is_empty() {
            let chunk_to_save = if !current_header.is_empty() && !current_chunk.starts_with(&current_header) {
                format!("{}\n{}", current_header, current_chunk)
            } else {
                current_chunk.clone()
            };
            chunks.push(chunk_to_save);
        }

        if chunks.is_empty() { continue; }
        
        let vectors = crate::ollama::embed(chunks.clone(), "nomic-embed-text:latest").await?;
        for (chunk, vector) in chunks.into_iter().zip(vectors) {
            new_index.add(&vector, ChunkMeta {
                path: rel_path.clone(),
                chunk,
                created_at: now,
                last_accessed: now,
                access_count: 0,
                pinned,
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

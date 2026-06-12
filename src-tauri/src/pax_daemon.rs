use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::time::{Duration, Instant};
use std::path::PathBuf;
use once_cell::sync::Lazy;
use tokio::sync::mpsc::Receiver;
use tauri::{AppHandle, Emitter};
use crate::{embeddings, settings, vault};

// How long user must be idle before Pax asks (near-instant)
const IDLE_THRESHOLD_SECS: u64 = 5;
// Poll interval — fast so idle detection is responsive
const CHECK_INTERVAL_SECS: u64 = 2;

const RECENT_WINDOW_SECS: u64 = 48 * 3600;
const MIN_CHUNKS_THRESHOLD: usize = 3;
const SEMANTIC_SIM_THRESHOLD: f32 = 0.82;
const PERTINENCE_SCORE_MIN: u32 = 6;
const SEMANTIC_NOTE_LIMIT: usize = 15;

// Timestamp of last user message (0 = never sent anything)
static LAST_ACTIVITY: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));
// Pax asked a question and user hasn't replied yet
static WAITING_FOR_REPLY: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
// Note that was the subject of the last Pax question
static LAST_ASKED_NOTE: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));
// Notes Pax must avoid until their timestamp expires (avoid_until secs)
static AVOIDED_NOTES: Lazy<Mutex<Vec<(String, u64)>>> = Lazy::new(|| Mutex::new(Vec::new()));

static BANNER_LAST_SENT: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));

pub enum PaxEvent {
    Startup,
    ForgeStep { message: String },
    WorkspaceChange,
}

pub struct PaxEventSender(pub tokio::sync::mpsc::Sender<PaxEvent>);

#[derive(Clone, Copy, PartialEq)]
enum ReplyQuality { Positive, Neutral, Ignored }

static LAST_QUALITY: Lazy<Mutex<ReplyQuality>> =
    Lazy::new(|| Mutex::new(ReplyQuality::Neutral));

// How long to avoid a note after the user *engaged* with the topic (not a rejection)
const AVOID_ENGAGED_SECS: u64 = 2 * 3600;
// How long to avoid after explicit rejection
const AVOID_DURATION_SECS: u64 = 24 * 3600;

fn is_rejection(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    [
        "le sujet est clos", "c'est clos", "on clos", "sujet clos",
        "stop", "arrête", "arrete", "j'en ai marre", "pas ce sujet",
        "laisse tomber", "passe à autre chose", "change de sujet",
        "parle pas de ça", "n'en parle plus", "oublie ça",
        "on passe", "c'est bon", "ça suffit",
    ].iter().any(|s| lower.contains(s))
}

fn avoid_last_note() {
    avoid_last_note_for(AVOID_DURATION_SECS);
}

fn avoid_last_note_engaged() {
    avoid_last_note_for(AVOID_ENGAGED_SECS);
}

fn avoid_last_note_for(duration_secs: u64) {
    if let Ok(mut last) = LAST_ASKED_NOTE.lock() {
        if let Some(note) = last.take() {
            let until = now_secs() + duration_secs;
            if let Ok(mut av) = AVOIDED_NOTES.lock() {
                av.retain(|(n, _)| n != &note);
                av.push((note, until));
            }
        }
    }
}

pub fn touch_activity(msg: &str) {
    let now = now_secs();
    let prev = LAST_ACTIVITY.swap(now, Ordering::Relaxed);

    let was_waiting = WAITING_FOR_REPLY.swap(false, Ordering::Relaxed);

    // If Pax was waiting and user replied, put the topic on cooldown so
    // Pax doesn't immediately loop back to the same note.
    if was_waiting {
        if is_rejection(msg) {
            avoid_last_note();
        } else {
            let q = if msg.len() > 60 || msg.contains('?') {
                ReplyQuality::Positive
            } else {
                ReplyQuality::Neutral
            };
            *LAST_QUALITY.lock().unwrap() = q;
            avoid_last_note_engaged();
        }
    }
}

fn emit_status(app: &AppHandle, status: &str, msg: &str) {
    let _ = app.emit("armata-agent-status", serde_json::json!({
        "agent": "pax",
        "status": status,
        "message": msg
    }));
}

#[derive(Debug)]
enum GapKind {
    ThinNote { note: String, chunk_count: usize, preview: String },
    SemanticGap { note_a: String, note_b: String, similarity: f32 },
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() { return 0.0; }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let ma: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if ma == 0.0 || mb == 0.0 { 0.0 } else { dot / (ma * mb) }
}

fn avg_vector(vecs: &[Vec<f32>]) -> Vec<f32> {
    if vecs.is_empty() { return Vec::new(); }
    let len = vecs[0].len();
    let mut sum = vec![0.0f32; len];
    for v in vecs {
        for (i, x) in v.iter().enumerate() {
            if i < len { sum[i] += x; }
        }
    }
    let n = vecs.len() as f32;
    sum.iter_mut().for_each(|x| *x /= n);
    sum
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn adaptive_cooldown(now_secs: u64) -> u64 {
    let jitter = now_secs % 60;
    match *LAST_QUALITY.lock().unwrap() {
        ReplyQuality::Positive => 180 + jitter / 2,
        ReplyQuality::Neutral  => 540 + jitter,
        ReplyQuality::Ignored  => 1200 + jitter * 3,
    }
}

fn find_gaps(vault_path: &str, index: &[embeddings::Entry]) -> Vec<GapKind> {
    let mut gaps = Vec::new();
    let now = now_secs();
    let notes = vault::list_vault_notes(vault_path);

    // Build set of currently avoided notes
    let avoided: Vec<String> = AVOIDED_NOTES.lock()
        .map(|av| av.iter()
            .filter(|(_, until)| *until > now)
            .map(|(n, _)| n.clone())
            .collect())
        .unwrap_or_default();

    // Gap A: recently modified thin notes
    for note in notes.iter().filter(|n| !avoided.contains(n)) {
        let full_path = PathBuf::from(vault_path).join(note);
        let mtime_age = std::fs::metadata(&full_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| std::time::SystemTime::now().duration_since(t).ok())
            .map(|d| d.as_secs())
            .unwrap_or(u64::MAX);

        if mtime_age > RECENT_WINDOW_SECS { continue; }

        let chunk_count = index.iter().filter(|e| e.path == *note).count();
        if chunk_count >= MIN_CHUNKS_THRESHOLD { continue; }

        let preview = vault::read_vault_note(vault_path, note)
            .unwrap_or_default()
            .split_whitespace()
            .take(200)
            .collect::<Vec<_>>()
            .join(" ");

        gaps.push(GapKind::ThinNote { note: note.clone(), chunk_count, preview });
    }

    // Gap B: semantic gaps — limit to most recently modified notes
    let mut recent_notes: Vec<(String, u64)> = notes.iter()
        .filter_map(|n| {
            let full = PathBuf::from(vault_path).join(n);
            let age = std::fs::metadata(&full)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| std::time::SystemTime::now().duration_since(t).ok())
                .map(|d| d.as_secs())
                .unwrap_or(u64::MAX);
            if age <= now { Some((n.clone(), age)) } else { None }
        })
        .collect();
    recent_notes.sort_by_key(|(_, age)| *age);
    recent_notes.truncate(SEMANTIC_NOTE_LIMIT);

    // Average vector per note (excluding avoided)
    let recent_notes: Vec<_> = recent_notes.into_iter()
        .filter(|(n, _)| !avoided.contains(n))
        .collect();

    let note_vecs: Vec<(String, Vec<f32>)> = recent_notes.iter()
        .filter_map(|(note, _)| {
            let chunks: Vec<Vec<f32>> = index.iter()
                .filter(|e| e.path == *note)
                .map(|e| e.vector.clone())
                .collect();
            if chunks.is_empty() { return None; }
            Some((note.clone(), avg_vector(&chunks)))
        })
        .collect();

    for i in 0..note_vecs.len() {
        for j in (i + 1)..note_vecs.len() {
            let sim = cosine_similarity(&note_vecs[i].1, &note_vecs[j].1);
            if sim < SEMANTIC_SIM_THRESHOLD { continue; }

            let note_a = &note_vecs[i].0;
            let note_b = &note_vecs[j].0;

            // Check if already linked
            let content_a = vault::read_vault_note(vault_path, note_a).unwrap_or_default();
            let stem_b = note_b.trim_end_matches(".md").rsplit('/').next().unwrap_or(note_b);
            if content_a.contains(&format!("[[{}]]", stem_b)) { continue; }

            gaps.push(GapKind::SemanticGap {
                note_a: note_a.clone(),
                note_b: note_b.clone(),
                similarity: sim,
            });
        }
    }

    // Prioritise: thin notes first, then semantic gaps; take top 3
    gaps.truncate(3);
    gaps
}

async fn evaluate_and_generate(gap: &GapKind, model: &str) -> Option<String> {
    let (gap_type, description, preview) = match gap {
        GapKind::ThinNote { note, chunk_count, preview } => (
            "thin_note",
            format!("La note '{}' a été modifiée récemment mais ne contient que {} chunk(s) — elle manque de profondeur.", note, chunk_count),
            preview.chars().take(400).collect::<String>(),
        ),
        GapKind::SemanticGap { note_a, note_b, similarity } => (
            "semantic_gap",
            format!("Les notes '{}' et '{}' ont une similarité sémantique de {:.0}% mais aucun wikilink ne les relie.", note_a, note_b, similarity * 100.0),
            String::new(),
        ),
    };

    let prompt = format!(
        "Tu analyses le Second Brain d'un utilisateur. Voici une lacune détectée :\n\
TYPE: {}\n\
DESCRIPTION: {}\n\
{}\n\n\
Évalue l'intérêt de poser une question à l'utilisateur (0-10) :\n\
10 = lacune critique, l'utilisateur voudra clairement approfondir\n\
6 = intéressant, vaut la peine de demander\n\
0 = trivial ou sans intérêt\n\n\
Réponds UNIQUEMENT avec du JSON valide :\n\
{{\"score\": <nombre>, \"question\": \"<question naturelle en français, max 2 phrases>\"}}",
        gap_type,
        description,
        if preview.is_empty() { String::new() } else { format!("Contenu existant (extrait) : {}", preview) }
    );

    let msgs = vec![serde_json::json!({"role": "user", "content": prompt})];
    let resp = crate::ollama::chat_once(msgs, model).await.ok()?;

    let json_str = {
        let r = resp.trim();
        let s = r.find('{').unwrap_or(0);
        let e = r.rfind('}').map(|i| i + 1).unwrap_or(r.len());
        if s < e { &r[s..e] } else { r }
    };

    let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let score = parsed["score"].as_u64().unwrap_or(0) as u32;
    let question = parsed["question"].as_str()?;

    if score >= PERTINENCE_SCORE_MIN && !question.trim().is_empty() {
        Some(question.trim().to_string())
    } else {
        None
    }
}

pub async fn run_pax(app: AppHandle, running: Arc<AtomicBool>) {
    emit_status(&app, "online", "Watching for knowledge gaps…");

    let mut last_question: Option<Instant> = None;

    loop {
        for _ in 0..CHECK_INTERVAL_SECS {
            if !running.load(Ordering::Relaxed) {
                emit_status(&app, "offline", "Pax stopped");
                return;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        if !running.load(Ordering::Relaxed) {
            emit_status(&app, "offline", "Pax stopped");
            return;
        }

        let waiting      = WAITING_FOR_REPLY.load(Ordering::Relaxed);
        let last_seen    = LAST_ACTIVITY.load(Ordering::Relaxed);
        let now          = now_secs();

        // ── State machine ──────────────────────────────────────────────────

        if let Some(last_q) = last_question {
            let since_q = last_q.elapsed().as_secs();

            if waiting {
                if last_q.elapsed().as_secs() > 900 {
                    WAITING_FOR_REPLY.store(false, Ordering::Relaxed);
                    *LAST_QUALITY.lock().unwrap() = ReplyQuality::Ignored;
                }
                continue;
            }

            // Always enforce a minimum cooldown after any question + reply cycle.
            if since_q < adaptive_cooldown(now) { continue; }
        }

        // ── Idle check ─────────────────────────────────────────────────────
        // last_seen == 0 means no chat activity ever → treat as infinitely idle.
        let idle_secs = if last_seen == 0 {
            u64::MAX
        } else {
            now.saturating_sub(last_seen)
        };
        if idle_secs < IDLE_THRESHOLD_SECS { continue; }

        // ── VramQueue — held until LLM call finishes ───────────────────────
        let _permit = {
            use tauri::Manager;
            let q = app.state::<crate::vram_queue::VramQueue>();
            match q.try_acquire("pax-curiosity") {
                Some(p) => p,
                None => {
                    emit_status(&app, "online", "Deferred — GPU busy");
                    continue;
                }
            }
        };

        // ── Detect gap and ask ──────────────────────────────────────────────
        let s = settings::load();
        let index = embeddings::load_index(&s.embeddings_path);
        let gaps = find_gaps(&s.vault_path, &index);

        if gaps.is_empty() {
            emit_status(&app, "online", "No gaps found this cycle");
            continue;
        }

        // Record which note this question is about (for rejection tracking)
        let note_for_gap = match &gaps[0] {
            GapKind::ThinNote { note, .. } => Some(note.clone()),
            GapKind::SemanticGap { note_a, .. } => Some(note_a.clone()),
        };

        match evaluate_and_generate(&gaps[0], &s.agents.light_model).await {
            Some(q) => {
                if let Ok(mut last) = LAST_ASKED_NOTE.lock() {
                    *last = note_for_gap;
                }
                let _ = app.emit("pax-question", serde_json::json!({ "question": q }));
                WAITING_FOR_REPLY.store(true, Ordering::Relaxed);
                last_question = Some(Instant::now());
                emit_status(&app, "online", &format!("Asked: {}…", q.chars().take(60).collect::<String>()));
            }
            None => {
                emit_status(&app, "online", "Gap found but score below threshold");
            }
        }
        drop(_permit);
    }
}

#[tauri::command]
pub async fn trigger_pax(app: tauri::AppHandle) -> Result<String, String> {
    let s = settings::load();
    let index = embeddings::load_index(&s.embeddings_path);
    let gaps = find_gaps(&s.vault_path, &index);

    if gaps.is_empty() {
        return Ok("Pax: no gaps detected in the current vault.".to_string());
    }

    let question = evaluate_and_generate(&gaps[0], &s.agents.light_model).await;
    match question {
        Some(q) => {
            let _ = app.emit("pax-question", serde_json::json!({ "question": q }));
            Ok(format!("Pax asked: {}", q))
        }
        None => Ok(format!("Pax: gap detected but score below threshold. Gap: {:?}", gaps[0])),
    }
}

pub async fn run_pax_banner(
    mut rx: Receiver<PaxEvent>,
    tx: tokio::sync::mpsc::Sender<PaxEvent>,
    app: AppHandle,
) {
    const BANNER_COOLDOWN_SECS: u64 = 1800;

    // Watcher sur agent_workspace — thread bloquant séparé
    let workspace = PathBuf::from(&settings::load().agent_workspace);
    let tx_ws = tx.clone();
    std::thread::spawn(move || {
        use notify::{Watcher, RecursiveMode, recommended_watcher};
        let (ntx, nrx) = std::sync::mpsc::channel();
        if let Ok(mut w) = recommended_watcher(ntx) {
            let _ = w.watch(&workspace, RecursiveMode::NonRecursive);
            for res in nrx {
                if let Ok(event) = res {
                    let is_code = event.paths.iter().any(|p| {
                        matches!(
                            p.extension().and_then(|e| e.to_str()),
                            Some("md" | "rs" | "py" | "txt")
                        )
                    });
                    if is_code {
                        let _ = tx_ws.blocking_send(PaxEvent::WorkspaceChange);
                    }
                }
            }
        }
    });

    while let Some(_event) = rx.recv().await {
        let now = now_secs();
        if now.saturating_sub(BANNER_LAST_SENT.load(Ordering::Relaxed)) < BANNER_COOLDOWN_SECS {
            continue;
        }

        let s = settings::load();
        let index = embeddings::load_index(&s.embeddings_path);
        let gaps = find_gaps(&s.vault_path, &index);
        if gaps.is_empty() { continue; }

        use tauri::Manager;
        let _permit = {
            let q = app.state::<crate::vram_queue::VramQueue>();
            match q.try_acquire("pax-banner") {
                Some(p) => p,
                None => continue,
            }
        };

        if let Some(question) = evaluate_and_generate(&gaps[0], &s.agents.light_model).await {
            if let Ok(mut last) = LAST_ASKED_NOTE.lock() {
                *last = match &gaps[0] {
                    GapKind::ThinNote { note, .. } => Some(note.clone()),
                    GapKind::SemanticGap { note_a, .. } => Some(note_a.clone()),
                };
            }
            BANNER_LAST_SENT.store(now, Ordering::Relaxed);
            let _ = app.emit("pax-banner", serde_json::json!({ "question": question }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_touch_activity_updates_timestamp() {
        touch_activity("test message");
        let stored = LAST_ACTIVITY.load(Ordering::Relaxed);
        assert!(stored > 0);
        let now = now_secs();
        assert!(now >= stored);
        assert!(now - stored < 5);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 0.0, 1.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-5);
    }

    #[test]
    fn test_avg_vector_single() {
        let vecs = vec![vec![2.0, 4.0]];
        let avg = avg_vector(&vecs);
        assert_eq!(avg, vec![2.0, 4.0]);
    }

    #[test]
    fn test_avg_vector_two() {
        let vecs = vec![vec![0.0, 2.0], vec![2.0, 0.0]];
        let avg = avg_vector(&vecs);
        assert_eq!(avg, vec![1.0, 1.0]);
    }

    #[test]
    fn test_avg_vector_empty() {
        let avg = avg_vector(&[]);
        assert!(avg.is_empty());
    }

    #[test]
    fn test_adaptive_cooldown_positive() {
        *LAST_QUALITY.lock().unwrap() = ReplyQuality::Positive;
        let c = adaptive_cooldown(0);
        assert!(c >= 180 && c < 210, "Positive cooldown should be ~3-4min, got {}", c);
    }

    #[test]
    fn test_adaptive_cooldown_neutral() {
        *LAST_QUALITY.lock().unwrap() = ReplyQuality::Neutral;
        let c = adaptive_cooldown(0);
        assert!(c >= 540 && c < 600, "Neutral cooldown should be ~9-10min, got {}", c);
    }

    #[test]
    fn test_adaptive_cooldown_ignored() {
        *LAST_QUALITY.lock().unwrap() = ReplyQuality::Ignored;
        let c = adaptive_cooldown(0);
        assert!(c >= 1200 && c < 1380, "Ignored cooldown should be ~20-23min, got {}", c);
    }
}

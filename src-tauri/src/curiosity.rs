use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct AskedQuestion {
    pub facet: String,
    pub question: String,
    pub ts: i64,
    pub answered: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ProposedTopic {
    pub topic: String,
    pub ts: i64,
    pub accepted: bool,
}

#[derive(Serialize, Deserialize, Default)]
pub struct CuriosityJournal {
    pub asked: Vec<AskedQuestion>,
    pub proposed_topics: Vec<ProposedTopic>,
}

use std::path::PathBuf;

fn journal_path() -> PathBuf {
    let s = crate::settings::load();
    PathBuf::from(&s.vault_path).join("memory").join("curiosity_journal.json")
}

pub fn load_journal() -> CuriosityJournal {
    std::fs::read_to_string(journal_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_journal(j: &CuriosityJournal) -> Result<(), String> {
    let path = journal_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(j).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())
}

pub fn already_asked(j: &CuriosityJournal, facet: &str, question: &str) -> bool {
    j.asked.iter().any(|a| a.facet == facet && a.question.trim() == question.trim())
}

pub fn already_proposed(j: &CuriosityJournal, topic: &str) -> bool {
    let t = topic.trim().to_lowercase();
    j.proposed_topics.iter().any(|p| p.topic.trim().to_lowercase() == t)
}

use std::collections::HashSet;

pub const BASE_FACETS: &[&str] = &[
    "opinions_values",
    "experiences",
    "relations",
    "gouts_media",
    "objectifs",
    "routines",
    "principes",
];

/// Pull hobby stems from a passions note: the word before ':' on each bullet.
pub fn parse_hobbies(passions: &str) -> Vec<String> {
    passions
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix("- ").or_else(|| l.strip_prefix("* "))?;
            let name = rest.split(':').next()?.trim().to_lowercase();
            if name.is_empty() || name.len() > 30 { None } else { Some(name) }
        })
        .collect()
}

pub fn all_facets(hobbies: &[String]) -> Vec<String> {
    let mut v: Vec<String> = BASE_FACETS.iter().map(|s| s.to_string()).collect();
    for h in hobbies {
        v.push(format!("hobby_depth:{}", h));
    }
    v
}

/// A facet counts as covered once it has at least one answered question.
pub fn covered_facets(j: &CuriosityJournal) -> HashSet<String> {
    j.asked.iter().filter(|a| a.answered).map(|a| a.facet.clone()).collect()
}

pub fn pick_empty_facet(j: &CuriosityJournal, facets: &[String]) -> Option<String> {
    let covered = covered_facets(j);
    facets.iter().find(|f| !covered.contains(*f)).cloned()
}

pub fn build_question_prompt(facet: &str, asked: &[String]) -> String {
    let avoid = if asked.is_empty() {
        "(aucune pour l'instant)".to_string()
    } else {
        asked.iter().map(|q| format!("- {}", q)).collect::<Vec<_>>().join("\n")
    };
    format!(
        "Tu aides à enrichir le profil personnel de l'utilisateur. Pose UNE SEULE question, \
courte, précise et concrète, pour en apprendre plus sur cette facette de lui : '{}'.\n\
Règles strictes :\n\
- Une seule question, en français, tutoiement.\n\
- Évite absolument ces questions déjà posées :\n{}\n\
- Si la facette semble large, descends dans un détail précis.\n\
- Réponds UNIQUEMENT avec la question, sans préambule ni guillemets.",
        facet, avoid
    )
}

#[tauri::command]
pub async fn curiosity_next_question(
    vram_queue: tauri::State<'_, crate::vram_queue::VramQueue>,
) -> Result<Option<String>, String> {
    // Don't compete with active chat for the GPU.
    let _permit = match vram_queue.try_acquire("curiosity") {
        Some(p) => p,
        None => return Ok(None),
    };

    let s = crate::settings::load();
    let journal = load_journal();

    let passions = crate::vault::read_vault_note(&s.vault_path, "passions.md").unwrap_or_default();
    let hobbies = parse_hobbies(&passions);
    let facets = all_facets(&hobbies);

    let Some(facet) = pick_empty_facet(&journal, &facets) else {
        return Ok(None); // everything covered
    };

    let asked_in_facet: Vec<String> = journal.asked.iter()
        .filter(|a| a.facet == facet)
        .map(|a| a.question.clone())
        .collect();

    let prompt = build_question_prompt(&facet, &asked_in_facet);
    let raw = crate::ollama::chat_once(
        vec![serde_json::json!({"role": "user", "content": prompt})],
        &s.agents.light_model,
    ).await?;

    let question = raw.trim().trim_matches('"').trim().to_string();
    if question.is_empty() || already_asked(&journal, &facet, &question) {
        return Ok(None);
    }

    let mut journal = journal;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    journal.asked.push(AskedQuestion { facet, question: question.clone(), ts: now, answered: false });
    let _ = save_journal(&journal);

    Ok(Some(question))
}

#[tauri::command]
pub fn curiosity_mark_answered(question: String) -> Result<(), String> {
    let mut journal = load_journal();
    if let Some(a) = journal.asked.iter_mut().find(|a| a.question.trim() == question.trim()) {
        a.answered = true;
    }
    save_journal(&journal)
}

/// Interest terms = hobby stems plus any longish words from mindset.
pub fn interest_terms(passions: &str, mindset: &str) -> Vec<String> {
    let mut terms = parse_hobbies(passions);
    for w in mindset.split(|c: char| !c.is_alphanumeric()) {
        let w = w.trim().to_lowercase();
        if w.len() >= 5 && !terms.contains(&w) {
            terms.push(w);
        }
    }
    terms.truncate(20);
    terms
}

#[tauri::command]
pub fn curiosity_propose_topic() -> Result<Option<String>, String> {
    let s = crate::settings::load();
    let journal = load_journal();
    let passions = crate::vault::read_vault_note(&s.vault_path, "passions.md").unwrap_or_default();
    let mindset = crate::vault::read_vault_note(&s.vault_path, "mindset.md").unwrap_or_default();

    let knowledge_dir = std::path::PathBuf::from(&s.vault_path).join("knowledge");

    for topic in interest_terms(&passions, &mindset) {
        if already_proposed(&journal, &topic) { continue; }
        let slug = crate::forge_daemon::url_slug(&topic);
        let note = knowledge_dir.join(format!("{}.md", slug));
        if note.exists() { continue; }
        return Ok(Some(topic));
    }
    Ok(None)
}

#[tauri::command]
pub async fn curiosity_fill_topic(
    vram_queue: tauri::State<'_, crate::vram_queue::VramQueue>,
    topic: String,
) -> Result<String, String> {
    let _permit = vram_queue.acquire("curiosity-fill").await.map_err(|e| e.to_string())?;
    let s = crate::settings::load();

    let web = crate::search::duckduckgo_search(&topic).await?;
    let prompt = format!(
        "Rédige une note de connaissance markdown concise sur '{}', à partir de ces \
résultats web. Titre, résumé, puis faits clés en bullets. Pas de blabla.\n\nRésultats:\n{}",
        topic, web
    );
    let body = crate::ollama::chat_once(
        vec![serde_json::json!({"role": "user", "content": prompt})],
        &s.agents.light_model,
    ).await?;

    let slug = crate::forge_daemon::url_slug(&topic);
    let rel = format!("knowledge/{}.md", slug);
    let content = format!("# {}\n\n{}\n", topic, body.trim());
    crate::vault::write_vault_note(&s.vault_path, &rel, &content)?;

    let mut journal = load_journal();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    journal.proposed_topics.push(ProposedTopic { topic: topic.clone(), ts: now, accepted: true });
    let _ = save_journal(&journal);

    let _ = crate::embeddings::reindex().await;
    Ok(format!("Connaissance ajoutée : {}", rel))
}

#[tauri::command]
pub fn curiosity_dismiss_topic(topic: String) -> Result<(), String> {
    let mut journal = load_journal();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    journal.proposed_topics.push(ProposedTopic { topic, ts: now, accepted: false });
    save_journal(&journal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn journal() -> CuriosityJournal {
        CuriosityJournal {
            asked: vec![AskedQuestion { facet: "objectifs".into(), question: "Quel est ton but ?".into(), ts: 0, answered: false }],
            proposed_topics: vec![ProposedTopic { topic: "airsoft hpa".into(), ts: 0, accepted: false }],
        }
    }

    #[test]
    fn already_asked_matches_facet_and_question() {
        let j = journal();
        assert!(already_asked(&j, "objectifs", "Quel est ton but ?"));
        assert!(!already_asked(&j, "objectifs", "Une autre question ?"));
    }

    #[test]
    fn already_proposed_matches_topic_case_insensitive() {
        let j = journal();
        assert!(already_proposed(&j, "Airsoft HPA"));
        assert!(!already_proposed(&j, "piano jazz"));
    }

    #[test]
    fn parse_hobbies_extracts_bullet_stems() {
        let passions = "# passions\n- Airsoft: une réplique\n- Piano: 19 ans\n- IA: veille";
        let h = parse_hobbies(passions);
        assert!(h.contains(&"airsoft".to_string()));
        assert!(h.contains(&"piano".to_string()));
        assert!(h.contains(&"ia".to_string()));
    }

    #[test]
    fn pick_empty_facet_skips_answered() {
        let mut j = CuriosityJournal::default();
        for f in &BASE_FACETS[..BASE_FACETS.len() - 1] {
            j.asked.push(AskedQuestion { facet: f.to_string(), question: "q".into(), ts: 0, answered: true });
        }
        let facets = all_facets(&[]);
        let picked = pick_empty_facet(&j, &facets).unwrap();
        assert_eq!(picked, *BASE_FACETS.last().unwrap());
    }

    #[test]
    fn build_question_prompt_lists_avoid_set_and_facet() {
        let p = build_question_prompt("hobby_depth:piano", &["Tu joues quoi ?".to_string()]);
        assert!(p.contains("hobby_depth:piano"));
        assert!(p.contains("Tu joues quoi ?"));
        assert!(p.to_lowercase().contains("une seule question"));
    }

    #[test]
    fn interest_terms_merges_hobbies_and_keywords() {
        let passions = "- Airsoft: x\n- Piano: y";
        let mindset = "J'aime l'optimisation et le hardware.";
        let terms = interest_terms(passions, mindset);
        assert!(terms.contains(&"airsoft".to_string()));
        assert!(terms.contains(&"piano".to_string()));
        assert!(!terms.is_empty());
    }
}

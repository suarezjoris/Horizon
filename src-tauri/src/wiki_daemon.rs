use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use chrono::{Local, Timelike};
use crate::settings;

fn emit_status(app: &AppHandle, status: &str, msg: &str) {
    let _ = app.emit("armata-agent-status", serde_json::json!({
        "agent": "wiki",
        "status": status,
        "message": msg
    }));
}

fn last_run_path(vault_path: &str) -> PathBuf {
    PathBuf::from(vault_path).join("knowledge/.wiki-last-run")
}

fn already_ran_today(vault_path: &str) -> bool {
    let today = Local::now().format("%Y-%m-%d").to_string();
    std::fs::read_to_string(last_run_path(vault_path))
        .ok()
        .map(|s| s.trim().to_string())
        .as_deref() == Some(today.as_str())
}

fn mark_ran_today(vault_path: &str) {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let path = last_run_path(vault_path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, today);
}

fn secs_until_noon() -> u64 {
    let now = Local::now();
    let elapsed = now.hour() as u64 * 3600 + now.minute() as u64 * 60 + now.second() as u64;
    let noon = 12 * 3600u64;
    if elapsed < noon {
        noon - elapsed
    } else {
        24 * 3600 - elapsed + noon
    }
}

async fn do_ingest(app: &AppHandle) {
    emit_status(app, "online", "Ingesting Wikipedia articles…");
    match crate::wikipedia::ingest_wikipedia(app.clone()).await {
        Ok(msg) => emit_status(app, "online", &msg),
        Err(e)  => emit_status(app, "warn", &format!("Ingest failed: {}", e)),
    }
}

pub async fn run_wiki_agent(app: AppHandle, running: Arc<AtomicBool>) {
    emit_status(&app, "online", "Scheduled daily at noon");

    let vault = settings::load().vault_path;

    // Boot catch-up: if it's past noon and today's run was missed, go now
    if Local::now().hour() >= 12 && !already_ran_today(&vault) {
        do_ingest(&app).await;
        mark_ran_today(&vault);
    }

    loop {
        let secs = secs_until_noon();
        emit_status(&app, "online", &format!(
            "Next run in {}h{}m", secs / 3600, (secs % 3600) / 60
        ));

        let mut remaining = secs;
        while remaining > 0 {
            if !running.load(Ordering::Relaxed) {
                emit_status(&app, "offline", "Wiki agent stopped");
                return;
            }
            tokio::time::sleep(Duration::from_secs(remaining.min(60))).await;
            remaining = remaining.saturating_sub(60);
        }

        if !running.load(Ordering::Relaxed) { break; }

        let vault = settings::load().vault_path;
        if !already_ran_today(&vault) {
            do_ingest(&app).await;
            mark_ran_today(&vault);
        }
    }

    emit_status(&app, "offline", "Wiki agent stopped");
}

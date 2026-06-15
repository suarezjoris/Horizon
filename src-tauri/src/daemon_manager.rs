use crate::{antenna, app_state::ArmataState, archivist, armata, forge_daemon, settings, vanguard, wiki_daemon};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use tauri::Manager;
use tokio::sync::mpsc;

#[tauri::command]
pub async fn toggle_agent_daemon(
    app: tauri::AppHandle,
    armata_state: tauri::State<'_, ArmataState>,
    agent: String,
    enabled: bool,
) -> Result<String, String> {
    {
        let mut flags = armata_state.running_flags.lock().unwrap();

        if !enabled {
            // Signal existing daemon to stop
            if let Some(flag) = flags.remove(&agent) {
                flag.store(false, Ordering::Relaxed);
            }
        } else {
            // Don't double-spawn
            if flags.contains_key(&agent) {
                return Ok(format!("{} already running", agent));
            }

            let flag = Arc::new(AtomicBool::new(true));
            let app_clone = app.clone();
            let flag_clone = flag.clone();

            match agent.as_str() {
                "archivist" => {
                    tokio::spawn(async move {
                        archivist::run_archivist(app_clone, flag_clone).await;
                    });
                }
                "vanguard" => {
                    tokio::spawn(async move {
                        vanguard::run_vanguard(app_clone, flag_clone).await;
                    });
                }
                "antenna" => {
                    tokio::spawn(async move {
                        antenna::run_antenna(app_clone, flag_clone).await;
                    });
                }
                "forge" => {
                    tokio::spawn(async move {
                        forge_daemon::run_forge(app_clone, flag_clone).await;
                    });
                }
                "wiki" => {
                    tokio::spawn(async move {
                        wiki_daemon::run_wiki_agent(app_clone, flag_clone).await;
                    });
                }

                _ => return Err(format!("Unknown agent: {}", agent)),
            }

            flags.insert(agent.clone(), flag);
        }
    } // MutexGuard is dropped here

    // Persist setting
    armata::toggle_agent(app, agent.clone(), enabled).await?;
    Ok(format!("{} → {}", agent, if enabled { "ONLINE" } else { "OFFLINE" }))
}

pub fn auto_start_daemons(app: &mut tauri::App) {


    let s = settings::load();
    let handle = app.handle().clone();

    // Auto-start daemons that were enabled at last shutdown
    if s.agents.archivist_enabled {
        let flag = Arc::new(AtomicBool::new(true));
        let app2 = handle.clone();
        let f2 = flag.clone();
        app.state::<ArmataState>().running_flags.lock().unwrap().insert("archivist".into(), flag);
        tauri::async_runtime::spawn(async move { archivist::run_archivist(app2, f2).await; });
    }
    if s.agents.vanguard_enabled {
        let flag = Arc::new(AtomicBool::new(true));
        let app2 = handle.clone();
        let f2 = flag.clone();
        app.state::<ArmataState>().running_flags.lock().unwrap().insert("vanguard".into(), flag);
        tauri::async_runtime::spawn(async move { vanguard::run_vanguard(app2, f2).await; });
    }
    if s.agents.antenna_enabled {
        let flag = Arc::new(AtomicBool::new(true));
        let app2 = handle.clone();
        let f2 = flag.clone();
        app.state::<ArmataState>().running_flags.lock().unwrap().insert("antenna".into(), flag);
        tauri::async_runtime::spawn(async move { antenna::run_antenna(app2, f2).await; });
    }
    if s.agents.forge_enabled {
        let flag = Arc::new(AtomicBool::new(true));
        let app2 = handle.clone();
        let f2 = flag.clone();
        app.state::<ArmataState>().running_flags.lock().unwrap().insert("forge".into(), flag);
        tauri::async_runtime::spawn(async move { forge_daemon::run_forge(app2, f2).await; });
    }
    if s.agents.wiki_enabled {
        let flag = Arc::new(AtomicBool::new(true));
        let app2 = handle.clone();
        let f2 = flag.clone();
        app.state::<ArmataState>().running_flags.lock().unwrap().insert("wiki".into(), flag);
        tauri::async_runtime::spawn(async move { wiki_daemon::run_wiki_agent(app2, f2).await; });
    }

}

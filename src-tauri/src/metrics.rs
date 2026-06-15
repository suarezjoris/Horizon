use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use sysinfo::{System, ProcessRefreshKind, RefreshKind};
use tauri::{AppHandle, Manager, Emitter};
use crate::app_state::ArmataState;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct DaemonMetric {
    pub cpu: f32,
    pub ram_mb: f32,
}

pub struct MetricsState {
    pub sys: std::sync::Mutex<System>,
}

impl MetricsState {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing().with_cpu())
        );
        sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        Self {
            sys: std::sync::Mutex::new(sys),
        }
    }
}

pub fn spawn_metrics_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if let Ok(metrics) = gather_metrics(&app) {
                let _ = app.emit("system-metrics", metrics);
            }
        }
    });
}

pub fn gather_metrics(app: &AppHandle) -> Result<HashMap<String, DaemonMetric>, String> {
    let state = app.state::<MetricsState>();
    let mut sys = state.sys.lock().map_err(|_| "Mutex poisoned")?;
    
    // Refresh only the current process
    let pid = sysinfo::get_current_pid().map_err(|e| e.to_string())?;
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_cpu().with_memory(),
    );

    let mut total_cpu = 0.0;
    let mut total_ram_mb = 0.0;

    if let Some(proc) = sys.process(pid) {
        total_cpu = proc.cpu_usage();
        total_ram_mb = proc.memory() as f32 / 1024.0 / 1024.0;
    }

    let armata_state = app.state::<ArmataState>();
    let active_daemons = {
        let flags = armata_state.running_flags.lock().unwrap();
        flags.keys().cloned().collect::<Vec<String>>()
    };

    let mut metrics = HashMap::new();
    let count = active_daemons.len();

    if count > 0 {
        // Distribute the process usage among active daemons for the "OS feel"
        // Give each a base slice, plus some pseudo-random jitter based on the name length
        let base_cpu = total_cpu / (count as f32);
        let base_ram = total_ram_mb / (count as f32);

        for (i, daemon) in active_daemons.iter().enumerate() {
            let jitter_cpu = (i as f32 * 0.1).sin() * 0.5; // Small oscillation
            let jitter_ram = (i as f32 * 0.5).cos() * 2.0;

            metrics.insert(daemon.clone(), DaemonMetric {
                cpu: (base_cpu + jitter_cpu).max(0.0),
                ram_mb: (base_ram + jitter_ram).max(0.0),
            });
        }
    }

    Ok(metrics)
}

#[tauri::command]
pub fn get_daemon_metrics(app: AppHandle) -> Result<HashMap<String, DaemonMetric>, String> {
    gather_metrics(&app)
}

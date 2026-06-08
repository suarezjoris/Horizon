use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use axum::{Router, routing::{get, post}, extract::{State, Json}, http::{HeaderMap, StatusCode}};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use crate::settings;

#[derive(Deserialize)]
struct CommandPayload {
    cmd: String,
}

#[derive(Serialize)]
struct CommandResponse {
    result: String,
}

#[derive(Clone)]
struct AntennaState {
    token: String,
    app: AppHandle,
}

pub fn verify_token(expected: &str, provided: &str) -> bool {
    !expected.is_empty() && expected == provided
}

fn emit_status(app: &AppHandle, status: &str, msg: &str) {
    let _ = app.emit("armata-agent-status", serde_json::json!({
        "agent": "antenna",
        "status": status,
        "message": msg
    }));
}

async fn handle_status() -> &'static str {
    "ARMATA ANTENNA ONLINE"
}

async fn handle_command(
    State(state): State<AntennaState>,
    headers: HeaderMap,
    Json(payload): Json<CommandPayload>,
) -> (StatusCode, Json<CommandResponse>) {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .trim_start_matches("Bearer ");

    if !verify_token(&state.token, auth) {
        return (StatusCode::UNAUTHORIZED, Json(CommandResponse { result: "Unauthorized".into() }));
    }

    emit_status(&state.app, "online", &format!("Remote command: {}", payload.cmd));

    let result = crate::armata::route_command(payload.cmd).await
        .unwrap_or_else(|e| format!("Error: {}", e));

    (StatusCode::OK, Json(CommandResponse { result }))
}

pub async fn run_antenna(app: AppHandle, running: Arc<AtomicBool>) {
    let s = settings::load();

    if s.agents.antenna_token == "changeme" || s.agents.antenna_token.is_empty() {
        emit_status(&app, "error", "Antenna refused: set a real token in Settings (current token is 'changeme')");
        return;
    }

    let port = s.agents.antenna_port;
    let token = s.agents.antenna_token.clone();

    let state = AntennaState { token, app: app.clone() };

    let router = Router::new()
        .route("/status", get(handle_status))
        .route("/command", post(handle_command))
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    emit_status(&app, "online", &format!("Listening on http://{}", addr));

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            emit_status(&app, "error", &format!("Bind failed: {}", e));
            return;
        }
    };

    let running_clone = running.clone();
    tokio::select! {
        result = axum::serve(listener, router) => {
            if let Err(e) = result {
                emit_status(&app, "error", &format!("Server error: {}", e));
            }
        }
        _ = async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                if !running_clone.load(Ordering::Relaxed) { return; }
            }
        } => {}
    }

    emit_status(&app, "offline", "Antenna stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_token_matches() {
        assert!(verify_token("secret123", "secret123"));
        assert!(!verify_token("secret123", "wrong"));
        assert!(!verify_token("secret123", ""));
    }
}

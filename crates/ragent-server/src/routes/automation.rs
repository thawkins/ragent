//! Automation service HTTP surface (spec `openhands` T-016; FR-013, FR-018).
//!
//! Two surfaces:
//!
//! - **Webhook ingress** (FR-013): `POST /auto/{id}` accepts a JSON (or plain
//!   text) payload from an external system, enqueues an agent run for the named
//!   automation with the payload supplied as prompt context, and returns `202
//!   Accepted` with the new run id. The ingress is **public** (like `/health`)
//!   because an external system such as a GitHub webhook has no bearer token;
//!   the run itself executes through the same permission-checked agent loop as
//!   any other turn, confined to the automation's configured backend (FR-033).
//! - **Run history** (FR-018): `GET /automation` lists the configured
//!   automations and their next-due times; `GET /automation/runs/{id}` lists an
//!   automation's run records; `POST /automation/{id}/run` enqueues a manual run.
//!   These are authenticated like the rest of the REST API.
//!
//! The service is built per request from the shared [`AppState`] (its config,
//! storage, and session processor); no per-request state is retained beyond the
//! durable run-history rows.

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Value, json};

use ragent_agent::automation::{
    AutomationService, backend_descriptor, compute_next_due, session_backend_override,
};

use super::{AppState, error_response, internal_error_response, serialize_response};

/// Resolve the execution-backend descriptor a session override names, if any
/// (FR-033). Exposed for diagnostics and tests.
#[must_use]
pub fn override_descriptor(session_id: &str) -> Option<ragent_config::BackendConfig> {
    session_backend_override(session_id)
}

/// The public automation webhook ingress (FR-013).
///
/// Registered on the outer router (like `/health`) so an external system can
/// reach it without a bearer token.
pub fn automation_routes() -> Router<AppState> {
    Router::new().route("/auto/{id}", post(webhook_ingest))
}

/// The authenticated automation run-history routes (FR-018).
///
/// Registered on the auth-protected sub-router so listing automations and runs
/// requires the native bearer token.
pub fn automation_admin_routes() -> Router<AppState> {
    Router::new()
        .route("/automation", get(list_automations))
        .route("/automation/runs/{id}", get(list_runs))
        .route("/automation/{id}/run", post(run_now))
}

/// A working directory for an automation run: the server's current directory
/// (serve is launched from the project root).
fn working_dir() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

/// Build an automation service from the shared state (FR-013).
///
/// The automation definitions come from the server's live config (the same
/// `AppState::config` every other route reads), so a `--config` server and a
/// test fixture behave identically.
async fn service(state: &AppState) -> AutomationService {
    let automation = state.config.read().await.automation_config();
    AutomationService::new(
        Arc::clone(&state.session_processor),
        automation,
        working_dir(),
    )
}

/// A backend-config descriptor for a backend label (FR-033). Exposed for
/// diagnostics and tests.
#[must_use]
pub fn descriptor_for(config: &ragent_config::Config, label: &str) -> ragent_config::BackendConfig {
    backend_descriptor(config, label)
}

/// Render a webhook payload as prompt context: the raw JSON (or text) body.
fn payload_text(body: &Bytes) -> String {
    let text = String::from_utf8_lossy(body);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "(empty payload)".to_string();
    }
    // Re-serialise valid JSON compactly so the model sees a stable shape; a
    // non-JSON body is passed through verbatim.
    match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => serde_json::to_string(&value).unwrap_or_else(|_| trimmed.to_string()),
        Err(_) => trimmed.to_string(),
    }
}

/// `POST /auto/{id}` - webhook ingress (FR-013).
async fn webhook_ingest(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let svc = Arc::new(service(&state).await);
    if let Some(response) = guard_automation(&svc, &id) {
        return response;
    }
    let payload = payload_text(&body);
    match svc
        .enqueue(&id, ragent_types::AutomationTrigger::Webhook, &payload)
        .await
    {
        Ok(run) => accepted_run(&run),
        Err(e) => internal_error_response("automation webhook enqueue", e).into_response(),
    }
}

/// Refuse an enabled-definition request early: `503` when the service is disabled
/// and `404` when no enabled automation has that id (FR-013, FR-018). Returns the
/// refusal response, or `None` when the request may proceed.
fn guard_automation(svc: &AutomationService, id: &str) -> Option<Response> {
    if !svc.is_enabled() {
        return Some(
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "automation service is disabled",
            )
            .into_response(),
        );
    }
    if svc.definition(id).is_none() {
        return Some(
            error_response(
                StatusCode::NOT_FOUND,
                format!("no enabled automation `{id}`"),
            )
            .into_response(),
        );
    }
    None
}

/// The `202 Accepted` body naming the enqueued run (FR-013, FR-018).
fn accepted_run(run: &ragent_types::AutomationRun) -> Response {
    (
        StatusCode::ACCEPTED,
        Json(json!({
            "run_id": run.id,
            "automation_id": run.automation_id,
            "trigger": run.trigger.as_str(),
            "backend": run.backend,
        })),
    )
        .into_response()
}

/// `GET /automation` - list configured automations (FR-014, FR-018).
async fn list_automations(State(state): State<AppState>) -> Response {
    let svc = service(&state).await;
    let now = Utc::now();
    let rows: Vec<Value> = svc
        .definitions()
        .iter()
        .map(|def| {
            let next_due = def
                .trigger
                .schedule()
                .and_then(|expr| compute_next_due(expr, now))
                .map(|t| t.to_rfc3339());
            json!({
                "id": def.id,
                "name": def.display_name(),
                "trigger": def.trigger.label(),
                "schedule": def.trigger.schedule(),
                "backend": def.backend_label(),
                "enabled": def.enabled,
                "next_due": next_due,
                "running": svc.is_running(&def.id),
            })
        })
        .collect();
    serialize_response(&rows, "automation list").into_response()
}

/// `GET /automation/runs/{id}` - an automation's run history, newest first
/// (FR-018).
async fn list_runs(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let svc = service(&state).await;
    match svc.list_runs(&id, 100).await {
        Ok(runs) => serialize_response(&runs, "automation runs").into_response(),
        Err(e) => internal_error_response("automation runs", e).into_response(),
    }
}

/// `POST /automation/{id}/run` - enqueue a manual run (FR-018).
async fn run_now(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let svc = Arc::new(service(&state).await);
    if let Some(response) = guard_automation(&svc, &id) {
        return response;
    }
    match svc.run_now(&id).await {
        Ok(run) => accepted_run(&run),
        Err(e) => internal_error_response("automation run", e).into_response(),
    }
}

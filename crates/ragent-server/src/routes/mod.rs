//! HTTP route handlers and server setup.
//!
//! Defines the Axum router, shared [`AppState`], and all REST/SSE endpoint
//! handlers for session management, messaging, permissions, and configuration.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::StatusCode,
    middleware,
    response::{
        IntoResponse, Response,
        sse::{Event as SseEvent, KeepAlive, Sse},
    },
    routing::{get, post},
};
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;
use tower_http::cors::CorsLayer;

use ragent_core::{
    agent::{self, AgentInfo},
    config::Config,
    event::{Event, EventBus},
    sanitize::redact_secrets,
    session::processor::SessionProcessor,
    storage::{SessionRow, Storage},
    task::TaskManager,
};

use crate::sse::event_to_sse;

/// Shared application state passed to every Axum handler.
#[derive(Clone)]
pub struct AppState {
    /// Broadcast bus for server-sent events (SSE) to connected clients.
    pub event_bus: Arc<EventBus>,
    /// Application configuration, behind an async read-write lock.
    pub config: Arc<tokio::sync::RwLock<Config>>,
    /// Persistent storage backend for sessions and related data.
    pub storage: Arc<Storage>,
    /// Processor responsible for running chat sessions to completion.
    pub session_processor: Arc<SessionProcessor>,
    /// Bearer token required for authenticating incoming API requests.
    pub auth_token: String,
    /// Per-client rate limiter tracking request counts and window timestamps.
    pub rate_limiter: Arc<std::sync::Mutex<HashMap<String, (u32, Instant)>>>,
}

/// Bind to `addr` and serve the ragent HTTP/SSE API.
///
/// # Errors
///
/// Returns an error if the TCP listener cannot bind or the server fails.
///
/// # Examples
///
/// ```rust,no_run
/// # use ragent_server::routes::{start_server, AppState};
/// # async fn example(state: AppState) -> anyhow::Result<()> {
/// start_server("127.0.0.1:3000", state).await?;
/// # Ok(())
/// # }
/// ```
pub async fn start_server(addr: &str, state: AppState) -> anyhow::Result<()> {
    tracing::info!("Server auth token: {}", state.auth_token);
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Server listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

/// Build the Axum [`Router`] with all ragent API routes and middleware.
///
/// # Examples
///
/// ```rust,no_run
/// # use ragent_server::routes::{router, AppState};
/// # fn example(state: AppState) {
/// let app = router(state);
/// // `app` is an axum::Router ready to be served
/// # }
/// ```
pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/config", get(get_config))
        .route("/providers", get(get_providers))
        .route("/sessions", get(list_sessions).post(create_session))
        .route("/sessions/{id}", get(get_session).delete(archive_session))
        .route(
            "/sessions/{id}/messages",
            get(get_messages).post(send_message),
        )
        .route("/sessions/{id}/abort", post(abort_session))
        .route("/sessions/{id}/permission/{req_id}", post(reply_permission))
        .route(
            "/sessions/{id}/tasks",
            get(list_tasks).post(spawn_task),
        )
        .route(
            "/sessions/{id}/tasks/{tid}",
            get(get_task).delete(cancel_task),
        )
        .route("/events", get(events_stream))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .route("/health", get(health))
        .merge(protected)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: middleware::Next,
) -> Response {
    let expected = format!("Bearer {}", state.auth_token);
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header == expected => next.run(request).await,
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "unauthorized" })),
        )
            .into_response(),
    }
}

// ── Handlers ──────────────────────────────────────────────────────

async fn health() -> &'static str {
    "ok"
}

async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read().await;
    Json(config.clone())
}

async fn get_providers(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let config = state.config.read().await;
    let provider_ids: Vec<String> = config.provider.keys().cloned().collect();
    serialize_response(provider_ids, "get_providers")
}

#[derive(Serialize)]
struct SessionResponse {
    id: String,
    title: String,
    directory: String,
    created_at: String,
    updated_at: String,
    summary: Option<String>,
}

impl From<SessionRow> for SessionResponse {
    fn from(row: SessionRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            directory: row.directory,
            created_at: row.created_at,
            updated_at: row.updated_at,
            summary: row.summary,
        }
    }
}

async fn list_sessions(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    match state.storage.list_sessions() {
        Ok(sessions) => {
            let resp: Vec<SessionResponse> = sessions.into_iter().map(Into::into).collect();
            serialize_response(resp, "list_sessions")
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[derive(Deserialize)]
struct CreateSessionRequest {
    directory: String,
}

async fn create_session(
    State(state): State<AppState>,
    Json(body): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let path = std::path::Path::new(&body.directory);
    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("Invalid directory: {e}") })),
            )
                .into_response();
        }
    };
    if !canonical.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Path is not a directory" })),
        )
            .into_response();
    }
    let directory = canonical.display().to_string();
    let id = uuid::Uuid::new_v4().to_string();
    match state.storage.create_session(&id, &directory) {
        Ok(()) => {
            state.event_bus.publish(Event::SessionCreated {
                session_id: id.clone(),
            });
            (
                StatusCode::CREATED,
                Json(serde_json::json!({ "id": id, "directory": directory })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn get_session(State(state): State<AppState>, Path(id): Path<String>) -> (StatusCode, Json<serde_json::Value>) {
    match state.storage.get_session(&id) {
        Ok(Some(session)) => {
            let resp: SessionResponse = session.into();
            serialize_response(resp, "get_session")
        }
        Ok(None) => error_response(StatusCode::NOT_FOUND, "session not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn archive_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.storage.archive_session(&id) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn get_messages(State(state): State<AppState>, Path(id): Path<String>) -> (StatusCode, Json<serde_json::Value>) {
    match state.storage.get_messages(&id) {
        Ok(messages) => serialize_response(messages, "get_messages"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[derive(Deserialize)]
struct SendMessageRequest {
    content: String,
}

async fn send_message(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SendMessageRequest>,
) -> Response {
    {
        let mut limiter = state.rate_limiter.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let entry = limiter.entry(id.clone()).or_insert((0, now));
        if now.duration_since(entry.1).as_secs() >= 60 {
            *entry = (1, now);
        } else {
            entry.0 += 1;
            if entry.0 > 60 {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(serde_json::json!({ "error": "rate limit exceeded: 60 requests per minute per session" })),
                )
                    .into_response();
            }
        }
    }

    let session_id = id.clone();
    let rx = state.event_bus.subscribe();
    let processor = state.session_processor.clone();
    let content = body.content.clone();
    let config = state.config.clone();

    tokio::spawn(async move {
        let cfg = config.read().await;
        let agent = agent::resolve_agent(&cfg.default_agent, &cfg)
            .unwrap_or_else(|_| AgentInfo::new("general", "General-purpose agent"));
        drop(cfg);
        if let Err(e) = processor
            .process_message(&session_id, &content, &agent, Arc::new(AtomicBool::new(false)))
            .await
        {
            tracing::error!(session_id = %session_id, error = %redact_secrets(&e.to_string()), "Failed to process message");
        }
    });

    let stream = BroadcastStream::new(rx).filter_map(move |result| {
        let session_id = id.clone();
        async move {
            match result {
                Ok(event) => {
                    if event_matches_session(&event, &session_id) {
                        Some(Ok::<_, std::convert::Infallible>(event_to_sse(&event)))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        }
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

async fn abort_session(State(state): State<AppState>, Path(id): Path<String>) -> (StatusCode, Json<serde_json::Value>) {
    match state.storage.get_session(&id) {
        Ok(Some(_)) => {
            if let Err(e) = state.storage.archive_session(&id) {
                tracing::error!(
                    session_id = %id,
                    error = %e,
                    "Failed to archive session during abort"
                );
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to archive session: {e}"),
                );
            }

            state.event_bus.publish(Event::SessionAborted {
                session_id: id.clone(),
                reason: "user_requested".to_string(),
            });

            tracing::info!(session_id = %id, "Session aborted");
            (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
        }
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Session not found"),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to look up session: {e}"),
        ),
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum PermissionReplyDecision {
    Allow,
    Deny,
}

#[derive(Deserialize)]
struct PermissionReply {
    decision: PermissionReplyDecision,
}

async fn reply_permission(
    State(state): State<AppState>,
    Path((id, req_id)): Path<(String, String)>,
    Json(body): Json<PermissionReply>,
) -> impl IntoResponse {
    let allowed = body.decision != PermissionReplyDecision::Deny;
    state.event_bus.publish(Event::PermissionReplied {
        session_id: id,
        request_id: req_id,
        allowed,
    });
    Json(serde_json::json!({ "ok": true }))
}

async fn events_stream(
    State(state): State<AppState>,
) -> Sse<impl futures::Stream<Item = Result<SseEvent, std::convert::Infallible>>> {
    let rx = state.event_bus.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| async move {
        match result {
            Ok(event) => Some(Ok(event_to_sse(&event))),
            Err(_) => None,
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ── Task Endpoints ────────────────────────────────────────────────

#[derive(Deserialize)]
struct SpawnTaskRequest {
    agent: String,
    task: String,
    background: Option<bool>,
    model: Option<String>,
}

#[derive(Serialize)]
struct TaskResponse {
    id: String,
    parent_session_id: String,
    agent_name: String,
    task_prompt: String,
    status: String,
    result: Option<String>,
    error: Option<String>,
    created_at: String,
    completed_at: Option<String>,
    background: bool,
}

async fn spawn_task(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(body): Json<SpawnTaskRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, Json<serde_json::Value>)> {
    // Verify session exists and get its directory
    let session = match state.storage.get_session(&session_id) {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Err(error_response(StatusCode::NOT_FOUND, "session not found"));
        }
        Err(e) => {
            return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
        }
    };

    let working_dir = std::path::Path::new(&session.directory);
    let background = body.background.unwrap_or(false);
    let task_manager = get_task_manager(&state)?;

    let result = if background {
        task_manager
            .spawn_background(
                &session_id,
                &body.agent,
                &body.task,
                body.model.as_deref(),
                working_dir,
            )
            .await
    } else {
        task_manager
            .spawn_sync(
                &session_id,
                &body.agent,
                &body.task,
                body.model.as_deref(),
                working_dir,
            )
            .await
            .map(|result| result.entry)
    };

    match result {
        Ok(entry) => {
            let response = task_entry_to_response(entry, background);
            Ok((StatusCode::CREATED, Json(response)))
        }
        Err(e) => Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

async fn list_tasks(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<(StatusCode, Json<Vec<TaskResponse>>), (StatusCode, Json<serde_json::Value>)> {
    // Verify session exists
    verify_session_exists(&state, &session_id).await?;

    let task_manager = get_task_manager(&state)?;

    let entries = task_manager.list_tasks(&session_id).await;
    let tasks: Vec<TaskResponse> = entries
        .into_iter()
        .map(|entry| task_entry_to_response(entry, false))
        .collect();
    Ok((StatusCode::OK, Json(tasks)))
}

async fn get_task(
    State(state): State<AppState>,
    Path((session_id, task_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, Json<serde_json::Value>)> {
    let task_manager = get_task_manager(&state)?;

    match task_manager.get_task(&task_id).await {
        Some(entry) => {
            if entry.parent_session_id != session_id {
                return Err(error_response(
                    StatusCode::FORBIDDEN,
                    "task does not belong to this session",
                ));
            }
            let response = task_entry_to_response(entry, false);
            Ok((StatusCode::OK, Json(response)))
        }
        None => Err(error_response(StatusCode::NOT_FOUND, "task not found")),
    }
}

async fn cancel_task(
    State(state): State<AppState>,
    Path((session_id, task_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let task_manager = get_task_manager(&state)?;

    // Verify task belongs to this session
    match task_manager.get_task(&task_id).await {
        Some(entry) => {
            if entry.parent_session_id != session_id {
                return Err(error_response(
                    StatusCode::FORBIDDEN,
                    "task does not belong to this session",
                ));
            }
        }
        None => {
            return Err(error_response(StatusCode::NOT_FOUND, "task not found"));
        }
    }

    match task_manager.cancel_task(&task_id).await {
        Ok(()) => Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true })))),
        Err(e) => Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// Helper to build standardized error JSON response bodies.
fn error_response(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({ "error": message.into() })))
}

/// Helper to serialize a value to JSON and return a response, or an internal server error.
fn serialize_response<T: serde::Serialize>(value: T, context: &str) -> (StatusCode, Json<serde_json::Value>) {
    match serde_json::to_value(&value) {
        Ok(val) => (StatusCode::OK, Json(val)),
        Err(e) => {
            tracing::warn!(error = %e, context, "Serialization failed");
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "serialization failed")
        }
    }
}

/// Helper to retrieve the task manager from session processor or return error response.
fn get_task_manager(state: &AppState) -> Result<Arc<TaskManager>, (StatusCode, Json<serde_json::Value>)> {
    state
        .session_processor
        .task_manager
        .get()
        .map(|tm| tm.clone())
        .ok_or_else(|| {
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "task manager not initialized")
        })
}

/// Helper to verify a session exists, returning an error response if it doesn't.
async fn verify_session_exists(state: &AppState, session_id: &str) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    match state.storage.get_session(session_id) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(error_response(StatusCode::NOT_FOUND, "session not found")),
        Err(e) => Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// Helper to convert a task entry to a TaskResponse.
fn task_entry_to_response(entry: ragent_core::task::TaskEntry, background: bool) -> TaskResponse {
    TaskResponse {
        id: entry.id.clone(),
        parent_session_id: entry.parent_session_id,
        agent_name: entry.agent_name,
        task_prompt: entry.task_prompt,
        status: format!("{}", entry.status),
        result: entry.result,
        error: entry.error,
        created_at: entry.created_at.to_rfc3339(),
        completed_at: entry.completed_at.map(|d| d.to_rfc3339()),
        background,
    }
}

fn event_matches_session(event: &Event, session_id: &str) -> bool {
    match event {
        Event::SessionCreated { session_id: sid }
        | Event::SessionUpdated { session_id: sid }
        | Event::MessageStart {
            session_id: sid, ..
        }
        | Event::TextDelta {
            session_id: sid, ..
        }
        | Event::ReasoningDelta {
            session_id: sid, ..
        }
        | Event::ToolCallStart {
            session_id: sid, ..
        }
        | Event::ToolCallEnd {
            session_id: sid, ..
        }
        | Event::MessageEnd {
            session_id: sid, ..
        }
        | Event::PermissionRequested {
            session_id: sid, ..
        }
        | Event::PermissionReplied {
            session_id: sid, ..
        }
        | Event::AgentSwitched {
            session_id: sid, ..
        }
        | Event::AgentSwitchRequested {
            session_id: sid, ..
        }
        | Event::AgentRestoreRequested {
            session_id: sid, ..
        }
        | Event::AgentError {
            session_id: sid, ..
        }
        | Event::TokenUsage {
            session_id: sid, ..
        }
        | Event::ToolsSent {
            session_id: sid, ..
        }
        | Event::ModelResponse {
            session_id: sid, ..
        }
        | Event::ToolCallArgs {
            session_id: sid, ..
        }
        | Event::ToolResult {
            session_id: sid, ..
        }
        | Event::SessionAborted {
            session_id: sid, ..
        }
        | Event::SubagentStart {
            session_id: sid, ..
        }
        | Event::SubagentComplete {
            session_id: sid, ..
        }
        | Event::SubagentCancelled {
            session_id: sid, ..
        } => sid == session_id,
        Event::McpStatusChanged { .. } => false,
        Event::CopilotDeviceFlowComplete { .. } => false,
        Event::LspStatusChanged { .. } => false,
    }
}

//! HTTP route handlers and server setup.
//!
//! Defines the Axum router, shared [`AppState`], and all REST/SSE endpoint
//! handlers for session management, messaging, permissions, and configuration.

use std::collections::HashMap;
use std::sync::Arc;
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
};

use crate::sse::event_to_sse;

/// Shared application state passed to every Axum handler.
#[derive(Clone)]
pub struct AppState {
    pub event_bus: Arc<EventBus>,
    pub config: Arc<tokio::sync::RwLock<Config>>,
    pub storage: Arc<Storage>,
    pub session_processor: Arc<SessionProcessor>,
    pub auth_token: String,
    pub rate_limiter: Arc<std::sync::Mutex<HashMap<String, (u32, Instant)>>>,
}

/// Bind to `addr` and serve the ragent HTTP/SSE API.
///
/// # Errors
///
/// Returns an error if the TCP listener cannot bind or the server fails.
pub async fn start_server(addr: &str, state: AppState) -> anyhow::Result<()> {
    tracing::info!("Server auth token: {}", state.auth_token);
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Server listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

/// Build the Axum [`Router`] with all ragent API routes and middleware.
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

async fn get_providers(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read().await;
    let provider_ids: Vec<&String> = config.provider.keys().collect();
    Json(serde_json::to_value(provider_ids).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "Failed to serialize provider list");
        serde_json::json!([])
    }))
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

async fn list_sessions(State(state): State<AppState>) -> impl IntoResponse {
    match state.storage.list_sessions() {
        Ok(sessions) => {
            let resp: Vec<SessionResponse> = sessions.into_iter().map(Into::into).collect();
            match serde_json::to_value(&resp) {
                Ok(val) => (StatusCode::OK, Json(val)).into_response(),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to serialize session list");
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "serialization failed" }))).into_response()
                }
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
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

async fn get_session(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.storage.get_session(&id) {
        Ok(Some(session)) => {
            let resp: SessionResponse = session.into();
            match serde_json::to_value(&resp) {
                Ok(val) => (StatusCode::OK, Json(val)).into_response(),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to serialize session");
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "serialization failed" }))).into_response()
                }
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "session not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn archive_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.storage.archive_session(&id) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn get_messages(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.storage.get_messages(&id) {
        Ok(messages) => match serde_json::to_value(&messages) {
            Ok(val) => (StatusCode::OK, Json(val)).into_response(),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to serialize messages");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "serialization failed" }))).into_response()
            }
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
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
        let mut limiter = state
            .rate_limiter
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
            .process_message(&session_id, &content, &agent)
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

async fn abort_session(Path(_id): Path<String>) -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true }))
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

// ── Helpers ──────────────────────────────────────────────────────

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
        | Event::AgentError {
            session_id: sid, ..
        }
        | Event::TokenUsage {
            session_id: sid, ..
        } => sid == session_id,
        Event::McpStatusChanged { .. } => false,
    }
}

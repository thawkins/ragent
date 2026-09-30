//! HTTP route handlers and server setup.
//!
//! Defines the Axum router, shared [`AppState`], and all REST/SSE endpoint
//! handlers for session management, messaging, permissions, and configuration.

pub mod memory;
pub mod research;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware,
    response::{
        IntoResponse, Response,
        sse::{KeepAlive, Sse},
    },
    routing::{get, post},
};
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::timeout::TimeoutLayer;

use ragent_agent::{
    Config,
    agent::{self, AgentInfo},
    event::{Event, EventBus},
    sanitize::redact_secrets,
    session::processor::SessionProcessor,
    storage::{SessionRow, Storage},
    task::AgentManager,
};

use crate::sse::event_to_sse;
use ragent_research::SessionEvent;

/// Maximum request body size accepted by every API route (F-M10).
///
/// 10 MiB comfortably covers the research POST body (~40 optional fields) and
/// large edit payloads while bounding memory use; it replaces Axum's implicit
/// 2 MiB default with an explicit, documented limit.
const MAX_REQUEST_BODY_BYTES: usize = 10 * 1024 * 1024;

/// Per-request server-side deadline (F-M10).
///
/// Bounds any handler, including the multi-second `spawn_blocking` storage
/// work behind the research endpoints, so a stalled downstream cannot pin a
/// connection indefinitely.
const REQUEST_TIMEOUT_SECS: u64 = 60;

/// Maximum concurrent SSE connections across the process (F-M9).
///
/// Every connected client holds a broadcast receiver plus a streaming task, so
/// `/events` fan-out must be bounded. Connections beyond this cap are rejected
/// with `503 Service Unavailable`.
const MAX_SSE_CONNECTIONS: usize = 64;

/// Requests permitted per rolling minute per rate-limit key (F-L1).
///
/// Referenced by the limiter condition and the error message so the advertised
/// limit and the enforced limit cannot drift apart.
const RATE_LIMIT_PER_MINUTE: u32 = 60;

/// Origins permitted to make cross-origin requests to the API (F-M8).
///
/// The API exposes `/config` and session mutation endpoints behind a bearer
/// token, so a wildcard CORS policy would let any web page the operator visits
/// script those routes. The allowlist is restricted to loopback origins used by
/// the bundled dev UI; no deployment should widen it without an explicit config
/// hook.
const ALLOWED_ORIGINS: [HeaderValue; 4] = [
    HeaderValue::from_static("http://localhost:3000"),
    HeaderValue::from_static("http://127.0.0.1:3000"),
    HeaderValue::from_static("http://localhost:9100"),
    HeaderValue::from_static("http://127.0.0.1:9100"),
];

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
    /// Uses `tokio::sync::Mutex` to avoid blocking the async runtime.
    /// Entries older than 120 seconds are evicted on access.
    pub rate_limiter: Arc<tokio::sync::Mutex<HashMap<String, (u32, Instant)>>>,
    /// Optional in-process coordinator for orchestration features.
    pub coordinator: Option<ragent_agent::orchestrator::Coordinator>,
    /// In-memory registry of running background research sessions.
    /// Keyed by research name; stores the broadcast channel for SSE events.
    pub research_runs:
        Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::broadcast::Sender<SessionEvent>>>>,
}

/// Count of currently open `/events` SSE connections across the process (F-M9).
///
/// Bounded by [`MAX_SSE_CONNECTIONS`]: a slot is reserved when a client
/// connects and released when its stream is dropped, so the fan-out cannot grow
/// without limit. A process-global counter is sufficient because the server
/// runs a single shared event bus per process.
static SSE_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

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
    tracing::info!("Server auth token configured");
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
        .route("/sessions/{id}/tasks", get(list_agents).post(spawn_task))
        .route(
            "/sessions/{id}/tasks/{tid}",
            get(get_task).delete(cancel_agent),
        )
        .route("/events", get(events_stream))
        // Memory API (Milestone 8)
        .nest("/memory", memory::memory_routes())
        // Research API (research system)
        .nest("/research", research::research_routes())
        // Orchestration endpoints (Milestone 3 - Task 3.1)
        .route("/orchestrator/start", post(orch_start))
        .route("/orchestrator/jobs/{id}", get(orch_job))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Serve static web UI files from the embedded static directory.
    // Axum >= 0.8 rejects `nest_service("/")` ("Nesting at the root is no
    // longer supported"), so the static tree is attached as the fallback for
    // any path that matched no API route above.
    let static_files =
        ServeDir::new(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/static"))
            .append_index_html_on_directories(true);

    // F-M8: an explicit loopback-only origin policy replaces
    // `CorsLayer::permissive()`. Only the verbs and headers the REST/SSE API
    // actually uses are allowed, and credentials are never reflected.
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(ALLOWED_ORIGINS))
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT]);

    Router::new()
        .route("/health", get(health))
        .merge(protected)
        .fallback_service(static_files)
        // F-M10: bound the request body and give every handler a deadline.
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(REQUEST_TIMEOUT_SECS),
        ))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(cors)
        .with_state(state)
}

/// Compare two secret strings in constant time.
///
/// FUNC-066: hash both sides to a fixed 32-byte BLAKE3 digest before comparing.
/// The previous length-early-return leaked the token length via timing, and its
/// byte loop stopped at the shorter slice. A fixed-length digest means the
/// comparison length is constant regardless of input length, and the byte loop
/// always runs the full digest. `res` accumulates every differing bit so the
/// loop cannot short-circuit.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let da = blake3::hash(a.as_bytes());
    let db = blake3::hash(b.as_bytes());
    let da = da.as_bytes();
    let db = db.as_bytes();
    let mut res: u8 = 0;
    for (x, y) in da.iter().zip(db.iter()) {
        res |= x ^ y;
    }
    res == 0
}

async fn auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: middleware::Next,
) -> Response {
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let provided = auth_header.and_then(|h| {
        h.get(..7)
            .filter(|p| p.eq_ignore_ascii_case("bearer "))
            .map(|_| &h[7..])
    });

    match provided {
        Some(token) if constant_time_eq(token, &state.auth_token) => next.run(request).await,
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
    // SEC-ragent-server-001 (SECTASKS T-015): never return a credential in a
    // response body. `Config` derives `Serialize` with no `skip_serializing`
    // on any secret field, so serialising it directly would hand every stored
    // API key / PAT / bot token / OAuth client secret to any authenticated
    // caller. Serialise, then replace each credential value in place.
    let mut value = match serde_json::to_value(&*config) {
        Ok(value) => value,
        Err(e) => {
            tracing::error!(error = %e, "failed to serialise config for GET /config");
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to serialise configuration".to_string(),
            )
            .into_response();
        }
    };
    redact_config_secrets(&mut value);
    Json(value).into_response()
}

/// Placeholder written in place of every credential in a config payload.
const REDACTED: &str = "<redacted>";

/// Replace every credential value in a serialised [`Config`] with
/// [`REDACTED`].
///
/// SEC-ragent-server-001 (SECTASKS T-015). The set of paths mirrors the
/// credential fields the config schema can hold: the search-engine API keys,
/// the GitLab PAT, the Telegram bot token, the Discord webhook URL, and the
/// Gmail OAuth client secret. Anything not on this list is returned as-is.
fn redact_config_secrets(value: &mut serde_json::Value) {
    /// Mask `object[key]` when it is a non-empty string.
    fn mask(object: &mut serde_json::Value, key: &str) {
        let Some(map) = object.as_object_mut() else {
            return;
        };
        if let Some(entry) = map.get_mut(key) {
            let is_present = entry
                .as_str()
                .is_some_and(|s| !s.trim().is_empty() && s != REDACTED);
            if is_present {
                *entry = serde_json::Value::String(REDACTED.to_string());
            }
        }
    }

    const TOP_LEVEL: &[&str] = &[
        "tavily_api_key",
        "langsearch_api_key",
        "perplexity_api_key",
        "exa_api_key",
        "serper_api_key",
    ];
    for key in TOP_LEVEL {
        mask(value, key);
    }

    // Nested credential holders, walked only to depth 2 so a deeply nested
    // attacker-controlled key cannot make the walk expensive.
    for section in ["gitlab", "gmail"] {
        if let Some(node) = value.get_mut(section) {
            for key in ["token", "client_secret", "client_id", "refresh_token"] {
                mask(node, key);
            }
        }
    }
    if let Some(channels) = value.get_mut("channels") {
        if let Some(telegram) = channels.get_mut("telegram") {
            mask(telegram, "bot_token");
        }
        if let Some(discord) = channels.get_mut("discord") {
            mask(discord, "webhook_url");
        }
    }
    if let Some(provider) = value.get_mut("provider") {
        if let Some(map) = provider.as_object_mut() {
            for (_, entry) in map.iter_mut() {
                for key in ["api_key", "token"] {
                    mask(entry, key);
                }
            }
        }
    }
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
    let storage = Arc::clone(&state.storage);
    match tokio::task::spawn_blocking(move || storage.list_sessions()).await {
        Ok(Ok(sessions)) => {
            let resp: Vec<SessionResponse> = sessions.into_iter().map(Into::into).collect();
            serialize_response(resp, "list_sessions")
        }
        Ok(Err(e)) => internal_error_response("list_sessions", e),
        Err(e) => internal_error_response("list_sessions", e),
    }
}

#[derive(Deserialize)]
struct CreateSessionRequest {
    directory: String,
}

#[tracing::instrument(skip(state, body))]
async fn create_session(
    State(state): State<AppState>,
    Json(body): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    // F-L9: throttle session creation through the shared limiter. There is no
    // session id yet, so all creations share one fixed bucket.
    if let Some(rate_limited) = check_rate_limit(&state, "create_session").await {
        return rate_limited.into_response();
    }

    let path = std::path::Path::new(&body.directory);
    let canonical = match tokio::fs::canonicalize(path).await {
        Ok(p) => p,
        Err(e) => {
            // SEC-ragent-server-008 (SECTASKS T-064): `canonicalize`'s error
            // embeds the resolved path component, so log it and answer
            // generically.
            tracing::warn!(error = %e, "session directory rejected");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid directory" })),
            )
                .into_response();
        }
    };
    let is_dir = tokio::fs::metadata(&canonical)
        .await
        .is_ok_and(|m| m.is_dir());
    if !is_dir {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid directory" })),
        )
            .into_response();
    }
    let directory = canonical.display().to_string();
    let id = uuid::Uuid::new_v4().to_string();
    let storage = Arc::clone(&state.storage);
    let id_for_spawn = id.clone();
    let dir_for_spawn = directory.clone();
    match tokio::task::spawn_blocking(move || storage.create_session(&id_for_spawn, &dir_for_spawn))
        .await
    {
        Ok(Ok(())) => {
            state.event_bus.publish(Event::SessionCreated {
                session_id: id.clone(),
            });
            (
                StatusCode::CREATED,
                Json(serde_json::json!({ "id": id, "directory": directory })),
            )
                .into_response()
        }
        Ok(Err(e)) => internal_error_response("create_session", e).into_response(),
        Err(e) => internal_error_response("create_session (task)", e).into_response(),
    }
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let storage = Arc::clone(&state.storage);
    match tokio::task::spawn_blocking(move || storage.get_session(&id)).await {
        Ok(Ok(Some(session))) => {
            let resp: SessionResponse = session.into();
            serialize_response(resp, "get_session")
        }
        Ok(Ok(None)) => error_response(StatusCode::NOT_FOUND, "session not found"),
        Ok(Err(e)) => internal_error_response("get_session", e),
        Err(e) => internal_error_response("get_session", e),
    }
}

async fn archive_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let storage = Arc::clone(&state.storage);
    match tokio::task::spawn_blocking(move || storage.archive_session(&id)).await {
        Ok(Ok(())) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))),
        Ok(Err(e)) => internal_error_response("archive_session", e),
        Err(e) => internal_error_response("archive_session", e),
    }
}

async fn get_messages(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let storage = Arc::clone(&state.storage);
    match tokio::task::spawn_blocking(move || storage.get_messages(&id)).await {
        Ok(Ok(messages)) => serialize_response(messages, "get_messages"),
        Ok(Err(e)) => internal_error_response("get_messages", e),
        Err(e) => internal_error_response("get_messages", e),
    }
}

#[derive(Deserialize)]
struct SendMessageRequest {
    content: String,
}

#[tracing::instrument(skip(state, body), fields(session_id = %id))]
async fn send_message(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SendMessageRequest>,
) -> Response {
    if let Some(rate_limited) = check_rate_limit(&state, &id).await {
        return rate_limited.into_response();
    }

    let session_id = id.clone();
    let rx = state.event_bus.subscribe();
    let processor = state.session_processor.clone();
    let content = body.content;
    let config = state.config;
    let provider_registry = state.session_processor.provider_registry.clone();

    tokio::spawn(async move {
        let cfg = config.read().await;
        let agent = agent::resolve_agent_with_model(&cfg.default_agent, &cfg, &provider_registry)
            .unwrap_or_else(|_| Arc::new(AgentInfo::new("general", "General-purpose agent")));
        drop(cfg);
        if let Err(e) = processor
            .process_message(
                &session_id,
                &content,
                &agent,
                Arc::new(AtomicBool::new(false)),
            )
            .await
        {
            tracing::error!(session_id = %session_id, error = %redact_secrets(&e.to_string()), "Failed to process message");
        }
    });

    // PERF-056: clone the session id once per connection; the filter closure
    // borrows it, and dropped (Lagged) events are counted and warned.
    let sse_session_id = id.clone();
    let lagged = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let stream = BroadcastStream::new(rx).filter_map({
        let lagged = Arc::clone(&lagged);
        move |result| {
            let mapped = match result {
                Ok(event) => {
                    if event_matches_session(&event, &sse_session_id) {
                        Some(Ok::<_, std::convert::Infallible>(event_to_sse(&event)))
                    } else {
                        None
                    }
                }
                Err(err) => {
                    let dropped = lagged.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    tracing::warn!(
                        session_id = %sse_session_id,
                        dropped_batches = dropped,
                        error = %err,
                        "session SSE client lagged; events dropped"
                    );
                    None
                }
            };
            std::future::ready(mapped)
        }
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// `POST /sessions/{id}/abort` - archive the session and broadcast a
/// [`Event::SessionAborted`].
///
/// F-L8: this route intentionally shares the archive path with
/// `DELETE /sessions/{id}`. There is no separate agent-side cancellation
/// surface for a session (the session processor's per-message loop is
/// detached), so "abort" means archive + an explicit `SessionAborted` event;
/// clients waiting on that event get the same effect as a delete. The route is
/// kept as public API and must not be removed.
#[tracing::instrument(skip(state), fields(session_id = %id))]
async fn abort_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let storage = Arc::clone(&state.storage);
    let id_for_check = id.clone();
    match tokio::task::spawn_blocking(move || storage.get_session(&id_for_check)).await {
        Ok(Ok(Some(_))) => {
            let storage2 = Arc::clone(&state.storage);
            let id2 = id.clone();
            match tokio::task::spawn_blocking(move || storage2.archive_session(&id2)).await {
                Ok(Ok(())) => {
                    state.event_bus.publish(Event::SessionAborted {
                        session_id: id.clone(),
                        reason: "user_requested".to_string(),
                    });
                    tracing::info!(session_id = %id, "Session aborted");
                    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
                }
                Ok(Err(e)) => {
                    tracing::error!(
                        session_id = %id,
                        error = %e,
                        "Failed to archive session during abort"
                    );
                    internal_error_response("abort_session (archive)", e)
                }
                Err(e) => internal_error_response("abort_session (archive task)", e),
            }
        }
        Ok(Ok(None)) => error_response(StatusCode::NOT_FOUND, "session not found"),
        Ok(Err(e)) => internal_error_response("abort_session (lookup)", e),
        Err(e) => internal_error_response("abort_session (lookup task)", e),
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum PermissionReplyDecision {
    Allow,
    Always,
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
    let decision = match body.decision {
        PermissionReplyDecision::Allow => ragent_agent::permission::PermissionDecision::Once,
        PermissionReplyDecision::Always => ragent_agent::permission::PermissionDecision::Always,
        PermissionReplyDecision::Deny => ragent_agent::permission::PermissionDecision::Deny,
    };
    state.event_bus.publish(Event::PermissionReplied {
        session_id: id,
        request_id: req_id,
        allowed,
        decision,
    });
    Json(serde_json::json!({ "ok": true }))
}

/// Query string accepted by `GET /events`.
#[derive(Debug, Default, Deserialize)]
struct EventsStreamQuery {
    /// Optional session id. When present, only events belonging to that
    /// session are forwarded (F-M9); when absent the stream is admin-wide and
    /// forwards every process-wide [`Event`].
    session: Option<String>,
}

/// `GET /events` - process-wide server-sent event stream.
///
/// Scope (F-M9): by default this endpoint is **admin-wide** and forwards every
/// process-wide [`Event`] to the authenticated client, regardless of session.
/// Pass `?session=<id>` to receive only that session's events (filtered with
/// [`event_matches_session`], matching the per-message stream). Concurrent
/// connections are capped at [`MAX_SSE_CONNECTIONS`]; beyond the cap the
/// request is rejected with `503 Service Unavailable`.
#[tracing::instrument(skip(state))]
async fn events_stream(
    State(state): State<AppState>,
    Query(query): Query<EventsStreamQuery>,
) -> Response {
    // F-M9: reserve a slot in the connection budget before subscribing, so a
    // flood of clients cannot each hold a broadcast receiver unboundedly.
    let reserved = SSE_CONNECTIONS.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
        (n < MAX_SSE_CONNECTIONS).then_some(n + 1)
    });
    if reserved.is_err() {
        tracing::warn!(
            limit = MAX_SSE_CONNECTIONS,
            "SSE connection limit reached; rejecting /events client"
        );
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "too many concurrent SSE connections",
        )
        .into_response();
    }

    let rx = state.event_bus.subscribe();
    let lagged = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let session_filter = query.session;
    let stream = BroadcastStream::new(rx).filter_map({
        let lagged = Arc::clone(&lagged);
        move |result| {
            let mapped = match result {
                Ok(event) => match session_filter.as_deref() {
                    Some(sid) if !event_matches_session(&event, sid) => None,
                    _ => Some(Ok::<_, std::convert::Infallible>(event_to_sse(&event))),
                },
                Err(err) => {
                    let dropped = lagged.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    tracing::warn!(
                        dropped_batches = dropped,
                        error = %err,
                        "events SSE client lagged; events dropped"
                    );
                    None
                }
            };
            std::future::ready(mapped)
        }
    });

    // Release the reserved slot when the stream is dropped (client disconnect).
    let tracked = TrackedSseStream {
        inner: stream,
        _guard: DropGuard,
    };
    Sse::new(tracked)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// An SSE stream tagged with a connection-count [`DropGuard`] (F-M9).
///
/// Holds a reserved slot in the global SSE connection budget for as long as the
/// stream (and thus the client connection) is alive, releasing it on drop.
struct TrackedSseStream<S> {
    inner: S,
    _guard: DropGuard,
}

impl<S> futures::Stream for TrackedSseStream<S>
where
    S: futures::Stream + Unpin,
{
    type Item = S::Item;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::pin::Pin::new(&mut self.inner).poll_next(cx)
    }
}

/// Decrements [`SSE_CONNECTIONS`] exactly once when dropped (F-M9).
struct DropGuard;

impl Drop for DropGuard {
    fn drop(&mut self) {
        SSE_CONNECTIONS.fetch_sub(1, Ordering::AcqRel);
    }
}

// ── Orchestration (Milestone 3) ───────────────────────────────────

/// Request body for `POST /orchestrator/start`.
#[derive(Deserialize)]
struct OrchestrateRequest {
    /// Optional job id; a UUID is generated when absent.
    id: Option<String>,
    /// Capability tags used to match agents for the job.
    required_capabilities: Vec<String>,
    /// Payload forwarded verbatim to every matched agent.
    payload: String,
    /// `"sync"` waits for all agents; `"async"` (default) returns immediately.
    mode: Option<String>,
}

/// `POST /orchestrator/start` - start a multi-agent job.
async fn orch_start(
    State(state): State<AppState>,
    Json(body): Json<OrchestrateRequest>,
) -> impl IntoResponse {
    // F-L9: throttle orchestrator starts through the shared limiter, keyed by
    // the requested job id (or the shared "orchestrator" bucket for generated
    // ids) so a single client cannot flood job creation.
    let rate_key = body
        .id
        .clone()
        .unwrap_or_else(|| "orchestrator".to_string());
    if let Some(rate_limited) = check_rate_limit(&state, &rate_key).await {
        return rate_limited.into_response();
    }

    let coord = match &state.coordinator {
        Some(c) => c.clone(),
        None => {
            return error_response(StatusCode::SERVICE_UNAVAILABLE, "orchestrator not enabled")
                .into_response();
        }
    };

    let job_id = body.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let desc = ragent_agent::orchestrator::JobDescriptor {
        id: job_id.clone(),
        required_capabilities: body.required_capabilities,
        payload: body.payload,
    };

    let mode = body.mode.unwrap_or_else(|| "async".to_string());
    if mode == "sync" {
        match coord.start_job_sync(desc).await {
            Ok(result) => (
                StatusCode::OK,
                Json(serde_json::json!({ "job_id": job_id, "result": result })),
            )
                .into_response(),
            Err(e) => internal_error_response("start_job_sync", e).into_response(),
        }
    } else {
        match coord.start_job_async(desc).await {
            Ok(id) => (
                StatusCode::ACCEPTED,
                Json(serde_json::json!({ "job_id": id })),
            )
                .into_response(),
            Err(e) => internal_error_response("start_job_async", e).into_response(),
        }
    }
}

/// `GET /orchestrator/jobs/{id}` - poll job status / result.
async fn orch_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match &state.coordinator {
        Some(c) => match c.get_job_result(&id).await {
            Some((status, result)) => (
                StatusCode::OK,
                Json(serde_json::json!({ "id": id, "status": status, "result": result })),
            ),
            None => error_response(StatusCode::NOT_FOUND, "job not found"),
        },
        None => error_response(StatusCode::SERVICE_UNAVAILABLE, "orchestrator not enabled"),
    }
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

#[tracing::instrument(skip(state, body), fields(session_id = %session_id))]
async fn spawn_task(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(body): Json<SpawnTaskRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, Json<serde_json::Value>)> {
    // F-L9: share the per-session rate-limit budget with `send_message`.
    if let Some(rate_limited) = check_rate_limit(&state, &session_id).await {
        return Err(rate_limited);
    }

    // Verify session exists and get its directory
    let storage = Arc::clone(&state.storage);
    let sid = session_id.clone();
    let session = match tokio::task::spawn_blocking(move || storage.get_session(&sid)).await {
        Ok(Ok(Some(s))) => s,
        Ok(Ok(None)) => {
            return Err(error_response(StatusCode::NOT_FOUND, "session not found"));
        }
        Ok(Err(e)) => {
            return Err(internal_error_response("spawn_task (session lookup)", e));
        }
        Err(e) => {
            return Err(internal_error_response(
                "spawn_task (session lookup task)",
                e,
            ));
        }
    };

    let working_dir = std::path::Path::new(&session.directory);
    let background = body.background.unwrap_or(false);
    let agent_manager = get_agent_manager(&state)?;

    let result = if background {
        agent_manager
            .spawn_background(
                &session_id,
                &body.agent,
                &body.task,
                body.model.as_deref(),
                working_dir,
            )
            .await
    } else {
        agent_manager
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
        Err(e) => Err(internal_error_response("spawn_task", e)),
    }
}

async fn list_agents(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<(StatusCode, Json<Vec<TaskResponse>>), (StatusCode, Json<serde_json::Value>)> {
    // Verify session exists
    verify_session_exists(&state, &session_id).await?;

    let agent_manager = get_agent_manager(&state)?;

    let entries = agent_manager.list_agents(&session_id).await;
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
    let agent_manager = get_agent_manager(&state)?;

    match agent_manager.get_task(&task_id).await {
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

async fn cancel_agent(
    State(state): State<AppState>,
    Path((session_id, task_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let agent_manager = get_agent_manager(&state)?;

    // Verify task belongs to this session
    match agent_manager.get_task(&task_id).await {
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

    match agent_manager.cancel_agent(&task_id).await {
        Ok(()) => Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true })))),
        Err(e) => Err(internal_error_response("cancel_agent", e)),
    }
}

/// Helper to build standardized error JSON response bodies.
pub(crate) fn error_response(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({ "error": message.into() })))
}

/// Apply the per-session rate limit to a mutating request.
///
/// FUNC-066: enforce exactly 60 requests per rolling minute per session. The
/// previous code incremented *then* rejected on `> 60`, so the 61st request was
/// the first rejection on a warmed window but the count had already been
/// bumped - an off-by-one against the advertised 60. Check before incrementing
/// so the boundary matches the message.
///
/// F-L9: the limiter was previously inlined in `send_message` only. It is now a
/// shared helper used by every mutating session route (`send_message`,
/// `spawn_task`, `orch_start`), so they all share one budget. Returns `Some`
/// response when the caller is over the limit, `None` when the request may
/// proceed.
async fn check_rate_limit(
    state: &AppState,
    key: &str,
) -> Option<(StatusCode, Json<serde_json::Value>)> {
    let mut limiter = state.rate_limiter.lock().await;
    let now = Instant::now();

    // Evict stale entries older than 120 seconds to bound memory.
    const EVICTION_WINDOW_SECS: u64 = 120;
    const MAX_ENTRIES: usize = 10_000;
    if limiter.len() > MAX_ENTRIES {
        limiter.retain(|_, (_, ts)| now.duration_since(*ts).as_secs() < EVICTION_WINDOW_SECS);
    }

    let entry = limiter.entry(key.to_string()).or_insert((0, now));
    if now.duration_since(entry.1).as_secs() >= RATE_LIMIT_PER_MINUTE.into() {
        *entry = (1, now);
        None
    } else if entry.0 >= RATE_LIMIT_PER_MINUTE {
        Some(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            format!("rate limit exceeded: {RATE_LIMIT_PER_MINUTE} requests per minute per session"),
        ))
    } else {
        entry.0 += 1;
        None
    }
}

/// Return a generic internal error to the client and keep the detail in the log.
///
/// SEC-ragent-server-008 (SECTASKS T-064): handlers formatted storage and
/// filesystem errors straight into the JSON body, disclosing database paths,
/// SQLite error texts, and other implementation detail. `context` names the
/// failing operation for the log; the client is told only that it failed.
pub fn internal_error_response(
    context: &str,
    error: impl std::fmt::Display,
) -> (StatusCode, Json<serde_json::Value>) {
    tracing::error!(context, error = %error, "request failed");
    error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
}

/// Helper to serialize a value to JSON and return a response, or an internal server error.
pub(crate) fn serialize_response<T: serde::Serialize>(
    value: T,
    context: &str,
) -> (StatusCode, Json<serde_json::Value>) {
    match serde_json::to_value(&value) {
        Ok(val) => (StatusCode::OK, Json(val)),
        Err(e) => {
            tracing::warn!(error = %e, context, "Serialization failed");
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "serialization failed")
        }
    }
}

/// Helper to retrieve the agent manager from session processor or return error response.
fn get_agent_manager(
    state: &AppState,
) -> Result<Arc<AgentManager>, (StatusCode, Json<serde_json::Value>)> {
    state
        .session_processor
        .agent_manager
        .get()
        .cloned()
        .ok_or_else(|| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "agent manager not initialized",
            )
        })
}

/// Helper to verify a session exists, returning an error response if it doesn't.
async fn verify_session_exists(
    state: &AppState,
    session_id: &str,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let storage = Arc::clone(&state.storage);
    let sid = session_id.to_string();
    match tokio::task::spawn_blocking(move || storage.get_session(&sid)).await {
        Ok(Ok(Some(_))) => Ok(()),
        Ok(Ok(None)) => Err(error_response(StatusCode::NOT_FOUND, "session not found")),
        Ok(Err(e)) => Err(internal_error_response("verify_session_exists", e)),
        Err(e) => Err(internal_error_response("verify_session_exists (task)", e)),
    }
}

/// Helper to convert a task entry to a `TaskResponse`.
fn task_entry_to_response(entry: ragent_agent::task::TaskEntry, background: bool) -> TaskResponse {
    TaskResponse {
        id: entry.id.clone(),
        parent_session_id: entry.parent_session_id,
        agent_name: entry.agent_name,
        task_prompt: entry.task_prompt,
        status: format!("{}", entry.status),
        result: entry.result.map(|s| s.to_string()),
        error: entry.error.map(|s| s.to_string()),
        created_at: entry.created_at.to_rfc3339(),
        completed_at: entry.completed_at.map(|d| d.to_rfc3339()),
        background,
    }
}

fn event_matches_session(event: &Event, session_id: &str) -> bool {
    event.session_id() == Some(session_id)
}

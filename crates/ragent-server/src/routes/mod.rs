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
use prompt_opt::{Completer, OptMethod, optimize};

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
    pub coordinator: Option<ragent_core::orchestrator::Coordinator>,
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
        .route("/sessions/{id}/tasks", get(list_tasks).post(spawn_task))
        .route(
            "/sessions/{id}/tasks/{tid}",
            get(get_task).delete(cancel_task),
        )
        .route("/events", get(events_stream))
        .route("/opt", post(prompt_opt_handler))
        // Orchestration endpoints (Milestone 3 — Task 3.1)
        .route("/orchestrator/metrics", get(orch_metrics))
        .route("/orchestrator/start", post(orch_start))
        .route("/orchestrator/jobs/{id}", get(orch_job))
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
    // Local constant-time equality function to avoid timing attacks on token comparison.
    fn constant_time_eq(a: &str, b: &str) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let mut res: u8 = 0;
        for (x, y) in a.as_bytes().iter().zip(b.as_bytes().iter()) {
            res |= x ^ y;
        }
        res == 0
    }

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.len() > 7 && header[..7].eq_ignore_ascii_case("Bearer ") => {
            let provided = &header[7..];
            if constant_time_eq(provided, &state.auth_token) {
                next.run(request).await
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": "unauthorized" })),
                )
                    .into_response()
            }
        }
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

#[tracing::instrument(skip(state, body))]
async fn create_session(
    State(state): State<AppState>,
    Json(body): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let path = std::path::Path::new(&body.directory);
    let canonical = match tokio::fs::canonicalize(path).await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("Invalid directory: {e}") })),
            )
                .into_response();
        }
    };
    let is_dir = tokio::fs::metadata(&canonical)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);
    if !is_dir {
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

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
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

async fn get_messages(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.storage.get_messages(&id) {
        Ok(messages) => serialize_response(messages, "get_messages"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
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
    {
        let mut limiter = state.rate_limiter.lock().await;
        let now = Instant::now();

        // Evict stale entries older than 120 seconds to bound memory.
        const EVICTION_WINDOW_SECS: u64 = 120;
        const MAX_ENTRIES: usize = 10_000;
        if limiter.len() > MAX_ENTRIES {
            limiter.retain(|_, (_, ts)| now.duration_since(*ts).as_secs() < EVICTION_WINDOW_SECS);
        }

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
    let content = body.content;
    let config = state.config;

    tokio::spawn(async move {
        let cfg = config.read().await;
        let agent = agent::resolve_agent(&cfg.default_agent, &cfg)
            .unwrap_or_else(|_| AgentInfo::new("general", "General-purpose agent"));
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

#[tracing::instrument(skip(state), fields(session_id = %id))]
async fn abort_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
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

#[tracing::instrument(skip(state))]
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

/// `GET /orchestrator/metrics` — return live counter snapshot.
async fn orch_metrics(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    match &state.coordinator {
        Some(c) => {
            let snap = c.metrics_snapshot();
            serialize_response(snap, "orch_metrics")
        }
        None => error_response(StatusCode::SERVICE_UNAVAILABLE, "orchestrator not enabled"),
    }
}

/// `POST /orchestrator/start` — start a multi-agent job.
async fn orch_start(
    State(state): State<AppState>,
    Json(body): Json<OrchestrateRequest>,
) -> impl IntoResponse {
    let coord = match &state.coordinator {
        Some(c) => c.clone(),
        None => {
            return error_response(StatusCode::SERVICE_UNAVAILABLE, "orchestrator not enabled")
                .into_response();
        }
    };

    let job_id = body.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let desc = ragent_core::orchestrator::JobDescriptor {
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
            Err(e) => {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
        }
    } else {
        match coord.start_job_async(desc).await {
            Ok(id) => (
                StatusCode::ACCEPTED,
                Json(serde_json::json!({ "job_id": id })),
            )
                .into_response(),
            Err(e) => {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
        }
    }
}

/// `GET /orchestrator/jobs/{id}` — poll job status / result.
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
    // Verify session exists and get its directory
    let session = match state.storage.get_session(&session_id) {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Err(error_response(StatusCode::NOT_FOUND, "session not found"));
        }
        Err(e) => {
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            ));
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
        Err(e) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )),
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
        Err(e) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )),
    }
}

/// `POST /opt` — apply a prompt optimization method via the configured LLM.
///
/// Request body: `{ "method": "<name>", "prompt": "<text>", "provider": "<id>", "model": "<id>" }`
///
/// `provider` and `model` are optional; when absent the handler returns an
/// error asking the caller to supply them.  The canonical method names are
/// the lower-snake-case variants returned by [`OptMethod::all`]; common aliases
/// (e.g. `costar`, `co-star`, `q*`) are also accepted.
///
/// Returns: `{ "method": "<name>", "result": "<optimized prompt>" }`
#[derive(Deserialize)]
struct PromptOptRequest {
    method: String,
    prompt: String,
    /// Provider id (e.g. `"anthropic"`).  Required when the server has no default.
    provider: Option<String>,
    /// Model id (e.g. `"claude-sonnet-4-20250514"`).  Required when no default.
    model: Option<String>,
}

#[derive(Serialize)]
struct PromptOptResponse {
    method: String,
    result: String,
}

/// Implements [`Completer`] for the HTTP server by constructing an LLM client
/// from the [`AppState`]'s storage (for the API key) and a fresh
/// [`ragent_core::provider::ProviderRegistry`].
struct ServerCompleter {
    storage: Arc<Storage>,
    provider_id: String,
    model_id: String,
}

#[async_trait::async_trait]
impl Completer for ServerCompleter {
    async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String> {
        use anyhow::Context as _;
        use futures::StreamExt as _;
        use ragent_core::{
            llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent},
            provider::ProviderRegistry,
        };

        let api_key = self
            .storage
            .get_provider_auth(&self.provider_id)
            .context("reading API key")?
            .unwrap_or_default();

        let registry = ProviderRegistry::new();
        let provider = registry
            .get(&self.provider_id)
            .with_context(|| format!("provider '{}' not found", self.provider_id))?;

        let client = provider
            .create_client(&api_key, None, &Default::default())
            .await
            .context("creating LLM client")?;

        let request = ChatRequest {
            model: self.model_id.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text(user.to_string()),
            }],
            tools: vec![],
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: Some(system.to_string()),
            options: Default::default(),
            session_id: None,
            request_id: None,
        };

        let mut stream = client.chat(request).await.context("starting LLM stream")?;
        let mut result = String::new();
        while let Some(event) = stream.next().await {
            if let StreamEvent::TextDelta { text } = event {
                result.push_str(&text);
            }
        }
        Ok(result)
    }
}

async fn prompt_opt_handler(
    State(state): State<AppState>,
    Json(body): Json<PromptOptRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let input = body.prompt.trim();
    if input.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "prompt must not be empty");
    }

    let method = match OptMethod::from_str(&body.method) {
        Some(m) => m,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                format!("unknown optimization method: {}", body.method),
            );
        }
    };

    let provider_id = match body.provider {
        Some(p) => p,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "provider field is required (e.g. \"anthropic\")",
            );
        }
    };
    let model_id = match body.model {
        Some(m) => m,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "model field is required (e.g. \"claude-sonnet-4-20250514\")",
            );
        }
    };

    let completer = ServerCompleter {
        storage: Arc::clone(&state.storage),
        provider_id,
        model_id,
    };

    match optimize(method, input, &completer).await {
        Ok(result) => serialize_response(
            PromptOptResponse {
                method: body.method,
                result,
            },
            "prompt_opt",
        ),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

/// Helper to build standardized error JSON response bodies.
fn error_response(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({ "error": message.into() })))
}

/// Helper to serialize a value to JSON and return a response, or an internal server error.
fn serialize_response<T: serde::Serialize>(
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

/// Helper to retrieve the task manager from session processor or return error response.
fn get_task_manager(
    state: &AppState,
) -> Result<Arc<TaskManager>, (StatusCode, Json<serde_json::Value>)> {
    state
        .session_processor
        .task_manager
        .get()
        .cloned()
        .ok_or_else(|| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "task manager not initialized",
            )
        })
}

/// Helper to verify a session exists, returning an error response if it doesn't.
async fn verify_session_exists(
    state: &AppState,
    session_id: &str,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    match state.storage.get_session(session_id) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(error_response(StatusCode::NOT_FOUND, "session not found")),
        Err(e) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )),
    }
}

/// Helper to convert a task entry to a `TaskResponse`.
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
    event.session_id() == Some(session_id)
}

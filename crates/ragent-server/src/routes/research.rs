//! HTTP handlers for the research system (T-036, T-037, T-038, T-039).
//!
//! Exposes the `ragent-research` crate behind a thin REST surface:
//!
//! - `GET    /research`              — list every research item (FR-012)
//! - `POST   /research`              — create + run a gathering session
//! - `GET    /research/{name}`       — show one item
//! - `DELETE /research/{name}`       — remove an item (with confirmation)
//!
//! All endpoints are mounted under the auth-protected router in
//! `routes/mod.rs`.

use std::path::{Path as StdPath, PathBuf};
use std::sync::{Arc, LazyLock};

/// Cached research root: `cwd/research`.
///
/// The working directory does not change during server runtime, so we compute
/// it once and reuse the value for every request instead of calling
/// `std::env::current_dir()` (a syscall) on each handler invocation.
static RESEARCH_ROOT: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("research")
});

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::routes::AppState;

use ragent_agent::research_adapter::build_research_session;

use ragent_research::{ResearchManager, SearchHit, SessionEvent, SessionObserver};

/// Build the `/research` sub-router.
///
/// Routes are relative — the router is nested under `/research` in
/// `routes/mod.rs`, so the full paths become `/research`, `/research/{name}`,
/// and `/research/{name}/events`.
pub fn research_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_research).post(create_research))
        .route("/{name}", get(show_research).delete(delete_research))
        .route("/{name}/events", get(research_events_stream))
}

/// Compute the research root from the process working directory. The HTTP
/// server runs against the same project root as the CLI, so this is the
/// straightforward `cwd/research`.
fn research_root() -> PathBuf {
    RESEARCH_ROOT.clone()
}

// ── GET /research ────────────────────────────────────────────────────────

async fn list_research(State(_state): State<AppState>) -> impl IntoResponse {
    let manager = ResearchManager::new(research_root());
    match manager.list(false).await {
        Ok(items) => {
            let rows: Vec<ResearchItemRow> = items
                .into_iter()
                .map(|i| {
                    let sources = i.source_count();
                    ResearchItemRow {
                        name: i.name.to_string(),
                        title: i.title,
                        status: i.status.as_str().to_string(),
                        created_at: i.created_at.to_rfc3339(),
                        modified_at: i.modified_at.to_rfc3339(),
                        sources,
                        topic: None,
                        queries: Vec::new(),
                        output_format: None,
                        model: None,
                    }
                })
                .collect();
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "items": rows,
                    "count": rows.len(),
                })),
            )
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Serialize)]
struct ResearchItemRow {
    name: String,
    title: String,
    status: String,
    created_at: String,
    modified_at: String,
    sources: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    queries: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}

// ── POST /research ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateResearchRequest {
    name: String,
    topic: String,
    title: Option<String>,
    sources_dir: Option<String>,
    template: Option<String>,
    /// `--from-url <URL>`: fetch one or more URLs and use their content as the
    /// research subject in place of (or alongside) an explicit topic. Each
    /// page is captured as a primary source; web search still runs. Pass an
    /// array of URLs to seed multiple pages.
    #[serde(default)]
    from_urls: Vec<String>,
    /// `--from-file <PATH>`: extract one or more local documents and use their
    /// content as research subjects in place of (or alongside) an explicit
    /// topic. Each extracted file is captured as the primary `Source::Other`;
    /// web search still runs. Pass an array of paths to seed multiple files.
    /// If any referenced file is a PDF, PDF web sources are automatically
    /// enabled for the gather phase.
    #[serde(default)]
    from_files: Vec<String>,
    #[serde(default)]
    use_local: bool,
    #[serde(default)]
    use_specs: bool,
    /// `--use-low-relevance`: keep low-relevance web sources instead of
    /// filtering them out.
    #[serde(default)]
    use_low_relevance: bool,
    /// `--no-scholarly`: disable scholarly search engines (e.g. OpenAlex)
    /// during web gathering.
    #[serde(default)]
    no_scholarly: bool,
    /// `--use-pdf`: allow PDF documents returned by web search or `--from-url`
    /// to be captured as sources. By default PDF web sources are skipped.
    #[serde(default)]
    use_pdf: bool,
    /// Override the maximum number of candidate pages fetched in parallel
    /// during the web-gathering phase. When `None` the engine default
    /// (`ragent_research::DEFAULT_FETCH_CONCURRENCY`, 10) is used.
    fetch_concurrency: Option<usize>,
    /// Override the per-page fetch timeout in seconds. When `None` the engine
    /// default (30 seconds) is used.
    fetch_timeout_secs: Option<u64>,
    /// Override the maximum number of local candidate scoring/spec-scan tasks
    /// run in parallel. When `None` the engine default
    /// (`ragent_research::DEFAULT_LOCAL_CONCURRENCY`, 8) is used.
    local_concurrency: Option<usize>,
    /// `--depth shallow|standard|deep`. When omitted the engine behaves as
    /// `Depth::Standard` and stays single-pass.
    #[serde(default)]
    depth: Option<String>,
    /// `--iterations N` override. When supplied the iterative branch is used.
    #[serde(default)]
    iterations: Option<u32>,
    /// `--format report|executive-summary|comparison-table|source-bibliography|imrad`.
    #[serde(default)]
    format: Option<String>,
    /// `--tier light|full|dissertation`. When omitted the engine default
    /// (`Tier::Full`) is used.
    #[serde(default)]
    tier: Option<String>,
    /// Optional wall-clock timeout in seconds for the entire web-gathering
    /// phase.
    #[serde(default)]
    web_phase_timeout_secs: Option<u64>,
    /// Optional wall-clock timeout in seconds for the entire local-gathering
    /// phase.
    #[serde(default)]
    local_phase_timeout_secs: Option<u64>,
    /// Maximum retry attempts for a failed sub-query search. When `None` the
    /// engine default (`DEFAULT_SEARCH_MAX_RETRIES`, 2) is used.
    #[serde(default)]
    search_max_retries: Option<u32>,
    /// Base delay in milliseconds for the first search-retry backoff. When
    /// `None` the engine default (200 ms) is used.
    #[serde(default)]
    search_retry_base_delay_ms: Option<u64>,
    /// Number of consecutive search-tool failures after which the circuit
    /// breaker opens. When `None` the engine default (3) is used.
    #[serde(default)]
    search_circuit_breaker_threshold: Option<u32>,
    /// Maximum web sources to capture.
    #[serde(default)]
    max_web_results: Option<usize>,
    /// Maximum in-project local sources to capture.
    #[serde(default)]
    max_local_sources: Option<usize>,
    /// Maximum sources to send to the LLM synthesis engine.
    #[serde(default)]
    max_synthesis_sources: Option<usize>,
}

impl CreateResearchRequest {
    /// Convert this HTTP request into the shared [`ResearchRunRequest`].
    fn to_run_request(&self) -> ragent_research::ResearchRunRequest {
        ragent_research::ResearchRunRequest {
            name: self.name.clone(),
            topic: self.topic.clone(),
            title: self.title.clone(),
            from_urls: self.from_urls.clone(),
            from_files: self.from_files.clone(),
            sources_dir: self.sources_dir.clone(),
            template: self.template.clone(),
            depth: self.depth.clone(),
            tier: self.tier.clone(),
            iterations: self.iterations,
            output_format: self.format.clone(),
            use_local: self.use_local,
            use_specs: self.use_specs,
            use_low_relevance: self.use_low_relevance,
            no_scholarly: self.no_scholarly,
            use_pdf: self.use_pdf,
            fetch_concurrency: self.fetch_concurrency,
            local_concurrency: self.local_concurrency,
            fetch_timeout_secs: self.fetch_timeout_secs,
            web_phase_timeout_secs: self.web_phase_timeout_secs,
            local_phase_timeout_secs: self.local_phase_timeout_secs,
            search_max_retries: self.search_max_retries,
            search_retry_base_delay_ms: self.search_retry_base_delay_ms,
            search_circuit_breaker_threshold: self.search_circuit_breaker_threshold,
            max_web_results: self.max_web_results,
            max_local_sources: self.max_local_sources,
            max_synthesis_sources: self.max_synthesis_sources,
        }
    }
}

async fn create_research(
    State(state): State<AppState>,
    Json(req): Json<CreateResearchRequest>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let manager = ResearchManager::new(research_root());
    let run_req = req.to_run_request();
    let cfg = state.config.read().await.clone();
    let config = ragent_research::build_session_config(&run_req, Some(&cfg));
    let title = req.title.clone().unwrap_or_else(|| {
        ragent_research::derive_title_files(
            &req.topic,
            req.from_urls.first().map(String::as_str),
            &req.from_files,
        )
    });

    // Validate name and check for duplicates synchronously so the caller
    // gets immediate feedback on bad requests.
    if let Err(e) = ragent_research::ResearchName::try_new(&req.name) {
        return error_response(StatusCode::BAD_REQUEST, &e.to_string()).into_response();
    }
    if manager.show(&req.name).await.is_ok() {
        return error_response(
            StatusCode::CONFLICT,
            &format!("research item '{}' already exists", req.name),
        )
        .into_response();
    }

    // Create a broadcast channel for SSE streaming of research events.
    // Subscribers to GET /research/{name}/events will receive events through
    // this channel.
    let (tx, _rx) = tokio::sync::broadcast::channel::<SessionEvent>(256);

    // Register the run in AppState so the SSE endpoint can find the channel.
    {
        let mut runs = state.research_runs.lock().await;
        runs.insert(req.name.clone(), tx.clone());
    }

    // Build a broadcast-based observer that forwards events to the channel.
    struct BroadcastObserver(tokio::sync::broadcast::Sender<SessionEvent>);
    impl SessionObserver for BroadcastObserver {
        fn on_event(&self, event: SessionEvent) {
            // Best-effort send; if there are no subscribers the event is
            // simply dropped (this is expected — the channel has no
            // receivers when nobody is listening to the SSE stream).
            let _ = self.0.send(event);
        }
    }
    let observer = BroadcastObserver(tx);

    // Wire the tool registry from the shared session processor.
    let project_root = research_root()
        .parent()
        .unwrap_or_else(|| StdPath::new("."))
        .to_path_buf();
    let provider_registry = state.session_processor.provider_registry.clone();
    let active_model =
        ragent_agent::agent::resolve_agent_with_model(&cfg.default_agent, &cfg, &provider_registry)
            .ok()
            .and_then(|agent| agent.model.clone());
    let session = build_research_session(
        &state.session_processor.tool_registry,
        manager.clone(),
        req.name.clone(),
        project_root,
        state.event_bus.clone(),
        Some(state.storage.clone()),
        Some(Arc::new(cfg)),
        Some(provider_registry),
        active_model,
        Some(req.name.as_str()),
    );

    // Spawn the research run as a background task so the HTTP response
    // returns immediately with 202 Accepted.
    let name_clone = req.name.clone();
    let title_clone = title.clone();
    let runs_registry = state.research_runs.clone();
    tokio::spawn(async move {
        match session
            .run(&name_clone, &title_clone, &config, Arc::new(observer))
            .await
        {
            Ok(_outcome) => {
                tracing::info!(
                    name = %name_clone,
                    "research: background run completed successfully"
                );
            }
            Err(e) => {
                tracing::error!(
                    name = %name_clone,
                    error = %e,
                    "research: background run failed"
                );
            }
        }
        // Clean up the run registry entry.
        let mut runs = runs_registry.lock().await;
        runs.remove(&name_clone);
    });

    // Return 202 Accepted with a Location header pointing to the SSE stream.
    let location = format!("/research/{}/events", req.name);
    let body = Json(serde_json::json!({
        "name": req.name,
        "title": title,
        "status": "accepted",
        "message": "Research run started. Connect to the location header for live events.",
    }));
    (
        StatusCode::ACCEPTED,
        [("location", location.as_str())],
        body,
    )
        .into_response()
}
// ── GET /research/{name} ────────────────────────────────────────────────

/// Query parameters for `GET /research/{name}`.
#[derive(Deserialize)]
struct ShowResearchQuery {
    /// When `true`, include the extended metadata fields (`topic`, `queries`,
    /// `output_format`, `model`) in the response. Defaults to `false` so the
    /// base response stays lightweight.
    #[serde(default)]
    full: bool,
}

async fn show_research(
    State(_state): State<AppState>,
    Path(name): Path<String>,
    axum::extract::Query(q): axum::extract::Query<ShowResearchQuery>,
) -> impl IntoResponse {
    let manager = ResearchManager::new(research_root());
    match manager.show(&name).await {
        Ok(item) => {
            let row = ResearchItemRow {
                name: item.name.to_string(),
                title: item.title.clone(),
                status: item.status.as_str().to_string(),
                created_at: item.created_at.to_rfc3339(),
                modified_at: item.modified_at.to_rfc3339(),
                sources: item.source_count(),
                topic: if q.full {
                    Some(item.topic.clone())
                } else {
                    None
                },
                queries: if q.full {
                    item.queries.clone()
                } else {
                    Vec::new()
                },
                output_format: if q.full {
                    item.output_format.clone()
                } else {
                    None
                },
                model: if q.full { item.model.clone() } else { None },
            };
            let search_hits: Vec<SearchHit> =
                manager.search(&item.title, 5).await.unwrap_or_default();
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "item": row,
                    "related": search_hits
                        .into_iter()
                        .map(|h| serde_json::json!({
                            "name": h.name,
                            "title": h.title,
                            "snippet": h.snippet,
                        }))
                        .collect::<Vec<_>>(),
                })),
            )
        }
        Err(ragent_research::ResearchError::NotFound(name, suggestions)) => error_response(
            StatusCode::NOT_FOUND,
            &format!("research item '{name}' not found. Closest matches: {suggestions}"),
        ),
        Err(ragent_research::ResearchError::InvalidName(_)) => {
            error_response(StatusCode::BAD_REQUEST, "invalid research name")
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ── DELETE /research/{name} ─────────────────────────────────────────────

#[derive(Deserialize)]
struct DeleteResearchQuery {
    /// Required confirmation token. Matches the supplied value verbatim.
    confirm: Option<String>,
}

async fn delete_research(
    State(_state): State<AppState>,
    Path(name): Path<String>,
    axum::extract::Query(q): axum::extract::Query<DeleteResearchQuery>,
) -> impl IntoResponse {
    let manager = ResearchManager::new(research_root());
    let confirm = q.confirm.unwrap_or_default();
    if confirm != format!("delete-{name}") {
        return error_response(
            StatusCode::PRECONDITION_FAILED,
            &format!(
                "refusing to delete research/{name}: pass `?confirm=delete-{name}` to confirm"
            ),
        );
    }
    match manager.delete(&name).await {
        Ok(()) => (
            StatusCode::NO_CONTENT,
            Json(serde_json::json!({ "deleted": name })),
        ),
        Err(ragent_research::ResearchError::NotFound(name, suggestions)) => error_response(
            StatusCode::NOT_FOUND,
            &format!("research item '{name}' not found. Closest matches: {suggestions}"),
        ),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ── helpers ─────────────────────────────────────────────────────────────

fn error_response(status: StatusCode, message: &str) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({ "error": message })))
}
// ── GET /research/{name}/events ──────────────────────────────────────────
//
// SSE endpoint that streams live research events for a background research
// run. The client connects to this URL after receiving a 202 Accepted from
// POST /research. Events are forwarded from the broadcast channel registered
// in AppState.research_runs.
//
// If the research run has already completed (or was never started), the
// stream closes immediately after sending a terminal event.

async fn research_events_stream(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    use tokio_stream::StreamExt;
    use tokio_stream::wrappers::BroadcastStream;

    // Look up the broadcast channel for this research run.
    let tx = {
        let runs = state.research_runs.lock().await;
        runs.get(&name).cloned()
    };

    let Some(tx) = tx else {
        // No active run — return 404 or check if the item exists on disk.
        let manager = ResearchManager::new(research_root());
        match manager.show(&name).await {
            Ok(item) => {
                // Item exists but no active run — return its current status.
                return (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    Json(serde_json::json!({
                        "name": item.name,
                        "status": item.status.as_str(),
                        "message": "No active research run. The item has already completed or was not started via POST /research.",
                    })),
                )
                    .into_response();
            }
            Err(_) => {
                return error_response(
                    StatusCode::NOT_FOUND,
                    &format!("no active research run for '{name}'"),
                )
                .into_response();
            }
        }
    };

    // Subscribe to the broadcast channel.
    let rx = tx.subscribe();
    let stream = BroadcastStream::new(rx).map(|result| {
        match result {
            Ok(event) => {
                // Serialize the SessionEvent as pure JSON (no CLI prefix).
                let json = ragent_research::session_event_json(&event);
                Ok::<_, std::convert::Infallible>(
                    axum::response::sse::Event::default()
                        .event("research")
                        .data(json),
                )
            }
            Err(_) => Ok(axum::response::sse::Event::default()),
        }
    });

    use axum::response::sse::{KeepAlive, Sse};
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

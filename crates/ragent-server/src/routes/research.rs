//! HTTP handlers for the research system (T-036, T-037, T-038, T-039).
//!
//! Exposes the `ragent-research` crate behind a thin REST surface:
//!
//! - `GET    /research`              — list every research item (FR-012)
//! - `POST   /research`              — create + run a gathering session in
//!   the background; returns 202 Accepted with the events URL in `Location`
//! - `GET    /research/{name}`       — show one item (`?full=true` includes
//!   extended metadata)
//! - `DELETE /research/{name}`       — remove an item (with confirmation)
//! - `GET    /research/{name}/events` — SSE stream of live events for a
//!   background run (subscribes to the broadcast channel registered by POST)
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

use super::error_response;
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
                .map(|i| ResearchItemRow::from_item(i, false))
                .collect();
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "items": rows,
                    "count": rows.len(),
                })),
            )
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
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

impl ResearchItemRow {
    /// Map a [`ragent_research::ResearchItem`] onto the API row shape.
    ///
    /// `full = false` (list view) omits the extended metadata fields;
    /// `full = true` (show view) fills them in. Shared by `list_research`
    /// and `show_research` so the field mapping exists in exactly one place.
    fn from_item(item: ragent_research::ResearchItem, full: bool) -> Self {
        Self {
            name: item.name.to_string(),
            title: item.title.clone(),
            status: item.status.as_str().to_string(),
            created_at: item.created_at.to_rfc3339(),
            modified_at: item.modified_at.to_rfc3339(),
            sources: item.source_count(),
            topic: full.then_some(item.topic),
            queries: if full { item.queries } else { Vec::new() },
            output_format: full.then_some(item.output_format).flatten(),
            model: full.then_some(item.model).flatten(),
        }
    }
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
    /// `--mode tiered|supervisor|competitive` research execution strategy
    /// (FR-001).
    #[serde(default)]
    mode: Option<String>,
    /// `--summarization-model <provider:model>` selects a lightweight model
    /// for summarizing fetched webpages independently from the synthesis
    /// model (FR-002).
    #[serde(default)]
    summarization_model: Option<String>,
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
    /// Optional research brief generated from the user's prompt. When
    /// supplied, downstream agents use this as their mission statement.
    #[serde(default)]
    brief: Option<String>,
    /// Model used by research agents / sub-topic workers (FR-013).
    #[serde(default)]
    research_model: Option<String>,
    /// Model used to compress or summarize intermediate findings (FR-013).
    #[serde(default)]
    compression_model: Option<String>,
    /// Model used to write the final report (FR-013).
    #[serde(default)]
    final_report_model: Option<String>,
    /// Maximum parallel researcher agents in supervisor/competitive modes
    /// (FR-012).
    #[serde(default)]
    max_concurrent_research_units: Option<usize>,
    /// `--evaluate` — run the deterministic self-evaluation scorecard and
    /// append it to the assembled report (FR-008 / T-015).
    #[serde(default)]
    evaluate: bool,
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
            mode: self.mode.clone(),
            clarify: None,
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
            summarization_model: self.summarization_model.clone(),
            brief: self.brief.clone(),
            research_model: self.research_model.clone(),
            compression_model: self.compression_model.clone(),
            final_report_model: self.final_report_model.clone(),
            max_concurrent_research_units: self.max_concurrent_research_units,
            evaluate: Some(self.evaluate),
        }
    }
}

async fn create_research(
    State(state): State<AppState>,
    Json(req): Json<CreateResearchRequest>,
) -> axum::response::Response {
    let manager = ResearchManager::new(research_root());

    // Validate the name and check for duplicates BEFORE building the session
    // config so rejected requests do not pay for config loading / cloning.
    if let Err(e) = ragent_research::ResearchName::try_new(&req.name) {
        return error_response(StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }
    if manager.show(&req.name).await.is_ok() {
        return error_response(
            StatusCode::CONFLICT,
            format!("research item '{}' already exists", req.name),
        )
        .into_response();
    }

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

    // Create a broadcast channel for SSE streaming of research events.
    // Subscribers to GET /research/{name}/events will receive events through
    // this channel. The registry entry doubles as an in-flight guard: a
    // concurrent POST with the same name is rejected here (the disk-based
    // duplicate check above only sees completed items).
    struct BroadcastObserver(tokio::sync::broadcast::Sender<SessionEvent>);
    impl SessionObserver for BroadcastObserver {
        fn on_event(&self, event: SessionEvent) {
            // Best-effort send; if there are no subscribers the event is
            // simply dropped (this is expected — the channel has no
            // receivers when nobody is listening to the SSE stream).
            let _ = self.0.send(event);
        }
    }
    let observer = {
        let mut runs = state.research_runs.lock().await;
        if runs.contains_key(&req.name) {
            drop(runs);
            return error_response(
                StatusCode::CONFLICT,
                format!("research run '{}' is already in progress", req.name),
            )
            .into_response();
        }
        let (tx, _rx) = tokio::sync::broadcast::channel::<SessionEvent>(256);
        runs.insert(req.name.clone(), tx.clone());
        BroadcastObserver(tx)
    };

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
    // returns immediately with 202 Accepted. The SSE sender is cloned so a
    // failed run can still deliver a terminal RunStep event to subscribers —
    // without it, the stream would simply close and look like a successful
    // run that emitted no events.
    let name_clone = req.name.clone();
    let title_clone = title.clone();
    let runs_registry = state.research_runs.clone();
    let err_tx = observer.0.clone();
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
                // Surface the failure to any SSE subscriber so the stream
                // ends with an explicit terminal event.
                let _ = err_tx.send(SessionEvent::RunStep {
                    step: "run".to_string(),
                    status: "failed".to_string(),
                    detail: Some(e.to_string()),
                });
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
            // Search before building the row so `item.title` can be moved
            // into the row instead of cloned.
            let search_hits: Vec<SearchHit> =
                manager.search(&item.title, 5).await.unwrap_or_default();
            let row = ResearchItemRow::from_item(item, q.full);
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
            format!("research item '{name}' not found. Closest matches: {suggestions}"),
        ),
        Err(ragent_research::ResearchError::InvalidName(_)) => {
            error_response(StatusCode::BAD_REQUEST, "invalid research name")
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
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
            format!("refusing to delete research/{name}: pass `?confirm=delete-{name}` to confirm"),
        );
    }
    match manager.delete(&name).await {
        Ok(()) => (
            StatusCode::NO_CONTENT,
            Json(serde_json::json!({ "deleted": name })),
        ),
        Err(ragent_research::ResearchError::NotFound(name, suggestions)) => error_response(
            StatusCode::NOT_FOUND,
            format!("research item '{name}' not found. Closest matches: {suggestions}"),
        ),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

// ── helpers ─────────────────────────────────────────────────────────────

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
                // `Json` already sets the content-type header.
                return (
                    StatusCode::OK,
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
                    format!("no active research run for '{name}'"),
                )
                .into_response();
            }
        }
    };

    // Subscribe to the broadcast channel.
    let rx = tx.subscribe();
    let stream = BroadcastStream::new(rx).map(|result| match result {
        Ok(event) => {
            // Serialize the SessionEvent as pure JSON (no CLI prefix).
            let json = ragent_research::session_event_json(&event);
            Ok::<_, std::convert::Infallible>(
                axum::response::sse::Event::default()
                    .event("research")
                    .data(json),
            )
        }
        // The broadcast channel lagged behind or the sender was dropped.
        // Emit a visible marker so clients know events were dropped rather
        // than silently swallowing the error as an empty event.
        Err(_) => Ok(axum::response::sse::Event::default()
            .event("research")
            .data("[LAGGED]")),
    });

    use axum::response::sse::{KeepAlive, Sse};
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::CreateResearchRequest;

    #[test]
    fn to_run_request_maps_new_mode_and_summarization_and_evaluate() {
        let req = CreateResearchRequest {
            name: "compete".into(),
            topic: "Compare A and B".into(),
            title: None,
            sources_dir: None,
            template: None,
            from_urls: Vec::new(),
            from_files: Vec::new(),
            use_local: false,
            use_specs: false,
            use_low_relevance: false,
            no_scholarly: false,
            use_pdf: false,
            fetch_concurrency: None,
            fetch_timeout_secs: None,
            local_concurrency: None,
            depth: None,
            iterations: None,
            format: Some("comparison-table".into()),
            mode: Some("competitive".into()),
            summarization_model: Some("ollama:phi4".into()),
            tier: Some("light".into()),
            web_phase_timeout_secs: None,
            local_phase_timeout_secs: None,
            search_max_retries: None,
            search_retry_base_delay_ms: None,
            search_circuit_breaker_threshold: None,
            max_web_results: None,
            max_local_sources: None,
            max_synthesis_sources: None,
            brief: None,
            research_model: Some("anthropic:claude-sonnet-4".into()),
            compression_model: None,
            final_report_model: None,
            max_concurrent_research_units: Some(3),
            evaluate: true,
        };

        let run = req.to_run_request();
        assert_eq!(run.mode, Some("competitive".into()));
        assert_eq!(run.output_format, Some("comparison-table".into()));
        assert_eq!(run.summarization_model, Some("ollama:phi4".into()));
        assert_eq!(run.evaluate, Some(true));
        assert_eq!(run.tier, Some("light".into()));
        assert_eq!(run.research_model, Some("anthropic:claude-sonnet-4".into()));
        assert_eq!(run.max_concurrent_research_units, Some(3));
    }

    #[test]
    fn to_run_request_preserves_defaults_when_optional_fields_omitted() {
        let req = CreateResearchRequest {
            name: "plain".into(),
            topic: "Rust".into(),
            title: None,
            sources_dir: None,
            template: None,
            from_urls: Vec::new(),
            from_files: Vec::new(),
            use_local: false,
            use_specs: false,
            use_low_relevance: false,
            no_scholarly: false,
            use_pdf: false,
            fetch_concurrency: None,
            fetch_timeout_secs: None,
            local_concurrency: None,
            depth: None,
            iterations: None,
            format: None,
            mode: None,
            summarization_model: None,
            tier: None,
            web_phase_timeout_secs: None,
            local_phase_timeout_secs: None,
            search_max_retries: None,
            search_retry_base_delay_ms: None,
            search_circuit_breaker_threshold: None,
            max_web_results: None,
            max_local_sources: None,
            max_synthesis_sources: None,
            brief: None,
            research_model: None,
            compression_model: None,
            final_report_model: None,
            max_concurrent_research_units: None,
            evaluate: false,
        };

        let run = req.to_run_request();
        assert!(run.mode.is_none());
        assert!(run.summarization_model.is_none());
        assert_eq!(run.evaluate, Some(false));
    }
}

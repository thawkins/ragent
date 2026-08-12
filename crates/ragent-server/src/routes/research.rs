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
use std::sync::Arc;

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

use ragent_research::{
    Depth, OutputFormat, ResearchManager, SearchHit, SessionConfig, SessionEvent, SessionObserver,
};

/// Build the `/research` sub-router.
pub fn research_routes() -> Router<AppState> {
    Router::new()
        .route("/research", get(list_research).post(create_research))
        .route(
            "/research/{name}",
            get(show_research).delete(delete_research),
        )
}

/// Compute the research root from the process working directory. The HTTP
/// server runs against the same project root as the CLI, so this is the
/// straightforward `cwd/research`.
fn research_root() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("research")
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
    /// `--from-file <PATH>`: extract a local document and use its content as
    /// the research subject in place of an explicit topic. The extracted
    /// content is captured as the primary `Source::Other`; web search still
    /// runs.
    from_file: Option<String>,
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
    /// `--format report|executive-summary|comparison-table|source-bibliography`.
    #[serde(default)]
    format: Option<String>,
}

async fn create_research(
    State(state): State<AppState>,
    Json(req): Json<CreateResearchRequest>,
) -> impl IntoResponse {
    let manager = ResearchManager::new(research_root());
    let config = SessionConfig {
        topic: req.topic.clone(),
        from_urls: req.from_urls.clone(),
        from_file: req.from_file.clone().map(PathBuf::from),
        sources_dir: req.sources_dir.map(PathBuf::from),
        template: req.template,
        disable_local: !req.use_local,
        disable_specs: !req.use_specs,
        fetch_concurrency: req
            .fetch_concurrency
            .unwrap_or(ragent_research::DEFAULT_FETCH_CONCURRENCY),
        depth: req.depth.as_deref().and_then(Depth::parse),
        iterations: req.iterations,
        output_format: req.format.as_deref().map_or(OutputFormat::Report, |s| {
            OutputFormat::parse(s).unwrap_or(OutputFormat::Report)
        }),
        use_low_relevance: req.use_low_relevance,
        disable_scholarly: req.no_scholarly,
        fetch_timeout_secs: req.fetch_timeout_secs.unwrap_or(30),
        local_concurrency: req
            .local_concurrency
            .unwrap_or(ragent_research::DEFAULT_LOCAL_CONCURRENCY),
        ..SessionConfig::default()
    };
    let title = req.title.clone().unwrap_or_else(|| {
        ragent_research::derive_title_full(
            &req.topic,
            req.from_urls.first().map(String::as_str),
            req.from_file.as_deref(),
        )
    });
    // Stream every gathering event as a server-sent response. The HTTP
    // layer doesn't currently expose SSE for research, so we collect the
    // events into a single JSON response instead. Future iterations can
    // upgrade this to a true `text/event-stream` without changing the
    // public surface.
    struct Collector(Arc<tokio::sync::Mutex<Vec<SessionEvent>>>);
    impl SessionObserver for Collector {
        fn on_event(&self, event: SessionEvent) {
            // Best-effort push; if the mutex is contended we drop the event
            // rather than block the session.
            if let Ok(mut g) = self.0.try_lock() {
                g.push(event);
            }
        }
    }
    let events = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let observer = Collector(events.clone());

    // Wire the tool registry from the shared session processor so web and
    // local gathering actually run. Without this the session had no gatherers
    // and always reported zero sources.
    let project_root = research_root()
        .parent()
        .unwrap_or_else(|| StdPath::new("."))
        .to_path_buf();
    let cfg = state.config.read().await.clone();
    let provider_registry = state.session_processor.provider_registry.clone();
    let active_model =
        ragent_agent::agent::resolve_agent_with_model(&cfg.default_agent, &cfg, &provider_registry)
            .ok()
            .and_then(|agent| agent.model);
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
    match session
        .run(&req.name, &title, &config, Arc::new(observer))
        .await
    {
        Ok(outcome) => {
            let collected = events.lock().await.clone();
            let events_json: Vec<serde_json::Value> = collected
                .iter()
                .map(ragent_research::render_session_event_json)
                .map(|line| {
                    // Strip the `ragent-research: ` prefix so the body is
                    // plain JSON.
                    line.trim_start_matches("ragent-research: ").to_string()
                })
                .map(|s| serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s)))
                .collect();
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "name": outcome.research_name,
                    "format": config.output_format.as_str(),
                    "total_sources": outcome.sources.len(),
                    "events": events_json,
                })),
            )
        }
        Err(e) => {
            let status = match &e {
                ragent_research::ResearchError::InvalidName(_) => StatusCode::BAD_REQUEST,
                ragent_research::ResearchError::AlreadyExists(_) => StatusCode::CONFLICT,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            error_response(status, &e.to_string())
        }
    }
}
// ── GET /research/{name} ────────────────────────────────────────────────

async fn show_research(
    State(_state): State<AppState>,
    Path(name): Path<String>,
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

//! Memory REST endpoints for the ragent HTTP server.
//!
//! Provides CRUD operations for both file-based memory blocks and
//! SQLite-backed structured memories.
//!
//! # Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | `/memory/blocks` | List all memory blocks |
//! | GET | `/memory/blocks/{label}` | Read a specific block |
//! | PUT | `/memory/blocks/{label}` | Create or update a block |
//! | DELETE | `/memory/blocks/{label}` | Delete a block |
//! | GET | `/memory/search` | Search structured memories (FTS5) |
//! | POST | `/memory/store` | Store a new structured memory |
//! | DELETE | `/memory/{id}` | Forget (delete) a structured memory |

use std::path::PathBuf;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get},
};
use ragent_core::{
    event::Event,
    memory::{BlockScope, BlockStorage, FileBlockStorage, load_all_blocks},
};
use serde::{Deserialize, Serialize};

use super::AppState;

// ── Response types ───────────────────────────────────────────────────

/// JSON representation of a memory block (API response).
#[derive(Serialize)]
pub struct BlockResponse {
    /// Block label (filename stem).
    pub label: String,
    /// Human-readable description.
    pub description: String,
    /// Storage scope: "global" or "project".
    pub scope: String,
    /// Maximum content size in bytes (0 = unlimited).
    pub limit: usize,
    /// Whether the block is read-only.
    pub read_only: bool,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// ISO 8601 last-updated timestamp.
    pub updated_at: String,
    /// Markdown content of the block.
    pub content: String,
}

/// JSON representation of a structured memory (API response).
#[derive(Serialize)]
pub struct MemoryResponse {
    /// Auto-generated row ID.
    pub id: i64,
    /// The memory content.
    pub content: String,
    /// Category (fact, pattern, preference, insight, error, workflow).
    pub category: String,
    /// Source of the memory.
    pub source: String,
    /// Confidence score (0.0–1.0).
    pub confidence: f64,
    /// Project this memory belongs to.
    pub project: String,
    /// Session that created this memory.
    pub session_id: String,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// ISO 8601 last-updated timestamp.
    pub updated_at: String,
    /// Number of times accessed in search results.
    pub access_count: i64,
    /// ISO 8601 timestamp of last access.
    pub last_accessed: Option<String>,
    /// Tags attached to this memory.
    pub tags: Vec<String>,
}

// ── Request types ─────────────────────────────────────────────────────

/// Request body for `PUT /memory/blocks/{label}`.
#[derive(Deserialize)]
pub struct PutBlockRequest {
    /// Markdown content for the block.
    pub content: String,
    /// Optional scope override: "global" or "project" (default: "project").
    #[serde(default = "default_scope")]
    pub scope: String,
    /// Optional description.
    #[serde(default)]
    pub description: String,
    /// Optional content size limit in bytes (0 = unlimited).
    #[serde(default)]
    pub limit: usize,
    /// Optional read-only flag.
    #[serde(default)]
    pub read_only: bool,
}

fn default_scope() -> String {
    "project".to_string()
}

/// Request body for `POST /memory/store`.
#[derive(Deserialize)]
pub struct StoreMemoryRequest {
    /// The memory content.
    pub content: String,
    /// Category: fact, pattern, preference, insight, error, workflow.
    pub category: String,
    /// Source identifier (e.g., "api", "auto-extract").
    #[serde(default = "default_source")]
    pub source: String,
    /// Confidence score (0.0–1.0).
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    /// Project identifier.
    #[serde(default)]
    pub project: String,
    /// Session ID.
    #[serde(default)]
    pub session_id: String,
    /// Tags for categorisation.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_source() -> String {
    "api".to_string()
}

fn default_confidence() -> f64 {
    0.7
}

/// Query parameters for `GET /memory/search`.
#[derive(Deserialize)]
pub struct SearchMemoryQuery {
    /// Search query string (FTS5).
    pub q: String,
    /// Optional comma-separated category filter.
    pub categories: Option<String>,
    /// Optional comma-separated tag filter.
    pub tags: Option<String>,
    /// Minimum confidence threshold (default: 0.0).
    #[serde(default)]
    pub min_confidence: f64,
    /// Maximum results (default: 20).
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    20
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Parse a scope string into a `BlockScope`.
fn parse_scope(s: &str) -> Result<BlockScope, (StatusCode, Json<serde_json::Value>)> {
    match s.to_lowercase().as_str() {
        "global" | "user" => Ok(BlockScope::Global),
        "project" => Ok(BlockScope::Project),
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("Invalid scope '{}'. Use 'global' or 'project'.", s),
        )),
    }
}

/// Convert a `MemoryBlock` into a JSON-friendly response.
fn block_to_response(
    scope: &BlockScope,
    block: &ragent_core::memory::MemoryBlock,
) -> BlockResponse {
    BlockResponse {
        label: block.label.clone(),
        description: block.description.clone(),
        scope: match scope {
            BlockScope::Global => "global".to_string(),
            BlockScope::Project => "project".to_string(),
        },
        limit: block.limit,
        read_only: block.read_only,
        created_at: block.created_at.to_rfc3339(),
        updated_at: block.updated_at.to_rfc3339(),
        content: block.content.clone(),
    }
}

/// Convert a `MemoryRow` and its tags into a JSON-friendly response.
fn memory_row_to_response(
    row: &ragent_core::storage::MemoryRow,
    tags: Vec<String>,
) -> MemoryResponse {
    MemoryResponse {
        id: row.id,
        content: row.content.clone(),
        category: row.category.clone(),
        source: row.source.clone(),
        confidence: row.confidence,
        project: row.project.clone(),
        session_id: row.session_id.clone(),
        created_at: row.created_at.clone(),
        updated_at: row.updated_at.clone(),
        access_count: row.access_count,
        last_accessed: row.last_accessed.clone(),
        tags,
    }
}

/// Get the current working directory for block storage resolution.
fn working_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_default()
}

// ── Handlers ──────────────────────────────────────────────────────────

/// `GET /memory/blocks` — list all memory blocks (global + project).
pub async fn list_blocks(State(_state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let storage = FileBlockStorage::new();
    let wd = working_dir();
    let blocks = load_all_blocks(&storage, &wd);

    let responses: Vec<BlockResponse> = blocks
        .iter()
        .map(|(scope, block)| block_to_response(scope, block))
        .collect();

    serialize_response(responses, "list_blocks")
}

/// `GET /memory/blocks/{label}` — read a specific memory block.
pub async fn get_block(
    State(_state): State<AppState>,
    Path(label): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let storage = FileBlockStorage::new();
    let wd = working_dir();

    // Try project scope first, then global.
    let result = storage
        .load(&label, &BlockScope::Project, &wd)
        .ok()
        .flatten()
        .map(|b| (BlockScope::Project, b))
        .or_else(|| {
            storage
                .load(&label, &BlockScope::Global, &wd)
                .ok()
                .flatten()
                .map(|b| (BlockScope::Global, b))
        });

    match result {
        Some((scope, block)) => serialize_response(block_to_response(&scope, &block), "get_block"),
        None => error_response(
            StatusCode::NOT_FOUND,
            format!("Block '{}' not found", label),
        ),
    }
}

/// `PUT /memory/blocks/{label}` — create or update a memory block.
pub async fn put_block(
    State(state): State<AppState>,
    Path(label): Path<String>,
    Json(body): Json<PutBlockRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Validate label
    if ragent_core::memory::MemoryBlock::validate_label(&label).is_err() {
        return error_response(
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid label '{}'. Use lowercase alphanumeric with hyphens.",
                label
            ),
        );
    }

    let scope = match parse_scope(&body.scope) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let wd = working_dir();
    let storage = FileBlockStorage::new();

    // Load existing block to preserve metadata, or create new
    let mut block = match storage.load(&label, &scope, &wd) {
        Ok(Some(existing)) => existing,
        _ => ragent_core::memory::MemoryBlock::new(&label, scope.clone()),
    };

    // Check read-only before modifying
    if block.read_only {
        return error_response(
            StatusCode::FORBIDDEN,
            format!("Block '{}' is read-only", label),
        );
    }

    block.scope = scope.clone();
    block.content = body.content;
    if !body.description.is_empty() {
        block.description = body.description;
    }
    if body.limit > 0 {
        block.limit = body.limit;
    }
    if body.read_only {
        block.read_only = true;
    }
    block.updated_at = chrono::Utc::now();

    match storage.save(&block, &wd) {
        Ok(()) => {
            state.event_bus.publish(Event::MemoryStored {
                session_id: "api".to_string(),
                id: 0, // Block operations don't have numeric IDs
                category: "block".to_string(),
            });
            serialize_response(block_to_response(&block.scope, &block), "put_block")
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save block: {}", e),
        ),
    }
}

/// `DELETE /memory/blocks/{label}` — delete a memory block.
pub async fn delete_block(
    State(state): State<AppState>,
    Path(label): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let storage = FileBlockStorage::new();
    let wd = working_dir();

    // Try project scope first, then global.
    let scope = if storage
        .load(&label, &BlockScope::Project, &wd)
        .ok()
        .flatten()
        .is_some()
    {
        BlockScope::Project
    } else if storage
        .load(&label, &BlockScope::Global, &wd)
        .ok()
        .flatten()
        .is_some()
    {
        BlockScope::Global
    } else {
        return error_response(
            StatusCode::NOT_FOUND,
            format!("Block '{}' not found", label),
        );
    };

    match storage.delete(&label, &scope, &wd) {
        Ok(()) => {
            state.event_bus.publish(Event::MemoryForgotten {
                session_id: "api".to_string(),
                count: 1,
            });
            (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete block: {}", e),
        ),
    }
}

/// `GET /memory/search` — search structured memories (FTS5).
pub async fn search_memories(
    State(state): State<AppState>,
    Query(query): Query<SearchMemoryQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let categories: Option<Vec<String>> = query
        .categories
        .as_ref()
        .map(|c| c.split(',').map(|s| s.trim().to_string()).collect());
    let tags: Option<Vec<String>> = query
        .tags
        .as_ref()
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    let results = state.storage.search_memories(
        &query.q,
        categories.as_deref(),
        tags.as_deref(),
        query.limit,
        query.min_confidence,
    );

    match results {
        Ok(rows) => {
            let responses: Vec<MemoryResponse> = rows
                .iter()
                .map(|row| {
                    let tags = state.storage.get_memory_tags(row.id).unwrap_or_default();
                    memory_row_to_response(row, tags)
                })
                .collect();

            state.event_bus.publish(Event::MemorySearched {
                session_id: "api".to_string(),
                query: query.q.clone(),
                result_count: responses.len(),
                mode: "fts".to_string(),
            });

            serialize_response(responses, "search_memories")
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Search failed: {}", e),
        ),
    }
}

/// `POST /memory/store` — store a new structured memory.
pub async fn store_memory(
    State(state): State<AppState>,
    Json(body): Json<StoreMemoryRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Validate category
    let valid_categories = [
        "fact",
        "pattern",
        "preference",
        "insight",
        "error",
        "workflow",
    ];
    if !valid_categories.contains(&body.category.as_str()) {
        return error_response(
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid category '{}'. Must be one of: {}",
                body.category,
                valid_categories.join(", ")
            ),
        );
    }

    // Validate confidence range
    if !(0.0..=1.0).contains(&body.confidence) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "Confidence must be between 0.0 and 1.0",
        );
    }

    match state.storage.create_memory(
        &body.content,
        &body.category,
        &body.source,
        body.confidence,
        &body.project,
        &body.session_id,
        &body.tags,
    ) {
        Ok(id) => {
            state.event_bus.publish(Event::MemoryStored {
                session_id: body.session_id.clone(),
                id,
                category: body.category.clone(),
            });

            // Fetch the created memory to return full response
            let row = state.storage.get_memory(id).ok().flatten();
            let tags = state.storage.get_memory_tags(id).unwrap_or_default();

            match row {
                Some(r) => (
                    StatusCode::CREATED,
                    Json(
                        serde_json::to_value(memory_row_to_response(&r, tags)).unwrap_or_default(),
                    ),
                ),
                None => (StatusCode::CREATED, Json(serde_json::json!({ "id": id }))),
            }
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to store memory: {}", e),
        ),
    }
}

/// `DELETE /memory/{id}` — forget (delete) a structured memory by ID.
pub async fn forget_memory(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Check existence first
    match state.storage.get_memory(id) {
        Ok(Some(_)) => match state.storage.delete_memory(id) {
            Ok(true) => {
                state.event_bus.publish(Event::MemoryForgotten {
                    session_id: "api".to_string(),
                    count: 1,
                });
                (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
            }
            Ok(false) => error_response(StatusCode::NOT_FOUND, "Memory not found"),
            Err(e) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Delete failed: {}", e),
            ),
        },
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Memory not found"),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Lookup failed: {}", e),
        ),
    }
}

// ── Helpers (shared with parent module) ───────────────────────────────

/// Standardized error JSON response.
fn error_response(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({ "error": message.into() })))
}

/// Serialize a value to JSON and return a response.
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

/// Register memory routes on an Axum router.
pub fn memory_routes() -> axum::Router<AppState> {
    use axum::routing::post;
    axum::Router::new()
        .route("/blocks", get(list_blocks))
        .route(
            "/blocks/{label}",
            get(get_block).put(put_block).delete(delete_block),
        )
        .route("/search", get(search_memories))
        .route("/store", post(store_memory))
        .route("/{id}", delete(forget_memory))
        .route("/visualisation", get(get_visualisation))
        .route("/visualisation/graph", get(get_visualisation_graph))
        .route("/visualisation/timeline", get(get_visualisation_timeline))
        .route("/visualisation/tags", get(get_visualisation_tags))
        .route("/visualisation/heatmap", get(get_visualisation_heatmap))
}
// ── Visualisation endpoints ──────────────────────────────────────────────────

/// GET /memory/visualisation — Generate visualisation data for all memories.
pub async fn get_visualisation(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let block_storage = FileBlockStorage::new();
    let working_dir = working_dir();

    match ragent_core::memory::generate_visualisation(&state.storage, &block_storage, &working_dir)
    {
        Ok(data) => serialize_response(data, "visualisation"),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to generate visualisation: {e}"),
        ),
    }
}

/// GET /memory/visualisation/graph — Memory category relationship graph.
pub async fn get_visualisation_graph(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match ragent_core::memory::generate_graph(&state.storage) {
        Ok(graph) => serialize_response(graph, "graph"),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to generate graph: {e}"),
        ),
    }
}

/// GET /memory/visualisation/timeline — Journal timeline.
pub async fn get_visualisation_timeline(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match ragent_core::memory::generate_timeline(&state.storage) {
        Ok(timeline) => serialize_response(timeline, "timeline"),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to generate timeline: {e}"),
        ),
    }
}

/// GET /memory/visualisation/tags — Tag cloud.
pub async fn get_visualisation_tags(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match ragent_core::memory::generate_tag_cloud(&state.storage) {
        Ok(cloud) => serialize_response(cloud, "tag_cloud"),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to generate tag cloud: {e}"),
        ),
    }
}

/// GET /memory/visualisation/heatmap — Access pattern heatmap.
pub async fn get_visualisation_heatmap(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match ragent_core::memory::generate_heatmap(&state.storage) {
        Ok(heatmap) => serialize_response(heatmap, "heatmap"),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to generate heatmap: {e}"),
        ),
    }
}

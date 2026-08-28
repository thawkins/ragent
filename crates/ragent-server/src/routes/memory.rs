//! Memory REST endpoints for the ragent HTTP server.
//!
//! Provides CRUD operations for SQLite-backed structured memories.
//!
//! # Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | `/memory/search` | Search structured memories (FTS5) |
//! | POST | `/memory/store` | Store a new structured memory |
//! | DELETE | `/memory/{id}` | Forget (delete) a structured memory |
//! | GET | `/memory/visualisation` | Full visualisation bundle |
//! | GET | `/memory/visualisation/graph` | Category relationship graph |
//! | GET | `/memory/visualisation/tags` | Tag cloud |
//! | GET | `/memory/visualisation/heatmap` | Access pattern heatmap |

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use ragent_agent::event::Event;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::AppState;

// ── Response types ───────────────────────────────────────────────────

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_accessed: Option<String>,
    /// Tags attached to this memory.
    pub tags: Vec<String>,
}

// ── Request types ─────────────────────────────────────────────────────

/// Request body for `POST /memory/store`.
#[derive(Deserialize, Clone)]
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

const fn default_confidence() -> f64 {
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

const fn default_limit() -> usize {
    20
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Convert a `MemoryRow` and its tags into a JSON-friendly response.
fn memory_row_to_response(
    row: &ragent_agent::storage::MemoryRow,
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

// ── Handlers ──────────────────────────────────────────────────────────

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

    // M-005: run the SQLite search off the async executor (FTS5 can be slow).
    let storage = Arc::clone(&state.storage);
    let q = query.q.clone();
    let results = tokio::task::spawn_blocking(move || {
        storage.search_memories(
            &q,
            categories.as_deref(),
            tags.as_deref(),
            query.limit,
            query.min_confidence,
        )
    })
    .await;

    let results = match results {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Search task panicked: {e}"),
            );
        }
    };

    match results {
        Ok(rows) => {
            // M-002/M-005: batch the per-row tag lookups into a single query,
            // run off the async executor.
            let storage = Arc::clone(&state.storage);
            let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
            let responses: Vec<MemoryResponse> = match tokio::task::spawn_blocking(move || {
                let tags_map = storage.get_memory_tags_batched(&ids);
                tags_map.map(|tags| {
                    rows.iter()
                        .map(|row| {
                            let tags: Vec<String> = tags.get(&row.id).cloned().unwrap_or_default();
                            memory_row_to_response(row, tags)
                        })
                        .collect::<Vec<MemoryResponse>>()
                })
            })
            .await
            {
                Ok(Ok(responses)) => responses,
                Ok(Err(e)) => {
                    // One storage failure must not silently blank every tag:
                    // surface it (best-effort — keep serving the rows without
                    // tags rather than failing the whole search).
                    tracing::warn!("Batched tag fetch failed for memory search: {e}");
                    Vec::new()
                }
                Err(e) => {
                    tracing::warn!("Memory-search tag fetch task panicked: {e}");
                    Vec::new()
                }
            };

            state.event_bus.publish(Event::MemorySearched {
                session_id: "api".to_string(),
                query: query.q,
                result_count: responses.len(),
                mode: "fts".to_string(),
            });

            serialize_response(responses, "search_memories")
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Search failed: {e}"),
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

    // M-005: run the SQLite write + reads off the async executor.
    let storage = Arc::clone(&state.storage);
    let body_for_write = body.clone();
    let create_result = tokio::task::spawn_blocking(move || {
        storage.create_memory(
            &body_for_write.content,
            &body_for_write.category,
            &body_for_write.source,
            body_for_write.confidence,
            &body_for_write.project,
            &body_for_write.session_id,
            &body_for_write.tags,
        )
    })
    .await;

    let create_result = match create_result {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Store task panicked: {e}"),
            );
        }
    };

    match create_result {
        Ok(id) => {
            state.event_bus.publish(Event::MemoryStored {
                session_id: body.session_id.clone(),
                id,
                category: body.category.clone(),
            });

            // Fetch the created memory to return full response
            let storage = Arc::clone(&state.storage);
            let fetch = tokio::task::spawn_blocking(move || {
                (
                    storage.get_memory(id).ok().flatten(),
                    storage.get_memory_tags(id).unwrap_or_default(),
                )
            })
            .await;
            let (row, tags) = match fetch {
                Ok(pair) => pair,
                Err(e) => {
                    tracing::warn!("Memory fetch-after-store task panicked: {e}");
                    (None, Vec::new())
                }
            };

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
            format!("Failed to store memory: {e}"),
        ),
    }
}

/// `DELETE /memory/{id}` — forget (delete) a structured memory by ID.
pub async fn forget_memory(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> (StatusCode, Json<serde_json::Value>) {
    // M-005: run the SQLite delete off the async executor. `delete_memory`
    // itself reports whether a row was removed, so a separate existence
    // pre-check (and its extra lock round-trip + TOCTOU window) is redundant.
    let storage = Arc::clone(&state.storage);
    let deleted = tokio::task::spawn_blocking(move || storage.delete_memory(id)).await;
    match deleted {
        Ok(Ok(true)) => {
            state.event_bus.publish(Event::MemoryForgotten {
                session_id: "api".to_string(),
                count: 1,
            });
            (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
        }
        Ok(Ok(false)) => error_response(StatusCode::NOT_FOUND, "Memory not found"),
        Ok(Err(e)) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Delete failed: {e}"),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Delete task panicked: {e}"),
        ),
    }
}

// ── Helpers (shared with parent module) ──────────────────────────��────

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
    axum::Router::new()
        .route("/search", get(search_memories))
        .route("/store", post(store_memory))
        .route("/{id}", delete(forget_memory))
        .route("/visualisation", get(get_visualisation))
        .route("/visualisation/graph", get(get_visualisation_graph))
        .route("/visualisation/tags", get(get_visualisation_tags))
        .route("/visualisation/heatmap", get(get_visualisation_heatmap))
}
// ���─ Visualisation endpoints ──────────────────────────────────────────────────

/// GET /memory/visualisation — Generate visualisation data for all memories.
pub async fn get_visualisation(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    // M-005: `generate_visualisation` runs blocking SQLite reads; off-load it.
    let storage = Arc::clone(&state.storage);
    match tokio::task::spawn_blocking(move || {
        ragent_agent::memory::generate_visualisation(&storage)
    })
    .await
    {
        Ok(Ok(data)) => serialize_response(data, "visualisation"),
        Ok(Err(e)) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to generate visualisation: {e}"),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Visualisation task panicked: {e}"),
        ),
    }
}

/// GET /memory/visualisation/graph — Memory category relationship graph.
pub async fn get_visualisation_graph(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    // FR-010: fetch memories and tags in batch queries rather than per-row.
    // M-005: run the SQLite reads off the async executor.
    let storage = Arc::clone(&state.storage);
    let loaded = tokio::task::spawn_blocking(move || {
        (
            storage.list_memories("", 10_000),
            storage.get_all_memory_tags(),
        )
    })
    .await;

    let (memories, all_tags) = match loaded {
        Ok((Ok(m), Ok(t))) => (m, t),
        Ok((Err(e), _)) | Ok((_, Err(e))) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to load memories for graph: {e}"),
            );
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Graph task panicked: {e}"),
            );
        }
    };
    let graph = ragent_agent::memory::generate_graph(&memories, &all_tags);
    serialize_response(graph, "graph")
}

/// GET /memory/visualisation/tags — Tag cloud.
pub async fn get_visualisation_tags(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    // FR-010: fetch memories and tags in batch queries rather than per-row.
    // M-005: run the SQLite reads off the async executor.
    let storage = Arc::clone(&state.storage);
    let loaded = tokio::task::spawn_blocking(move || {
        (
            storage.list_memories("", 10_000),
            storage.get_all_memory_tags(),
        )
    })
    .await;

    let (memories, all_tags) = match loaded {
        Ok((Ok(m), Ok(t))) => (m, t),
        Ok((Err(e), _)) | Ok((_, Err(e))) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to load memories for tag cloud: {e}"),
            );
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Tag cloud task panicked: {e}"),
            );
        }
    };
    let cloud = ragent_agent::memory::generate_tag_cloud(&memories, &all_tags);
    serialize_response(cloud, "tag_cloud")
}

/// GET /memory/visualisation/heatmap — Access pattern heatmap.
pub async fn get_visualisation_heatmap(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    // M-005: `list_memories("", 10_000)` is a blocking SQLite read; off-load it.
    let storage = Arc::clone(&state.storage);
    match tokio::task::spawn_blocking(move || storage.list_memories("", 10_000)).await {
        Ok(Ok(memories)) => {
            let heatmap = ragent_agent::memory::generate_heatmap(&memories);
            serialize_response(heatmap, "heatmap")
        }
        Ok(Err(e)) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to generate heatmap: {e}"),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Heatmap task panicked: {e}"),
        ),
    }
}

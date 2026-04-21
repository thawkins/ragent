//! Journal REST endpoints for the ragent HTTP server.
//!
//! Provides CRUD and search operations for the append-only journal system.
//!
//! # Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | `/journal/entries` | List journal entries (paginated, optional tag filter) |
//! | GET | `/journal/entries/{id}` | Read a specific entry |
//! | POST | `/journal/entries` | Create a new journal entry |
//! | GET | `/journal/search` | FTS5 search of journal entries |

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use ragent_agent as ragent_core;
use ragent_core::event::Event;
use serde::{Deserialize, Serialize};

use super::AppState;

// ── Response types ───────────────────────────────────────────────────

/// JSON representation of a journal entry (API response).
#[derive(Serialize)]
pub struct JournalEntryResponse {
    /// Unique entry identifier (UUID v4).
    pub id: String,
    /// Short title describing the entry.
    pub title: String,
    /// Full content of the journal entry.
    pub content: String,
    /// Project this entry belongs to.
    pub project: String,
    /// Session that created this entry.
    pub session_id: String,
    /// ISO 8601 timestamp of the observation/event.
    pub timestamp: String,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// Tags attached to this entry.
    pub tags: Vec<String>,
}

// ── Request types ─────────────────────────────────────────────────────

/// Request body for `POST /journal/entries`.
#[derive(Deserialize)]
pub struct CreateEntryRequest {
    /// Short title describing the entry.
    pub title: String,
    /// Full content of the journal entry.
    pub content: String,
    /// Project this entry belongs to.
    #[serde(default)]
    pub project: String,
    /// Session that created this entry.
    #[serde(default = "default_session_id")]
    pub session_id: String,
    /// Tags for categorisation.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_session_id() -> String {
    "api".to_string()
}

/// Query parameters for `GET /journal/entries`.
#[derive(Deserialize)]
pub struct ListEntriesQuery {
    /// Maximum number of entries to return (default: 50).
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Optional tag filter — only entries with this tag.
    pub tag: Option<String>,
}

fn default_limit() -> usize {
    50
}

/// Query parameters for `GET /journal/search`.
#[derive(Deserialize)]
pub struct SearchJournalQuery {
    /// Search query string (FTS5).
    pub q: String,
    /// Optional comma-separated tag filter.
    pub tags: Option<String>,
    /// Maximum results (default: 20).
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

fn default_search_limit() -> usize {
    20
}

// ── Helpers ─────��─────────────────────────────────────────────────────

/// Convert a `JournalEntryRow` and its tags into a JSON-friendly response.
fn entry_to_response(
    row: &ragent_core::storage::JournalEntryRow,
    tags: Vec<String>,
) -> JournalEntryResponse {
    JournalEntryResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        content: row.content.clone(),
        project: row.project.clone(),
        session_id: row.session_id.clone(),
        timestamp: row.timestamp.clone(),
        created_at: row.created_at.clone(),
        tags,
    }
}

// ── Handlers ──────────────────────────────────────────────────────────

/// `GET /journal/entries` — list journal entries (paginated, optional tag filter).
pub async fn list_entries(
    State(state): State<AppState>,
    Query(query): Query<ListEntriesQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let rows = if let Some(ref tag) = query.tag {
        state.storage.list_journal_entries_by_tag(tag, query.limit)
    } else {
        state.storage.list_journal_entries(query.limit)
    };

    match rows {
        Ok(entries) => {
            let responses: Vec<JournalEntryResponse> = entries
                .iter()
                .map(|row| {
                    let tags = state.storage.get_journal_tags(&row.id).unwrap_or_default();
                    entry_to_response(row, tags)
                })
                .collect();
            serialize_response(responses, "list_entries")
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to list entries: {}", e),
        ),
    }
}

/// `GET /journal/entries/{id}` — read a specific journal entry.
pub async fn get_entry(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.storage.get_journal_entry(&id) {
        Ok(Some(row)) => {
            let tags = state.storage.get_journal_tags(&id).unwrap_or_default();
            serialize_response(entry_to_response(&row, tags), "get_entry")
        }
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Entry not found"),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Lookup failed: {}", e),
        ),
    }
}

/// `POST /journal/entries` — create a new journal entry.
pub async fn create_entry(
    State(state): State<AppState>,
    Json(body): Json<CreateEntryRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if body.title.trim().is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Title must not be empty");
    }
    if body.content.trim().is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Content must not be empty");
    }

    let id = uuid::Uuid::new_v4().to_string();

    match state.storage.create_journal_entry(
        &id,
        &body.title,
        &body.content,
        &body.project,
        &body.session_id,
        &body.tags,
    ) {
        Ok(()) => {
            state.event_bus.publish(Event::JournalEntryCreated {
                session_id: body.session_id.clone(),
                id: id.clone(),
                title: body.title.clone(),
            });

            // Return the created entry
            let row = state.storage.get_journal_entry(&id).ok().flatten();
            let tags = state.storage.get_journal_tags(&id).unwrap_or_default();

            match row {
                Some(r) => (
                    StatusCode::CREATED,
                    Json(serde_json::to_value(entry_to_response(&r, tags)).unwrap_or_default()),
                ),
                None => (StatusCode::CREATED, Json(serde_json::json!({ "id": id }))),
            }
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create entry: {}", e),
        ),
    }
}

/// `GET /journal/search` — FTS5 search of journal entries.
pub async fn search_entries(
    State(state): State<AppState>,
    Query(query): Query<SearchJournalQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let tags: Option<Vec<String>> = query
        .tags
        .as_ref()
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    let results = state
        .storage
        .search_journal_entries(&query.q, tags.as_deref(), query.limit);

    match results {
        Ok(entries) => {
            let responses: Vec<JournalEntryResponse> = entries
                .iter()
                .map(|row| {
                    let tags = state.storage.get_journal_tags(&row.id).unwrap_or_default();
                    entry_to_response(row, tags)
                })
                .collect();

            state.event_bus.publish(Event::JournalSearched {
                session_id: "api".to_string(),
                query: query.q.clone(),
                result_count: responses.len(),
            });

            serialize_response(responses, "search_entries")
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Search failed: {}", e),
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

/// Register journal routes on an Axum router.
pub fn journal_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/entries", get(list_entries).post(create_entry))
        .route("/entries/{id}", get(get_entry))
        .route("/search", get(search_entries))
}

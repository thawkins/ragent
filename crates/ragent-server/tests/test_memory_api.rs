//! Integration tests for memory and journal REST endpoints.
//!
//! Tests exercise the full request/response cycle through the Axum router
//! using `tower::ServiceExt::oneshot` without opening a TCP socket.

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ragent_agent as ragent_core;
use ragent_core::config::Config;
use ragent_core::event::EventBus;
use ragent_core::permission::PermissionChecker;
use ragent_core::provider::ProviderRegistry;
use ragent_core::session::SessionManager;
use ragent_core::session::processor::SessionProcessor;
use ragent_core::storage::Storage;
use ragent_core::tool::ToolRegistry;
use ragent_server::routes::{AppState, router};
use tower::ServiceExt;

/// Build a minimal [`AppState`] suitable for testing memory/journal endpoints.
fn test_state(token: &str) -> AppState {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(16));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(ProviderRegistry::new()),
        tool_registry: Arc::new(ToolRegistry::new()),
        permission_checker: Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        stream_config: Default::default(),
        extraction_engine: std::sync::OnceLock::new(),
        auto_approve: false,
    });

    AppState {
        event_bus,
        config: Arc::new(tokio::sync::RwLock::new(Config::default())),
        storage,
        session_processor: processor,
        auth_token: token.to_string(),
        rate_limiter: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        coordinator: None,
    }
}

/// Add auth header to a request builder.
fn add_auth(req: axum::http::request::Builder, token: &str) -> axum::http::request::Builder {
    req.header("Authorization", format!("Bearer {}", token))
}

// ── Memory Block Endpoints ────────────────────────────────────────────

#[tokio::test]
async fn test_memory_blocks_requires_auth() {
    let app = router(test_state("secret"));
    let req = Request::builder()
        .uri("/memory/blocks")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_memory_blocks_list_empty() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .uri("/memory/blocks")
        .header("Authorization", "Bearer tok")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_memory_search_requires_auth() {
    let app = router(test_state("secret"));
    let req = Request::builder()
        .uri("/memory/search?q=test")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_memory_search_empty_results() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .uri("/memory/search?q=test")
        .header("Authorization", "Bearer tok")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_memory_store_requires_auth() {
    let app = router(test_state("secret"));
    let req = Request::builder()
        .method("POST")
        .uri("/memory/store")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"content":"test","category":"fact"}"#))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_memory_store_and_search() {
    let state = test_state("tok");

    // Pre-seed a memory in storage directly
    state
        .storage
        .create_memory(
            "Prefer anyhow for apps",
            "preference",
            "test",
            0.8,
            "",
            "",
            &[],
        )
        .unwrap();

    let app = router(state);

    // Store a memory via API
    let req = Request::builder()
        .method("POST")
        .uri("/memory/store")
        .header("Authorization", "Bearer tok")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"content":"Use Result for error handling","category":"pattern","confidence":0.9,"tags":["rust","error-handling"]}"#,
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_memory_store_invalid_category() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .method("POST")
        .uri("/memory/store")
        .header("Authorization", "Bearer tok")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"content":"test","category":"invalid_cat"}"#))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_memory_store_invalid_confidence() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .method("POST")
        .uri("/memory/store")
        .header("Authorization", "Bearer tok")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"content":"test","category":"fact","confidence":2.0}"#,
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_memory_forget_requires_auth() {
    let app = router(test_state("secret"));
    let req = Request::builder()
        .method("DELETE")
        .uri("/memory/1")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_memory_forget_not_found() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .method("DELETE")
        .uri("/memory/9999")
        .header("Authorization", "Bearer tok")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Journal Endpoints ──────────────────────────────────────────────────

#[tokio::test]
async fn test_journal_entries_requires_auth() {
    let app = router(test_state("secret"));
    let req = Request::builder()
        .uri("/journal/entries")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_journal_entries_list_empty() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .uri("/journal/entries")
        .header("Authorization", "Bearer tok")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_journal_search_requires_auth() {
    let app = router(test_state("secret"));
    let req = Request::builder()
        .uri("/journal/search?q=test")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_journal_create_and_search() {
    let state = test_state("tok");
    let storage = state.storage.clone();

    // Pre-seed a journal entry directly via storage
    storage
        .create_journal_entry(
            "test-id-1",
            "Discovered pattern",
            "Always use Result for fallible operations in Rust.",
            "",
            "",
            &["rust".to_string(), "pattern".to_string()],
        )
        .unwrap();

    let app = router(state);

    // Search for the entry
    let req = Request::builder()
        .uri("/journal/search?q=Result+fallible")
        .header("Authorization", "Bearer tok")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_journal_create_via_api() {
    let state = test_state("tok");
    let app = router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/journal/entries")
        .header("Authorization", "Bearer tok")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"title":"API test entry","content":"Created via HTTP API","tags":["test"]}"#,
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_journal_create_empty_title_rejected() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .method("POST")
        .uri("/journal/entries")
        .header("Authorization", "Bearer tok")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"title":"","content":"Some content"}"#))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_journal_get_entry_not_found() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .uri("/journal/entries/nonexistent-id")
        .header("Authorization", "Bearer tok")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_journal_entries_with_tag_filter() {
    let state = test_state("tok");
    let storage = state.storage.clone();

    // Pre-seed entries with different tags
    storage
        .create_journal_entry(
            "id-1",
            "Rust pattern",
            "Use Result everywhere",
            "",
            "",
            &["rust".to_string()],
        )
        .unwrap();
    storage
        .create_journal_entry(
            "id-2",
            "Python pattern",
            "Use try/except for errors",
            "",
            "",
            &["python".to_string()],
        )
        .unwrap();

    let app = router(state);

    // Filter by tag
    let req = Request::builder()
        .uri("/journal/entries?tag=rust")
        .header("Authorization", "Bearer tok")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Memory Block CRUD ──────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_block_get_not_found() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .uri("/memory/blocks/nonexistent")
        .header("Authorization", "Bearer tok")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_memory_block_delete_not_found() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .method("DELETE")
        .uri("/memory/blocks/nonexistent")
        .header("Authorization", "Bearer tok")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

//! HTTP endpoint tests for the research API (RESEARCHPLAN.md Phase 4).
//!
//! Tests the Axum routes for the research system using in-process `oneshot`
//! requests. The research routes read from `cwd/research` (via a `LazyLock`),
//! so on-disk items are created with unique `test-` prefixed names and cleaned
//! up after each test. Tests that touch the filesystem are serialized with
//! a global mutex to avoid concurrent INDEX.md writes.

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ragent_agent::Config;
use ragent_agent::event::EventBus;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::ProviderRegistry;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool::ToolRegistry;
use ragent_research::ResearchManager;
use ragent_server::routes::{AppState, router};
use tower::ServiceExt;

/// Global mutex to serialize tests that touch `cwd/research` on disk.
static FS_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Build a minimal [`AppState`] for testing research routes.
fn test_state(token: &str) -> AppState {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(16));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(ProviderRegistry::new()),
        tool_registry: Arc::new(ToolRegistry::new()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        stream_config: Default::default(),
        extraction_engine: std::sync::OnceLock::new(),
        auto_approve: false,
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
    });
    AppState {
        event_bus,
        config: Arc::new(tokio::sync::RwLock::new(Config::default())),
        storage,
        session_processor: processor,
        auth_token: token.to_string(),
        rate_limiter: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        coordinator: None,
        research_runs: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    }
}

/// Helper to build an authenticated GET request.
fn auth_get(token: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

/// Helper to build an authenticated DELETE request.
fn auth_delete(token: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

/// Helper to build an authenticated POST request with a JSON body.
fn auth_post(token: &str, uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Read the response body as a UTF-8 string.
async fn body_string(resp: axum::http::Response<Body>) -> String {
    use http_body_util::BodyExt;
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// Unique test item name to avoid conflicts with real research items.
fn test_name() -> String {
    format!(
        "test-research-routes-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

/// Create a research item on disk in `cwd/research` and return its name.
async fn create_test_item(name: &str, topic: &str) {
    let cwd = std::env::current_dir().unwrap();
    let mgr = ResearchManager::new(cwd.join("research"));
    mgr.create(name, &format!("Test: {topic}"), topic)
        .await
        .unwrap();
}

/// Delete a research item from disk.
async fn delete_test_item(name: &str) {
    let cwd = std::env::current_dir().unwrap();
    let mgr = ResearchManager::new(cwd.join("research"));
    let _ = mgr.delete(name).await;
}

// ── Auth tests ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_research_list_requires_auth() {
    let app = router(test_state("secret"));
    let req = Request::builder()
        .uri("/research")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_research_list_returns_empty_or_items() {
    let app = router(test_state("tok"));
    let resp = app.oneshot(auth_get("tok", "/research")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    // The response should be valid JSON with an "items" array and "count".
    assert!(body.contains("\"items\""));
    assert!(body.contains("\"count\""));
}

// ── Show (GET /research/{name}) tests ───────────────────────────────────

#[tokio::test]
async fn test_research_show_not_found() {
    let _guard = FS_LOCK.lock().await;
    let app = router(test_state("tok"));
    let resp = app
        .oneshot(auth_get("tok", "/research/nonexistent-item-xyz"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = body_string(resp).await;
    assert!(body.contains("not found"));
}

#[tokio::test]
async fn test_research_show_returns_item_fields() {
    let _guard = FS_LOCK.lock().await;
    let name = test_name();
    create_test_item(&name, "test topic for show").await;

    let app = router(test_state("tok"));
    let resp = app
        .oneshot(auth_get("tok", &format!("/research/{name}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["item"]["name"], name);
    assert!(
        parsed["item"]["title"]
            .as_str()
            .unwrap()
            .contains("test topic")
    );
    assert_eq!(parsed["item"]["status"], "draft");
    assert_eq!(parsed["item"]["sources"], 0);
    // Without ?full=true, extended fields should be absent (skip_serialized)
    assert!(parsed["item"].get("topic").is_none() || parsed["item"]["topic"].is_null());
    assert!(parsed["item"].get("queries").is_none() || parsed["item"]["queries"].is_null());

    delete_test_item(&name).await;
}

#[tokio::test]
async fn test_research_show_full_includes_extended_fields() {
    let _guard = FS_LOCK.lock().await;
    let name = test_name();
    create_test_item(&name, "test topic for full show").await;

    let app = router(test_state("tok"));
    let resp = app
        .oneshot(auth_get("tok", &format!("/research/{name}?full=true")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    // With ?full=true, the topic field should be present
    assert_eq!(parsed["item"]["topic"], "test topic for full show");
    // queries should be an empty array (skip_serializing_if empty, but present)
    // Actually skip_serializing_if = "Vec::is_empty" means it's omitted when empty.
    // So we just verify topic is present.

    delete_test_item(&name).await;
}

// ── Delete tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_research_delete_requires_confirmation() {
    let _guard = FS_LOCK.lock().await;
    let name = test_name();
    create_test_item(&name, "topic").await;

    let app = router(test_state("tok"));
    // Without ?confirm=delete-{name}
    let resp = app
        .oneshot(auth_delete("tok", &format!("/research/{name}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PRECONDITION_FAILED);
    let body = body_string(resp).await;
    assert!(body.contains("confirm"));

    delete_test_item(&name).await;
}

#[tokio::test]
async fn test_research_delete_with_confirmation() {
    let _guard = FS_LOCK.lock().await;
    let name = test_name();
    create_test_item(&name, "topic").await;

    let app = router(test_state("tok"));
    let confirm = format!("delete-{name}");
    let resp = app
        .oneshot(auth_delete(
            "tok",
            &format!("/research/{name}?confirm={confirm}"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let body = body_string(resp).await;
    assert!(body.contains("\"deleted\""));

    // Verify it's actually gone
    let cwd = std::env::current_dir().unwrap();
    let mgr = ResearchManager::new(cwd.join("research"));
    assert!(mgr.show(&name).await.is_err());
}

#[tokio::test]
async fn test_research_delete_not_found() {
    let _guard = FS_LOCK.lock().await;
    let app = router(test_state("tok"));
    let resp = app
        .oneshot(auth_delete(
            "tok",
            "/research/nonexistent-xyz?confirm=delete-nonexistent-xyz",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── POST /research tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_research_post_invalid_name() {
    let _guard = FS_LOCK.lock().await;
    let app = router(test_state("tok"));
    let body = r#"{"name":"INVALID NAME!","topic":"test"}"#;
    let resp = app
        .oneshot(auth_post("tok", "/research", body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_research_post_returns_202_with_location() {
    let _guard = FS_LOCK.lock().await;
    let name = test_name();
    let body = format!(r#"{{"name":"{name}","topic":"test topic for post"}}"#);
    let app = router(test_state("tok"));
    let resp = app
        .oneshot(auth_post("tok", "/research", &body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Location header should point to the SSE stream
    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, format!("/research/{name}/events"));

    // Body should contain the name and "accepted" status
    let body = body_string(resp).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["name"], name);
    assert_eq!(parsed["status"], "accepted");

    // Clean up — the background task may fail (no LLM), but the item
    // was created on disk. Wait a moment for the spawn to start, then
    // delete the item.
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    delete_test_item(&name).await;
}

#[tokio::test]
async fn test_research_post_duplicate_returns_conflict() {
    let _guard = FS_LOCK.lock().await;
    let name = test_name();
    create_test_item(&name, "topic").await;

    let app = router(test_state("tok"));
    let body = format!(r#"{{"name":"{name}","topic":"duplicate"}}"#);
    let resp = app
        .oneshot(auth_post("tok", "/research", &body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = body_string(resp).await;
    assert!(body.contains("already exists"));

    delete_test_item(&name).await;
}

// ── SSE events endpoint ─────────────────────────────────────────────────

#[tokio::test]
async fn test_research_events_no_active_run_returns_status() {
    let _guard = FS_LOCK.lock().await;
    let name = test_name();
    create_test_item(&name, "topic").await;

    let app = router(test_state("tok"));
    let resp = app
        .oneshot(auth_get("tok", &format!("/research/{name}/events")))
        .await
        .unwrap();
    // When there's no active run but the item exists, it returns 200 with
    // a JSON status blob.
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("No active research run"));

    delete_test_item(&name).await;
}

#[tokio::test]
async fn test_research_events_not_found() {
    let _guard = FS_LOCK.lock().await;
    let app = router(test_state("tok"));
    let resp = app
        .oneshot(auth_get("tok", "/research/nonexistent-xyz/events"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

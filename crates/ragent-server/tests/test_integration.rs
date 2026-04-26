//! Integration tests for auth middleware, rate limiting, and SSE stream.
//!
//! Uses an in-process Axum router with [`tower::ServiceExt::oneshot`] to
//! exercise HTTP endpoints without opening a TCP socket.

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ragent_agent as ragent_core;
use ragent_core::config::Config;
use ragent_core::event::{Event, EventBus};
use ragent_core::permission::PermissionChecker;
use ragent_core::provider::ProviderRegistry;
use ragent_core::session::SessionManager;
use ragent_core::session::processor::SessionProcessor;
use ragent_core::storage::Storage;
use ragent_core::tool::ToolRegistry;
use ragent_server::routes::{AppState, router};
use tower::ServiceExt;

/// Build a minimal [`AppState`] suitable for testing auth and rate-limit paths.
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

// ── Auth middleware tests ─────────────────────────────────────────────────

#[tokio::test]
async fn test_health_no_auth_required() {
    let app = router(test_state("secret"));
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_protected_route_rejects_missing_auth() {
    let app = router(test_state("secret"));
    let req = Request::builder()
        .uri("/sessions")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_protected_route_rejects_wrong_token() {
    let app = router(test_state("correct-token"));
    let req = Request::builder()
        .uri("/sessions")
        .header("Authorization", "Bearer wrong-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_protected_route_rejects_non_bearer_scheme() {
    let app = router(test_state("secret"));
    let req = Request::builder()
        .uri("/sessions")
        .header("Authorization", "Basic secret")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_protected_route_accepts_valid_token() {
    let app = router(test_state("my-secret"));
    let req = Request::builder()
        .uri("/sessions")
        .header("Authorization", "Bearer my-secret")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // GET /sessions should return 200 with an empty list, not 401.
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Rate limiter tests ───────────────────────────────────────────────────

#[tokio::test]
async fn test_rate_limiter_allows_under_limit() {
    let state = test_state("tok");
    // Pre-seed a session so send_message can find it.
    state.storage.create_session("s1", "/tmp").unwrap();

    let rate_limiter = state.rate_limiter.clone();
    {
        let mut lim = rate_limiter.lock().await;
        // Simulate 59 requests (under the 60-per-minute limit).
        lim.insert("s1".to_string(), (59, std::time::Instant::now()));
    }

    let app = router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/sessions/s1/messages")
        .header("Authorization", "Bearer tok")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"content":"hi"}"#))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // 60th request should still be allowed (limit is >60).
    assert_ne!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_rate_limiter_rejects_over_limit() {
    let state = test_state("tok");
    state.storage.create_session("s1", "/tmp").unwrap();

    let rate_limiter = state.rate_limiter.clone();
    {
        let mut lim = rate_limiter.lock().await;
        // Simulate exactly 60 requests already made.
        lim.insert("s1".to_string(), (60, std::time::Instant::now()));
    }

    let app = router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/sessions/s1/messages")
        .header("Authorization", "Bearer tok")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"content":"hi"}"#))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_rate_limiter_resets_after_window() {
    let state = test_state("tok");
    state.storage.create_session("s1", "/tmp").unwrap();

    let rate_limiter = state.rate_limiter.clone();
    {
        let mut lim = rate_limiter.lock().await;
        // 100 requests but the window started 61 seconds ago → should reset.
        let old = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(61))
            .unwrap();
        lim.insert("s1".to_string(), (100, old));
    }

    let app = router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/sessions/s1/messages")
        .header("Authorization", "Bearer tok")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"content":"hi"}"#))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // Window expired → count resets → request allowed.
    assert_ne!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

// ── SSE stream tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_events_stream_requires_auth() {
    let app = router(test_state("secret"));
    let req = Request::builder()
        .uri("/events")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_events_stream_returns_sse() {
    let state = test_state("tok");
    let bus = state.event_bus.clone();

    let app = router(state);
    let req = Request::builder()
        .uri("/events")
        .header("Authorization", "Bearer tok")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("text/event-stream"),
        "expected SSE content-type, got: {ct}"
    );

    // Publish an event — the stream should eventually contain it.
    bus.publish(Event::SessionCreated {
        session_id: "test-sse".into(),
    });
}

// ── Case-insensitive Bearer tests ────────────────────────────────────────

#[tokio::test]
async fn test_auth_accepts_lowercase_bearer() {
    let app = router(test_state("my-secret"));
    let req = Request::builder()
        .uri("/sessions")
        .header("Authorization", "bearer my-secret")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_auth_accepts_uppercase_bearer() {
    let app = router(test_state("my-secret"));
    let req = Request::builder()
        .uri("/sessions")
        .header("Authorization", "BEARER my-secret")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_auth_accepts_mixed_case_bearer() {
    let app = router(test_state("my-secret"));
    let req = Request::builder()
        .uri("/sessions")
        .header("Authorization", "BeArEr my-secret")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── create_session validation tests ──────────────────────────────────────

#[tokio::test]
async fn test_create_session_invalid_path() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .method("POST")
        .uri("/sessions")
        .header("Authorization", "Bearer tok")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"directory":"/nonexistent/path/that/does/not/exist"}"#,
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_session_not_a_directory() {
    // /dev/null exists but is not a directory.
    let app = router(test_state("tok"));
    let req = Request::builder()
        .method("POST")
        .uri("/sessions")
        .header("Authorization", "Bearer tok")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"directory":"/dev/null"}"#))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_session_success() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .method("POST")
        .uri("/sessions")
        .header("Authorization", "Bearer tok")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"directory":"/tmp"}"#))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

//! M6 hardening tests for `ragent-server` (ANTIPAT F-M8/F-M9/F-M10).
//!
//! Covers the explicit CORS origin policy, the `/events` SSE connection cap and
//! `?session=` filter, and the request body limit. Uses an in-process Axum
//! router via [`tower::ServiceExt::oneshot`].

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures::StreamExt;
use ragent_agent::Config;
use ragent_agent::event::{Event, EventBus};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::ProviderRegistry;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool::ToolRegistry;
use ragent_server::routes::{AppState, router};
use tower::ServiceExt;

/// Build a minimal [`AppState`] suitable for exercising the router layers.
fn test_state(token: &str) -> AppState {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(64));
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
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
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
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
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
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
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

/// A dropped `/events` response releases its connection slot, so a fresh
/// connection on an idle router still succeeds (F-M9).
#[tokio::test]
async fn test_events_connection_slot_released_on_drop() {
    let app = router(test_state("tok"));
    let make_req = || {
        Request::builder()
            .uri("/events")
            .header("Authorization", "Bearer tok")
            .body(Body::empty())
            .unwrap()
    };

    let resp = app.clone().oneshot(make_req()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    drop(resp);

    let resp2 = app.oneshot(make_req()).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
}

/// The `?session=` query filters the global stream to one session (F-M9).
#[tokio::test]
async fn test_events_stream_filters_by_session() {
    let state = test_state("tok");
    let bus = state.event_bus.clone();
    let app = router(state);

    let req = Request::builder()
        .uri("/events?session=wanted")
        .header("Authorization", "Bearer tok")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // A decoy for another session must NOT be forwarded; the wanted session's
    // event must be. The first frame read (after skipping none) must be the
    // wanted event.
    bus.publish(Event::SessionCreated {
        session_id: "decoy".into(),
    });
    bus.publish(Event::SessionCreated {
        session_id: "wanted".into(),
    });

    let mut body = resp.into_body().into_data_stream();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(5), body.next())
        .await
        .expect("an SSE frame within the timeout")
        .expect("at least one frame")
        .expect("frame decodes");
    let text = String::from_utf8_lossy(&frame);
    assert!(
        text.contains("\"session_id\":\"wanted\""),
        "expected the wanted session's event, got: {text}"
    );
    assert!(
        !text.contains("decoy"),
        "decoy session event leaked through the filter: {text}"
    );
}

/// A cross-origin request from a non-allowlisted origin gets no CORS grant (F-M8).
#[tokio::test]
async fn test_cors_rejects_unknown_origin() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .uri("/health")
        .header("Origin", "https://evil.example.com")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert!(
        resp.headers().get("access-control-allow-origin").is_none(),
        "non-allowlisted origin must not receive an allow-origin header"
    );
}

/// A cross-origin request from a loopback dev origin is allowed (F-M8).
#[tokio::test]
async fn test_cors_allows_localhost_origin() {
    let app = router(test_state("tok"));
    let req = Request::builder()
        .uri("/health")
        .header("Origin", "http://localhost:3000")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let origin = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok());
    assert_eq!(origin, Some("http://localhost:3000"));
}

/// A body well within the cap is accepted (not 413) by the extractor (F-M10).
#[tokio::test]
async fn test_body_limit_accepts_normal_body() {
    let state = test_state("tok");
    state.storage.create_session("s1", "/tmp").unwrap();
    let app = router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/sessions/s1/messages")
        .header("Authorization", "Bearer tok")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"content":"hello"}"#))
        .unwrap();
    let status = app.oneshot(req).await.unwrap().status();
    assert_ne!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

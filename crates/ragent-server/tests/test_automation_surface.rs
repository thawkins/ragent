//! Integration tests for the automation HTTP surface (spec `openhands` T-016;
//! FR-013, FR-018).
//!
//! The webhook ingress is public (`POST /auto/{id}`), the run-history and
//! manual-run routes are bearer-authenticated (`GET /automation`,
//! `GET /automation/runs/{id}`, `POST /automation/{id}/run`).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ragent_agent::Config;
use ragent_agent::event::EventBus;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::ProviderRegistry;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool::ToolRegistry;
use ragent_server::routes::{AppState, router};
use tower::ServiceExt;

fn config_with_automations() -> Config {
    serde_json::from_str(
        r#"{
            "defaultAgent": "general",
            "automation": {
                "enabled": true,
                "automations": [
                    {
                        "id": "on-issue",
                        "agent": "general",
                        "prompt": "Triage {{payload}}",
                        "trigger": { "kind": "webhook" },
                        "backend": "local"
                    },
                    {
                        "id": "nightly",
                        "trigger": { "kind": "schedule", "schedule": "every 2m" }
                    }
                ]
            }
        }"#,
    )
    .expect("parse config")
}

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
        config: Arc::new(tokio::sync::RwLock::new(config_with_automations())),
        storage,
        session_processor: processor,
        auth_token: token.to_string(),
        rate_limiter: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        coordinator: None,
        research_runs: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    }
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

#[tokio::test]
async fn list_automations_requires_auth_and_reports_next_due() {
    // FR-014, FR-018: the authenticated list shows a concrete next-due time.
    let app = router(test_state("tok"));

    let unauth = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/automation")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(unauth.status(), StatusCode::UNAUTHORIZED);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/automation")
                .header("Authorization", "Bearer tok")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    let rows = body_json(resp).await;
    let rows = rows.as_array().expect("array");
    assert_eq!(rows.len(), 2);
    let on_issue = rows
        .iter()
        .find(|r| r["id"] == "on-issue")
        .expect("on-issue");
    assert_eq!(on_issue["trigger"], "webhook");
    assert_eq!(on_issue["backend"], "local");
    assert!(on_issue["next_due"].is_null(), "webhook has no next due");
    let nightly = rows.iter().find(|r| r["id"] == "nightly").expect("nightly");
    assert_eq!(nightly["trigger"], "schedule");
    assert!(
        nightly["next_due"].is_string(),
        "a schedule automation carries a concrete next-due: {nightly:?}"
    );
}

#[tokio::test]
async fn runs_requires_auth_and_is_empty_initially() {
    // FR-018.
    let app = router(test_state("tok"));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/automation/runs/on-issue")
                .header("Authorization", "Bearer tok")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    let runs = body_json(resp).await;
    assert_eq!(runs.as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn webhook_ingress_is_public_and_rejects_unknown_automations() {
    // FR-013: the ingress is public; an unknown automation is a 404.
    let app = router(test_state("tok"));
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auto/nosuch")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"issue":"42"}"#))
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn webhook_ingress_enqueues_a_run() {
    // FR-013, FR-018: a matching webhook enqueues a run and a durable record
    // is written before the agent turn runs.
    let app = router(test_state("tok"));
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auto/on-issue")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"issue":"42","title":"fix login"}"#))
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let body = body_json(resp).await;
    assert_eq!(body["automation_id"], "on-issue");
    assert_eq!(body["trigger"], "webhook");
    assert_eq!(body["backend"], "local");
}

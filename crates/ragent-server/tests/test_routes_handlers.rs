//! Handler coverage for the core REST routes in `routes::mod`.
//!
//! Extends `test_integration.rs` by exercising the session CRUD handlers,
//! permission replies, task sub-agent endpoints, config/provider reads, and
//! the orchestrator endpoints - all in process via
//! [`tower::ServiceExt::oneshot`] so no TCP port is bound.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ragent_agent::task::{AgentManager, ReportStatus, TaskEntry, TaskStatus};
use ragent_server::routes::{AppState, router};
use tower::ServiceExt;

/// Wire an [`AgentManager`] into the processor so the task routes resolve.
fn wire_agent_manager(state: &AppState) {
    let manager = Arc::new(AgentManager::new(
        Arc::clone(&state.event_bus),
        Arc::clone(&state.session_processor),
        8,
        60,
    ));
    let _ = state.session_processor.agent_manager.set(manager);
}

/// Build an authenticated GET request.
fn get(token: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

/// Build an authenticated DELETE request.
fn del(token: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

/// Build an authenticated POST request with a JSON body.
fn post(token: &str, uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Read a response body as a JSON value.
async fn json_body(resp: axum::response::Response) -> serde_json::Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

fn sample_entry(session: &str, task_id: &str) -> TaskEntry {
    TaskEntry {
        id: task_id.to_string(),
        parent_session_id: session.to_string(),
        child_session_id: format!("child-{task_id}"),
        agent_name: "general".to_string(),
        task_prompt: "do a thing".to_string(),
        background: false,
        detached: false,
        status: TaskStatus::Completed,
        result: Some(Arc::from("all done")),
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: Some(chrono::Utc::now()),
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::Complete,
        time_bound_secs: None,
    }
}

// -- Config / providers ---------------------------------------------------

#[tokio::test]
async fn test_get_config_returns_object() {
    let app = router(crate::support::test_state("tok"));
    let resp = app.oneshot(get("tok", "/config")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert!(body.is_object());
}

#[tokio::test]
async fn test_get_providers_returns_array() {
    let app = router(crate::support::test_state("tok"));
    let resp = app.oneshot(get("tok", "/providers")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert!(body.is_array());
}

// -- Session CRUD ---------------------------------------------------------

#[tokio::test]
async fn test_get_session_not_found() {
    let app = router(crate::support::test_state("tok"));
    let resp = app.oneshot(get("tok", "/sessions/nope")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = json_body(resp).await;
    assert_eq!(body["error"], "session not found");
}

#[tokio::test]
async fn test_get_session_success() {
    let state = crate::support::test_state("tok");
    state.storage.create_session("s1", "/work").unwrap();
    let app = router(state);
    let resp = app.oneshot(get("tok", "/sessions/s1")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["id"], "s1");
}

#[tokio::test]
async fn test_list_sessions_contains_created() {
    let state = crate::support::test_state("tok");
    state.storage.create_session("s1", "/work").unwrap();
    let app = router(state);
    let resp = app.oneshot(get("tok", "/sessions")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert!(body.as_array().unwrap().iter().any(|s| s["id"] == "s1"));
}

#[tokio::test]
async fn test_get_messages_empty_for_session() {
    let state = crate::support::test_state("tok");
    state.storage.create_session("s1", "/work").unwrap();
    let app = router(state);
    let resp = app
        .oneshot(get("tok", "/sessions/s1/messages"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(json_body(resp).await.is_array());
}

#[tokio::test]
async fn test_archive_session_ok() {
    let state = crate::support::test_state("tok");
    state.storage.create_session("s1", "/work").unwrap();
    let app = router(state);
    let resp = app.oneshot(del("tok", "/sessions/s1")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["ok"], true);
}

#[tokio::test]
async fn test_abort_session_not_found() {
    let app = router(crate::support::test_state("tok"));
    let resp = app
        .oneshot(post("tok", "/sessions/missing/abort", ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_abort_session_ok_and_publishes_event() {
    let state = crate::support::test_state("tok");
    state.storage.create_session("s1", "/work").unwrap();
    let mut rx = state.event_bus.subscribe();
    let app = router(state);
    let resp = app
        .oneshot(post("tok", "/sessions/s1/abort", ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let event = rx.try_recv().expect("SessionAborted should be published");
    let (name, data) = ragent_server::sse::event_to_parts(&event);
    assert_eq!(name, "session_aborted");
    assert!(data.contains("s1"));
}

// -- Permission reply -----------------------------------------------------

#[tokio::test]
async fn test_reply_permission_allow() {
    let state = crate::support::test_state("tok");
    let mut rx = state.event_bus.subscribe();
    let app = router(state);
    let resp = app
        .oneshot(post(
            "tok",
            "/sessions/s1/permission/r1",
            r#"{"decision":"allow"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let event = rx
        .try_recv()
        .expect("PermissionReplied should be published");
    let (name, data) = ragent_server::sse::event_to_parts(&event);
    assert_eq!(name, "permission_replied");
    assert!(data.contains("\"allowed\":true"));
}

#[tokio::test]
async fn test_reply_permission_deny() {
    let state = crate::support::test_state("tok");
    let mut rx = state.event_bus.subscribe();
    let app = router(state);
    let resp = app
        .oneshot(post(
            "tok",
            "/sessions/s1/permission/r2",
            r#"{"decision":"deny"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let event = rx.try_recv().unwrap();
    let (_, data) = ragent_server::sse::event_to_parts(&event);
    assert!(data.contains("\"allowed\":false"));
}

#[tokio::test]
async fn test_reply_permission_always() {
    let state = crate::support::test_state("tok");
    let app = router(state);
    let resp = app
        .oneshot(post(
            "tok",
            "/sessions/s1/permission/r3",
            r#"{"decision":"always"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_reply_permission_invalid_decision_rejected() {
    let app = router(crate::support::test_state("tok"));
    let resp = app
        .oneshot(post(
            "tok",
            "/sessions/s1/permission/r1",
            r#"{"decision":"maybe"}"#,
        ))
        .await
        .unwrap();
    // Axum rejects an unparseable JSON body with 422.
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// -- Task endpoints -------------------------------------------------------

#[tokio::test]
async fn test_list_tasks_session_not_found() {
    let state = crate::support::test_state("tok");
    wire_agent_manager(&state);
    let app = router(state);
    let resp = app
        .oneshot(get("tok", "/sessions/nope/tasks"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_tasks_empty() {
    let state = crate::support::test_state("tok");
    state.storage.create_session("s1", "/work").unwrap();
    wire_agent_manager(&state);
    let app = router(state);
    let resp = app.oneshot(get("tok", "/sessions/s1/tasks")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_list_tasks_returns_seeded_entry() {
    let state = crate::support::test_state("tok");
    state.storage.create_session("s1", "/work").unwrap();
    wire_agent_manager(&state);
    let manager = state.session_processor.agent_manager.get().unwrap().clone();
    manager
        .seed_completed_for_test(sample_entry("s1", "t1"))
        .await;
    let app = router(state);
    let resp = app.oneshot(get("tok", "/sessions/s1/tasks")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body[0]["id"], "t1");
    assert_eq!(body[0]["agent_name"], "general");
}

#[tokio::test]
async fn test_get_task_not_found() {
    let state = crate::support::test_state("tok");
    wire_agent_manager(&state);
    let app = router(state);
    let resp = app
        .oneshot(get("tok", "/sessions/s1/tasks/nope"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_task_wrong_session_forbidden() {
    let state = crate::support::test_state("tok");
    wire_agent_manager(&state);
    let manager = state.session_processor.agent_manager.get().unwrap().clone();
    manager
        .seed_completed_for_test(sample_entry("other-session", "t1"))
        .await;
    let app = router(state);
    let resp = app
        .oneshot(get("tok", "/sessions/s1/tasks/t1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_task_success() {
    let state = crate::support::test_state("tok");
    wire_agent_manager(&state);
    let manager = state.session_processor.agent_manager.get().unwrap().clone();
    manager
        .seed_completed_for_test(sample_entry("s1", "t1"))
        .await;
    let app = router(state);
    let resp = app
        .oneshot(get("tok", "/sessions/s1/tasks/t1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["id"], "t1");
    assert_eq!(body["status"], "completed");
}

#[tokio::test]
async fn test_cancel_task_not_found() {
    let state = crate::support::test_state("tok");
    wire_agent_manager(&state);
    let app = router(state);
    let resp = app
        .oneshot(del("tok", "/sessions/s1/tasks/nope"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_cancel_task_wrong_session_forbidden() {
    let state = crate::support::test_state("tok");
    wire_agent_manager(&state);
    let manager = state.session_processor.agent_manager.get().unwrap().clone();
    manager
        .seed_completed_for_test(sample_entry("other", "t1"))
        .await;
    let app = router(state);
    let resp = app
        .oneshot(del("tok", "/sessions/s1/tasks/t1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_cancel_task_finished_reports_internal_error() {
    // A seeded (already-completed) task has no live cancel flag, so
    // `AgentManager::cancel_agent` fails; the route maps that to a 500.
    let state = crate::support::test_state("tok");
    wire_agent_manager(&state);
    let manager = state.session_processor.agent_manager.get().unwrap().clone();
    manager
        .seed_completed_for_test(sample_entry("s1", "t1"))
        .await;
    let app = router(state);
    let resp = app
        .oneshot(del("tok", "/sessions/s1/tasks/t1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = json_body(resp).await;
    assert_eq!(body["error"], "internal server error");
}

#[tokio::test]
async fn test_spawn_task_without_agent_manager_500() {
    // No `wire_agent_manager`: the handler must answer 500 on the missing
    // manager rather than panic.
    let state = crate::support::test_state("tok");
    state.storage.create_session("s1", "/work").unwrap();
    let app = router(state);
    let resp = app
        .oneshot(post(
            "tok",
            "/sessions/s1/tasks",
            r#"{"agent":"general","task":"hi"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_spawn_task_session_not_found() {
    let state = crate::support::test_state("tok");
    wire_agent_manager(&state);
    let app = router(state);
    let resp = app
        .oneshot(post(
            "tok",
            "/sessions/missing/tasks",
            r#"{"agent":"general","task":"hi"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// -- Orchestrator ---------------------------------------------------------

#[tokio::test]
async fn test_orch_start_disabled_503() {
    let app = router(crate::support::test_state("tok"));
    let resp = app
        .oneshot(post(
            "tok",
            "/orchestrator/start",
            r#"{"required_capabilities":[],"payload":"x"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = json_body(resp).await;
    assert_eq!(body["error"], "orchestrator not enabled");
}

#[tokio::test]
async fn test_orch_job_disabled_503() {
    let app = router(crate::support::test_state("tok"));
    let resp = app
        .oneshot(get("tok", "/orchestrator/jobs/j1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

// -- Static fallback ------------------------------------------------------

#[tokio::test]
async fn test_unknown_path_falls_back_to_static() {
    let app = router(crate::support::test_state("tok"));
    // The static fallback serves index.html for unknown paths.
    let resp = app.oneshot(get("tok", "/not-a-route")).await.unwrap();
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::NOT_FOUND,
        "unexpected status: {}",
        resp.status()
    );
}

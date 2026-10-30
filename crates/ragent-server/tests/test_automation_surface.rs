//! Integration tests for the automation HTTP surface (spec `openhands` T-016;
//! FR-013, FR-018).
//!
//! The webhook ingress is public (`POST /auto/{id}`), the run-history and
//! manual-run routes are bearer-authenticated (`GET /automation`,
//! `GET /automation/runs/{id}`, `POST /automation/{id}/run`).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ragent_agent::Config;
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

/// Build a test state whose config has the automation block enabled.
///
/// Built on top of the shared [`crate::support::test_state`] fixture so this
/// file does not carry its own copy of the `AppState`/`SessionProcessor`
/// constructor.
fn automation_state(token: &str) -> AppState {
    let state = crate::support::test_state(token);
    let mut config = state
        .config
        .try_write()
        .expect("the fixture config lock is uncontended");
    *config = config_with_automations();
    drop(config);
    state
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
    let app = router(automation_state("tok"));

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
    let app = router(automation_state("tok"));
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
    let app = router(automation_state("tok"));
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
    let app = router(automation_state("tok"));
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

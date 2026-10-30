//! M6 hardening tests for `ragent-server` (ANTIPAT F-M8/F-M9/F-M10).
//!
//! Covers the explicit CORS origin policy, the `/events` SSE connection cap and
//! `?session=` filter, and the request body limit. Uses an in-process Axum
//! router via [`tower::ServiceExt::oneshot`].

use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures::StreamExt;
use ragent_agent::event::Event;
use ragent_server::routes::router;
use tower::ServiceExt;

/// A dropped `/events` response releases its connection slot, so a fresh
/// connection on an idle router still succeeds (F-M9).
#[tokio::test]
async fn test_events_connection_slot_released_on_drop() {
    let app = router(crate::support::test_state("tok"));
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
    let state = crate::support::test_state("tok");
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
    let app = router(crate::support::test_state("tok"));
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
    let app = router(crate::support::test_state("tok"));
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
    let state = crate::support::test_state("tok");
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

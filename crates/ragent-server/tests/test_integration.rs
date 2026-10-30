//! Integration tests for auth middleware, rate limiting, and SSE stream.
//!
//! Uses an in-process Axum router with [`tower::ServiceExt::oneshot`] to
//! exercise HTTP endpoints without opening a TCP socket.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ragent_agent::event::Event;
use ragent_server::routes::router;
use tower::ServiceExt;

// -- Auth middleware tests -------------------------------------------------

#[tokio::test]
async fn test_health_no_auth_required() {
    let app = router(crate::support::test_state("secret"));
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_protected_route_rejects_missing_auth() {
    let app = router(crate::support::test_state("secret"));
    let req = Request::builder()
        .uri("/sessions")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_protected_route_rejects_wrong_token() {
    let app = router(crate::support::test_state("correct-token"));
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
    let app = router(crate::support::test_state("secret"));
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
    let app = router(crate::support::test_state("my-secret"));
    let req = Request::builder()
        .uri("/sessions")
        .header("Authorization", "Bearer my-secret")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // GET /sessions should return 200 with an empty list, not 401.
    assert_eq!(resp.status(), StatusCode::OK);
}

// -- Rate limiter tests ---------------------------------------------------

#[tokio::test]
async fn test_rate_limiter_allows_under_limit() {
    let state = crate::support::test_state("tok");
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
    // 59 prior requests, so this (the 60th) is still within the 60/min limit.
    assert_ne!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_rate_limiter_rejects_over_limit() {
    let state = crate::support::test_state("tok");
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
    let state = crate::support::test_state("tok");
    state.storage.create_session("s1", "/tmp").unwrap();

    let rate_limiter = state.rate_limiter.clone();
    {
        let mut lim = rate_limiter.lock().await;
        // 100 requests but the window started 61 seconds ago -> should reset.
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
    // Window expired -> count resets -> request allowed.
    assert_ne!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

// -- SSE stream tests -----------------------------------------------------

#[tokio::test]
async fn test_events_stream_requires_auth() {
    let app = router(crate::support::test_state("secret"));
    let req = Request::builder()
        .uri("/events")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_events_stream_returns_sse() {
    let state = crate::support::test_state("tok");
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

    // Publish an event - the stream should eventually contain it.
    bus.publish(Event::SessionCreated {
        session_id: "test-sse".into(),
    });
}

// -- Case-insensitive Bearer tests ----------------------------------------

#[tokio::test]
async fn test_auth_accepts_lowercase_bearer() {
    let app = router(crate::support::test_state("my-secret"));
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
    let app = router(crate::support::test_state("my-secret"));
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
    let app = router(crate::support::test_state("my-secret"));
    let req = Request::builder()
        .uri("/sessions")
        .header("Authorization", "BeArEr my-secret")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// -- create_session validation tests --------------------------------------

#[tokio::test]
async fn test_create_session_invalid_path() {
    let app = router(crate::support::test_state("tok"));
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
    let app = router(crate::support::test_state("tok"));
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
    let app = router(crate::support::test_state("tok"));
    // SEC-062: a directory must be inside the configured project root, so use a
    // subdirectory of the server's working directory rather than `/tmp`.
    let dir = std::env::current_dir()
        .expect("cwd")
        .join("target")
        .join("temp");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let body = format!(r#"{{"directory":"{}"}}"#, dir.display());
    let req = Request::builder()
        .method("POST")
        .uri("/sessions")
        .header("Authorization", "Bearer tok")
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

// -- FUNC-066: token comparison + rate-limit boundary ------------------------

#[tokio::test]
async fn func066_auth_accepts_token_of_any_length_and_rejects_others() {
    // The constant-time comparison hashes both sides, so tokens of differing
    // lengths are still correctly compared.
    let app = router(crate::support::test_state(
        "a-much-longer-correct-token-value",
    ));

    let req_ok = Request::builder()
        .uri("/sessions")
        .header("Authorization", "Bearer a-much-longer-correct-token-value")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req_ok).await.unwrap().status(),
        StatusCode::OK
    );

    // A same-prefix token of different length must be rejected.
    let req_bad = Request::builder()
        .uri("/sessions")
        .header("Authorization", "Bearer a-much-longer-correct-token-valu")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(req_bad).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn func066_rate_limiter_enforces_exactly_60() {
    // 59 prior requests -> the 60th is allowed.
    let state = crate::support::test_state("tok");
    state.storage.create_session("s1", "/tmp").unwrap();
    {
        let mut lim = state.rate_limiter.lock().await;
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
    assert_ne!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::TOO_MANY_REQUESTS
    );

    // 60 prior requests -> the next (61st) is rejected.
    let state = crate::support::test_state("tok");
    state.storage.create_session("s1", "/tmp").unwrap();
    {
        let mut lim = state.rate_limiter.lock().await;
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
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}

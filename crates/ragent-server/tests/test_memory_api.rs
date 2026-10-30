//! Integration tests for memory REST endpoints.
//!
//! Tests exercise the full request/response cycle through the Axum router
//! using `tower::ServiceExt::oneshot` without opening a TCP socket.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ragent_server::routes::router;
use tower::ServiceExt;

/// Add auth header to a request builder.
///
/// Currently unused in this file but kept as a documented support helper for
/// future authenticated endpoint tests.
#[allow(dead_code)]
fn add_auth(req: axum::http::request::Builder, token: &str) -> axum::http::request::Builder {
    req.header("Authorization", format!("Bearer {token}"))
}

// -- Memory Block Endpoints --------------------------------------------

#[tokio::test]
async fn test_memory_search_requires_auth() {
    let app = router(crate::support::test_state("secret"));
    let req = Request::builder()
        .uri("/memory/search?q=test")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_memory_search_empty_results() {
    let app = router(crate::support::test_state("tok"));
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
    let app = router(crate::support::test_state("secret"));
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
    let state = crate::support::test_state("tok");

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
    let app = router(crate::support::test_state("tok"));
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
    let app = router(crate::support::test_state("tok"));
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
    let app = router(crate::support::test_state("secret"));
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
    let app = router(crate::support::test_state("tok"));
    let req = Request::builder()
        .method("DELETE")
        .uri("/memory/9999")
        .header("Authorization", "Bearer tok")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

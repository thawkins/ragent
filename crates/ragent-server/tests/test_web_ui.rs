//! Tests for the web UI static file serving and integration.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ragent_server::routes::router;
use tower::ServiceExt;

#[tokio::test]
async fn test_health_endpoint_route_exists() {
    // `GET /health` is registered on the real router and answers without auth.
    // Driving it through `oneshot` proves the route exists and is wired to a
    // handler, rather than merely that the router factory type-checks.
    let app = router(crate::support::test_state("secret"));
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .expect("build request");
    let resp = app.oneshot(req).await.expect("route /health");
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_static_files_directory_exists() {
    // Verify the static directory exists and contains index.html
    let static_path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/static/index.html");
    assert!(
        static_path.exists(),
        "Static index.html should exist at {:?}",
        static_path
    );
}

#[tokio::test]
async fn test_index_html_content() {
    // Verify index.html contains expected content
    let static_path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/static/index.html");
    let content = std::fs::read_to_string(&static_path).expect("Should be able to read index.html");

    assert!(
        content.contains("RAgent"),
        "index.html should contain RAgent title"
    );
    assert!(
        content.contains("AI Coding Agent"),
        "index.html should describe the agent"
    );
    assert!(
        content.contains("EventSource"),
        "index.html should use SSE for real-time updates"
    );
    assert!(
        content.contains("/events"),
        "index.html should connect to /events endpoint"
    );
    assert!(
        content.contains("/sessions"),
        "index.html should use /sessions API"
    );
}

#[tokio::test]
async fn test_serve_dir_feature_enabled() {
    // `tower_http::services::ServeDir` is reachable, and a root that does not
    // exist answers 404 rather than panicking, which is the behaviour the
    // static-file fallback depends on.
    use tower_http::services::ServeDir;
    let missing = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/temp/no-such-static-root");
    let service = ServeDir::new(missing);
    let req = Request::builder()
        .uri("/index.html")
        .body(Body::empty())
        .expect("build request");
    let resp = service.oneshot(req).await.expect("ServeDir responds");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

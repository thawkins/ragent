use ragent_core::event::EventBus;
use ragent_core::tool::webfetch::WebFetchTool;
use ragent_core::tool::{Tool, ToolContext};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn make_ctx() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        task_manager: None,
    }
}

fn tool() -> WebFetchTool {
    WebFetchTool
}

// ── Tool trait ───────────────────────────────────────────────────

#[test]
fn test_webfetch_name_and_permission() {
    let t = tool();
    assert_eq!(t.name(), "webfetch");
    assert_eq!(t.permission_category(), "web");
}

#[test]
fn test_webfetch_schema() {
    let schema = tool().parameters_schema();
    let props = &schema["properties"];
    assert!(props["url"].is_object());
    assert!(props["format"].is_object());
    assert!(props["max_length"].is_object());
    assert!(props["timeout"].is_object());

    let required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(required.contains(&"url"));
    assert!(!required.contains(&"format"));
}

// ── Parameter validation ─────────────────────────────────────────

#[tokio::test]
async fn test_webfetch_missing_url() {
    let ctx = make_ctx();
    let result = tool().execute(json!({}), &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("url"));
}

#[tokio::test]
async fn test_webfetch_invalid_scheme() {
    let ctx = make_ctx();
    let result = tool()
        .execute(json!({ "url": "ftp://example.com" }), &ctx)
        .await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("http:// and https://"));
}

#[tokio::test]
async fn test_webfetch_invalid_scheme_file() {
    let ctx = make_ctx();
    let result = tool()
        .execute(json!({ "url": "file:///etc/passwd" }), &ctx)
        .await;
    assert!(result.is_err());
}

// ── Local HTTP server integration tests ──────────────────────────

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Start a minimal async HTTP server that serves the given body with the
/// given content type. Returns the URL. The server handles exactly one request.
async fn start_test_server(body: &'static str, content_type: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf).await;

            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: {}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 \r\n\
                 {}",
                content_type,
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        }
    });

    // Give the spawned task a moment to start listening
    tokio::task::yield_now().await;
    url
}

async fn start_error_server(status: u16) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf).await;

            let response = format!(
                "HTTP/1.1 {} Not Found\r\n\
                 Content-Length: 0\r\n\
                 Connection: close\r\n\
                 \r\n",
                status
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        }
    });

    tokio::task::yield_now().await;
    url
}

#[tokio::test]
async fn test_webfetch_plain_text() {
    let body: &'static str = "Hello from the test server!\nLine 2\nLine 3\n";
    let url = start_test_server(body, "text/plain").await;

    let ctx = make_ctx();
    let result = tool()
        .execute(json!({ "url": url }), &ctx)
        .await
        .unwrap();

    assert!(result.content.contains("Hello from the test server!"));
    assert!(result.content.contains("Line 3"));

    let meta = result.metadata.unwrap();
    assert_eq!(meta["status"], 200);
    assert!(meta["content_type"].as_str().unwrap().contains("text/plain"));
}

#[tokio::test]
async fn test_webfetch_html_to_text() {
    let body: &'static str = "<html><body><h1>Title</h1><p>Paragraph text.</p></body></html>";
    let url = start_test_server(body, "text/html").await;

    let ctx = make_ctx();
    let result = tool()
        .execute(json!({ "url": url }), &ctx)
        .await
        .unwrap();

    // Should contain the text content, not HTML tags
    assert!(result.content.contains("Title"));
    assert!(result.content.contains("Paragraph text"));
    assert!(!result.content.contains("<h1>"));
    assert!(!result.content.contains("<p>"));
}

#[tokio::test]
async fn test_webfetch_html_raw_format() {
    let body: &'static str = "<html><body><h1>Title</h1></body></html>";
    let url = start_test_server(body, "text/html").await;

    let ctx = make_ctx();
    let result = tool()
        .execute(json!({ "url": url, "format": "raw" }), &ctx)
        .await
        .unwrap();

    // Should keep raw HTML
    assert!(result.content.contains("<h1>"));
    assert!(result.content.contains("</h1>"));
}

#[tokio::test]
async fn test_webfetch_max_length_truncation() {
    // Build a long body at compile time via a leaked allocation
    let long: &'static str = Box::leak("A".repeat(1000).into_boxed_str());
    let url = start_test_server(long, "text/plain").await;

    let ctx = make_ctx();
    let result = tool()
        .execute(json!({ "url": url, "max_length": 100 }), &ctx)
        .await
        .unwrap();

    assert!(result.content.len() < 200);
    assert!(result.content.contains("[Content truncated]"));
}

#[tokio::test]
async fn test_webfetch_metadata() {
    let body: &'static str = "test content\n";
    let url = start_test_server(body, "text/plain").await;

    let ctx = make_ctx();
    let result = tool()
        .execute(json!({ "url": url }), &ctx)
        .await
        .unwrap();

    let meta = result.metadata.unwrap();
    assert_eq!(meta["status"], 200);
    assert!(meta["lines"].as_u64().unwrap() >= 1);
    assert!(meta["url"].as_str().unwrap().starts_with("http://"));
}

#[tokio::test]
async fn test_webfetch_connection_refused() {
    let ctx = make_ctx();
    let result = tool()
        .execute(
            json!({ "url": "http://127.0.0.1:1", "timeout": 2 }),
            &ctx,
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_webfetch_http_error() {
    let url = start_error_server(404).await;
    let ctx = make_ctx();
    let result = tool()
        .execute(json!({ "url": url }), &ctx)
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("404"));
}

#![allow(clippy::assert_is_empty)]
//! Integration tests for the HTTP MCP transport client.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ragent_agent::mcp::McpClientBackend;
use ragent_agent::mcp::http::HttpMcpClient;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Start a one-shot mock HTTP server and return its address.
///
/// The server reads a single HTTP request, then writes `response` and closes.
async fn mock_server_once(response: String) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 4096];
        let n = socket.read(&mut buf).await.unwrap_or(0);
        // Ignore the request body; we only need to respond.
        let _request = String::from_utf8_lossy(&buf[..n]);
        let response_bytes = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            response.len(),
            response
        );
        let _ = socket.write_all(response_bytes.as_bytes()).await;
    });

    addr
}

/// Start a one-shot mock HTTP server and return the request bytes via a channel.
async fn mock_server_once_capture(response: String) -> (SocketAddr, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 4096];
        let n = socket.read(&mut buf).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]).to_string();
        let _ = tx.send(request);
        let response_bytes = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            response.len(),
            response
        );
        let _ = socket.write_all(response_bytes.as_bytes()).await;
    });

    (addr, rx)
}

#[tokio::test]
async fn test_http_mcp_client_lists_tools() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"echo","description":"Echoes input","inputSchema":{"type":"object"}}]}}"#;
    let addr = mock_server_once(body.to_string()).await;

    let client = HttpMcpClient::new(format!("http://{addr}"), HashMap::new());
    let tools = client.list_tools().await;

    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo");
    assert_eq!(tools[0].description, "Echoes input");
}

#[tokio::test]
async fn test_http_mcp_client_calls_tool() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"hello world"}],"isError":false}}"#;
    let addr = mock_server_once(body.to_string()).await;

    let client = HttpMcpClient::new(format!("http://{addr}"), HashMap::new());
    let result = client
        .call_tool("srv", "echo", json!({"msg": "hello"}))
        .await
        .expect("tool call should succeed");

    let content = result.get("content").unwrap().as_array().unwrap();
    assert_eq!(
        content[0].get("text").unwrap().as_str().unwrap(),
        "hello world"
    );
}

#[tokio::test]
async fn test_http_mcp_client_sends_custom_headers() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
    let (addr, rx) = mock_server_once_capture(body.to_string()).await;

    let mut headers = HashMap::new();
    headers.insert(
        "Authorization".to_string(),
        "Bearer secret-token".to_string(),
    );
    headers.insert("X-Custom".to_string(), "value".to_string());

    let client = HttpMcpClient::new(format!("http://{addr}"), headers);
    let tools = client.list_tools().await;
    assert!(tools.is_empty());

    let request = rx.await.unwrap();
    let lower = request.to_lowercase();
    assert!(lower.contains("authorization: bearer secret-token"));
    assert!(lower.contains("x-custom: value"));
}

/// Start a mock server that always returns the same non-2xx status for
/// every request (supports multiple connections for retry testing).
async fn mock_server_always_status(status: String, response: String) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let accept_result = listener.accept().await;
            if accept_result.is_err() {
                break;
            }
            let (mut socket, _) = accept_result.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let _request = String::from_utf8_lossy(&buf[..n]);
            let response_bytes = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                response.len(),
                response
            );
            let _ = socket.write_all(response_bytes.as_bytes()).await;
        }
    });

    addr
}

/// Return an HTTP reason phrase for a small set of status codes used in these
/// tests.
fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "200 OK",
        503 => "503 Service Unavailable",
        _ => "500 Internal Server Error",
    }
}

/// Start a mock HTTP server that returns a fixed sequence of responses, one per
/// connection.
///
/// The server accepts connections until `responses.len()` requests have been
/// served. Each request is read and discarded; the matching response is sent and
/// the connection closes. This is useful for testing retry / reconnect flows
/// where the same server must first fail and then recover.
async fn mock_server_sequential(responses: Vec<(u16, String)>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let idx = Arc::new(AtomicUsize::new(0));
    let count = responses.len();

    tokio::spawn(async move {
        loop {
            let i = idx.fetch_add(1, Ordering::SeqCst);
            if i >= count {
                break;
            }
            let (status, body) = responses.get(i).unwrap();
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            // Ignore the request body; we only need to respond in order.
            let _request = String::from_utf8_lossy(&buf[..n]);
            let response_bytes = format!(
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                status_reason(*status),
                body.len(),
                body
            );
            let _ = socket.write_all(response_bytes.as_bytes()).await;
        }
    });

    addr
}

#[tokio::test]
async fn test_http_mcp_client_reports_non_2xx() {
    let body = r#"{"error":"unavailable"}"#;
    let addr =
        mock_server_always_status("503 Service Unavailable".to_string(), body.to_string()).await;

    let client = HttpMcpClient::new(format!("http://{addr}"), HashMap::new());
    // call_tool propagates errors. With auto-reconnect (FR-014) the client
    // retries 3 times, but the mock always returns 503, so all retries fail.
    // The final error should mention either the 503 status or the disconnect.
    let result = client.call_tool("srv", "echo", json!({})).await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("503") || err.contains("disconnect"),
        "error should mention 503 status or disconnect: {err}"
    );
    // The client should be marked disconnected after exhausting retries.
    assert!(client.is_disconnected());
}

#[tokio::test]
async fn test_http_mcp_client_reports_json_rpc_error() {
    let body = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#;
    let addr = mock_server_once(body.to_string()).await;

    let client = HttpMcpClient::new(format!("http://{addr}"), HashMap::new());
    let result = client.list_tools_for_server("srv").await;

    assert!(result.is_empty());
}

#[tokio::test]
async fn test_http_mcp_client_round_trip() {
    let responses = vec![
        (
            200u16,
            r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"echo","description":"Echoes input","inputSchema":{"type":"object"}}]}}"#.to_string(),
        ),
        (
            200u16,
            r#"{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"hello world"}],"isError":false}}"#.to_string(),
        ),
    ];
    let addr = mock_server_sequential(responses).await;

    let client = HttpMcpClient::new(format!("http://{addr}"), HashMap::new());

    let tools = client.list_tools().await;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo");

    let result = client
        .call_tool("srv", "echo", json!({"msg": "hello"}))
        .await
        .expect("tool call should succeed");
    let content = result.get("content").unwrap().as_array().unwrap();
    assert_eq!(
        content[0].get("text").unwrap().as_str().unwrap(),
        "hello world"
    );
}

#[tokio::test]
async fn test_http_mcp_client_reconnects_after_503() {
    let error_body = r#"{"error":"unavailable"}"#;
    let list_body = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"echo","description":"Echoes input","inputSchema":{"type":"object"}}]}}"#;
    let call_body = r#"{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"hello world"}],"isError":false}}"#;

    // Four 503 responses exhaust the auto-retry attempts (1 initial + 3
    // retries with 1s/2s/4s backoff). The next two 200 responses let the
    // client reconnect and complete a full tool-list + tool-call round trip.
    let responses = vec![
        (503u16, error_body.to_string()),
        (503u16, error_body.to_string()),
        (503u16, error_body.to_string()),
        (503u16, error_body.to_string()),
        (200u16, list_body.to_string()),
        (200u16, call_body.to_string()),
    ];
    let addr = mock_server_sequential(responses).await;

    let client = HttpMcpClient::new(format!("http://{addr}"), HashMap::new());

    // First attempt fails and marks the client disconnected.
    let tools = client.list_tools().await;
    assert!(
        tools.is_empty(),
        "list_tools should return empty after 503 retries"
    );
    assert!(
        client.is_disconnected(),
        "client should be marked disconnected after exhausting retries"
    );

    // After the server recovers, the next request clears the disconnected flag
    // and succeeds.
    let tools = client.list_tools().await;
    assert_eq!(tools.len(), 1, "client should reconnect and list tools");
    assert_eq!(tools[0].name, "echo");
    assert!(
        !client.is_disconnected(),
        "client should no longer be disconnected after a successful request"
    );

    let result = client
        .call_tool("srv", "echo", json!({"msg": "hello"}))
        .await
        .expect("tool call after reconnect should succeed");
    let content = result.get("content").unwrap().as_array().unwrap();
    assert_eq!(
        content[0].get("text").unwrap().as_str().unwrap(),
        "hello world"
    );
}

#[tokio::test]
async fn test_http_mcp_client_reuses_shared_reqwest_client() {
    let shared_client = reqwest::Client::new();

    let body_a = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"echo","description":"Echoes input","inputSchema":{"type":"object"}}]}}"#;
    let addr_a = mock_server_once(body_a.to_string()).await;
    let client_a = HttpMcpClient::new(format!("http://{addr_a}"), HashMap::new())
        .with_client(shared_client.clone());

    let body_b = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"hello world"}],"isError":false}}"#;
    let addr_b = mock_server_once(body_b.to_string()).await;
    let client_b =
        HttpMcpClient::new(format!("http://{addr_b}"), HashMap::new()).with_client(shared_client);

    let tools = client_a.list_tools().await;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo");

    let result = client_b
        .call_tool("srv", "echo", json!({}))
        .await
        .expect("tool call should succeed");
    let content = result.get("content").unwrap().as_array().unwrap();
    assert_eq!(
        content[0].get("text").unwrap().as_str().unwrap(),
        "hello world"
    );
}

#[tokio::test]
async fn test_http_mcp_client_with_custom_client_sends_headers() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
    let (addr, rx) = mock_server_once_capture(body.to_string()).await;

    let shared_client = reqwest::Client::builder()
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(
                "x-shared-token",
                reqwest::header::HeaderValue::from_static("shared"),
            );
            h
        })
        .build()
        .expect("build client");

    let client =
        HttpMcpClient::new(format!("http://{addr}"), HashMap::new()).with_client(shared_client);
    let _ = client.list_tools().await;

    let request = rx.await.unwrap();
    assert!(request.to_lowercase().contains("x-shared-token: shared"));
}

//! End-to-end coverage for the HTTP MCP transport (spec `openhands` FR-025).
//!
//! FR-025 requires that an MCP server configured with an `http` transport
//! connects over HTTP "in addition to the existing `stdio` transport". The unit
//! tests in `test_mcp_http.rs` exercise the [`HttpMcpClient`] in isolation; the
//! adopt tests in `test_mcp_tool_index.rs` pin the already-running-server path.
//! This file drives the whole thing through [`McpClient`] - the same entry point
//! the session uses - against a live HTTP mock: connect, discover tools, invoke a
//! tool, and disconnect, so the transport is proven to work *alongside* stdio
//! rather than only in a unit fixture.
//!
//! [`HttpMcpClient`]: ragent_agent::mcp::http::HttpMcpClient

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::post;
use ragent_agent::mcp::{McpClient, McpStatus};
use ragent_config::{McpServerConfig, McpTransport};
use serde_json::json;

/// A minimal Streamable-HTTP MCP server: `initialize`, `tools/list`, `tools/call`.
#[derive(Clone, Default)]
struct Seen {
    initializes: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
}

async fn handler(State(seen): State<Seen>, headers: HeaderMap, body: String) -> impl IntoResponse {
    let request: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let id = request.get("id").cloned().unwrap_or(json!(1));
    match request.get("method").and_then(|m| m.as_str()).unwrap_or("") {
        "initialize" => {
            seen.initializes.fetch_add(1, Ordering::SeqCst);
            let frame = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "protocolVersion": "2024-11-05" }
            });
            let mut response = (
                [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                format!("event: message\ndata: {frame}\n\n"),
            )
                .into_response();
            response.headers_mut().insert(
                "mcp-session-id",
                axum::http::HeaderValue::from_static("http-transport-session"),
            );
            response
        }
        "tools/list" => {
            assert!(
                headers.contains_key("mcp-session-id"),
                "every request after initialize must replay the negotiated session"
            );
            let frame = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [
                        { "name": "echo", "description": "Echo a value", "inputSchema": { "type": "object" } }
                    ]
                }
            });
            (
                [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                format!("event: message\ndata: {frame}\n\n"),
            )
                .into_response()
        }
        "tools/call" => {
            seen.calls.fetch_add(1, Ordering::SeqCst);
            let name = request
                .get("params")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let frame = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": format!("called {name}") }],
                    "isError": false
                }
            });
            (
                [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                format!("event: message\ndata: {frame}\n\n"),
            )
                .into_response()
        }
        other => panic!("unexpected MCP method {other}"),
    }
}

/// Boot the mock and return its address.
async fn spawn_server(seen: Seen) -> SocketAddr {
    let router = Router::new().route("/mcp", post(handler)).with_state(seen);
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind test server");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    addr
}

/// FR-025: an `http`-transport server connects, exposes its tools, and answers a
/// tool call through the ordinary [`McpClient`] surface - exactly as a `stdio`
/// server would.
#[tokio::test]
async fn http_transport_connects_lists_and_calls_tools() {
    let seen = Seen::default();
    let addr = spawn_server(seen.clone()).await;

    let config = McpServerConfig {
        type_: McpTransport::Http,
        url: Some(format!("http://{addr}/mcp")),
        ..McpServerConfig::default()
    };

    let mut client = McpClient::new();
    client
        .connect("http-server", config)
        .await
        .expect("the http MCP server must connect");

    let server = client
        .servers()
        .iter()
        .find(|s| s.id == "http-server")
        .expect("the http server is registered");
    assert_eq!(server.status, McpStatus::Connected);
    assert_eq!(server.tools.len(), 1, "the http server's tools are listed");
    assert_eq!(server.tools[0].name, "echo");

    // The tool is dispatchable through the same name-based entry point stdio
    // servers use.
    let result = client
        .call_tool_by_name("echo", json!({ "value": "hi" }))
        .await
        .expect("the http tool call must succeed");
    let text = result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert_eq!(text, "called echo");
    assert_eq!(seen.calls.load(Ordering::SeqCst), 1);

    client.disconnect("http-server").await.expect("disconnect");
    assert!(
        client
            .servers()
            .iter()
            .find(|s| s.id == "http-server")
            .is_none_or(|s| s.status != McpStatus::Connected)
    );
}

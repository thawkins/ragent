//! Coverage for the MCP client refresh paths and PATH-based discovery.
//!
//! `McpClient::refresh_tools` and `refresh_tools_for_server` are public but were
//! not exercised by any existing test. This file drives them against a live
//! in-process Streamable-HTTP MCP server (the same shape as
//! `test_mcp_http_transport.rs`) so a refresh re-queries the server and updates
//! the cached tool manifest.
//!
//! The discovery test prepends a tempdir holding a fake known MCP executable to
//! `PATH` so the `SystemPath` discovery row is hit deterministically. `PATH` is
//! process-global, so those tests are `#[serial_test::serial]` and opt into
//! `unsafe` for `std::env::set_var` (Rust 2024).
//!
//! No network beyond loopback; no real MCP server is spawned.

#![allow(unsafe_code)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::post;
use ragent_agent::mcp::discovery::{McpDiscoverySource, discover, discover_locations};
use ragent_agent::mcp::{McpClient, McpStatus};
use ragent_config::{McpServerConfig, McpTransport};
use serde_json::json;

/// Mock MCP server state: each `tools/list` returns an incrementing tool name so
/// a refresh is observable as a change in the cached manifest.
#[derive(Clone, Default)]
struct Mock {
    lists: Arc<AtomicUsize>,
}

async fn handler(State(mock): State<Mock>, headers: HeaderMap, body: String) -> impl IntoResponse {
    let request: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let id = request.get("id").cloned().unwrap_or(json!(1));
    let sse = |frame: serde_json::Value| {
        (
            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
            format!("event: message\ndata: {frame}\n\n"),
        )
            .into_response()
    };
    match request.get("method").and_then(|m| m.as_str()).unwrap_or("") {
        "initialize" => {
            let mut response = sse(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "protocolVersion": "2024-11-05" }
            }));
            response.headers_mut().insert(
                "mcp-session-id",
                axum::http::HeaderValue::from_static("refresh-session"),
            );
            response
        }
        "tools/list" => {
            assert!(
                headers.contains_key("mcp-session-id"),
                "every request after initialize must replay the negotiated session"
            );
            let n = mock.lists.fetch_add(1, Ordering::SeqCst) + 1;
            sse(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [{
                        "name": format!("tool-{n}"),
                        "description": "refreshable",
                        "inputSchema": { "type": "object" }
                    }]
                }
            }))
        }
        "tools/call" => sse(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "content": [{ "type": "text", "text": "ok" }], "isError": false }
        })),
        other => panic!("unexpected MCP method {other}"),
    }
}

async fn spawn_server(mock: Mock) -> SocketAddr {
    let router = Router::new().route("/mcp", post(handler)).with_state(mock);
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind test server");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    addr
}

fn http_config(addr: SocketAddr) -> McpServerConfig {
    McpServerConfig {
        type_: McpTransport::Http,
        url: Some(format!("http://{addr}/mcp")),
        ..McpServerConfig::default()
    }
}

/// FR-013/FR-014: `refresh_tools` re-queries every connected server and updates
/// its cached manifest. `refresh_tools_for_server` does the same for one server.
#[tokio::test]
async fn refresh_tools_requeries_connected_http_server() {
    let mock = Mock::default();
    let addr = spawn_server(mock.clone()).await;

    let mut client = McpClient::new();
    client
        .connect("refreshable", http_config(addr))
        .await
        .expect("connect");
    // connect() lists once -> tool-1.
    assert_eq!(
        client.list_tools_for_server("refreshable")[0].name,
        "tool-1"
    );

    client.refresh_tools().await.expect("refresh all");
    // refresh_tools() lists again -> tool-2.
    assert_eq!(
        client.list_tools_for_server("refreshable")[0].name,
        "tool-2",
        "refresh_tools must update the cached manifest"
    );

    let tools = client
        .refresh_tools_for_server("refreshable")
        .await
        .expect("refresh one");
    assert_eq!(tools[0].name, "tool-3");
    assert_eq!(
        client.list_tools_for_server("refreshable")[0].name,
        "tool-3",
        "refresh_tools_for_server must update the cached manifest"
    );

    // The name-based index was rebuilt from the refreshed manifests.
    let result = client
        .call_tool_by_name("tool-3", json!({"value": "x"}))
        .await
        .expect("call refreshed tool");
    assert_eq!(result["content"][0]["text"], "ok");

    client.disconnect("refreshable").await.expect("disconnect");
}

/// `refresh_tools` skips non-connected servers and `refresh_tools_for_server`
/// errors for a server with no live connection.
#[tokio::test]
async fn refresh_skips_disconnected_and_errors_for_unknown() {
    let mut client = McpClient::new();

    // No connected servers: refresh_tools is a no-op success.
    client
        .refresh_tools()
        .await
        .expect("refresh with no servers");

    // A Disabled server has no connection and is skipped by refresh_tools.
    client.register_disabled("off", McpServerConfig::default());
    client
        .refresh_tools()
        .await
        .expect("refresh skips disabled");
    assert_eq!(
        client
            .servers()
            .iter()
            .find(|s| s.id == "off")
            .map(|s| s.status.clone()),
        Some(McpStatus::Disabled)
    );

    // A server id with no connection is an explicit error.
    let err = client
        .refresh_tools_for_server("off")
        .await
        .expect_err("disconnected server must not refresh");
    assert!(
        err.to_string().contains("not connected"),
        "unexpected error: {err}"
    );
}

// -- PATH-based discovery ----------------------------------------------------

/// Prepend a tempdir holding a fake known MCP executable to `PATH` so the
/// `SystemPath` discovery row finds it deterministically.
#[serial_test::serial]
#[tokio::test]
async fn discover_hits_path_row_for_fake_executable() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let exe = tmp.path().join("mcp-server-filesystem");
    std::fs::write(&exe, b"#!/bin/sh\nexit 0\n").expect("write fake exe");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake exe");
    }

    let original = std::env::var_os("PATH").unwrap_or_default();
    let mut entries = vec![tmp.path().to_path_buf()];
    entries.extend(std::env::split_paths(&original));
    let joined = std::env::join_paths(entries).expect("join PATH");
    // SAFETY: process-global mutation; serialised by `serial_test::serial`.
    unsafe {
        std::env::set_var("PATH", &joined);
    }

    let discovered = discover().await;
    let locations = discover_locations().await;

    // SAFETY: restore the original PATH before asserting so a panic cannot leave
    // a tempdir on PATH for the rest of the process.
    unsafe {
        std::env::set_var("PATH", &original);
    }

    let filesystem = discovered
        .iter()
        .find(|s| s.id == "filesystem")
        .expect("the fake filesystem server must be discovered on PATH");
    assert!(
        matches!(filesystem.source, McpDiscoverySource::SystemPath),
        "found via PATH, got: {:?}",
        filesystem.source
    );
    assert_eq!(filesystem.executable, exe);

    let path_row = locations
        .iter()
        .find(|l| l.label.starts_with("PATH"))
        .expect("a PATH row");
    assert!(
        path_row.servers.iter().any(|s| s.id == "filesystem"),
        "the PATH row must list the discovered server"
    );

    // A discovered server converts to a disabled stdio config.
    let config = filesystem.to_config();
    assert!(config.disabled);
    assert_eq!(config.type_, McpTransport::Stdio);
}

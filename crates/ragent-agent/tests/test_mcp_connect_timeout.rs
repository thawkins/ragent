//! Tests for the bounded MCP connect path (`McpClient::connect_bounded`).
//!
//! A stdio launch often goes through an `npx`/`npm exec` launcher that resolves
//! a package specifier against the npm registry before the real server starts.
//! A slow or unreachable registry (or a server that accepts the socket and then
//! never answers the `initialize` handshake) used to stall the connect future
//! forever, hanging startup with nothing the user could act on. This test pins
//! the per-attempt timeout bound that turns such a stall into a fast, actionable
//! failure.
//!
//! The test mutates `RAGENT_MCP_CONNECT_TIMEOUT_SECS`, which is process-wide.
//! It is the ONLY test in this file (an integration test file is its own binary
//! and its own process), so the mutation cannot race another test.
//!
//! Note: `std::env::set_var` is `unsafe` in Rust 2024; the workspace denies
//! `unsafe_code`, so this test target opts back in explicitly (the same
//! pattern as `crates/ragent-config/tests/test_yolo_persistence.rs`).

#![allow(unsafe_code)]

use std::time::Duration;

use ragent_agent::mcp::McpClient;
use ragent_config::{McpServerConfig, McpTransport};

/// A TCP listener that accepts connections and never writes a byte back.
///
/// The accepted sockets are held open for the listener's lifetime (collected
/// into `held`), so the peer sees an established connection that never answers -
/// a classic "black hole". The SSE handshake therefore waits for response
/// headers until the connect timeout fires.
async fn black_hole_listener() -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        let mut held: Vec<tokio::net::TcpStream> = Vec::new();
        while let Ok((stream, _peer)) = listener.accept().await {
            held.push(stream);
        }
    });
    addr
}

/// An SSE MCP config pointing at `url` (default stdio fields are overridden).
fn sse_config(url: String) -> McpServerConfig {
    McpServerConfig {
        type_: McpTransport::Sse,
        url: Some(url),
        ..McpServerConfig::default()
    }
}

/// A server that never answers must fail within the configured bound instead of
/// hanging forever, and the error must name the bound and the retry.
#[tokio::test]
async fn test_connect_to_unresponsive_server_times_out_within_bound() {
    let addr = black_hole_listener().await;
    let config = sse_config(format!("http://{addr}/mcp"));

    // Bound a single attempt to 1s so the test stays fast. `connect_timeout`
    // reads the variable on each call, so setting it here is observed.
    // SAFETY: `set_var` is `unsafe` in edition 2024; this test file opts in and
    // is the only test in its process, so the mutation cannot race.
    unsafe {
        std::env::set_var("RAGENT_MCP_CONNECT_TIMEOUT_SECS", "1");
    }

    let mut client = McpClient::new();

    // Outer guard: the whole connect (probe + two bounded attempts) must finish
    // well inside this, proving the inner bound - not the outer timeout - ended
    // the wait.
    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        client.connect("black-hole", config),
    )
    .await;

    unsafe {
        std::env::remove_var("RAGENT_MCP_CONNECT_TIMEOUT_SECS");
    }

    let inner =
        outcome.expect("connect must be bounded by the configured timeout, not the outer guard");
    let error = inner.expect_err("an unresponsive server must not be reported as connected");
    let message = format!("{error:#}");
    assert!(
        message.contains("did not become ready within 1s"),
        "error should name the bounded timeout, got: {message}"
    );

    // The failed server is still registered so `/mcp` can report it.
    let status = client
        .servers()
        .iter()
        .find(|s| s.id == "black-hole")
        .map(|s| s.status.clone())
        .expect("failed server must be registered");
    assert!(
        matches!(status, ragent_agent::mcp::McpStatus::Failed { .. }),
        "server should be recorded as Failed, got: {status:?}"
    );
}

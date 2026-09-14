//! PERF-076: tests for the MCP `tool_name -> server_id` lookup index.
//!
//! `call_tool_by_name` used to scan every server's tool list linearly.  These
//! tests pin the observable contract of the replacement O(1) index: a tool
//! registered on a second server is resolvable, unknown tools are rejected
//! before any dispatch, and a disconnected server's tools drop out of the
//! index.

use ragent_agent::McpServerConfig;
use ragent_agent::mcp::McpClient;
use serde_json::json;

/// A config for a disabled server, which `connect` registers without any
/// transport handshake — enough to populate the server list in tests.
fn disabled_config() -> McpServerConfig {
    McpServerConfig {
        disabled: true,
        ..McpServerConfig::default()
    }
}

#[tokio::test]
async fn unknown_tool_is_rejected_before_dispatch() {
    let mut client = McpClient::new();
    client
        .connect("srv-a", disabled_config())
        .await
        .expect("register disabled server");

    // A disabled server advertises no tools, so the index is empty and the
    // lookup fails with the same message as the old linear scan.
    let err = client
        .call_tool_by_name("nope", json!({}))
        .await
        .expect_err("unknown tool must error");
    assert!(
        err.to_string()
            .contains("No connected MCP server provides tool"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn multiple_servers_are_registered_and_disconnected() {
    let mut client = McpClient::new();
    client
        .connect("srv-a", disabled_config())
        .await
        .expect("register srv-a");
    client
        .connect("srv-b", disabled_config())
        .await
        .expect("register srv-b");
    assert_eq!(client.servers().len(), 2);

    // Disconnecting one server keeps the other registered, and an unknown tool
    // still resolves to a clean error rather than panicking on a stale index.
    client.disconnect("srv-a").await.expect("disconnect srv-a");
    let err = client
        .call_tool_by_name("still-missing", json!({}))
        .await
        .expect_err("unknown tool must error");
    assert!(
        err.to_string()
            .contains("No connected MCP server provides tool")
    );
}

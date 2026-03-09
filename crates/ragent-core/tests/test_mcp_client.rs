use ragent_core::config::{McpServerConfig, McpTransport};
use ragent_core::mcp::*;

// ── New client ───────────────────────────────────────────────────

#[test]
fn test_mcp_client_new() {
    let client = McpClient::new();
    assert!(client.servers().is_empty());
    assert!(client.list_tools().is_empty());
}

// ── Connect disabled server registers without connecting ────────

#[tokio::test]
async fn test_mcp_client_connect_disabled() {
    let mut client = McpClient::new();

    client
        .connect(
            "github",
            McpServerConfig {
                type_: McpTransport::Stdio,
                command: Some("gh-mcp".to_string()),
                args: vec!["--mode".to_string(), "stdio".to_string()],
                disabled: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(client.servers().len(), 1);
    assert_eq!(client.servers()[0].id, "github");
    assert_eq!(client.servers()[0].status, McpStatus::Disabled);
    assert!(client.servers()[0].tools.is_empty());
}

// ── Multiple disabled servers ───────────────────────────────────

#[tokio::test]
async fn test_mcp_client_multiple_disabled_servers() {
    let mut client = McpClient::new();

    for name in &["server1", "server2", "server3"] {
        client
            .connect(
                name,
                McpServerConfig {
                    disabled: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    assert_eq!(client.servers().len(), 3);
    let ids: Vec<&str> = client.servers().iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"server1"));
    assert!(ids.contains(&"server2"));
    assert!(ids.contains(&"server3"));
}

// ── List tools empty for disabled servers ───────────────────────

#[tokio::test]
async fn test_mcp_client_list_tools_empty_disabled() {
    let mut client = McpClient::new();
    client
        .connect(
            "test",
            McpServerConfig {
                disabled: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let tools = client.list_tools();
    assert!(tools.is_empty());
}

// ── Call tool on non-existent server returns error ───────────────

#[tokio::test]
async fn test_mcp_client_call_tool_not_connected() {
    let client = McpClient::new();
    let result = client
        .call_tool("missing", "any_tool", serde_json::json!({}))
        .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not connected"),
        "Expected 'not connected' error, got: {err_msg}"
    );
}

// ── call_tool_by_name errors when no server has the tool ────────

#[tokio::test]
async fn test_mcp_client_call_tool_by_name_not_found() {
    let client = McpClient::new();
    let result = client
        .call_tool_by_name("nonexistent_tool", serde_json::json!({}))
        .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("No connected MCP server"),
        "Expected 'No connected MCP server' error, got: {err_msg}"
    );
}

// ── call_tool_by_name skips disabled servers ─────────────────────

#[tokio::test]
async fn test_mcp_client_call_tool_by_name_skips_disabled() {
    let mut client = McpClient::new();
    client
        .connect(
            "disabled-srv",
            McpServerConfig {
                disabled: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let result = client
        .call_tool_by_name("any_tool", serde_json::json!({}))
        .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("No connected MCP server"));
}

// ── Connect with invalid command records failure ────────────────

#[tokio::test]
async fn test_mcp_client_connect_invalid_command() {
    let mut client = McpClient::new();

    let result = client
        .connect(
            "bad-server",
            McpServerConfig {
                type_: McpTransport::Stdio,
                command: Some("nonexistent-binary-xyz-999".to_string()),
                ..Default::default()
            },
        )
        .await;

    assert!(result.is_err());
    assert_eq!(client.servers().len(), 1);
    assert!(matches!(
        client.servers()[0].status,
        McpStatus::Failed { .. }
    ));
}

// ── Connect without command for stdio returns error ─────────────

#[tokio::test]
async fn test_mcp_client_connect_stdio_no_command() {
    let mut client = McpClient::new();

    let result = client
        .connect(
            "no-cmd",
            McpServerConfig {
                type_: McpTransport::Stdio,
                command: None,
                ..Default::default()
            },
        )
        .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("command"));
}

// ── Connect without url for HTTP returns error ──────────────────

#[tokio::test]
async fn test_mcp_client_connect_http_no_url() {
    let mut client = McpClient::new();

    let result = client
        .connect(
            "no-url",
            McpServerConfig {
                type_: McpTransport::Http,
                url: None,
                ..Default::default()
            },
        )
        .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("url"));
}

// ── Disconnect non-existent is a no-op ──────────────────────────

#[tokio::test]
async fn test_mcp_client_disconnect_nonexistent() {
    let mut client = McpClient::new();
    let result = client.disconnect("nonexistent").await;
    assert!(result.is_ok());
}

// ── Disconnect all on empty client ──────────────────────────────

#[tokio::test]
async fn test_mcp_client_disconnect_all_empty() {
    let mut client = McpClient::new();
    let result = client.disconnect_all().await;
    assert!(result.is_ok());
}

// ── list_tools_for_server returns empty for unknown server ──────

#[test]
fn test_mcp_client_list_tools_for_server_unknown() {
    let client = McpClient::new();
    let tools = client.list_tools_for_server("nonexistent");
    assert!(tools.is_empty());
}

// ── list_tools_for_server returns empty for disabled server ─────

#[tokio::test]
async fn test_mcp_client_list_tools_for_server_disabled() {
    let mut client = McpClient::new();
    client
        .connect(
            "disabled-srv",
            McpServerConfig {
                disabled: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let tools = client.list_tools_for_server("disabled-srv");
    assert!(tools.is_empty());
}

// ── refresh_tools on empty client is a no-op ────────────────────

#[tokio::test]
async fn test_mcp_client_refresh_tools_empty() {
    let mut client = McpClient::new();
    let result = client.refresh_tools().await;
    assert!(result.is_ok());
}

// ── refresh_tools skips disabled servers ─────────────────────────

#[tokio::test]
async fn test_mcp_client_refresh_tools_skips_disabled() {
    let mut client = McpClient::new();
    client
        .connect(
            "disabled",
            McpServerConfig {
                disabled: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let result = client.refresh_tools().await;
    assert!(result.is_ok());
    assert!(client.list_tools().is_empty());
}

// ── refresh_tools_for_server errors on unconnected server ───────

#[tokio::test]
async fn test_mcp_client_refresh_tools_for_server_not_connected() {
    let mut client = McpClient::new();
    let result = client.refresh_tools_for_server("nonexistent").await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("not connected"));
}

// ── McpStatus serde ──────────────────────────────────────────────

#[test]
fn test_mcp_status_serde() {
    let statuses = vec![
        McpStatus::Connected,
        McpStatus::Disabled,
        McpStatus::Failed {
            error: "connection refused".to_string(),
        },
        McpStatus::NeedsAuth,
    ];

    for status in &statuses {
        let json = serde_json::to_string(status).unwrap();
        let deserialized: McpStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(&deserialized, status);
    }
}

// ── McpToolDef serde ─────────────────────────────────────────────

#[test]
fn test_mcp_tool_def_serde() {
    let tool = McpToolDef {
        name: "search".to_string(),
        description: "Search the web".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"}
            }
        }),
    };

    let json = serde_json::to_string(&tool).unwrap();
    let deserialized: McpToolDef = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "search");
    assert_eq!(deserialized.description, "Search the web");
}

// ── McpClient default ────────────────────────────────────────────

#[test]
fn test_mcp_client_default() {
    let client = McpClient::default();
    assert!(client.servers().is_empty());
}

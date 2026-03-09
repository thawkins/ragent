use ragent_core::mcp::*;
use ragent_core::config::{McpServerConfig, McpTransport};

// ── New client ───────────────────────────────────────────────────

#[test]
fn test_mcp_client_new() {
    let client = McpClient::new();
    assert!(client.servers().is_empty());
    assert!(client.list_tools().is_empty());
}

// ── Connect registers server ────────────────────────────────────

#[tokio::test]
async fn test_mcp_client_connect() {
    let mut client = McpClient::new();

    client
        .connect(
            "github",
            McpServerConfig {
                type_: McpTransport::Stdio,
                command: Some("gh-mcp".to_string()),
                args: vec!["--mode".to_string(), "stdio".to_string()],
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

// ── Multiple servers ─────────────────────────────────────────────

#[tokio::test]
async fn test_mcp_client_multiple_servers() {
    let mut client = McpClient::new();

    client
        .connect("server1", McpServerConfig::default())
        .await
        .unwrap();
    client
        .connect("server2", McpServerConfig::default())
        .await
        .unwrap();
    client
        .connect("server3", McpServerConfig::default())
        .await
        .unwrap();

    assert_eq!(client.servers().len(), 3);
    let ids: Vec<&str> = client.servers().iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"server1"));
    assert!(ids.contains(&"server2"));
    assert!(ids.contains(&"server3"));
}

// ── List tools (empty stub) ──────────────────────────────────────

#[tokio::test]
async fn test_mcp_client_list_tools_empty() {
    let mut client = McpClient::new();
    client
        .connect("test", McpServerConfig::default())
        .await
        .unwrap();

    let tools = client.list_tools();
    assert!(tools.is_empty());
}

// ── Call tool (stub) ─────────────────────────────────────────────

#[tokio::test]
async fn test_mcp_client_call_tool_stub() {
    let client = McpClient::new();
    let result = client
        .call_tool("any", "any_tool", serde_json::json!({}))
        .await
        .unwrap();

    assert!(result.is_object());
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

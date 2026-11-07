//! An MCP server entry that omits `type` loads as a stdio server.
//!
//! `type` is optional in every other MCP client's server config, so ragent must
//! treat a missing key as `stdio` rather than failing the whole config load.

use ragent_config::{Config, McpServerConfig, McpTransport};
use std::collections::HashMap;

#[test]
fn test_mcp_server_config_omitting_type_is_stdio() {
    let json = r#"{
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-everything"]
    }"#;
    let config: McpServerConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.type_, McpTransport::Stdio);
    assert_eq!(config.command.as_deref(), Some("npx"));
    assert_eq!(config.args.len(), 2);
}

#[test]
fn test_mcp_server_config_without_type_or_command_still_parses() {
    // A typeless entry is not a parse error even when it names no command: the
    // shape is accepted here and the usability check happens when the server is
    // actually connected (`validate_mcp_config`).
    let config: McpServerConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(config.type_, McpTransport::Stdio);
}

#[test]
fn test_mcp_server_config_explicit_type_still_honoured() {
    let http: McpServerConfig =
        serde_json::from_str(r#"{"type": "http", "url": "https://example.com/mcp"}"#).unwrap();
    assert_eq!(http.type_, McpTransport::Http);

    let sse: McpServerConfig =
        serde_json::from_str(r#"{"type": "sse", "url": "https://example.com/sse"}"#).unwrap();
    assert_eq!(sse.type_, McpTransport::Sse);

    let stdio: McpServerConfig =
        serde_json::from_str(r#"{"type": "stdio", "command": "node"}"#).unwrap();
    assert_eq!(stdio.type_, McpTransport::Stdio);
}

#[test]
fn test_mcp_transport_default_is_stdio() {
    assert_eq!(McpTransport::default(), McpTransport::Stdio);
    assert_eq!(McpServerConfig::default().type_, McpTransport::Stdio);
}

#[test]
fn test_config_from_json_map_defaults_missing_type_to_stdio() {
    // The whole `mcp` section: one entry omits `type`, one declares `http`.
    let json = r#"{
        "mcp": {
            "everything": {
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-everything"]
            },
            "remote": {
                "type": "http",
                "url": "https://example.com/mcp"
            }
        }
    }"#;
    let config: Config = serde_json::from_str(json).unwrap();
    let mcp: &HashMap<String, McpServerConfig> = &config.mcp;

    let everything = mcp.get("everything").expect("typeless entry loaded");
    assert_eq!(everything.type_, McpTransport::Stdio);
    assert_eq!(everything.command.as_deref(), Some("npx"));

    let remote = mcp.get("remote").expect("typed entry loaded");
    assert_eq!(remote.type_, McpTransport::Http);
    assert_eq!(remote.url.as_deref(), Some("https://example.com/mcp"));
}

#[test]
fn test_mcp_server_config_serializes_type() {
    // The default is a load-time convenience: a config that is written back
    // still carries an explicit `type`, so other MCP clients read it unambiguously.
    let config = McpServerConfig {
        command: Some("node".to_string()),
        ..McpServerConfig::default()
    };
    let value: serde_json::Value = serde_json::to_value(&config).unwrap();
    assert_eq!(value.get("type").and_then(|v| v.as_str()), Some("stdio"));
}

#![allow(missing_docs, unused_variables, unused_imports, dead_code, unused_mut)]

//! Tests for MCP server discovery.

use ragent_core::mcp::discovery::*;
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn test_discovered_mcp_server_to_config() {
    let server = DiscoveredMcpServer {
        id: "filesystem".to_string(),
        name: "Filesystem MCP Server".to_string(),
        executable: PathBuf::from("/usr/local/bin/mcp-server-filesystem"),
        args: vec!["--root".to_string(), "/home/user".to_string()],
        env: HashMap::from([("MCP_DEBUG".to_string(), "true".to_string())]),
        source: McpDiscoverySource::SystemPath,
    };

    let config = server.to_config();
    assert_eq!(
        config.command,
        Some("/usr/local/bin/mcp-server-filesystem".to_string())
    );
    assert_eq!(config.args, vec!["--root", "/home/user"]);
    assert_eq!(config.env.get("MCP_DEBUG"), Some(&"true".to_string()));
    // Discovered servers are disabled by default until user enables them
    assert!(config.disabled);
}

#[test]
fn test_discovered_mcp_server_npm_global_source() {
    let server = DiscoveredMcpServer {
        id: "github".to_string(),
        name: "GitHub MCP Server".to_string(),
        executable: PathBuf::from(
            "/usr/local/lib/node_modules/@modelcontextprotocol/server-github/dist/index.js",
        ),
        args: vec![],
        env: HashMap::new(),
        source: McpDiscoverySource::NpmGlobal {
            prefix_dir: PathBuf::from("/usr/local"),
        },
    };

    let config = server.to_config();
    assert!(config.command.unwrap().contains("server-github"));
    assert!(config.disabled);
}

#[test]
fn test_discovered_mcp_server_registry_source() {
    let server = DiscoveredMcpServer {
        id: "custom-server".to_string(),
        name: "My Custom Server".to_string(),
        executable: PathBuf::from("/home/user/bin/custom-mcp"),
        args: vec!["--config".to_string(), "prod.json".to_string()],
        env: HashMap::from([("API_KEY".to_string(), "secret123".to_string())]),
        source: McpDiscoverySource::McpRegistry {
            registry_dir: PathBuf::from("/home/user/.mcp/servers"),
        },
    };

    let config = server.to_config();
    assert_eq!(
        config.command,
        Some("/home/user/bin/custom-mcp".to_string())
    );
    assert_eq!(config.args, vec!["--config", "prod.json"]);
    assert_eq!(config.env.get("API_KEY"), Some(&"secret123".to_string()));
}

#[tokio::test]
async fn test_discover_returns_vec() {
    // Discovery should return a Vec (even if empty on systems without MCP servers)
    let servers = ragent_core::mcp::McpClient::discover().await;
    // Just verify it doesn't panic and returns the right type
    let _: Vec<DiscoveredMcpServer> = servers;
}

#[tokio::test]
async fn test_discover_deduplicates_by_id() {
    // The discover function should deduplicate servers by id
    // This is tested by the implementation - verify the function exists
    let servers = ragent_core::mcp::discover_servers().await;

    // Check for unique IDs
    let mut seen = std::collections::HashSet::new();
    for server in &servers {
        assert!(
            seen.insert(&server.id),
            "Duplicate server ID found: {}",
            server.id
        );
    }
}

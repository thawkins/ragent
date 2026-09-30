//! Inline tests for `discovery.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_discovered_server_to_config() {
    let server = DiscoveredMcpServer {
        id: "test".to_string(),
        name: "Test Server".to_string(),
        executable: PathBuf::from("/usr/bin/mcp-test"),
        args: vec!["--arg1".to_string()],
        env: HashMap::from([("KEY".to_string(), "VALUE".to_string())]),
        source: McpDiscoverySource::SystemPath,
    };

    let config = server.to_config();
    assert_eq!(config.command, Some("/usr/bin/mcp-test".to_string()));
    assert_eq!(config.args, vec!["--arg1"]);
    assert!(config.disabled);
    assert_eq!(config.env.get("KEY"), Some(&"VALUE".to_string()));
}

#[test]
fn test_which_sync_not_found() {
    // Test with a definitely non-existent command
    let result = which_sync("definitely_not_a_real_command_12345");
    assert!(result.is_none());
}

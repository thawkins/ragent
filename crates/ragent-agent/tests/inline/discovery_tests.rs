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
        definition_file: None,
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

#[tokio::test]
async fn test_discover_locations_reports_every_registry_dir() {
    let locations = discover_locations().await;
    // PATH and npm global always come first, then each registry directory.
    assert!(locations.len() >= 2, "PATH + npm rows are always present");
    assert_eq!(locations[0].label, "PATH (known executables)");
    assert_eq!(locations[1].label, "npm global");
    // Every registry location is labelled and carries a path.
    for location in &locations[2..] {
        assert_ne!(location.label, "");
        assert!(location.path.is_some(), "registry rows carry a path");
    }
    // The ragent-native registry directory is always probed.
    assert!(
        locations.iter().any(|l| l.label == "ragent-native"),
        "ragent-native registry location missing"
    );
}

#[tokio::test]
async fn test_discover_dedups_but_locations_do_not() {
    // `discover` collapses to unique ids; `discover_locations` preserves the
    // per-location split so `/mcp config` can count each place separately.
    let flat = discover().await;
    let ids: std::collections::HashSet<&str> = flat.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids.len(), flat.len(), "discover must return unique ids");

    let grouped = discover_locations().await;
    let grouped_total: usize = grouped.iter().map(|l| l.servers.len()).sum();
    assert!(
        grouped_total >= flat.len(),
        "grouped total must be at least the de-duplicated total"
    );
}

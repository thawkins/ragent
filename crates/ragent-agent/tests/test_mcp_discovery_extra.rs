//! Tests-only coverage for MCP auto-discovery (`ragent_agent::mcp::discovery`).
//!
//! `discover_locations` scans `PATH`, npm-global roots, and the well-known MCP
//! registry directories. These tests assert the shape of the returned locations
//! (always at least the `PATH` row, every location labelled, discovered servers
//! disabled until the user opts in) without depending on what is installed on the
//! host.

use ragent_agent::mcp::discovery::{
    DiscoveredMcpServer, McpDiscoverySource, discover, discover_locations,
};

#[tokio::test]
async fn discover_locations_always_reports_a_path_row() {
    let locations = discover_locations().await;
    assert!(!locations.is_empty(), "at least the PATH row is returned");

    let path_row = locations
        .iter()
        .find(|l| l.label.starts_with("PATH"))
        .expect("a PATH location row");
    assert!(path_row.exists, "PATH always exists");
    assert!(
        path_row.path.is_none(),
        "PATH is not a filesystem directory"
    );
}

#[tokio::test]
async fn discover_locations_rows_are_well_formed() {
    let locations = discover_locations().await;
    for location in &locations {
        assert!(!location.label.is_empty(), "every location has a label");
        // A directory-backed location reports a path; a non-existent one is
        // flagged `exists: false` and carries no servers.
        if !location.exists {
            assert!(
                location.servers.is_empty(),
                "a missing location {} holds no servers",
                location.label
            );
        }
    }
}

#[tokio::test]
async fn discover_is_a_deduplicated_subset_of_locations() {
    let locations = discover_locations().await;
    let total: usize = locations.iter().map(|l| l.servers.len()).sum();
    let discovered = discover().await;
    assert!(
        discovered.len() <= total,
        "dedup cannot grow the count ({total} raw)"
    );

    // Ids are unique after de-duplication.
    let mut ids: Vec<&str> = discovered.iter().map(|s| s.id.as_str()).collect();
    let before = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(before, ids.len(), "discover() must de-duplicate by id");
}

#[test]
fn discovered_server_to_config_is_disabled_by_default() {
    let server = DiscoveredMcpServer {
        id: "filesystem".to_string(),
        name: "Filesystem MCP Server".to_string(),
        executable: std::path::PathBuf::from("/usr/bin/mcp-server-filesystem"),
        args: vec!["--root".to_string(), "/tmp".to_string()],
        env: std::collections::HashMap::new(),
        source: McpDiscoverySource::SystemPath,
    };

    let config = server.to_config();
    assert!(
        config.disabled,
        "a discovered server is disabled until the user opts in"
    );
    assert_eq!(
        config.command.as_deref(),
        Some("/usr/bin/mcp-server-filesystem")
    );
    assert_eq!(config.args, vec!["--root".to_string(), "/tmp".to_string()]);
}

#[test]
fn discovery_source_variants_are_distinct() {
    let a = McpDiscoverySource::SystemPath;
    let b = McpDiscoverySource::NpmGlobal {
        prefix_dir: std::path::PathBuf::from("/npm"),
    };
    let c = McpDiscoverySource::McpRegistry {
        registry_dir: std::path::PathBuf::from("/reg"),
    };
    // The enum is `Debug`; the variants differ in payload.
    assert!(!format!("{a:?}").contains("prefix"));
    assert!(format!("{b:?}").contains("/npm"));
    assert!(format!("{c:?}").contains("/reg"));
}

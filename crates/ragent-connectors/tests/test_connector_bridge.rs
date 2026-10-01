//! Tests for the connector MCP bridge (spec `connectors` T-006; FR-003, FR-020,
//! FR-026, FR-033): enabled connectors resolve into `(bridged_id,
//! McpServerConfig)` pairs and a colliding bridged id is refused.
//!
//! Every test is offline and rooted under `target/temp/` (no `/tmp`, per
//! AGENTS.md).

use std::path::{Path, PathBuf};

use ragent_config::{McpServerConfig, McpTransport};
use ragent_connectors::{
    BridgeRefusal, ConnectorAuthShape, ConnectorDescriptor, ConnectorId, ConnectorServer,
    MANIFEST_FILE, StoreDirs, StoreLedger, resolve_servers, scanned_bridge, store_dirs_at,
};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII scratch tree under `target/temp/`.
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/connectors-test/bridge-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp tree creatable");
        Self(path)
    }

    fn store(&self) -> PathBuf {
        self.0.join(".ragent/connectors")
    }

    fn dirs(&self) -> StoreDirs {
        store_dirs_at(&self.0, Some(&self.store()), None)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A stdio server with the given id and command.
fn stdio(id: &str, command: &str) -> ConnectorServer {
    ConnectorServer {
        id: id.to_string(),
        transport: "stdio".to_string(),
        command: Some(command.to_string()),
        args: Vec::new(),
        env: Default::default(),
        url: None,
        headers: Default::default(),
    }
}

/// A descriptor with the given id and servers.
fn descriptor(id: &str, servers: Vec<ConnectorServer>) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("valid id"),
        name: id.to_string(),
        description: String::new(),
        category: String::new(),
        tags: Vec::new(),
        source: String::new(),
        provenance: Default::default(),
        auth: ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers,
        unsupported: Vec::new(),
    }
}

/// Write a descriptor as a connector manifest in `store/<id>/connector.json`.
fn write_connector(store: &Path, descriptor: &ConnectorDescriptor) {
    let dir = store.join(descriptor.id.as_str());
    std::fs::create_dir_all(&dir).expect("connector dir creatable");
    ragent_connectors::write_manifest(&dir, descriptor).expect("manifest writable");
    assert!(dir.join(MANIFEST_FILE).is_file());
}

/// Enable a connector by writing the store ledger.
fn enable(dirs: &StoreDirs, connector_id: &str) {
    let store = dirs.project.as_ref().expect("project store");
    let mut ledger = StoreLedger::load(store);
    ledger.state_mut(connector_id).enabled = true;
    ledger.save(store).expect("ledger save");
}

// ── resolve_servers: prefixes, multi-server, conversion (FR-003, FR-020, FR-026) ──

#[test]
fn two_server_connector_yields_two_prefixed_pairs() {
    let descriptor = descriptor(
        "echo",
        vec![stdio("alpha", "/bin/echo"), stdio("beta", "/bin/true")],
    );
    let plan = resolve_servers([&descriptor], std::iter::empty());

    assert!(plan.refused.is_empty(), "no refusals for a clean plan");
    let ids: Vec<&str> = plan.servers.iter().map(|s| s.server_id.as_str()).collect();
    assert_eq!(ids, vec!["echo.alpha", "echo.beta"]);
    assert!(plan.servers.iter().all(|s| s.connector_id == "echo"));

    // FR-020: registration under the bridged id surfaces tools as
    // `mcp_<bridged-id>_<tool>`; the bridged id (dots included) is the server id.
    assert_eq!(plan.servers[0].server_id, "echo.alpha");
    // The converted config carries the command and the stdio transport.
    assert_eq!(plan.servers[0].config.command.as_deref(), Some("/bin/echo"));
    assert_eq!(plan.servers[0].config.type_, McpTransport::Stdio);
}

#[test]
fn server_map_converts_transport_url_and_headers() {
    let mut server = ConnectorServer {
        id: "main".to_string(),
        transport: "http".to_string(),
        command: None,
        args: Vec::new(),
        env: Default::default(),
        url: Some("https://mcp.example/mcp".to_string()),
        headers: std::collections::BTreeMap::from([("X-Key".to_string(), "v".to_string())]),
    };
    server.env.insert("TOKEN".to_string(), "secret".to_string());
    let descriptor = descriptor("web", vec![server]);

    let plan = resolve_servers([&descriptor], std::iter::empty());
    let config: &McpServerConfig = &plan.servers[0].config;

    assert_eq!(config.type_, McpTransport::Http);
    assert_eq!(config.url.as_deref(), Some("https://mcp.example/mcp"));
    assert_eq!(config.headers.get("X-Key").map(String::as_str), Some("v"));
    assert_eq!(config.env.get("TOKEN").map(String::as_str), Some("secret"));
    assert!(!config.disabled, "a bridged server is enabled by default");
}

#[test]
fn unexpressible_server_is_skipped_but_supported_ones_bridge() {
    // A grpc server ragent does not speak, plus one expressible stdio server.
    let mut unsupported = stdio("weird", "/bin/x");
    unsupported.transport = "grpc".to_string();
    let descriptor = descriptor("mixed", vec![unsupported, stdio("ok", "/bin/true")]);

    let plan = resolve_servers([&descriptor], std::iter::empty());
    assert_eq!(plan.servers.len(), 1);
    assert_eq!(plan.servers[0].server_id, "mixed.ok");
    assert!(
        plan.refused.is_empty(),
        "an unsupported server is skipped, not refused"
    );
}

#[test]
fn servers_of_groups_a_multi_server_connector() {
    let descriptor = descriptor(
        "two",
        vec![stdio("alpha", "/bin/a"), stdio("beta", "/bin/b")],
    );
    let plan = resolve_servers([&descriptor], std::iter::empty());

    let grouped: Vec<&str> = plan
        .servers_of("two")
        .map(|s| s.server_id.as_str())
        .collect();
    assert_eq!(grouped, vec!["two.alpha", "two.beta"]);
    assert_eq!(plan.servers_of("absent").count(), 0);
}

// ── resolve_servers: refusals (FR-033) ───────────────────────────────────────

#[test]
fn bridged_id_colliding_with_a_configured_mcp_key_is_refused() {
    let descriptor = descriptor("echo", vec![stdio("echo", "/bin/true")]);
    let plan = resolve_servers([&descriptor], ["echo.echo"]);

    assert!(
        plan.servers.is_empty(),
        "the colliding server is not bridged"
    );
    assert_eq!(plan.refused.len(), 1);
    assert_eq!(plan.refused[0].server_id, "echo.echo");
    assert_eq!(plan.refused[0].reason, BridgeRefusal::Configured);
    assert!(plan.refused[0].describe().contains("echo.echo"));
    assert!(plan.refused[0].describe().contains("ragent.json"));
}

#[test]
fn bridged_id_colliding_across_connectors_is_refused_and_owned() {
    // Both halves may contain dots, so two differently-named connectors can still
    // bridge to the same id: `a.b` + server `c` == `a` + server `b.c` == `a.b.c`.
    let first = descriptor("a.b", vec![stdio("c", "/bin/first")]);
    let second = descriptor("a", vec![stdio("b.c", "/bin/second")]);

    let plan = resolve_servers([&first, &second], std::iter::empty());
    assert_eq!(plan.servers.len(), 1);
    assert_eq!(plan.servers[0].server_id, "a.b.c");
    assert_eq!(plan.servers[0].connector_id, "a.b");
    assert_eq!(plan.refused.len(), 1);
    assert_eq!(plan.refused[0].connector_id, "a");
    assert_eq!(plan.refused[0].server_id, "a.b.c");
    assert_eq!(
        plan.refused[0].reason,
        BridgeRefusal::OwnedBy("a.b".to_string())
    );
    assert!(plan.refused[0].describe().contains("'a.b'"));
}

#[test]
fn duplicate_server_id_within_one_connector_is_refused() {
    let descriptor = descriptor(
        "dup",
        vec![stdio("main", "/bin/a"), stdio("main", "/bin/b")],
    );
    let plan = resolve_servers([&descriptor], std::iter::empty());

    assert_eq!(plan.servers.len(), 1);
    assert_eq!(plan.refused.len(), 1);
    assert_eq!(plan.refused[0].server_id, "dup.main");
    assert_eq!(plan.refused[0].reason, BridgeRefusal::Duplicate);
}

#[test]
fn later_connector_does_not_overwrite_earlier_accepted_server() {
    // The same server declared twice (as a second copy of the same connector)
    // bridges once: the earlier definition wins and the later is refused, so a
    // re-registration never silently overwrites the existing server (FR-033).
    let first = descriptor("a", vec![stdio("m", "/bin/a")]);
    let second = descriptor("a", vec![stdio("m", "/bin/b")]);
    let plan = resolve_servers([&first, &second], std::iter::empty());

    assert_eq!(plan.servers.len(), 1);
    assert_eq!(plan.servers[0].config.command.as_deref(), Some("/bin/a"));
    assert_eq!(plan.refused[0].reason, BridgeRefusal::Duplicate);
}

// ── scanned_bridge: enabled-only, store-driven (FR-018) ──────────────────────

#[test]
fn disabled_connector_contributes_nothing() {
    let tree = TempTree::new("disabled");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("echo", "/bin/true")]),
    );

    // Not enabled: inert (FR-018).
    let plan = scanned_bridge(&tree.dirs(), std::iter::empty());
    assert!(plan.is_empty());
    assert!(plan.refused.is_empty());

    enable(&tree.dirs(), "echo");
    let plan = scanned_bridge(&tree.dirs(), std::iter::empty());
    assert_eq!(plan.servers.len(), 1);
    assert_eq!(plan.servers[0].server_id, "echo.echo");
}

#[test]
fn scanned_bridge_prefixes_ids_and_resolves_configured_collisions() {
    let tree = TempTree::new("scanned");
    write_connector(
        &tree.store(),
        &descriptor(
            "two",
            vec![stdio("alpha", "/bin/a"), stdio("beta", "/bin/b")],
        ),
    );
    enable(&tree.dirs(), "two");

    let plan = scanned_bridge(&tree.dirs(), ["two.beta"]);
    assert_eq!(plan.servers.len(), 1);
    assert_eq!(plan.servers[0].server_id, "two.alpha");
    assert_eq!(plan.refused.len(), 1);
    assert_eq!(plan.refused[0].server_id, "two.beta");
    assert_eq!(plan.refused[0].reason, BridgeRefusal::Configured);
}

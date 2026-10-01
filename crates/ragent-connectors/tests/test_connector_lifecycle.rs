//! Tests for the connector session lifecycle (spec `connectors` T-008; FR-008,
//! FR-012, FR-013, FR-018, FR-019, FR-032, FR-033): session-start load, connect,
//! disconnect, and the enable/disable transitions.
//!
//! Every test is offline and rooted under `target/temp/` (no `/tmp`, per
//! AGENTS.md). A fake [`McpConnect`] records the connect/disconnect calls and
//! can be told to fail a named server, so the per-server, containment, and
//! collision behaviour is asserted without touching a real MCP process.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use ragent_config::{ConnectorsConfig, McpServerConfig};
use ragent_connectors::{
    AlwaysEnabled, ConnectReport, ConnectorAuthShape, ConnectorDescriptor, ConnectorEnv,
    ConnectorId, ConnectorLifecycleState, ConnectorServer, ConnectorSession,
    InMemoryCredentialStore, LifecycleError, MANIFEST_FILE, MapEnv, MapServerLedger, McpConnect,
    ServerState, StoreDirs, StoreLedger, store_dirs_at, store_secret,
};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII scratch tree under `target/temp/`.
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/connectors-test/lifecycle-{name}-{}-{unique}",
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

/// A descriptor with the given id, servers, and auth shape.
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

/// Set a connector's enable flag in its store ledger.
fn set_enabled(store: &Path, connector_id: &str, enabled: bool) {
    let mut ledger = StoreLedger::load(store);
    ledger.state_mut(connector_id).enabled = enabled;
    ledger.save(store).expect("ledger save");
}

/// A fake MCP client: records calls, returns per-server tools, fails named ids.
#[derive(Default)]
struct FakeConnect {
    fail: BTreeSet<String>,
    tools_for: BTreeMap<String, Vec<String>>,
    connects: Mutex<Vec<String>>,
    disconnects: Mutex<Vec<String>>,
}

impl FakeConnect {
    /// Tools registry names for `server_id`, defaulting to one tool per server so
    /// a connect is observable even when the test does not declare tools.
    fn tools(&self, server_id: &str) -> Vec<String> {
        self.tools_for
            .get(server_id)
            .cloned()
            .unwrap_or_else(|| vec![format!("mcp_{}_default", server_id.replace('.', "_"))])
    }

    fn connect_calls(&self) -> Vec<String> {
        self.connects.lock().expect("not poisoned").clone()
    }

    fn disconnect_calls(&self) -> Vec<String> {
        self.disconnects.lock().expect("not poisoned").clone()
    }
}

#[async_trait]
impl McpConnect for FakeConnect {
    async fn connect(
        &mut self,
        server_id: &str,
        _config: McpServerConfig,
    ) -> Result<Vec<String>, String> {
        self.connects
            .lock()
            .expect("not poisoned")
            .push(server_id.to_string());
        if self.fail.contains(server_id) {
            return Err(format!("simulated connect failure for {server_id}"));
        }
        Ok(self.tools(server_id))
    }

    async fn disconnect(&mut self, server_id: &str) -> Result<(), String> {
        self.disconnects
            .lock()
            .expect("not poisoned")
            .push(server_id.to_string());
        Ok(())
    }
}

/// Build a [`ConnectorEnv`] over the given seams.
fn env<'a>(
    connect: &'a mut FakeConnect,
    credentials: &'a InMemoryCredentialStore,
    env_source: &'a MapEnv,
    server_ledger: &'a dyn ragent_connectors::ServerEnableLedger,
) -> ConnectorEnv<'a> {
    ConnectorEnv {
        connect,
        credentials,
        env: env_source,
        server_ledger,
    }
}

/// A one-shot session over `dirs` with the given configured `mcp` ids.
fn session(dirs: &StoreDirs, configured: Vec<String>) -> ConnectorSession {
    ConnectorSession::new(dirs.clone(), ConnectorsConfig::default(), configured)
}

// ── FR-018: a disabled connector is fully inert ────────────────────────────

#[tokio::test]
async fn disabled_connector_starts_no_server_but_is_still_reported() {
    let tree = TempTree::new("disabled-inert");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("srv", "/bin/echo")]),
    );
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    let report = session
        .start(&mut env(
            &mut connect,
            &credentials,
            &env_source,
            &AlwaysEnabled,
        ))
        .await;

    assert!(connect.connect_calls().is_empty(), "no server started");
    assert_eq!(report.disabled, vec!["echo".to_string()]);
    assert_eq!(report.reports.len(), 1);
    assert_eq!(report.reports[0].state, ConnectorLifecycleState::Disabled);
    // Still reported (FR-018): the status snapshot lists it as disabled.
    let statuses = session.statuses();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].id, "echo");
    assert_eq!(statuses[0].state, ConnectorLifecycleState::Disabled);
}

// ── FR-008, FR-012, FR-020: enabled connector connects and surfaces tools ──

#[tokio::test]
async fn enabled_connector_connects_and_surfaces_tools() {
    let tree = TempTree::new("enabled-connects");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("srv", "/bin/echo")]),
    );
    set_enabled(&tree.store(), "echo", true);
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    connect.tools_for.insert(
        "echo.srv".to_string(),
        vec!["mcp_echo_srv_echo".to_string()],
    );
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    let report = session
        .start(&mut env(
            &mut connect,
            &credentials,
            &env_source,
            &AlwaysEnabled,
        ))
        .await;

    assert_eq!(connect.connect_calls(), vec!["echo.srv".to_string()]);
    assert_eq!(report.connected(), 1);
    let r = &report.reports[0];
    assert_eq!(r.state, ConnectorLifecycleState::Connected);
    assert_eq!(r.connected_servers(), 1);
    assert_eq!(r.tools(), 1);
    assert_eq!(r.servers[0].server_id, "echo.srv");
    assert_eq!(r.servers[0].state, ServerState::Connected);
    assert_eq!(r.servers[0].tools, vec!["mcp_echo_srv_echo".to_string()]);
}

#[tokio::test]
async fn enable_marks_connector_enabled_connects_and_reports_tool_count() {
    let tree = TempTree::new("enable-live");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("srv", "/bin/echo")]),
    );
    // Installed disabled; the live /connectors enable must connect it now.
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    let report = session
        .enable(
            "echo",
            &mut env(&mut connect, &credentials, &env_source, &AlwaysEnabled),
        )
        .await
        .expect("enable ok");

    assert_eq!(connect.connect_calls(), vec!["echo.srv".to_string()]);
    assert_eq!(report.state, ConnectorLifecycleState::Connected);
    assert_eq!(report.tools(), 1);
    // The durable store ledger records the enable (FR-012).
    assert!(
        StoreLedger::load(&tree.store())
            .state("echo")
            .unwrap()
            .enabled
    );
}

#[tokio::test]
async fn enable_is_idempotent_when_already_connected() {
    let tree = TempTree::new("enable-idempotent");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("srv", "/bin/echo")]),
    );
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    let first = session
        .enable(
            "echo",
            &mut env(&mut connect, &credentials, &env_source, &AlwaysEnabled),
        )
        .await
        .expect("first enable");
    let second = session
        .enable(
            "echo",
            &mut env(&mut connect, &credentials, &env_source, &AlwaysEnabled),
        )
        .await
        .expect("second enable");

    assert_eq!(first.tools(), second.tools());
    assert_eq!(
        connect.connect_calls(),
        vec!["echo.srv".to_string()],
        "no second connection for an already-connected connector"
    );
}

// ── FR-013: disable disconnects and deregisters exactly its tools ──────────

#[tokio::test]
async fn disable_disconnects_and_deregisters_exactly_its_servers() {
    let tree = TempTree::new("disable-live");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("srv", "/bin/echo")]),
    );
    set_enabled(&tree.store(), "echo", true);
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    connect.tools_for.insert(
        "echo.srv".to_string(),
        vec!["mcp_echo_srv_echo".to_string()],
    );
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    let started = session
        .start(&mut env(
            &mut connect,
            &credentials,
            &env_source,
            &AlwaysEnabled,
        ))
        .await;
    assert_eq!(started.connected(), 1);

    let report = session
        .disable(
            "echo",
            &mut env(&mut connect, &credentials, &env_source, &AlwaysEnabled),
        )
        .await
        .expect("disable ok");

    assert_eq!(report.servers_disconnected, 1);
    assert_eq!(report.tools_deregistered, 1);
    assert_eq!(connect.disconnect_calls(), vec!["echo.srv".to_string()]);
    assert!(
        !StoreLedger::load(&tree.store())
            .state("echo")
            .unwrap()
            .enabled
    );
    let statuses = session.statuses();
    assert_eq!(statuses[0].state, ConnectorLifecycleState::Disabled);
    assert!(statuses[0].servers.is_empty());
}

// ── FR-026, FR-016: per-server independence and containment ────────────────

#[tokio::test]
async fn one_failing_server_does_not_hide_the_other() {
    let tree = TempTree::new("multi-server");
    write_connector(
        &tree.store(),
        &descriptor(
            "two-server",
            vec![stdio("alpha", "/bin/echo"), stdio("beta", "/bin/false")],
        ),
    );
    set_enabled(&tree.store(), "two-server", true);
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    connect.fail.insert("two-server.beta".to_string());
    connect.tools_for.insert(
        "two-server.alpha".to_string(),
        vec!["mcp_two_server_alpha_echo".to_string()],
    );
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    let report = session
        .start(&mut env(
            &mut connect,
            &credentials,
            &env_source,
            &AlwaysEnabled,
        ))
        .await;

    let r = &report.reports[0];
    assert_eq!(r.state, ConnectorLifecycleState::Connected);
    assert_eq!(r.connected_servers(), 1);
    let alpha = r
        .servers
        .iter()
        .find(|s| s.server_id == "two-server.alpha")
        .unwrap();
    let beta = r
        .servers
        .iter()
        .find(|s| s.server_id == "two-server.beta")
        .unwrap();
    assert_eq!(alpha.state, ServerState::Connected);
    assert_eq!(beta.state, ServerState::Errored);
    assert!(
        beta.error
            .as_deref()
            .unwrap()
            .contains("simulated connect failure")
    );
}

#[tokio::test]
async fn every_server_failing_records_connector_errored_with_cause() {
    let tree = TempTree::new("all-fail");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("srv", "/bin/false")]),
    );
    set_enabled(&tree.store(), "echo", true);
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    connect.fail.insert("echo.srv".to_string());
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    let report = session
        .start(&mut env(
            &mut connect,
            &credentials,
            &env_source,
            &AlwaysEnabled,
        ))
        .await;

    let r = &report.reports[0];
    assert_eq!(r.state, ConnectorLifecycleState::Errored);
    assert!(
        r.error
            .as_deref()
            .unwrap()
            .contains("simulated connect failure")
    );
}

// ── FR-018 (server ledger): a server switched off is not started ───────────

#[tokio::test]
async fn server_disabled_in_the_durable_ledger_is_not_started() {
    let tree = TempTree::new("server-ledger");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("srv", "/bin/echo")]),
    );
    set_enabled(&tree.store(), "echo", true);
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    let ledger = MapServerLedger::new().with_disabled("echo.srv");
    let report = session
        .start(&mut env(&mut connect, &credentials, &env_source, &ledger))
        .await;

    assert!(
        connect.connect_calls().is_empty(),
        "server switched off is not started"
    );
    let r = &report.reports[0];
    assert_eq!(r.state, ConnectorLifecycleState::Enabled);
    assert_eq!(r.servers[0].state, ServerState::Disconnected);
}

// ── FR-019: connect/disconnect without a restart ───────────────────────────

#[tokio::test]
async fn connect_refuses_when_the_connector_is_disabled() {
    let tree = TempTree::new("connect-disabled");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("srv", "/bin/echo")]),
    );
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    let err = session
        .connect(
            "echo",
            &mut env(&mut connect, &credentials, &env_source, &AlwaysEnabled),
        )
        .await
        .expect_err("disabled connector refuses connect");
    assert!(matches!(err, LifecycleError::Disabled(id) if id == "echo"));
    assert!(connect.connect_calls().is_empty());
}

#[tokio::test]
async fn connect_after_disable_restarts_connection_without_restart() {
    let tree = TempTree::new("reconnect");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("srv", "/bin/echo")]),
    );
    set_enabled(&tree.store(), "echo", true);
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    session
        .start(&mut env(
            &mut connect,
            &credentials,
            &env_source,
            &AlwaysEnabled,
        ))
        .await;
    session
        .disconnect(
            "echo",
            &mut env(&mut connect, &credentials, &env_source, &AlwaysEnabled),
        )
        .await
        .expect("disconnect ok");
    let report = session
        .connect(
            "echo",
            &mut env(&mut connect, &credentials, &env_source, &AlwaysEnabled),
        )
        .await
        .expect("reconnect ok");
    assert_eq!(report.state, ConnectorLifecycleState::Connected);
    assert_eq!(connect.connect_calls().len(), 2);
}

#[tokio::test]
async fn disconnect_leaves_the_connector_enabled() {
    let tree = TempTree::new("disconnect-keeps-enabled");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("srv", "/bin/echo")]),
    );
    set_enabled(&tree.store(), "echo", true);
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    session
        .start(&mut env(
            &mut connect,
            &credentials,
            &env_source,
            &AlwaysEnabled,
        ))
        .await;
    let report = session
        .disconnect(
            "echo",
            &mut env(&mut connect, &credentials, &env_source, &AlwaysEnabled),
        )
        .await
        .expect("disconnect ok");

    assert_eq!(report.servers_disconnected, 1);
    assert_eq!(connect.disconnect_calls(), vec!["echo.srv".to_string()]);
    // Still enabled in the ledger and in the tracked state (FR-019).
    assert!(
        StoreLedger::load(&tree.store())
            .state("echo")
            .unwrap()
            .enabled
    );
    assert_eq!(
        session.statuses()[0].state,
        ConnectorLifecycleState::Enabled
    );
}

// ── FR-022, FR-032: auth gate ──────────────────────────────────────────────

#[tokio::test]
async fn connector_needing_auth_is_errored_and_not_connected() {
    let tree = TempTree::new("needs-auth");
    let mut desc = descriptor("needs-token", vec![stdio("srv", "/bin/echo")]);
    desc.auth = ConnectorAuthShape::Token;
    desc.credential = Some("NEEDS_TOKEN_VALUE".to_string());
    write_connector(&tree.store(), &desc);
    set_enabled(&tree.store(), "needs-token", true);
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    let report = session
        .start(&mut env(
            &mut connect,
            &credentials,
            &env_source,
            &AlwaysEnabled,
        ))
        .await;

    assert!(
        connect.connect_calls().is_empty(),
        "auth gate refuses connect"
    );
    let r = &report.reports[0];
    assert_eq!(r.state, ConnectorLifecycleState::Errored);
    assert_eq!(r.auth, ragent_connectors::AuthState::NeedsAuth);
    assert!(r.error.as_deref().unwrap().starts_with("auth failed"));
}

#[tokio::test]
async fn connector_with_a_stored_secret_connects() {
    let tree = TempTree::new("auth-satisfied");
    let mut desc = descriptor("needs-token", vec![stdio("srv", "/bin/echo")]);
    desc.auth = ConnectorAuthShape::Token;
    desc.credential = Some("NEEDS_TOKEN_VALUE".to_string());
    write_connector(&tree.store(), &desc);
    set_enabled(&tree.store(), "needs-token", true);
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let credentials = InMemoryCredentialStore::new();
    store_secret(&desc, "secret-token", &credentials).expect("secret stored");
    let mut connect = FakeConnect::default();
    let env_source = MapEnv::new();
    let report = session
        .start(&mut env(
            &mut connect,
            &credentials,
            &env_source,
            &AlwaysEnabled,
        ))
        .await;

    assert_eq!(connect.connect_calls(), vec!["needs-token.srv".to_string()]);
    assert_eq!(report.reports[0].state, ConnectorLifecycleState::Connected);
}

// ── FR-033: server-id collision refusal ────────────────────────────────────

#[tokio::test]
async fn bridged_id_colliding_with_a_configured_server_is_refused() {
    let tree = TempTree::new("collision-configured");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("srv", "/bin/echo")]),
    );
    set_enabled(&tree.store(), "echo", true);
    let dirs = tree.dirs();
    // The configured `ragent.json` mcp set already holds `echo.srv`.
    let mut session = session(&dirs, vec!["echo.srv".to_string()]);

    let mut connect = FakeConnect::default();
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    let report = session
        .start(&mut env(
            &mut connect,
            &credentials,
            &env_source,
            &AlwaysEnabled,
        ))
        .await;

    assert!(connect.connect_calls().is_empty(), "colliding id refused");
    let r = &report.reports[0];
    assert_eq!(r.refused.len(), 1);
    assert_eq!(r.refused[0].server_id, "echo.srv");
    assert_eq!(r.state, ConnectorLifecycleState::Errored);
}

#[tokio::test]
async fn two_connectors_colliding_on_a_bridged_id_refuse_the_later() {
    let tree = TempTree::new("collision-cross");
    // `a` + server `b.c` and `a.b` + server `c` both bridge to `a.b.c`.
    write_connector(
        &tree.store(),
        &descriptor("a", vec![stdio("b.c", "/bin/echo")]),
    );
    write_connector(
        &tree.store(),
        &descriptor("a.b", vec![stdio("c", "/bin/echo")]),
    );
    set_enabled(&tree.store(), "a", true);
    set_enabled(&tree.store(), "a.b", true);
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    let report = session
        .start(&mut env(
            &mut connect,
            &credentials,
            &env_source,
            &AlwaysEnabled,
        ))
        .await;

    // `a` wins (loaded first in id order); `a.b` collides.
    assert_eq!(connect.connect_calls(), vec!["a.b.c".to_string()]);
    let first = &report.reports[0];
    let second = &report.reports[1];
    assert_eq!(first.id, "a");
    assert_eq!(first.state, ConnectorLifecycleState::Connected);
    assert_eq!(second.id, "a.b");
    assert_eq!(second.refused.len(), 1);
    assert_eq!(second.state, ConnectorLifecycleState::Errored);
}

// ── FR-021: disabled subsystem is inert ────────────────────────────────────

#[tokio::test]
async fn disabled_subsystem_discovers_and_connects_nothing() {
    let tree = TempTree::new("subsystem-off");
    write_connector(
        &tree.store(),
        &descriptor("echo", vec![stdio("srv", "/bin/echo")]),
    );
    set_enabled(&tree.store(), "echo", true);
    let dirs = tree.dirs();
    let config = ConnectorsConfig {
        enabled: false,
        ..Default::default()
    };
    let mut session = ConnectorSession::new(dirs, config, Vec::new());

    let mut connect = FakeConnect::default();
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    let report = session
        .start(&mut env(
            &mut connect,
            &credentials,
            &env_source,
            &AlwaysEnabled,
        ))
        .await;

    assert!(report.reports.is_empty());
    assert!(connect.connect_calls().is_empty());
    assert!(session.statuses().is_empty());
}

// ── shutdown: session end disconnects everything ───────────────────────────

#[tokio::test]
async fn shutdown_disconnects_every_connected_server() {
    let tree = TempTree::new("shutdown");
    write_connector(
        &tree.store(),
        &descriptor(
            "two-server",
            vec![stdio("alpha", "/bin/echo"), stdio("beta", "/bin/true")],
        ),
    );
    set_enabled(&tree.store(), "two-server", true);
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    session
        .start(&mut env(
            &mut connect,
            &credentials,
            &env_source,
            &AlwaysEnabled,
        ))
        .await;
    assert_eq!(
        session.statuses()[0].state,
        ConnectorLifecycleState::Connected
    );

    session.shutdown(&mut connect).await;
    let mut calls = connect.disconnect_calls();
    calls.sort();
    assert_eq!(
        calls,
        vec![
            "two-server.alpha".to_string(),
            "two-server.beta".to_string()
        ]
    );
    assert_eq!(
        session.statuses()[0].state,
        ConnectorLifecycleState::Enabled
    );
}

// ── unknown connector id is a contained refusal ────────────────────────────

#[tokio::test]
async fn unknown_connector_is_refused_by_every_transition() {
    let tree = TempTree::new("unknown");
    let dirs = tree.dirs();
    let mut session = session(&dirs, Vec::new());

    let mut connect = FakeConnect::default();
    let credentials = InMemoryCredentialStore::new();
    let env_source = MapEnv::new();
    assert!(matches!(
        session
            .enable("nope", &mut env(&mut connect, &credentials, &env_source, &AlwaysEnabled))
            .await,
        Err(LifecycleError::UnknownConnector(id)) if id == "nope"
    ));
    assert!(matches!(
        session
            .disable(
                "nope",
                &mut env(&mut connect, &credentials, &env_source, &AlwaysEnabled)
            )
            .await,
        Err(LifecycleError::UnknownConnector(_))
    ));
    assert!(matches!(
        session
            .connect(
                "nope",
                &mut env(&mut connect, &credentials, &env_source, &AlwaysEnabled)
            )
            .await,
        Err(LifecycleError::UnknownConnector(_))
    ));
}

// ── report helpers ─────────────────────────────────────────────────────────

#[test]
fn connect_report_helpers_count_servers_and_tools() {
    let report = ConnectReport {
        id: "x".to_string(),
        state: ConnectorLifecycleState::Connected,
        auth: ragent_connectors::AuthState::NotRequired,
        servers: vec![
            ragent_connectors::ServerReport {
                server_id: "x.a".to_string(),
                state: ServerState::Connected,
                tools: vec!["mcp_x_a_one".to_string()],
                error: None,
            },
            ragent_connectors::ServerReport {
                server_id: "x.b".to_string(),
                state: ServerState::Errored,
                tools: Vec::new(),
                error: Some("boom".to_string()),
            },
        ],
        refused: Vec::new(),
        error: None,
    };
    assert_eq!(report.connected_servers(), 1);
    assert_eq!(report.tools(), 1);
}

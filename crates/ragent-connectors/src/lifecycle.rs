//! Connector session lifecycle: session-start load, connect, and disconnect
//! (spec `connectors` T-008; FR-008, FR-012, FR-013, FR-018, FR-019, FR-032,
//! FR-033).
//!
//! [`ConnectorSession`] is the session-facing owner of every connector's runtime
//! state. It composes the earlier layers:
//!
//! - discovery and the store enable ledger come from [`crate::store`] (FR-001,
//!   FR-018);
//! - collision-free server resolution comes from [`crate::bridge`] (FR-003,
//!   FR-026, FR-033);
//! - the authentication gate comes from [`crate::auth`] (FR-022, FR-032).
//!
//! The session drives the *existing* MCP client through the [`McpConnect`] seam
//! (this crate must not depend on `ragent-agent`), and consults the durable,
//! global per-server enable ledger through [`ServerEnableLedger`] so a server the
//! user switched off in `/mcp` stays off here too.
//!
//! # Lifecycle (FR-008, FR-012, FR-013, FR-018, FR-019)
//!
//! - [`ConnectorSession::start`] loads each **enabled** connector once at session
//!   start: it resolves the connector's servers, applies the enable ledgers,
//!   connects each bridged server, and records the connector state as `connected`
//!   or `errored` with the cause. A disabled connector is fully inert - it starts
//!   no server, registers no tool, and is still reported (FR-018).
//! - [`ConnectorSession::enable`] / [`ConnectorSession::disable`] are the live
//!   transitions: enable marks the connector enabled, connects its servers now,
//!   and reports the state and per-server tool counts (FR-012); disable marks it
//!   disabled, disconnects each bridged server, and reports exactly how many
//!   servers and tools were dropped (FR-013).
//! - [`ConnectorSession::connect`] / [`ConnectorSession::disconnect`] toggle the
//!   connection **without a session restart**: connect is refused while the
//!   connector is disabled; disconnect leaves the connector enabled (FR-019).
//!
//! # Containment (FR-016, FR-026, FR-032, FR-033)
//!
//! Per-server state is independent, so one failing server on a multi-server
//! connector does not hide the others (FR-026). A connect failure is recorded as
//! `errored` with its cause and never panics (FR-016). An auth-shaped connector
//! with no valid credential is recorded `errored` with an `auth failed` cause and
//! is not retried in a loop (FR-022, FR-032). A bridged id that collides with a
//! configured `ragent.json` server or another connector's server is refused and
//! reported rather than silently overwriting (FR-033).
//!
//! The module is modelled on `ragent_plugins::session` / `ragent_plugins::lifecycle`
//! so the two session integrations read the same way.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use ragent_config::{ConnectorsConfig, McpServerConfig};

use crate::auth::{AuthRequirement, AuthState, CredentialStore, EnvSource, resolve};
use crate::bridge::{RefusedServer, resolve_servers};
use crate::descriptor::ConnectorDescriptor;
use crate::error::ConnectorError;
use crate::store::{StoreDirs, StoreLedger, scan_dirs};

/// The seam through which the session connects and disconnects MCP servers
/// (FR-008, FR-020).
///
/// The connector crate must not depend on `ragent-agent` (the MCP client), so the
/// session implements this over the live `McpClient`: `connect` connects the
/// server under `server_id` and returns the **registry names** of the tools now
/// surfaced (`mcp_<server>_<tool>`, FR-020); `disconnect` tears the connection
/// down and deregisters those tools. Both are contained - a failure is an error
/// string, never a panic.
#[async_trait]
pub trait McpConnect: Send {
    /// Connect `server_id` with `config`, register its tools, and return the
    /// registry names of the surfaced tools (FR-020).
    ///
    /// # Errors
    ///
    /// Returns the connection failure cause when the server cannot be reached.
    async fn connect(
        &mut self,
        server_id: &str,
        config: McpServerConfig,
    ) -> Result<Vec<String>, String>;

    /// Disconnect `server_id`; its contributed tools are deregistered.
    ///
    /// # Errors
    ///
    /// Returns the teardown failure cause when the connection cannot be dropped.
    async fn disconnect(&mut self, server_id: &str) -> Result<(), String>;
}

/// The durable per-server enable ledger seam (FR-018, assumption A3).
///
/// The session supplies the global `mcp_state.json` ledger so a bridged server
/// switched off in `/mcp` is not started by a connector. Implemented over
/// `ragent_agent::mcp::McpEnableLedger` by the session; [`AlwaysEnabled`] is the
/// default no-ledger implementation and [`MapServerLedger`] is the test double.
pub trait ServerEnableLedger: Send + Sync {
    /// Whether `server_id` should be started (absent means enabled).
    fn is_enabled(&self, server_id: &str) -> bool;
}

/// A [`ServerEnableLedger`] under which every server is enabled.
#[derive(Debug, Clone, Copy, Default)]
pub struct AlwaysEnabled;

impl ServerEnableLedger for AlwaysEnabled {
    fn is_enabled(&self, _server_id: &str) -> bool {
        true
    }
}

/// An offline [`ServerEnableLedger`] backed by a set of disabled ids
/// (spec `connectors` T-008 test seam).
#[derive(Debug, Clone, Default)]
pub struct MapServerLedger {
    disabled: BTreeSet<String>,
}

impl MapServerLedger {
    /// An empty ledger (every server enabled).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `server_id` as disabled.
    #[must_use]
    pub fn with_disabled(mut self, server_id: &str) -> Self {
        self.disabled.insert(server_id.to_string());
        self
    }
}

impl ServerEnableLedger for MapServerLedger {
    fn is_enabled(&self, server_id: &str) -> bool {
        !self.disabled.contains(server_id)
    }
}

/// The seams a lifecycle operation needs (FR-008): the MCP client, the encrypted
/// credential store, the environment, and the durable per-server enable ledger.
pub struct ConnectorEnv<'a> {
    /// The MCP client seam (connect/disconnect).
    pub connect: &'a mut dyn McpConnect,
    /// The encrypted credential store (FR-005, FR-022).
    pub credentials: &'a dyn CredentialStore,
    /// The environment source for the `env` auth shape (FR-022).
    pub env: &'a dyn EnvSource,
    /// The durable per-server enable ledger (FR-018).
    pub server_ledger: &'a dyn ServerEnableLedger,
}

/// The lifecycle state of a connector, the value `/connectors list` prints
/// (FR-009).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectorLifecycleState {
    /// The connector is disabled; it starts no server and registers no tool
    /// (FR-018).
    Disabled,
    /// The connector is enabled but no server is currently connected (FR-019).
    Enabled,
    /// At least one bridged server is connected (FR-008).
    Connected,
    /// The connector failed to load or connect; see the recorded cause (FR-016,
    /// FR-032).
    Errored,
}

impl ConnectorLifecycleState {
    /// The lowercase label the reports print.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Enabled => "enabled",
            Self::Connected => "connected",
            Self::Errored => "errored",
        }
    }
}

impl std::fmt::Display for ConnectorLifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// The connection state of one bridged server (FR-026).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServerState {
    /// The server is not connected (disabled in a ledger, or disconnected).
    Disconnected,
    /// The server is connected and its tools are registered.
    Connected,
    /// The server failed to connect; see the recorded cause.
    Errored,
}

impl ServerState {
    /// The lowercase label the reports print.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connected => "connected",
            Self::Errored => "errored",
        }
    }
}

impl std::fmt::Display for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A report line for one bridged server (FR-026).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerReport {
    /// The bridged registry id (`<connector-id>.<server>`).
    pub server_id: String,
    /// The server's connection state.
    pub state: ServerState,
    /// The registry names of the tools the server surfaced (FR-020).
    pub tools: Vec<String>,
    /// The failure cause when `state == Errored`.
    pub error: Option<String>,
}

/// The result of a load/connect/enable operation for one connector (FR-008,
/// FR-012).
#[derive(Debug, Clone)]
pub struct ConnectReport {
    /// The connector id.
    pub id: String,
    /// The resulting connector state.
    pub state: ConnectorLifecycleState,
    /// The connector's authentication state (FR-014, FR-022).
    pub auth: AuthState,
    /// One report per bridged server, in declaration order (FR-026).
    pub servers: Vec<ServerReport>,
    /// Bridged ids refused by the collision guard (FR-033).
    pub refused: Vec<RefusedServer>,
    /// The connector-level failure cause when `state == Errored`.
    pub error: Option<String>,
}

impl ConnectReport {
    /// The number of bridged servers that connected.
    #[must_use]
    pub fn connected_servers(&self) -> usize {
        self.servers
            .iter()
            .filter(|s| s.state == ServerState::Connected)
            .count()
    }

    /// The total number of tools surfaced across every bridged server.
    #[must_use]
    pub fn tools(&self) -> usize {
        self.servers.iter().map(|s| s.tools.len()).sum()
    }
}

/// The result of a session-start load (FR-008, FR-018).
#[derive(Debug, Clone, Default)]
pub struct StartReport {
    /// One report per tracked connector, in id order.
    pub reports: Vec<ConnectReport>,
    /// Ids of connectors that were disabled and therefore inert (FR-018).
    pub disabled: Vec<String>,
}

impl StartReport {
    /// The number of connectors that reached `connected`.
    #[must_use]
    pub fn connected(&self) -> usize {
        self.reports
            .iter()
            .filter(|r| r.state == ConnectorLifecycleState::Connected)
            .count()
    }

    /// The number of connectors that reached `errored`.
    #[must_use]
    pub fn errored(&self) -> usize {
        self.reports
            .iter()
            .filter(|r| r.state == ConnectorLifecycleState::Errored)
            .count()
    }
}

/// The result of a disable or disconnect operation (FR-013, FR-019).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisableReport {
    /// The connector id.
    pub id: String,
    /// The number of servers that were connected and are now disconnected.
    pub servers_disconnected: usize,
    /// The number of tools deregistered from the session.
    pub tools_deregistered: usize,
}

/// A refusal or failure from a lifecycle transition.
#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    /// No store contains a connector with this id.
    #[error("connector lifecycle: unknown connector id {0}")]
    UnknownConnector(String),

    /// The connector is disabled and cannot be connected without being enabled
    /// first (FR-018, FR-019).
    #[error("connector lifecycle: connector {0} is disabled; run `/connectors enable {0}` first")]
    Disabled(String),

    /// An I/O failure while reading or writing the store's enable ledger.
    #[error("connector lifecycle: {0}")]
    Store(#[from] ConnectorError),
}

/// One connector's live runtime record.
#[derive(Debug)]
struct ConnectorRuntime {
    descriptor: ConnectorDescriptor,
    store: PathBuf,
    enabled: bool,
    auth: AuthState,
    state: ConnectorLifecycleState,
    error: Option<String>,
    /// The bridged ids this connector currently claims (for cross-connector
    /// collision checks, FR-033).
    claimed: BTreeSet<String>,
    servers: Vec<ServerRuntime>,
    refused: Vec<RefusedServer>,
}

/// One bridged server's live runtime record.
#[derive(Debug)]
struct ServerRuntime {
    server_id: String,
    state: ServerState,
    tools: Vec<String>,
    error: Option<String>,
}

impl ServerRuntime {
    /// The public report for this server.
    fn to_report(&self) -> ServerReport {
        ServerReport {
            server_id: self.server_id.clone(),
            state: self.state,
            tools: self.tools.clone(),
            error: self.error.clone(),
        }
    }
}

impl ConnectorRuntime {
    /// Build the public report for this connector.
    fn connect_report(&self) -> ConnectReport {
        ConnectReport {
            id: self.descriptor.id.as_str().to_string(),
            state: self.state,
            auth: self.auth,
            servers: self.servers.iter().map(ServerRuntime::to_report).collect(),
            refused: self.refused.clone(),
            error: self.error.clone(),
        }
    }
}

/// A public, snapshot view of one tracked connector (FR-009).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorStatus {
    /// The connector id.
    pub id: String,
    /// The display name.
    pub name: String,
    /// The lifecycle state.
    pub state: ConnectorLifecycleState,
    /// The authentication state (FR-014, FR-022).
    pub auth: AuthState,
    /// The connector-level failure cause when `state == Errored`.
    pub error: Option<String>,
    /// One report per bridged server.
    pub servers: Vec<ServerReport>,
}

/// The session-facing owner of every connector's lifecycle (FR-008).
#[derive(Debug)]
pub struct ConnectorSession {
    dirs: StoreDirs,
    config: ConnectorsConfig,
    /// Existing `ragent.json` `mcp` keys, the collision surface for FR-033.
    configured_server_ids: Vec<String>,
    /// Tracked connectors, keyed by id.
    tracked: BTreeMap<String, ConnectorRuntime>,
}

/// Disconnect every currently-connected server in `servers`, best-effort.
///
/// A failed teardown is logged, never propagated (a failed disconnect still
/// leaves the connector disabled). Returns `(servers_disconnected,
/// tools_deregistered)` for the operation's report.
async fn teardown_connected(
    connector: &str,
    connect: &mut dyn McpConnect,
    servers: &[ServerRuntime],
) -> (usize, usize) {
    let mut servers_disconnected = 0usize;
    let mut tools_deregistered = 0usize;
    for server in servers {
        if server.state == ServerState::Connected {
            if let Err(error) = connect.disconnect(&server.server_id).await {
                tracing::warn!(
                    connector,
                    server_id = %server.server_id,
                    error = %error,
                    "connector server failed to disconnect cleanly"
                );
            }
            servers_disconnected += 1;
            tools_deregistered += server.tools.len();
        }
    }
    (servers_disconnected, tools_deregistered)
}

impl ConnectorSession {
    /// Create a session over `dirs` with the given `connectors` configuration and
    /// the set of existing `ragent.json` `mcp` server ids (the FR-033 collision
    /// surface).
    #[must_use]
    pub fn new(
        dirs: StoreDirs,
        config: ConnectorsConfig,
        configured_server_ids: Vec<String>,
    ) -> Self {
        Self {
            dirs,
            config,
            configured_server_ids,
            tracked: BTreeMap::new(),
        }
    }

    /// The `connectors` configuration this session was built with. Command
    /// surfaces honour the master switch before any work (FR-021).
    #[must_use]
    pub const fn config(&self) -> &ConnectorsConfig {
        &self.config
    }

    /// Whether the connector subsystem is enabled (FR-021).
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.is_enabled()
    }

    /// Load every enabled connector once at session start (FR-008, FR-018).
    ///
    /// A disabled connector contributes nothing and is reported as `disabled`; an
    /// enabled connector resolves its servers, applies the enable ledgers,
    /// connects each bridged server, and records `connected` or `errored` with
    /// the cause. When the subsystem is disabled (FR-021) the call is inert and
    /// discovers nothing.
    pub async fn start(&mut self, env: &mut ConnectorEnv<'_>) -> StartReport {
        self.tracked.clear();
        if !self.is_enabled() {
            return StartReport::default();
        }
        let scanned = scan_dirs(self.dirs.clone());
        let mut reports = Vec::new();
        let mut disabled = Vec::new();
        for connector in scanned {
            let Ok(descriptor) = connector.outcome else {
                continue;
            };
            let store = connector.store.clone();
            let enabled = connector.enabled;
            let id = descriptor.id.as_str().to_string();
            let (runtime, report) = self.load_one(descriptor, store, enabled, env).await;
            if runtime.state == ConnectorLifecycleState::Disabled {
                disabled.push(id.clone());
            }
            reports.push(report);
            self.tracked.insert(id, runtime);
        }
        StartReport { reports, disabled }
    }

    /// Enable a connector and connect its servers now (FR-012).
    ///
    /// Idempotent: an already-connected connector re-reports its state without a
    /// second connection.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::UnknownConnector`] when no store holds `id`, or
    /// [`LifecycleError::Store`] when the enable ledger cannot be written.
    pub async fn enable(
        &mut self,
        id: &str,
        env: &mut ConnectorEnv<'_>,
    ) -> Result<ConnectReport, LifecycleError> {
        if let Some(runtime) = self.tracked.get(id)
            && runtime.enabled
            && runtime.state == ConnectorLifecycleState::Connected
        {
            return Ok(runtime.connect_report());
        }
        let (descriptor, store) = self.locate(id)?;
        self.set_store_enabled(&store, id, true)?;
        let (runtime, report) = self.load_one(descriptor, store, true, env).await;
        self.tracked.insert(id.to_string(), runtime);
        Ok(report)
    }

    /// Disable a connector, disconnect its servers, and deregister their tools
    /// (FR-013).
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::UnknownConnector`] when no store holds `id`, or
    /// [`LifecycleError::Store`] when the disable ledger cannot be written.
    pub async fn disable(
        &mut self,
        id: &str,
        env: &mut ConnectorEnv<'_>,
    ) -> Result<DisableReport, LifecycleError> {
        if !self.tracked.contains_key(id) {
            let (descriptor, store) = self.locate(id)?;
            let (runtime, _) = self.load_one(descriptor, store, false, env).await;
            self.tracked.insert(id.to_string(), runtime);
        }
        let store = self
            .tracked
            .get(id)
            .map(|runtime| runtime.store.clone())
            .ok_or_else(|| LifecycleError::UnknownConnector(id.to_string()))?;
        self.set_store_enabled(&store, id, false)?;

        let Some(runtime) = self.tracked.get_mut(id) else {
            return Err(LifecycleError::UnknownConnector(id.to_string()));
        };
        let (servers_disconnected, tools_deregistered) =
            teardown_connected(id, env.connect, &runtime.servers).await;
        runtime.enabled = false;
        runtime.state = ConnectorLifecycleState::Disabled;
        runtime.servers.clear();
        runtime.claimed.clear();
        runtime.error = None;

        Ok(DisableReport {
            id: id.to_string(),
            servers_disconnected,
            tools_deregistered,
        })
    }

    /// Connect an enabled connector's servers without a session restart
    /// (FR-019). Idempotent when already connected.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::Disabled`] when the connector is disabled (a
    /// disabled connector must be enabled first), or
    /// [`LifecycleError::UnknownConnector`] when no store holds `id`.
    pub async fn connect(
        &mut self,
        id: &str,
        env: &mut ConnectorEnv<'_>,
    ) -> Result<ConnectReport, LifecycleError> {
        if let Some(runtime) = self.tracked.get(id)
            && runtime.state == ConnectorLifecycleState::Connected
        {
            return Ok(runtime.connect_report());
        }
        let (descriptor, store) = self.locate(id)?;
        let enabled = self.tracked.get(id).is_some_and(|runtime| runtime.enabled)
            || StoreLedger::load(&store)
                .state(id)
                .is_some_and(|state| state.enabled);
        if !enabled {
            return Err(LifecycleError::Disabled(id.to_string()));
        }
        let (runtime, report) = self.load_one(descriptor, store, true, env).await;
        self.tracked.insert(id.to_string(), runtime);
        Ok(report)
    }

    /// Disconnect a connector's servers while leaving it enabled (FR-019).
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::UnknownConnector`] when the connector has not
    /// been tracked or discovered in a store.
    pub async fn disconnect(
        &mut self,
        id: &str,
        env: &mut ConnectorEnv<'_>,
    ) -> Result<DisableReport, LifecycleError> {
        let Some(runtime) = self.tracked.get_mut(id) else {
            return Err(LifecycleError::UnknownConnector(id.to_string()));
        };
        let (servers_disconnected, tools_deregistered) =
            teardown_connected(id, env.connect, &runtime.servers).await;
        runtime.servers.clear();
        runtime.claimed.clear();
        runtime.error = None;
        runtime.state = if runtime.enabled {
            ConnectorLifecycleState::Enabled
        } else {
            ConnectorLifecycleState::Disabled
        };
        Ok(DisableReport {
            id: id.to_string(),
            servers_disconnected,
            tools_deregistered,
        })
    }

    /// Disconnect every connected server at session end (FR-008).
    pub async fn shutdown(&mut self, connect: &mut dyn McpConnect) {
        for runtime in self.tracked.values_mut() {
            let _ = teardown_connected("shutdown", connect, &runtime.servers).await;
            runtime.servers.clear();
            runtime.claimed.clear();
            if runtime.enabled {
                runtime.state = ConnectorLifecycleState::Enabled;
            }
        }
    }

    /// A snapshot of every tracked connector, in id order (FR-009).
    #[must_use]
    pub fn statuses(&self) -> Vec<ConnectorStatus> {
        self.tracked
            .values()
            .map(|runtime| ConnectorStatus {
                id: runtime.descriptor.id.as_str().to_string(),
                name: runtime.descriptor.name.clone(),
                state: runtime.state,
                auth: runtime.auth,
                error: runtime.error.clone(),
                servers: runtime
                    .servers
                    .iter()
                    .map(ServerRuntime::to_report)
                    .collect(),
            })
            .collect()
    }

    /// Load one connector into a runtime and its report (FR-008, FR-018, FR-022,
    /// FR-026, FR-032, FR-033).
    async fn load_one(
        &self,
        descriptor: ConnectorDescriptor,
        store: PathBuf,
        enabled: bool,
        env: &mut ConnectorEnv<'_>,
    ) -> (ConnectorRuntime, ConnectReport) {
        let id = descriptor.id.as_str().to_string();
        let auth = match resolve(&descriptor, env.credentials, env.env) {
            Ok(auth) => auth,
            Err(error) => {
                // A credential-store read fault must not vanish; record auth as
                // failed and warn, matching the harness path.
                tracing::warn!(
                    connector = %id,
                    error = %error,
                    "connector lifecycle could not resolve the auth state; recording auth failed"
                );
                AuthState::Failed
            }
        };

        // Cross-connector collision surface (FR-033): the configured ids plus
        // every bridged id another tracked connector already claims.
        let mut ids: Vec<&str> = self
            .configured_server_ids
            .iter()
            .map(String::as_str)
            .collect();
        let extra = self.claimed_except(&id);
        ids.extend(extra.iter().map(String::as_str));
        let plan = resolve_servers(std::iter::once(&descriptor), ids);

        let mut runtime = ConnectorRuntime {
            store,
            enabled,
            auth,
            state: ConnectorLifecycleState::Disabled,
            error: None,
            claimed: BTreeSet::new(),
            servers: Vec::new(),
            refused: plan.refused.clone(),
            descriptor,
        };

        // A disabled connector is fully inert (FR-018).
        if !enabled {
            let report = runtime.connect_report();
            return (runtime, report);
        }

        // Validation: an unexpressible connector records its cause (FR-025).
        if let Err(error) = runtime.descriptor.validate() {
            runtime.state = ConnectorLifecycleState::Errored;
            runtime.error = Some(error.to_string());
            let report = runtime.connect_report();
            return (runtime, report);
        }

        // Auth gate: no valid credential refuses the connect and is not retried
        // (FR-022, FR-032).
        if !runtime.auth.permits_connect() {
            let guidance = AuthRequirement::for_descriptor(&runtime.descriptor).guidance();
            runtime.state = ConnectorLifecycleState::Errored;
            runtime.error = Some(if guidance.is_empty() {
                "auth failed".to_string()
            } else {
                format!("auth failed: {guidance}")
            });
            let report = runtime.connect_report();
            return (runtime, report);
        }

        // Connect each accepted server independently (FR-026), skipping a server
        // the durable ledger switched off (FR-018).
        for server in &plan.servers {
            let bridged = server.server_id.clone();
            runtime.claimed.insert(bridged.clone());
            let server_runtime = if env.server_ledger.is_enabled(&bridged) {
                match env.connect.connect(&bridged, server.config.clone()).await {
                    Ok(tools) => ServerRuntime {
                        server_id: bridged,
                        state: ServerState::Connected,
                        tools,
                        error: None,
                    },
                    Err(cause) => ServerRuntime {
                        server_id: bridged,
                        state: ServerState::Errored,
                        tools: Vec::new(),
                        error: Some(cause),
                    },
                }
            } else {
                ServerRuntime {
                    server_id: bridged,
                    state: ServerState::Disconnected,
                    tools: Vec::new(),
                    error: None,
                }
            };
            runtime.servers.push(server_runtime);
        }

        let first_error = runtime.servers.iter().find_map(|s| s.error.clone());
        let all_errored = !runtime.servers.is_empty()
            && runtime
                .servers
                .iter()
                .all(|s| s.state == ServerState::Errored);
        runtime.state = aggregate_state(&runtime.servers);
        runtime.error = all_errored
            .then(|| first_error.unwrap_or_else(|| "servers failed to connect".to_string()));
        if runtime.servers.is_empty() {
            runtime.state = ConnectorLifecycleState::Errored;
            runtime.error = Some(if runtime.refused.is_empty() {
                "no expressible server to connect".to_string()
            } else {
                "every bridged server id was refused".to_string()
            });
        }
        let report = runtime.connect_report();
        (runtime, report)
    }

    /// The bridged ids claimed by every tracked connector other than `id`
    /// (FR-033).
    fn claimed_except(&self, id: &str) -> Vec<String> {
        self.tracked
            .iter()
            .filter(|(key, _)| key.as_str() != id)
            .flat_map(|(_, runtime)| runtime.claimed.iter().cloned())
            .collect()
    }

    /// Find a connector across the configured stores (project wins on collision),
    /// returning its descriptor and store directory (FR-001).
    fn locate(&self, id: &str) -> Result<(ConnectorDescriptor, PathBuf), LifecycleError> {
        scan_dirs(self.dirs.clone())
            .into_iter()
            .find_map(|connector| {
                let descriptor = connector.outcome.ok()?;
                (descriptor.id.as_str() == id).then_some((descriptor, connector.store))
            })
            .ok_or_else(|| LifecycleError::UnknownConnector(id.to_string()))
    }

    /// Persist the connector's enable flag into its store's `_state.json` ledger
    /// (FR-012, FR-013).
    fn set_store_enabled(
        &self,
        store: &Path,
        id: &str,
        enabled: bool,
    ) -> Result<(), LifecycleError> {
        let mut ledger = StoreLedger::load(store);
        ledger.state_mut(id).enabled = enabled;
        ledger.save(store)?;
        Ok(())
    }
}

/// The aggregate connector state from its per-server states (FR-026): connected
/// when any server connected, errored when one failed and none connected,
/// otherwise enabled-but-not-connected.
fn aggregate_state(servers: &[ServerRuntime]) -> ConnectorLifecycleState {
    if servers.iter().any(|s| s.state == ServerState::Connected) {
        ConnectorLifecycleState::Connected
    } else if servers.iter().any(|s| s.state == ServerState::Errored) {
        ConnectorLifecycleState::Errored
    } else {
        ConnectorLifecycleState::Enabled
    }
}

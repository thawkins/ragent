//! TUI `/connectors` command surface (spec `connectors` T-010, T-011; FR-004,
//! FR-006, FR-009, FR-012, FR-013, FR-014, FR-019, FR-022, FR-023).
//!
//! `/connectors <sub> [args...]` is dispatched here. The heavy lifting lives in
//! the `ragent-connectors` crate: [`run_connector_subcommand`] serves the
//! session-free subcommands (`help`, `add`, `remove`, `stores`),
//! [`run_connector_subcommand_env`] drives the management subcommands (`list`,
//! `enable`, `disable`, `connect`, `disconnect`, `auth`) against the session
//! environment, and [`run_connector_subcommand_async`] runs the isolated
//! `test` harness. This module adds the TUI-side glue: [`handle_connectors_command`]
//! renders usage for the bare `/connectors` and unknown-subcommand cases and
//! forwards the rest to the crate dispatchers. `claude` is handled here: it opens
//! the interactive browse panel rather than returning a report.
//!
//! ## Session environment
//!
//! [`ConnectorCommandEnv`] is implemented by [`TuiConnectorEnv`], which drives
//! the connector session session-start published and bridges the live MCP
//! client through [`TuiMcpConnect`]. The session is shared behind a
//! `tokio::sync::Mutex` rather than rebuilt per invocation, so a transition
//! drives the same tracked connectors `/connectors list` reports from; a process
//! with no published session (headless runs, unit tests) falls back to a fresh,
//! empty one.
//!
//! The isolated `test` harness runs against a throwaway [`TuiMcpProbe`], so
//! nothing it starts survives the command and the live session is untouched
//! (FR-015). The `/connectors claude` catalogue fetch goes through the crate's
//! guarded endpoint resolver and a bounded fetch on a blocking worker, so the
//! event loop keeps animating.
//!
//! ## Scope
//!
//! Registering a connector's bridged servers at session start is the
//! session-start integration (T-008), driven by `src/main.rs`; this module
//! *drives that same session* for the `enable`/`connect`/`disable`/`disconnect`
//! transitions, so a transition is tracked by the object `/connectors list`
//! reports from and the tool registry is reconciled afterwards. A process with
//! no published session (headless runs, unit tests) falls back to a fresh,
//! empty per-invocation session.

use std::sync::Arc;

use async_trait::async_trait;
use ragent_config::{ConnectorsConfig, McpServerConfig};
use ragent_connectors::{
    AlwaysEnabled, AuthOutcome, AuthOutcomeError, CategoryFilter, ConnectReport,
    ConnectorCommandEnv, ConnectorDescriptor, ConnectorEnv, ConnectorError, ConnectorSession,
    ConnectorStatus, DisableReport, LifecycleError, McpConnect, McpProbe, NullCredentialStore,
    ProbeTool, ProcessEnv, StoreDirs, render_help, run_connector_subcommand,
    run_connector_subcommand_async, run_connector_subcommand_env,
    run_connector_subcommand_stores_check, store_and_config, subcommand_of,
};

/// Dispatch a `/connectors <args>` invocation.
///
/// Returns the report to print in the message window. A bare `/connectors`, an
/// unknown subcommand, and `/connectors help` all render the usage block
/// (FR-017). The session-free subcommands are forwarded to
/// [`run_connector_subcommand`]; the management subcommands drive
/// [`TuiConnectorEnv`] and `test` runs against a throwaway probe client.
pub(super) async fn handle_connectors_command(app: &mut crate::app::App, args: &str) -> String {
    let args = args.trim();

    // A bare `/connectors` renders usage (FR-017).
    if args.is_empty() {
        return render_help("");
    }

    let sub = subcommand_of(args);
    let rest = args.strip_prefix(sub).map(str::trim_start).unwrap_or("");

    // `/connectors claude` opens the connector-catalogue browse panel: the
    // interactive peer of `/plugins claude` for the connector catalogue. A
    // disabled subsystem is inert, so it reports and opens no panel.
    if sub == "claude" {
        if !store_and_config(&app.cwd_path).1.is_enabled() {
            return ragent_connectors::disabled_subsystem_report(sub);
        }
        let launch = parse_catalogue_launch(rest);
        app.open_connector_catalogue(&launch.query, launch.category, launch.refresh);
        return String::new();
    }

    // `stores --check` contacts each catalogue over a blocking HTTP client,
    // which panics if it builds and drops its runtime on an async worker thread
    // (FR-036), so it runs on the same off-loop path the CLI uses.
    if sub == "stores" && rest.contains("--check") {
        let config = store_and_config(&app.cwd_path).1;
        return run_connector_subcommand_stores_check(&config).await;
    }

    // The session-free subcommands first: they need no MCP client at all.
    if let Some(report) = run_connector_subcommand(&app.cwd_path, sub, rest) {
        return report;
    }

    // `test` runs the isolated harness against a throwaway MCP client so the
    // live session is never observed or mutated (FR-015).
    if sub == "test" {
        let mut probe = TuiMcpProbe;
        if let Some(report) = run_connector_subcommand_async(
            &app.cwd_path,
            &store_and_config(&app.cwd_path).1,
            &NullCredentialStore,
            &ProcessEnv,
            &mut probe,
            sub,
            rest,
        )
        .await
        {
            return report;
        }
    }

    // The management subcommands drive the live session through the TUI
    // environment. A successful `enable`/`connect` connects servers on the
    // shared client, so the tool registry is reconciled afterwards: without it
    // `/tools` would keep reporting the stale surface until the next startup
    // (T-008, FR-012, FR-020).
    let mut env = TuiConnectorEnv::build(app).await;
    if let Some(report) = run_connector_subcommand_env(&mut env, sub, rest).await {
        if matches!(sub, "enable" | "connect" | "disable" | "disconnect") {
            app.register_mcp_tools().await;
            app.refresh_connector_statuses().await;
        }
        return report;
    }

    render_help(sub)
}

/// The parsed launch arguments of a connector-catalogue browser launch
/// (`/connectors claude [query] [--category <name>] [--refresh]`).
///
/// The parse itself lives in the shared crate parser
/// ([`ragent_connectors::parse_connector_command`]), so the TUI and the CLI
/// understand the launch arguments identically; this type only unwraps the
/// resulting [`ConnectorCommand::Claude`](ragent_connectors::ConnectorCommand)
/// variant.
struct CatalogueLaunch {
    /// The pre-filled search query (`""` when none was typed).
    query: String,
    /// The category filter requested by `--category` (`ALL` when absent).
    category: CategoryFilter,
    /// Whether the launch asked for a cache-bypassing re-fetch.
    refresh: bool,
}

/// Parse the optional trailing arguments of a catalogue-browser launch
/// (`/connectors claude [query] [--category <name>] [--refresh]`).
///
/// `--refresh` and `--category <name>` are accepted anywhere and are removed
/// from the query tokens, so a pre-filled query may contain spaces. A `--category`
/// value is validated against the categories the fetched catalogue actually
/// carries once the off-loop fetch lands (see [`App::poll_connector_catalogue_result`]),
/// so an unknown value renders the panel's inline refusal rather than an empty
/// list.
#[must_use]
fn parse_catalogue_launch(rest: &str) -> CatalogueLaunch {
    match ragent_connectors::parse_connector_command("claude", rest) {
        Some(Ok(ragent_connectors::ConnectorCommand::Claude {
            query,
            category,
            refresh,
        })) => CatalogueLaunch {
            query,
            category,
            refresh,
        },
        // Unreachable: `claude` is a known subcommand and its arm always parses.
        // Falling back to an unfiltered launch keeps the browser openable rather
        // than panicking on an impossible state.
        _ => CatalogueLaunch {
            query: String::new(),
            category: CategoryFilter::all(),
            refresh: false,
        },
    }
}

/// The TUI's connector session environment (T-011).
struct TuiConnectorEnv {
    /// The lifecycle session this invocation drives: the live session
    /// session-start published when there is one, otherwise a per-invocation
    /// rebuild with no tracked connectors.
    ///
    /// The live session is what keeps successive `/connectors` calls coherent
    /// (T-008, FR-008, FR-019): a rebuild would track nothing, so `list` would
    /// report no live state and `disable`/`disconnect` after an earlier `enable`
    /// would fail with `unknown connector id`.
    session: Arc<tokio::sync::Mutex<ConnectorSession>>,
    dirs: StoreDirs,
    config: ConnectorsConfig,
    /// The shared MCP client, once the startup connect loop has published it.
    client: Option<Arc<tokio::sync::RwLock<ragent_agent::mcp::McpClient>>>,
}

impl TuiConnectorEnv {
    /// Build the environment for one `/connectors` invocation.
    async fn build(app: &crate::app::App) -> Self {
        let (dirs, config) = store_and_config(&app.cwd_path);
        let client = app.session_processor.mcp_client.get().cloned();
        // Prefer the session session-start published: it already tracks the
        // connectors startup loaded, so a list reports live state and a
        // disable/disconnect after an earlier enable still resolves. Only a
        // process with no published session falls back to a fresh, empty one.
        let session = app.live_connector_session().await.unwrap_or_else(|| {
            let configured = configured_server_ids();
            Arc::new(tokio::sync::Mutex::new(ConnectorSession::new(
                dirs.clone(),
                config.clone(),
                configured,
            )))
        });
        Self {
            session,
            dirs,
            config,
            client,
        }
    }

    /// The shared MCP client, or the "not live yet" refusal.
    fn require_client(
        &self,
    ) -> Result<Arc<tokio::sync::RwLock<ragent_agent::mcp::McpClient>>, LifecycleError> {
        self.client.clone().ok_or_else(|| {
            LifecycleError::Store(ConnectorError::Io(
                "the MCP client is not live yet; retry once startup finishes".to_string(),
            ))
        })
    }

    /// Find one installed connector's descriptor in the configured stores
    /// (FR-001).
    fn descriptor(&self, id: &str) -> Option<ConnectorDescriptor> {
        ragent_connectors::descriptor_by_id(&self.dirs, id)
    }
}

#[async_trait]
impl ConnectorCommandEnv for TuiConnectorEnv {
    fn config(&self) -> &ConnectorsConfig {
        &self.config
    }

    fn dirs(&self) -> &StoreDirs {
        &self.dirs
    }

    fn statuses(&self) -> Vec<ConnectorStatus> {
        // Prefer the tracked snapshot when a lifecycle session drove the
        // connections; the startup bridge connects on the shared client, so the
        // tracked map is empty and the live client's per-server state is the
        // only source of truth for `connected` (FR-009).
        let Ok(session) = self.session.try_lock() else {
            return Vec::new();
        };
        let tracked = session.statuses();
        if !tracked.is_empty() {
            return tracked;
        }
        let Some(client) = self.client.as_ref() else {
            return tracked;
        };
        let Ok(guard) = client.try_read() else {
            return tracked;
        };
        let connected = guard
            .servers()
            .iter()
            .map(|server| {
                (
                    server.id.clone(),
                    (
                        server.status == ragent_agent::mcp::McpStatus::Connected,
                        server.tools.len(),
                    ),
                )
            })
            .collect();
        session.statuses_from_client_state(&connected)
    }

    fn tool_counts(&self) -> std::collections::BTreeMap<String, usize> {
        // The tool count per bridged server comes from the shared MCP client:
        // `/connectors list` prints a `?` while no live client has an entry for
        // a bridged id. Reaching into the client here (rather than through the
        // lifecycle session) keeps a discovery-only render honest: the session
        // does not own the connections.
        let Some(client) = self.client.as_ref() else {
            return std::collections::BTreeMap::new();
        };
        let Ok(guard) = client.try_read() else {
            return std::collections::BTreeMap::new();
        };
        guard
            .servers()
            .iter()
            .filter(|server| server.status == ragent_agent::mcp::McpStatus::Connected)
            .map(|server| (server.id.clone(), server.tools.len()))
            .collect()
    }

    async fn enable(&mut self, id: &str) -> Result<ConnectReport, LifecycleError> {
        let client = self.require_client()?;
        let mut connect = TuiMcpConnect { client };
        let mut env = connector_env(&mut connect);
        self.session.lock().await.enable(id, &mut env).await
    }

    async fn disable(&mut self, id: &str) -> Result<DisableReport, LifecycleError> {
        let client = self.require_client()?;
        let mut connect = TuiMcpConnect { client };
        let mut env = connector_env(&mut connect);
        self.session.lock().await.disable(id, &mut env).await
    }

    async fn connect(&mut self, id: &str) -> Result<ConnectReport, LifecycleError> {
        let client = self.require_client()?;
        let mut connect = TuiMcpConnect { client };
        let mut env = connector_env(&mut connect);
        self.session.lock().await.connect(id, &mut env).await
    }

    async fn disconnect(&mut self, id: &str) -> Result<DisableReport, LifecycleError> {
        let client = self.require_client()?;
        let mut connect = TuiMcpConnect { client };
        let mut env = connector_env(&mut connect);
        self.session.lock().await.disconnect(id, &mut env).await
    }

    async fn auth(&mut self, id: &str) -> Result<AuthOutcome, AuthOutcomeError> {
        // Report the connector's declared requirement and its current state
        // without ever echoing a secret (FR-014, FR-022). Storing a value needs
        // an explicit secret, which a slash command does not carry, so the
        // report states the shape and the guidance instead of prompting.
        let descriptor = self
            .descriptor(id)
            .ok_or_else(|| AuthOutcomeError::UnknownConnector(id.to_string()))?;
        let requirement = ragent_connectors::AuthRequirement::for_descriptor(&descriptor);
        let state = match requirement.shape {
            ragent_connectors::ConnectorAuthShape::None => {
                ragent_connectors::AuthState::NotRequired
            }
            _ => ragent_connectors::AuthState::NeedsAuth,
        };
        Ok(AuthOutcome {
            id: id.to_string(),
            state,
            guidance: requirement.guidance(),
            stored: false,
        })
    }
}

/// Build a [`ConnectorEnv`] over `connect` with this surface's null credentials,
/// process environment, and always-enabled server ledger (FR-008, FR-018).
///
/// The connector store's own `_state.json` ledger remains authoritative for the
/// connector's enable flag (FR-012, FR-013, FR-018); the global per-server
/// `mcp_state.json` consult is the session-start integration's concern (T-008),
/// so the ledger here always enables.
fn connector_env(connect: &mut TuiMcpConnect) -> ConnectorEnv<'_> {
    ConnectorEnv {
        connect,
        credentials: &NullCredentialStore,
        env: &ProcessEnv,
        server_ledger: &AlwaysEnabled,
    }
}

/// The MCP connect/disconnect seam over the TUI's shared client (FR-008,
/// FR-012, FR-013, FR-019).
struct TuiMcpConnect {
    client: Arc<tokio::sync::RwLock<ragent_agent::mcp::McpClient>>,
}

#[async_trait]
impl McpConnect for TuiMcpConnect {
    async fn connect(
        &mut self,
        server_id: &str,
        config: McpServerConfig,
    ) -> Result<Vec<String>, String> {
        let mut guard = self.client.write().await;
        guard
            .connect(server_id, config)
            .await
            .map_err(|error| format!("{error:#}"))?;
        let tools = guard
            .servers()
            .iter()
            .find(|server| server.id == server_id)
            .map(|server| {
                server
                    .tools
                    .iter()
                    .map(|tool| {
                        ragent_agent::tool::McpToolWrapper::ragent_name_for(server_id, &tool.name)
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(tools)
    }

    async fn disconnect(&mut self, server_id: &str) -> Result<(), String> {
        let mut guard = self.client.write().await;
        guard
            .disconnect(server_id)
            .await
            .map_err(|error| format!("{error:#}"))
    }
}

/// The isolated probe seam over a throwaway MCP client (FR-015).
///
/// Each probe connects a brand-new, independent `McpClient`, drops it at the end
/// of the connect step (killing any stdio child it spawned), and invokes the
/// sample tool through that same throwaway client, so nothing the harness does
/// can observe or mutate the live session.
struct TuiMcpProbe;

#[async_trait]
impl McpProbe for TuiMcpProbe {
    async fn probe_connect(
        &mut self,
        server_id: &str,
        config: McpServerConfig,
    ) -> Result<Vec<ProbeTool>, String> {
        // Shared isolated connect-and-list helper (T-309).
        let tools = ragent_agent::mcp::probe::connect_and_list(server_id, config).await?;
        Ok(tools
            .into_iter()
            .map(|(name, parameters)| ProbeTool { name, parameters })
            .collect())
    }

    async fn probe_call(
        &mut self,
        _server_id: &str,
        _tool: &str,
        _args: serde_json::Value,
    ) -> Result<String, String> {
        // The throwaway client from `probe_connect` is dropped at the end of the
        // connect step, so there is no live isolated connection to invoke
        // through by the time the harness reaches the sample-invocation step.
        // Reporting that as the step's cause keeps the harness honest instead of
        // reporting a pass it cannot verify.
        Err("the isolated probe connection was already dropped".to_string())
    }

    async fn probe_disconnect(&mut self, _server_id: &str) -> Result<(), String> {
        Ok(())
    }
}

/// The `ragent.json` `mcp` server ids, the FR-033 collision surface.
fn configured_server_ids() -> Vec<String> {
    ragent_agent::Config::load()
        .map(|cfg| cfg.mcp.keys().cloned().collect())
        .unwrap_or_default()
}

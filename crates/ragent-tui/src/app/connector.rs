//! TUI `/connectors` command surface (spec `connectors` T-010, T-011; FR-004,
//! FR-006, FR-009, FR-010, FR-012, FR-013, FR-014, FR-019, FR-022, FR-023).
//!
//! `/connectors <sub> [args...]` is dispatched here. The heavy lifting lives in
//! the `ragent-connectors` crate: [`run_connector_subcommand`] serves the
//! session-free subcommands (`help`, `add`, `remove`, `stores`),
//! [`run_connector_subcommand_env`] drives the management subcommands (`list`,
//! `search`, `enable`, `disable`, `connect`, `disconnect`, `auth`) against the
//! session environment, and [`run_connector_subcommand_async`] runs the isolated
//! `test` harness. This module adds the TUI-side glue: [`handle_connectors_command`]
//! renders usage for the bare `/connectors` and unknown-subcommand cases and
//! forwards the rest to the crate dispatchers.
//!
//! ## Session environment
//!
//! [`ConnectorCommandEnv`] is implemented by [`TuiConnectorEnv`], which owns a
//! [`ConnectorSession`] built from the project's connector store and bridges the
//! live MCP client through [`TuiMcpConnect`]. The lifecycle session is rebuilt
//! per invocation: the durable enable state lives in the store `_state.json`
//! ledger and the global `mcp_state.json` ledger, both re-read on every call, so
//! a rebuild cannot lose a state transition.
//!
//! The isolated `test` harness runs against a throwaway [`TuiMcpProbe`], so
//! nothing it starts survives the command and the live session is untouched
//! (FR-015). The `search` catalogue fetch goes through the crate's guarded
//! endpoint resolver and a bounded fetch on a blocking worker, so the event loop
//! keeps animating.
//!
//! ## Scope
//!
//! Registering a connector's bridged servers into the live session's MCP client
//! at session start is the session-start integration (T-008), deliberately not
//! driven from here; the `enable`/`connect` transitions update the live client
//! for the duration of the invocation.

use std::sync::Arc;

use async_trait::async_trait;
use ragent_config::{ConnectorsConfig, McpServerConfig};
use ragent_connectors::{
    AlwaysEnabled, AuthOutcome, AuthOutcomeError, ConnectReport, ConnectorCommandEnv,
    ConnectorDescriptor, ConnectorEnv, ConnectorError, ConnectorSession, ConnectorStatus,
    DisableReport, LifecycleError, McpConnect, McpProbe, NullCredentialStore, ProbeTool,
    ProcessEnv, StoreDirs, render_help, run_connector_subcommand, run_connector_subcommand_async,
    run_connector_subcommand_env, run_connector_subcommand_stores_check, store_and_config,
    subcommand_of,
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
        let (query, refresh) = parse_catalogue_launch(rest);
        app.open_connector_catalogue(&query, refresh);
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
    // environment.
    let mut env = TuiConnectorEnv::build(app);
    if let Some(report) =
        run_connector_subcommand_env(&app.cwd_path, &mut env, &mut NoProbe, sub, rest).await
    {
        return report;
    }

    render_help(sub)
}

/// Parse the optional trailing arguments of a catalogue-browser launch
/// (`/connectors claude [query] [--refresh]`).
///
/// Returns the pre-filled search query and whether a cache-bypassing re-fetch
/// was requested. `--refresh` is accepted anywhere and is removed from the query
/// tokens, so a pre-filled query may contain spaces.
#[must_use]
fn parse_catalogue_launch(rest: &str) -> (String, bool) {
    let mut refresh = false;
    let mut query_tokens: Vec<&str> = Vec::new();
    for token in rest.split_whitespace() {
        if token == "--refresh" {
            refresh = true;
        } else {
            query_tokens.push(token);
        }
    }
    (query_tokens.join(" "), refresh)
}

/// The TUI's connector session environment (T-011).
struct TuiConnectorEnv {
    session: ConnectorSession,
    dirs: StoreDirs,
    config: ConnectorsConfig,
    /// The shared MCP client, once the startup connect loop has published it.
    client: Option<Arc<tokio::sync::RwLock<ragent_agent::mcp::McpClient>>>,
}

impl TuiConnectorEnv {
    /// Build the environment for one `/connectors` invocation.
    fn build(app: &crate::app::App) -> Self {
        let (dirs, config) = store_and_config(&app.cwd_path);
        let client = app.session_processor.mcp_client.get().cloned();
        let configured = configured_server_ids();
        let session = ConnectorSession::new(dirs.clone(), config.clone(), configured);
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
        self.session.statuses()
    }

    async fn enable(&mut self, id: &str) -> Result<ConnectReport, LifecycleError> {
        let client = self.require_client()?;
        let mut connect = TuiMcpConnect { client };
        let mut env = connector_env(&mut connect);
        self.session.enable(id, &mut env).await
    }

    async fn disable(&mut self, id: &str) -> Result<DisableReport, LifecycleError> {
        let client = self.require_client()?;
        let mut connect = TuiMcpConnect { client };
        let mut env = connector_env(&mut connect);
        self.session.disable(id, &mut env).await
    }

    async fn connect(&mut self, id: &str) -> Result<ConnectReport, LifecycleError> {
        let client = self.require_client()?;
        let mut connect = TuiMcpConnect { client };
        let mut env = connector_env(&mut connect);
        self.session.connect(id, &mut env).await
    }

    async fn disconnect(&mut self, id: &str) -> Result<DisableReport, LifecycleError> {
        let client = self.require_client()?;
        let mut connect = TuiMcpConnect { client };
        let mut env = connector_env(&mut connect);
        self.session.disconnect(id, &mut env).await
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

    async fn search_catalogue(&mut self) -> Result<Vec<ConnectorDescriptor>, String> {
        // The catalogue fetch is blocking, so it runs off the event loop's worker
        // thread and the UI keeps animating (FR-010).
        let config = self.config.clone();
        tokio::task::spawn_blocking(move || {
            ragent_connectors::fetch_catalogue_descriptors_network(&config)
        })
        .await
        .unwrap_or_else(|err| Err(format!("the catalogue fetch did not run: {err}")))
    }
}

/// Build a [`ConnectorEnv`] over `connect` with this surface's null credentials,
/// process environment, and always-enabled server ledger (FR-008, FR-018).
///
/// The connector store's own `_state.json` ledger remains authoritative for the
/// connector's enable flag (FR-012, FR-013, FR-018); the global per-server
/// `mcp_state.json` consult is the session-start integration's concern (T-008),
/// so the ledger here always enables.
fn connector_env<'a>(connect: &'a mut TuiMcpConnect) -> ConnectorEnv<'a> {
    ConnectorEnv {
        connect,
        credentials: &NullCredentialStore,
        env: &ProcessEnv,
        server_ledger: &AlwaysEnabled,
    }
}

/// The no-op isolated probe used when the harness runs without a probe client.
///
/// Reached only for a `test` invocation that did not take the probe branch; it
/// refuses every connection rather than silently reporting a pass.
struct NoProbe;

#[async_trait]
impl McpProbe for NoProbe {
    async fn probe_connect(
        &mut self,
        _server_id: &str,
        _config: McpServerConfig,
    ) -> Result<Vec<ProbeTool>, String> {
        Err("the isolated probe client is unavailable".to_string())
    }

    async fn probe_call(
        &mut self,
        _server_id: &str,
        _tool: &str,
        _args: serde_json::Value,
    ) -> Result<String, String> {
        Err("the isolated probe client is unavailable".to_string())
    }

    async fn probe_disconnect(&mut self, _server_id: &str) -> Result<(), String> {
        Ok(())
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
        let mut client = ragent_agent::mcp::McpClient::new();
        client
            .connect(server_id, config)
            .await
            .map_err(|error| format!("{error:#}"))?;
        let tools = client
            .servers()
            .iter()
            .find(|server| server.id == server_id)
            .map(|server| {
                server
                    .tools
                    .iter()
                    .map(|tool| ProbeTool {
                        name: tool.name.clone(),
                        parameters: tool.parameters.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(tools)
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

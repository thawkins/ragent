//! `ragent connectors` CLI parity surface (spec `connectors` T-014; FR-004,
//! FR-006).
//!
//! Mirrors the TUI `/connectors` slash family so the connector store can be
//! managed from a shell without launching the TUI:
//! `ragent connectors <sub> [args...]` for the twelve subcommands (`list`,
//! `search`, `add`, `remove`, `enable`, `disable`, `connect`, `disconnect`,
//! `auth`, `test`, `stores`, `help`). The parse-and-run logic lives entirely in
//! the `ragent-connectors` crate ([`ragent_connectors::run_connector_subcommand`]
//! for the session-free subcommands and
//! [`ragent_connectors::run_connector_subcommand_env`] for the management
//! subcommands); this module supplies only the CLI-side glue:
//!
//! - the argument vector is joined and split into a subcommand token and the
//!   remaining text exactly as the TUI dispatch arm does;
//! - the management subcommands drive an ephemeral
//!   [`ragent_connectors::ConnectorSession`] through a [`CliConnectorEnv`], and
//!   `test` drives an isolated probe, so the CLI never borrows a live session
//!   (a one-shot invocation has none);
//! - reports print to stdout, with the `From: /connectors ...` TUI attribution
//!   header replaced by a plain `ragent connectors ...` line, the
//!   `## /connectors` usage title rewritten to `## ragent connectors`, and the
//!   usage-table rows swapping the `/connectors` trigger for
//!   `ragent connectors` (FR-004: one wording, two surfaces).
//!
//! No MCP client is connected for a one-shot CLI invocation, so a connector's
//! live tool counts stay `?` (unknown, not zero): the count is only known from a
//! live connection.

use anyhow::Result;

use async_trait::async_trait;
use ragent_config::{ConnectorsConfig, McpServerConfig};
use ragent_connectors::{
    AlwaysEnabled, AuthOutcome, AuthOutcomeError, AuthState, CONNECTOR_SUBCOMMANDS, ConnectReport,
    ConnectorCommandEnv, ConnectorDescriptor, ConnectorSession, ConnectorStatus, DisableReport,
    LifecycleError, McpConnect, McpProbe, NullCredentialStore, ProbeTool, ProcessEnv, StoreDirs,
    render_help, run_connector_subcommand, run_connector_subcommand_async,
    run_connector_subcommand_env, run_connector_subcommand_stores_check, store_and_config,
    subcommand_of,
};

/// Entry point for `ragent connectors <args...>`, invoked from the CLI dispatcher.
///
/// `args` is everything after the `connectors` verb (may be empty). Prints the
/// report for the requested subcommand to stdout and returns `Ok(())`; refusal
/// and usage cases render `[err]`/usage text rather than returning an error
/// (matching the TUI surface, which never propagates `/connectors` failures).
///
/// # Errors
///
/// Returns an error only when the current working directory cannot be resolved.
pub async fn run_cli(args: &[String]) -> Result<()> {
    let joined = args.join(" ");
    let text = joined.trim();

    // A bare `ragent connectors` renders usage (FR-004, FR-017 parity).
    if text.is_empty() {
        println!("{}", cli_usage());
        return Ok(());
    }

    let sub = subcommand_of(text);
    // Strip the typed prefix rather than slicing by `sub.len()`: the two agree
    // today, but a byte slice indexed by `sub` would panic if they ever
    // diverged. Mirrors the TUI dispatch arm in
    // `crates/ragent-tui/src/app/connector.rs`.
    let rest = text.strip_prefix(sub).map(str::trim_start).unwrap_or("");

    // `claude` opens the interactive catalogue browser, which owns the screen
    // and needs a terminal. A one-shot CLI invocation has neither, and a
    // headless run must not hang on a panel nobody can see, so the CLI
    // describes the browser and points at the TUI surface instead of opening
    // it (one wording, two surfaces: FR-004).
    if sub == "claude" {
        println!("{}", cli_body(&catalogue_browser_pointer()));
        return Ok(());
    }

    // `help` and any unrecognised subcommand both render the usage block.
    if sub == "help" || !CONNECTOR_SUBCOMMANDS.contains(&sub) {
        println!("{}", cli_usage());
        return Ok(());
    }

    let workdir = std::env::current_dir()?;

    // `stores --check` contacts each catalogue over a blocking HTTP client,
    // which panics if it builds and drops its runtime on an async worker thread
    // (FR-036), so it runs on the off-loop blocking path.
    if sub == "stores" && rest.contains("--check") {
        let config = store_and_config(&workdir).1;
        let report = run_connector_subcommand_stores_check(&config).await;
        println!("{}", cli_body(&report));
        return Ok(());
    }

    // The session-free subcommands first: they need no MCP client at all.
    if let Some(report) = run_connector_subcommand(&workdir, sub, rest) {
        println!("{}", cli_body(&report));
        return Ok(());
    }

    // `test` runs the isolated harness against a throwaway MCP client so no
    // live session is observed or mutated (FR-015). The harness itself creates
    // and drops its client inside the connect step, so nothing it starts
    // outlives this invocation.
    if sub == "test" {
        let mut probe = CliMcpProbe;
        if let Some(report) = run_connector_subcommand_async(
            &workdir,
            &store_and_config(&workdir).1,
            &NullCredentialStore,
            &ProcessEnv,
            &mut probe,
            sub,
            rest,
        )
        .await
        {
            println!("{}", cli_body(&report));
            return Ok(());
        }
    }

    // The management subcommands drive the ephemeral session environment.
    let mut env = CliConnectorEnv::build(&workdir);
    let report = run_connector_subcommand_env(&workdir, &mut env, &mut NeverProbe, sub, rest)
        .await
        .unwrap_or_else(|| render_help(sub));
    println!("{}", cli_body(&report));
    Ok(())
}

/// Render the `ragent connectors` help block from the shared
/// [`ragent_connectors::render_help`] body.
///
/// Delegates to the `ragent-connectors` crate rather than hand-copying the usage
/// table (see `ANTIPAT.md` M3.10), then rewrites the TUI attribution/heading for
/// the non-TUI surface.
#[must_use]
fn cli_usage() -> String {
    cli_body(&render_help("help"))
}

/// The `ragent connectors claude` pointer block.
///
/// The interactive Claude connector-catalogue browser lives on the TUI surface
/// (`/connectors claude`); a one-shot CLI invocation has no terminal to hand it,
/// so the CLI states what the browser does and how to open it, and names the
/// non-interactive equivalents. ASCII only, in the house attribution style.
#[must_use]
fn catalogue_browser_pointer() -> String {
    "From: /connectors claude\n\n\
     The Claude connector-catalogue browser is an interactive TUI panel: run \
     `/connectors claude [query] [--refresh]` inside ragent to filter the \
     catalogue, browse it with Up/Down, and install a connector with Enter.\n\n\
     From a shell, use the non-interactive equivalents:\n\
     - `ragent connectors search <query> [--category <name>]` - search the catalogue;\n\
     - `ragent connectors add <id>` - install a connector from the catalogue."
        .to_string()
}

/// Rewrite a shared `/connectors` report for the CLI surface (FR-006): the TUI
/// `From: /connectors ...` attribution becomes `ragent connectors ...`, the
/// `## /connectors` usage heading becomes `## ragent connectors`, and the
/// usage-table rows swap the `/connectors` trigger for `ragent connectors`. The
/// body is otherwise printed verbatim so both surfaces stay in step.
#[must_use]
fn cli_body(report: &str) -> String {
    let rewritten = report
        .strip_prefix("From: /connectors")
        .map(|tail| format!("ragent connectors{tail}"))
        .unwrap_or_else(|| report.to_string());
    let rewritten = rewritten
        .replace("## /connectors", "## ragent connectors")
        .replace("`/connectors ", "`ragent connectors ");
    format!("{rewritten}\n")
}

/// The CLI connector session environment.
///
/// A one-shot invocation has no live session, so the environment owns an
/// ephemeral [`ConnectorSession`] over a [`CliMcpConnect`] that tracks only the
/// connections it started itself. The durable enable state lives in the store
/// `_state.json` ledger and the global `mcp_state.json` ledger, both re-read on
/// every invocation, so a throwaway session cannot lose a state transition.
struct CliConnectorEnv {
    session: ConnectorSession,
    dirs: StoreDirs,
    config: ConnectorsConfig,
}

impl CliConnectorEnv {
    /// Build the environment for one `ragent connectors` invocation.
    fn build(workdir: &std::path::Path) -> Self {
        let (dirs, config) = store_and_config(workdir);
        let session = ConnectorSession::new(dirs.clone(), config.clone(), Vec::new());
        Self {
            session,
            dirs,
            config,
        }
    }

    /// Find one installed connector's descriptor in the configured stores
    /// (FR-001).
    fn descriptor(&self, id: &str) -> Option<ConnectorDescriptor> {
        ragent_connectors::descriptor_by_id(&self.dirs, id)
    }
}

#[async_trait]
impl ConnectorCommandEnv for CliConnectorEnv {
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
        let mut connect = CliMcpConnect;
        let mut env = connector_env(&mut connect);
        self.session.enable(id, &mut env).await
    }

    async fn disable(&mut self, id: &str) -> Result<DisableReport, LifecycleError> {
        let mut connect = CliMcpConnect;
        let mut env = connector_env(&mut connect);
        self.session.disable(id, &mut env).await
    }

    async fn connect(&mut self, id: &str) -> Result<ConnectReport, LifecycleError> {
        let mut connect = CliMcpConnect;
        let mut env = connector_env(&mut connect);
        self.session.connect(id, &mut env).await
    }

    async fn disconnect(&mut self, id: &str) -> Result<DisableReport, LifecycleError> {
        let mut connect = CliMcpConnect;
        let mut env = connector_env(&mut connect);
        self.session.disconnect(id, &mut env).await
    }

    async fn auth(&mut self, id: &str) -> Result<AuthOutcome, AuthOutcomeError> {
        // Report the connector's declared requirement and its current state
        // without ever echoing a secret (FR-014, FR-022). Storing a value needs
        // an explicit secret, which the argument list does not carry, so the
        // report states the shape and the guidance instead of prompting.
        let descriptor = self
            .descriptor(id)
            .ok_or_else(|| AuthOutcomeError::UnknownConnector(id.to_string()))?;
        let requirement = ragent_connectors::AuthRequirement::for_descriptor(&descriptor);
        let state = match requirement.shape {
            ragent_connectors::ConnectorAuthShape::None => AuthState::NotRequired,
            _ => AuthState::NeedsAuth,
        };
        Ok(AuthOutcome {
            id: id.to_string(),
            state,
            guidance: requirement.guidance(),
            stored: false,
        })
    }

    async fn search_catalogue(&mut self) -> Result<Vec<ConnectorDescriptor>, String> {
        // The catalogue fetch is blocking, so it runs off the async worker thread
        // on the same spawn_blocking path the `stores --check` probe uses.
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
fn connector_env<'a>(connect: &'a mut CliMcpConnect) -> ragent_connectors::ConnectorEnv<'a> {
    ragent_connectors::ConnectorEnv {
        connect,
        credentials: &NullCredentialStore,
        env: &ProcessEnv,
        server_ledger: &AlwaysEnabled,
    }
}

/// The MCP connect seam for the CLI surface.
///
/// A CLI invocation has no shared MCP client, so no server can be started or
/// stopped in-process: `connect` reports the unknown tool list and `disconnect`
/// is a no-op (the child process, if any, was started by an earlier invocation
/// and is not this process's to stop).
#[derive(Debug)]
struct CliMcpConnect;

#[async_trait]
impl McpConnect for CliMcpConnect {
    async fn connect(
        &mut self,
        _server_id: &str,
        _config: McpServerConfig,
    ) -> Result<Vec<String>, String> {
        // No live client is available for a one-shot invocation, so the
        // advertised tool list is unknown, never a fabricated empty list.
        Ok(Vec::new())
    }

    async fn disconnect(&mut self, _server_id: &str) -> Result<(), String> {
        Ok(())
    }
}

/// The isolated probe seam over a throwaway MCP client (FR-015).
///
/// Mirrors the TUI probe: each connect builds a brand-new, independent
/// `McpClient`, and the harness observes nothing of any live session.
struct CliMcpProbe;

#[async_trait]
impl McpProbe for CliMcpProbe {
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

/// A probe that is never driven: the management subcommands do not invoke a
/// tool, and `test` supplies [`CliMcpProbe`] instead.
struct NeverProbe;

#[async_trait]
impl McpProbe for NeverProbe {
    async fn probe_connect(
        &mut self,
        _server_id: &str,
        _config: McpServerConfig,
    ) -> Result<Vec<ProbeTool>, String> {
        Err("the management subcommands do not connect an isolated probe".to_string())
    }

    async fn probe_call(
        &mut self,
        _server_id: &str,
        _tool: &str,
        _args: serde_json::Value,
    ) -> Result<String, String> {
        Err("the management subcommands do not connect an isolated probe".to_string())
    }

    async fn probe_disconnect(&mut self, _server_id: &str) -> Result<(), String> {
        Ok(())
    }
}

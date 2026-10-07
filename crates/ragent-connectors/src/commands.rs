//! `/connectors` parse-and-run glue shared by the TUI and the CLI (spec
//! `connectors` T-010, T-011; FR-004, FR-006, FR-009, FR-010, FR-012, FR-013,
//! FR-014, FR-018, FR-019, FR-021, FR-022, FR-023, FR-025).
//!
//! This module is the single entry point the command surfaces (the TUI
//! `/connectors` slash family, T-010) and the `ragent connectors` CLI parity
//! (T-014) call. It owns:
//!
//! - the configuration/store resolver ([`store_and_config`]), mirroring
//!   `ragent_plugins::surface::store_and_config`;
//! - the subcommand parser ([`parse_connector_command`]) that recognises every
//!   token of the family, accepts the `--force` flag on `add`, the `--verbose`
//!   flag on `list`, the `--check` flag on `stores`, the `--refresh` flag on
//!   `claude`, and the optional `--category <name>` argument on
//!   `list`/`claude` (FR-041), whose unknown values render the family's `[err]`
//!   row ([`crate::management::CategoryError`]) and change no state. `claude`
//!   is parsed here but served by the owning surface (the TUI opens the browse
//!   panel, the CLI prints the browser pointer), so the parser is the single
//!   place its launch arguments are understood;
//! - the synchronous dispatcher ([`run_connector_subcommand`]) for the sessions
//!   that need no runtime (`help`, `add`, `remove`, `stores`), honouring the
//!   master switch `connectors.enabled` before any work (FR-021);
//! - the asynchronous dispatchers ([`run_connector_subcommand_async`] for the
//!   isolated `test` harness, FR-015, and [`run_connector_subcommand_env`] for
//!   the management subcommands, T-011), which drive a caller-supplied session
//!   environment ([`ConnectorCommandEnv`], implemented by the owning session
//!   over the live MCP client and catalogue fetcher);
//! - the shared report wording for every management subcommand
//!   ([`enable_report`], [`disable_report`], [`connect_report`],
//!   [`disconnect_report`], [`auth_report`], [`lifecycle_error_report`]), so the
//!   TUI and CLI print one wording.
//!
//! Keeping the parse-and-run here means the two surfaces share one wording and
//! one set of guards.

use std::path::Path;

use async_trait::async_trait;
use ragent_config::ConnectorsConfig;

use crate::add::{InstallSource, add, classify_source};
use crate::auth::{AuthState, CredentialStore, EnvSource};
use crate::browse::CategoryFilter;
use crate::descriptor::ConnectorDescriptor;
use crate::fetch::{CatalogueCache, CatalogueLimits, default_fetcher, now_unix_secs};
use crate::harness::{McpProbe, run_harness};
use crate::help::{CONNECTOR_SUBCOMMANDS, attribution, render_help};
use crate::lifecycle::{ConnectReport, ConnectorStatus, DisableReport, LifecycleError};
use crate::management::{ListInput, render_list};
use crate::remove::remove;
use crate::report::{
    add_error_report, add_report, disabled_subsystem_report, remove_error_report, remove_report,
};
use crate::store::{StoreDirs, store_dirs};
use crate::store_index::CatalogueKind;
use crate::store_ops::{fetch_effective_catalogue, stores_report_with_check};

/// A parsed `/connectors` subcommand (FR-004).
///
/// Every token of the family is represented so the parser and the autocomplete
/// surface are complete; the run bodies for the management variants arrive with
/// T-011/T-013.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorCommand {
    /// `list [--verbose] [--category <name>]`.
    List {
        /// Include per-connector details (`--verbose`).
        verbose: bool,
        /// The active category filter (`ALL` when unfiltered; FR-041).
        category: CategoryFilter,
    },
    /// `claude [query] [--category <name>] [--refresh]`.
    Claude {
        /// The pre-filled search query (`""` when none was typed). The category
        /// filter is applied on top of it (FR-041).
        query: String,
        /// The active category filter (`ALL` when unfiltered; FR-041).
        category: CategoryFilter,
        /// Whether the launch asked for a cache-bypassing catalogue re-fetch.
        refresh: bool,
    },
    /// `add <id|source> [--force]`.
    Add {
        /// The source path, URL, or catalogue id to install from.
        source: String,
        /// Overwrite an existing connector with the same id.
        force: bool,
    },
    /// `remove <id>`.
    Remove {
        /// The connector id to uninstall.
        id: String,
    },
    /// `enable <id>`.
    Enable {
        /// The connector id to enable.
        id: String,
    },
    /// `disable <id>`.
    Disable {
        /// The connector id to disable.
        id: String,
    },
    /// `connect <id>`.
    Connect {
        /// The connector id whose servers should connect.
        id: String,
    },
    /// `disconnect <id>`.
    Disconnect {
        /// The connector id whose servers should disconnect.
        id: String,
    },
    /// `auth <id>`.
    Auth {
        /// The connector id whose credential is managed.
        id: String,
    },
    /// `test <id>`.
    Test {
        /// The connector id to test in isolation.
        id: String,
    },
    /// `stores [--check]`.
    Stores {
        /// Probe each catalogue endpoint (`--check`).
        check: bool,
    },
}

/// Why a `/connectors` subcommand could not be parsed (malformed arguments -
/// reported as an `[err]` row that changes no state, per the SPEC error policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorArgError {
    /// `add` was given no source.
    MissingAddSource,
    /// A single-id subcommand (`remove`, `enable`, `disable`, `connect`,
    /// `disconnect`, `auth`, `test`) was given no connector id.
    MissingId,
}

impl ConnectorArgError {
    /// Render the usage error for the offending subcommand (ASCII only, FR-006).
    #[must_use]
    pub fn report(self, sub: &str) -> String {
        let usage = match self {
            Self::MissingAddSource => {
                "Usage: `/connectors add <id|source> [--force]`\n\
                 Source forms: a catalogue id, a local directory, a local `.zip`/`.tar.gz` \
                 file, or an `https://` URL ending in `.zip`/`.tar.gz`."
            }
            Self::MissingId => "Usage: `/connectors <id>`",
        };
        format!("{}\n\n[err] Missing argument.\n\n{usage}", attribution(sub))
    }
}

/// Resolve the connector store directories and configuration from the live
/// environment, honouring the `connectors.store_dir` override (FR-007).
///
/// `workdir` is the project working directory the store is resolved against. A
/// config that cannot be read falls back to the compiled defaults (matching the
/// rest of the codebase's tolerant config loading); the failure is logged rather
/// than silently discarded so a malformed `ragent.json` is visible.
#[must_use]
pub fn store_and_config(workdir: &Path) -> (StoreDirs, ConnectorsConfig) {
    let config = match ragent_config::Config::load() {
        Ok(config) => config.connectors.unwrap_or_default(),
        Err(error) => {
            tracing::warn!(
                error = %error,
                "connector config could not be loaded; using compiled defaults"
            );
            ConnectorsConfig::default()
        }
    };
    let dirs = store_dirs(workdir, config.store_dir.as_deref());
    (dirs, config)
}

/// Whether `sub` is a recognised `/connectors` subcommand token (FR-004).
#[must_use]
pub fn is_known_subcommand(sub: &str) -> bool {
    CONNECTOR_SUBCOMMANDS.contains(&sub)
}

/// The optional `--category <name>` argument on `list`/`claude` (FR-041).
///
/// The flag is accepted anywhere in the argument list and consumes the next
/// whitespace-delimited token as its value; a missing value degrades to the
/// unfiltered `ALL` selection so a malformed flag never filters everything out.
/// Returns the parsed filter and the remaining tokens with the flag (and its
/// value) removed.
fn take_category_filter(tokens: &[&str]) -> (CategoryFilter, Vec<String>) {
    let mut filter = CategoryFilter::all();
    let mut remaining = Vec::new();
    let mut iter = tokens.iter();
    while let Some(&token) = iter.next() {
        if token == "--category" {
            if let Some(name) = iter.next() {
                filter = CategoryFilter::parse(name);
            }
        } else {
            remaining.push(token.to_string());
        }
    }
    (filter, remaining)
}

/// Parse `/connectors <sub> <args>` into a typed [`ConnectorCommand`] (FR-004).
///
/// Returns `None` when `sub` is not a recognised subcommand token (the caller
/// renders the usage block). Returns `Some(Err(_))` for a recognised subcommand
/// with malformed arguments, and `Some(Ok(_))` for a valid command.
#[must_use]
pub fn parse_connector_command(
    sub: &str,
    args: &str,
) -> Option<Result<ConnectorCommand, ConnectorArgError>> {
    let tokens: Vec<&str> = args.split_whitespace().collect();
    match sub {
        "list" => {
            let verbose = tokens.contains(&"--verbose");
            let (category, _) = take_category_filter(&tokens);
            Some(Ok(ConnectorCommand::List { verbose, category }))
        }
        "claude" => {
            let (category, remaining) = take_category_filter(&tokens);
            let mut refresh = false;
            let mut query_tokens: Vec<&str> = Vec::new();
            for token in &remaining {
                if *token == "--refresh" {
                    refresh = true;
                } else {
                    query_tokens.push(token);
                }
            }
            Some(Ok(ConnectorCommand::Claude {
                query: query_tokens.join(" "),
                category,
                refresh,
            }))
        }
        "add" => {
            let mut force = false;
            let mut source_tokens: Vec<&str> = Vec::new();
            for token in &tokens {
                if *token == "--force" {
                    force = true;
                } else {
                    source_tokens.push(token);
                }
            }
            if source_tokens.is_empty() {
                return Some(Err(ConnectorArgError::MissingAddSource));
            }
            Some(Ok(ConnectorCommand::Add {
                source: source_tokens.join(" "),
                force,
            }))
        }
        "remove" => id_command(&tokens, |id| ConnectorCommand::Remove { id }),
        "enable" => id_command(&tokens, |id| ConnectorCommand::Enable { id }),
        "disable" => id_command(&tokens, |id| ConnectorCommand::Disable { id }),
        "connect" => id_command(&tokens, |id| ConnectorCommand::Connect { id }),
        "disconnect" => id_command(&tokens, |id| ConnectorCommand::Disconnect { id }),
        "auth" => id_command(&tokens, |id| ConnectorCommand::Auth { id }),
        "test" => id_command(&tokens, |id| ConnectorCommand::Test { id }),
        "stores" => Some(Ok(ConnectorCommand::Stores {
            check: tokens.contains(&"--check"),
        })),
        _ => None,
    }
}

/// Parse a subcommand whose only argument is a connector id: the first token, or
/// [`ConnectorArgError::MissingId`].
fn id_command(
    tokens: &[&str],
    make: impl FnOnce(String) -> ConnectorCommand,
) -> Option<Result<ConnectorCommand, ConnectorArgError>> {
    Some(
        first_id(tokens)
            .map(make)
            .ok_or(ConnectorArgError::MissingId),
    )
}

/// The connector reference the single-reference subcommands act on: every token
/// joined by a space, or `None` when there are none.
///
/// Joining the tokens lets a multi-word display name (`Microsoft Learn`) be
/// quoted and passed as one shell word without the whitespace being split off;
/// an unquoted multi-word reference is joined the same way, so `enable Microsoft
/// Learn` resolves `Microsoft Learn`.
fn first_id(tokens: &[&str]) -> Option<String> {
    if tokens.is_empty() {
        return None;
    }
    Some(tokens.join(" "))
}

/// Run a `/connectors <sub> <rest>` invocation and return the report to print,
/// or `None` when the subcommand has no handler on this surface (the caller
/// renders the usage block).
///
/// The master switch `connectors.enabled` is honoured first: while it is false
/// every recognised subcommand other than `help` reports that the subsystem is
/// disabled and performs no discovery or fetch (FR-021).
///
/// `add` installs from a catalogue id (resolved through one bounded catalogue
/// fetch) or from a local source (no network access), recording the connector
/// disabled (FR-011). `remove` refuses while the connector is enabled (FR-030).
/// `stores` renders the endpoint report, probing only when `--check` is present
/// (FR-036, NFR-003).
///
/// The management subcommands (`list`, `search`, `enable`, `disable`, `connect`,
/// `disconnect`, `auth`) return `None` here: they drive a session or a catalogue
/// fetch, so they are served by the async dispatcher
/// [`run_connector_subcommand_env`] instead. `test` is likewise served there.
#[must_use]
pub fn run_connector_subcommand(workdir: &Path, sub: &str, rest: &str) -> Option<String> {
    // `help` is a pure usage render with no store or config access (FR-017).
    if sub == "help" {
        return Some(render_help(sub));
    }

    let (dirs, config) = store_and_config(workdir);

    // The master switch is consulted before any other work (FR-021).
    if is_known_subcommand(sub) && !config.is_enabled() {
        return Some(disabled_subsystem_report(sub));
    }

    let parsed = parse_connector_command(sub, rest)?;
    let report = match parsed {
        Ok(ConnectorCommand::Add { source, force }) => {
            run_add(&dirs, workdir, &config, &source, force)
        }
        Ok(ConnectorCommand::Remove { id }) => match remove(&dirs, &id) {
            Ok(outcome) => remove_report(&outcome),
            Err(err) => remove_error_report(&err),
        },
        Ok(ConnectorCommand::Stores { check }) => {
            let args = if check { "--check" } else { "" };
            stores_report_with_check(&config, default_fetcher().as_ref(), args)
        }
        // The management subcommands drive a session or a catalogue fetch and
        // are served by `run_connector_subcommand_env`; `claude` opens a panel
        // and is served by the owning surface. Returning `None` lets the caller
        // fall back to the usage block on a pure-sync surface.
        Ok(
            ConnectorCommand::List { .. }
            | ConnectorCommand::Claude { .. }
            | ConnectorCommand::Enable { .. }
            | ConnectorCommand::Disable { .. }
            | ConnectorCommand::Connect { .. }
            | ConnectorCommand::Disconnect { .. }
            | ConnectorCommand::Auth { .. }
            | ConnectorCommand::Test { .. },
        ) => return None,
        Err(arg_err) => arg_err.report(sub),
    };
    Some(report)
}

/// Run a `/connectors <sub> <rest>` invocation whose body needs the isolated
/// MCP harness or a blocking catalogue probe, returning the report to print or
/// `None` when the subcommand has no async handler on this surface (FR-015,
/// FR-021, FR-036).
///
/// `stores --check` is served here because it performs a *blocking* network
/// fetch ([`NetworkCatalogueFetcher`](crate::fetch::NetworkCatalogueFetcher)):
/// `reqwest::blocking` builds and drops its own tokio runtime, and dropping a
/// runtime from inside an async context panics ("Cannot drop a runtime in a
/// context where blocking is not allowed"). Running the probe on
/// [`tokio::task::spawn_blocking`] keeps it off the runtime's worker thread so
/// the event loop keeps animating and the probe cannot abort the process. The
/// `stores` report without `--check` stays pure and is handled by
/// [`run_connector_subcommand`].
pub async fn run_connector_subcommand_stores_check(config: &ConnectorsConfig) -> String {
    let connectors = config.clone();
    tokio::task::spawn_blocking(move || {
        stores_report_with_check(&connectors, default_fetcher().as_ref(), "--check")
    })
    .await
    .unwrap_or_else(|err| {
        format!(
            "{}\n\n[err] the catalogue probe did not run: {err}",
            attribution("stores")
        )
    })
}

/// Run a `/connectors <sub> <rest>` invocation whose body needs the isolated
/// MCP harness, returning the report to print or `None` when the subcommand has
/// no async handler on this surface (FR-015, FR-021).
///
/// This is the dispatcher the TUI and the CLI parity surface drive: `test <id>`
/// connects the connector's servers in isolation, invokes one advertised tool
/// once, and tears the connection down again (FR-015). The master switch is
/// honoured first, so a disabled subsystem refuses the harness without
/// attempting any connection (FR-021); a malformed argument renders the `[err]`
/// report.
///
/// The caller supplies the isolated [`McpProbe`] the harness drives, the
/// encrypted credential store, and the environment source, so the harness
/// borrows no live session (FR-015).
pub async fn run_connector_subcommand_async(
    workdir: &Path,
    config: &ConnectorsConfig,
    credentials: &dyn CredentialStore,
    env: &dyn EnvSource,
    probe: &mut dyn McpProbe,
    sub: &str,
    rest: &str,
) -> Option<String> {
    // The async dispatcher owns the test harness and the management subcommands;
    // `help`, `add`, `remove`, and `stores` are served by
    // `run_connector_subcommand`.
    match sub {
        "test" => {}
        "list" | "claude" | "enable" | "disable" | "connect" | "disconnect" | "auth" => {
            // The management subcommands need a session lifecycle; the
            // session-aware entry point is `run_connector_subcommand_env`.
            // `claude` opens a browse panel owned by the calling surface.
            return None;
        }
        _ => return None,
    }
    match parse_connector_command(sub, rest) {
        Some(Ok(ConnectorCommand::Test { id })) => {
            let resolved = resolve_connector_reference(workdir, &id);
            match run_harness(workdir, config, credentials, env, probe, &resolved).await {
                Ok(report) => Some(report),
                Err(err) => Some(err.report(&resolved)),
            }
        }
        Some(Err(arg_err)) => Some(arg_err.report(sub)),
        // Unreachable: `sub == "test"` always parses to a `Test`.
        Some(Ok(_)) | None => None,
    }
}

/// Resolve a user-supplied connector reference (identifier, slug, or display
/// name) to the installed connector's canonical id.
///
/// `/connectors <verb> <ref>` accepts a connector's id, its slug, or its
/// display name, so a connector is addressable without knowing its store
/// directory name. Resolution is a single store scan shared by the harness and
/// the lifecycle subcommands; an unresolved reference is returned unchanged so
/// the downstream code reports it as an unknown connector under the name the
/// user typed.
#[must_use]
pub fn resolve_connector_reference(workdir: &Path, reference: &str) -> String {
    let (dirs, _config) = store_and_config(workdir);
    canonical_id(&dirs, reference)
}

/// Resolve a connector reference (identifier, slug, or display name) to the
/// installed connector's canonical id against `dirs` (FR-001).
///
/// This is the store-only half of [`resolve_connector_reference`], for callers
/// that already hold the store dirs (the session environments and the CLI
/// lifecycle seam) and should not perform a second config load. An unresolved
/// reference is returned unchanged so the downstream code reports it as an
/// unknown connector under the name the user typed.
#[must_use]
pub fn canonical_id(dirs: &crate::store::StoreDirs, reference: &str) -> String {
    match crate::store::descriptor_by_id(dirs, reference) {
        Some(descriptor) => descriptor.id.as_str().to_string(),
        None => reference.to_string(),
    }
}

/// The seam through which the management subcommands reach a live session's
/// MCP lifecycle and the connector catalogue (FR-009, FR-010, FR-012, FR-013,
/// FR-018, FR-019, FR-022).
///
/// The command glue in this module owns the *wording* of every management
/// report; the owning session supplies the runtime operations. Splitting the
/// session out lets the exact same dispatcher serve the TUI and the
/// `ragent connectors` CLI parity (T-014) while keeping this crate free of any
/// dependency on `ragent-agent`.
#[async_trait]
pub trait ConnectorCommandEnv {
    /// The `connectors` configuration the master switch is read from (FR-021).
    fn config(&self) -> &ConnectorsConfig;

    /// The store directories the `list` scan reads (FR-001).
    fn dirs(&self) -> &StoreDirs;

    /// The live per-connector status snapshot, or an empty vector when no
    /// session has loaded the connectors (FR-009).
    ///
    /// A surface whose connectors were connected by the shared client (the
    /// binary's startup bridge) rather than by a [`crate::ConnectorSession::start`]
    /// call overrides this to derive the snapshot from the client's per-server
    /// state, so `list` reports `connected` instead of the store enable flag.
    fn statuses(&self) -> Vec<ConnectorStatus>;

    /// The live advertised tool count per bridged server id
    /// (`<connector-id>.<server>`), or an empty map when no live client is
    /// reachable (FR-009).
    ///
    /// A count is the tool surface a bridged server actually exposes, which the
    /// lifecycle state alone cannot express: a server that is `connected` but
    /// advertises nothing is distinguishable from one whose client has not been
    /// read yet. The default implementation reports no counts, so a surface
    /// without a live client renders `?` rather than a misleading `0`.
    fn tool_counts(&self) -> std::collections::BTreeMap<String, usize> {
        std::collections::BTreeMap::new()
    }

    /// Enable a connector and connect its servers now (FR-012).
    ///
    /// # Errors
    ///
    /// Returns the refusal/failure cause when the connector is unknown or its
    /// enable ledger cannot be written.
    async fn enable(&mut self, id: &str) -> Result<ConnectReport, LifecycleError>;

    /// Disable a connector, disconnect its servers, and deregister their tools
    /// (FR-013).
    ///
    /// # Errors
    ///
    /// Returns the refusal/failure cause when the connector is unknown or its
    /// ledger cannot be written.
    async fn disable(&mut self, id: &str) -> Result<DisableReport, LifecycleError>;

    /// Connect an enabled connector's servers without a session restart
    /// (FR-019).
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::Disabled`] while the connector is disabled, or
    /// when the connector is unknown.
    async fn connect(&mut self, id: &str) -> Result<ConnectReport, LifecycleError>;

    /// Disconnect a connector's servers while leaving it enabled (FR-019).
    ///
    /// # Errors
    ///
    /// Returns the refusal/failure cause when the connector is unknown.
    async fn disconnect(&mut self, id: &str) -> Result<DisableReport, LifecycleError>;

    /// Manage a connector's credential and report its resulting auth state
    /// (FR-014, FR-022). The secret value is never passed back.
    ///
    /// # Errors
    ///
    /// Returns the refusal/failure cause when the connector is unknown or the
    /// credential store cannot be written.
    async fn auth(&mut self, id: &str) -> Result<AuthOutcome, AuthOutcomeError>;
}

/// The result of `/connectors auth <id>` (FR-014, FR-022).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthOutcome {
    /// The connector id.
    pub id: String,
    /// The resulting authentication state (never a secret value).
    pub state: AuthState,
    /// Human-readable guidance for the shape, shown when no action was possible
    /// (FR-014, FR-022).
    pub guidance: String,
    /// Whether a secret was actually written by this invocation.
    pub stored: bool,
}

/// Why `/connectors auth <id>` could not run (FR-014, FR-022).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthOutcomeError {
    /// No store holds the connector id.
    #[error("connector auth: unknown connector id {0}")]
    UnknownConnector(String),
    /// The credential store rejected the write.
    #[error("connector auth: {0}")]
    Store(String),
}

/// Run the async `/connectors <sub> <rest>` subcommands, returning the report to
/// print or `None` when the subcommand has no handler on this surface (FR-009,
/// FR-010, FR-012, FR-013, FR-014, FR-018, FR-019, FR-021, FR-022, FR-025).
///
/// This is the async twin of [`run_connector_subcommand`]: the management
/// subcommands (`list`, `search`, `enable`, `disable`, `connect`, `disconnect`,
/// `auth`) are served here, while the pure store subcommands (`add`, `remove`,
/// `stores`, `help`) continue to be served by the synchronous entry point and
/// the isolated harness (`test`) by [`run_connector_subcommand_async`]. The
/// caller supplies the session environment, so the dispatcher borrows no live
/// client of its own.
///
/// The master switch is honoured first: a disabled subsystem reports that fact
/// for every subcommand other than `help` and attempts no connection or fetch
/// (FR-021).
pub async fn run_connector_subcommand_env(
    env: &mut (impl ConnectorCommandEnv + Send + ?Sized),
    sub: &str,
    rest: &str,
) -> Option<String> {
    // `help` is a pure usage render with no store or config access (FR-017).
    if sub == "help" {
        return Some(render_help(sub));
    }
    if !is_known_subcommand(sub) {
        return None;
    }
    if !env.config().is_enabled() {
        return Some(disabled_subsystem_report(sub));
    }

    let parsed = match parse_connector_command(sub, rest) {
        Some(Ok(command)) => command,
        Some(Err(arg_err)) => return Some(arg_err.report(sub)),
        // A token the parser does not recognise has no handler here.
        None => return None,
    };

    let report = match parsed {
        ConnectorCommand::List { verbose, category } => {
            let statuses = env.statuses();
            let tool_counts = env.tool_counts();
            let input = ListInput {
                dirs: env.dirs(),
                statuses: &statuses,
                tool_counts: &tool_counts,
            };
            match render_list(&input, &category, verbose) {
                Ok(report) => report,
                Err(err) => err.report("list"),
            }
        }
        ConnectorCommand::Enable { id } => {
            let id = canonical_id(env.dirs(), &id);
            match env.enable(&id).await {
                Ok(outcome) => enable_report(&outcome),
                Err(err) => lifecycle_error_report("enable", &id, &err),
            }
        }
        ConnectorCommand::Disable { id } => {
            let id = canonical_id(env.dirs(), &id);
            match env.disable(&id).await {
                Ok(outcome) => disable_report(&outcome),
                Err(err) => lifecycle_error_report("disable", &id, &err),
            }
        }
        ConnectorCommand::Connect { id } => {
            let id = canonical_id(env.dirs(), &id);
            match env.connect(&id).await {
                Ok(outcome) => connect_report(&outcome),
                Err(err) => lifecycle_error_report("connect", &id, &err),
            }
        }
        ConnectorCommand::Disconnect { id } => {
            let id = canonical_id(env.dirs(), &id);
            match env.disconnect(&id).await {
                Ok(outcome) => disconnect_report(&outcome),
                Err(err) => lifecycle_error_report("disconnect", &id, &err),
            }
        }
        ConnectorCommand::Auth { id } => match env.auth(&id).await {
            Ok(outcome) => auth_report(&outcome),
            Err(err) => format!("{}\n\n[err] {err}", attribution(&format!("auth {id}"))),
        },
        // The `test` harness is served by `run_connector_subcommand_async`,
        // which owns the isolated probe; reaching it here means the caller
        // skipped that entry point, so it has no handler on this surface.
        ConnectorCommand::Test { .. } => return None,
        // `stores --check` probes each catalogue over a blocking HTTP client,
        // which must run off the async worker thread (see
        // `run_connector_subcommand_stores_check`). A plain `stores` performs no
        // I/O and is served by `run_connector_subcommand`.
        ConnectorCommand::Stores { check: true } => {
            run_connector_subcommand_stores_check(env.config()).await
        }
        // The remaining store subcommands are served by `run_connector_subcommand`,
        // and `claude` opens a browse panel owned by the calling surface.
        ConnectorCommand::Add { .. }
        | ConnectorCommand::Remove { .. }
        | ConnectorCommand::Claude { .. }
        | ConnectorCommand::Stores { check: false } => return None,
    };
    Some(report)
}

/// Fetch the catalogue descriptors a `search` query filters (FR-010).
///
/// Fetches every [`CatalogueKind`] under its resolved endpoint and returns the
/// union of the descriptors that survived normalisation, together with a short
/// cause describing the first failure, so every surface searches one catalogue
/// with one wording.
///
/// A fetch failure is non-fatal when a later store succeeds; the returned cause
/// names the first failure so an empty union still reports why it is empty
/// (FR-010).
///
/// # Errors
///
/// Returns a short ASCII cause when no catalogue could be fetched.
pub fn fetch_catalogue_descriptors_network(
    config: &ConnectorsConfig,
) -> Result<Vec<ConnectorDescriptor>, String> {
    fetch_catalogue_descriptors_with_skipped(config).map(|(descriptors, _skipped)| descriptors)
}

/// [`fetch_catalogue_descriptors_network`] with the parser's skip count.
///
/// The browse panel reports a short list as a partial result rather than a
/// silent one, so it needs the number of entries the provider dropped as
/// malformed or unexpressible alongside the descriptors that survived.
///
/// # Errors
///
/// Returns a short ASCII cause when no catalogue could be fetched.
pub fn fetch_catalogue_descriptors_with_skipped(
    config: &ConnectorsConfig,
) -> Result<(Vec<ConnectorDescriptor>, usize), String> {
    let limits = CatalogueLimits::from(&config.stores_or_default());
    let cache = CatalogueCache::default_cache();
    let now = now_unix_secs();
    let mut descriptors = Vec::new();
    let mut skipped = 0;
    let mut first_failure: Option<String> = None;
    for kind in CatalogueKind::ALL {
        match fetch_effective_catalogue(
            default_fetcher().as_ref(),
            kind,
            config,
            &limits,
            cache.as_ref(),
            now,
        ) {
            Ok(fetched) => {
                descriptors.extend(fetched.catalogue.connectors);
                skipped += fetched.catalogue.skipped;
            }
            Err(error) => {
                tracing::warn!(
                    catalogue = kind.token(),
                    error = %error,
                    "connector catalogue fetch failed during search"
                );
                first_failure.get_or_insert_with(|| error.to_string());
            }
        }
    }
    match (descriptors.is_empty(), first_failure) {
        (true, Some(cause)) => Err(cause),
        _ => Ok((descriptors, skipped)),
    }
}

/// Fetch the catalogue descriptors a catalogue-browser
/// query filters, excluding connectors already installed in `dirs` (FR-010).
///
/// A catalogue entry's id is the connector id, so an entry whose id is already
/// present in the store cannot be installed (the install refuses the
/// collision); dropping such entries at the source means the search report
/// lists what can still be added, and the browse panel never offers a row that
/// would be refused. An entry installed under a different on-disk id (a
/// pre-existing UUID install) is not matched and stays listed.
///
/// # Errors
///
/// Returns a short ASCII cause when no catalogue could be fetched.
pub fn search_installable_catalogue(
    config: &ConnectorsConfig,
    dirs: &crate::store::StoreDirs,
) -> Result<Vec<ConnectorDescriptor>, String> {
    search_installable_catalogue_with_skipped(config, dirs)
        .map(|(descriptors, _skipped)| descriptors)
}

/// [`search_installable_catalogue`] with the parser's skip count.
///
/// The browse panel reports a short list as a partial result rather than a
/// silent one, so it needs the number of entries the provider dropped as
/// malformed or unexpressible *plus* the already-installed entries omitted.
///
/// # Errors
///
/// Returns a short ASCII cause when no catalogue could be fetched.
pub fn search_installable_catalogue_with_skipped(
    config: &ConnectorsConfig,
    dirs: &crate::store::StoreDirs,
) -> Result<(Vec<ConnectorDescriptor>, usize), String> {
    let (descriptors, skipped) = fetch_catalogue_descriptors_with_skipped(config)?;
    let installed: std::collections::BTreeSet<String> = crate::store::scan_dirs(dirs.clone())
        .into_iter()
        .filter_map(|connector| connector.outcome.ok())
        .map(|descriptor| descriptor.id.as_str().to_string())
        .collect();
    let before = descriptors.len();
    let descriptors: Vec<ConnectorDescriptor> = descriptors
        .into_iter()
        .filter(|descriptor| !installed.contains(descriptor.id.as_str()))
        .collect();
    let omitted = before.saturating_sub(descriptors.len());
    Ok((descriptors, skipped + omitted))
}

/// Render the `[err]` report for a refused lifecycle transition (FR-016).
#[must_use]
pub fn lifecycle_error_report(sub: &str, id: &str, error: &LifecycleError) -> String {
    format!("{}\n\n[err] {error}", attribution(&format!("{sub} {id}")))
}

/// The shared body of the `enable`/`connect` success reports: they differ only in
/// the attribution sub and the leading state verb.
fn connect_like_report(kind: &str, verb: &str, outcome: &ConnectReport) -> String {
    let mut lines = vec![
        attribution(&format!("{kind} {}", outcome.id)),
        String::new(),
        state_line(verb, outcome),
    ];
    lines.extend(server_lines(outcome));
    if let Some(error) = &outcome.error {
        lines.push(format!("[err] {}", error));
    }
    lines.extend(refused_lines(outcome));
    lines.join("\n")
}

/// Render the `/connectors enable` success report (FR-012).
#[must_use]
pub fn enable_report(outcome: &ConnectReport) -> String {
    connect_like_report("enable", "Enabled", outcome)
}

/// Render the `/connectors connect` success report (FR-019).
#[must_use]
pub fn connect_report(outcome: &ConnectReport) -> String {
    connect_like_report("connect", "Connected", outcome)
}

/// Render the `/connectors disable` success report (FR-013), confirming exactly
/// how many servers and tools were dropped.
#[must_use]
pub fn disable_report(outcome: &DisableReport) -> String {
    format!(
        "{}\n\n[ok] Connector `{id}` is now disabled and its servers disconnected; \
         disconnected {servers} server(s) and deregistered {tools} tool(s).",
        attribution(&format!("disable {}", outcome.id)),
        id = outcome.id,
        servers = outcome.servers_disconnected,
        tools = outcome.tools_deregistered,
    )
}

/// Render the `/connectors disconnect` success report (FR-019), leaving the
/// connector enabled.
#[must_use]
pub fn disconnect_report(outcome: &DisableReport) -> String {
    format!(
        "{}\n\n[ok] Connector `{id}` is disconnected but still enabled; \
         dropped {servers} server(s) and {tools} tool(s).",
        attribution(&format!("disconnect {}", outcome.id)),
        id = outcome.id,
        servers = outcome.servers_disconnected,
        tools = outcome.tools_deregistered,
    )
}

/// Render the `/connectors auth <id>` report (FR-014, FR-022).
///
/// The secret value is never echoed: the report states the shape, the resulting
/// state label, and the guidance for the shape.
#[must_use]
pub fn auth_report(outcome: &AuthOutcome) -> String {
    let mut lines = vec![attribution(&format!("auth {}", outcome.id)), String::new()];
    if outcome.stored {
        lines.push(format!(
            "[ok] Connector `{}`: credential stored; auth state {}.",
            outcome.id,
            outcome.state.label()
        ));
    } else {
        lines.push(format!(
            "[ok] Connector `{}`: auth state {}.",
            outcome.id,
            outcome.state.label()
        ));
    }
    if !outcome.guidance.is_empty() {
        lines.push(outcome.guidance.clone());
    }
    lines.join("\n")
}

/// The one-line state sentence shared by the `enable` and `connect` reports.
fn state_line(verb: &str, outcome: &ConnectReport) -> String {
    format!(
        "[ok] {verb} connector `{id}`: state {state}, auth {auth}, {servers} server(s) connected, \
         {tools} tool(s) exposed.",
        id = outcome.id,
        state = outcome.state.label(),
        auth = outcome.auth.label(),
        servers = outcome.connected_servers(),
        tools = outcome.tools(),
    )
}

/// One line per bridged server, reporting each server's state independently
/// (FR-026).
fn server_lines(outcome: &ConnectReport) -> Vec<String> {
    outcome
        .servers
        .iter()
        .map(|server| match &server.error {
            Some(cause) => format!(
                "- {id}: [err] {state} - {cause}",
                id = server.server_id,
                state = server.state.label()
            ),
            None => format!(
                "- {id}: {state}, {} tool(s)",
                server.tools.len(),
                id = server.server_id,
                state = server.state.label()
            ),
        })
        .collect()
}

/// The refused-server lines for a collision guard report (FR-033).
fn refused_lines(outcome: &ConnectReport) -> Vec<String> {
    outcome
        .refused
        .iter()
        .map(|refused| format!("[err] Refused server: {}", refused.describe()))
        .collect()
}

/// Install a connector from a catalogue id or a local/URL source (FR-011).
///
/// A catalogue id is resolved against the configured catalogue (one bounded
/// fetch); a local directory, archive, or `https://` URL stages with no network
/// access and no catalogue. A refusal renders the `[err]` report.
fn run_add(
    dirs: &StoreDirs,
    workdir: &Path,
    config: &ConnectorsConfig,
    source: &str,
    force: bool,
) -> String {
    let catalogue = match classify_source(source, workdir) {
        InstallSource::CatalogueId(_) => fetch_catalogue_descriptors(config),
        InstallSource::Local(_) => Vec::new(),
    };
    match add(dirs, workdir, source, force, &catalogue) {
        Ok(outcome) => add_report(&outcome),
        Err(err) => add_error_report(&err),
    }
}

/// Fetch the configured catalogue's descriptors for a catalogue-id install
/// (FR-024). A fetch failure or a refused endpoint yields an empty catalogue, so
/// the install is refused as `catalogue holds no connector with id ...` rather
/// than silently succeeding; the fetch failure is logged by the shared fetch.
fn fetch_catalogue_descriptors(config: &ConnectorsConfig) -> Vec<ConnectorDescriptor> {
    fetch_catalogue_descriptors_network(config).unwrap_or_default()
}

//! `/plugins list`, `/plugins enable`, and `/plugins disable` command family
//! (spec `plugins` T-013; FR-009, FR-011, FR-012, FR-016, FR-022, FR-025).
//!
//! This module is the single entry point the command surfaces (the TUI
//! `/plugins` slash family, T-014) and CLI parity (T-017) call for the three
//! non-store subcommands:
//!
//! - [`render_list`] renders one row per discovered plugin (id, name, version,
//!   dialect, state, contributed tool/command/skill/agent/hook names and
//!   counts), a summary line, any unsupported-capability notices (FR-025), and
//!   — in verbose mode — the per-plugin telemetry counters (FR-009, FR-022). It
//!   scans manifests only and executes no plugin code (FR-023).
//! - [`run_control_command`] parses the raw argument text, drives
//!   [`PluginSession::enable`] / [`PluginSession::disable`] (which update the
//!   ledger, load/unload the sandbox, and register/deregister contributions via
//!   the caller's [`PluginSurface`]), and returns the report string.
//!
//! Keeping the parse-and-run here means the TUI and the CLI share one wording
//! and one set of guards, mirroring the store-command glue in
//! [`crate::commands`].

use std::path::PathBuf;

use crate::bridge::plugin_skill_names;
use crate::help::attribution;
use crate::lifecycle::PluginManager;
use crate::session::{DisableOutcome, EnableOutcome, PluginSession, PluginSurface};
use crate::store::{LifecycleState, ScannedPlugin, StoreLedger};
use crate::tool_adapter::plugin_tool_name;

/// A parsed `/plugins` control subcommand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlCommand {
    /// `list [--verbose]`.
    List {
        /// Include per-plugin telemetry counters (FR-022).
        verbose: bool,
    },
    /// `enable <pluginid>`.
    Enable {
        /// The plugin id to enable.
        plugin_id: String,
    },
    /// `disable <pluginid>`.
    Disable {
        /// The plugin id to disable.
        plugin_id: String,
    },
}

/// Why a control subcommand could not be parsed (malformed arguments — reported
/// as an `[err]` row that changes no state, per the SPEC error policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlArgError {
    /// `enable` was given no plugin id.
    MissingEnableId,
    /// `disable` was given no plugin id.
    MissingDisableId,
}

impl ControlArgError {
    /// Render the usage error for the offending subcommand.
    #[must_use]
    pub fn report(self, sub: &str) -> String {
        let (id, what) = match self {
            Self::MissingEnableId => ("enable", "<pluginid>"),
            Self::MissingDisableId => ("disable", "<pluginid>"),
        };
        format!(
            "{}\n\n[err] Missing {what}.\n\nUsage: `/plugins {id} {what}`",
            attribution(sub)
        )
    }
}

/// Parse `/plugins <sub> <args>` for the three control subcommands.
///
/// Returns `None` when `sub` is none of `list`/`enable`/`disable` (the caller
/// falls through to the other subcommands), `Some(Err(_))` for malformed
/// arguments, and `Some(Ok(_))` for a valid command. `list` accepts `--verbose`
/// (alias `-v`) anywhere; `enable`/`disable` take the first token as the plugin
/// id.
#[must_use]
pub fn parse_control_command(
    sub: &str,
    args: &str,
) -> Option<Result<ControlCommand, ControlArgError>> {
    match sub {
        "list" => {
            let verbose = args
                .split_whitespace()
                .any(|token| token == "--verbose" || token == "-v");
            Some(Ok(ControlCommand::List { verbose }))
        }
        "enable" => Some(match args.split_whitespace().next() {
            Some(id) => Ok(ControlCommand::Enable {
                plugin_id: id.to_string(),
            }),
            None => Err(ControlArgError::MissingEnableId),
        }),
        "disable" => Some(match args.split_whitespace().next() {
            Some(id) => Ok(ControlCommand::Disable {
                plugin_id: id.to_string(),
            }),
            None => Err(ControlArgError::MissingDisableId),
        }),
        _ => None,
    }
}

/// Parse and dispatch one control subcommand into `session` (FR-009, FR-011,
/// FR-012). Returns `None` for a non-control subcommand so the caller can try
/// the next family; refusals and lifecycle errors render as `[err]` text rather
/// than a returned `Err`, so the surfaces never have to branch on failure.
///
/// When the master switch `plugins.enabled` is false the subsystem is inert: no
/// discovery runs and the subcommand reports the disabled subsystem (SPEC
/// configuration schema; acceptance criterion 8).
pub fn run_control_command(
    session: &mut PluginSession,
    surface: &mut impl PluginSurface,
    sub: &str,
    args: &str,
) -> Option<String> {
    let parsed = parse_control_command(sub, args)?;
    if !session.manager().config().is_enabled() {
        return Some(disabled_subsystem_report(sub));
    }
    let report = match parsed {
        Ok(ControlCommand::List { verbose }) => render_list(session.manager(), verbose),
        Ok(ControlCommand::Enable { plugin_id }) => match session.enable(&plugin_id, surface) {
            Ok(outcome) => enable_report(&outcome),
            Err(err) => enable_error_report(&plugin_id, &err),
        },
        Ok(ControlCommand::Disable { plugin_id }) => match session.disable(&plugin_id, surface) {
            Ok(outcome) => disable_report(&outcome),
            Err(err) => disable_error_report(&plugin_id, &err),
        },
        Err(arg_err) => arg_err.report(sub),
    };
    Some(report)
}

/// The `[err]` report shown by a control subcommand while the master switch
/// `plugins.enabled` is false (SPEC configuration schema; acceptance criterion
/// 8). No discovery runs and no plugin code executes.
#[must_use]
pub fn disabled_subsystem_report(sub: &str) -> String {
    format!(
        "{}\n\n[err] Plugin subsystem is disabled \
         (plugins.enabled = false); no plugins were discovered.",
        attribution(sub)
    )
}

// ── `/plugins list` (FR-009, FR-022, FR-025) ─────────────────────────────────

/// One rendered `/plugins list` row.
struct Row {
    id: String,
    name: String,
    version: String,
    dialect: String,
    state: LifecycleState,
    tools: Vec<String>,
    commands: Vec<String>,
    /// Skill names contributed by the declared `skills` sections (FR-029).
    skills: Vec<String>,
    /// Declared agent-profile paths (FR-032), shown in the contributions block.
    agents: Vec<String>,
    /// Declared hook trigger names (FR-033).
    hooks: Vec<String>,
    unsupported: Vec<String>,
    error: Option<String>,
    store: PathBuf,
}

/// Render the `/plugins list` table (FR-009). Discovery parses manifests only
/// and executes no plugin code (FR-023); `verbose` appends per-plugin telemetry
/// counters (FR-022).
#[must_use]
pub fn render_list(manager: &PluginManager, verbose: bool) -> String {
    let scanned = manager.discover();
    if scanned.is_empty() {
        return format!("{}\n\nNo plugins discovered.", attribution("list"));
    }

    let rows: Vec<Row> = scanned.iter().map(|p| row_for(p, manager)).collect();

    let mut lines = vec![attribution("list"), String::new()];
    lines.push(table_header());
    lines.push(table_separator());
    for row in &rows {
        lines.push(table_row(row));
    }

    let with_contributions: Vec<&Row> = rows
        .iter()
        .filter(|r| {
            !r.tools.is_empty()
                || !r.commands.is_empty()
                || !r.skills.is_empty()
                || !r.agents.is_empty()
                || !r.hooks.is_empty()
        })
        .collect();
    if !with_contributions.is_empty() {
        lines.push(String::new());
        lines.push("Contributions:".to_string());
        for row in with_contributions {
            let mut parts = vec![
                format!("tools [{}]", row.tools.join(", ")),
                format!("commands [{}]", row.commands.join(", ")),
            ];
            if !row.skills.is_empty() {
                parts.push(format!("skills [{}]", row.skills.join(", ")));
            }
            if !row.agents.is_empty() {
                parts.push(format!("agents [{}]", row.agents.join(", ")));
            }
            if !row.hooks.is_empty() {
                parts.push(format!("hooks [{}]", row.hooks.join(", ")));
            }
            lines.push(format!("- {}: {}", row.id, parts.join("; ")));
        }
    }

    let with_unsupported: Vec<&Row> = rows.iter().filter(|r| !r.unsupported.is_empty()).collect();
    if !with_unsupported.is_empty() {
        lines.push(String::new());
        lines.push("Unsupported capabilities:".to_string());
        for row in with_unsupported {
            lines.push(format!("- {}: {}", row.id, row.unsupported.join(", ")));
        }
    }

    let with_errors: Vec<&Row> = rows.iter().filter(|r| r.error.is_some()).collect();
    if !with_errors.is_empty() {
        lines.push(String::new());
        lines.push("Errors:".to_string());
        for row in with_errors {
            lines.push(format!(
                "- {}: {}",
                row.id,
                row.error.as_deref().unwrap_or_default()
            ));
        }
    }

    lines.push(String::new());
    lines.push(summary_line(&rows));

    if verbose {
        lines.push(String::new());
        lines.push("Telemetry:".to_string());
        for row in &rows {
            lines.push(telemetry_line(row));
        }
    }

    lines.join("\n")
}

/// Build the display row for one discovered plugin.
fn row_for(plugin: &ScannedPlugin, manager: &PluginManager) -> Row {
    let id = match &plugin.outcome {
        Ok(parsed) => parsed.descriptor.id.clone(),
        Err(_) => plugin
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    };
    let state = manager.state_of(&id);
    let (name, version, dialect, unsupported, error) = match &plugin.outcome {
        Ok(parsed) => (
            parsed.descriptor.name.clone(),
            parsed.descriptor.version.clone(),
            parsed.descriptor.dialect.to_string(),
            parsed.descriptor.unsupported_capabilities.clone(),
            None,
        ),
        Err(failure) => (
            id.clone(),
            "-".to_string(),
            "?".to_string(),
            Vec::new(),
            Some(failure.error.to_string()),
        ),
    };
    // Contributed names come from the live record when loaded (manifest plus
    // runtime `register_tool`/`register_command`), else from the parsed
    // manifest so a disabled plugin still lists what it would contribute.
    let (tools, commands) = match manager.get(&id) {
        Some(loaded) => (
            loaded.registered_tool_names(),
            loaded
                .commands
                .iter()
                .map(|c| c.decl.name.clone())
                .collect::<Vec<String>>(),
        ),
        None => match &plugin.outcome {
            Ok(parsed) => (
                parsed
                    .tools
                    .iter()
                    .map(|t| plugin_tool_name(&id, &t.name))
                    .collect::<Vec<String>>(),
                parsed
                    .commands
                    .iter()
                    .map(|c| c.decl.name.clone())
                    .collect::<Vec<String>>(),
            ),
            Err(_) => (Vec::new(), Vec::new()),
        },
    };
    // FR-032/FR-033: agent profiles and hook triggers come from the manifest
    // (they are not runtime registrations). FR-029: skill names are resolved
    // from each declared skills directory, one level deep.
    let (skills, agents, hooks) = match &plugin.outcome {
        Ok(parsed) => (
            plugin_skill_names(&parsed.descriptor.root, &parsed.skills),
            parsed.agents.clone(),
            parsed
                .hooks
                .iter()
                .map(|h| h.trigger.clone())
                .collect::<Vec<String>>(),
        ),
        Err(_) => (Vec::new(), Vec::new(), Vec::new()),
    };
    Row {
        id,
        name,
        version,
        dialect,
        state,
        tools,
        commands,
        skills,
        agents,
        hooks,
        unsupported,
        error,
        store: plugin.store.clone(),
    }
}

const W_ID: usize = 28;
const W_NAME: usize = 20;
const W_VERSION: usize = 8;
const W_DIALECT: usize = 7;
const W_STATE: usize = 8;
const W_TOOLS: usize = 5;
const W_COMMANDS: usize = 8;
const W_SKILLS: usize = 6;
const W_AGENTS: usize = 6;
const W_HOOKS: usize = 5;

fn table_header() -> String {
    format!(
        "| {:<w_id$} | {:<w_name$} | {:<w_ver$} | {:<w_dia$} | {:<w_state$} | {:>wt$} | {:>wc$} | {:>ws$} | {:>wa$} | {:>wh$} |",
        "ID",
        "Name",
        "Version",
        "Dialect",
        "State",
        "Tools",
        "Commands",
        "Skills",
        "Agents",
        "Hooks",
        w_id = W_ID,
        w_name = W_NAME,
        w_ver = W_VERSION,
        w_dia = W_DIALECT,
        w_state = W_STATE,
        wt = W_TOOLS,
        wc = W_COMMANDS,
        ws = W_SKILLS,
        wa = W_AGENTS,
        wh = W_HOOKS,
    )
}

fn table_separator() -> String {
    let cell = |w: usize| "-".repeat(w + 2);
    format!(
        "|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|",
        cell(W_ID),
        cell(W_NAME),
        cell(W_VERSION),
        cell(W_DIALECT),
        cell(W_STATE),
        cell(W_TOOLS),
        cell(W_COMMANDS),
        cell(W_SKILLS),
        cell(W_AGENTS),
        cell(W_HOOKS),
    )
}

fn table_row(row: &Row) -> String {
    format!(
        "| {:<w_id$} | {:<w_name$} | {:<w_ver$} | {:<w_dia$} | {:<w_state$} | {:>wt$} | {:>wc$} | {:>ws$} | {:>wa$} | {:>wh$} |",
        truncate(&row.id, W_ID),
        truncate(&row.name, W_NAME),
        truncate(&row.version, W_VERSION),
        truncate(&row.dialect, W_DIALECT),
        row.state.to_string(),
        row.tools.len(),
        row.commands.len(),
        row.skills.len(),
        row.agents.len(),
        row.hooks.len(),
        w_id = W_ID,
        w_name = W_NAME,
        w_ver = W_VERSION,
        w_dia = W_DIALECT,
        w_state = W_STATE,
        wt = W_TOOLS,
        wc = W_COMMANDS,
        ws = W_SKILLS,
        wa = W_AGENTS,
        wh = W_HOOKS,
    )
}

fn truncate(value: &str, width: usize) -> String {
    value.chars().take(width).collect()
}

/// The FR-009 summary line: totals by lifecycle state. A plugin is either
/// enabled or disabled; `loaded` is a transient sub-state of enabled (the
/// plugin is enabled and running this session), so loaded plugins are folded
/// into the `enabled` total.
fn summary_line(rows: &[Row]) -> String {
    let count = |state: LifecycleState| rows.iter().filter(|r| r.state == state).count();
    let enabled = count(LifecycleState::Loaded) + count(LifecycleState::Enabled);
    format!(
        "Total: {} plugin(s) - {} enabled, {} disabled, {} errored.",
        rows.len(),
        enabled,
        count(LifecycleState::Disabled),
        count(LifecycleState::Errored),
    )
}

/// One plugin's persisted telemetry counters (FR-022).
fn telemetry_line(row: &Row) -> String {
    let counters = StoreLedger::load(&row.store)
        .state(&row.id)
        .map(|s| s.counters.clone())
        .unwrap_or_default();
    format!(
        "- {}: loads_ok={}, load_failures={}, tool_invocations={}, tool_failures={}, \
         consecutive_failures={}",
        row.id,
        counters.loads_ok,
        counters.load_failures,
        counters.tool_invocations,
        counters.tool_failures,
        counters.consecutive_failures,
    )
}

// ── `/plugins enable` / `/plugins disable` reports (FR-011, FR-012) ──────────

/// Render the success-or-error report for `/plugins enable` (FR-011). The
/// declared permissions are stated before the outcome, matching the security
/// posture in the SPEC.
#[must_use]
pub fn enable_report(outcome: &EnableOutcome) -> String {
    let id = &outcome.report.id;
    let permissions = if outcome.report.declared_permissions.is_empty() {
        "none".to_string()
    } else {
        outcome.report.declared_permissions.join(", ")
    };
    let body = if outcome.report.state != LifecycleState::Loaded {
        format!(
            "[err] Plugin `{id}` failed to load: {}",
            outcome.report.error.as_deref().unwrap_or("unknown cause")
        )
    } else if let Some(cause) = &outcome.registration.error {
        format!("[err] Plugin `{id}` loaded but registration was refused: {cause}")
    } else {
        format!(
            "[ok] Plugin `{id}` is loaded; registered {} tool(s) and {} command(s).",
            outcome.registration.tools.len(),
            outcome.registration.commands.len(),
        )
    };
    format!(
        "{}\n\nDeclared permissions: {permissions}\n{body}",
        attribution(&format!("enable {id}"))
    )
}

/// Render the `[err]` report for `/plugins enable` refusing an unknown plugin.
#[must_use]
pub fn enable_error_report(plugin_id: &str, err: &crate::error::PluginError) -> String {
    format!(
        "{}\n\n[err] {err}",
        attribution(&format!("enable {plugin_id}"))
    )
}

/// Render the success-or-error report for `/plugins disable` (FR-012),
/// confirming how many tools and commands were deregistered.
#[must_use]
pub fn disable_report(outcome: &DisableOutcome) -> String {
    format!(
        "{}\n\n[ok] Plugin `{id}` is disabled; deregistered {tools} tool(s) and \
         {commands} command(s).",
        attribution(&format!("disable {}", outcome.id)),
        id = outcome.id,
        tools = outcome.tools_deregistered,
        commands = outcome.commands_deregistered,
    )
}

/// Render the `[err]` report for `/plugins disable` refusing an unknown plugin.
#[must_use]
pub fn disable_error_report(plugin_id: &str, err: &crate::error::PluginError) -> String {
    format!(
        "{}\n\n[err] {err}",
        attribution(&format!("disable {plugin_id}"))
    )
}

//! Command-surface helpers shared by the TUI and CLI `/plugins` front ends
//! (spec `plugins` T-014/T-017; FR-021, FR-024).
//!
//! Both the TUI slash family ([`crate::run_control_command`] callers in
//! `ragent-tui`) and the `ragent plugins` CLI parity surface need the same two
//! pieces of glue: an ephemeral [`PluginSurface`] to record the names a single
//! `/plugins` invocation registers, and a `_and_config` resolver that reads the
//! `plugins` config block and derives the store directories. Keeping one copy
//! here means the two surfaces cannot drift.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use crate::command_adapter::PluginCommandAdapter;
use crate::commands::run_store_command;
use crate::control::run_control_command;
use crate::harness::run_test_command;
use crate::session::{PluginSession, PluginSurface};
use crate::store::{StoreDirs, store_dirs};
use crate::tool_adapter::{PluginToolAdapter, plugin_tool_name};

/// An ephemeral [`PluginSurface`] for one `/plugins` invocation: it records the
/// names registered during the call so `enable`/`disable` can collision-check
/// and report contributions. Nothing is persisted — the surface exists only for
/// the duration of the call.
///
/// `existing_tools` / `existing_commands` are seeded by the caller (built-in
/// tool registry plus the `SLASH_COMMANDS` triggers) so collision rejection
/// (FR-024) is checked against the real surface.
#[derive(Debug, Default)]
pub struct ScratchSurface {
    existing_tools: BTreeSet<String>,
    existing_commands: BTreeSet<String>,
    tools: BTreeSet<String>,
    commands: BTreeSet<String>,
}

impl ScratchSurface {
    /// Build a surface seeded with the caller's pre-existing tool and command
    /// names (the surface a fresh session already presents).
    #[must_use]
    pub fn seeded(existing_tools: BTreeSet<String>, existing_commands: BTreeSet<String>) -> Self {
        Self {
            existing_tools,
            existing_commands,
            ..Self::default()
        }
    }

    /// Union the caller's pre-existing names (`base`) with the names registered
    /// during this call (`registered`), so collision checks see the real surface.
    fn merged(base: &BTreeSet<String>, registered: &BTreeSet<String>) -> BTreeSet<String> {
        let mut set = base.clone();
        set.extend(registered.iter().cloned());
        set
    }
}

impl PluginSurface for ScratchSurface {
    fn existing_tool_names(&self) -> BTreeSet<String> {
        Self::merged(&self.existing_tools, &self.tools)
    }

    fn existing_command_names(&self) -> BTreeSet<String> {
        Self::merged(&self.existing_commands, &self.commands)
    }

    fn register_tool(&mut self, adapter: Arc<PluginToolAdapter>) {
        self.tools
            .insert(plugin_tool_name(adapter.plugin_id(), adapter.tool_name()));
    }

    fn register_command(&mut self, adapter: Arc<PluginCommandAdapter>) {
        self.commands.insert(adapter.name().to_string());
    }

    fn deregister_tool(&mut self, registry_name: &str) {
        self.tools.remove(registry_name);
    }

    fn deregister_command(&mut self, trigger: &str) {
        self.commands.remove(trigger);
    }
}

/// Resolve the plugin store directories and configuration from the live
/// environment, honouring the `plugins.store_dir` override (FR-001).
///
/// `workdir` is the project working directory the store is resolved against.
/// A config that cannot be read falls back to the compiled defaults (matching
/// the rest of the codebase's tolerant config loading); the failure is logged
/// rather than silently discarded so a malformed `ragent.json` is visible.
#[must_use]
pub fn store_and_config(workdir: &Path) -> (StoreDirs, ragent_config::PluginsConfig) {
    let config = match ragent_config::Config::load() {
        Ok(config) => config.plugins.unwrap_or_default(),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "plugin config could not be loaded; using compiled defaults"
            );
            ragent_config::PluginsConfig::default()
        }
    };
    let dirs = store_dirs(workdir, config.store_dir.as_deref());
    (dirs, config)
}

/// Parse-and-run the management subcommands of `/plugins <sub> <rest>` on the
/// shared surface.
///
/// This is the one dispatch ladder both front ends ([`crate::run_cli`] for
/// `ragent plugins` and the TUI `/plugins` family) drive, so the store / test /
/// control fall-through can never drift between them. `existing_tools` and
/// `existing_commands` seed the collision surface (FR-024).
///
/// Returns `None` when `sub` is not a management subcommand (`add` / `remove` /
/// `list` / `enable` / `disable` / `test`), so the caller can render its own
/// (surface-specific) usage text. When it is a management subcommand the report
/// string to print is returned, including the usage fall-back when the control
/// parse rejects an otherwise-known `sub`.
#[must_use]
pub fn run_plugin_subcommand(
    workdir: &Path,
    sub: &str,
    rest: &str,
    existing_tools: BTreeSet<String>,
    existing_commands: BTreeSet<String>,
) -> Option<String> {
    let (dirs, config) = store_and_config(workdir);

    // `/plugins stores` reports each store's effective endpoint and its source
    // (spec `pluginstores` FR-031). It is a pure config read: no session and no
    // store access. With `--check` it additionally contacts each store endpoint
    // and reports availability and catalogue size; the fetch is bounded by the
    // configured time/byte budget and every failure is contained as `[err]`.
    if sub == "stores" {
        let stores = config.stores_or_default();
        let probes = crate::commands::stores_check_requested(rest).then(|| {
            crate::commands::probe_stores(&stores, crate::store_seam::default_fetcher().as_ref())
        });
        return Some(crate::commands::render_stores_report_with_probes(
            &stores,
            probes.as_deref(),
        ));
    }

    // Store subcommands (add / remove) need no live session.
    if let Some(report) = run_store_command(&config, &dirs, workdir, sub, rest) {
        return Some(report);
    }

    // The isolated test harness must not touch any live session (FR-013).
    if let Some(report) = run_test_command(dirs.clone(), &config, sub, rest) {
        return Some(report);
    }

    // Control subcommands (list / enable / disable) drive a live session for the
    // duration of this call; the sandbox contexts are dropped on return. Seed the
    // collision surface so collision rejection (FR-024) is checked against the
    // real surface.
    let mut surface = ScratchSurface::seeded(existing_tools, existing_commands);
    let mut session = PluginSession::start(dirs, config, &mut surface);
    run_control_command(&mut session, &mut surface, sub, rest)
        .or_else(|| Some(crate::help::render_help(sub)))
}

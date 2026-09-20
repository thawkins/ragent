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
use crate::session::PluginSurface;
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
}

impl PluginSurface for ScratchSurface {
    fn existing_tool_names(&self) -> BTreeSet<String> {
        let mut set = self.existing_tools.clone();
        set.extend(self.tools.iter().cloned());
        set
    }

    fn existing_command_names(&self) -> BTreeSet<String> {
        let mut set = self.existing_commands.clone();
        set.extend(self.commands.iter().cloned());
        set
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

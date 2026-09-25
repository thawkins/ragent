//! Plugin bridging helpers: surface enabled plugins' contributions to the
//! session (spec `plugins` FR-030, FR-031).
//!
//! Enabled plugins can contribute MCP servers (from their manifest
//! `mcpServers` section). [`plugin_mcp_servers`] merges those with the servers
//! configured in `ragent.json`, so a session can connect both at startup
//! exactly the same way.
//!
//! Enabled plugins can also contribute slash commands. [`plugin_commands`]
//! resolves those (prompt-command bodies included) into a list the session
//! registers into its command surface. The skills bridge lives in
//! [`crate::skill::effective_skill_dirs`], next to the skill discovery it feeds.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use ragent_config::McpServerConfig;

use crate::skill::plugin_store_dirs;

/// Merge the MCP servers contributed by enabled plugins with `configured`
/// (spec `plugins` FR-030).
///
/// Returns `(server_id, config)` pairs sorted by id. Configured servers keep
/// their unprefixed ids and take precedence: a plugin server whose id would
/// collide with a configured id is dropped, as are later plugins colliding on
/// the same id. Plugin server ids are prefixed `<plugin-id>.<server>` by the
/// bridge, so collisions are only possible against a configured server that
/// happens to use the same dotted id.
#[must_use]
pub fn plugin_mcp_servers<S: std::hash::BuildHasher>(
    working_dir: &Path,
    configured: &HashMap<String, McpServerConfig, S>,
) -> Vec<(String, McpServerConfig)> {
    let mut taken: std::collections::HashSet<String> = configured.keys().cloned().collect();
    let mut merged: Vec<(String, McpServerConfig)> = configured
        .iter()
        .map(|(id, config)| (id.clone(), config.clone()))
        .collect();
    for (id, config) in ragent_plugins::scanned_plugin_mcp_servers(&plugin_store_dirs(working_dir))
    {
        if taken.insert(id.clone()) {
            merged.push((id, config));
        }
    }
    merged.sort_by(|a, b| a.0.cmp(&b.0));
    merged
}

/// One slash command contributed by an enabled plugin, ready for the session
/// command surface (spec `plugins` FR-031).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCommand {
    /// The plugin id that contributes the command.
    pub plugin_id: String,
    /// The command's own name, without a leading `/` (e.g. `commit`).
    pub name: String,
    /// The slash trigger to register (no leading `/`). This is the bare `name`
    /// when it is free, otherwise the namespaced `plugin:<id>:<name>` form so a
    /// plugin can never shadow a built-in command, a skill, or another plugin.
    pub trigger: String,
    /// Human-readable description for autocomplete and help.
    pub description: String,
    /// The prompt body for a prompt command, else `None` for an inline command
    /// (whose handler runs inside the plugin's sandbox).
    pub prompt: Option<String>,
}

/// Resolve the slash commands contributed by enabled plugins (FR-031).
///
/// `existing` is the set of triggers already taken (the built-in
/// `SLASH_COMMANDS` surface plus any user-invocable skill names). A command
/// whose bare name is free registers under that name; a colliding name
/// registers under the namespaced `plugin:<id>:<name>` trigger instead, so a
/// plugin can never shadow an existing command. The list is sorted by trigger.
#[must_use]
pub fn plugin_commands(working_dir: &Path, existing: &BTreeSet<String>) -> Vec<PluginCommand> {
    let mut taken = existing.clone();
    let mut out: Vec<PluginCommand> = Vec::new();
    for (plugin_id, def) in ragent_plugins::scanned_plugin_commands(&plugin_store_dirs(working_dir))
    {
        let name = def.decl.name.clone();
        let trigger = if taken.contains(&name) {
            format!("plugin:{plugin_id}:{name}")
        } else {
            name.clone()
        };
        if taken.contains(&trigger) {
            continue;
        }
        taken.insert(trigger.clone());
        out.push(PluginCommand {
            plugin_id,
            name,
            trigger,
            description: def.decl.description.clone(),
            prompt: def.prompt().map(str::to_string),
        });
    }
    out.sort_by(|a, b| a.trigger.cmp(&b.trigger));
    out
}

/// Resolve the MCP servers contributed by enabled plugins for `working_dir`,
/// each tagged with the plugin that declared it.
///
/// The owning plugin id is kept alongside the bridged `<plugin-id>.<server>` id
/// because a surface that reports a plugin's MCP servers beside its other
/// contributions (`/plugins list`) cannot recover the plugin id from the
/// bridged id alone. The plugin store root is derived from the loaded config's
/// path (a relative `.ragent/ragent.json` is anchored at the current working
/// directory) so a caller resolves the same store the session's config load
/// did.
#[must_use]
pub fn plugin_mcp_contributions(working_dir: &Path) -> Vec<ragent_plugins::PluginMcpContribution> {
    let store_root = crate::skill::effective_plugin_store_root(working_dir);
    let override_dir = ragent_config::Config::load()
        .ok()
        .and_then(|config| config.plugins)
        .and_then(|plugins| plugins.store_dir);
    ragent_plugins::scanned_plugin_mcp_contributions(&ragent_plugins::store_dirs(
        &store_root,
        override_dir.as_deref(),
    ))
}

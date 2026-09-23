//! Plugin bridges: surface a loaded plugin's `skills` and `mcpServers`
//! sections to the agent session (spec `plugins` FR-029, FR-030).
//!
//! Two bridges are provided, both operating on the **enabled** plugins
//! discovered by [`crate::store::scan_dirs`] (a disabled plugin is inert and
//! contributes nothing, FR-016):
//!
//! - [`plugin_skill_dirs`] / [`scanned_plugin_skill_dirs`] — resolve each
//!   plugin's `skills` section into absolute directories so the session's
//!   skill discovery can append them to its scan roots. A skill directory is
//!   the folder that *contains* per-skill subdirectories, each holding a
//!   `SKILL.md` (the same layout `SkillRegistry::load` expects).
//! - [`plugin_mcp_servers`] / [`scanned_plugin_mcp_servers`] — resolve each
//!   plugin's `mcpServers` section into `(server_id, McpServerConfig)` pairs so
//!   the session can connect them at startup exactly like a configured server.
//!   The server id is prefixed with the plugin id (`<plugin-id>.<server>`) so
//!   two plugins cannot collide on a server name, and it never collides with a
//!   plain `ragent.json` `mcp` key (those are unprefixed).
//!
//! A plugin's `mcpServers` is either inline (`{"<id>": {command, args, ...}}`)
//! or a string naming an external MCP-config file (the Claude marketplace
//! `"mcpServers": "./mcp.json"` shape); the external file is resolved relative
//! to the plugin root, guarded against escaping it, and its own top-level
//! `mcpServers` object is read.
//!
//! These functions read the plugin manifest from disk (via
//! [`crate::manifest::parse_plugin_dir`]) but never execute plugin JavaScript.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use ragent_config::McpServerConfig;

use crate::manifest::{
    AGENTS_DIR, ParsedManifest, PluginCommandDef, PluginHook, PluginMcpServer, extract_mcp_servers,
};
use crate::store::{ScannedPlugin, StoreDirs, scan_dirs};

/// Scan the stores once and return the parsed manifests of the **enabled**
/// plugins (FR-016). A disabled plugin is inert and contributes nothing; a
/// plugin whose manifest cannot be parsed is reported by discovery and skipped
/// here. Shared by every `scanned_plugin_*` bridge so the enable/parse filter
/// lives in one place.
fn enabled_manifests(dirs: &StoreDirs) -> Vec<ParsedManifest> {
    scan_dirs(dirs.clone())
        .into_iter()
        .filter(|plugin| plugin.enabled)
        .filter_map(|plugin| plugin.outcome.ok())
        .collect()
}

/// Resolve the skill directories contributed by the enabled plugins in the
/// stores described by `dirs` (FR-029 skills bridge).
///
/// Returns absolute paths, de-duplicated and sorted. A plugin whose manifest
/// cannot be parsed contributes nothing (it is reported by discovery).
#[must_use]
pub fn scanned_plugin_skill_dirs(dirs: &StoreDirs) -> Vec<PathBuf> {
    let mut out: BTreeSet<PathBuf> = BTreeSet::new();
    for parsed in enabled_manifests(dirs) {
        out.extend(plugin_skill_dirs(&parsed.descriptor.root, &parsed.skills));
    }
    out.into_iter().collect()
}

/// Resolve the MCP servers contributed by the enabled plugins in the stores
/// described by `dirs` (FR-030 MCP bridge).
///
/// Returns `(server_id, config)` pairs with ids of the form
/// `<plugin-id>.<server>`; ids are de-duplicated (first wins) and the result is
/// sorted by id for a stable connect order.
#[must_use]
pub fn scanned_plugin_mcp_servers(dirs: &StoreDirs) -> Vec<(String, McpServerConfig)> {
    let mut by_id: BTreeMap<String, McpServerConfig> = BTreeMap::new();
    for parsed in enabled_manifests(dirs) {
        for (id, config) in plugin_mcp_servers(
            &parsed.descriptor.id,
            &parsed.descriptor.root,
            &parsed.mcp_servers,
            parsed.raw_mcp.as_ref(),
        ) {
            by_id.entry(id).or_insert(config);
        }
    }
    by_id.into_iter().collect()
}

/// Resolve a plugin's declared skill directories against its root (FR-029).
///
/// Every declared path is joined onto `root` and normalised; a path that
/// escapes the plugin root (via `..`) or is absolute is dropped. The resulting
/// directory is the skill *parent* directory (the one containing `SKILL.md`
/// subdirectories).
#[must_use]
pub fn plugin_skill_dirs(root: &Path, declared: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for raw in declared {
        if let Some(resolved) = resolve_within(root, raw)
            && !out.contains(&resolved)
        {
            out.push(resolved);
        }
    }
    out
}

/// Map a plugin's MCP servers to `(id, config)` pairs (FR-030).
///
/// `servers` are the inline entries already extracted by the manifest parser;
/// `raw` is the raw `mcpServers` value. When `raw` is a top-level string (the
/// Claude marketplace `"mcpServers": "./mcp.json"` shape) the named file is
/// read and its own `mcpServers` object supplies the entries instead. The id
/// is `<plugin_id>.<server_id>`.
#[must_use]
pub fn plugin_mcp_servers(
    plugin_id: &str,
    root: &Path,
    servers: &[PluginMcpServer],
    raw: Option<&serde_json::Value>,
) -> Vec<(String, McpServerConfig)> {
    // External-file shape: a top-level string names an MCP-config file whose
    // own `mcpServers` object supplies the entries (Claude marketplace).
    let resolved: Vec<PluginMcpServer> = match raw {
        Some(serde_json::Value::String(rel)) => {
            read_external_mcp_servers(root, rel).unwrap_or_default()
        }
        _ => servers.to_vec(),
    };

    let mut out = Vec::new();
    for server in resolved {
        if let Some(config) = to_config(&server) {
            out.push((format!("{plugin_id}.{}", server.id), config));
        }
    }
    out
}

/// Build a [`McpServerConfig`] from a bridged server, returning `None` when the
/// entry has neither a command nor a URL (unusable).
fn to_config(server: &PluginMcpServer) -> Option<McpServerConfig> {
    let transport = match server.transport.as_str() {
        "sse" => ragent_config::McpTransport::Sse,
        "http" => ragent_config::McpTransport::Http,
        _ => ragent_config::McpTransport::Stdio,
    };
    if server.command.is_none() && server.url.is_none() {
        return None;
    }
    Some(McpServerConfig {
        type_: transport,
        command: server.command.clone(),
        args: server.args.clone(),
        env: server.env.clone().into_iter().collect(),
        url: server.url.clone(),
        headers: server.headers.clone().into_iter().collect(),
        disabled: false,
        notification: ragent_config::trigger::McpNotificationMode::None,
    })
}

/// Read an external MCP-config file named by a plugin's `mcpServers` string
/// value, returning the servers from its top-level `mcpServers` object.
/// Returns `None` when the path escapes the plugin root, cannot be read, or
/// does not carry an `mcpServers` object.
fn read_external_mcp_servers(root: &Path, rel: &str) -> Option<Vec<PluginMcpServer>> {
    let path = resolve_within(root, rel)?;
    let bytes = std::fs::read(&path).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let raw = value.get("mcpServers")?;
    let (servers, _unbridged) = extract_mcp_servers(Some(raw));
    Some(servers)
}

/// Join `rel` onto `root` and normalise it, returning `None` when the result
/// escapes `root` or `rel` is absolute. Purely lexical (no filesystem access),
/// so a plugin cannot use a symlink to point outside its root here — the
/// caller's own reads stay within the plugin directory.
fn resolve_within(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel_path = Path::new(rel.trim());
    if rel_path.is_absolute() {
        return None;
    }
    let mut depth: i32 = 0;
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for comp in rel_path.components() {
        match comp {
            Component::Normal(part) => {
                parts.push(part);
                depth += 1;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
                parts.pop();
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    let mut resolved = root.to_path_buf();
    for part in parts {
        resolved.push(part);
    }
    Some(resolved)
}

/// Resolve the slash commands contributed by the enabled plugins in the stores
/// described by `dirs` (FR-031 command bridge).
///
/// Returns `(plugin_id, command)` pairs, sorted by plugin id then command name.
/// Only **enabled** plugins contribute: a disabled plugin is inert. A plugin
/// whose manifest cannot be parsed contributes nothing. Prompt-command file
/// bodies are read here (from disk), so the caller receives a self-contained
/// definition it can list and inject without further plugin access.
#[must_use]
pub fn scanned_plugin_commands(dirs: &StoreDirs) -> Vec<(String, PluginCommandDef)> {
    let mut out: Vec<(String, PluginCommandDef)> = Vec::new();
    for parsed in enabled_manifests(dirs) {
        for def in &parsed.commands {
            out.push((parsed.descriptor.id.clone(), def.clone()));
        }
    }
    out.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.decl.name.cmp(&b.1.decl.name))
    });
    out
}

/// List the skill names contributed by a plugin's declared `skills` sections
/// (FR-029).
///
/// Each declared directory is resolved against the plugin root (guarding against
/// escape) and scanned one level deep for subdirectories that hold a `SKILL.md`;
/// the subdirectory name is the skill name, mirroring the layout the skill
/// registry's loader expects. Returns names sorted and de-duplicated; a
/// directory that does not exist or holds no `SKILL.md` subdirectories
/// contributes nothing.
#[must_use]
pub fn plugin_skill_names(root: &Path, declared: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for dir in plugin_skill_dirs(root, declared) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() || !path.join("SKILL.md").is_file() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && !out.iter().any(|existing| existing == name)
            {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    out
}

/// Convenience over [`scanned_plugin_skill_dirs`] used by tests and callers
/// holding a single already-discovered plugin.
#[must_use]
pub fn skills_of(scanned: &ScannedPlugin) -> Vec<PathBuf> {
    match &scanned.outcome {
        Ok(parsed) if scanned.enabled => plugin_skill_dirs(&parsed.descriptor.root, &parsed.skills),
        _ => Vec::new(),
    }
}

/// Resolve the agent-profile files contributed by the enabled plugins in the
/// stores described by `dirs` (FR-032 agents bridge).
///
/// Returns absolute paths to each declared or discovered agent profile,
/// de-duplicated and sorted. A plugin whose manifest cannot be parsed
/// contributes nothing (it is reported by discovery).
#[must_use]
pub fn scanned_plugin_agent_files(dirs: &StoreDirs) -> Vec<PathBuf> {
    let mut out: BTreeSet<PathBuf> = BTreeSet::new();
    for parsed in enabled_manifests(dirs) {
        out.extend(plugin_agent_files(&parsed.descriptor.root, &parsed.agents));
    }
    out.into_iter().collect()
}

/// Resolve a plugin's declared agent profiles against its root (FR-032).
///
/// Every declared path is joined onto `root` and normalised; a path that
/// escapes the plugin root (via `..`) or is absolute is dropped. A bare name
/// gets the `agents/` directory and `.md` extension applied, mirroring the
/// Claude `agents/<name>.md` layout; a path that already carries an extension
/// is used as-is.
#[must_use]
pub fn plugin_agent_files(root: &Path, declared: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for raw in declared {
        let rel = agent_rel_path(raw);
        if let Some(resolved) = resolve_within(root, &rel)
            && !out.contains(&resolved)
        {
            out.push(resolved);
        }
    }
    out
}

/// Normalise a declared agent path: a bare name becomes `agents/<name>.md`; a
/// path with an extension is used verbatim.
fn agent_rel_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if Path::new(trimmed).extension().is_some() {
        trimmed.to_string()
    } else {
        format!("{AGENTS_DIR}/{trimmed}.md")
    }
}

/// Resolve the hooks contributed by the enabled plugins in the stores described
/// by `dirs` (FR-033 hooks bridge).
///
/// Returns `(plugin_id, trigger, command, timeout_secs)` tuples sorted by
/// plugin id then declaration order. The session layer maps each trigger name
/// onto its own hook trigger and drops any it does not recognise.
#[must_use]
pub fn scanned_plugin_hooks(dirs: &StoreDirs) -> Vec<PluginHook> {
    let mut out: Vec<PluginHook> = Vec::new();
    for parsed in enabled_manifests(dirs) {
        out.extend(parsed.hooks.iter().cloned());
    }
    out.sort_by(|a, b| {
        a.plugin_id
            .cmp(&b.plugin_id)
            .then_with(|| a.trigger.cmp(&b.trigger))
            .then_with(|| a.command.cmp(&b.command))
    });
    out
}

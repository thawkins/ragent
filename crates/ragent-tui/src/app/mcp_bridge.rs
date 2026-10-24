//! Bridged MCP server enumeration shared by every `/mcp` surface.
//!
//! The session's MCP server set is not just the `ragent.json` `mcp` section:
//! enabled plugins contribute their `mcpServers` entries (spec `plugins`
//! FR-030) and enabled connectors contribute their resolved servers (spec
//! `connectors` FR-003, FR-020, FR-026), both under a `<owner-id>.<server-id>`
//! bridged id. `src/main.rs` merges the three sources before its connect loop
//! runs, so every surface that reports "the MCP servers this session has" must
//! merge the same three or it reports a set the session never connected.
//!
//! This module is that one merge, plus the origin bookkeeping the surfaces need
//! but the bridged id cannot express (both halves may themselves contain dots):
//!
//! - `/mcp` builds its display list from [`merged_mcp_servers`];
//! - `/mcp discover` appends [`bridged_discover_rows`] so a bridged server can be
//!   enabled and disabled from the same dialog as a discovered one;
//! - `/mcp connect` / `/mcp disconnect` resolve the config with
//!   [`resolve_config`];
//! - `/mcp remove` refuses a bridged id and points at the owner's own removal
//!   command, because deleting one server out of a plugin's or connector's
//!   manifest would leave the owner installed but silently incomplete.
//!
//! Resolution follows `src/main.rs` exactly: configured entries first (they win
//! an id collision), then plugins, then connectors (a connector server whose id
//! collides with anything already taken is refused by
//! `ragent_connectors::scanned_bridge` and is therefore never listed here), and
//! the connector leg is skipped entirely when `connectors.enabled` is false.
//!
//! Nothing here connects or writes; every function is a read-only scan.

use std::collections::HashMap;
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};

use ragent_agent::McpServerConfig;
use ragent_agent::mcp::discovery::DiscoveredMcpServer;

/// Which subsystem contributed a bridged MCP server (`<owner-id>.<server-id>`).
///
/// The bridged id alone cannot say this: a plugin id and a connector id are both
/// ordinary identifiers, and both may contain dots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeOrigin {
    /// Contributed by an enabled plugin's `mcpServers` section (FR-030).
    Plugin {
        /// The plugin that declared the server.
        plugin_id: String,
    },
    /// Contributed by an enabled connector (FR-003).
    Connector {
        /// The connector that declared the server.
        connector_id: String,
    },
}

impl BridgeOrigin {
    /// The short subsystem label a row prints in its source column.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Plugin { .. } => "plugin",
            Self::Connector { .. } => "connector",
        }
    }

    /// The owning plugin or connector id.
    pub fn owner(&self) -> &str {
        match self {
            Self::Plugin { plugin_id } => plugin_id,
            Self::Connector { connector_id } => connector_id,
        }
    }

    /// The command that removes the owning plugin or connector.
    ///
    /// `/mcp remove` refuses a bridged id and names this instead: the definition
    /// belongs to the owner, so uninstalling the owner is the only removal that
    /// leaves its manifest consistent.
    pub fn removal_hint(&self) -> String {
        match self {
            Self::Plugin { plugin_id } => format!("`/plugins remove {plugin_id}`"),
            Self::Connector { connector_id } => format!("`/connectors remove {connector_id}`"),
        }
    }
}

/// One MCP server the session has because a plugin or a connector declares it.
#[derive(Debug, Clone)]
pub struct BridgedMcpServer {
    /// The bridged registry id (`<owner-id>.<server-id>`), which is what the
    /// session registers the server under and what the tools are prefixed with.
    pub id: String,
    /// The transport configuration the session connects with.
    pub config: McpServerConfig,
    /// File that carries the declaration: the plugin manifest, the external
    /// `mcp.json` it names, or the connector's `connector.json`.
    pub source_file: Option<PathBuf>,
    /// The subsystem that contributed it.
    pub origin: BridgeOrigin,
}

/// Every MCP server contributed by an enabled plugin or an enabled connector.
///
/// `configured` is the `ragent.json` `mcp` section: its ids are already taken, so
/// a bridged id that would collide with one is never listed (a connector bridge
/// refuses it, and the plugin leg skips it). The result is sorted by bridged id.
pub fn bridged_servers<S: BuildHasher>(
    working_dir: &Path,
    configured: &HashMap<String, McpServerConfig, S>,
) -> Vec<BridgedMcpServer> {
    let mut taken: std::collections::HashSet<String> = configured.keys().cloned().collect();
    let mut out: Vec<BridgedMcpServer> = Vec::new();

    // Plugins first: the plugin bridge predates the connector bridge, and
    // `src/main.rs` merges in this order.
    for contribution in ragent_agent::plugin::plugin_mcp_contributions(working_dir) {
        if taken.insert(contribution.server_id.clone()) {
            out.push(BridgedMcpServer {
                id: contribution.server_id,
                config: contribution.config,
                source_file: Some(contribution.source_file),
                origin: BridgeOrigin::Plugin {
                    plugin_id: contribution.plugin_id,
                },
            });
        }
    }

    out.extend(connector_servers(working_dir, &taken));
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The connector leg of [`bridged_servers`]: every accepted server of an enabled
/// connector, with the `connector.json` that declares it recorded as its file.
///
/// Returns nothing when `connectors.enabled` is false (FR-021), matching
/// `src/main.rs`, which creates no connector session in that case.
fn connector_servers(
    working_dir: &Path,
    taken: &std::collections::HashSet<String>,
) -> Vec<BridgedMcpServer> {
    let Ok(config) = ragent_config::Config::load() else {
        return Vec::new();
    };
    let connectors = config.connectors.clone().unwrap_or_default();
    if !connectors.is_enabled() {
        return Vec::new();
    }
    let dirs = ragent_connectors::store_dirs(working_dir, connectors.store_dir.as_deref());
    let taken_ids: Vec<&str> = taken.iter().map(String::as_str).collect();
    let plan = ragent_connectors::scanned_bridge(&dirs, taken_ids);
    if plan.is_empty() {
        return Vec::new();
    }
    // One extra scan maps a connector id onto the directory its manifest lives
    // in, so `/mcp` names the file the definition came from instead of only the
    // connector id. The bridge itself does not carry a path.
    let manifests: HashMap<String, PathBuf> = ragent_connectors::scan_dirs(&dirs)
        .into_iter()
        .map(|scanned| {
            (
                scanned.id,
                scanned.dir.join(ragent_connectors::MANIFEST_FILE),
            )
        })
        .collect();
    let mut out = Vec::new();
    for server in plan.servers {
        if taken.contains(&server.server_id) {
            continue;
        }
        out.push(BridgedMcpServer {
            source_file: manifests.get(&server.connector_id).cloned(),
            id: server.server_id,
            config: server.config,
            origin: BridgeOrigin::Connector {
                connector_id: server.connector_id,
            },
        });
    }
    out
}

/// The full MCP server set the session runs: configured, then bridged.
///
/// `(id, config, source_file)` mirrors `ragent_agent::plugin::merged_mcp_servers`,
/// with the connector leg added. A configured server carries `None` (the caller
/// knows the `ragent.json` it loaded); a bridged server carries the file that
/// declares it, which is what `/mcp` prints in its `Defined in` column.
pub fn merged_mcp_servers<S: BuildHasher>(
    working_dir: &Path,
    configured: &HashMap<String, McpServerConfig, S>,
) -> Vec<(String, McpServerConfig, Option<PathBuf>)> {
    let mut out: Vec<(String, McpServerConfig, Option<PathBuf>)> = configured
        .iter()
        .map(|(id, config)| (id.clone(), config.clone(), None))
        .collect();
    out.extend(
        bridged_servers(working_dir, configured)
            .into_iter()
            .map(|server| (server.id, server.config, server.source_file)),
    );
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The connect configuration for `server_id`, wherever its definition lives.
///
/// A configured `ragent.json` entry wins; otherwise an enabled plugin's or
/// connector's bridged server supplies it. `None` means no definition exists, so
/// enabling the id must be refused rather than written into the enable ledger.
pub fn resolve_config<S: BuildHasher>(
    working_dir: &Path,
    configured: &HashMap<String, McpServerConfig, S>,
    server_id: &str,
) -> Option<McpServerConfig> {
    if let Some(config) = configured.get(server_id) {
        return Some(config.clone());
    }
    bridged_servers(working_dir, configured)
        .into_iter()
        .find(|server| server.id == server_id)
        .map(|server| server.config)
}

/// Whether a `/mcp discover` row stands for a server bridged from a plugin or a
/// connector (as opposed to one discovery merely proposed).
///
/// A bridged row is already declared by its owner's manifest, so enabling it
/// must not write a `ragent.json` entry, and removing it is refused outright
/// (see [`BridgeOrigin::removal_hint`]).
pub fn is_bridged_source(source: &ragent_agent::mcp::McpDiscoverySource) -> bool {
    matches!(
        source,
        ragent_agent::mcp::McpDiscoverySource::Plugin { .. }
            | ragent_agent::mcp::McpDiscoverySource::Connector { .. }
    )
}

/// The bridged server with `server_id`, when one exists.
///
/// Used by `/mcp remove` (which refuses a bridged id and names the owner) and by
/// the `/mcp discover` toggle (which must not write a `ragent.json` entry for a
/// server whose definition already exists).
pub fn find_bridged<S: BuildHasher>(
    working_dir: &Path,
    configured: &HashMap<String, McpServerConfig, S>,
    server_id: &str,
) -> Option<BridgedMcpServer> {
    if configured.contains_key(server_id) {
        return None;
    }
    bridged_servers(working_dir, configured)
        .into_iter()
        .find(|server| server.id == server_id)
}

/// The `/mcp remove` refusal for a bridged server, naming the owner's own
/// removal command instead.
///
/// `/mcp remove` deletes a definition from the file that declares it. A bridged
/// server's definition is one entry inside a plugin's `mcpServers` object or a
/// connector's `connector.json`; pruning that entry would leave the owner
/// installed with a server list it no longer declares, which the next bridge
/// scan and `/plugins list` / `/connectors list` would report as drift. So the
/// id is refused and the owner's uninstall command is named. Enabling and
/// disabling, which are what a bridged server actually needs, work through
/// `/mcp connect|disconnect` and the `/mcp discover` dialog.
pub fn bridged_removal_refusal(server_id: &str, server: &BridgedMcpServer) -> String {
    let label = server.origin.label();
    format!(
        "From: /mcp remove\n\n[err] `{server_id}` is bridged from {label} `{}` and cannot be \
         removed here.\n\nA bridged server's definition is an entry inside the {label}'s own \
         manifest{}, which only that {label} manages. Remove the {label} instead:\n  {}\n\nTo \
         stop the server without uninstalling the {label}, use `/mcp disconnect {server_id}` or \
         the `/mcp discover` dialog.",
        server.origin.owner(),
        match &server.source_file {
            Some(path) => format!(" (`{}`)", path.display()),
            None => String::new(),
        },
        server.origin.removal_hint(),
    )
}

/// One bridged server as a `/mcp discover` dialog row.
///
/// The rows reuse [`DiscoveredMcpServer`] so the dialog has one row type: the
/// `source` variant carries the origin (and therefore the owning plugin or
/// connector) and `definition_file` carries the file that declares the server.
/// `executable` is the `stdio` command for a stdio server and empty for an
/// `http`/`sse` one; the dialog does not paint it, and a bridged row is never
/// persisted through `to_config`, so it is only informational.
pub fn discover_row_for(server: BridgedMcpServer) -> DiscoveredMcpServer {
    // The owning name is read before the origin is consumed: `name` labels the
    // row and `source` carries the origin variant.
    let name = server.origin.owner().to_string();
    let source = match server.origin {
        BridgeOrigin::Plugin { plugin_id } => {
            ragent_agent::mcp::McpDiscoverySource::Plugin { plugin_id }
        }
        BridgeOrigin::Connector { connector_id } => {
            ragent_agent::mcp::McpDiscoverySource::Connector { connector_id }
        }
    };
    DiscoveredMcpServer {
        name,
        executable: server
            .config
            .command
            .clone()
            .map(PathBuf::from)
            .unwrap_or_default(),
        args: server.config.args.clone(),
        env: server.config.env.clone(),
        source,
        definition_file: server.source_file,
        id: server.id,
    }
}

/// The bridged servers as `/mcp discover` dialog rows (see [`discover_row_for`]).
pub fn bridged_discover_rows<S: BuildHasher>(
    working_dir: &Path,
    configured: &HashMap<String, McpServerConfig, S>,
) -> Vec<DiscoveredMcpServer> {
    bridged_servers(working_dir, configured)
        .into_iter()
        .map(discover_row_for)
        .collect()
}

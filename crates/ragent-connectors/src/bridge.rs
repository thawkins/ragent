//! Connector MCP bridge (spec `connectors` T-006; FR-003, FR-020, FR-026,
//! FR-033).
//!
//! The bridge is the pure, offline step between discovery and the session's MCP
//! client: it resolves each **enabled** connector's server definitions into
//! `(bridged_server_id, McpServerConfig)` pairs whose bridged id is
//! `<connector-id>.<server>` (FR-003), so two connectors cannot collide on a
//! server name and the bridged server never clashes with a plain `ragent.json`
//! `mcp` key (those are unprefixed).
//!
//! Registering a server under its bridged id is all FR-020 needs: the existing
//! MCP tool path names every advertised tool `mcp_<server>_<tool>` (see
//! `McpToolWrapper::ragent_name_for` in `ragent-agent`), so a connector's tools
//! surface as the ordinary MCP tools of `<connector-id>.<server>` and are subject
//! to the normal permission engine.
//!
//! A connector that declares several servers contributes several pairs (FR-026),
//! each resolved independently so one server's failure cannot hide another's. A
//! bridged id that would collide with a configured `ragent.json` `mcp` key or
//! with another server already bridged is refused and reported rather than
//! silently overwriting the existing server (FR-033).
//!
//! The bridge reads the store and its enable ledger but **never connects an MCP
//! server**; connecting the resolved servers is the session lifecycle's job
//! (T-008). The module is shaped on
//! `ragent_plugins::bridge::scanned_plugin_mcp_servers`.

use ragent_config::McpServerConfig;

use crate::descriptor::ConnectorDescriptor;
use crate::store::{StoreDirs, scan_dirs};

/// One MCP server an enabled connector contributes to the session.
#[derive(Debug, Clone)]
pub struct BridgedServer {
    /// The connector that declared the server.
    pub connector_id: String,
    /// The bridged registry id (`<connector-id>.<server>`); this is what the
    /// session registers the server under and what the tool path prefixes tools
    /// with (FR-003, FR-020).
    pub server_id: String,
    /// The transport configuration the session connects with.
    pub config: McpServerConfig,
}

/// Why a bridged server id was refused (FR-033).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeRefusal {
    /// The bridged id collides with an existing `ragent.json` `mcp` key.
    Configured,
    /// The bridged id collides with a server the same connector already declared
    /// (a duplicate server id within one connector).
    Duplicate,
    /// The bridged id collides with another connector's already-bridged server;
    /// carries the id of the connector that owns it.
    OwnedBy(String),
}

impl BridgeRefusal {
    /// A one-line, non-secret explanation of the refusal (FR-033).
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Configured => "collides with a configured ragent.json mcp server".to_string(),
            Self::Duplicate => "is declared twice by the connector".to_string(),
            Self::OwnedBy(owner) => format!("collides with connector '{owner}'"),
        }
    }
}

/// A bridged server id that was refused during resolution (FR-033).
#[derive(Debug, Clone)]
pub struct RefusedServer {
    /// The connector that declared the server.
    pub connector_id: String,
    /// The bridged registry id that was refused.
    pub server_id: String,
    /// Why it was refused.
    pub reason: BridgeRefusal,
}

impl RefusedServer {
    /// A one-line report line naming the refused id and its cause.
    #[must_use]
    pub fn describe(&self) -> String {
        format!("server '{}' {}", self.server_id, self.reason.describe())
    }
}

/// The pure result of resolving connectors into bridged MCP servers.
///
/// Every accepted server appears in [`BridgePlan::servers`] and every refused id
/// appears in [`BridgePlan::refused`], so a caller can connect the accepted
/// servers and report the refused ones (FR-026, FR-033).
#[derive(Debug, Clone, Default)]
pub struct BridgePlan {
    /// Accepted `(bridged_id, config)` servers, in declaration order.
    pub servers: Vec<BridgedServer>,
    /// Refused bridged ids, in declaration order.
    pub refused: Vec<RefusedServer>,
}

impl BridgePlan {
    /// Whether no server was accepted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }

    /// The accepted servers contributed by one connector (FR-026).
    pub fn servers_of<'a>(
        &'a self,
        connector_id: &'a str,
    ) -> impl Iterator<Item = &'a BridgedServer> {
        self.servers
            .iter()
            .filter(move |server| server.connector_id == connector_id)
    }
}

/// Resolve `descriptors` into a [`BridgePlan`] (FR-003, FR-026, FR-033).
///
/// Only a descriptor's expressible servers are bridged; a server ragent cannot
/// speak is skipped here (its capability is reported by the descriptor's
/// `unsupported` labels, FR-025). `configured_ids` is the set of existing
/// `ragent.json` `mcp` keys; a bridged id that clashes with one is refused, as is
/// a bridged id another server in `descriptors` already claimed.
#[must_use]
pub fn resolve_servers<'d, 'c>(
    descriptors: impl IntoIterator<Item = &'d ConnectorDescriptor>,
    configured_ids: impl IntoIterator<Item = &'c str>,
) -> BridgePlan {
    let configured: std::collections::BTreeSet<&str> = configured_ids.into_iter().collect();
    let mut plan = BridgePlan::default();
    // Tracks every accepted bridged id and the connector that owns it.
    let mut owners: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();

    for descriptor in descriptors {
        let connector_id = descriptor.id.as_str();
        for server in descriptor.supported_servers() {
            let server_id = descriptor.bridged_id(&server.id);
            let reason = if configured.contains(server_id.as_str()) {
                Some(BridgeRefusal::Configured)
            } else {
                owners.get(&server_id).map(|owner| {
                    if owner == connector_id {
                        BridgeRefusal::Duplicate
                    } else {
                        BridgeRefusal::OwnedBy(owner.clone())
                    }
                })
            };

            match reason {
                Some(reason) => plan.refused.push(RefusedServer {
                    connector_id: connector_id.to_string(),
                    server_id,
                    reason,
                }),
                None => {
                    owners.insert(server_id.clone(), connector_id.to_string());
                    plan.servers.push(BridgedServer {
                        connector_id: connector_id.to_string(),
                        server_id,
                        config: server.to_mcp_config(),
                    });
                }
            }
        }
    }

    plan
}

/// Scan the connector stores and resolve the **enabled** connectors into a
/// [`BridgePlan`] (FR-003, FR-026, FR-033).
///
/// A disabled connector is inert and contributes nothing (FR-018); a connector
/// whose manifest cannot be parsed is reported by discovery and skipped here.
/// `configured_ids` is the set of existing `ragent.json` `mcp` keys, checked for
/// collision exactly as in [`resolve_servers`]. No MCP server is connected.
#[must_use]
pub fn scanned_bridge<'c>(
    dirs: &StoreDirs,
    configured_ids: impl IntoIterator<Item = &'c str>,
) -> BridgePlan {
    let descriptors = enabled_descriptors(dirs);
    resolve_servers(descriptors.iter(), configured_ids)
}

/// The descriptors of the enabled connectors discovered in `dirs`, in stable id
/// order (FR-001, FR-018).
///
/// Shared by the bridge so the enable/parse filter lives in one place; a disabled
/// connector or one whose manifest cannot be parsed contributes nothing.
fn enabled_descriptors(dirs: &StoreDirs) -> Vec<ConnectorDescriptor> {
    scan_dirs(dirs.clone())
        .into_iter()
        .filter(|connector| connector.enabled)
        .filter_map(|connector| connector.outcome.ok())
        .collect()
}

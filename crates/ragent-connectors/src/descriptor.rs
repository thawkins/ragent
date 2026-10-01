//! Connector descriptor model and validation rules (spec `connectors` T-002;
//! FR-002, FR-025).
//!
//! A *connector* is the user-facing unit of the connector system: a catalogue
//! entry naming an integration (Google Drive, Slack, GitHub, ...) that reaches
//! the outside world through one or more MCP servers. This module defines the
//! normalised [`ConnectorDescriptor`] every command surface renders, so the
//! `/connectors` family and the `ragent connectors` CLI print one wording
//! (FR-002).
//!
//! # Descriptor model
//!
//! A descriptor carries an id, display name, description, category, tags,
//! provenance, an optional authentication shape (plus OAuth scopes and the name
//! of the encrypted credential that satisfies it), a list of MCP server
//! definitions, and a list of recorded unsupported-capability labels.
//!
//! # Validation
//!
//! [`ConnectorDescriptor::validate`] enforces the model's shape:
//!
//! - the id is a single ordinary path component (it becomes the connector store
//!   directory name);
//! - at least one server is declared;
//! - at least one declared server is *expressible* - its transport is one
//!   ragent speaks (`stdio`, `sse`, `http`) and it carries the command or URL
//!   that transport needs (FR-025).
//!
//! A server that cannot be expressed is never dropped: its capability is
//! recorded in [`ConnectorDescriptor::unsupported`] as a labelled
//! unsupported-capability string (FR-025), and the label is reported by
//! `/connectors list`.
//!
//! # Collision predicates
//!
//! The module also owns the predicates later tasks use to refuse a collision
//! rather than silently overwrite an existing definition:
//! [`connector_id_collides`] (FR-027, an install whose id already exists) and
//! [`server_id_collides`] (FR-033, a bridged server id that clashes with a
//! configured `ragent.json` `mcp` key or another connector's bridged server).
//! The bridged id shape (`<connector-id>.<server>`) is owned by
//! [`bridged_server_id`] so the bridge and its collision check cannot drift.

use std::collections::BTreeMap;
use std::fmt;

use ragent_config::{McpServerConfig, McpTransport};
use ragent_types::guard::validate_identifier;
use serde::{Deserialize, Serialize};

use crate::error::ConnectorError;

/// Unsupported-capability label recorded for a server whose id is empty or
/// unusable as a path component.
pub const UNSUP_SERVER_ID: &str = "server id";
/// Unsupported-capability label recorded for a server whose declared transport
/// ragent does not speak (anything other than `stdio`, `sse`, or `http`).
pub const UNSUP_TRANSPORT: &str = "server transport";
/// Unsupported-capability label recorded for a `stdio` server with no command.
pub const UNSUP_COMMAND: &str = "server command";
/// Unsupported-capability label recorded for an `sse`/`http` server with no URL.
pub const UNSUP_URL: &str = "server url";

/// The authentication shape a connector declares (spec `connectors` FR-005,
/// FR-014, FR-022, FR-023).
///
/// The shape determines what a connector needs before it can connect: nothing
/// (`none`), an environment variable in the session (`env`), a stored secret
/// (`token`/`password`), or an OAuth authorization-code exchange (`oauth`, whose
/// granted scopes are carried separately in
/// [`ConnectorDescriptor::auth_scope`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConnectorAuthShape {
    /// The connector needs no authentication (the default).
    #[default]
    None,
    /// Authentication is read from an environment variable in the session.
    Env,
    /// Authentication is a static bearer/API token held in the credential store.
    Token,
    /// Authentication is an OAuth authorization-code exchange.
    Oauth,
    /// Authentication is a username/password pair.
    Password,
}

impl ConnectorAuthShape {
    /// Whether this shape requires a credential (or environment variable)
    /// before the connector may connect (FR-022).
    ///
    /// `none` never does; every other shape does. A shape that requires
    /// authentication but has no valid credential is reported as `needs auth`
    /// and refuses to connect.
    #[must_use]
    pub fn requires_credential(self) -> bool {
        !matches!(self, Self::None)
    }

    /// The lowercase label the reports print (`none`, `env`, `token`, `oauth`,
    /// `password`), matching serde's wire names so config and display cannot
    /// drift.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Env => "env",
            Self::Token => "token",
            Self::Oauth => "oauth",
            Self::Password => "password",
        }
    }
}

impl fmt::Display for ConnectorAuthShape {
    /// Render the label the reports print, exactly as [`Self::label`] does.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Where an installed connector came from (spec `connectors` FR-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConnectorProvenance {
    /// Installed from a catalogue entry (the default).
    #[default]
    Catalogue,
    /// Installed from a local directory or archive.
    Local,
    /// Installed from an `https://` URL.
    Url,
}

impl ConnectorProvenance {
    /// The lowercase label the reports print (`catalogue`, `local`, `url`).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Catalogue => "catalogue",
            Self::Local => "local",
            Self::Url => "url",
        }
    }
}

impl fmt::Display for ConnectorProvenance {
    /// Render the label the reports print, exactly as [`Self::label`] does.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// A connector id: a stable, unique key that is also a single path component
/// (spec `connectors` FR-002).
///
/// The id forms the connector store directory name, so it must be one ordinary
/// path component: `[A-Za-z0-9._-]+`, at most
/// [`MAX_IDENTIFIER_LEN`](ragent_types::guard::MAX_IDENTIFIER_LEN) characters,
/// with no `/` or `\`, no `..`, and not absolute. Construction validates the
/// shape with the shared [`validate_identifier`] guard (ANTIPAT M14) so the
/// identifier rule has one home.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ConnectorId(String);

impl ConnectorId {
    /// Validate and construct a connector id.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectorError::InvalidId`] when `raw` (after trimming) is
    /// empty, too long, or not a single ordinary path component.
    pub fn new(raw: &str) -> Result<Self, ConnectorError> {
        let trimmed = raw.trim();
        validate_identifier(trimmed, "connector id")
            .map_err(|detail| ConnectorError::InvalidId { detail })?;
        Ok(Self(trimmed.to_string()))
    }

    /// The id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConnectorId {
    /// Render the connector id.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for ConnectorId {
    type Err = ConnectorError;

    /// Parse and validate a connector id (see [`ConnectorId::new`]).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for ConnectorId {
    type Error = ConnectorError;

    /// Validate an owned string into a connector id (serde `try_from`).
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl From<ConnectorId> for String {
    /// Unwrap the id into its owned string form (serde `into`).
    fn from(id: ConnectorId) -> Self {
        id.0
    }
}

/// One MCP server definition declared by a connector.
///
/// The definition is the connector-system spelling of the fields
/// [`McpServerConfig`] needs; [`Self::is_supported`] decides whether ragent can
/// express it, and [`Self::to_mcp_config`] performs the (infallible)
/// conversion. A definition that is not supported is never dropped - the owning
/// [`ConnectorDescriptor::validate`] records a labelled unsupported capability
/// (FR-025).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorServer {
    /// Server id, unique within the connector. Prefixed with the connector id
    /// by the bridge so two connectors cannot collide on a server name.
    pub id: String,
    /// Transport name (`stdio` default, `sse`, `http`); an unknown value makes
    /// the server unsupported.
    #[serde(default = "default_transport_label")]
    pub transport: String,
    /// Executable to launch for a `stdio` transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Arguments passed to `command`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Environment variables injected into the server process.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// Endpoint URL for an `sse`/`http` transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Extra HTTP headers for an `sse`/`http` transport.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
}

/// Hand-written `Debug` for [`ConnectorServer`].
///
/// `env` and `headers` carry credential material (API keys, bearer tokens), so
/// the derived `Debug` would leak them into every `tracing::debug!("{server:?}")`
/// (see `ANTIPAT.md` M3.14). The values are replaced with presence flags and key
/// names only, matching `ragent_plugins::manifest::PluginMcpServer`.
impl fmt::Debug for ConnectorServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectorServer")
            .field("id", &self.id)
            .field("transport", &self.transport)
            .field("command", &self.command)
            .field("args", &self.args)
            .field("env_keys", &self.env.keys().collect::<Vec<_>>())
            .field("url", &self.url)
            .field("header_keys", &self.headers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ConnectorServer {
    /// Whether ragent can express this server as an [`McpServerConfig`] (FR-025).
    ///
    /// A server is supported when its transport is one ragent speaks (`stdio`,
    /// `sse`, or `http`, case-insensitively) and it carries what that transport
    /// needs: a `command` for `stdio`, a `url` for `sse`/`http`.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        match self.transport_kind() {
            Some(McpTransport::Stdio) => self.command.as_deref().is_some_and(non_empty),
            Some(McpTransport::Sse | McpTransport::Http) => {
                self.url.as_deref().is_some_and(non_empty)
            }
            None => false,
        }
    }

    /// The parsed transport, or `None` when ragent does not speak it.
    #[must_use]
    pub fn transport_kind(&self) -> Option<McpTransport> {
        match self.transport.trim().to_ascii_lowercase().as_str() {
            "" | "stdio" => Some(McpTransport::Stdio),
            "sse" => Some(McpTransport::Sse),
            "http" => Some(McpTransport::Http),
            _ => None,
        }
    }

    /// The recorded unsupported-capability label for this server, if it is not
    /// expressible as an MCP server (FR-025).
    ///
    /// The label names the first unexpressible aspect, so a caller can retain
    /// and report it. Returns `None` for a supported server.
    #[must_use]
    pub fn unsupported_label(&self) -> Option<&'static str> {
        if validate_identifier(self.id.trim(), "server id").is_err() {
            return Some(UNSUP_SERVER_ID);
        }
        match self.transport_kind() {
            None => Some(UNSUP_TRANSPORT),
            Some(McpTransport::Stdio) if !self.command.as_deref().is_some_and(non_empty) => {
                Some(UNSUP_COMMAND)
            }
            Some(McpTransport::Sse | McpTransport::Http)
                if !self.url.as_deref().is_some_and(non_empty) =>
            {
                Some(UNSUP_URL)
            }
            Some(_) => None,
        }
    }

    /// Convert this definition into the [`McpServerConfig`] the existing
    /// [`McpClient`](../../ragent-agent/src/mcp/mod.rs) connects with.
    ///
    /// The conversion is infallible: a server whose transport ragent does not
    /// speak falls back to `stdio` (the transport the connector bridge maps
    /// unknown names to), so this method is only meaningful for a server for
    /// which [`Self::is_supported`] returned `true`. Always gate a call on
    /// `is_supported` first.
    #[must_use]
    pub fn to_mcp_config(&self) -> McpServerConfig {
        let type_ = self.transport_kind().unwrap_or(McpTransport::Stdio);
        McpServerConfig {
            type_,
            command: self.command.clone(),
            args: self.args.clone(),
            env: self.env.clone().into_iter().collect(),
            url: self.url.clone(),
            headers: self.headers.clone().into_iter().collect(),
            disabled: false,
            notification: ragent_config::trigger::McpNotificationMode::None,
        }
    }
}

/// A normalised connector descriptor (spec `connectors` FR-002, FR-025).
///
/// Every command surface renders this one shape: the `/connectors` family and
/// the `ragent connectors` CLI print the same wording because they print the
/// same descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorDescriptor {
    /// Stable, unique connector id (also the connector store directory name).
    pub id: ConnectorId,
    /// Human-readable display name.
    pub name: String,
    /// One-line description.
    #[serde(default)]
    pub description: String,
    /// Category, the value the category filter compares against (FR-039).
    #[serde(default)]
    pub category: String,
    /// Free-form tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Where the connector came from (catalogue origin or install source).
    #[serde(default)]
    pub source: String,
    /// Install provenance (catalogue, local, url).
    #[serde(default)]
    pub provenance: ConnectorProvenance,
    /// How the connector is authenticated (FR-005, FR-022).
    #[serde(default)]
    pub auth: ConnectorAuthShape,
    /// OAuth scopes the connector requests; empty for every other shape.
    #[serde(default)]
    pub auth_scope: Vec<String>,
    /// Name of the credential the auth shape resolves (FR-005). The *name*, not
    /// the secret value; a secret is never written here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
    /// One or more MCP server definitions the connector wraps.
    #[serde(default)]
    pub servers: Vec<ConnectorServer>,
    /// Unsupported-capability labels recorded during validation (FR-025). Retained
    /// so the labels are reported rather than silently dropped.
    #[serde(default)]
    pub unsupported: Vec<String>,
}

impl ConnectorDescriptor {
    /// Validate the descriptor's shape (FR-002, FR-025).
    ///
    /// On success, [`ConnectorDescriptor::unsupported`] holds one labelled entry
    /// for every server that cannot be expressed as an MCP server. A descriptor
    /// with a server list in which *nothing* is expressible fails with
    /// [`ConnectorError::NoSupportedServers`], carrying the recorded labels so
    /// the caller can report them.
    ///
    /// # Errors
    ///
    /// - [`ConnectorError::InvalidId`] when the id is not a single ordinary path
    ///   component (already enforced by [`ConnectorId`] construction, re-checked
    ///   for a descriptor deserialised from a manifest).
    /// - [`ConnectorError::NoServers`] when no server is declared.
    /// - [`ConnectorError::NoSupportedServers`] when every declared server is
    ///   unexpressible.
    pub fn validate(&mut self) -> Result<(), ConnectorError> {
        // Re-check the id: a descriptor may arrive deserialised from a manifest
        // rather than constructed through `ConnectorId::new`.
        validate_identifier(self.id.as_str(), "connector id")
            .map_err(|detail| ConnectorError::InvalidId { detail })?;

        if self.servers.is_empty() {
            return Err(ConnectorError::NoServers {
                id: self.id.to_string(),
            });
        }

        self.unsupported.clear();
        let mut supported = 0usize;
        for server in &self.servers {
            match server.unsupported_label() {
                None => supported += 1,
                Some(label) => self
                    .unsupported
                    .push(format!("server '{}': {label}", server.id)),
            }
        }

        if supported == 0 {
            return Err(ConnectorError::NoSupportedServers {
                id: self.id.to_string(),
                reason: self.unsupported.join("; "),
            });
        }
        Ok(())
    }

    /// The servers ragent can express as MCP servers (FR-025).
    ///
    /// An unexpressible server is skipped here but stays visible through
    /// [`Self::unsupported`].
    #[must_use]
    pub fn supported_servers(&self) -> Vec<&ConnectorServer> {
        self.servers.iter().filter(|s| s.is_supported()).collect()
    }

    /// Whether the connector declares any expressible server (FR-025).
    #[must_use]
    pub fn has_supported_server(&self) -> bool {
        self.servers.iter().any(ConnectorServer::is_supported)
    }

    /// The bridged server id for one of this connector's servers
    /// (`<connector-id>.<server-id>`), matching the plugin MCP bridge shape
    /// (spec `plugins` FR-030).
    #[must_use]
    pub fn bridged_id(&self, server_id: &str) -> String {
        bridged_server_id(self.id.as_str(), server_id)
    }
}

/// The bridged server id for a connector's server: `<connector-id>.<server-id>`
/// (FR-003, FR-033).
///
/// Both halves may themselves contain dots, so a caller that needs to recover
/// the parts must keep the connector id alongside the bridged id rather than
/// splitting the string.
#[must_use]
pub fn bridged_server_id(connector_id: &str, server_id: &str) -> String {
    format!("{connector_id}.{server_id}")
}

/// Whether installing a connector under `id` collides with an existing
/// connector (FR-027).
///
/// `existing_ids` is every connector id already known to the store.
#[must_use]
pub fn connector_id_collides<'a>(
    id: &str,
    existing_ids: impl IntoIterator<Item = &'a str>,
) -> bool {
    existing_ids.into_iter().any(|existing| existing == id)
}

/// Whether a bridged server id collides with an already-registered server id
/// (FR-033).
///
/// `existing_ids` is every server id already present: the `ragent.json` `mcp`
/// keys (unprefixed) plus every other connector's bridged ids
/// (`<connector-id>.<server>`). The later registration is rejected and reported
/// rather than silently overwriting the existing server.
#[must_use]
pub fn server_id_collides<'a>(
    bridged_id: &str,
    existing_ids: impl IntoIterator<Item = &'a str>,
) -> bool {
    existing_ids
        .into_iter()
        .any(|existing| existing == bridged_id)
}

/// The default transport label a server with no declared transport uses.
fn default_transport_label() -> String {
    "stdio".to_string()
}

/// Whether a string is non-empty after trimming.
fn non_empty(value: &str) -> bool {
    !value.trim().is_empty()
}

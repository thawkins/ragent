//! Connector catalogue entry model (spec `connectors` T-005; FR-002, FR-025).
//!
//! A [`ConnectorEntry`] is the *pre-validation* catalogue record: the id, name,
//! description, category, tags, source, authentication shape, and MCP server
//! definitions one catalogue document carries, before [`ConnectorDescriptor`]
//! validation decides whether the entry is expressible.
//!
//! The catalogue fetch ([`crate::fetch`]) and every [`ConnectorProvider`]
//! ([`crate::provider`]) normalise a vendor document into a list of entries;
//! [`ConnectorEntry::to_descriptor`] then turns one entry into the validated
//! [`ConnectorDescriptor`] the rest of the system renders. Keeping the two steps
//! apart is what lets an entry that cannot be expressed be *skipped and counted*
//! rather than failing the whole document (FR-025).
//!
//! # Tolerant field reading
//!
//! [`ConnectorEntry::from_value`] is the one reader every provider shares. It
//! accepts ragent's own catalogue shape and, tolerantly, the vendor shapes the
//! shipped catalogues use:
//!
//! - the id is the explicit `id` when that is already a readable key, otherwise
//!   the `slug`, otherwise the display name, and the raw `id` only as the last
//!   resort; a catalogue that ships UUID `id`s (the Claude connector directory)
//!   is therefore named by its slug (`google-drive`) rather than a UUID, while a
//!   catalogue whose `id` is human-readable (`echo`) keeps that stable id; the
//!   display name falls back from `name` to `display_name`;
//! - the description falls back from `description` to `one_liner` (the Claude
//!   feed's one-line summary);
//! - the category is read from `category` or the first member of `categories`;
//! - tags are read from `tags`/`keywords` or, failing those, `categories` and
//!   `works_with`;
//! - the server list is read from a `servers` array, an `mcpServers` object, or a
//!   single `remote` object (the Claude connector-directory form);
//! - a server's transport is read from `transport` or `type` and normalised
//!   through [`normalise_transport`], so a vendor spelling
//!   (`streamable-http`, `streamable_http`) maps onto the transport ragent
//!   speaks while a genuinely foreign transport (`grpc`) stays unexpressible
//!   (FR-025).
//!
//! [`ConnectorProvider`]: crate::provider::ConnectorProvider
//! [`crate::fetch`]: crate::fetch
//! [`crate::provider`]: crate::provider

use std::collections::BTreeMap;

use serde_json::{Map, Value};
use url::Url;

use ragent_types::guard::validate_identifier;

use crate::descriptor::{
    ConnectorAuthShape, ConnectorDescriptor, ConnectorId, ConnectorProvenance, ConnectorServer,
};
use crate::error::ConnectorError;

/// A refusal or skip from [`ConnectorEntry::from_value`].
///
/// Every variant is contained and reportable; the catalogue transform counts a
/// rejected entry rather than failing the document (FR-025).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConnectorEntryError {
    /// The catalogue item was not a JSON object.
    #[error("catalogue entry is not a JSON object")]
    NotAnObject,

    /// A required field was missing, empty, or wrong-typed. The field name is
    /// carried so the skip can be reported.
    #[error("catalogue entry is missing the `{0}` field")]
    EmptyField(&'static str),
}

/// One catalogue entry, before validation (FR-002, FR-025).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorEntry {
    /// Stable catalogue key, also the connector store directory name.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// One-line description.
    pub description: String,
    /// Category, the value the category filter compares against (FR-039).
    pub category: String,
    /// Free-form tags.
    pub tags: Vec<String>,
    /// Catalogue origin or install source.
    pub source: String,
    /// Authentication shape the connector declares.
    pub auth: ConnectorAuthShape,
    /// OAuth scopes the connector requests; empty for every other shape.
    pub auth_scope: Vec<String>,
    /// Name of the credential satisfying the auth shape; never the value.
    pub credential: Option<String>,
    /// One or more MCP server definitions.
    pub servers: Vec<ConnectorServer>,
}

impl ConnectorEntry {
    /// Normalise one parsed catalogue item into a [`ConnectorEntry`] (FR-002,
    /// FR-025).
    ///
    /// `origin` is the endpoint URL the document was fetched from; it is the
    /// default `source` when the entry carries none. See the module docs for the
    /// tolerated field aliases.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectorEntryError::NotAnObject`] when the item is not a JSON
    /// object and [`ConnectorEntryError::EmptyField`] when the id (or name) or the
    /// server list is missing or empty. An entry whose servers are all
    /// unexpressible is still returned here; the caller discovers that at
    /// [`ConnectorEntry::to_descriptor`] time and skips it (FR-025).
    pub fn from_value(item: &Value, origin: &Url) -> Result<Self, ConnectorEntryError> {
        let map = item.as_object().ok_or(ConnectorEntryError::NotAnObject)?;

        let name = optional_str(map, "name").or_else(|| optional_str(map, "display_name"));
        // The id preference is: the explicit `id` when it is already a readable
        // key, otherwise the `slug`, otherwise the display name, with the raw
        // `id` accepted only as the last resort. A catalogue that ships UUID
        // `id`s (the Claude connector directory) is therefore named by its
        // readable slug or display name; a catalogue whose `id` is already
        // human-readable (`echo`, `google-drive`) keeps that stable id. Each
        // candidate must be a usable path component.
        let explicit_id = optional_str(map, "id");
        let slug = optional_str(map, "slug");
        let candidate = if readable_connector_id(explicit_id.as_deref()) {
            explicit_id
        } else if usable_connector_id(slug.as_deref()) {
            slug
        } else if usable_connector_id(name.as_deref()) {
            name.clone()
        } else {
            None
        };
        let id = candidate.ok_or(ConnectorEntryError::EmptyField("id"))?;
        let name = name.unwrap_or_else(|| id.clone());

        let servers = parse_servers(map)?;
        let auth = auth_shape(map);
        let auth_scope = if map.contains_key("auth_scope") {
            string_array(map, "auth_scope")
        } else {
            string_array(map, "scopes")
        };

        Ok(Self {
            id,
            name,
            description: optional_str(map, "description")
                .or_else(|| optional_str(map, "one_liner"))
                .unwrap_or_default(),
            category: category_of(map),
            tags: collect_tags(map),
            source: optional_str(map, "source").unwrap_or_else(|| origin.as_str().to_string()),
            auth,
            auth_scope,
            credential: optional_str(map, "credential"),
            servers,
        })
    }

    /// Turn this entry into a validated [`ConnectorDescriptor`] (FR-002, FR-025).
    ///
    /// `provenance` is the install provenance the catalogue implies
    /// ([`ConnectorProvenance::Catalogue`] for a catalogue entry).
    ///
    /// # Errors
    ///
    /// Returns the [`ConnectorError`] [`ConnectorDescriptor::validate`] reports:
    /// [`ConnectorError::InvalidId`] for an unusable id,
    /// [`ConnectorError::NoServers`] when no server is declared, and
    /// [`ConnectorError::NoSupportedServers`] when every declared server is
    /// unexpressible (the skip case, FR-025).
    pub fn to_descriptor(
        &self,
        provenance: ConnectorProvenance,
    ) -> Result<ConnectorDescriptor, ConnectorError> {
        let mut descriptor = ConnectorDescriptor {
            id: ConnectorId::new(&self.id)?,
            name: self.name.clone(),
            description: self.description.clone(),
            category: self.category.clone(),
            tags: self.tags.clone(),
            source: self.source.clone(),
            provenance,
            auth: self.auth,
            auth_scope: self.auth_scope.clone(),
            credential: self.credential.clone(),
            servers: self.servers.clone(),
            unsupported: Vec::new(),
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    /// Validate this entry as a [`ConnectorProvenance::Catalogue`] descriptor.
    ///
    /// # Errors
    ///
    /// As [`ConnectorEntry::to_descriptor`].
    pub fn to_catalogue_descriptor(&self) -> Result<ConnectorDescriptor, ConnectorError> {
        self.to_descriptor(ConnectorProvenance::Catalogue)
    }
}

/// Normalise a vendor transport spelling onto the transport ragent speaks
/// (FR-025).
///
/// `streamable-http`/`streamable_http`/`streamablehttp`/`http` all normalise to
/// `http`; `sse` and `stdio` are already ragent's names. Anything else (for
/// example `grpc`) is returned unchanged, so it stays unexpressible and the
/// entry is skipped with a recorded label rather than silently mistranslated.
#[must_use]
pub fn normalise_transport(token: &str) -> String {
    match token.trim().to_ascii_lowercase().as_str() {
        "" | "stdio" => "stdio".to_string(),
        "sse" => "sse".to_string(),
        "http" | "streamable-http" | "streamable_http" | "streamablehttp" | "streamable" => {
            "http".to_string()
        }
        other => other.to_string(),
    }
}

/// Parse the server list from a `servers` array, an `mcpServers` object, or a
/// single `remote` object (FR-025).
fn parse_servers(map: &Map<String, Value>) -> Result<Vec<ConnectorServer>, ConnectorEntryError> {
    if let Some(Value::Array(items)) = map.get("servers") {
        let mut servers = Vec::with_capacity(items.len());
        for item in items {
            servers.push(parse_server(item, None)?);
        }
        if servers.is_empty() {
            return Err(ConnectorEntryError::EmptyField("servers"));
        }
        return Ok(servers);
    }
    if let Some(Value::Object(mcp)) = map.get("mcpServers") {
        let mut servers = Vec::with_capacity(mcp.len());
        for (name, value) in mcp {
            servers.push(parse_server(value, Some(name))?);
        }
        if servers.is_empty() {
            return Err(ConnectorEntryError::EmptyField("servers"));
        }
        return Ok(servers);
    }
    if let Some(Value::Object(remote)) = map.get("remote") {
        // The Claude connector-directory form: a single remote server. The server
        // id is a stable `main` (the feed's `server_label` is a hostname, not a
        // usable identifier); the connector id already namespaces the bridge.
        return Ok(vec![parse_server(
            &Value::Object(remote.clone()),
            Some("main"),
        )?]);
    }
    // A top-level `command`/`url` describes a single server directly.
    if map.contains_key("command") || map.contains_key("url") {
        return Ok(vec![parse_server(&Value::Object(map.clone()), None)?]);
    }
    Err(ConnectorEntryError::EmptyField("servers"))
}

/// Parse one server definition, accepting `transport` or `type` for the transport
/// name, normalising the transport, and defaulting an unnamed server to
/// `fallback_id` (or `main`).
fn parse_server(
    item: &Value,
    fallback_id: Option<&str>,
) -> Result<ConnectorServer, ConnectorEntryError> {
    let map = item.as_object().ok_or(ConnectorEntryError::NotAnObject)?;
    let id = optional_str(map, "id")
        .or_else(|| fallback_id.map(str::to_string))
        .or_else(|| optional_str(map, "name"))
        .unwrap_or_else(|| "main".to_string());
    let transport = optional_str(map, "transport")
        .or_else(|| optional_str(map, "type"))
        .map_or_else(|| "stdio".to_string(), |token| normalise_transport(&token));
    Ok(ConnectorServer {
        id,
        transport,
        command: optional_str(map, "command"),
        args: string_array(map, "args"),
        env: string_map(map, "env"),
        url: optional_str(map, "url").or_else(|| first_url_option(map)),
        headers: string_map(map, "headers"),
    })
}

/// The first `url_options[].url` of a remote server, when the direct `url` is
/// absent (the Claude feed's multi-region form).
fn first_url_option(map: &Map<String, Value>) -> Option<String> {
    map.get("url_options")
        .and_then(Value::as_array)
        .and_then(|options| {
            options.iter().find_map(|option| {
                option
                    .as_object()
                    .and_then(|o| o.get("url"))
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_string)
            })
        })
}

/// The authentication shape an entry declares (FR-005, FR-022).
///
/// Reads an explicit `auth` token when present; otherwise infers from the Claude
/// feed's `remote` posture (`is_authless: true` or `auth_posture: "no_auth"`
/// means `none`, a required posture means `oauth`).
fn auth_shape(map: &Map<String, Value>) -> ConnectorAuthShape {
    if let Some(token) = optional_str(map, "auth") {
        return match token.trim().to_ascii_lowercase().as_str() {
            "env" => ConnectorAuthShape::Env,
            "token" => ConnectorAuthShape::Token,
            "oauth" => ConnectorAuthShape::Oauth,
            "password" => ConnectorAuthShape::Password,
            _ => ConnectorAuthShape::None,
        };
    }
    let remote = map.get("remote").and_then(Value::as_object);
    if let Some(authless) = remote
        .and_then(|r| r.get("is_authless"))
        .and_then(Value::as_bool)
    {
        return if authless {
            ConnectorAuthShape::None
        } else {
            ConnectorAuthShape::Oauth
        };
    }
    match remote
        .and_then(|r| r.get("auth_posture"))
        .and_then(Value::as_str)
    {
        Some(posture) if posture.eq_ignore_ascii_case("no_auth") => ConnectorAuthShape::None,
        Some(_) => ConnectorAuthShape::Oauth,
        None => ConnectorAuthShape::None,
    }
}

/// The category an entry declares: an explicit `category`, else the first member
/// of a `categories` array (FR-039).
fn category_of(map: &Map<String, Value>) -> String {
    optional_str(map, "category").unwrap_or_else(|| {
        string_array(map, "categories")
            .into_iter()
            .next()
            .unwrap_or_default()
    })
}

/// Whether `candidate` can name a connector: present, non-blank, and a usable
/// single path component (see [`validate_identifier`]).
fn usable_connector_id(candidate: Option<&str>) -> bool {
    candidate.is_some_and(|value| validate_identifier(value, "connector id").is_ok())
}

/// Whether an explicit catalogue `id` is already a readable connector key, so an
/// entry with a human id (`echo`, `google-drive`, `needs-token`) keeps that
/// stable id rather than being renamed to its display name.
///
/// A bare UUID (the Claude connector directory's opaque key) is *not* readable,
/// so such an entry is named by its `slug` or display name instead; a UUID that
/// happens to fall through (no slug, a display name that is not a usable path
/// component) is still accepted as the last resort.
fn readable_connector_id(candidate: Option<&str>) -> bool {
    usable_connector_id(candidate) && !candidate.is_some_and(is_uuid)
}

/// Whether `value` is a canonical 8-4-4-4-12 hyphenated hexadecimal UUID.
fn is_uuid(value: &str) -> bool {
    let mut groups = value.split('-');
    let mut expected = [8usize, 4, 4, 4, 12].into_iter();
    groups.all(|group| expected.next() == Some(group.len()))
        && expected.next().is_none()
        && value.chars().all(|c| c == '-' || c.is_ascii_hexdigit())
}

/// An optional non-empty string field.
fn optional_str(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
}

/// Collect tags from `tags`/`keywords`, falling back to `categories` and
/// `works_with` so a vendor feed's classification still yields tags.
fn collect_tags(map: &Map<String, Value>) -> Vec<String> {
    for key in ["tags", "keywords"] {
        let tags = string_array(map, key);
        if !tags.is_empty() {
            return tags;
        }
    }
    let mut tags = string_array(map, "categories");
    for tag in string_array(map, "works_with") {
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    tags
}

/// Read an array of strings from `key`, skipping empty and non-string members.
fn string_array(map: &Map<String, Value>, key: &str) -> Vec<String> {
    match map.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// Read an object of string-to-string pairs from `key`, skipping non-string
/// members.
fn string_map(map: &Map<String, Value>, key: &str) -> BTreeMap<String, String> {
    match map.get(key) {
        Some(Value::Object(entries)) => entries
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect(),
        _ => BTreeMap::new(),
    }
}

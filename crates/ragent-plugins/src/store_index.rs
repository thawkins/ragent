//! Plugin store registry and store-index entry model (spec `pluginstores`
//! T-002; FR-001, FR-024).
//!
//! This module owns the two parts of the store-browser domain that touch no
//! network:
//!
//! - the **store registry** ([`StoreCatalog`]): the Codex and Claude stores
//!   ([`StoreKind`]), each addressed by a validated `https` store-index
//!   endpoint ([`StoreEndpoint`]). Every endpoint enters the registry through
//!   [`StoreEndpoint::parse`], which refuses anything that is not an absolute
//!   `https` URL, so a non-`https` endpoint can never reach the fetch (FR-024).
//! - the **store-index entry model** ([`StoreEntry`]): the per-plugin record a
//!   store index carries. `id`, `name`, `version`, and `source` are required;
//!   every other field is optional and tolerated when absent (FR-003).
//!
//! [`StoreEntry::from_value`] converts one parsed JSON object into a
//! [`StoreEntry`], returning a [`StoreEntryError`] for a missing required field
//! or a wrong-typed value, so the fetch ([`crate::store_fetch`]) can skip and
//! count malformed entries without panicking (FR-025). The compiled default
//! endpoints ([`DEFAULT_CODEX_STORE_URL`], [`DEFAULT_CLAUDE_STORE_URL`]) are the
//! single source of the out-of-the-box endpoints (FR-027, FR-029, NFR-001).
//! [`StoreKind::effective_endpoint`] performs the config-over-default resolution
//! (FR-027, FR-028, FR-029, NFR-002): a non-empty `plugins.stores.<name>.url`
//! wins wholesale, otherwise the compiled default for that store is used, and the
//! result is always run through the `https` scheme guard.

use reqwest::Url;
use serde::{Deserialize, Serialize};

/// The compiled-in default store-index URL for the Codex store (FR-027,
/// FR-029, NFR-001).
///
/// This is the single source of the Codex out-of-the-box endpoint: when no
/// non-empty `plugins.stores.codex.url` is configured, the resolver (T-015)
/// falls back to this literal. It is OpenAI's official plugin marketplace
/// (`openai/plugins`), served as the repo-scoped `.agents/plugins/marketplace.json`
/// document. It is an absolute `https` URL, so it passes the
/// [`StoreEndpoint::parse`] scheme guard exactly like a configured endpoint
/// (FR-029). No other module may carry a default store-endpoint literal
/// (NFR-001).
pub const DEFAULT_CODEX_STORE_URL: &str =
    "https://raw.githubusercontent.com/openai/plugins/main/.agents/plugins/marketplace.json";

/// The compiled-in default store-index URL for the Claude store (FR-027,
/// FR-029, NFR-001).
///
/// This is the single source of the Claude out-of-the-box endpoint: when no
/// non-empty `plugins.stores.claude.url` is configured, the resolver (T-015)
/// falls back to this literal. It is Anthropic's official plugin directory
/// (`anthropics/claude-plugins-official`), served as the
/// `.claude-plugin/marketplace.json` document. It is an absolute `https` URL, so
/// it passes the [`StoreEndpoint::parse`] scheme guard exactly like a configured
/// endpoint (FR-029). No other module may carry a default store-endpoint literal
/// (NFR-001).
pub const DEFAULT_CLAUDE_STORE_URL: &str = "https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json";

/// The two plugin stores the browser knows about (FR-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoreKind {
    /// The OpenAI Codex plugin store.
    Codex,
    /// The Claude Code / Claude Desktop plugin store.
    Claude,
}

impl StoreKind {
    /// Both stores, in display order (Codex first, then Claude).
    pub const ALL: [Self; 2] = [Self::Codex, Self::Claude];

    /// The store's human-readable label, used in panel titles and reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
        }
    }

    /// The store's command token (`codex` / `claude`), matching the
    /// `/plugins <token>` subcommand and the `plugins.stores.<token>` key.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }

    /// Resolve a store from its command token, case-insensitively.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            _ => None,
        }
    }

    /// The raw `plugins.stores.<name>.url` override for this store, when one is
    /// configured (FR-019). Returns the value verbatim, including an empty
    /// string, so the caller can decide whether an empty override falls back to
    /// the compiled default (FR-028). No default is applied here.
    #[must_use]
    pub fn configured_url(self, stores: &ragent_config::PluginStoresConfig) -> Option<&str> {
        let endpoint = match self {
            Self::Codex => stores.codex.as_ref(),
            Self::Claude => stores.claude.as_ref(),
        };
        endpoint.and_then(|e| e.url.as_deref())
    }

    /// The compiled default store-index URL for this store (FR-027, FR-029).
    ///
    /// The single accessor for the two default literals declared by T-014, so
    /// resolution never repeats a literal (NFR-001).
    #[must_use]
    pub const fn default_url(self) -> &'static str {
        match self {
            Self::Codex => DEFAULT_CODEX_STORE_URL,
            Self::Claude => DEFAULT_CLAUDE_STORE_URL,
        }
    }

    /// Resolve the effective store-index endpoint for this store together with
    /// where it came from (FR-031).
    ///
    /// Applies the same resolution rules as [`StoreKind::effective_endpoint`]
    /// and additionally reports the provenance, so `/plugins stores` can tag
    /// each endpoint `config` or `default` (FR-031). A non-empty
    /// `plugins.stores.<name>.url` override is [`EndpointSource::Config`]
    /// (FR-028); an absent block, an absent endpoint, or an
    /// empty/whitespace-only override is [`EndpointSource::Default`] (FR-027).
    /// The chosen value is parsed through the [`StoreEndpoint`] `https` guard
    /// exactly as on the [`StoreKind::effective_endpoint`] path (FR-024, FR-029).
    pub fn effective_endpoint_with_source(
        self,
        stores: &ragent_config::PluginStoresConfig,
    ) -> Result<(StoreEndpoint, EndpointSource), StoreEndpointError> {
        let (raw, source) = match self.configured_url(stores) {
            Some(configured) if !configured.trim().is_empty() => {
                (configured, EndpointSource::Config)
            }
            _ => (self.default_url(), EndpointSource::Default),
        };
        Ok((StoreEndpoint::parse(raw)?, source))
    }

    /// Resolve the effective store-index endpoint for this store (FR-027,
    /// FR-028, FR-029, NFR-002).
    ///
    /// A non-empty `plugins.stores.<name>.url` override wins wholesale (FR-028);
    /// an absent block, an absent endpoint, or an empty/whitespace-only override
    /// falls back to the compiled default ([`StoreKind::default_url`]) with no
    /// error, so a missing `plugins.stores` block never fails a launch (FR-027,
    /// FR-030, NFR-002). The chosen value is parsed through the
    /// [`StoreEndpoint`] `https` guard, so the default path is guarded exactly
    /// like the configured path (FR-029); a configured non-`https` or malformed
    /// value is refused here (FR-024). Called once per launch, so a config change
    /// takes effect without a rebuild (NFR-002).
    pub fn effective_endpoint(
        self,
        stores: &ragent_config::PluginStoresConfig,
    ) -> Result<StoreEndpoint, StoreEndpointError> {
        self.effective_endpoint_with_source(stores)
            .map(|(endpoint, _)| endpoint)
    }
}

/// Where a store's effective endpoint came from (FR-031).
///
/// Produced by [`StoreKind::effective_endpoint_with_source`] so the
/// `/plugins stores` report can tell the user whether a
/// `plugins.stores.<name>.url` override is in effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointSource {
    /// A non-empty `plugins.stores.<name>.url` override (FR-028).
    Config,
    /// The compiled default endpoint (FR-027).
    Default,
}

impl EndpointSource {
    /// The lowercase tag used by the `/plugins stores` report (FR-031).
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Default => "default",
        }
    }
}

impl std::fmt::Display for StoreKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A validated store-index endpoint: an absolute `https` URL with a host.
///
/// The inner URL is private so an endpoint can only be built through
/// [`StoreEndpoint::parse`] (or [`TryFrom`]), which enforces the `https` scheme
/// guard (FR-024, FR-029). A non-`https` endpoint is never representable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreEndpoint(Url);

impl StoreEndpoint {
    /// Parse and validate a store-index endpoint (FR-024).
    ///
    /// Accepts only an absolute `https` URL that carries a host. An empty or
    /// whitespace-only string yields [`StoreEndpointError::Empty`]; any other
    /// failure names the offending scheme or the malformed input.
    pub fn parse(raw: &str) -> Result<Self, StoreEndpointError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(StoreEndpointError::Empty);
        }
        let url = Url::parse(trimmed).map_err(|e| StoreEndpointError::Malformed(e.to_string()))?;
        if url.scheme() != "https" {
            return Err(StoreEndpointError::NotHttps {
                scheme: url.scheme().to_string(),
            });
        }
        if url.host_str().is_none() {
            return Err(StoreEndpointError::MissingHost);
        }
        Ok(Self(url))
    }

    /// The parsed URL.
    #[must_use]
    pub const fn as_url(&self) -> &Url {
        &self.0
    }

    /// The endpoint as its string form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for StoreEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl TryFrom<&str> for StoreEndpoint {
    type Error = StoreEndpointError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

/// A refused store-index endpoint (FR-024).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreEndpointError {
    /// The endpoint string was empty or whitespace-only.
    #[error("store endpoint is empty")]
    Empty,

    /// The endpoint string could not be parsed as a URL.
    #[error("store endpoint is not a valid URL: {0}")]
    Malformed(String),

    /// The endpoint used a scheme other than `https`.
    #[error("store endpoint must use https, not {scheme}")]
    NotHttps {
        /// The refused scheme.
        scheme: String,
    },

    /// The endpoint parsed but carried no host.
    #[error("store endpoint has no host")]
    MissingHost,
}

/// The registry of plugin stores and their resolved endpoints (FR-001).
///
/// Holds one optional endpoint per [`StoreKind`]. Endpoints are inserted only
/// after passing the [`StoreEndpoint`] `https` guard, so the registry can never
/// carry a non-`https` endpoint. The compiled default endpoints and the
/// config-over-default resolution (T-014, T-015) populate this via
/// [`StoreCatalog::insert`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreCatalog {
    codex: Option<StoreEndpoint>,
    claude: Option<StoreEndpoint>,
}

impl StoreCatalog {
    /// An empty registry, ready for [`StoreCatalog::insert`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            codex: None,
            claude: None,
        }
    }

    /// Register (or replace) the endpoint for one store.
    pub fn insert(&mut self, kind: StoreKind, endpoint: StoreEndpoint) {
        match kind {
            StoreKind::Codex => self.codex = Some(endpoint),
            StoreKind::Claude => self.claude = Some(endpoint),
        }
    }

    /// The endpoint registered for a store, if any.
    #[must_use]
    pub fn endpoint(&self, kind: StoreKind) -> Option<&StoreEndpoint> {
        match kind {
            StoreKind::Codex => self.codex.as_ref(),
            StoreKind::Claude => self.claude.as_ref(),
        }
    }

    /// The two store kinds this registry tracks, in display order (FR-001).
    #[must_use]
    pub const fn kinds(&self) -> [StoreKind; 2] {
        StoreKind::ALL
    }

    /// Whether no endpoint has been registered yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.codex.is_none() && self.claude.is_none()
    }
}

/// One plugin record from a store index (FR-003).
///
/// `id`, `name`, `version`, and `source` are required; `description`,
/// `dialect`, `tags`, and `homepage` are optional and default when absent, so an
/// index may omit them (FR-003).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreEntry {
    /// Stable plugin identifier (used for the installed-set check).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Plugin version string.
    pub version: String,
    /// Install source passed to the install pipeline (directory, archive, or
    /// `https` URL). Never trusted beyond the existing `add` guards (FR-024).
    pub source: String,
    /// Short description; empty when the index omits it.
    #[serde(default)]
    pub description: String,
    /// Declared dialect token (`codex` / `claude`), when the index supplies one.
    #[serde(default)]
    pub dialect: Option<String>,
    /// Search and grouping tags; empty when the index omits them.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Store homepage URL, when the index supplies one.
    #[serde(default)]
    pub homepage: Option<String>,
}

impl StoreEntry {
    /// Build a [`StoreEntry`] from one parsed JSON object (FR-003, FR-025).
    ///
    /// Optional fields may be absent. A non-object value, a missing required
    /// field, a wrong-typed value, or an empty required string returns a
    /// [`StoreEntryError`]; the caller skips and counts the entry rather than
    /// panicking.
    pub fn from_value(value: &serde_json::Value) -> Result<Self, StoreEntryError> {
        if !value.is_object() {
            return Err(StoreEntryError::NotAnObject);
        }
        let entry: Self = serde::Deserialize::deserialize(value)
            .map_err(|e| StoreEntryError::Invalid(e.to_string()))?;
        for (field, text) in [
            ("id", entry.id.as_str()),
            ("name", entry.name.as_str()),
            ("version", entry.version.as_str()),
            ("source", entry.source.as_str()),
        ] {
            if text.trim().is_empty() {
                return Err(StoreEntryError::EmptyField(field));
            }
        }
        Ok(entry)
    }
}

/// A store-index entry that could not be accepted (FR-025).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreEntryError {
    /// The value was not a JSON object.
    #[error("store entry is not a JSON object")]
    NotAnObject,

    /// A required field was absent or wrong-typed.
    #[error("store entry rejected: {0}")]
    Invalid(String),

    /// A required field was present but empty.
    #[error("store entry has an empty required field: {0}")]
    EmptyField(&'static str),

    /// The entry's install source could not be normalised into a form the
    /// install pipeline accepts (vendor marketplaces carry source shapes that
    /// have no ragent equivalent). The entry is skipped, not fatal (FR-025).
    #[error("store entry rejected: unsupported source: {0}")]
    Unsupported(String),
}

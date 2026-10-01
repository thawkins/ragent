//! Connector catalogue registry and endpoint resolution (spec `connectors`
//! T-016; FR-034, FR-035, FR-037, FR-038).
//!
//! This module owns the part of the catalogue domain that touches no network:
//! the **compiled default endpoint** and the **config-over-default resolver**
//! that chooses which catalogue endpoint a fetch will contact.
//!
//! - [`DEFAULT_CLAUDE_CATALOGUE_URL`] is the single compiled-in source of the
//!   Claude connector-catalogue endpoint (FR-034, FR-038). No other module may
//!   carry a second copy of that literal.
//! - [`CatalogueKind::effective_endpoint_with_source`] performs the resolution
//!   (FR-035): a non-empty `connectors.stores.<name>.url` override wins
//!   wholesale, otherwise the compiled default is used, and the result is
//!   always run through the [`CatalogueEndpoint`] absolute-`https`-with-host
//!   guard (FR-037).
//! - [`EndpointSource`] records which of the two paths produced the endpoint, so
//!   `/connectors stores` can tag each row `config` or `default` (FR-036) from
//!   the same value the fetch uses.
//!
//! Resolution is a pure, offline function: it reads configuration and the
//! compiled constant, and performs no network access (NFR-003). It is called
//! once per launch, so editing `connectors.stores.claude.url` takes effect on
//! the next launch with no rebuild (NFR-002).
//!
//! The module is modelled on `ragent_plugins::store_index` (FR-025, NFR-001 of
//! spec `pluginstores`) so the two default-endpoint disciplines read the same
//! way.

use ragent_config::ConnectorsConfig;
use url::Url;

/// The compiled-in default Claude connector-catalogue endpoint (FR-034,
/// FR-038).
///
/// This is the single source of the Claude connector-catalogue endpoint: when
/// no non-empty `connectors.stores.claude.url` is configured, the resolver
/// ([`CatalogueKind::effective_endpoint_with_source`]) falls back to this
/// literal (FR-035). It is Anthropic's connector-directory feed - the same feed
/// the in-app Claude connector catalog is served from - as a JSON `servers`
/// document. It is an absolute `https` URL, so it passes the
/// [`CatalogueEndpoint::parse`] scheme guard exactly like a configured endpoint
/// (FR-037). No other module may carry a default connector-endpoint literal
/// (FR-038, NFR-001).
pub const DEFAULT_CLAUDE_CATALOGUE_URL: &str = "https://api.anthropic.com/api/directory/servers";

/// A connector catalogue ragent ships a compiled default endpoint for (FR-034).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatalogueKind {
    /// The Claude connector catalogue.
    Claude,
}

impl CatalogueKind {
    /// Every catalogue the connector system knows about, in display order.
    pub const ALL: [Self; 1] = [Self::Claude];

    /// The catalogue's human-readable label, used in reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
        }
    }

    /// The catalogue's command token, matching the `connectors.stores.<token>`
    /// configuration key and the `/connectors stores` row.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Claude => "claude",
        }
    }

    /// The compiled default endpoint for this catalogue (FR-034, FR-038).
    ///
    /// The single accessor for the compiled default literal, so resolution
    /// never repeats the literal (FR-038).
    #[must_use]
    pub const fn default_url(self) -> &'static str {
        match self {
            Self::Claude => DEFAULT_CLAUDE_CATALOGUE_URL,
        }
    }

    /// The raw `connectors.stores.<name>.url` override for this catalogue, when
    /// one is configured (FR-035).
    ///
    /// Returns `None` for an absent `connectors` block, an absent `stores`
    /// block, an absent entry, or an empty/whitespace-only `url`, so every
    /// "no real override" case resolves to the compiled default.
    #[must_use]
    pub fn configured_url(self, connectors: &ConnectorsConfig) -> Option<&str> {
        connectors.catalogue_url(self.token())
    }

    /// Resolve the effective catalogue endpoint for this catalogue together
    /// with where it came from (FR-035, FR-037, NFR-002, NFR-003).
    ///
    /// A non-empty `connectors.stores.<name>.url` override is
    /// [`EndpointSource::Config`]; an absent block, an absent endpoint, or an
    /// empty/whitespace-only override falls back to the compiled default
    /// ([`CatalogueKind::default_url`]) as [`EndpointSource::Default`] with no
    /// error (FR-035). The chosen value is parsed through the
    /// [`CatalogueEndpoint`] `https` guard, so the default path is guarded
    /// exactly like the configured path (FR-037): a non-`https` or malformed
    /// value is refused naming the offending scheme or input, never silently
    /// substituted.
    ///
    /// The function performs no network access (NFR-003) and is called once per
    /// launch, so a configuration edit takes effect without a rebuild
    /// (NFR-002).
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogueEndpointError`] when the chosen value is not an
    /// absolute `https` URL carrying a host (FR-037).
    pub fn effective_endpoint_with_source(
        self,
        connectors: &ConnectorsConfig,
    ) -> Result<(CatalogueEndpoint, EndpointSource), CatalogueEndpointError> {
        let (raw, source) = match self.configured_url(connectors) {
            Some(configured) if !configured.trim().is_empty() => {
                (configured, EndpointSource::Config)
            }
            _ => (self.default_url(), EndpointSource::Default),
        };
        Ok((CatalogueEndpoint::parse(raw)?, source))
    }

    /// Resolve the effective catalogue endpoint for this catalogue
    /// (FR-035, FR-037).
    ///
    /// The provenance-discarding form of
    /// [`CatalogueKind::effective_endpoint_with_source`], for call sites that
    /// only need the endpoint.
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogueEndpointError`] when the chosen value is not an
    /// absolute `https` URL carrying a host (FR-037).
    pub fn effective_endpoint(
        self,
        connectors: &ConnectorsConfig,
    ) -> Result<CatalogueEndpoint, CatalogueEndpointError> {
        self.effective_endpoint_with_source(connectors)
            .map(|(endpoint, _)| endpoint)
    }
}

impl std::fmt::Display for CatalogueKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Where a catalogue's effective endpoint came from (FR-036).
///
/// Produced by [`CatalogueKind::effective_endpoint_with_source`] so the
/// `/connectors stores` report can tag each row from the same value the fetch
/// uses, and the two can never disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointSource {
    /// A non-empty `connectors.stores.<name>.url` override (FR-035).
    Config,
    /// The compiled default endpoint (FR-034).
    Default,
}

impl EndpointSource {
    /// The lowercase tag used by the `/connectors stores` report (FR-036).
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Default => "default",
        }
    }
}

/// A validated connector catalogue endpoint: an absolute `https` URL with a
/// host (FR-037).
///
/// The inner URL is private so an endpoint can only be built through
/// [`CatalogueEndpoint::parse`] (or [`TryFrom`]), which enforces the `https`
/// guard. A non-`https` or host-less endpoint is never representable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueEndpoint(Url);

impl CatalogueEndpoint {
    /// Parse and validate a connector catalogue endpoint (FR-037).
    ///
    /// Accepts only an absolute `https` URL that carries a host. An empty or
    /// whitespace-only string yields [`CatalogueEndpointError::Empty`]; any
    /// other failure names the offending scheme or the malformed input.
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogueEndpointError`] when the input is empty, is not a
    /// parseable URL, is not `https`, or carries no host.
    pub fn parse(raw: &str) -> Result<Self, CatalogueEndpointError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(CatalogueEndpointError::Empty);
        }
        let url =
            Url::parse(trimmed).map_err(|e| CatalogueEndpointError::Malformed(e.to_string()))?;
        if url.scheme() != "https" {
            return Err(CatalogueEndpointError::NotHttps {
                scheme: url.scheme().to_string(),
            });
        }
        if url.host_str().is_none() {
            return Err(CatalogueEndpointError::MissingHost);
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

impl std::fmt::Display for CatalogueEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl TryFrom<&str> for CatalogueEndpoint {
    type Error = CatalogueEndpointError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

/// A refused connector catalogue endpoint (FR-037).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogueEndpointError {
    /// The endpoint string was empty or whitespace-only.
    #[error("catalogue endpoint is empty")]
    Empty,

    /// The endpoint string could not be parsed as a URL.
    #[error("catalogue endpoint is not a valid URL: {0}")]
    Malformed(String),

    /// The endpoint used a scheme other than `https`.
    #[error("catalogue endpoint must use https, not {scheme}")]
    NotHttps {
        /// The refused scheme.
        scheme: String,
    },

    /// The endpoint parsed but carried no host.
    #[error("catalogue endpoint has no host")]
    MissingHost,
}

/// The resolved catalogue endpoints a session will contact, keyed by
/// [`CatalogueKind`] (FR-035).
///
/// Holds one optional endpoint per catalogue. Endpoints are inserted only after
/// passing the [`CatalogueEndpoint`] `https` guard, so the registry can never
/// carry a non-`https` endpoint. The resolver populates this once per launch
/// (NFR-002) so every consumer - the catalogue fetch and the
/// `/connectors stores` report - reads the same resolved value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogueCatalog {
    claude: Option<CatalogueEndpoint>,
}

impl CatalogueCatalog {
    /// An empty registry, ready for [`CatalogueCatalog::insert`].
    #[must_use]
    pub const fn new() -> Self {
        Self { claude: None }
    }

    /// Record the resolved endpoint for `kind`, replacing any previous value.
    pub fn insert(&mut self, kind: CatalogueKind, endpoint: CatalogueEndpoint) {
        match kind {
            CatalogueKind::Claude => self.claude = Some(endpoint),
        }
    }

    /// The resolved endpoint for `kind`, when one has been recorded.
    #[must_use]
    pub const fn endpoint(&self, kind: CatalogueKind) -> Option<&CatalogueEndpoint> {
        match kind {
            CatalogueKind::Claude => self.claude.as_ref(),
        }
    }

    /// Whether the registry holds no endpoint at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.claude.is_none()
    }
}

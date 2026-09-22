//! Injectable store-index fetch seam and offline fixture fetcher (spec
//! `pluginstores` T-019; FR-032, FR-033, FR-037, NFR-003, NFR-004).
//!
//! The store browser resolves an effective endpoint per launch ([`StoreKind`])
//! and then downloads its store index. That download is the one step in the
//! store-browser domain that touches the network. This module makes that step a
//! **trait** ([`StoreIndexFetcher`]) so the browser can be driven by an
//! alternative implementation:
//!
//! - [`NetworkStoreFetcher`] is the **production-transparent** default
//!   ([`default_fetcher`]): it forwards verbatim to
//!   [`crate::store_fetch::fetch_index`], so the real launch path still performs
//!   the live HTTPS request and nothing about production behaviour changes.
//! - [`FixtureStoreFetcher`] is the **offline** implementation: it maps each
//!   endpoint URL to in-memory index bytes and returns the parsed
//!   [`StoreIndex`] directly, performing no network I/O at all. It is the seam
//!   the default-endpoint tests use to exercise the compiled default endpoints
//!   for both stores deterministically and without live access (FR-037, NFR-003).
//!
//! [`FixtureStoreFetcher::with_default_endpoints`] seeds one fixture index per
//! compiled default endpoint ([`DEFAULT_CODEX_STORE_URL`],
//! [`DEFAULT_CLAUDE_STORE_URL`]), so a test can prove the browser works out of
//! the box against *both* stores with no `plugins.stores` block present
//! (FR-032, NFR-004). Each fixture index carries at least one entry whose id
//! matches an installed fixture under `assets/plugins/fixtures/` and one entry
//! that omits the optional fields, so the fetch tolerance path is exercised too
//! (FR-003).
//!
//! The fixture fetcher honours the same byte ceiling the network path does, so a
//! too-large fixture surfaces the same contained [`StoreError::TooLarge`] a real
//! oversized body would (FR-025). Every implementation is fallible and contained:
//! no fetch path panics on an unknown endpoint, an oversized fixture, or a
//! malformed body.
//!
//! [`DEFAULT_CODEX_STORE_URL`]: crate::store_index::DEFAULT_CODEX_STORE_URL
//! [`DEFAULT_CLAUDE_STORE_URL`]: crate::store_index::DEFAULT_CLAUDE_STORE_URL

use std::collections::HashMap;
use std::sync::Arc;

use crate::store_fetch::{FetchLimits, StoreError, StoreIndex};
use crate::store_index::{StoreEndpoint, StoreKind};
use crate::store_provider::provider_for;

/// The injectable seam the store browser drives to obtain a store index.
///
/// The production implementation ([`NetworkStoreFetcher`]) downloads over HTTPS;
/// the test implementation ([`FixtureStoreFetcher`]) returns in-memory fixture
/// bytes. Making this a trait is what lets the default-endpoint tests run
/// offline (FR-037, NFR-003) without changing the live launch path.
///
/// The `kind` parameter is threaded through so the fetcher parses through the
/// store's provider ([`crate::store_provider`]); a fixture can therefore also key
/// its bytes on the store it is answering for.
///
/// Implementers must be `Send + Sync` because the browser hands the fetcher to
/// an off-loop worker so the event loop never stalls (FR-016, FR-026). Every
/// failure is returned as a [`StoreError`]; no implementation panics.
pub trait StoreIndexFetcher: Send + Sync {
    /// Fetch and parse the store index served by `endpoint`, bounded by `limits`.
    ///
    /// Returns the parsed [`StoreIndex`] on success or a contained
    /// [`StoreError`] on any failure (network, status, timeout, oversized body,
    /// malformed JSON, or an unknown offline fixture endpoint).
    fn fetch_index(
        &self,
        kind: StoreKind,
        endpoint: &StoreEndpoint,
        limits: &FetchLimits,
    ) -> Result<StoreIndex, StoreError>;
}

impl<F> StoreIndexFetcher for F
where
    F: Fn(StoreKind, &StoreEndpoint, &FetchLimits) -> Result<StoreIndex, StoreError> + Send + Sync,
{
    fn fetch_index(
        &self,
        kind: StoreKind,
        endpoint: &StoreEndpoint,
        limits: &FetchLimits,
    ) -> Result<StoreIndex, StoreError> {
        self(kind, endpoint, limits)
    }
}

/// The production store-index fetcher: the real HTTPS download (FR-024, FR-037).
///
/// A transparent wrapper around [`crate::store_fetch::fetch_index`] so the
/// default launch path is unchanged; it is the value [`default_fetcher`] returns.
#[derive(Debug, Clone, Copy, Default)]
pub struct NetworkStoreFetcher;

impl StoreIndexFetcher for NetworkStoreFetcher {
    fn fetch_index(
        &self,
        kind: StoreKind,
        endpoint: &StoreEndpoint,
        limits: &FetchLimits,
    ) -> Result<StoreIndex, StoreError> {
        crate::store_fetch::fetch_index(kind, endpoint, limits)
    }
}

/// The production fetcher used by the live launch path: a [`NetworkStoreFetcher`]
/// behind the sharing type the browser stores (FR-037).
#[must_use]
pub fn default_fetcher() -> Arc<dyn StoreIndexFetcher> {
    Arc::new(NetworkStoreFetcher)
}

/// An offline store-index fetcher that serves in-memory fixture bytes per
/// endpoint, performing no network I/O (FR-037, NFR-003).
///
/// The map is keyed by the endpoint's exact URL string, so a fixture is served
/// only for the endpoint it was registered under. An endpoint with no registered
/// fixture yields a contained [`StoreError::Network`] rather than attempting a
/// real request, which is what keeps the default-endpoint tests hermetic.
#[derive(Debug, Clone, Default)]
pub struct FixtureStoreFetcher {
    indexes: HashMap<String, Vec<u8>>,
}

impl FixtureStoreFetcher {
    /// An empty fixture fetcher with no endpoints registered.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register the raw index bytes served for `endpoint`.
    #[must_use]
    pub fn with_index(mut self, endpoint: &str, bytes: impl Into<Vec<u8>>) -> Self {
        self.indexes.insert(endpoint.to_string(), bytes.into());
        self
    }

    /// Seed one fixture index per compiled default endpoint
    /// ([`StoreKind::default_url`]) for both stores (FR-032, NFR-004).
    ///
    /// After this call the fetcher answers a fetch of either compiled default
    /// endpoint with a fixture index, so the browser can be exercised out of the
    /// box with no `plugins.stores` block and no live access.
    #[must_use]
    pub fn with_default_endpoints(mut self) -> Self {
        for kind in StoreKind::ALL {
            self.indexes
                .insert(kind.default_url().to_string(), default_fixture_bytes(kind));
        }
        self
    }

    /// The endpoint URLs this fetcher can serve, sorted, for test introspection.
    #[must_use]
    pub fn endpoints(&self) -> Vec<&str> {
        let mut endpoints: Vec<&str> = self.indexes.keys().map(String::as_str).collect();
        endpoints.sort_unstable();
        endpoints
    }
}

impl StoreIndexFetcher for FixtureStoreFetcher {
    fn fetch_index(
        &self,
        kind: StoreKind,
        endpoint: &StoreEndpoint,
        limits: &FetchLimits,
    ) -> Result<StoreIndex, StoreError> {
        let Some(bytes) = self.indexes.get(endpoint.as_str()) else {
            return Err(StoreError::Network {
                detail: format!("no offline fixture registered for {endpoint}"),
            });
        };
        // Mirror the network path's byte ceiling so an oversized fixture surfaces
        // the same contained error a real oversized body would (FR-025).
        if bytes.len() as u64 > limits.max_bytes {
            return Err(StoreError::TooLarge {
                limit: limits.max_bytes,
            });
        }
        provider_for(kind).parse_index(bytes, endpoint.as_url())
    }
}

/// A fixture store-index document for `kind`'s compiled default endpoint
/// (FR-032, NFR-004).
///
/// Each fixture carries two entries: one whose id matches an installed fixture
/// under `assets/plugins/fixtures/` (so the installed-colour path has data), and
/// one that omits every optional field (so the tolerant parse path is exercised).
/// The sources are absolute URLs, matching the spec's store-index format.
fn default_fixture_bytes(kind: StoreKind) -> Vec<u8> {
    match kind {
        StoreKind::Codex => br#"{
  "store": "codex",
  "plugins": [
    {
      "id": "codex-weather",
      "name": "Codex Weather",
      "version": "1.2.0",
      "description": "Weather lookups for the agent",
      "source": "https://raw.githubusercontent.com/thawkins/ragent/main/assets/plugins/fixtures/codex-weather",
      "tags": ["weather", "http"]
    },
    {
      "id": "codex-time",
      "name": "Codex Time",
      "version": "0.9.1",
      "source": "https://example.org/codex-time.zip"
    }
  ]
}"#
        .to_vec(),
        StoreKind::Claude => br#"{
  "store": "claude",
  "plugins": [
    {
      "id": "claude-todo",
      "name": "Claude Todo",
      "version": "0.3.0",
      "description": "Todo list helpers",
      "source": "https://raw.githubusercontent.com/thawkins/ragent/main/assets/plugins/fixtures/claude-todo",
      "dialect": "claude",
      "tags": ["todo"]
    },
    {
      "id": "claude-time",
      "name": "Claude Time",
      "version": "0.1.0",
      "source": "https://example.org/claude-time.zip"
    }
  ]
}"#
        .to_vec(),
    }
}

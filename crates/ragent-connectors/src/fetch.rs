//! Catalogue fetch, cache, and bounded download (spec `connectors` T-005;
//! FR-024, FR-025, FR-031).
//!
//! This module turns a validated [`CatalogueEndpoint`] into a
//! [`ConnectorCatalogue`]. It is the only place in the catalogue domain that
//! performs network I/O, and it is deliberately fallible end-to-end:
//!
//! - **HTTPS only** (FR-024): the endpoint is a [`CatalogueEndpoint`], which can
//!   only be built from an absolute `https` URL, and redirects are followed only
//!   to other `https` locations, so a plaintext downgrade is impossible.
//! - **Bounded** (FR-031): [`CatalogueLimits::timeout`] bounds the wall-clock and
//!   [`CatalogueLimits::max_bytes`] bounds the body; a body that exceeds the cap
//!   aborts the fetch with [`ConnectorError::CatalogueTooLarge`].
//! - **Contained** (FR-031): every failure is a [`ConnectorError`] variant; the
//!   fetch never panics on a network error, a non-2xx status, malformed JSON, an
//!   oversized body, or a malformed entry.
//! - **Cache-safe** (FR-031): a malformed, unrecognised, or oversize document
//!   aborts before the cache is written, so any previously cached catalogue is
//!   left untouched. A successful fetch within [`CatalogueLimits::cache_ttl`] is
//!   served from the cache with no network round trip.
//! - **Skip-tolerant** (FR-025): an entry that cannot be normalised or whose
//!   servers are all unexpressible is counted in [`ConnectorCatalogue::skipped`]
//!   rather than failing the whole document.
//!
//! [`fetch_bytes`] is blocking by design (`reqwest::blocking`); the caller runs
//! it on an off-loop task so the event loop keeps animating. [`CatalogueFetcher`]
//! is the injectable seam the default-endpoint tests use to feed fixture bytes
//! with no live request (NFR-003).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::Digest as _;

use ragent_config::ConnectorStoresConfig;

use crate::descriptor::ConnectorDescriptor;
use crate::error::ConnectorError;
use crate::provider::{ParsedCatalogue, parse_catalogue, provider_for};
use crate::store_index::{CatalogueEndpoint, CatalogueKind};

/// Read/write chunk size for streaming a catalogue body into memory.
const STREAM_CHUNK_BYTES: usize = 65_536;

/// Maximum `https`-only redirect hops followed before a fetch is refused.
const MAX_REDIRECT_HOPS: usize = 5;

/// Name of the on-disk catalogue cache directory.
pub const CACHE_DIR: &str = "catalogues";

/// Wall-clock timeout, body-size ceiling, and cache TTL for one catalogue fetch.
///
/// Populated from `connectors.stores` configuration (`timeout_ms`,
/// `max_index_bytes`, `cache_ttl_secs`) and handed to [`fetch_bytes`] and
/// [`fetch_catalogue`]. A zero timeout or byte ceiling falls back to the compiled
/// default so a misconfiguration cannot make every fetch fail; a zero cache TTL
/// disables the cache (FR-031).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogueLimits {
    /// Per-request wall-clock timeout.
    pub timeout: Duration,
    /// Maximum accepted catalogue body size in bytes.
    pub max_bytes: u64,
    /// Time-to-live for a cached catalogue; `Duration::ZERO` disables the cache.
    pub cache_ttl: Duration,
}

impl CatalogueLimits {
    /// Compiled default timeout: 10 seconds.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(10_000);
    /// Compiled default body ceiling: 2 MiB.
    pub const DEFAULT_MAX_BYTES: u64 = 2 * 1024 * 1024;
    /// Compiled default cache TTL: 1 hour.
    pub const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(3_600);

    /// Build limits from a timeout, a byte ceiling, and a cache TTL.
    #[must_use]
    pub const fn new(timeout: Duration, max_bytes: u64, cache_ttl: Duration) -> Self {
        Self {
            timeout,
            max_bytes,
            cache_ttl,
        }
    }

    /// Build limits from millisecond/second configuration values, mapping a zero
    /// timeout or byte ceiling onto the compiled default and treating a zero
    /// cache TTL as "cache disabled".
    #[must_use]
    pub const fn from_config(timeout_ms: u64, max_bytes: u64, cache_ttl_secs: u64) -> Self {
        Self {
            timeout: if timeout_ms == 0 {
                Self::DEFAULT_TIMEOUT
            } else {
                Duration::from_millis(timeout_ms)
            },
            max_bytes: if max_bytes == 0 {
                Self::DEFAULT_MAX_BYTES
            } else {
                max_bytes
            },
            cache_ttl: Duration::from_secs(cache_ttl_secs),
        }
    }
}

impl Default for CatalogueLimits {
    fn default() -> Self {
        Self::new(
            Self::DEFAULT_TIMEOUT,
            Self::DEFAULT_MAX_BYTES,
            Self::DEFAULT_CACHE_TTL,
        )
    }
}

impl From<&ConnectorStoresConfig> for CatalogueLimits {
    fn from(cfg: &ConnectorStoresConfig) -> Self {
        Self::from_config(cfg.timeout_ms, cfg.max_index_bytes, cfg.cache_ttl_secs)
    }
}

/// A fetched and normalised catalogue plus the endpoint it came from (FR-025).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorCatalogue {
    /// The endpoint URL the catalogue was fetched from.
    pub endpoint: String,
    /// The connectors that survived normalisation and validation (FR-025).
    pub connectors: Vec<ConnectorDescriptor>,
    /// How many catalogue entries were skipped as malformed or unexpressible
    /// (FR-025).
    pub skipped: usize,
    /// Unix timestamp (seconds) the catalogue was fetched, used for the cache TTL.
    pub fetched_at: u64,
}

/// The result of [`fetch_catalogue`]: the catalogue and whether it was served
/// from the cache (FR-031).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedCatalogue {
    /// The catalogue.
    pub catalogue: ConnectorCatalogue,
    /// Whether the catalogue was served from the on-disk cache rather than a
    /// live fetch.
    pub from_cache: bool,
}

/// The injectable seam the catalogue search drives to obtain a parsed catalogue
/// (NFR-003).
///
/// The production implementation ([`NetworkCatalogueFetcher`]) downloads over
/// HTTPS; the test implementation ([`FixtureCatalogueFetcher`]) returns in-memory
/// fixture bytes. Making this a trait is what lets the default-endpoint tests run
/// offline without changing the live launch path.
pub trait CatalogueFetcher: Send + Sync {
    /// Fetch and parse the catalogue served by `endpoint`, bounded by `limits`.
    ///
    /// # Errors
    ///
    /// Returns a contained [`ConnectorError`] on any failure (network, status,
    /// timeout, oversized body, malformed JSON, or an unknown offline fixture
    /// endpoint).
    fn fetch(
        &self,
        kind: CatalogueKind,
        endpoint: &CatalogueEndpoint,
        limits: &CatalogueLimits,
    ) -> Result<ParsedCatalogue, ConnectorError>;
}

impl<F> CatalogueFetcher for F
where
    F: Fn(
            CatalogueKind,
            &CatalogueEndpoint,
            &CatalogueLimits,
        ) -> Result<ParsedCatalogue, ConnectorError>
        + Send
        + Sync,
{
    fn fetch(
        &self,
        kind: CatalogueKind,
        endpoint: &CatalogueEndpoint,
        limits: &CatalogueLimits,
    ) -> Result<ParsedCatalogue, ConnectorError> {
        self(kind, endpoint, limits)
    }
}

/// The production catalogue fetcher: the real HTTPS download (FR-024, FR-031).
#[derive(Debug, Clone, Copy, Default)]
pub struct NetworkCatalogueFetcher;

impl CatalogueFetcher for NetworkCatalogueFetcher {
    fn fetch(
        &self,
        kind: CatalogueKind,
        endpoint: &CatalogueEndpoint,
        limits: &CatalogueLimits,
    ) -> Result<ParsedCatalogue, ConnectorError> {
        let bytes = fetch_bytes(endpoint, limits)?;
        parse_catalogue(&*provider_for(kind), &bytes, endpoint.as_url())
    }
}

/// The production catalogue fetcher (FR-024).
#[must_use]
pub fn default_fetcher() -> Box<dyn CatalogueFetcher> {
    Box::new(NetworkCatalogueFetcher)
}

/// An offline catalogue fetcher that serves in-memory fixture bytes per endpoint,
/// performing no network I/O (NFR-003).
#[derive(Debug, Clone, Default)]
pub struct FixtureCatalogueFetcher {
    indexes: HashMap<String, Vec<u8>>,
}

impl FixtureCatalogueFetcher {
    /// An empty fixture fetcher with no endpoints registered.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register the raw catalogue bytes served for `endpoint`.
    #[must_use]
    pub fn with_index(mut self, endpoint: &str, bytes: impl Into<Vec<u8>>) -> Self {
        self.indexes.insert(endpoint.to_string(), bytes.into());
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

impl CatalogueFetcher for FixtureCatalogueFetcher {
    fn fetch(
        &self,
        kind: CatalogueKind,
        endpoint: &CatalogueEndpoint,
        limits: &CatalogueLimits,
    ) -> Result<ParsedCatalogue, ConnectorError> {
        let Some(bytes) = self.indexes.get(endpoint.as_str()) else {
            return Err(ConnectorError::CatalogueFetch {
                detail: format!("no offline fixture registered for {endpoint}"),
            });
        };
        // Mirror the network path's byte ceiling so an oversized fixture
        // surfaces the same contained error a real oversized body would.
        if bytes.len() as u64 > limits.max_bytes {
            return Err(ConnectorError::CatalogueTooLarge {
                limit: limits.max_bytes,
            });
        }
        parse_catalogue(&*provider_for(kind), bytes, endpoint.as_url())
    }
}

/// Download the raw catalogue bytes from a validated endpoint (FR-031).
///
/// Enforces the timeout and the byte ceiling; returns a [`ConnectorError`] for a
/// non-2xx status, a refused redirect, an oversized body, a timeout, or a network
/// failure.
///
/// # Errors
///
/// Returns [`ConnectorError::CatalogueFetch`] for a transport/status failure and
/// [`ConnectorError::CatalogueTooLarge`] for an oversized body.
pub fn fetch_bytes(
    endpoint: &CatalogueEndpoint,
    limits: &CatalogueLimits,
) -> Result<Vec<u8>, ConnectorError> {
    let client = reqwest::blocking::Client::builder()
        .redirect(https_only_redirects())
        .timeout(limits.timeout)
        .build()
        .map_err(|e| ConnectorError::CatalogueFetch {
            detail: format!("http client: {e}"),
        })?;

    let response = client.get(endpoint.as_url().clone()).send().map_err(|e| {
        ConnectorError::CatalogueFetch {
            detail: if e.is_timeout() {
                format!("request timed out after {} ms", limits.timeout.as_millis())
            } else {
                e.to_string()
            },
        }
    })?;

    let status = response.status();
    if status.is_redirection() {
        // Only reached when a redirect was refused (non-https or too many hops).
        return Err(ConnectorError::CatalogueFetch {
            detail: format!("redirect refused (HTTP {})", status.as_u16()),
        });
    }
    if !status.is_success() {
        return Err(ConnectorError::CatalogueFetch {
            detail: format!("HTTP {}", status.as_u16()),
        });
    }
    if let Some(len) = response.content_length()
        && len > limits.max_bytes
    {
        return Err(ConnectorError::CatalogueTooLarge {
            limit: limits.max_bytes,
        });
    }
    read_capped(response, limits.max_bytes)
}

/// Read `reader` fully, aborting once more than `max_bytes` has been read
/// (FR-031).
///
/// Pure and reader-agnostic so the size cap is unit-testable offline with a
/// [`std::io::Cursor`]; [`fetch_bytes`] passes the HTTP response as the reader.
///
/// # Errors
///
/// Returns [`ConnectorError::CatalogueFetch`] on a read failure and
/// [`ConnectorError::CatalogueTooLarge`] when the body exceeds `max_bytes`.
pub fn read_capped(
    mut reader: impl std::io::Read,
    max_bytes: u64,
) -> Result<Vec<u8>, ConnectorError> {
    let mut bytes = Vec::new();
    let mut chunk = vec![0_u8; STREAM_CHUNK_BYTES];
    loop {
        let read = reader
            .read(&mut chunk)
            .map_err(|e| ConnectorError::CatalogueFetch {
                detail: e.to_string(),
            })?;
        if read == 0 {
            break;
        }
        if (bytes.len() + read) as u64 > max_bytes {
            return Err(ConnectorError::CatalogueTooLarge { limit: max_bytes });
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    Ok(bytes)
}

/// A redirect policy that follows only `https` targets, up to five hops
/// (FR-024).
fn https_only_redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.url().scheme() != "https" {
            attempt.stop()
        } else if attempt.previous().len() >= MAX_REDIRECT_HOPS {
            attempt.error("too many redirects")
        } else {
            attempt.follow()
        }
    })
}

/// An on-disk catalogue cache keyed by endpoint URL and catalogue (FR-031).
///
/// A cached catalogue is stored as `<root>/<catalogue-token>-<sha256>.json` and
/// served for [`CatalogueLimits::cache_ttl`] seconds. The cache is written only
/// after a document has been fetched *and* parsed successfully, so a malformed,
/// unrecognised, or oversize document leaves any previously cached catalogue
/// untouched (FR-031).
#[derive(Debug, Clone)]
pub struct CatalogueCache {
    root: PathBuf,
}

impl CatalogueCache {
    /// A cache rooted at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The default cache under the user-global state dir
    /// (`~/.config/ragent/catalogues/`), or `None` when it cannot be resolved.
    #[must_use]
    pub fn default_cache() -> Option<Self> {
        ragent_config::user_dirs::global_state_dir().map(|dir| Self::new(dir.join(CACHE_DIR)))
    }

    /// The cache directory root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The on-disk path a catalogue is cached at (FR-031).
    #[must_use]
    pub fn path_for(&self, kind: CatalogueKind, endpoint: &str) -> PathBuf {
        self.root
            .join(format!("{}-{}.json", kind.token(), cache_key(endpoint)))
    }

    /// Load the cached catalogue for `endpoint` when it is within `ttl` (FR-031).
    ///
    /// Returns `None` when the cache is disabled (`ttl` zero), the file is
    /// absent, unreadable, or unparseable, the file names a different endpoint,
    /// or the entry is older than `ttl`. `now` is the current Unix time in
    /// seconds, threaded in so the TTL check is pure and testable.
    #[must_use]
    pub fn load(
        &self,
        kind: CatalogueKind,
        endpoint: &str,
        ttl: Duration,
        now: u64,
    ) -> Option<ConnectorCatalogue> {
        if ttl.is_zero() {
            return None;
        }
        let path = self.path_for(kind, endpoint);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => return None, // absent or unreadable cache: a miss, not an error
        };
        let catalogue: ConnectorCatalogue = match serde_json::from_slice(&bytes) {
            Ok(catalogue) => catalogue,
            Err(error) => {
                tracing::debug!(error = %error, path = %path.display(), "connector catalogue cache unreadable");
                return None;
            }
        };
        if catalogue.endpoint != endpoint {
            return None;
        }
        if now.saturating_sub(catalogue.fetched_at) > ttl.as_secs() {
            return None;
        }
        Some(catalogue)
    }

    /// Write `catalogue` to the cache (FR-031).
    ///
    /// # Errors
    ///
    /// Returns [`ConnectorError::Io`] when the cache directory or file cannot be
    /// written.
    pub fn store(
        &self,
        kind: CatalogueKind,
        catalogue: &ConnectorCatalogue,
    ) -> Result<(), ConnectorError> {
        std::fs::create_dir_all(&self.root).map_err(ConnectorError::io)?;
        let mut bytes = serde_json::to_vec_pretty(catalogue).map_err(ConnectorError::io)?;
        bytes.push(b'\n');
        std::fs::write(self.path_for(kind, &catalogue.endpoint), bytes).map_err(ConnectorError::io)
    }
}

/// Fetch a catalogue, serving it from `cache` when fresh (FR-031).
///
/// The cache is consulted first; on a miss the `fetcher` is invoked and, on
/// success, the result is written to the cache. A cache-write failure is logged
/// and does not fail the fetch, because the catalogue in hand is valid.
///
/// # Errors
///
/// Returns the [`ConnectorError`] the `fetcher` reports; a failed fetch leaves
/// any cached catalogue untouched (FR-031).
pub fn fetch_catalogue(
    fetcher: &dyn CatalogueFetcher,
    kind: CatalogueKind,
    endpoint: &CatalogueEndpoint,
    limits: &CatalogueLimits,
    cache: Option<&CatalogueCache>,
    now: u64,
) -> Result<FetchedCatalogue, ConnectorError> {
    if let Some(cache) = cache
        && let Some(catalogue) = cache.load(kind, endpoint.as_str(), limits.cache_ttl, now)
    {
        return Ok(FetchedCatalogue {
            catalogue,
            from_cache: true,
        });
    }

    let parsed = fetcher.fetch(kind, endpoint, limits)?;
    let catalogue = ConnectorCatalogue {
        endpoint: endpoint.as_str().to_string(),
        connectors: parsed.connectors,
        skipped: parsed.skipped,
        fetched_at: now,
    };
    if let Some(cache) = cache
        && let Err(error) = cache.store(kind, &catalogue)
    {
        tracing::debug!(error = %error, "connector catalogue cache write skipped");
    }
    Ok(FetchedCatalogue {
        catalogue,
        from_cache: false,
    })
}

/// The current Unix time in seconds (saturating to `0` before the epoch).
#[must_use]
pub fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The cache file key for an endpoint: `sha256(<endpoint-url>)`, hex-encoded.
fn cache_key(endpoint: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(endpoint.as_bytes());
    hex::encode(hasher.finalize())
}

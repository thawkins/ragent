//! Store-index fetch: HTTPS-only download, timeout, byte cap, JSON parse,
//! per-entry validation (spec `pluginstores` T-003; FR-003, FR-013, FR-016,
//! FR-023, FR-024, FR-025).
//!
//! This module turns a validated [`StoreEndpoint`] into a [`StoreIndex`]. It is
//! the only place in the store-browser domain that performs network I/O, and it
//! is deliberately fallible end-to-end:
//!
//! - **HTTPS only** (FR-024): the endpoint is a [`StoreEndpoint`], which can only
//!   be built from an absolute `https` URL, and redirects are followed only to
//!   other `https` locations, so a plaintext downgrade is impossible.
//! - **Bounded** (FR-013, FR-025): a request timeout bounds the wall-clock, and a
//!   streamed byte counter (plus `Content-Length` when offered) aborts once the
//!   body exceeds [`FetchLimits::max_bytes`].
//! - **Contained** (FR-025): every failure is a [`StoreError`] variant; the fetch
//!   never panics on a network error, a non-2xx status, malformed JSON, an
//!   oversized body, or a malformed entry. Malformed entries are skipped and
//!   counted so the remainder still renders (FR-013).
//! - **No execution** (FR-023): the body is parsed as JSON only. No plugin
//!   JavaScript is loaded, evaluated, or imported at any point during a fetch.
//!
//! [`fetch_bytes`] is blocking by design (`reqwest::blocking`); the caller runs
//! it on the TUI's off-loop task so the event loop and the agent turn keep
//! animating while the request is in flight (FR-016, FR-026; wired in T-008).
//! [`StoreIndex::from_bytes`] is the injectable parse seam the default-endpoint
//! tests use to feed fixture bytes without any live request (FR-037, T-019).

use std::time::Duration;

use crate::store_index::{StoreEndpoint, StoreEndpointError, StoreEntry, StoreKind};
use crate::store_provider::provider_for;

/// Wall-clock timeout and body-size ceiling for one store-index fetch.
///
/// Populated from `plugins.stores` configuration (`timeout_ms`,
/// `max_index_bytes`) and handed to [`fetch_bytes`]. A zero limit falls back to
/// the compiled default so a misconfiguration cannot make every fetch fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchLimits {
    /// Per-request wall-clock timeout.
    pub timeout: Duration,
    /// Maximum accepted index body size in bytes.
    pub max_bytes: u64,
}

impl FetchLimits {
    /// Compiled default timeout: 10 seconds.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(10_000);
    /// Compiled default body ceiling: 2 MiB.
    pub const DEFAULT_MAX_BYTES: u64 = 2 * 1024 * 1024;

    /// Build limits from a timeout and a byte ceiling.
    #[must_use]
    pub const fn new(timeout: Duration, max_bytes: u64) -> Self {
        Self { timeout, max_bytes }
    }

    /// Build limits from a millisecond timeout and a byte ceiling, mapping a
    /// zero timeout or ceiling onto the compiled default.
    #[must_use]
    pub const fn from_millis(timeout_ms: u64, max_bytes: u64) -> Self {
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
        }
    }
}

impl Default for FetchLimits {
    fn default() -> Self {
        Self::new(Self::DEFAULT_TIMEOUT, Self::DEFAULT_MAX_BYTES)
    }
}

impl From<&ragent_config::PluginStoresConfig> for FetchLimits {
    fn from(cfg: &ragent_config::PluginStoresConfig) -> Self {
        Self::from_millis(cfg.timeout_ms, cfg.max_index_bytes)
    }
}

/// A parsed store index: the store token (when the document carries one), the
/// accepted entries, and how many entries were skipped as malformed (FR-003).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoreIndex {
    /// The `"store"` token from the document body, when present (informational).
    pub store: Option<String>,
    /// Accepted entries, in document order.
    pub entries: Vec<StoreEntry>,
    /// Entries skipped because a required field was missing, empty, or
    /// wrong-typed (counted, not fatal; FR-013).
    pub skipped: usize,
}

impl StoreIndex {
    /// Parse index bytes into a [`StoreIndex`] (FR-003, FR-013, FR-025).
    ///
    /// Accepts the spec shape (`{"plugins": [...]}`) and, tolerantly, a bare
    /// top-level JSON array. Malformed JSON is a [`StoreError::MalformedJson`];
    /// a document that is neither of those shapes is a
    /// [`StoreError::MalformedShape`]. Individual entries that fail validation
    /// are skipped and counted in [`StoreIndex::skipped`] rather than failing
    /// the whole parse (FR-013).
    ///
    /// This is also the **seam** the default-endpoint tests use to feed fixture
    /// bytes with no live request (FR-037, NFR-003).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, StoreError> {
        let root: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| StoreError::MalformedJson {
                detail: e.to_string(),
            })?;
        Self::from_value(&root)
    }

    /// Parse an already-decoded JSON value into a [`StoreIndex`].
    ///
    /// See [`StoreIndex::from_bytes`] for the accepted shapes.
    pub fn from_value(root: &serde_json::Value) -> Result<Self, StoreError> {
        let (store, plugins): (Option<String>, &serde_json::Value) = match root {
            serde_json::Value::Object(map) => {
                let store = map
                    .get("store")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
                let plugins = map
                    .get("plugins")
                    .ok_or_else(|| StoreError::MalformedShape {
                        detail: "object is missing the \"plugins\" array".to_string(),
                    })?;
                (store, plugins)
            }
            serde_json::Value::Array(_) => (None, root),
            _ => {
                return Err(StoreError::MalformedShape {
                    detail: "expected an object with a \"plugins\" array or a bare array"
                        .to_string(),
                });
            }
        };

        let items = plugins
            .as_array()
            .ok_or_else(|| StoreError::MalformedShape {
                detail: "\"plugins\" must be a JSON array".to_string(),
            })?;

        let mut entries = Vec::new();
        let mut skipped = 0usize;
        for item in items {
            match StoreEntry::from_value(item) {
                Ok(entry) => entries.push(entry),
                // A malformed entry is skipped and counted, never fatal (FR-013).
                Err(_) => skipped += 1,
            }
        }
        Ok(Self {
            store,
            entries,
            skipped,
        })
    }
}

/// Fetch a store index over HTTPS and parse it with the store's provider
/// (FR-003, FR-013, FR-024, FR-025).
///
/// Blocking: call this from the off-loop task (T-008) so the TUI stays
/// responsive (FR-016, FR-026). The body is parsed by the provider for `kind`,
/// so each store's own marketplace shape is normalised into the same
/// [`StoreIndex`] (FR-027, FR-029). Every failure is a [`StoreError`].
pub fn fetch_index(
    kind: StoreKind,
    endpoint: &StoreEndpoint,
    limits: &FetchLimits,
) -> Result<StoreIndex, StoreError> {
    let bytes = fetch_bytes(endpoint, limits)?;
    provider_for(kind).parse_index(&bytes, endpoint.as_url())
}

/// Download the raw index bytes from a validated endpoint (FR-013, FR-024).
///
/// Enforces the timeout and the byte ceiling; returns a [`StoreError`] for a
/// non-2xx status, a refused redirect, an oversized body, a timeout, or a
/// network failure. No plugin code is executed.
pub fn fetch_bytes(endpoint: &StoreEndpoint, limits: &FetchLimits) -> Result<Vec<u8>, StoreError> {
    let client = reqwest::blocking::Client::builder()
        .redirect(https_only_redirects())
        .timeout(limits.timeout)
        .build()
        .map_err(|e| StoreError::Network {
            detail: format!("http client: {e}"),
        })?;

    let response = client.get(endpoint.as_url().clone()).send().map_err(|e| {
        if e.is_timeout() {
            StoreError::Timeout {
                ms: limits.timeout.as_millis() as u64,
            }
        } else {
            StoreError::Network {
                detail: e.to_string(),
            }
        }
    })?;

    let status = response.status();
    if status.is_redirection() {
        // Only reached when a redirect was refused (non-https or too many).
        return Err(StoreError::Redirect {
            status: status.as_u16(),
        });
    }
    if !status.is_success() {
        return Err(StoreError::Http {
            status: status.as_u16(),
        });
    }
    // Reject early when the server advertises an oversized body.
    if let Some(len) = response.content_length()
        && len > limits.max_bytes
    {
        return Err(StoreError::TooLarge {
            limit: limits.max_bytes,
        });
    }
    read_capped(response, limits.max_bytes)
}

/// Read `reader` fully, aborting once more than `max_bytes` has been read
/// (FR-013, FR-025).
///
/// Pure and reader-agnostic so the size cap is unit-testable offline with a
/// [`std::io::Cursor`]; [`fetch_bytes`] passes the HTTP response as the reader.
pub fn read_capped(mut reader: impl std::io::Read, max_bytes: u64) -> Result<Vec<u8>, StoreError> {
    let mut bytes = Vec::new();
    let mut chunk = vec![0_u8; 65536];
    loop {
        let read = reader.read(&mut chunk).map_err(|e| StoreError::Network {
            detail: e.to_string(),
        })?;
        if read == 0 {
            break;
        }
        if (bytes.len() + read) as u64 > max_bytes {
            return Err(StoreError::TooLarge { limit: max_bytes });
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    Ok(bytes)
}

/// A redirect policy that follows only `https` targets, up to five hops
/// (FR-024).
///
/// A plaintext downgrade (or any non-`https` location) is stopped, so the
/// endpoint cannot be bypassed by a redirect chain. The stopped response is a
/// 3xx, surfaced as [`StoreError::Redirect`].
fn https_only_redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.url().scheme() != "https" {
            attempt.stop()
        } else if attempt.previous().len() >= 5 {
            attempt.error("too many redirects")
        } else {
            attempt.follow()
        }
    })
}

/// The label for a store, used in error text and reporting (FR-013).
#[must_use]
pub fn store_label(kind: StoreKind) -> &'static str {
    kind.label()
}

/// A contained store-index fetch or parse failure (FR-013, FR-025).
///
/// Every variant carries enough detail to name the cause in the panel's inline
/// error row; none of them panics the process (FR-025).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// The endpoint was not an acceptable `https` URL (FR-024).
    #[error("store endpoint rejected: {0}")]
    Endpoint(#[from] StoreEndpointError),

    /// The request could not be sent or the connection failed (DNS, TLS, reset).
    #[error("store fetch failed: {detail}")]
    Network {
        /// Underlying failure detail.
        detail: String,
    },

    /// The request exceeded the wall-clock timeout (FR-013).
    #[error("store fetch timed out after {ms} ms")]
    Timeout {
        /// The timeout that elapsed, in milliseconds.
        ms: u64,
    },

    /// The server returned a non-2xx status (FR-013).
    #[error("store fetch returned HTTP {status}")]
    Http {
        /// The HTTP status code.
        status: u16,
    },

    /// A redirect was refused because it targeted a non-`https` location, or
    /// because the chain exceeded the hop limit (FR-024).
    #[error("store fetch redirect refused (HTTP {status}); only https redirects are followed")]
    Redirect {
        /// The status of the response whose redirect was refused.
        status: u16,
    },

    /// The body exceeded the configured byte ceiling (FR-013, FR-025).
    #[error("store index exceeds the {limit} byte limit")]
    TooLarge {
        /// The configured ceiling, in bytes.
        limit: u64,
    },

    /// The body was not valid JSON (FR-013).
    #[error("store index is not valid JSON: {detail}")]
    MalformedJson {
        /// The serde JSON failure detail, including the error position.
        detail: String,
    },

    /// The body parsed as JSON but was not a store-index shape (FR-013).
    #[error("store index has an unexpected shape: {detail}")]
    MalformedShape {
        /// What was expected instead.
        detail: String,
    },
}

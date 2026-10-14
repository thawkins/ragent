//! Shared manifest-enrichment plumbing for the plugin-store and
//! connector-catalogue browsers (spec `pluginstores` FR-003, FR-048; spec
//! `connectors` FR-044, T-026).
//!
//! Both browsers enrich a listing by fetching each entry's own manifest from a
//! raw GitHub URL derived from its install source, then filling the fields the
//! listing omitted. The URL derivation is dialect-specific and stays in each
//! crate; the HTTP client, the bounded cache, the `https`-only redirect policy,
//! the byte-capped fetch, the GitHub owner/repo split, and the manifest-body
//! parser were byte-identical copies in each crate and live here once, so a
//! constant, cache bound, or redirect-policy change cannot drift between the
//! two surfaces (ANTIPAT L7).
//!
//! Enrichment is best-effort and never fatal: a missing manifest, a transport
//! failure, non-JSON bytes, or an oversized body yields no metadata and the row
//! keeps whatever the listing declared.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;

/// Maximum number of entries enriched per listing. A bound keeps a pathological
/// listing from issuing an unbounded number of requests (ANTIPAT L7).
pub const MAX_ENRICH_ENTRIES: usize = 500;

/// Maximum number of manifest lookups in flight at once.
pub const ENRICH_CONCURRENCY: usize = 8;

/// Wall-clock timeout for one manifest fetch.
pub const MANIFEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum accepted manifest body size in bytes.
pub const MAX_MANIFEST_BYTES: u64 = 256 * 1024;

/// Maximum number of manifest results retained in the process-global cache.
const MAX_CACHE_ENTRIES: usize = 1024;

/// Manifest-derived metadata for one entry: the fields a listing typically
/// omits. The connector catalogue only ever consumes `description`; a plugin
/// store consumes both.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ManifestMeta {
    /// The manifest's declared version, when it carries a non-blank one.
    pub version: Option<String>,
    /// The manifest's description (`description`, else the `interface` block's
    /// short/long description), when it carries a non-blank one.
    pub description: Option<String>,
}

impl ManifestMeta {
    /// Whether this metadata carries nothing worth applying.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.version.is_none() && self.description.is_none()
    }

    /// Merge `add` into `self`, preferring the first non-blank value for each
    /// field.
    pub fn merge(&mut self, add: Self) {
        if self.version.is_none() {
            self.version = add.version;
        }
        if self.description.is_none() {
            self.description = add.description;
        }
    }
}

/// One entry's resolved manifest metadata, keyed by the entry id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestUpdate {
    /// The listing entry id the metadata belongs to.
    pub id: String,
    /// The resolved metadata.
    pub meta: ManifestMeta,
}

/// The production manifest source's HTTP client, built lazily on first use
/// (`https`-only redirects, bounded by [`MANIFEST_TIMEOUT`]).
///
/// The client is constructed inside the blocking enrichment thread that calls
/// it, so a handle held by something that never enriches (an offline test, a
/// headless run) never builds a blocking client - which would otherwise spawn a
/// runtime that must not be dropped from an async context. A client that cannot
/// be built leaves the source inert: every lookup returns `None` rather than
/// panicking.
#[derive(Debug, Default)]
pub struct NetworkManifestSource {
    client: OnceLock<Option<reqwest::blocking::Client>>,
}

impl NetworkManifestSource {
    /// Build the source. No client is constructed until the first lookup.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: OnceLock::new(),
        }
    }

    /// The shared HTTP client, built on first use, or `None` when it cannot be
    /// built.
    #[must_use]
    pub fn client(&self) -> Option<&reqwest::blocking::Client> {
        self.client
            .get_or_init(|| {
                reqwest::blocking::Client::builder()
                    .redirect(crate::net::https_only_redirects())
                    .timeout(MANIFEST_TIMEOUT)
                    .user_agent(concat!("ragent/", env!("CARGO_PKG_VERSION")))
                    .build()
                    .ok()
            })
            .as_ref()
    }
}

/// Fetch one raw manifest URL and parse it, returning `None` on any failure.
#[must_use]
pub fn fetch_and_parse(client: &reqwest::blocking::Client, url: &str) -> Option<ManifestMeta> {
    let response = client.get(url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    if let Some(len) = response.content_length()
        && len > MAX_MANIFEST_BYTES
    {
        return None;
    }
    let bytes = read_capped(response, MAX_MANIFEST_BYTES).ok()?;
    Some(parse_manifest_meta(&bytes))
}

/// Read a stream into a byte vector, refusing a body larger than `max_bytes`.
fn read_capped(mut reader: impl std::io::Read, max_bytes: u64) -> Result<Vec<u8>, ()> {
    let mut bytes = Vec::new();
    let mut chunk = vec![0_u8; 8 * 1024];
    loop {
        let read = reader.read(&mut chunk).map_err(|_| ())?;
        if read == 0 {
            break;
        }
        if (bytes.len() + read) as u64 > max_bytes {
            return Err(());
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    Ok(bytes)
}

/// Extract `(owner, repo)` from an `https://github.com/<owner>/<repo>` URL.
///
/// A trailing `.git` and any `/tree/...` view suffix are ignored; a URL on any
/// other host (or with fewer than two path segments) yields `None`.
#[must_use]
pub fn github_owner_repo(url: &str) -> Option<(String, String)> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.host_str() != Some("github.com") && parsed.host_str() != Some("www.github.com") {
        return None;
    }
    let segments: Vec<&str> = parsed
        .path()
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() < 2 {
        return None;
    }
    let owner = segments[0];
    let repo = segments[1].strip_suffix(".git").unwrap_or(segments[1]);
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

/// Parse a manifest body into [`ManifestMeta`].
///
/// `description` is the top-level `description`, falling back to the `interface`
/// block's `shortDescription`, then `longDescription`, then `description`, then a
/// top-level `longDescription`. Blank or wrong-typed fields are treated as
/// absent; malformed JSON yields an empty metadata value.
#[must_use]
pub fn parse_manifest_meta(bytes: &[u8]) -> ManifestMeta {
    let Ok(root) = serde_json::from_slice::<Value>(bytes) else {
        return ManifestMeta::default();
    };
    ManifestMeta {
        version: nonblank(root.get("version")),
        description: nonblank(root.get("description"))
            .or_else(|| {
                nonblank(
                    root.get("interface")
                        .and_then(|i| i.get("shortDescription")),
                )
            })
            .or_else(|| nonblank(root.get("interface").and_then(|i| i.get("longDescription"))))
            .or_else(|| nonblank(root.get("interface").and_then(|i| i.get("description"))))
            .or_else(|| nonblank(root.get("longDescription"))),
    }
}

/// A non-blank string value, or `None`.
fn nonblank(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// The process-global manifest cache, keyed by raw manifest URL.
fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Cache::default()))
}

/// A bounded URL -> metadata cache, tracking insertion order for FIFO eviction.
#[derive(Default)]
struct Cache {
    by_url: HashMap<String, ManifestMeta>,
    order: VecDeque<String>,
}

/// Look up a cached manifest result (a negative result is cached as an empty
/// metadata value). Best-effort: a poisoned lock is recovered rather than
/// propagated, because a cache miss is never fatal.
#[must_use]
pub fn cache_get(url: &str) -> Option<ManifestMeta> {
    let cache = cache().lock().unwrap_or_else(|p| p.into_inner());
    cache.by_url.get(url).cloned()
}

/// Cache a manifest result. Both hits and misses are stored: a negative result
/// is kept as an empty metadata value so a later open does not re-issue a
/// known-404 request.
pub fn cache_put(url: String, meta: Option<ManifestMeta>) {
    // A negative result is stored as an empty metadata value.
    let meta = meta.unwrap_or_default();
    let mut cache = cache().lock().unwrap_or_else(|p| p.into_inner());
    if cache.by_url.insert(url.clone(), meta).is_none() {
        cache.order.push_back(url);
    }
    while cache.order.len() > MAX_CACHE_ENTRIES {
        if let Some(oldest) = cache.order.pop_front() {
            cache.by_url.remove(&oldest);
        }
    }
}

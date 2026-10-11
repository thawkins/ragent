//! Best-effort manifest metadata enrichment for connector-catalogue entries
//! (spec `connectors` FR-044, T-026; shares the plumbing with
//! `ragent_plugins::store_enrich`).
//!
//! A catalogue listing names each connector and how to install it, but the
//! connector's own `description` usually lives in the connector manifest, not in
//! the catalogue. Rendering such a catalogue directly therefore shows rows with
//! blank descriptions.
//!
//! This module closes that gap without changing the catalogue fetch: after a
//! catalogue lands, each entry whose install source is a `github.com` git source
//! of the form `git+<https-url>#<ref>[:<subpath>]` has its own manifest fetched
//! at the raw URL derived from that source
//! (`https://raw.githubusercontent.com/<owner>/<repo>/<ref>/<subpath>/connector.json`,
//! with `.claude-plugin/connector.json` tried after it), and the manifest's
//! `description` fills the entry's blank field (never overwriting a value the
//! catalogue already declared). Entries whose source is the catalogue endpoint
//! itself derive no manifest and are simply left alone.
//!
//! - [`catalogue_manifest_urls`] derives the candidate raw manifest URLs from an
//!   install source. Only a `github.com` git source yields any URL.
//! - [`NetworkManifestSource`] fetches and parses those URLs over `https` with a
//!   timeout and a byte cap, caching the result per URL (including a negative
//!   result) so a later open of the same catalogue does not re-fetch. The HTTP
//!   client, cache, fetch, and parser live once in [`ragent_surface::manifest`]
//!   and are shared with the plugin-store enricher so the two surfaces cannot
//!   drift.
//! - [`enrich_catalogue_into`] runs the per-entry lookups across a small bounded
//!   thread pool and reports each resolved [`ManifestUpdate`] through a sink, so
//!   the caller can apply progress as it arrives.
//! - [`ManifestSource`] is the injectable seam: the production
//!   [`NetworkManifestSource`] is the default, and an offline implementation can
//!   be supplied in tests (NFR-003).
//!
//! Enrichment is **best-effort and never fatal**: a missing manifest, a transport
//! failure, non-JSON bytes, or an oversized body yields no update for that entry
//! and the row keeps whatever the catalogue declared.

use std::sync::atomic::{AtomicUsize, Ordering};

use ragent_surface::manifest::{self, NetworkManifestSource as SharedNetworkManifestSource};
use ragent_types::guard::is_safe_operand;

use crate::descriptor::ConnectorDescriptor;

pub use ragent_surface::manifest::{
    ENRICH_CONCURRENCY, MANIFEST_TIMEOUT, MAX_ENRICH_ENTRIES, MAX_MANIFEST_BYTES, ManifestMeta,
    ManifestUpdate, parse_manifest_meta,
};

/// The connector manifest file names to try, in priority order.
const MANIFEST_NAMES: [&str; 2] = ["connector.json", ".claude-plugin/connector.json"];

/// The injectable seam that resolves one entry's manifest metadata.
///
/// The production implementation ([`NetworkManifestSource`]) fetches the
/// manifest over `https`; an offline implementation returns fixture metadata so
/// the enrichment path can be exercised with no network access (NFR-003).
/// Implementations must be `Send + Sync` because enrichment runs off the event
/// loop across a thread pool; every method is fallible and no implementation
/// panics.
pub trait ManifestSource: Send + Sync {
    /// Resolve the manifest metadata for `entry`, or `None` when the entry has
    /// no derivable manifest or it could not be read.
    fn manifest_meta(&self, entry: &ConnectorDescriptor) -> Option<ManifestMeta>;
}

/// The production manifest source: an `https` fetch of the raw manifest.
///
/// The HTTP client is built lazily on first use, inside the blocking enrichment
/// thread that calls it, so a handle held by something that never enriches (an
/// offline test, a headless run) never constructs a blocking client - which
/// would otherwise spawn a runtime that must not be dropped from an async
/// context.
#[derive(Debug, Default)]
pub struct NetworkManifestSource {
    inner: SharedNetworkManifestSource,
}

impl NetworkManifestSource {
    /// Build the source. No client is constructed until the first lookup.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: SharedNetworkManifestSource::new(),
        }
    }
}

impl ManifestSource for NetworkManifestSource {
    fn manifest_meta(&self, entry: &ConnectorDescriptor) -> Option<ManifestMeta> {
        let client = self.inner.client()?;
        let mut meta = ManifestMeta::default();
        for url in catalogue_manifest_urls(&entry.source) {
            let cached = match manifest::cache_get(&url) {
                Some(cached) => cached,
                None => {
                    let fetched = manifest::fetch_and_parse(client, &url);
                    manifest::cache_put(url, fetched.clone());
                    fetched.unwrap_or_default()
                }
            };
            if let Some(description) = cached.description {
                meta.description = Some(description);
                break;
            }
        }
        if meta.description.is_none() {
            None
        } else {
            Some(meta)
        }
    }
}

/// The candidate raw manifest URLs for an install `source`.
///
/// Only a `github.com` `git+` source yields URLs:
/// `https://raw.githubusercontent.com/<owner>/<repo>/<ref>/<subpath>/connector.json`,
/// with `.claude-plugin/connector.json` tried after it. A source with no
/// derivable GitHub repository (the catalogue endpoint itself, an archive `https`
/// URL, a local path, a non-GitHub host) yields an empty list.
#[must_use]
pub fn catalogue_manifest_urls(source: &str) -> Vec<String> {
    let Some(git) = parse_git_source(source) else {
        return Vec::new();
    };
    let Some((owner, repo)) = manifest::github_owner_repo(&git.url) else {
        return Vec::new();
    };
    let ref_name = git.ref_name.as_deref().unwrap_or("HEAD");
    let subpath = git
        .subpath
        .as_deref()
        .map(|p| p.trim_matches('/'))
        .filter(|p| !p.is_empty());
    let mut bases = Vec::with_capacity(MANIFEST_NAMES.len());
    for manifest in MANIFEST_NAMES {
        let mut url = format!("https://raw.githubusercontent.com/{owner}/{repo}/{ref_name}");
        if let Some(subpath) = subpath {
            url.push('/');
            url.push_str(subpath);
        }
        url.push('/');
        url.push_str(manifest);
        bases.push(url);
    }
    bases
}

/// A parsed `git+<https-url>#<ref>[:<subpath>]` source.
struct GitSource {
    url: String,
    ref_name: Option<String>,
    subpath: Option<String>,
}

/// Parse a `git+` source into its url, ref, and subpath parts.
///
/// The guards mirror `ragent_plugins::add::parse_git_source` (SEC-ragent-plugins-001):
/// only an `https://` or `file://` remote is accepted, and the ref and subpath are
/// validated with [`is_safe_operand`] (a non-empty `[A-Za-z0-9._/-]` string that does
/// not begin with `-` and contains no `..`). A `git+` source is untrusted content -
/// it is emitted from a marketplace document fetched over the network - so a ref of
/// `--upload-pack=<cmd>` must never be allowed to derive a URL from it.
fn parse_git_source(source: &str) -> Option<GitSource> {
    let rest = source.strip_prefix("git+")?;
    let (url, tail) = match rest.split_once('#') {
        Some((url, tail)) => (url, Some(tail)),
        None => (rest, None),
    };
    if !(url.starts_with("https://") || url.starts_with("file://")) {
        return None;
    }
    let (ref_name, subpath) = match tail.and_then(|t| t.split_once(':')) {
        Some((ref_name, subpath)) => (git_ref(ref_name), git_subpath(subpath)),
        None => (tail.and_then(git_ref), None),
    };
    Some(GitSource {
        url: url.to_string(),
        ref_name,
        subpath,
    })
}

/// A git ref, treating an empty value or `HEAD` as "the default branch" and
/// refusing anything that is not safe to hand to `git` as a positional argument
/// (SEC-ragent-plugins-001). Mirrors `ragent_plugins::add::git_ref`.
fn git_ref(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("HEAD") {
        None
    } else if is_safe_operand(trimmed) && !trimmed.contains("..") {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// A repository-relative subpath, normalised without a leading `./` and validated
/// with the same charset as [`git_ref`]. Mirrors `ragent_plugins::add::git_subpath`.
fn git_subpath(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_start_matches("./");
    if trimmed.is_empty() {
        None
    } else if is_safe_operand(trimmed) && !trimmed.contains("..") {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Enrich `entries`, reporting each resolved update to `sink`.
///
/// Every entry whose install source is a `git+` source is looked up through
/// `source` on a bounded thread pool ([`ENRICH_CONCURRENCY`]); `sink` is invoked
/// once per entry that resolved metadata and is called from the worker threads,
/// so it must be `Sync`. Entries with no derivable manifest resolve to nothing
/// and are skipped, and at most [`MAX_ENRICH_ENTRIES`] lookups are issued. This
/// function is synchronous and blocking by design; the caller runs it off the TUI
/// event loop.
pub fn enrich_catalogue_into(
    entries: &[ConnectorDescriptor],
    source: &dyn ManifestSource,
    sink: &(dyn Fn(ManifestUpdate) + Sync),
) {
    // A `git+` source is the only kind that can yield a manifest URL, so it is
    // the cheap pre-filter: the exact URL derivation happens once inside
    // `ManifestSource::manifest_meta`, never twice.
    let candidates: Vec<&ConnectorDescriptor> = entries
        .iter()
        .filter(|entry| entry.source.starts_with("git+"))
        .take(MAX_ENRICH_ENTRIES)
        .collect();
    if candidates.is_empty() {
        return;
    }
    let next = AtomicUsize::new(0);
    let workers = ENRICH_CONCURRENCY.min(candidates.len());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= candidates.len() {
                        break;
                    }
                    let entry = candidates[index];
                    if let Some(meta) = source.manifest_meta(entry) {
                        sink(ManifestUpdate {
                            id: entry.id.as_str().to_string(),
                            meta,
                        });
                    }
                }
            });
        }
    });
}

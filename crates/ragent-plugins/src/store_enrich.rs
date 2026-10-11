//! Best-effort manifest metadata enrichment for store-browser entries (spec
//! `pluginstores` FR-003, FR-048).
//!
//! A vendor marketplace index is a *listing*: it names each plugin and where to
//! install it, but the plugin's own `version` and `description` usually live in
//! the plugin's own manifest, not in the index. The Codex marketplace carries
//! neither field at all, and most Claude entries carry a description but no
//! version. Rendering such an index directly therefore shows a store with no
//! descriptions (Codex) and no versions (both).
//!
//! This module closes that gap without changing the index fetch: after a store
//! index lands, each entry whose install source is a GitHub git source has its
//! own manifest fetched at the raw URL derived from that source (for example
//! `git+https://github.com/openai/plugins#main:plugins/linear` resolves to
//! `https://raw.githubusercontent.com/openai/plugins/main/plugins/linear/.codex-plugin/plugin.json`),
//! and the manifest's `version` and `description` fill the entry's blank fields.
//!
//! - [`manifest_urls`] derives the candidate raw manifest URLs from an install
//!   source and the store kind (dialect-appropriate names first). Only a
//!   `github.com` git source yields any URL; an archive `https` source yields
//!   none, so entry types with no derivable manifest are simply left alone.
//! - [`NetworkManifestSource`] fetches and parses those URLs over `https` with a
//!   timeout and a byte cap, caching the result per URL (including a negative
//!   result) so a later open of the same store does not re-fetch. The HTTP
//!   client, cache, fetch, and parser live once in
//!   [`ragent_surface::manifest`] and are shared with the connector-catalogue
//!   enricher so the two surfaces cannot drift.
//! - [`enrich_into`] runs the per-entry lookups across a small bounded thread
//!   pool and reports each resolved [`ManifestUpdate`] through a sink, so the
//!   caller can apply progress as it arrives rather than waiting for the whole
//!   set.
//! - [`ManifestSource`] is the injectable seam: the production
//!   [`NetworkManifestSource`] is the default, and an offline implementation can
//!   be supplied in tests exactly as [`crate::store_seam::StoreIndexFetcher`] is
//!   (NFR-003).
//!
//! Enrichment is **best-effort and never fatal** (FR-025): a missing manifest, a
//! transport failure, non-JSON bytes, or an oversized body yields no update for
//! that entry and the row keeps whatever the index declared.

use std::sync::atomic::{AtomicUsize, Ordering};

use ragent_surface::manifest::{self, NetworkManifestSource as SharedNetworkManifestSource};

use crate::add::parse_git_source;
use crate::store_index::{StoreEntry, StoreKind};

pub use ragent_surface::manifest::{
    ENRICH_CONCURRENCY, MANIFEST_TIMEOUT, MAX_ENRICH_ENTRIES, MAX_MANIFEST_BYTES, ManifestMeta,
    ManifestUpdate, parse_manifest_meta,
};

/// The injectable seam that resolves one entry's manifest metadata.
///
/// The production implementation ([`NetworkManifestSource`]) fetches the
/// manifest over `https`; an offline implementation returns fixture metadata so
/// the enrichment path can be exercised with no network access (NFR-003).
/// Implementations must be `Send + Sync` because enrichment runs off the event
/// loop across a thread pool; every method is fallible and no implementation
/// panics.
pub trait ManifestSource: Send + Sync {
    /// Resolve the manifest metadata for `entry` in the store `kind`, or `None`
    /// when the entry has no derivable manifest or it could not be read.
    fn manifest_meta(&self, entry: &StoreEntry, kind: StoreKind) -> Option<ManifestMeta>;
}

/// The production manifest source: an `https` fetch of the raw manifest.
///
/// The HTTP client is built lazily on first use, inside the blocking enrichment
/// thread that calls it, so an `App` that holds this source but never enriches
/// (an offline test, a headless run) never constructs a blocking client - which
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
    fn manifest_meta(&self, entry: &StoreEntry, kind: StoreKind) -> Option<ManifestMeta> {
        let client = self.inner.client()?;
        let mut meta = ManifestMeta::default();
        for url in manifest_urls(&entry.source, kind) {
            let cached = match manifest::cache_get(&url) {
                Some(cached) => cached,
                None => {
                    let fetched = manifest::fetch_and_parse(client, &url);
                    manifest::cache_put(url, fetched.clone());
                    fetched.unwrap_or_default()
                }
            };
            meta.merge(cached);
            if meta.version.is_some() && meta.description.is_some() {
                break;
            }
        }
        if meta.is_empty() { None } else { Some(meta) }
    }
}

/// The candidate raw manifest URLs for an install `source` in store `kind`.
///
/// Only a `github.com` git source yields URLs; the dialect-appropriate manifest
/// names are ordered first (`.codex-plugin/plugin.json` for Codex,
/// `.claude-plugin/plugin.json` for Claude), followed by a bare `plugin.json`. A
/// source with no derivable GitHub repository (an archive `https` URL, a local
/// path, a non-GitHub host) yields an empty list.
#[must_use]
pub fn manifest_urls(source: &str, kind: StoreKind) -> Vec<String> {
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
    let mut bases = Vec::with_capacity(3);
    for manifest in manifest_names(kind) {
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

/// The manifest file names to try for a store, in priority order.
fn manifest_names(kind: StoreKind) -> [&'static str; 3] {
    match kind {
        StoreKind::Codex => [
            crate::descriptor::CODEX_NESTED_MANIFEST,
            crate::descriptor::GENERIC_MANIFEST_FILE,
            crate::descriptor::CODEX_MANIFEST_FILE,
        ],
        StoreKind::Claude => [
            crate::descriptor::CLAUDE_NESTED_MANIFEST,
            crate::descriptor::GENERIC_MANIFEST_FILE,
            crate::descriptor::CLAUDE_MANIFEST_FILE,
        ],
    }
}

/// Enrich `entries` for store `kind`, reporting each resolved update to `sink`.
///
/// Every entry whose install source is a `git+` source is looked up through
/// `source` on a bounded thread pool ([`ENRICH_CONCURRENCY`]); `sink` is invoked
/// once per entry that resolved metadata and is called from the worker threads,
/// so it must be `Sync`. Entries with no derivable manifest resolve to nothing
/// and are skipped, and at most [`MAX_ENRICH_ENTRIES`] lookups are issued. This
/// function is synchronous and blocking by design; the caller runs it off the TUI
/// event loop (FR-016, FR-026).
pub fn enrich_into(
    entries: &[StoreEntry],
    kind: StoreKind,
    source: &dyn ManifestSource,
    sink: &(dyn Fn(ManifestUpdate) + Sync),
) {
    // A `git+` source is the only kind that can yield a manifest URL, so it is
    // the cheap pre-filter: the exact URL derivation happens once inside
    // `ManifestSource::manifest_meta`, never twice.
    let candidates: Vec<&StoreEntry> = entries
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
                    if let Some(meta) = source.manifest_meta(entry, kind) {
                        sink(ManifestUpdate {
                            id: entry.id.clone(),
                            meta,
                        });
                    }
                }
            });
        }
    });
}

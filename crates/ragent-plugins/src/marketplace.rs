//! Marketplace-inline manifests for manifest-less plugin stubs (FR-002,
//! FR-025).
//!
//! Several entries of the official Claude marketplace
//! (`anthropics/claude-plugins-official`) are **self-describing stubs**: the
//! marketplace document carries the plugin's whole manifest inline (the
//! `rust-analyzer-lsp` entry ships its `lspServers` block in the marketplace
//! JSON), while the repo subdirectory the `source` points at holds only a
//! README and a LICENSE. Cloning such a source yields no recognisable
//! manifest, so `/plugins add` used to stop at `no plugin manifest found`
//! (spec `plugins` T-024 follow-up).
//!
//! The bridge works in three steps:
//!
//! 1. [`record_document`] stores the raw marketplace document bytes whenever a
//!    store index is parsed, keyed by the document's SHA-256 content hash.
//! 2. [`StoreProvider::parse_index`](crate::store_provider::StoreProvider)
//!    calls [`inline_manifest_for`], which extracts one candidate manifest
//!    body per plugin name from the *live* document (the same bytes the
//!    provider is parsing), and appends `#<key>` to that entry's install
//!    source, where `key = sha256(<origin-url> + "\n" + <document bytes>)`.
//! 3. [`materialize_at`] is consulted by the install pipeline once the clone
//!    is on disk: when the checked-out subdirectory carries no recognisable
//!    manifest, the recorded inline manifest is written to
//!    `.claude-plugin/plugin.json`, turning the stub into an ordinary
//!    Claude-dialect plugin. A directory that already has a manifest is
//!    returned untouched, so an upstream fix (adding a real manifest) wins
//!    automatically.
//!
//! The key embeds the origin URL and the document bytes, so a manifest is
//! only ever materialised for the exact marketplace document it was taken
//! from; a git source without the `#<key>` suffix, or a key that was never
//! registered in this process, installs exactly as before.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};

use sha2::Digest;

/// The marker character separating a plugin's git subpath from the
/// marketplace-manifest key in an install source string.
pub const INLINE_MANIFEST_KEY_SEP: char = '@';

/// The filename an inline manifest is materialised into.
pub const MATERIALIZED_MANIFEST_REL: &str = ".claude-plugin/plugin.json";

/// Lowercase hex length of a SHA-256 digest (ANTIPAT L12).
const SHA256_HEX_LEN: usize = 64;

/// Maximum number of marketplace documents retained in the process-global
/// registry (ANTIPAT L7).
///
/// The registry only needs the documents whose inline manifests are still
/// pending an install in this process; a long run that parses many store
/// indexes would otherwise grow it without bound. Once the cap is reached the
/// oldest-inserted key (FIFO, `HashMap` plus insertion order) is evicted so a
/// live document is never displaced by a fresh one.
const MAX_MARKETPLACE_DOCUMENTS: usize = 64;

/// Marketplace fields that are never part of the materialised manifest.
///
/// `source`, `category`, `tags`, `keywords`, `author`, `homepage` describe the
/// marketplace listing, and `strict` is a Claude Code host flag with no ragent
/// equivalent; everything else (`name`, `version`, `description`,
/// `lspServers`, ...) is manifest content in the stub shape.
const LISTING_ONLY_FIELDS: &[&str] = &[
    "source", "category", "tags", "keywords", "author", "homepage", "strict",
];

/// The recorded marketplace documents, keyed by [`document_key`].
///
/// Insertion order is tracked separately so the registry can evict the oldest
/// document once [`MAX_MARKETPLACE_DOCUMENTS`] is reached (ANTIPAT L7).
struct DocumentRegistry {
    by_key: HashMap<String, String>,
    insertion_order: VecDeque<String>,
}

impl DocumentRegistry {
    /// Store `body` under `key`, evicting the oldest document when the cap is
    /// reached. Re-recording an existing key refreshes its body in place
    /// without growing the registry.
    fn insert(&mut self, key: String, body: String) {
        if self.by_key.insert(key.clone(), body).is_none() {
            self.insertion_order.push_back(key);
        }
        while self.insertion_order.len() > MAX_MARKETPLACE_DOCUMENTS {
            if let Some(oldest) = self.insertion_order.pop_front() {
                self.by_key.remove(&oldest);
            }
        }
    }

    /// The body recorded under `key`, when present.
    fn get(&self, key: &str) -> Option<&String> {
        self.by_key.get(key)
    }
}

/// The process-global marketplace-document registry, keyed by
/// [`document_key`].
fn registry() -> &'static Mutex<DocumentRegistry> {
    static REGISTRY: OnceLock<Mutex<DocumentRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Mutex::new(DocumentRegistry {
            by_key: HashMap::new(),
            insertion_order: VecDeque::new(),
        })
    })
}

/// Recover from a poisoned registry lock, mirroring the TUI's
/// `recover_poisoned` discipline: the install pipeline must not panic.
fn lock_registry() -> std::sync::MutexGuard<'static, DocumentRegistry> {
    registry().lock().unwrap_or_else(|p| p.into_inner())
}

/// The lookup key binding an install source to one marketplace document:
/// `sha256(<origin-url> + "\n" + <document bytes>)`, hex-encoded.
#[must_use]
pub fn document_key(origin: &str, bytes: &[u8]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(origin.as_bytes());
    hasher.update(b"\n");
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Record a marketplace document (`origin` URL + raw bytes) so a later
/// install can look up its inline manifests. Called once per store-index
/// parse; identical re-parses overwrite the same key.
///
/// Returns the [`document_key`] the document was recorded under so callers
/// that need the key do not hash the document a second time.
pub fn record_document(origin: &str, bytes: &[u8]) -> String {
    let key = document_key(origin, bytes);
    let body = String::from_utf8_lossy(bytes).into_owned();
    lock_registry().insert(key.clone(), body);
    key
}

/// Sections that prove a marketplace entry IS the manifest (a self-describing
/// stub) rather than a plain listing pointing at a real plugin on disk.
///
/// These are the hosting-config sections Claude Code hosts consume straight
/// from the marketplace document - the shape of the `*-lsp` stubs. A plain
/// `description` does NOT qualify: most real plugin entries carry one, and
/// materialising a manifest for those would shadow the real manifest in the
/// clone.
const INLINE_CONTENT_SECTIONS: &[&str] = &["lspServers", "lsp_servers", "monitors"];

/// Extract the materialisable manifest for the plugin `name` from the raw
/// marketplace document, when the entry carries recognised manifest content
/// inline.
///
/// Returns `Some(json)` when the entry named `name` carries a hosting-config
/// section ([`INLINE_CONTENT_SECTIONS`], e.g. `lspServers`), with the
/// listing-only fields stripped. Returns `None` for a name the document does
/// not list, an entry with no inline content, or a document that cannot be
/// parsed - the install then proceeds without materialisation, so a plain
/// listing entry still finds the real manifest in its clone.
#[must_use]
pub fn inline_manifest_for(document: &str, name: &str) -> Option<String> {
    let root: serde_json::Value = serde_json::from_str(document).ok()?;
    inline_manifest_in(&root, name)
}

/// The [`serde_json::Value`] form of [`inline_manifest_for`] for callers that
/// already hold a parsed document - avoids a serialise/re-parse round trip.
#[must_use]
pub fn inline_manifest_in(root: &serde_json::Value, name: &str) -> Option<String> {
    let plugins = match root {
        serde_json::Value::Array(items) => items.clone(),
        serde_json::Value::Object(map) => match map.get("plugins") {
            Some(serde_json::Value::Array(items)) => items.clone(),
            _ => return None,
        },
        _ => return None,
    };
    for item in plugins {
        let serde_json::Value::Object(mut entry) = item else {
            continue;
        };
        let entry_name = entry
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if entry_name.is_empty() || entry_name != name.trim() {
            continue;
        }
        if !INLINE_CONTENT_SECTIONS
            .iter()
            .any(|s| entry.contains_key(*s))
        {
            return None;
        }
        for field in LISTING_ONLY_FIELDS {
            entry.remove(*field);
        }
        return serde_json::to_string_pretty(&serde_json::Value::Object(entry)).ok();
    }
    None
}

/// Look up the recorded inline manifest for plugin `name` under the
/// marketplace key `key` (the `#<key>` suffix of the install source).
///
/// Returns `None` when the key was never registered in this process or the
/// document lists no inline content for `name`.
#[must_use]
pub fn lookup(key: &str, name: &str) -> Option<String> {
    let document = lock_registry().get(key)?.clone();
    inline_manifest_for(&document, name)
}

/// Split an install source into `(source, key)` at the trailing
/// `@<sha256-hex>` inline-manifest marker.
///
/// A suffix is a key only when it is exactly 64 lowercase hex characters, so
/// an arbitrary `@` inside a URL (`https://user@host/...`) or a git ref is
/// never mistaken for one.
#[must_use]
pub fn split_manifest_key(source: &str) -> (&str, Option<&str>) {
    let Some((head, tail)) = source.rsplit_once(INLINE_MANIFEST_KEY_SEP) else {
        return (source, None);
    };
    let is_key = tail.len() == SHA256_HEX_LEN
        && tail
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if is_key {
        (head, Some(tail))
    } else {
        (source, None)
    }
}

/// Materialise the recorded inline manifest for a manifest-less plugin stub.
///
/// `root` is the plugin directory (the cloned repository, or the
/// sparse-checked-out subdirectory); `name` the marketplace entry's plugin
/// name; `key` the `#<key>` suffix of the install source. When the plugin
/// directory (at `root`, descending a single wrapper directory just like the
/// install pipeline does) already carries a recognisable manifest, or no
/// manifest was recorded, `root` is returned unchanged and nothing is
/// written.
///
/// On a hit the manifest is written to `<plugin>/.claude-plugin/plugin.json`
/// and the plugin directory is returned.
#[must_use]
pub fn materialize_at(
    root: std::path::PathBuf,
    name: &str,
    key: Option<&str>,
) -> std::path::PathBuf {
    let Some(key) = key else {
        return root;
    };
    // SEC-ragent-plugins-007 (SECTASKS T-062): `name` is the final `#`/`:`/`/`
    // separated segment of the install source and was joined without a `..`
    // guard, so a source ending in `:..` pointed `root.join(name)` at the
    // staging parent. Only a single normal component may be used to probe the
    // wrapper directory; otherwise return the root unchanged.
    if !is_single_normal_component(name) {
        return root;
    }
    let sub = root.join(name);
    // Probe `root` first, then a one-level `<root>/<name>` wrapper directory.
    // A shared outer `PathBuf` handle for the early return sidesteps
    // holding a borrow of `root` across the loop: each `Ok` arm records the
    // hit and we return the (un-borrowed) `root` once after the loop. This
    // also drops the original `root.clone()` head candidate.
    let mut done = false;
    for candidate in [&root, &sub] {
        if !candidate.is_dir() {
            continue;
        }
        match crate::descriptor::detect_dialect(candidate) {
            Ok(Some(_)) => done = true,
            Ok(None) => {
                let Some(manifest) = lookup(key, name) else {
                    continue;
                };
                let dir = candidate.join(".claude-plugin");
                if std::fs::create_dir_all(&dir).is_err() {
                    continue;
                }
                let path = dir.join("plugin.json");
                if std::fs::write(&path, manifest.as_bytes()).is_ok() {
                    tracing::info!(
                        plugin = %name,
                        manifest = %path.display(),
                        "materialised marketplace-inline plugin manifest"
                    );
                }
                done = true;
            }
            Err(e) => {
                tracing::debug!(
                    plugin = %name,
                    candidate = %candidate.display(),
                    error = %e,
                    "marketplace materialise: dialect probe failed"
                );
            }
        }
        if done {
            break;
        }
    }
    root
}

/// Whether `name` is a single non-empty `Path::Component::Normal` segment.
///
/// Guards the marketplace materialisation wrapper-directory join against a
/// source-derived name of `..` or a multi-segment/absolute value
/// (SEC-ragent-plugins-007, SECTASKS T-062).
fn is_single_normal_component(name: &str) -> bool {
    if name.trim().is_empty() {
        return false;
    }
    let mut components = std::path::Path::new(name).components();
    matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none()
}

//! Per-store index providers: dialect-specific transforms from a vendor
//! marketplace document into the internal store-entry model (spec
//! `pluginstores`; FR-003, FR-027, FR-029).
//!
//! Each store serves its catalogue in its own shape. A [`StoreProvider`] is the
//! strategy that knows one store's shape and normalises it into the same
//! [`StoreIndex`] the browser renders, so the rest of the store-browser domain
//! (fetch, filter, cursor, install) is store-agnostic:
//!
//! - [`CodexStoreProvider`] parses OpenAI's `openai/plugins` marketplace: a
//!   top-level `plugins` array whose entries carry a `name` (used as the
//!   plugin id) and a `source` that is either an object
//!   (`{"source":"local","path":"./plugins/x"}`, `{"source":"url","url":...}`,
//!   `{"source":"git-subdir","url":...,"path":...}`) or a plain string.
//! - [`ClaudeStoreProvider`] parses Anthropic's `claude-plugins-official`
//!   marketplace: entries carry a `name`, an optional `version`, and a `source`
//!   that is either a repo-relative string (`"./plugins/x"`) or an object
//!   (`{"source":"url","url":...}`, `{"source":"git-subdir",...}`).
//!
//! Both providers share the same tolerant, per-entry-skip discipline the native
//! parser already uses (FR-013, FR-025): an entry whose source cannot be resolved
//! is counted in [`StoreIndex::skipped`] rather than failing the whole index.
//!
//! ## Native documents are still parsed strictly
//!
//! ragent's own store-index format (entries carrying an `id`) is untouched: when
//! any entry in the document carries an `id`, the document is handed to the
//! strict native parser ([`StoreIndex::from_value`]) exactly as before, so the
//! hand-written fixtures and their validation tests are unaffected. Only a
//! document whose entries carry no `id` (a vendor catalogue) is run through the
//! tolerant vendor transform.
//!
//! ## Source resolution
//!
//! A vendor source is normalised into a string the existing install pipeline can
//! consume. Repo-relative paths are resolved against the marketplace document's
//! **repository root** (derived from a GitHub endpoint URL), producing an
//! installable `git+<repo>#<ref>:<path>` source; git sources are likewise
//! rendered as `git+<https-url>#<ref>:<path>`. The install pipeline accepts no
//! bare `https` directory URL, so a repo-relative entry is only installable when
//! the endpoint names a GitHub repository (`raw.githubusercontent.com` or
//! `github.com`). Sources that cannot be made installable (unknown scheme, empty
//! path, SSH remote, a non-GitHub origin) are reported as
//! [`StoreEntryError::Unsupported`] and the entry is skipped.

use reqwest::Url;
use serde_json::{Map, Value};

use crate::store_fetch::{StoreError, StoreIndex};
use crate::store_index::{StoreEntry, StoreEntryError, StoreKind};

/// A source value resolved from one vendor marketplace entry.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ResolvedSource {
    /// A source string the install pipeline accepts.
    Ready(String),
    /// The source shape is not installable by ragent; the entry is skipped.
    Unsupported(String),
}

/// The strategy that knows one store's marketplace shape.
///
/// Implementers are stateless unit values selected by [`provider_for`]. The
/// trait is the seam that makes the store-browser domain store-agnostic: the
/// fetch path, the fixture seam, and `probe_stores` all parse through a
/// provider rather than assuming a single catalogue format.
pub trait StoreProvider: Send + Sync {
    /// The store this provider parses.
    fn kind(&self) -> StoreKind;

    /// Parse a marketplace document into a [`StoreIndex`].
    ///
    /// `origin` is the endpoint URL the bytes were fetched from; it anchors
    /// repo-relative sources. Malformed JSON is a [`StoreError::MalformedJson`];
    /// a document with no `plugins` array is a [`StoreError::MalformedShape`];
    /// an entry that cannot be normalised is skipped and counted (FR-013).
    fn parse_index(&self, bytes: &[u8], origin: &Url) -> Result<StoreIndex, StoreError>;
}

/// The provider for `kind` (FR-001, FR-003).
#[must_use]
pub fn provider_for(kind: StoreKind) -> Box<dyn StoreProvider> {
    match kind {
        StoreKind::Codex => Box::new(CodexStoreProvider),
        StoreKind::Claude => Box::new(ClaudeStoreProvider),
    }
}

/// The OpenAI `openai/plugins` marketplace provider.
#[derive(Debug, Clone, Copy, Default)]
pub struct CodexStoreProvider;

impl StoreProvider for CodexStoreProvider {
    fn kind(&self) -> StoreKind {
        StoreKind::Codex
    }

    fn parse_index(&self, bytes: &[u8], origin: &Url) -> Result<StoreIndex, StoreError> {
        parse_index_with(bytes, origin, StoreKind::Codex, resolve_codex_source)
    }
}

/// The Anthropic `claude-plugins-official` marketplace provider.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeStoreProvider;

impl StoreProvider for ClaudeStoreProvider {
    fn kind(&self) -> StoreKind {
        StoreKind::Claude
    }

    fn parse_index(&self, bytes: &[u8], origin: &Url) -> Result<StoreIndex, StoreError> {
        parse_index_with(bytes, origin, StoreKind::Claude, resolve_claude_source)
    }
}

/// Parse `bytes` with a per-store `resolver`, returning a [`StoreIndex`].
///
/// Native documents (any entry carries an `id`) are delegated to the strict
/// native parser so existing fixtures and tests keep their exact behaviour; a
/// vendor document is normalised entry by entry with `resolver`.
fn parse_index_with(
    bytes: &[u8],
    origin: &Url,
    kind: StoreKind,
    resolver: fn(&Value, &Url) -> ResolvedSource,
) -> Result<StoreIndex, StoreError> {
    let root: Value = serde_json::from_slice(bytes).map_err(|e| StoreError::MalformedJson {
        detail: e.to_string(),
    })?;
    let items = plugins_array(&root)?;

    // A native ragent store index carries explicit `id` fields; parse it exactly
    // as before so its strict validation is unchanged.
    if items.iter().any(entry_has_id) {
        return StoreIndex::from_value(&root);
    }

    let store = root
        .get("store")
        .and_then(Value::as_str)
        .map(str::to_string);

    let mut entries = Vec::new();
    let mut skipped = 0usize;
    for item in items {
        match transform_vendor_entry(item, origin, kind, resolver) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                skipped += 1;
                tracing::debug!(error = %e, "store entry skipped");
            }
        }
    }
    Ok(StoreIndex {
        store,
        entries,
        skipped,
    })
}

/// The `plugins` array from a marketplace document.
///
/// Accepts the spec object shape (`{"plugins":[...]}`) and, tolerantly, a bare
/// top-level JSON array. Anything else is a [`StoreError::MalformedShape`].
fn plugins_array(root: &Value) -> Result<&Vec<Value>, StoreError> {
    match root {
        Value::Array(items) => Ok(items),
        Value::Object(map) => match map.get("plugins") {
            Some(Value::Array(items)) => Ok(items),
            _ => Err(StoreError::MalformedShape {
                detail: "object is missing a \"plugins\" array".to_string(),
            }),
        },
        _ => Err(StoreError::MalformedShape {
            detail: "expected an object with a \"plugins\" array or a bare array".to_string(),
        }),
    }
}

/// Whether a value is an object carrying an `id` field (the native marker).
fn entry_has_id(item: &Value) -> bool {
    item.as_object()
        .map(|map| map.contains_key("id"))
        .unwrap_or(false)
}

/// Normalise one vendor marketplace entry into a [`StoreEntry`].
fn transform_vendor_entry(
    item: &Value,
    origin: &Url,
    kind: StoreKind,
    resolver: fn(&Value, &Url) -> ResolvedSource,
) -> Result<StoreEntry, StoreEntryError> {
    let map = item.as_object().ok_or(StoreEntryError::NotAnObject)?;

    let name_field = required_str(map, "name");
    let id = required_str(map, "id")
        .or_else(|| name_field.clone())
        .ok_or(StoreEntryError::EmptyField("id"))?;
    let name = name_field.unwrap_or_else(|| id.clone());
    let version = map
        .get("version")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("0")
        .to_string();
    let description = map
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let homepage = map
        .get("homepage")
        .and_then(Value::as_str)
        .filter(|h| !h.trim().is_empty())
        .map(str::to_string);
    let tags = collect_tags(map);
    let dialect = map
        .get("dialect")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| Some(kind.token().to_string()));

    let source_value = map
        .get("source")
        .ok_or(StoreEntryError::EmptyField("source"))?;
    let source = match resolver(source_value, origin) {
        ResolvedSource::Ready(source) => source,
        ResolvedSource::Unsupported(detail) => return Err(StoreEntryError::Unsupported(detail)),
    };

    Ok(StoreEntry {
        id,
        name,
        version,
        source,
        description,
        dialect,
        tags,
        homepage,
    })
}

/// A non-empty string field, or `None`.
fn required_str(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
}

/// Collect `tags` (or, as a fallback, `keywords`) from a vendor entry.
fn collect_tags(map: &Map<String, Value>) -> Vec<String> {
    for key in ["tags", "keywords"] {
        if let Some(Value::Array(items)) = map.get(key) {
            let tags: Vec<String> = items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            if !tags.is_empty() {
                return tags;
            }
        }
    }
    Vec::new()
}

/// Resolve a Codex marketplace `source` value.
fn resolve_codex_source(source: &Value, origin: &Url) -> ResolvedSource {
    match source {
        Value::String(raw) => resolve_relative(raw, origin),
        Value::Object(map) => match map.get("source").and_then(Value::as_str) {
            Some("local") => map
                .get("path")
                .and_then(Value::as_str)
                .map(|path| resolve_relative(path, origin))
                .unwrap_or_else(|| {
                    ResolvedSource::Unsupported(
                        "codex source: local entry without a path".to_string(),
                    )
                }),
            Some("url") => map
                .get("url")
                .and_then(Value::as_str)
                .map(git_https_source)
                .unwrap_or_else(|| {
                    ResolvedSource::Unsupported("codex source: url entry without a url".to_string())
                }),
            Some("git-subdir") => git_subdir_source(map, "codex"),
            Some(other) => {
                ResolvedSource::Unsupported(format!("codex source: unsupported kind {other}"))
            }
            None => ResolvedSource::Unsupported(
                "codex source: object without a source kind".to_string(),
            ),
        },
        _ => ResolvedSource::Unsupported("codex source: not a string or object".to_string()),
    }
}

/// Resolve a Claude marketplace `source` value.
fn resolve_claude_source(source: &Value, origin: &Url) -> ResolvedSource {
    match source {
        Value::String(raw) => resolve_relative(raw, origin),
        Value::Object(map) => match map.get("source").and_then(Value::as_str) {
            Some("url") | Some("git") | Some("github") => map
                .get("url")
                .and_then(Value::as_str)
                .map(git_https_source)
                .unwrap_or_else(|| {
                    ResolvedSource::Unsupported(
                        "claude source: url entry without a url".to_string(),
                    )
                }),
            Some("git-subdir") => git_subdir_source(map, "claude"),
            Some(other) => {
                ResolvedSource::Unsupported(format!("claude source: unsupported kind {other}"))
            }
            None => ResolvedSource::Unsupported(
                "claude source: object without a source kind".to_string(),
            ),
        },
        _ => ResolvedSource::Unsupported("claude source: not a string or object".to_string()),
    }
}

/// Resolve a repo-relative (or already absolute) string source.
///
/// A path is anchored to a repository root only when the marketplace document
/// was fetched from a GitHub repository URL (`raw.githubusercontent.com` or
/// `github.com`), which yields an installable `git+<repo>#<ref>:<path>` source.
/// For any other origin (a bare directory listing, a local fixture path) there is
/// no GitHub repository to install from, so a repo-relative path cannot be made
/// installable and the entry is skipped rather than pointing at a non-existent
/// `https` directory (FR-044).
fn resolve_relative(raw: &str, origin: &Url) -> ResolvedSource {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return ResolvedSource::Unsupported("empty source".to_string());
    }
    if trimmed.starts_with("https://") {
        return ResolvedSource::Ready(trimmed.to_string());
    }
    if trimmed.contains("://") || trimmed.starts_with("git@") {
        return ResolvedSource::Unsupported(format!("non-https source {trimmed}"));
    }
    // A repo-relative path ("./plugins/x"); resolve it against the repo root.
    let path = trimmed.trim_start_matches("./").trim_start_matches('/');
    if path.is_empty() {
        return ResolvedSource::Unsupported(format!("empty repo-relative source {trimmed}"));
    }
    match repo_git_source(origin, path) {
        Some(source) => ResolvedSource::Ready(source),
        None => ResolvedSource::Unsupported(format!(
            "repo-relative source {trimmed} has no github repository to install from"
        )),
    }
}

/// Build a `git+<github-repo>#<ref>:<path>` source for a repo-relative entry.
///
/// Returns `None` when `origin` is not a GitHub repository URL: only
/// `raw.githubusercontent.com` (`/<owner>/<repo>/<ref>/<path>`) and
/// `github.com/<owner>/<repo>/...` addresses name an installable repository.
/// GitHub's HTML `tree`/`blob`/`raw` view segments are not part of the repo path
/// and are discarded, so a `github.com/.../tree/<ref>/<dir>` origin still anchors
/// the entry at the repository root with `<ref>` and `<dir>` applied.
fn repo_git_source(origin: &Url, path: &str) -> Option<String> {
    let host = origin.host_str()?;
    let segments: Vec<&str> = origin.path().split('/').filter(|s| !s.is_empty()).collect();
    let (owner, repo, ref_name) = if host == "raw.githubusercontent.com" {
        // /<owner>/<repo>/<ref>/<path...>
        if segments.len() < 3 {
            return None;
        }
        (segments[0], segments[1], segments[2])
    } else if host == "github.com" || host == "www.github.com" {
        if segments.len() < 2 {
            return None;
        }
        // Optional `/tree/<ref>` or `/blob/<ref>` view segment carries the ref.
        let view_ref = match segments.get(2) {
            Some(&"tree" | &"blob" | &"raw") => segments.get(3).copied(),
            _ => None,
        };
        (segments[0], segments[1], view_ref.unwrap_or("HEAD"))
    } else {
        return None;
    };
    let path = path.trim_start_matches('/');
    let ref_name = if ref_name.trim().is_empty() {
        "HEAD"
    } else {
        ref_name
    };
    Some(format!(
        "git+https://github.com/{owner}/{repo}#{ref_name}:{path}"
    ))
}

/// Resolve a `git-subdir` source object into `git+<https-url>#<ref>:<path>`.
fn git_subdir_source(map: &Map<String, Value>, store: &str) -> ResolvedSource {
    let Some(url) = map.get("url").and_then(Value::as_str) else {
        return ResolvedSource::Unsupported(format!("{store} source: git-subdir without a url"));
    };
    let Some(path) = map
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|p| !p.is_empty())
    else {
        return ResolvedSource::Unsupported(format!("{store} source: git-subdir without a path"));
    };
    let ref_name = map
        .get("ref")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .unwrap_or("HEAD");
    let normalized = normalize_git_https(url);
    if !normalized.starts_with("https://") {
        return ResolvedSource::Unsupported(format!("{store} source: non-https git remote {url}"));
    }
    let path = path.trim_start_matches("./");
    ResolvedSource::Ready(format!("git+{normalized}#{ref_name}:{path}"))
}

/// Resolve a whole-repository git source into `git+<https-url>#HEAD`.
fn git_https_source(raw: &str) -> ResolvedSource {
    let normalized = normalize_git_https(raw);
    if normalized.starts_with("https://") {
        ResolvedSource::Ready(format!("git+{normalized}#HEAD"))
    } else {
        ResolvedSource::Unsupported(format!("non-https git remote {raw}"))
    }
}

/// Strip a `git+` prefix and a trailing `.git` from a git remote URL.
fn normalize_git_https(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(rest) = s.strip_prefix("git+") {
        s = rest;
    }
    s.strip_suffix(".git").unwrap_or(s).to_string()
}

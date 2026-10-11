//! `/connectors marketplace <store> add|remove` source resolution and the
//! durable marketplace ledger (spec `connectors` FR-043).
//!
//! This module owns the two pieces the `marketplace` command family needs beyond
//! the pure report rendering in [`crate::commands`], ported from
//! `ragent_plugins::marketplace_ops` (spec `pluginstores` FR-048):
//!
//! - [`resolve_source`] turns a user-typed marketplace source into a display
//!   `name` and (when fetchable) an `https` marketplace-index URL. Four source
//!   forms are accepted, mirroring the `/plugins marketplace add` prompt:
//!   - `owner/repo` (GitHub shorthand) - resolved to the catalogue's raw
//!     marketplace document on branch `main`;
//!   - `git@github.com:owner/repo.git` (SSH) - resolved the same way;
//!   - `https://.../marketplace.json` (or a GitHub repo URL) - used directly;
//!   - `./path/to/marketplace` (a local directory) - recorded as a local,
//!     non-fetchable marketplace (its `name` is read from the manifest when it
//!     is readable).
//! - [`add_marketplace`] / [`remove_marketplace`] edit the **user-global**
//!   `connectors.stores.marketplaces` array in `~/.config/ragent/ragent.json`
//!   with a minimal, atomic JSON splice, so a user-owned marketplace is never
//!   shadowed by (or written into) an untrusted project config.
//!
//! No network access happens here: a source is resolved syntactically and the
//! URL is recorded for the browser to fetch later.

use std::path::Path;

use serde_json::Value;

/// A resolved marketplace source: a display name and, when it is fetchable over
/// `https`, the marketplace-index URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceSource {
    /// The marketplace display name (the repo slug, or a local directory name).
    pub name: String,
    /// The `https` marketplace-index URL, or `None` for a local (non-fetchable)
    /// marketplace.
    pub url: Option<String>,
}

/// Why a marketplace source could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarketplaceSourceError {
    /// The source string was empty or whitespace-only.
    #[error("no marketplace source given")]
    Empty,
    /// The source carried no recognisable owner/repo or URL.
    #[error(
        "unrecognised marketplace source `{0}`: expected owner/repo, an https:// \
             URL, or a local marketplace directory"
    )]
    Unsupported(String),
}

/// The Claude catalogue's marketplace document path (relative to a repository
/// root) - the `.claude-plugin/marketplace.json` shape the Claude plugin
/// marketplaces publish, reused so one repo layout serves both subsystems (spec
/// `pluginstores` FR-048).
pub const MARKETPLACE_DOC_PATH: &str = ".claude-plugin/marketplace.json";

/// Resolve a user-typed marketplace `source` (spec `connectors` FR-043).
///
/// Accepts `owner/repo`, `git@github.com:owner/repo.git`, an `https://` URL, or
/// a local directory path. Returns a [`MarketplaceSourceError`] for an empty or
/// unrecognisable source. Performs no network access; the URL is derived
/// syntactically and anchored to branch `main`.
pub fn resolve_source(
    source: &str,
    cwd: &Path,
) -> Result<MarketplaceSource, MarketplaceSourceError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(MarketplaceSourceError::Empty);
    }

    // SSH git remote: git@github.com:owner/repo.git
    if let Some(rest) = source.strip_prefix("git@github.com:") {
        let slug = rest.trim_end_matches(".git").trim_end_matches('/');
        return owner_repo_slug(slug)
            .ok_or_else(|| MarketplaceSourceError::Unsupported(source.to_string()));
    }

    // GitHub repo URL: https://github.com/owner/repo (optionally .git).
    if let Some(rest) = source
        .strip_prefix("https://github.com/")
        .or_else(|| source.strip_prefix("http://github.com/"))
    {
        let slug = rest
            .trim_end_matches(".git")
            .trim_end_matches('/')
            .split('/')
            .take(2)
            .collect::<Vec<_>>()
            .join("/");
        if !slug.contains('/') {
            return Err(MarketplaceSourceError::Unsupported(source.to_string()));
        }
        return owner_repo_slug(&slug)
            .ok_or_else(|| MarketplaceSourceError::Unsupported(source.to_string()));
    }

    // Any other https URL: a marketplace index document (or a raw repo file).
    if source.starts_with("https://") {
        let name = name_from_url(source);
        return Ok(MarketplaceSource {
            name,
            url: Some(source.to_string()),
        });
    }

    // Explicit local directory: ./path or /abs/path or a bare path that exists.
    let looks_local = source.starts_with("./")
        || source.starts_with("../")
        || source.starts_with('/')
        || cwd.join(source).is_dir();
    if looks_local {
        let path = if source.starts_with('/') {
            std::path::PathBuf::from(source)
        } else {
            cwd.join(source)
        };
        let name = local_marketplace_name(&path)
            .or_else(|| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "local".to_string());
        return Ok(MarketplaceSource { name, url: None });
    }

    // Bare `owner/repo` GitHub shorthand.
    if let Some(resolved) = owner_repo_slug(source) {
        return Ok(resolved);
    }

    Err(MarketplaceSourceError::Unsupported(source.to_string()))
}

/// Build the resolved source for an `owner/repo` slug, or `None` when the slug
/// is not a two-segment pair.
fn owner_repo_slug(slug: &str) -> Option<MarketplaceSource> {
    let slug = slug.trim_matches('/');
    let (owner, repo) = slug.split_once('/')?;
    if owner.is_empty() || repo.is_empty() || repo.contains('/') {
        return None;
    }
    Some(MarketplaceSource {
        name: repo.to_string(),
        url: Some(format!(
            "https://raw.githubusercontent.com/{owner}/{repo}/main/{MARKETPLACE_DOC_PATH}"
        )),
    })
}

/// Derive a display name from a marketplace-index URL.
///
/// A `raw.githubusercontent.com/<owner>/<repo>/...` URL names the repo; any
/// other URL falls back to its final path segment before the file name, then
/// the host.
fn name_from_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://raw.githubusercontent.com/") {
        let mut segments = rest.split('/');
        if segments.next().is_some()
            && let Some(repo) = segments.next()
            && !repo.is_empty()
        {
            return repo.to_string();
        }
    }
    let trimmed = url.trim_end_matches('/');
    let without_file = trimmed
        .rsplit_once('/')
        .map(|(head, _)| head)
        .unwrap_or(trimmed);
    if let Some(last) = without_file.rsplit('/').next()
        && !last.is_empty()
        && !last.starts_with("https:")
    {
        return last.to_string();
    }
    "marketplace".to_string()
}

/// Read the `name` field from a local marketplace manifest, if it is present
/// and readable.
fn local_marketplace_name(path: &Path) -> Option<String> {
    let candidate = path.join(MARKETPLACE_DOC_PATH);
    let root = if candidate.is_file() {
        candidate
    } else {
        path.join("marketplace.json")
    };
    let bytes = std::fs::read(root).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    value
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Add (or replace) a marketplace entry in the user-global
/// `connectors.stores.marketplaces` array (spec `connectors` FR-043).
///
/// `store_token` is `claude`; the entry is matched by `name` within that
/// store. Without `force` a duplicate name is refused (returns `Ok(false)` and
/// leaves the file untouched); with `force` the existing entry is replaced.
///
/// # Errors
///
/// Returns a short message when the global config cannot be read, parsed, or
/// written.
pub fn add_marketplace(
    store_token: &str,
    name: &str,
    url: Option<&str>,
    source: &str,
    force: bool,
) -> Result<bool, String> {
    let path = global_config_path()?;
    let mut value = read_or_empty_object(&path)?;
    let marketplaces = marketplaces_array_mut(&mut value)?;

    let existing = marketplaces
        .iter()
        .position(|entry| matches_entry(entry, store_token, name));
    if let Some(index) = existing {
        if !force {
            return Ok(false);
        }
        marketplaces.remove(index);
    }
    let mut entry = serde_json::Map::new();
    entry.insert("store".to_string(), Value::String(store_token.to_string()));
    entry.insert("name".to_string(), Value::String(name.to_string()));
    if let Some(url) = url {
        entry.insert("url".to_string(), Value::String(url.to_string()));
    }
    entry.insert("source".to_string(), Value::String(source.to_string()));
    marketplaces.push(Value::Object(entry));
    write_config(&path, &value)?;
    Ok(true)
}

/// Remove the marketplace entry named `name` for `store_token` from the
/// user-global config (spec `connectors` FR-043).
///
/// Returns `Ok(true)` when an entry was removed and `Ok(false)` when no
/// matching entry existed (the file is left untouched in that case).
///
/// # Errors
///
/// Returns a short message when the global config cannot be read, parsed, or
/// written.
pub fn remove_marketplace(store_token: &str, name: &str) -> Result<bool, String> {
    let path = global_config_path()?;
    let mut value = read_or_empty_object(&path)?;
    let marketplaces = marketplaces_array_mut(&mut value)?;
    let before = marketplaces.len();
    marketplaces.retain(|entry| !matches_entry(entry, store_token, name));
    let removed = marketplaces.len() != before;
    if removed {
        write_config(&path, &value)?;
    }
    Ok(removed)
}

/// Whether a stored marketplace entry is the `name` entry for `store_token`.
///
/// The store token is compared case-insensitively and the name exactly, matching
/// the `add`/`remove` contract so both mutate the same entry.
fn matches_entry(entry: &Value, store_token: &str, name: &str) -> bool {
    entry
        .get("store")
        .and_then(Value::as_str)
        .is_some_and(|s| s.eq_ignore_ascii_case(store_token))
        && entry
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|n| n == name)
}

/// The user-global config path, or a short error when none can be resolved.
fn global_config_path() -> Result<std::path::PathBuf, String> {
    ragent_config::Config::global_config_path()
        .ok_or_else(|| "no user-global config directory could be resolved".to_string())
}

/// Read the global config as a JSON object, defaulting to an empty object when
/// the file is absent. A non-object or unparseable file yields an error so a
/// bad file is never silently overwritten with defaults.
fn read_or_empty_object(path: &Path) -> Result<Value, String> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|e| format!("global config {} is not valid JSON: {e}", path.display()))?;
            if value.is_object() {
                Ok(value)
            } else {
                Err(format!(
                    "global config {} is not a JSON object",
                    path.display()
                ))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Object(Default::default())),
        Err(e) => Err(format!("cannot read global config {}: {e}", path.display())),
    }
}

/// The mutable `connectors.stores.marketplaces` array inside `value`, creating
/// the `connectors` / `stores` / `marketplaces` path when any level is absent
/// (or a non-object, which is replaced).
fn marketplaces_array_mut(value: &mut Value) -> Result<&mut Vec<Value>, String> {
    let root = value
        .as_object_mut()
        .ok_or_else(|| "global config root is not an object".to_string())?;
    let connectors = root
        .entry("connectors")
        .or_insert_with(|| Value::Object(Default::default()));
    if !connectors.is_object() {
        *connectors = Value::Object(Default::default());
    }
    let connectors = connectors
        .as_object_mut()
        .ok_or_else(|| "global config connectors block is not an object".to_string())?;
    let stores = connectors
        .entry("stores")
        .or_insert_with(|| Value::Object(Default::default()));
    if !stores.is_object() {
        *stores = Value::Object(Default::default());
    }
    let stores = stores
        .as_object_mut()
        .ok_or_else(|| "global config connectors.stores block is not an object".to_string())?;
    let marketplaces = stores
        .entry("marketplaces")
        .or_insert_with(|| Value::Array(Vec::new()));
    if !marketplaces.is_array() {
        *marketplaces = Value::Array(Vec::new());
    }
    marketplaces
        .as_array_mut()
        .ok_or_else(|| "connectors.stores.marketplaces is not an array".to_string())
}

/// Write `value` back to `path` atomically (temp file in the same directory,
/// then rename), creating the parent directory when needed, and drop the config
/// crate's on-disk load cache so the next `Config::load` sees the edit.
fn write_config(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| format!("cannot serialise global config: {e}"))?;
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    std::fs::write(&tmp, &bytes).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("cannot replace {}: {e}", path.display()))?;
    // The write bypasses `Config::save`, so invalidate the M-025 load cache
    // explicitly; otherwise a `Config::load` earlier in this process (which
    // seeded the cache before the edit) would hide the new marketplace from the
    // `/connectors marketplace list` report and the browse fetch.
    ragent_config::Config::invalidate_load_cache();
    Ok(())
}

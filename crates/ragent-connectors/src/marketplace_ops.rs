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

// `MarketplaceSource` and `MarketplaceSourceError` are shared with the
// `/plugins` marketplace family (byte-identical copies before this); re-export
// the single definition.
pub use ragent_surface::marketplace::{MarketplaceSource, MarketplaceSourceError};

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
    ragent_surface::marketplace::resolve_source(source, cwd, MARKETPLACE_DOC_PATH)
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

/// Write `value` back to `path` atomically and drop the config crate's on-disk
/// load cache so the next `Config::load` sees the edit.
///
/// Aliases [`ragent_surface::config_splice::write_atomic_json`], the single
/// shared implementation (also used by the `/plugins` marketplace family).
use ragent_surface::config_splice::write_atomic_json as write_config;

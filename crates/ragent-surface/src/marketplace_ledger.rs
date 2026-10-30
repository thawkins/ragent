//! Shared marketplace-ledger editing for the `/plugins` and `/connectors`
//! marketplace families.
//!
//! Both subsystems keep a durable, user-global list of extra marketplaces at
//! `<root>.stores.marketplaces` in `~/.config/ragent/ragent.json`, where `<root>`
//! is `plugins` for `/plugins marketplace` and `connectors` for
//! `/connectors marketplace`. `ragent-plugins/src/marketplace_ops.rs` and
//! `ragent-connectors/src/marketplace_ops.rs` carried byte-identical copies of
//! the six helpers below, differing only in that one JSON key (verified by
//! diff). They now have one implementation here, parameterised by the root key,
//! so the atomic-write discipline and the "a bad file is never silently
//! overwritten" rule cannot drift between the two subsystems.
//!
//! No network access happens in this module: entries record a URL that the
//! caller's browser fetches later.

use std::path::Path;

use serde_json::Value;

use crate::config_splice::write_atomic_json;

/// Whether a stored marketplace entry is the `name` entry for `store_token`.
///
/// The store token is compared case-insensitively and the name exactly, matching
/// the `add`/`remove` contract so both mutate the same entry.
#[must_use]
pub fn matches_entry(entry: &Value, store_token: &str, name: &str) -> bool {
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
///
/// # Errors
///
/// Returns a message when the platform config directory cannot be determined.
pub fn global_config_path() -> Result<std::path::PathBuf, String> {
    ragent_config::Config::global_config_path()
        .ok_or_else(|| "no user-global config directory could be resolved".to_string())
}

/// Read the global config as a JSON object, defaulting to an empty object when
/// the file is absent.
///
/// A non-object or unparseable file yields an error so a bad file is never
/// silently overwritten with defaults.
///
/// # Errors
///
/// Returns a message when the file exists but cannot be read, is not valid JSON,
/// or is not a JSON object.
pub fn read_or_empty_object(path: &Path) -> Result<Value, String> {
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

/// The mutable `<root>.stores.marketplaces` array inside `value`, creating the
/// `<root>` / `stores` / `marketplaces` path when any level is absent (or a
/// non-object, which is replaced).
///
/// `root_key` is `plugins` or `connectors`.
///
/// # Errors
///
/// Returns a message naming the level that could not be materialised as an
/// object, or when `marketplaces` cannot be made an array.
pub fn marketplaces_array_mut<'a>(
    value: &'a mut Value,
    root_key: &str,
) -> Result<&'a mut Vec<Value>, String> {
    let root = value
        .as_object_mut()
        .ok_or_else(|| "global config root is not an object".to_string())?;
    let section = root
        .entry(root_key)
        .or_insert_with(|| Value::Object(Default::default()));
    if !section.is_object() {
        *section = Value::Object(Default::default());
    }
    let section = section
        .as_object_mut()
        .ok_or_else(|| format!("{root_key} config value is not an object"))?;
    let stores = section
        .entry("stores")
        .or_insert_with(|| Value::Object(Default::default()));
    if !stores.is_object() {
        *stores = Value::Object(Default::default());
    }
    let stores = stores
        .as_object_mut()
        .ok_or_else(|| format!("{root_key}.stores config value is not an object"))?;
    let marketplaces = stores
        .entry("marketplaces")
        .or_insert_with(|| Value::Array(Vec::new()));
    if !marketplaces.is_array() {
        *marketplaces = Value::Array(Vec::new());
    }
    marketplaces
        .as_array_mut()
        .ok_or_else(|| format!("{root_key}.stores.marketplaces is not an array"))
}

/// Add (or replace) a marketplace entry in the user-global
/// `<root_key>.stores.marketplaces` array.
///
/// `store_token` identifies the store (`codex` / `claude`); the entry is matched
/// by `name` within that store. Without `force` a duplicate name is refused
/// (returns `Ok(false)` and leaves the file untouched); with `force` the existing
/// entry is replaced.
///
/// # Errors
///
/// Returns a short message when the global config cannot be read, parsed, or
/// written.
pub fn add_marketplace(
    root_key: &str,
    store_token: &str,
    name: &str,
    url: Option<&str>,
    source: &str,
    force: bool,
) -> Result<bool, String> {
    let path = global_config_path()?;
    let mut value = read_or_empty_object(&path)?;
    let marketplaces = marketplaces_array_mut(&mut value, root_key)?;

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
    write_atomic_json(&path, &value)?;
    Ok(true)
}

/// Remove the marketplace entry named `name` for `store_token` from the
/// user-global config.
///
/// Returns `Ok(true)` when an entry was removed and `Ok(false)` when no matching
/// entry existed (the file is left untouched in that case).
///
/// # Errors
///
/// Returns a short message when the global config cannot be read, parsed, or
/// written.
pub fn remove_marketplace(root_key: &str, store_token: &str, name: &str) -> Result<bool, String> {
    let path = global_config_path()?;
    let mut value = read_or_empty_object(&path)?;
    let marketplaces = marketplaces_array_mut(&mut value, root_key)?;
    let before = marketplaces.len();
    marketplaces.retain(|entry| !matches_entry(entry, store_token, name));
    let removed = marketplaces.len() != before;
    if removed {
        write_atomic_json(&path, &value)?;
    }
    Ok(removed)
}

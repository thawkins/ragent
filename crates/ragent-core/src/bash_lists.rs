//! Runtime bash allowlist / denylist management.
//!
//! At startup the lists are populated from the merged global (`~/.config/ragent/ragent.json`)
//! and project (`./ragent.json`) config files.  The `/bash add|remove` slash commands then
//! mutate the in-memory lists and persist changes back to whichever config file the user
//! targets (project by default, global with the `--global` flag).
//!
//! # Interaction with validation in `tool::bash`
//!
//! - **allowlist** entries are command prefixes.  A command whose first word matches an
//!   allowlist entry is exempt from the built-in banned-command check, allowing e.g. `curl`
//!   to be re-enabled without entering YOLO mode.
//! - **denylist** entries are substring patterns.  If any entry is found anywhere in the
//!   command string the command is rejected unconditionally.

use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

use anyhow::{Context, Result};

// ── Global state ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct BashLists {
    /// Command prefixes exempted from the banned-command check.
    pub allowlist: Vec<String>,
    /// Substring patterns that unconditionally reject a command.
    pub denylist: Vec<String>,
}

static BASH_LISTS: OnceLock<RwLock<BashLists>> = OnceLock::new();

fn global() -> &'static RwLock<BashLists> {
    BASH_LISTS.get_or_init(|| RwLock::new(BashLists::default()))
}

// ── Initialisation ────────────────────────────────────────────────────────────

/// Load the bash lists from the merged global + project config.
///
/// Call this once at startup. Subsequent loads (e.g. after `/reload`) replace
/// the in-memory state.
pub fn load_from_config() {
    let lists = match crate::config::Config::load() {
        Ok(cfg) => BashLists {
            allowlist: cfg.bash.allowlist,
            denylist: cfg.bash.denylist,
        },
        Err(e) => {
            tracing::warn!("bash_lists: failed to load config: {e}");
            BashLists::default()
        }
    };

    if let Ok(mut guard) = global().write() {
        *guard = lists;
    }
}

// ── Read accessors ────────────────────────────────────────────────────────────

/// Returns a snapshot of the current allowlist.
pub fn get_allowlist() -> Vec<String> {
    global()
        .read()
        .map(|g| g.allowlist.clone())
        .unwrap_or_default()
}

/// Returns a snapshot of the current denylist.
pub fn get_denylist() -> Vec<String> {
    global()
        .read()
        .map(|g| g.denylist.clone())
        .unwrap_or_default()
}

/// Returns `true` if the command's first token matches any user-defined allowlist entry.
pub fn is_allowlisted(command: &str) -> bool {
    let first_token = command.split_whitespace().next().unwrap_or("");
    global().read().map_or(false, |g| {
        g.allowlist
            .iter()
            .any(|entry| first_token == entry.as_str())
    })
}

/// Returns the first user-defined denylist pattern that appears in `command`, if any.
pub fn matches_denylist(command: &str) -> Option<String> {
    global().read().ok().and_then(|g| {
        g.denylist
            .iter()
            .find(|p| command.contains(p.as_str()))
            .cloned()
    })
}

// ── Mutation helpers ──────────────────────────────────────────────────────────

/// The config scope targeted by a mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Write to `./ragent.json` (project-level config).
    Project,
    /// Write to `~/.config/ragent/ragent.json` (global config).
    Global,
}

impl Scope {
    fn config_path(self) -> Result<PathBuf> {
        match self {
            Scope::Project => Ok(PathBuf::from("ragent.json")),
            Scope::Global => {
                let dir = dirs::config_dir()
                    .context("Cannot determine global config directory")?;
                Ok(dir.join("ragent").join("ragent.json"))
            }
        }
    }
}

/// Add `entry` to the allowlist.  Persists to the chosen config file.
pub fn add_allowlist(entry: &str, scope: Scope) -> Result<()> {
    {
        let mut g = global().write().map_err(|_| anyhow::anyhow!("lock poisoned"))?;
        if !g.allowlist.contains(&entry.to_string()) {
            g.allowlist.push(entry.to_string());
        }
    }
    patch_config(scope, |bash| {
        if let Some(arr) = bash["allowlist"].as_array_mut() {
            let val = serde_json::Value::String(entry.to_string());
            if !arr.contains(&val) {
                arr.push(val);
            }
        }
    })
}

/// Remove `entry` from the allowlist.  Persists to the chosen config file.
pub fn remove_allowlist(entry: &str, scope: Scope) -> Result<bool> {
    let removed = {
        let mut g = global().write().map_err(|_| anyhow::anyhow!("lock poisoned"))?;
        let before = g.allowlist.len();
        g.allowlist.retain(|e| e != entry);
        g.allowlist.len() < before
    };
    patch_config(scope, |bash| {
        if let Some(arr) = bash["allowlist"].as_array_mut() {
            arr.retain(|v| v.as_str() != Some(entry));
        }
    })?;
    Ok(removed)
}

/// Add `pattern` to the denylist.  Persists to the chosen config file.
pub fn add_denylist(pattern: &str, scope: Scope) -> Result<()> {
    {
        let mut g = global().write().map_err(|_| anyhow::anyhow!("lock poisoned"))?;
        if !g.denylist.contains(&pattern.to_string()) {
            g.denylist.push(pattern.to_string());
        }
    }
    patch_config(scope, |bash| {
        if let Some(arr) = bash["denylist"].as_array_mut() {
            let val = serde_json::Value::String(pattern.to_string());
            if !arr.contains(&val) {
                arr.push(val);
            }
        }
    })
}

/// Remove `pattern` from the denylist.  Persists to the chosen config file.
pub fn remove_denylist(pattern: &str, scope: Scope) -> Result<bool> {
    let removed = {
        let mut g = global().write().map_err(|_| anyhow::anyhow!("lock poisoned"))?;
        let before = g.denylist.len();
        g.denylist.retain(|e| e != pattern);
        g.denylist.len() < before
    };
    patch_config(scope, |bash| {
        if let Some(arr) = bash["denylist"].as_array_mut() {
            arr.retain(|v| v.as_str() != Some(pattern));
        }
    })?;
    Ok(removed)
}

// ── Config file I/O ───────────────────────────────────────────────────────────

/// Read the target config as a JSON Value, apply `mutate` to the `bash` sub-object,
/// then write the result back.  Creates the file (and parent dirs) if absent.
fn patch_config<F>(scope: Scope, mutate: F) -> Result<()>
where
    F: FnOnce(&mut serde_json::Value),
{
    let path = scope.config_path()?;

    // Read existing content (empty object if file absent)
    let mut root: serde_json::Value = if path.exists() {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("Reading {}", path.display()))?;
        serde_json::from_str(&text)
            .with_context(|| format!("Parsing {}", path.display()))?
    } else {
        serde_json::json!({})
    };

    // Ensure the `bash` key is an object
    if !root["bash"].is_object() {
        root["bash"] = serde_json::json!({ "allowlist": [], "denylist": [] });
    }
    // Ensure both arrays exist
    if !root["bash"]["allowlist"].is_array() {
        root["bash"]["allowlist"] = serde_json::json!([]);
    }
    if !root["bash"]["denylist"].is_array() {
        root["bash"]["denylist"] = serde_json::json!([]);
    }

    mutate(&mut root["bash"]);

    // Write back
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Creating directory {}", parent.display()))?;
        }
    }
    let text = serde_json::to_string_pretty(&root)
        .context("Serialising updated config")?;
    std::fs::write(&path, text)
        .with_context(|| format!("Writing {}", path.display()))?;

    tracing::info!(path = %path.display(), "bash_lists: config updated");
    Ok(())
}

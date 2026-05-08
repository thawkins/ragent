//! Runtime directory/file allowlist / denylist management for permissions.
//!
//! At startup the lists are populated from the merged global (`~/.config/ragent/ragent.json`)
//! and project (`./.ragent/ragent.json`) config files. The `/dirs add|remove` slash commands then
//! mutate the in-memory lists and persist changes back to whichever config file the user
//! targets (project by default, global with the `--global` flag).
//!
//! # Interaction with permission checking
//!
//! - **allowlist** entries are glob patterns. File operations matching these patterns
//!   are automatically allowed without prompting.
//! - **denylist** entries are glob patterns. File operations matching these patterns
//!   are automatically denied without prompting.
//!
//! The permission system checks these lists before prompting the user.

use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

use anyhow::{Context, Result};
use globset::{GlobSet, GlobSetBuilder};

// ── Built-in directory patterns ───────────────────────────────────────────────

/// Built-in allowlist: directory patterns that are automatically allowed without prompting.
///
/// These patterns are always considered safe for file operations and do not require
/// user confirmation. Empty by default, but can be extended with commonly safe patterns.
#[allow(dead_code)]
pub const BUILTIN_ALLOWLIST: &[&str] = &[
    // Currently empty - can be extended with safe patterns like:
    // "target/**",    // Build artifacts
    // ".git/**",      // Git internals
    // "node_modules/**", // Dependencies
];

/// Built-in denylist: directory patterns that are automatically denied without prompting.
///
/// These patterns represent dangerous locations where file operations should always be blocked
/// to prevent accidental system damage or data loss.
#[allow(dead_code)]
pub const BUILTIN_DENYLIST: &[&str] = &[
    // System-critical directories
    "/bin/**",
    "/sbin/**",
    "/boot/**",
    "/dev/**",
    "/proc/**",
    "/sys/**",
    "/etc/**",
    "/usr/bin/**",
    "/usr/sbin/**",
    "/usr/lib/**",
    "/lib/**",
    "/lib64/**",
    // macOS system directories
    "/System/**",
    "/Library/**",
    "/Applications/**",
    "/private/**",
    // Windows system directories
    "C:/Windows/**",
    "C:/Program Files/**",
    "C:/Program Files (x86)/**",
];

/// Returns the built-in allowlist and denylist patterns.
///
/// These are the baseline patterns that are always active before user-defined
/// patterns are applied.
#[must_use]
pub fn get_builtin_lists() -> (Vec<String>, Vec<String>) {
    (
        BUILTIN_ALLOWLIST.iter().map(|s| (*s).to_string()).collect(),
        BUILTIN_DENYLIST.iter().map(|s| (*s).to_string()).collect(),
    )
}

// ── Global state ─────────────────────────────────────────────────────────────

/// In-memory snapshot of the merged directory allowlist and denylist.
#[derive(Debug, Clone, Default)]
pub struct DirLists {
    /// Glob patterns for file operations automatically allowed.
    pub allowlist: Vec<String>,
    /// Glob patterns for file operations automatically denied.
    pub denylist: Vec<String>,
}

/// Compiled glob patterns for efficient matching.
pub struct CompiledDirLists {
    /// Compiled allowlist patterns.
    pub allowlist: GlobSet,
    /// Compiled denylist patterns.
    pub denylist: GlobSet,
}

impl CompiledDirLists {
    /// Check if a resource matches the allowlist.
    #[must_use]
    pub fn is_allowed(&self, resource: &str) -> bool {
        self.allowlist.is_match(resource)
    }

    /// Check if a resource matches the denylist.
    #[must_use]
    pub fn is_denied(&self, resource: &str) -> bool {
        self.denylist.is_match(resource)
    }
}

static DIR_LISTS: OnceLock<RwLock<DirLists>> = OnceLock::new();
static COMPILED_ALLOWLIST: OnceLock<RwLock<GlobSet>> = OnceLock::new();
static COMPILED_DENYLIST: OnceLock<RwLock<GlobSet>> = OnceLock::new();

fn global() -> &'static RwLock<DirLists> {
    DIR_LISTS.get_or_init(|| RwLock::new(DirLists::default()))
}

fn compiled_allowlist() -> &'static RwLock<GlobSet> {
    COMPILED_ALLOWLIST.get_or_init(|| RwLock::new(GlobSet::empty()))
}

fn compiled_denylist() -> &'static RwLock<GlobSet> {
    COMPILED_DENYLIST.get_or_init(|| RwLock::new(GlobSet::empty()))
}

/// Recompile patterns and update the compiled allowlist cache.
fn recompile_allowlist() -> Result<()> {
    let g = global()
        .read()
        .map_err(|_| anyhow::anyhow!("lock poisoned"))?;
    let compiled = compile_patterns(&g.allowlist);
    if let Ok(mut guard) = compiled_allowlist().write() {
        *guard = compiled;
    }
    Ok(())
}

/// Recompile patterns and update the compiled denylist cache.
fn recompile_denylist() -> Result<()> {
    let g = global()
        .read()
        .map_err(|_| anyhow::anyhow!("lock poisoned"))?;
    let compiled = compile_patterns(&g.denylist);
    if let Ok(mut guard) = compiled_denylist().write() {
        *guard = compiled;
    }
    Ok(())
}

/// Compile a list of glob patterns into a GlobSet.
fn compile_patterns(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if let Ok(glob) = globset::Glob::new(pattern) {
            builder.add(glob);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

// ── Initialisation ────────────────────────────────────────────────────────────

/// Load the directory lists from the merged global + project config.
///
/// Call this once at startup. Subsequent loads (e.g. after `/reload`) replace
 /// the in-memory state.
 pub fn load_from_config() {
     let lists = match crate::config::Config::load() {
         Ok(cfg) => {
             let mut allowlist = Vec::new();
             let mut denylist = Vec::new();
 
             // Use the new dedicated dirs field from config
             allowlist.extend(cfg.dirs.allowlist);
             denylist.extend(cfg.dirs.denylist);
 
             DirLists {
                 allowlist,
                 denylist,
             }
         }
         Err(e) => {
             tracing::warn!("dir_lists: failed to load config: {e}");
             DirLists::default()
         }
     };
 
     // Compile patterns for efficient matching
     let compiled_allow = compile_patterns(&lists.allowlist);
     let compiled_deny = compile_patterns(&lists.denylist);
 
     if let Ok(mut guard) = global().write() {
         *guard = lists;
     }
     if let Ok(mut guard) = compiled_allowlist().write() {
         *guard = compiled_allow;
     }
     if let Ok(mut guard) = compiled_denylist().write() {
         *guard = compiled_deny;
     }
 }
// ── Read accessors ────────────────────────────────────────────────────────────

/// Returns a snapshot of the current allowlist.
#[must_use]
pub fn get_allowlist() -> Vec<String> {
    global()
        .read()
        .map(|g| g.allowlist.clone())
        .unwrap_or_default()
}

/// Returns a snapshot of the current denylist.
#[must_use]
pub fn get_denylist() -> Vec<String> {
    global()
        .read()
        .map(|g| g.denylist.clone())
        .unwrap_or_default()
}

/// Returns the compiled allowlist for efficient matching.
#[must_use]
pub fn get_compiled_allowlist() -> GlobSet {
    compiled_allowlist()
        .read()
        .map(|g| g.clone())
        .unwrap_or_else(|_| GlobSet::empty())
}

/// Returns the compiled denylist for efficient matching.
#[must_use]
pub fn get_compiled_denylist() -> GlobSet {
    compiled_denylist()
        .read()
        .map(|g| g.clone())
        .unwrap_or_else(|_| GlobSet::empty())
}

// ── Write accessors ───────────────────────────────────────────────────────────

/// Scope for config persistence: project-local or user-global.
#[derive(Debug, Clone, Copy)]
pub enum Scope {
    /// Write to `./.ragent/ragent.json` (project-level config).
    Project,
    /// Write to `~/.config/ragent/ragent.json` (global config).
    Global,
}

impl Scope {
    fn config_path(self) -> Result<PathBuf> {
        match self {
            Self::Project => Ok(PathBuf::from(".ragent").join("ragent.json")),
            Self::Global => {
                let dir = dirs::config_dir().context("Cannot determine global config directory")?;
                Ok(dir.join("ragent").join("ragent.json"))
            }
        }
    }
}

/// Add `pattern` to the allowlist. Persists to the chosen config file.
pub fn add_allowlist(pattern: &str, scope: Scope) -> Result<()> {
    {
        let mut g = global()
            .write()
            .map_err(|_| anyhow::anyhow!("lock poisoned"))?;
        if !g.allowlist.contains(&pattern.to_string()) {
            g.allowlist.push(pattern.to_string());
        }
    }
    recompile_allowlist()?;
    patch_config(scope, |root| {
        // Ensure dirs object exists
        if !root["dirs"].is_object() {
            root["dirs"] = serde_json::json!({ "allowlist": [], "denylist": [] });
        }
        if !root["dirs"]["allowlist"].is_array() {
            root["dirs"]["allowlist"] = serde_json::json!([]);
        }
        if !root["dirs"]["denylist"].is_array() {
            root["dirs"]["denylist"] = serde_json::json!([]);
        }

        if let Some(arr) = root["dirs"]["allowlist"].as_array_mut() {
            let val = serde_json::Value::String(pattern.to_string());
            if !arr.contains(&val) {
                arr.push(val);
            }
        }
    })
}

/// Remove `pattern` from the allowlist. Persists to the chosen config file.
pub fn remove_allowlist(pattern: &str, scope: Scope) -> Result<bool> {
    let removed = {
        let mut g = global()
            .write()
            .map_err(|_| anyhow::anyhow!("lock poisoned"))?;
        let before = g.allowlist.len();
        g.allowlist.retain(|e| e != pattern);
        g.allowlist.len() < before
    };
    recompile_allowlist()?;
    patch_config(scope, |root| {
        if let Some(arr) = root["dirs"]["allowlist"].as_array_mut() {
            arr.retain(|v| v.as_str() != Some(pattern));
        }
    })?;
    Ok(removed)
}

/// Add `pattern` to the denylist. Persists to the chosen config file.
pub fn add_denylist(pattern: &str, scope: Scope) -> Result<()> {
    {
        let mut g = global()
            .write()
            .map_err(|_| anyhow::anyhow!("lock poisoned"))?;
        if !g.denylist.contains(&pattern.to_string()) {
            g.denylist.push(pattern.to_string());
        }
    }
    recompile_denylist()?;
    patch_config(scope, |root| {
        if let Some(arr) = root["dirs"]["denylist"].as_array_mut() {
            let val = serde_json::Value::String(pattern.to_string());
            if !arr.contains(&val) {
                arr.push(val);
            }
        }
    })
}

/// Remove `pattern` from the denylist. Persists to the chosen config file.
pub fn remove_denylist(pattern: &str, scope: Scope) -> Result<bool> {
    let removed = {
        let mut g = global()
            .write()
            .map_err(|_| anyhow::anyhow!("lock poisoned"))?;
        let before = g.denylist.len();
        g.denylist.retain(|e| e != pattern);
        g.denylist.len() < before
    };
    recompile_denylist()?;
    patch_config(scope, |root| {
        if let Some(arr) = root["dirs"]["denylist"].as_array_mut() {
            arr.retain(|v| v.as_str() != Some(pattern));
        }
    })?;
    Ok(removed)
}

// ── Config file I/O ───────────────────────────────────────────────────────────

/// Read the target config as a JSON Value, apply `mutate` to the root object,
/// then write the result back. Creates the file (and parent dirs) if absent.
fn patch_config<F>(scope: Scope, mutate: F) -> Result<()>
where
    F: FnOnce(&mut serde_json::Value),
{
    let path = scope.config_path()?;

    // Read existing content (empty object if file absent)
    let mut root: serde_json::Value = if path.exists() {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("Reading {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("Parsing {}", path.display()))?
    } else {
        serde_json::json!({})
    };

    // Ensure the `dirs` key exists with allowlist/denylist arrays
    if !root["dirs"].is_object() {
        root["dirs"] = serde_json::json!({ "allowlist": [], "denylist": [] });
    }
    if !root["dirs"]["allowlist"].is_array() {
        root["dirs"]["allowlist"] = serde_json::json!([]);
    }
    if !root["dirs"]["denylist"].is_array() {
        root["dirs"]["denylist"] = serde_json::json!([]);
    }

    mutate(&mut root);

    // Write back
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Creating directory {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(&root).context("Serialising updated config")?;
    std::fs::write(&path, text).with_context(|| format!("Writing {}", path.display()))?;

    tracing::info!(path = %path.display(), "dir_lists: config updated");
    Ok(())
}

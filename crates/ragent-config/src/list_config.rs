//! Shared config-file plumbing for the runtime list modules (T-306).
//!
//! The `bash_lists` and `dir_lists` modules both persist list mutations to a
//! user-selected config file (project `./.ragent/ragent.json` or global
//! `~/.config/ragent/ragent.json`). The [`Scope`] type, the config-path
//! resolution, and the read-modify-write cycle were byte-identical copies in
//! each module; they now live here once and both modules delegate.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::{Value, json};

/// The config scope targeted by a runtime list mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Write to `./.ragent/ragent.json` (project-level config).
    Project,
    /// Write to `~/.config/ragent/ragent.json` (global config).
    Global,
}

impl Scope {
    /// Resolve the config file path this scope writes to.
    ///
    /// # Errors
    ///
    /// Returns an error when the global config directory cannot be determined.
    pub fn config_path(self) -> Result<PathBuf> {
        match self {
            Self::Project => Ok(PathBuf::from(".ragent").join("ragent.json")),
            Self::Global => crate::Config::global_config_path()
                .context("Cannot determine global config directory"),
        }
    }
}

/// Read `scope`'s config as JSON, run `mutate` on the named top-level section,
/// then write it back.
///
/// The section object is created when absent; each entry in `default_arrays`
/// is ensured to be a JSON array before `mutate` runs. The file and any missing
/// parent directories are created on write.
///
/// # Errors
///
/// Returns an error if the config cannot be read, parsed, or written.
pub fn patch_config<F>(
    scope: Scope,
    section: &str,
    default_arrays: &[&str],
    mutate: F,
) -> Result<()>
where
    F: FnOnce(&mut Value),
{
    let path = scope.config_path()?;

    // Read existing content (empty object if the file is absent).
    let mut root: Value = if path.exists() {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("Reading {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("Parsing {}", path.display()))?
    } else {
        json!({})
    };

    // Ensure the section object exists with the required array fields.
    if !root[section].is_object() {
        root[section] = json!({});
    }
    for field in default_arrays {
        if !root[section][field].is_array() {
            root[section][field] = json!([]);
        }
    }

    mutate(&mut root[section]);

    // Write back, creating the parent directory if needed.
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Creating directory {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(&root).context("Serialising updated config")?;
    std::fs::write(&path, text).with_context(|| format!("Writing {}", path.display()))?;

    tracing::info!(path = %path.display(), section, "list_config: config updated");
    Ok(())
}

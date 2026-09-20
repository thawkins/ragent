//! Plugin descriptor model and dialect recognition rules (spec `plugins`
//! T-002; FR-002, FR-025).
//!
//! Two third-party manifest dialects are recognised:
//!
//! - **Codex** — a directory containing `codex-plugin.json`, or a `plugin.json`
//!   whose top-level body contains a `"codex"` marker field.
//! - **Claude** — a directory containing `claude-plugin.json`, or a
//!   `.claude-plugin/plugin.json` file.
//!
//! Both dialects are normalised by later tasks (T-003) into the single
//! [`PluginDescriptor`] model defined here. Manifest sections with no ragent
//! equivalent are recorded verbatim in
//! [`PluginDescriptor::unsupported_capabilities`] and reported, never silently
//! dropped (FR-025).
//!
//! Recognition is pure and unit-testable: [`recognise_dialect`] reasons over an
//! abstract directory listing plus optional manifest bytes, so tests do not
//! touch the filesystem; [`detect_dialect`] is the thin filesystem wrapper used
//! by discovery (T-005).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::PluginError;

/// File name recognising the Codex dialect directly.
pub const CODEX_MANIFEST_FILE: &str = "codex-plugin.json";
/// Generic manifest file name; Codex when it carries a top-level `"codex"`
/// marker field (FR-002).
pub const GENERIC_MANIFEST_FILE: &str = "plugin.json";
/// Top-level JSON field that marks a `plugin.json` as a Codex manifest.
pub const CODEX_MARKER_FIELD: &str = "codex";
/// File name recognising the Claude dialect directly.
pub const CLAUDE_MANIFEST_FILE: &str = "claude-plugin.json";
/// Claude Code's nested manifest location (`.claude-plugin/plugin.json`).
pub const CLAUDE_NESTED_MANIFEST: &str = ".claude-plugin/plugin.json";

/// The manifest dialect a plugin directory was authored in.
///
/// Each directory carries exactly one dialect; a directory whose contents match
/// both dialects is ambiguous and rejected with
/// [`PluginError::AmbiguousManifest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginDialect {
    /// OpenAI Codex plugin (`codex-plugin.json` or marked `plugin.json`).
    Codex,
    /// Claude Code / Claude Desktop plugin (`claude-plugin.json` or
    /// `.claude-plugin/plugin.json`).
    Claude,
}

impl std::fmt::Display for PluginDialect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Codex => f.write_str("codex"),
            Self::Claude => f.write_str("claude"),
        }
    }
}

/// Normalised internal representation of a plugin manifest (FR-002).
///
/// Fields that have no ragent equivalent in the source dialect (for example
/// Claude Desktop MCP-server transport sections) are recorded in
/// [`unsupported_capabilities`](Self::unsupported_capabilities) so they are
/// reported rather than silently dropped (FR-025).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginDescriptor {
    /// Stable plugin identifier (used in tool names as `plugin_<id>_<tool>`).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Plugin version string as declared by the manifest.
    pub version: String,
    /// Recognised source dialect.
    pub dialect: PluginDialect,
    /// Entry-point JavaScript file, absolute within the plugin root.
    pub entry: PathBuf,
    /// Permissions the manifest requests (for example `"network.outbound"`).
    pub requested_permissions: Vec<String>,
    /// Host-API version the plugin was written against (FR-019 version check).
    pub api_version: u32,
    /// Capability descriptions from the manifest that the host API cannot
    /// satisfy; reported, never silently dropped (FR-025).
    pub unsupported_capabilities: Vec<String>,
    /// Absolute path of the manifest this descriptor was parsed from.
    pub manifest_path: PathBuf,
    /// Absolute path of the plugin root directory.
    pub root: PathBuf,
}

/// The result of recognising a plugin directory's dialect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialectMatch {
    /// The recognised dialect.
    pub dialect: PluginDialect,
    /// Manifest path relative to the plugin root directory.
    pub manifest_rel: PathBuf,
}

/// Recognise a directory's plugin dialect from a directory listing plus
/// optional manifest bytes (FR-002). Pure: performs no I/O itself.
///
/// `entries` are the directory's file and subdirectory names (a plain
/// `read_dir` listing suffices; subdirectory names should be passed with a
/// trailing path separator stripped, i.e. the plain entry name — subdirectory
/// files such as `.claude-plugin/plugin.json` are supplied through
/// `read_entry`). `read_entry` lazily returns the bytes of a named file, used
/// to inspect `plugin.json` and `.claude-plugin/plugin.json`; call it with
/// `|_| None` for a purely name-based check that never reads manifest bodies.
///
/// Returns `Ok(None)` when no dialect is recognised, `Ok(Some(_))` for exactly
/// one dialect, and [`PluginError::AmbiguousManifest`] when both dialects match.
///
/// # Errors
///
/// Returns [`PluginError::AmbiguousManifest`] when the directory matches both
/// the Codex and Claude recognition rules.
pub fn recognise_dialect(
    entries: &[String],
    read_entry: impl Fn(&str) -> Option<Vec<u8>>,
) -> Result<Option<DialectMatch>, PluginError> {
    let codex = codex_match(entries, &read_entry);
    let claude = claude_match(entries, &read_entry);

    match (codex, claude) {
        (Some(_), Some(_)) => Err(PluginError::AmbiguousManifest),
        (Some(m), None) | (None, Some(m)) => Ok(Some(m)),
        (None, None) => Ok(None),
    }
}

/// Detect the dialect of the plugin directory at `root` by reading its
/// directory listing and (when needed) manifest bodies from disk.
///
/// Returns `Ok(None)` when `root` is not a recognisable plugin directory.
/// Individual unreadable manifest files are treated as absent (recognition
/// continues over the remaining names).
///
/// # Errors
///
/// Returns [`PluginError::AmbiguousManifest`] when the directory matches both
/// dialects, or [`crate::error::PluginError::Io`] when the directory itself
/// cannot be listed.
pub fn detect_dialect(root: &Path) -> Result<Option<DialectMatch>, PluginError> {
    let read_dir = std::fs::read_dir(root).map_err(PluginError::io)?;
    let mut entries = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(PluginError::io)?;
        entries.push(entry.file_name().to_string_lossy().into_owned());
    }
    entries.sort_unstable();

    recognise_dialect(&entries, |name| std::fs::read(root.join(name)).ok())
}

/// Codex recognition: `codex-plugin.json`, or a `plugin.json` containing a
/// top-level `"codex"` marker field (FR-002).
fn codex_match(
    entries: &[String],
    read_entry: &impl Fn(&str) -> Option<Vec<u8>>,
) -> Option<DialectMatch> {
    if entries.iter().any(|e| e == CODEX_MANIFEST_FILE) {
        return Some(DialectMatch {
            dialect: PluginDialect::Codex,
            manifest_rel: PathBuf::from(CODEX_MANIFEST_FILE),
        });
    }
    if entries.iter().any(|e| e == GENERIC_MANIFEST_FILE)
        && let Some(bytes) = read_entry(GENERIC_MANIFEST_FILE)
        && let Ok(body) = serde_json::from_slice::<serde_json::Value>(&bytes)
        && body.get(CODEX_MARKER_FIELD).is_some()
    {
        return Some(DialectMatch {
            dialect: PluginDialect::Codex,
            manifest_rel: PathBuf::from(GENERIC_MANIFEST_FILE),
        });
    }
    None
}

/// Claude recognition: `claude-plugin.json`, or `.claude-plugin/plugin.json`
/// (FR-002).
fn claude_match(
    entries: &[String],
    read_entry: &impl Fn(&str) -> Option<Vec<u8>>,
) -> Option<DialectMatch> {
    if entries.iter().any(|e| e == CLAUDE_MANIFEST_FILE) {
        return Some(DialectMatch {
            dialect: PluginDialect::Claude,
            manifest_rel: PathBuf::from(CLAUDE_MANIFEST_FILE),
        });
    }
    if entries.iter().any(|e| e == ".claude-plugin") && read_entry(CLAUDE_NESTED_MANIFEST).is_some()
    {
        return Some(DialectMatch {
            dialect: PluginDialect::Claude,
            manifest_rel: PathBuf::from(CLAUDE_NESTED_MANIFEST),
        });
    }
    None
}

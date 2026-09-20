//! Manifest parsing for both plugin dialects, plus the host-API version check
//! (spec `plugins` T-003; FR-002, FR-019, FR-025).
//!
//! Each parser takes manifest bytes and the owning directories, tolerates
//! unknown fields (serde ignores them), extracts declared tools/commands when
//! present, and records sections the host API cannot satisfy as
//! [`PluginDescriptor::unsupported_capabilities`] (FR-025).
//!
//! All parsing functions are pure over their byte input: the only I/O is
//! confined to [`parse_plugin_dir`], the convenience wrapper that reads the
//! manifest from disk.
//!
//! Unsupported-capability labels are stable strings consumed by
//! `/plugins list` / `/plugins test`; add new labels as new dialect sections
//! are recognised, never reword existing ones.

use std::path::Path;

use serde::Deserialize;

use crate::descriptor::{DialectMatch, PluginDescriptor, PluginDialect, detect_dialect};
use crate::error::PluginError;

/// Unsupported-capability label for MCP-server transport sections in either
/// dialect (FR-025).
pub const UNSUP_MCP: &str = "mcp server transports";

/// Unsupported-capability label for the Claude Desktop `mounts` section.
pub const UNSUP_DESKTOP_MOUNTS: &str = "claude desktop mounts";

/// Unsupported-capability label for the Claude Desktop `window` section.
pub const UNSUP_DESKTOP_WINDOW: &str = "claude desktop window";

/// Unsupported-capability label for Codex `permissions.fs` filesystem grants.
pub const UNSUP_FS: &str = "codex permissions.fs";

/// Unsupported-capability label for Codex `permissions.exec` subprocess grants.
pub const UNSUP_EXEC: &str = "codex permissions.exec";

/// The host-API version this ragent build presents to plugin code (FR-019).
pub const HOST_API_VERSION: u32 = 1;

/// Host-API version mismatch between a plugin manifest and this ragent build
/// (FR-019): the plugin declares an API version newer than the host.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("plugin requires host API v{declared}, this ragent build provides v{host}")]
pub struct VersionMismatch {
    /// Version declared by the plugin manifest.
    pub declared: u32,
    /// Version provided by the host.
    pub host: u32,
}

/// Refuse a plugin whose declared host-API version is newer than the host's
/// (FR-019). Equal or lower versions are accepted (the v1 API is
/// additive-only, so older declarations keep working).
///
/// # Errors
///
/// Returns [`VersionMismatch`] when `declared > host`.
pub fn check_api_version(declared: u32, host: u32) -> Result<(), VersionMismatch> {
    if declared > host {
        Err(VersionMismatch { declared, host })
    } else {
        Ok(())
    }
}

/// A tool contributed by a plugin, normalised from the manifest declaration
/// or provided by `register_tool` at load time (FR-005).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginToolDecl {
    /// Tool name unique within the plugin; registered as
    /// `plugin_<pluginid>_<name>`.
    pub name: String,
    /// Human-readable description shown to the model.
    pub description: String,
    /// JSON schema for the tool's `parameters` object.
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// A slash command contributed by a plugin (FR-004).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginCommandDecl {
    /// Command name (without the leading `/`); must not collide with an
    /// existing slash command (FR-024).
    pub name: String,
    /// Human-readable description shown in autocomplete.
    pub description: String,
    /// Usage hint shown in help output; optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,
}

/// A single normalised permission grant requested by a manifest. Kept as a
/// plain string: the permission subsystem matches the text verbatim against
/// its rule engine (unknown grants fall through to `ask`, matching how plugin
/// tools behave by default per A5).
pub type PermissionRequest = String;

/// Raw Codex-dialect manifest shape. Unknown fields are ignored; dialect
/// recognition has already consumed the `"codex"` marker by the time these
/// bytes reach this parser.
#[derive(Debug, Deserialize)]
struct CodexManifest {
    name: String,
    version: String,
    /// Entry point relative to the plugin root.
    entry: String,
    /// Explicit plugin id; derived from `name` when absent.
    id: Option<String>,
    /// Declared host-API version; absent means v1.
    api_version: Option<u32>,
    #[serde(default)]
    tools: Vec<PluginToolDecl>,
    #[serde(default)]
    commands: Vec<PluginCommandDecl>,
    permissions: Option<CodexPermissions>,
    /// MCP-server transport sections: recorded as unsupported (FR-025).
    #[serde(rename = "mcp_servers", alias = "mcpServers")]
    mcp_servers: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct CodexPermissions {
    /// Network permission list; `"outbound"` maps to `network.outbound`.
    network: Option<Vec<String>>,
    /// Filesystem grants: no ragent host-API equivalent (FR-025).
    fs: Option<serde_json::Value>,
    /// Subprocess grants: no ragent host-API equivalent (FR-025).
    exec: Option<serde_json::Value>,
}

/// Raw Claude-dialect manifest shape (`claude-plugin.json` or
/// `.claude-plugin/plugin.json`).
#[derive(Debug, Deserialize)]
struct ClaudeManifest {
    name: String,
    version: Option<String>,
    /// Entry point under `entry`, `main`, or Claude Desktop's `server.entry`.
    entry: Option<String>,
    main: Option<String>,
    server: Option<ClaudeServer>,
    id: Option<String>,
    api_version: Option<u32>,
    #[serde(default)]
    tools: Vec<PluginToolDecl>,
    #[serde(default)]
    commands: Vec<PluginCommandDecl>,
    permissions: Option<Vec<PermissionRequest>>,
    /// Explicit capabilities list; each entry is validated against the v1
    /// host-API capability set and unknown entries recorded (FR-025).
    capabilities: Option<Vec<String>>,
    /// MCP-server transport sections: no ragent equivalent (FR-025).
    #[serde(rename = "mcp_servers", alias = "mcpServers")]
    mcp_servers: Option<serde_json::Value>,
    /// Claude Desktop mount-point section (unsupported).
    mounts: Option<serde_json::Value>,
    /// Claude Desktop window section (unsupported).
    window: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ClaudeServer {
    entry: Option<String>,
}

/// Capability names satisfiable by host API v1 (FR-004). Manifests declaring
/// entries outside this set have those entries recorded as unsupported
/// (FR-025). The set only grows, and entries are never retyped (A4).
///
/// `host_api` keys its permission gate on exactly these names (FR-020).
pub const V1_CAPABILITIES: &[&str] = &[
    "tools",
    "commands",
    "config",
    "message",
    "log",
    "plugin.read_text_file",
];

/// The outcome of parsing a manifest: the descriptor plus the contributions
/// carried inside the manifest body (kept off the descriptor, which is a
/// stable identity/identity-path model shared with discovery; T-010/T-012
/// consume these declarations).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedManifest {
    /// The normalised descriptor (FR-002).
    pub descriptor: PluginDescriptor,
    /// Tools declared in the manifest body.
    pub tools: Vec<PluginToolDecl>,
    /// Slash commands declared in the manifest body.
    pub commands: Vec<PluginCommandDecl>,
}

/// A plugin id is lowercase alphanumerics plus `-`/`_`, derived from the
/// display name when the manifest omits an explicit `id`.
#[must_use]
pub fn derive_id(name: &str) -> String {
    let collapsed: Vec<&str> = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect();
    let id = collapsed.join("-").to_ascii_lowercase();
    if id.is_empty() {
        "unnamed-plugin".to_string()
    } else {
        id
    }
}

/// Parse a Codex-dialect manifest into a [`ParsedManifest`] (FR-002).
///
/// `root` is the plugin root directory (anchors the entry path); `manifest_rel`
/// is the manifest path relative to `root`.
///
/// # Errors
///
/// Returns [`PluginError::ManifestParse`] when the bytes are not valid JSON or
/// violate the Codex manifest shape, including the JSON error position.
pub fn parse_codex_manifest(
    root: &Path,
    manifest_rel: &Path,
    bytes: &[u8],
) -> Result<ParsedManifest, PluginError> {
    let manifest_path = root.join(manifest_rel);
    let raw: CodexManifest =
        serde_json::from_slice(bytes).map_err(|e| PluginError::ManifestParse {
            manifest: manifest_path.clone(),
            detail: e.to_string(),
        })?;
    if raw.entry.trim().is_empty() {
        return Err(PluginError::ManifestParse {
            manifest: manifest_path,
            detail: "manifest entry point is empty".to_string(),
        });
    }

    let mut requested_permissions = Vec::new();
    let mut unsupported = Vec::new();
    if let Some(permissions) = raw.permissions {
        if let Some(network) = permissions.network {
            requested_permissions
                .extend(network.into_iter().map(|grant| format!("network.{grant}")));
        }
        if permissions.fs.is_some() {
            unsupported.push(UNSUP_FS.to_string());
        }
        if permissions.exec.is_some() {
            unsupported.push(UNSUP_EXEC.to_string());
        }
    }
    if raw.mcp_servers.is_some() {
        unsupported.push(UNSUP_MCP.to_string());
    }

    Ok(ParsedManifest {
        descriptor: PluginDescriptor {
            id: raw
                .id
                .filter(|id| !id.trim().is_empty())
                .unwrap_or_else(|| derive_id(&raw.name)),
            name: raw.name,
            version: raw.version,
            dialect: PluginDialect::Codex,
            entry: root.join(&raw.entry),
            requested_permissions,
            api_version: raw.api_version.unwrap_or(HOST_API_VERSION),
            unsupported_capabilities: unsupported,
            manifest_path,
            root: root.to_path_buf(),
        },
        tools: raw.tools,
        commands: raw.commands,
    })
}

/// Parse a Claude-dialect manifest into a [`ParsedManifest`] (FR-002).
///
/// Entry resolution follows the documented Claude shapes: top-level `entry`,
/// then `main`, then Desktop's `server.entry`. MCP-server transport sections
/// and Desktop-only sections are recorded through [`UNSUP_MCP`] /
/// [`UNSUP_DESKTOP_MOUNTS`] / [`UNSUP_DESKTOP_WINDOW`] (FR-025).
///
/// # Errors
///
/// Returns [`PluginError::ManifestParse`] on invalid JSON or a missing/empty
/// entry point.
pub fn parse_claude_manifest(
    root: &Path,
    manifest_rel: &Path,
    bytes: &[u8],
) -> Result<ParsedManifest, PluginError> {
    let manifest_path = root.join(manifest_rel);
    let raw: ClaudeManifest =
        serde_json::from_slice(bytes).map_err(|e| PluginError::ManifestParse {
            manifest: manifest_path.clone(),
            detail: e.to_string(),
        })?;

    let entry = raw
        .entry
        .or(raw.main)
        .or_else(|| raw.server.and_then(|s| s.entry))
        .ok_or_else(|| PluginError::ManifestParse {
            manifest: manifest_path.clone(),
            detail: "manifest has no entry point (expected entry, main, or server.entry)".into(),
        })?;
    if entry.trim().is_empty() {
        return Err(PluginError::ManifestParse {
            manifest: manifest_path,
            detail: "manifest entry point is empty".to_string(),
        });
    }

    let mut unsupported = Vec::new();
    if raw.mcp_servers.is_some() {
        unsupported.push(UNSUP_MCP.to_string());
    }
    if raw.mounts.is_some() {
        unsupported.push(UNSUP_DESKTOP_MOUNTS.to_string());
    }
    if raw.window.is_some() {
        unsupported.push(UNSUP_DESKTOP_WINDOW.to_string());
    }
    if let Some(capabilities) = raw.capabilities {
        unsupported.extend(
            capabilities
                .into_iter()
                .filter(|c| !V1_CAPABILITIES.contains(&c.as_str())),
        );
    }

    Ok(ParsedManifest {
        descriptor: PluginDescriptor {
            id: raw
                .id
                .filter(|id| !id.trim().is_empty())
                .unwrap_or_else(|| derive_id(&raw.name)),
            name: raw.name,
            version: raw.version.unwrap_or_else(|| "0.0.0".to_string()),
            dialect: PluginDialect::Claude,
            entry: root.join(entry),
            requested_permissions: raw.permissions.unwrap_or_default(),
            api_version: raw.api_version.unwrap_or(HOST_API_VERSION),
            unsupported_capabilities: unsupported,
            manifest_path,
            root: root.to_path_buf(),
        },
        tools: raw.tools,
        commands: raw.commands,
    })
}

/// Parse the manifest named by a [`DialectMatch`], dispatching on dialect
/// (FR-002).
///
/// # Errors
///
/// See [`parse_codex_manifest`] / [`parse_claude_manifest`].
pub fn parse_manifest(
    root: &Path,
    matched: &DialectMatch,
    bytes: &[u8],
) -> Result<ParsedManifest, PluginError> {
    match matched.dialect {
        PluginDialect::Codex => parse_codex_manifest(root, &matched.manifest_rel, bytes),
        PluginDialect::Claude => parse_claude_manifest(root, &matched.manifest_rel, bytes),
    }
}

/// Recognise and parse the plugin rooted at `root` (FR-002).
///
/// Reads the manifest from disk; performs no other I/O and executes no plugin
/// code (FR-023).
///
/// # Errors
///
/// Returns [`PluginError::AmbiguousManifest`] for a directory matching both
/// dialects, [`PluginError::ManifestParse`] when the manifest bytes are
/// unreadable or invalid, and [`PluginError::Io`] when the directory cannot be
/// listed.
pub fn parse_plugin_dir(root: &Path) -> Result<Option<ParsedManifest>, PluginError> {
    let Some(matched) = detect_dialect(root)? else {
        return Ok(None);
    };
    let manifest_path = root.join(&matched.manifest_rel);
    let bytes = std::fs::read(&manifest_path).map_err(PluginError::io)?;
    parse_manifest(root, &matched, &bytes).map(Some)
}

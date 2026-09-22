//! Manifest parsing for both plugin dialects, plus the host-API version check
//! (spec `plugins` T-003; FR-002, FR-019, FR-025).
//!
//! Each parser takes manifest bytes and the owning directories, tolerates
//! unknown fields (serde ignores them), extracts declared tools/commands when
//! present, and records sections the host API cannot satisfy as
//! [`PluginDescriptor::unsupported_capabilities`] (FR-025).
//!
//! A manifest that declares no JavaScript entry point (`entry`, `main`, or
//! `server.entry`) is a non-JS plugin — a skill-only or MCP-only package, the
//! shape the official Claude marketplace ships. Such a plugin parses with
//! `entry = None`, installs, lists, and loads inertly.
//!
//! The `skills` and `mcpServers` sections are **bridged**: this parser extracts
//! them into [`ParsedManifest::skills`] and [`ParsedManifest::mcp_servers`], and
//! the session layer consumes them (skill directories are appended to the
//! skill-discovery roots; MCP server entries are mapped to `McpServerConfig`
//! and connected at startup). A section that is present but whose shape cannot
//! be bridged is still recorded as unsupported (FR-025) rather than dropped.
//!
//! Hooks are declared either inline in the manifest `hooks` section or in a
//! `hooks.json` file at the plugin root or under `hooks/` (the Claude layout,
//! where the file wraps its triggers in a top-level `hooks` object); both
//! sources are merged into [`ParsedManifest::hooks`].
//!
//! All parsing functions are pure over their byte input: the only I/O is
//! confined to [`parse_plugin_dir`], the convenience wrapper that reads the
//! manifest from disk.
//!
//! Unsupported-capability labels are stable strings consumed by
//! `/plugins list` / `/plugins test`; add new labels as new dialect sections
//! are recognised, never reword existing ones.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::descriptor::{DialectMatch, PluginDescriptor, PluginDialect, detect_dialect};
use crate::error::PluginError;

/// Unsupported-capability label for MCP-server transport sections in either
/// dialect whose shape has no bridged equivalent (FR-025).
pub const UNSUP_MCP: &str = "mcp server transports";

/// Unsupported-capability label for a `skills` section whose shape has no
/// bridged equivalent (FR-025).
pub const UNSUP_SKILLS: &str = "plugin skills";

/// Unsupported-capability label for the Claude Desktop `mounts` section.
pub const UNSUP_DESKTOP_MOUNTS: &str = "claude desktop mounts";

/// Unsupported-capability label for the Claude Desktop `window` section.
pub const UNSUP_DESKTOP_WINDOW: &str = "claude desktop window";

/// Unsupported-capability label for Codex `permissions.fs` filesystem grants.
pub const UNSUP_FS: &str = "codex permissions.fs";

/// Unsupported-capability label for Codex `permissions.exec` subprocess grants.
pub const UNSUP_EXEC: &str = "codex permissions.exec";

/// Unsupported-capability label for an `agents` section whose shape has no
/// bridged equivalent (FR-025).
pub const UNSUP_AGENTS: &str = "plugin agents";

/// Unsupported-capability label for a `hooks` section whose shape has no
/// bridged equivalent (FR-025).
pub const UNSUP_HOOKS: &str = "plugin hooks";

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

/// Where a plugin command's behaviour comes from (FR-004, FR-031).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandSource {
    /// The command has a JavaScript handler inside the plugin's entry sandbox
    /// (declared in the manifest `commands[]` array or via
    /// `ragent.register_command`); dispatch runs the handler and surfaces its
    /// return value.
    Inline,
    /// The command is a prompt template read from a plugin-relative markdown
    /// file (the Claude `commands/<name>.md` shape); dispatch injects the file
    /// body, after argument substitution, into the session as a prompt.
    File {
        /// Path of the markdown file (absolute, anchored at the plugin root).
        path: PathBuf,
        /// Raw markdown body (everything after the YAML frontmatter).
        body: String,
    },
}

/// A slash command contributed by a plugin, in one of two dispatchable forms
/// (FR-004, FR-031).
///
/// An inline command ([`Self::source`] is [`CommandSource::Inline`]) dispatches
/// into the plugin's JavaScript sandbox; a prompt command (source is
/// [`CommandSource::File`]) carries the rendered prompt body the session
/// injects when the command is invoked.
#[derive(Clone, PartialEq, Eq)]
pub struct PluginCommandDef {
    /// Declaration metadata (name, description, usage).
    pub decl: PluginCommandDecl,
    /// Where the command's behaviour comes from.
    pub source: CommandSource,
}

impl std::fmt::Debug for PluginCommandDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Redact the prompt body: a command template can carry large content and
        // is not useful in logs.
        let source = match &self.source {
            CommandSource::Inline => "Inline",
            CommandSource::File { .. } => "File{..}",
        };
        f.debug_struct("PluginCommandDef")
            .field("decl", &self.decl)
            .field("source", &source)
            .finish()
    }
}

impl PluginCommandDef {
    /// An inline command backed by a JavaScript handler.
    #[must_use]
    pub fn inline(decl: PluginCommandDecl) -> Self {
        Self {
            decl,
            source: CommandSource::Inline,
        }
    }

    /// A prompt command read from a plugin markdown file.
    #[must_use]
    pub fn from_file(decl: PluginCommandDecl, path: PathBuf, body: String) -> Self {
        Self {
            decl,
            source: CommandSource::File { path, body },
        }
    }

    /// The prompt body when this is a prompt command, else `None`.
    #[must_use]
    pub fn prompt(&self) -> Option<&str> {
        match &self.source {
            CommandSource::File { body, .. } => Some(body),
            CommandSource::Inline => None,
        }
    }
}

/// Plugin `commands/` directory name (Claude-dialect prompt commands).
pub const COMMANDS_DIR: &str = "commands";

/// Plugin `agents/` directory name (Claude / Codex subagent profiles).
pub const AGENTS_DIR: &str = "agents";

/// Plugin `hooks/` directory name (Claude-dialect hook scripts).
pub const HOOKS_DIR: &str = "hooks";

/// Hook-declaration file name, checked at the plugin root and inside
/// [`HOOKS_DIR`] (the Claude `hooks/hooks.json` layout).
pub const HOOKS_FILE: &str = "hooks.json";

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
    /// Entry point relative to the plugin root. `None` for a non-JS
    /// (skill-only / MCP-only) plugin, which still installs, lists, and loads
    /// inertly.
    #[serde(default)]
    entry: Option<String>,
    id: Option<String>,
    /// Declared host-API version; absent means v1.
    api_version: Option<u32>,
    #[serde(default)]
    tools: Vec<PluginToolDecl>,
    /// Declared slash commands. Tolerant of the shapes both dialects use:
    /// an array of `{name, description, usage?}` objects, or a directory list
    /// in the Claude `commands/` style (see [`extract_command_decls`]).
    commands: Option<serde_json::Value>,
    permissions: Option<CodexPermissions>,
    /// Plugin skill directory declarations (string or list of strings);
    /// bridged to the skill-discovery roots.
    skills: Option<serde_json::Value>,
    /// MCP-server transport sections; bridged to `McpServerConfig` when the
    /// shape is recognised, otherwise recorded as unsupported (FR-025).
    #[serde(rename = "mcp_servers", alias = "mcpServers")]
    mcp_servers: Option<serde_json::Value>,
    /// Subagent profile declarations (string or list of strings naming
    /// `agents/*.md` profiles); bridged to the agent-discovery roots.
    agents: Option<serde_json::Value>,
    /// Hook declarations; bridged to the session hook engine when the shape is
    /// recognised, otherwise recorded as unsupported (FR-025).
    hooks: Option<serde_json::Value>,
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
    /// Absent (all three) means a non-JS plugin with `entry = None`.
    entry: Option<String>,
    main: Option<String>,
    server: Option<ClaudeServer>,
    id: Option<String>,
    api_version: Option<u32>,
    #[serde(default)]
    tools: Vec<PluginToolDecl>,
    /// Declared slash commands. Tolerant of the shapes both dialects use:
    /// an array of `{name, description, usage?}` objects, or a directory list
    /// in the Claude `commands/` style (see [`extract_command_decls`]).
    commands: Option<serde_json::Value>,
    permissions: Option<Vec<PermissionRequest>>,
    /// Explicit capabilities list; each entry is validated against the v1
    /// host-API capability set and unknown entries recorded (FR-025).
    capabilities: Option<Vec<String>>,
    /// Plugin skill directory declarations (string or list of strings);
    /// bridged to the skill-discovery roots.
    skills: Option<serde_json::Value>,
    /// MCP-server transport sections; bridged to `McpServerConfig` when the
    /// shape is recognised, otherwise recorded as unsupported (FR-025).
    #[serde(rename = "mcp_servers", alias = "mcpServers")]
    mcp_servers: Option<serde_json::Value>,
    /// Subagent profile declarations (string or list of strings naming
    /// `agents/*.md` profiles); bridged to the agent-discovery roots.
    agents: Option<serde_json::Value>,
    /// Hook declarations; bridged to the session hook engine when the shape is
    /// recognised, otherwise recorded as unsupported (FR-025).
    hooks: Option<serde_json::Value>,
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

/// One MCP server contributed by a plugin, in the shape shared by the Codex
/// `mcp_servers`/`mcpServers` and Claude `mcpServers` sections. The session
/// layer maps this to a `McpServerConfig` and connects it at startup (the
/// plugin MCP bridge, FR-030).
///
/// Entry values may be either the server object itself or a string naming an
/// external MCP-config file resolved relative to the plugin root (the Claude
/// marketplace `"mcpServers": "./mcp.json"` shape); see
/// [`extract_mcp_servers`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginMcpServer {
    /// Server id (key under `mcpServers`). Prefixed with the plugin id by the
    /// bridge so two plugins cannot collide on a server name.
    pub id: String,
    /// Executable to launch for a stdio transport.
    pub command: Option<String>,
    /// Arguments passed to `command`.
    pub args: Vec<String>,
    /// Environment variables injected into the server process.
    pub env: std::collections::BTreeMap<String, String>,
    /// Endpoint URL for `sse`/`http` transports.
    pub url: Option<String>,
    /// Extra HTTP headers for `sse`/`http` transports.
    pub headers: std::collections::BTreeMap<String, String>,
    /// Transport name declared by the entry (`stdio` default, `sse`, `http`).
    pub transport: String,
}

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
    /// Slash commands declared in the manifest body (inline handlers) plus any
    /// prompt commands discovered in the plugin's `commands/` directory.
    pub commands: Vec<PluginCommandDef>,
    /// Skill directory declarations from the `skills` section, verbatim
    /// (relative paths are resolved against the plugin root by the bridge).
    /// Empty when the manifest declares no skills.
    pub skills: Vec<String>,
    /// MCP servers bridged from the `mcp_servers`/`mcpServers` section. An
    /// entry whose value names an external file is resolved by the bridge.
    pub mcp_servers: Vec<PluginMcpServer>,
    /// Raw `mcpServers` value, kept so the bridge can follow a string source to
    /// an external `mcp.json` file (the Claude marketplace shape).
    pub raw_mcp: Option<serde_json::Value>,
    /// Agent profile declarations from the `agents` section, verbatim (relative
    /// paths are resolved against the plugin root by the bridge). Empty when the
    /// manifest declares no agents.
    pub agents: Vec<String>,
    /// Hook declarations bridged from the `hooks` section, normalised into the
    /// session hook shape (trigger + command). Empty when none are declared.
    pub hooks: Vec<PluginHook>,
}

/// One hook contributed by a plugin (spec `plugins` FR-033), normalised into the
/// shape the session hook engine consumes: a trigger name and the shell command
/// to run at that trigger.
///
/// The trigger is the plugin-dialect event name exactly as declared (for example
/// Claude's `PreToolUse`, or ragent's own `pre_tool_use`); the session layer maps
/// it onto its own trigger type and drops the ones it does not recognise.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginHook {
    /// Plugin id that declared the hook (filled by the bridge).
    pub plugin_id: String,
    /// Absolute plugin root, exported as `CLAUDE_PLUGIN_ROOT` when the hook runs
    /// so a Claude-dialect command can resolve `${CLAUDE_PLUGIN_ROOT}/...`.
    pub plugin_root: PathBuf,
    /// Declared trigger name (e.g. `PreToolUse`, `post_tool_use`).
    pub trigger: String,
    /// Shell command to run at the trigger.
    pub command: String,
    /// Timeout in seconds; `None` uses the session hook default.
    pub timeout_secs: Option<u64>,
    /// Optional tool match expression (the Claude `matcher` group field or
    /// per-entry `if` field, e.g. `Edit|Write` or `Bash(git commit:*)`).
    /// `None` matches every tool. Only consulted for tool-scoped triggers.
    pub matcher: Option<String>,
}

impl PluginHook {
    /// Build a hook with the plugin id and root stamped on.
    #[must_use]
    pub fn new(
        plugin_id: &str,
        root: &Path,
        trigger: &str,
        command: String,
        timeout_secs: Option<u64>,
        matcher: Option<String>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            plugin_root: root.to_path_buf(),
            trigger: trigger.to_string(),
            command,
            timeout_secs,
            matcher,
        }
    }
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

/// Extract declared commands from the manifest `commands` field (FR-004,
/// FR-031).
///
/// Accepts an array of strings (each naming a prompt-command markdown file, its
/// stem becoming the command name) or an array of objects. An object without a
/// `file` field is an inline (JavaScript) command — the shape ragent has always
/// dispatched into `globalThis.__ragent_commands[name]`; an object carrying a
/// `file` is a prompt command whose prompt is read from that markdown file
/// (resolved relative to the plugin root, with `commands/` applied to a
/// relative name). Objects are tolerant of the `allowed-tools` and
/// `description` frontmatter fields the Claude marketplace shape carries.
#[must_use]
pub fn extract_command_decls(
    raw: Option<&serde_json::Value>,
    root: &Path,
) -> Vec<PluginCommandDef> {
    let Some(items) = raw.and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        if let Some(name) = item.as_str() {
            let name = name.trim();
            if !name.is_empty()
                && let Some(def) = prompt_command_for_file(root, name, name, None)
            {
                out.push(def);
            }
            continue;
        }
        let Some(obj) = item.as_object() else {
            continue;
        };
        let name = obj
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let file = obj
            .get("file")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        match file {
            Some(rel) => {
                if let Some(def) = prompt_command_for_file(root, rel, name, Some(obj)) {
                    out.push(def);
                }
            }
            None => out.push(PluginCommandDef::inline(PluginCommandDecl {
                name: name.to_string(),
                description: obj
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                usage: obj
                    .get("usage")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
            })),
        }
    }
    out
}

/// Discover prompt commands from a plugin's `commands/` directory (FR-031):
/// each `*.md` file is one command named after its file stem, with a
/// `description` read from its YAML frontmatter. Returns an empty vector when
/// the directory does not exist.
#[must_use]
pub fn scan_command_dir(root: &Path) -> Vec<PluginCommandDef> {
    let dir = root.join(COMMANDS_DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()).map(str::trim) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        if let Some(def) = prompt_command_from_path(&path, name, None) {
            out.push(def);
        }
    }
    out.sort_by(|a, b| a.decl.name.cmp(&b.decl.name));
    out
}

/// Build a prompt command for a command whose markdown file is named `rel`
/// (relative to `root`), resolving its metadata from `meta` when supplied.
fn prompt_command_for_file(
    root: &Path,
    rel: &str,
    name: &str,
    meta: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<PluginCommandDef> {
    let mut file = rel.to_string();
    if !std::path::Path::new(&file)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
    {
        file = format!("{file}.md");
    }
    let path = if std::path::Path::new(&file).is_absolute() {
        std::path::PathBuf::from(&file)
    } else {
        root.join(COMMANDS_DIR).join(&file)
    };
    prompt_command_from_path(&path, name, meta)
}

/// Read a prompt-command markdown file, returning the definition with the
/// frontmatter-stripped body. `None` when the file cannot be read.
fn prompt_command_from_path(
    path: &Path,
    name: &str,
    meta: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<PluginCommandDef> {
    let content = std::fs::read_to_string(path).ok()?;
    let (description, body) = parse_command_markdown(&content);
    let description = meta
        .and_then(|m| m.get("description"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or(description);
    let usage = meta
        .and_then(|m| m.get("usage"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    Some(PluginCommandDef::from_file(
        PluginCommandDecl {
            name: name.to_string(),
            description,
            usage,
        },
        path.to_path_buf(),
        body,
    ))
}

/// Split a command markdown file into `(description, body)`.
///
/// The file may open with a `---`-delimited YAML frontmatter block that carries
/// a `description`; everything after the block is the prompt body. A file with
/// no frontmatter is all body, with an empty description. Frontmatter fields
/// other than `description` (e.g. `allowed-tools`) are ignored.
fn parse_command_markdown(content: &str) -> (String, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (String::new(), content.to_string());
    }
    let after = &trimmed[3..];
    let after = after
        .strip_prefix('\n')
        .unwrap_or_else(|| after.strip_prefix("\r\n").unwrap_or(after));
    // Find the closing --- at the start of a line.
    let mut offset = 0usize;
    let mut closing = None;
    for line in after.lines() {
        if line.trim() == "---" {
            closing = Some(offset);
            break;
        }
        offset += line.len() + 1;
    }
    let Some(closing) = closing else {
        return (String::new(), content.to_string());
    };
    let frontmatter = &after[..closing];
    let rest = &after[closing + 3..];
    let body = rest
        .strip_prefix('\n')
        .unwrap_or_else(|| rest.strip_prefix("\r\n").unwrap_or(rest));
    let description = frontmatter
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("description:")
                .map(|v| v.trim().trim_matches('"').trim_matches('\'').to_string())
        })
        .unwrap_or_default();
    (description, body.to_string())
}

/// Extract the `skills` section into the list of declared directory strings
/// (FR-029 skills bridge).
///
/// Accepts a single string or an array of strings; any other shape contributes
/// nothing and is reported by the caller as unsupported. Directory existence is
/// not checked here — the bridge resolves and validates paths against the
/// plugin root.
#[must_use]
pub fn extract_skill_dirs(raw: Option<&serde_json::Value>) -> Vec<String> {
    match raw {
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// Extract the `agents` section into the list of declared agent-profile paths
/// (spec `plugins` FR-032 agents bridge).
///
/// Accepts a single string or an array of strings, each naming an agent profile
/// relative to the plugin root (a bare name gets `agents/` and `.md` applied by
/// the bridge, mirroring the Claude `agents/` directory shape). Any other shape
/// contributes nothing and is reported by the caller as unsupported. Existence
/// is not checked here.
#[must_use]
pub fn extract_agent_decls(raw: Option<&serde_json::Value>) -> Vec<String> {
    match raw {
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// Scan a plugin's `agents/` directory for agent-profile files (FR-032).
///
/// Returns the plugin-relative path of every `.md` (and `.json`) file found,
/// sorted for a stable order. Returns an empty vector when the directory does
/// not exist. Only one directory level is scanned; nested directories are left
/// to the discovery walker that the bridge copies the files into.
#[must_use]
pub fn scan_agent_dir(root: &Path) -> Vec<String> {
    scan_profile_dir(root, AGENTS_DIR)
}

/// Map a plugin's `hooks` declarations into [`PluginHook`]s (FR-033 hooks
/// bridge).
///
/// Accepts the shapes both dialects use:
///
/// - an object keyed by trigger name, each value a command string
///   (`{"PreToolUse": "./check.sh"}`);
/// - an object keyed by trigger name, each value an array of entries, each
///   entry either a string command, a flat entry object `{command|command_path,
///   timeout_secs?}`, or a **group** `{matcher?, hooks: [entry...]}` (the
///   official Claude `hooks/hooks.json` shape);
/// - a flat array of `{trigger, command, timeout_secs?}` objects (ragent's own
///   `ragent.json` hook shape).
///
/// `plugin_id` is stamped onto every returned hook and `root` is recorded as the
/// hook's plugin root (so the session layer can export `CLAUDE_PLUGIN_ROOT`). A
/// trigger with a value of any other shape contributes no hook, so the caller
/// records the section as unsupported when the result is empty (FR-025).
#[must_use]
pub fn extract_hooks(
    plugin_id: &str,
    root: &Path,
    raw: Option<&serde_json::Value>,
) -> Vec<PluginHook> {
    let Some(value) = raw else {
        return Vec::new();
    };
    let mut out = Vec::new();
    match value {
        serde_json::Value::Object(map) => {
            for (trigger, entries) in map {
                collect_hook_entries(plugin_id, root, trigger, entries, &mut out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                let Some(obj) = item.as_object() else {
                    continue;
                };
                let trigger = obj
                    .get("trigger")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                if trigger.is_empty() {
                    continue;
                }
                if let Some(command) = hook_command(obj) {
                    out.push(PluginHook::new(
                        plugin_id,
                        root,
                        trigger,
                        command,
                        hook_timeout(obj),
                        None,
                    ));
                }
            }
        }
        _ => {}
    }
    out
}

/// Append every hook entry under one trigger key (object or array value shape).
fn collect_hook_entries(
    plugin_id: &str,
    root: &Path,
    trigger: &str,
    entries: &serde_json::Value,
    out: &mut Vec<PluginHook>,
) {
    match entries {
        serde_json::Value::String(command) => out.push(PluginHook::new(
            plugin_id,
            root,
            trigger,
            command.clone(),
            None,
            None,
        )),
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(command) = item.as_str() {
                    out.push(PluginHook::new(
                        plugin_id,
                        root,
                        trigger,
                        command.to_string(),
                        None,
                        None,
                    ));
                } else if let Some(obj) = item.as_object() {
                    collect_hook_object(plugin_id, root, trigger, obj, None, out);
                }
            }
        }
        _ => {}
    }
}

/// Append one entry object under `trigger`, unwrapping the Claude group shape
/// (`{matcher, hooks: [...]}`) when present.
///
/// A flat entry (`{command, timeout?}`) contributes itself; a group contributes
/// each of its child entries, stamped with the group's `matcher` (inherited
/// through `inherited_matcher`). An entry whose `type` is present and not
/// `"command"` is skipped (ragent only runs commands).
fn collect_hook_object(
    plugin_id: &str,
    root: &Path,
    trigger: &str,
    obj: &serde_json::Map<String, serde_json::Value>,
    inherited_matcher: Option<String>,
    out: &mut Vec<PluginHook>,
) {
    if let Some(children) = obj.get("hooks").and_then(serde_json::Value::as_array) {
        let matcher = hook_matcher(obj).or(inherited_matcher);
        for child in children {
            if let Some(child_obj) = child.as_object() {
                collect_hook_object(plugin_id, root, trigger, child_obj, matcher.clone(), out);
            } else if let Some(command) = child.as_str() {
                out.push(PluginHook::new(
                    plugin_id,
                    root,
                    trigger,
                    command.to_string(),
                    None,
                    matcher.clone(),
                ));
            }
        }
        return;
    }
    if let Some(kind) = obj.get("type").and_then(serde_json::Value::as_str)
        && kind != "command"
    {
        return;
    }
    if let Some(command) = hook_command(obj) {
        out.push(PluginHook::new(
            plugin_id,
            root,
            trigger,
            command,
            hook_timeout(obj),
            hook_matcher(obj).or(inherited_matcher),
        ));
    }
}

/// Read the hooks a plugin declares in a `hooks.json` file, checking the plugin
/// root and the `hooks/` subdirectory (the Claude layout). Returns the top-level
/// `hooks` object when the file carries that envelope, else the whole document.
///
/// Returns an empty vector when neither location holds a readable file. A file
/// that is not valid JSON contributes nothing (the manifest parser reports the
/// plugin's own errors; a bad hook file must not fail the whole plugin).
#[must_use]
pub fn read_plugin_hooks_file(root: &Path) -> Vec<(String, serde_json::Value)> {
    for path in [root.join(HOOKS_FILE), root.join(HOOKS_DIR).join(HOOKS_FILE)] {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(serde_json::Value::Object(mut map)) =
            serde_json::from_slice::<serde_json::Value>(&bytes)
        else {
            continue;
        };
        let value = map
            .remove("hooks")
            .unwrap_or(serde_json::Value::Object(map));
        let mut out = Vec::new();
        if let serde_json::Value::Object(triggers) = value {
            for (trigger, entries) in triggers {
                out.push((trigger, entries));
            }
        }
        return out;
    }
    Vec::new()
}

/// A hook entry's match expression: `matcher` (the Claude group field), or `if`
/// (the Claude per-entry field). `None`/`*`/empty match every tool.
fn hook_matcher(obj: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    obj.get("matcher")
        .or_else(|| obj.get("if"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "*")
        .map(str::to_string)
}

/// A hook entry's command: `command`, or `command_path` (the Claude marketplace
/// field for a script under `hooks/`).
fn hook_command(obj: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    obj.get("command")
        .or_else(|| obj.get("command_path"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// A hook entry's optional timeout in seconds: `timeout_secs`, or the Claude
/// `timeout` field. A value in milliseconds is normalised to whole seconds.
fn hook_timeout(obj: &serde_json::Map<String, serde_json::Value>) -> Option<u64> {
    if let Some(secs) = obj.get("timeout_secs").and_then(serde_json::Value::as_u64) {
        return Some(secs);
    }
    obj.get("timeout")
        .and_then(serde_json::Value::as_u64)
        .map(|value| if value >= 1000 { value / 1000 } else { value })
}

/// List the `.md`/`.json` files directly under `root/<dir>`, as plugin-relative
/// paths, sorted. Empty when the directory is absent.
fn scan_profile_dir(root: &Path, dir: &str) -> Vec<String> {
    let path = root.join(dir);
    let Ok(entries) = std::fs::read_dir(&path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let file = entry.path();
        if !file.is_file() {
            continue;
        }
        let ext = file.extension().and_then(|e| e.to_str());
        if !matches!(ext, Some("md" | "json")) {
            continue;
        }
        if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
            out.push(format!("{dir}/{name}"));
        }
    }
    out.sort();
    out
}

/// Map one `mcpServers` entry value to a [`PluginMcpServer`] (FR-030 MCP
/// bridge).
///
/// `key` is the server id. The entry must be a JSON object (the common case:
/// `{"command": ..., "args": [...], "env": {...}, "url": ..., "headers": ...}`
/// with an optional `type`/`transport`). Returns `None` for any other shape
/// (including a string), which the caller records as unsupported (FR-025). The
/// external-file shape is handled at the top level by the bridge, not per
/// entry.
#[must_use]
pub fn map_mcp_server_entry(key: &str, value: &serde_json::Value) -> Option<PluginMcpServer> {
    use std::collections::BTreeMap;

    fn string_map(v: Option<&serde_json::Value>) -> BTreeMap<String, String> {
        v.and_then(serde_json::Value::as_object)
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default()
    }

    let serde_json::Value::Object(obj) = value else {
        return None;
    };
    let transport = obj
        .get("type")
        .or_else(|| obj.get("transport"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("stdio")
        .to_string();
    Some(PluginMcpServer {
        id: key.to_string(),
        command: obj
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        args: obj
            .get("args")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        env: string_map(obj.get("env")),
        url: obj
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        headers: string_map(obj.get("headers")),
        transport,
    })
}

/// Extract the `mcpServers` section into bridged servers plus the count of
/// entries whose shape could not be bridged (FR-030 MCP bridge).
///
/// Only a JSON object of server entries yields bridged servers. A top-level
/// string (the external-file shape) yields no inline servers and no unbridged
/// count — the bridge resolves it through [`ParsedManifest::raw_mcp`]. Any
/// other top-level shape (array, number, bool) is counted as one unbridged
/// section so the caller keeps an FR-025 unsupported label, and an object entry
/// that is not a server object is counted individually.
#[must_use]
pub fn extract_mcp_servers(raw: Option<&serde_json::Value>) -> (Vec<PluginMcpServer>, usize) {
    match raw {
        Some(serde_json::Value::Object(map)) => {
            let mut servers = Vec::new();
            let mut unbridged = 0usize;
            for (key, value) in map {
                match map_mcp_server_entry(key, value) {
                    Some(server) => servers.push(server),
                    None => unbridged += 1,
                }
            }
            (servers, unbridged)
        }
        Some(serde_json::Value::String(_)) | None => (Vec::new(), 0),
        Some(_) => (Vec::new(), 1),
    }
}

/// Parse a Codex-dialect manifest into a [`ParsedManifest`] (FR-002).
///
/// `root` is the plugin root directory (anchors the entry path); `manifest_rel`
/// is the manifest path relative to `root`.
///
/// # Errors
///
/// Returns [`PluginError::ManifestParse`] when the bytes are not valid JSON,
/// violate the Codex manifest shape, or carry a declared `entry` field that is
/// present but blank. A manifest with no `entry` is accepted as a non-JS
/// (skill-only / MCP-only) plugin with `entry = None`.
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
    // A declared entry point is resolved now; a manifest that declares none is
    // a non-JS plugin (skill-only / MCP-only) and loads inertly.
    let entry = resolve_entry(&raw.entry, root, &manifest_path, "entry")?;

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
    // FR-029/FR-030 bridges: extract `skills` and `mcpServers`. An entry with no
    // bridged equivalent keeps the FR-025 unsupported label.
    let skills = extract_skill_dirs(raw.skills.as_ref());
    let (mcp_servers, unbridged_mcp) = extract_mcp_servers(raw.mcp_servers.as_ref());
    if unbridged_mcp > 0 {
        unsupported.push(UNSUP_MCP.to_string());
    }
    // FR-031: declared commands are inline or file-backed prompt commands.
    let commands = extract_command_decls(raw.commands.as_ref(), root);

    // FR-032: agent profiles — declared in the manifest and/or present under
    // `agents/`. Both sets are merged (manifest first) and de-duplicated.
    let mut agents = extract_agent_decls(raw.agents.as_ref());
    for rel in scan_agent_dir(root) {
        if !agents.contains(&rel) {
            agents.push(rel);
        }
    }
    if raw.agents.is_some() && agents.is_empty() {
        unsupported.push(UNSUP_AGENTS.to_string());
    }
    // FR-033: hooks are normalised to (trigger, command); an unrecognised
    // section shape leaves no hooks and keeps the unsupported label.
    let id = raw
        .id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
        .map_or_else(|| derive_id(&raw.name), str::to_string);
    let mut hooks = extract_hooks(&id, root, raw.hooks.as_ref());
    for (trigger, entries) in read_plugin_hooks_file(root) {
        collect_hook_entries(&id, root, &trigger, &entries, &mut hooks);
    }
    if raw.hooks.is_some() && hooks.is_empty() {
        unsupported.push(UNSUP_HOOKS.to_string());
    }

    Ok(ParsedManifest {
        descriptor: PluginDescriptor {
            id,
            name: raw.name,
            version: raw.version,
            dialect: PluginDialect::Codex,
            entry,
            requested_permissions,
            api_version: raw.api_version.unwrap_or(HOST_API_VERSION),
            unsupported_capabilities: unsupported,
            manifest_path,
            root: root.to_path_buf(),
        },
        tools: raw.tools,
        commands,
        skills,
        mcp_servers,
        raw_mcp: raw.mcp_servers,
        agents,
        hooks,
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
/// Returns [`PluginError::ManifestParse`] on invalid JSON, or when a declared
/// entry field (`entry`/`main`/`server.entry`) is present but blank. A manifest
/// declaring none of them is accepted as a non-JS (skill-only / MCP-only)
/// plugin with `entry = None`.
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

    // A declared entry point is resolved now; a manifest that declares none
    // (no `entry`, `main`, or `server.entry`) is a non-JS plugin
    // (skill-only / MCP-only) and loads inertly.
    let declared = raw
        .entry
        .or(raw.main)
        .or_else(|| raw.server.and_then(|s| s.entry));
    let entry = resolve_entry(
        &declared,
        root,
        &manifest_path,
        "entry, main, or server.entry",
    )?;

    let mut unsupported = Vec::new();
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

    // FR-029/FR-030 bridges: extract `skills` and `mcpServers`. A section whose shape
    // has no bridged equivalent keeps the FR-025 unsupported label.
    let skills = extract_skill_dirs(raw.skills.as_ref());
    if raw.skills.is_some() && skills.is_empty() {
        unsupported.push(UNSUP_SKILLS.to_string());
    }
    let (mcp_servers, unbridged_mcp) = extract_mcp_servers(raw.mcp_servers.as_ref());
    if unbridged_mcp > 0 {
        unsupported.push(UNSUP_MCP.to_string());
    }
    // FR-031: commands from the manifest `commands` section plus the Claude
    // `commands/` directory. Manifest declarations come first so they win any
    // name clash with a directory file.
    let mut commands = extract_command_decls(raw.commands.as_ref(), root);
    for def in scan_command_dir(root) {
        if !commands.iter().any(|c| c.decl.name == def.decl.name) {
            commands.push(def);
        }
    }

    // FR-032: agent profiles declared in the manifest and/or present under
    // `agents/` (manifest first, de-duplicated).
    let mut agents = extract_agent_decls(raw.agents.as_ref());
    for rel in scan_agent_dir(root) {
        if !agents.contains(&rel) {
            agents.push(rel);
        }
    }
    if raw.agents.is_some() && agents.is_empty() {
        unsupported.push(UNSUP_AGENTS.to_string());
    }
    // FR-033: hooks normalised to (trigger, command); a section whose shape
    // yields no hooks keeps the unsupported label.
    let id = raw
        .id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
        .map_or_else(|| derive_id(&raw.name), str::to_string);
    let mut hooks = extract_hooks(&id, root, raw.hooks.as_ref());
    for (trigger, entries) in read_plugin_hooks_file(root) {
        collect_hook_entries(&id, root, &trigger, &entries, &mut hooks);
    }
    if raw.hooks.is_some() && hooks.is_empty() {
        unsupported.push(UNSUP_HOOKS.to_string());
    }

    Ok(ParsedManifest {
        descriptor: PluginDescriptor {
            id,
            name: raw.name,
            version: raw.version.unwrap_or_else(|| "0.0.0".to_string()),
            dialect: PluginDialect::Claude,
            entry,
            requested_permissions: raw.permissions.unwrap_or_default(),
            api_version: raw.api_version.unwrap_or(HOST_API_VERSION),
            unsupported_capabilities: unsupported,
            manifest_path,
            root: root.to_path_buf(),
        },
        tools: raw.tools,
        commands,
        skills,
        mcp_servers,
        raw_mcp: raw.mcp_servers,
        agents,
        hooks,
    })
}

/// Resolve an optional declared entry point into an absolute path (FR-002).
///
/// Returns `Ok(None)` when `declared` is absent (a non-JS, skill-only/MCP-only
/// plugin), and `Ok(Some(root/entry))` otherwise. A declared-but-blank entry is
/// a malformed manifest and errors. A non-empty declaration is never otherwise
/// rejected: a file that turns out to be absent is reported later, at
/// load/entry time, exactly as before.
fn resolve_entry(
    declared: &Option<String>,
    root: &Path,
    manifest: &Path,
    field: &str,
) -> Result<Option<PathBuf>, PluginError> {
    match declared {
        None => Ok(None),
        Some(entry) if entry.trim().is_empty() => Err(PluginError::ManifestParse {
            manifest: manifest.to_path_buf(),
            detail: format!("manifest {field} is empty"),
        }),
        Some(entry) => Ok(Some(root.join(entry))),
    }
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
/// code (FR-023). A directory carrying both nested dialect manifests resolves to
/// Claude (the multi-target layout) and parses via [`parse_claude_manifest`]; see
/// [`crate::descriptor::recognise_dialect`].
///
/// # Errors
///
/// Returns [`PluginError::AmbiguousManifest`] for a directory matching both
/// dialects in any combination other than the both-nested-manifest multi-target
/// pairing, [`PluginError::ManifestParse`] when the manifest bytes are
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

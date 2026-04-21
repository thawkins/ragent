//! Core shared state for the TUI application.
//!
//! This module contains the primary `App` state struct, related UI state enums,
//! and small helpers used by the TUI renderer and input handler.

use anyhow::Result;
use arboard::ImageData;
use image::{ImageBuffer, Rgba};
use lru::LruCache;
use ratatui::layout::Rect;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8};

use ragent_core::agent::{AgentInfo, CustomAgentDef};
use ragent_core::event::EventBus;
use ragent_core::lsp::{LspServer, SharedLspManager, discovery::DiscoveredServer};
use ragent_core::mcp::{McpServer, discovery::DiscoveredMcpServer};
use ragent_core::message::Message;
use ragent_core::permission::PermissionRequest;
use ragent_core::provider::ProviderRegistry;
use ragent_core::session::processor::SessionProcessor;
use ragent_core::storage::Storage;
use ragent_team::team::{SwarmState, TeamConfig, TeamMember};

use crate::theme::StatusHistory;

// Pending confirmation field is stored on App (defined in app.rs) as Option<PendingForceCleanup>.

/// Atomically update a JSON config file with file locking.
///
/// 1. Opens (or creates) a sibling `.lock` file and acquires an exclusive
///    `flock` on it (a separate lock file avoids inode confusion caused by
///    the atomic rename below).
/// 2. Reads the current JSON (missing/empty file → `{}`).
/// 3. Calls `updater` to mutate the parsed JSON value.
/// 4. Writes the result to a unique temp file in the same directory, then
///    atomically renames it over the original so readers never see a partial
///    write.
/// 5. Releases the lock.
///
/// # Errors
///
/// Returns an error string if any I/O or JSON (de)serialisation step fails.
pub fn atomic_config_update<F>(config_path: &std::path::Path, updater: F) -> Result<(), String>
where
    F: FnOnce(&mut serde_json::Value) -> Result<(), String>,
{
    use fs2::FileExt;
    use std::fs::OpenOptions;

    // Use a dedicated lock file so the flock survives the atomic rename of
    // the config file (flock is inode-based; renaming swaps inodes).
    let lock_path = config_path.with_extension("json.lock");
    let lock_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| format!("open lock {}: {e}", lock_path.display()))?;

    lock_file
        .lock_exclusive()
        .map_err(|e| format!("lock {}: {e}", lock_path.display()))?;

    // Read current content while holding the lock.
    let raw = std::fs::read_to_string(config_path).unwrap_or_default();

    let mut json: serde_json::Value = if raw.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", config_path.display()))?
    };

    updater(&mut json)?;

    let out = serde_json::to_string_pretty(&json).map_err(|e| format!("serialise config: {e}"))?;

    // Write to a unique temp file in the same directory, then rename.
    let parent = config_path
        .parent()
        .ok_or_else(|| "config path has no parent directory".to_string())?;
    let tmp =
        tempfile::NamedTempFile::new_in(parent).map_err(|e| format!("create temp file: {e}"))?;
    std::fs::write(tmp.path(), &out).map_err(|e| format!("write temp file: {e}"))?;
    tmp.persist(config_path)
        .map_err(|e| format!("rename temp → {}: {e}", config_path.display()))?;

    lock_file
        .unlock()
        .map_err(|e| format!("unlock {}: {e}", lock_path.display()))?;

    Ok(())
}

/// Returns `true` if `path` has a recognised image file extension.
pub fn is_image_path(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" | "tif")
    )
}

/// Decode `%XX` percent-encoding in a file-URI path component.
///
/// Decodes percent-encoded bytes into raw bytes and constructs a [`PathBuf`].
/// On Unix, raw bytes are preserved via `OsStr::from_bytes` so non-UTF-8 paths
/// round-trip correctly.  On other platforms, invalid UTF-8 is replaced with
/// the Unicode replacement character (lossy).
pub fn percent_decode_path(s: &str) -> std::path::PathBuf {
    let input = s.as_bytes();
    let mut bytes = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'%' && i + 2 < input.len() {
            if let Ok(decoded) =
                u8::from_str_radix(std::str::from_utf8(&input[i + 1..i + 3]).unwrap_or(""), 16)
            {
                bytes.push(decoded);
                i += 3;
                continue;
            }
        }
        bytes.push(input[i]);
        i += 1;
    }
    bytes_to_path(&bytes)
}

#[cfg(unix)]
fn bytes_to_path(bytes: &[u8]) -> std::path::PathBuf {
    use std::os::unix::ffi::OsStrExt;
    std::path::PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn bytes_to_path(bytes: &[u8]) -> std::path::PathBuf {
    std::path::PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

/// Maximum raw pixel buffer size we accept from the clipboard (50 MB).
const MAX_CLIPBOARD_IMAGE_BYTES: usize = 50 * 1024 * 1024;

/// Maximum dimension (width or height) we accept from the clipboard.
const MAX_CLIPBOARD_IMAGE_DIM: u32 = 16_384;

/// Encode `arboard::ImageData` (raw RGBA pixels) as a PNG saved to a
/// securely-created temp file.
///
/// Returns the path of the written file.  The file is persisted (not
/// auto-deleted) so the caller can attach it to a message.
///
/// # Errors
///
/// Returns an error if:
/// - The image exceeds the maximum allowed size or dimensions
/// - The image dimensions don't match the pixel buffer size
/// - The temporary file cannot be created or written
pub fn save_clipboard_image_to_temp(img_data: &ImageData<'_>) -> Result<std::path::PathBuf> {
    let buf_len = img_data.bytes.len();
    if buf_len > MAX_CLIPBOARD_IMAGE_BYTES {
        anyhow::bail!(
            "clipboard image too large ({:.1} MB, limit {:.0} MB)",
            buf_len as f64 / (1024.0 * 1024.0),
            MAX_CLIPBOARD_IMAGE_BYTES as f64 / (1024.0 * 1024.0),
        );
    }

    let width = img_data.width as u32;
    let height = img_data.height as u32;
    if width > MAX_CLIPBOARD_IMAGE_DIM || height > MAX_CLIPBOARD_IMAGE_DIM {
        anyhow::bail!(
            "clipboard image dimensions too large ({width}×{height}, max {MAX_CLIPBOARD_IMAGE_DIM}×{MAX_CLIPBOARD_IMAGE_DIM})"
        );
    }

    let bytes = img_data.bytes.as_ref().to_vec();
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, bytes)
        .ok_or_else(|| anyhow::anyhow!("clipboard image dimensions mismatch pixel buffer"))?;

    // Create a secure temporary file (O_EXCL, restrictive permissions).
    let tmp_file = tempfile::Builder::new()
        .prefix("ragent_paste_")
        .suffix(".png")
        .tempfile()
        .map_err(|e| anyhow::anyhow!("failed to create secure temp file: {e}"))?;

    img.save(tmp_file.path())?;

    // Prevent auto-deletion — the caller owns the file lifecycle.
    let path = tmp_file
        .into_temp_path()
        .keep()
        .map_err(|_| anyhow::anyhow!("failed to persist temp image file"))?;

    Ok(path)
}

/// Severity level for a log entry displayed in the log panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Informational message (prompts sent, session created, etc.).
    Info,
    /// Tool-related activity (call start, call end).
    Tool,
    /// Warning or recoverable issue.
    Warn,
    /// Unrecoverable error.
    Error,
}

/// A single entry in the log panel.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Wall-clock timestamp (UTC).
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Severity / category.
    pub level: LogLevel,
    /// Human-readable log message.
    pub message: String,
    /// Session ID this log entry belongs to (for filtering by agent).
    pub session_id: Option<String>,
    /// Agent ID that produced this log (for distinguishing teammates in multi-agent scenarios).
    pub agent_id: Option<String>,
}

/// A single completed LLM request used for `/llmstats` aggregation.
#[derive(Debug, Clone)]
pub struct LlmRequestStat {
    /// Provider/model identifier captured when the response completed.
    pub model_ref: String,
    /// Round-trip time for the request in milliseconds.
    pub elapsed_ms: u64,
    /// Prompt/input tokens reported by the provider.
    pub input_tokens: u64,
    /// Output/completion tokens reported by the provider.
    pub output_tokens: u64,
}

/// Aggregated LLM performance metrics for a single model in the current session.
#[derive(Debug, Clone, Copy)]
pub struct LlmStatsSummary {
    /// Number of completed request samples.
    pub samples: usize,
    /// Average round-trip latency in milliseconds.
    pub avg_elapsed_ms: f64,
    /// Average prompt/input throughput in tokens per second.
    pub avg_prompt_tps: f64,
    /// Average output throughput in tokens per second.
    pub avg_output_tps: f64,
}

/// Which screen the TUI is currently showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenMode {
    /// Three-panel chat layout with status bar, messages, and input.
    Chat,
}

/// Providers that ragent can connect to.
pub const PROVIDER_LIST: &[(&str, &str)] = &[
    ("anthropic", "Anthropic (Claude)"),
    ("openai", "OpenAI (GPT)"),
    ("gemini", "Google Gemini"),
    ("huggingface", "Hugging Face"),
    ("generic_openai", "Generic OpenAI API"),
    ("copilot", "GitHub Copilot"),
    ("ollama_cloud", "Ollama Cloud"),
    ("ollama", "Ollama (Local)"),
];

/// Entry in the model picker with full metadata for table display.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelPickerEntry {
    /// Model identifier (e.g. "gpt-4o").
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Context window size in tokens.
    pub context_window: usize,
    /// Max output tokens, if specified.
    pub max_output: Option<usize>,
    /// Input cost per million tokens.
    pub cost_input: f64,
    /// Output cost per million tokens.
    pub cost_output: f64,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Whether the model supports vision.
    pub vision: bool,
    /// Whether the model supports tool use.
    pub tool_use: bool,
    /// Cost tier label (e.g., "Free", "Low", "Medium", "High", "Premium").
    /// For Copilot, this shows the premium request tier based on multiplier.
    pub cost_tier: String,
    /// Cost multiplier relative to baseline (e.g., "0x", "1x", "3x", "10x").
    /// For Copilot, this is the premium request multiplier from GitHub docs.
    /// For other providers, this is relative to the least expensive model.
    pub cost_multiplier: String,
}

/// State of the interactive provider-setup dialog.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderSetupStep {
    /// Choosing which provider to configure.
    SelectProvider {
        /// Index of the highlighted provider in [`PROVIDER_LIST`].
        selected: usize,
    },
    /// Entering an API key for the chosen provider.
    EnterKey {
        /// The provider id (e.g. "anthropic").
        provider_id: String,
        /// Human-readable display name.
        provider_name: String,
        /// The key text entered so far.
        key_input: String,
        /// Cursor position (char index) inside `key_input`.
        key_cursor: usize,
        /// Optional API base URL (used by Generic OpenAI API provider).
        endpoint_input: String,
        /// Cursor position (char index) inside `endpoint_input`.
        endpoint_cursor: usize,
        /// Whether endpoint input is currently focused.
        editing_endpoint: bool,
        /// Optional error message from a previous attempt.
        error: Option<String>,
    },
    /// Waiting for the user to complete Copilot device flow authorisation.
    DeviceFlowPending {
        /// Short code the user enters at the verification URL.
        user_code: String,
        /// URL the user must visit (e.g. `https://github.com/login/device`).
        verification_uri: String,
    },
    /// Choosing which model to use from the selected provider.
    SelectModel {
        /// The provider id (e.g. "anthropic").
        provider_id: String,
        /// Human-readable provider display name.
        provider_name: String,
        /// Available models with full metadata.
        models: Vec<ModelPickerEntry>,
        /// Index of the highlighted model.
        selected: usize,
    },
    /// Setup complete — briefly confirm success.
    Done {
        /// Provider that was just configured.
        provider_name: String,
        /// Model that was selected, if any.
        model_name: Option<String>,
    },
    /// Choosing which agent to switch to.
    SelectAgent {
        /// Available agent names, descriptions, and custom flag.
        agents: Vec<(String, String, bool)>,
        /// Index of the highlighted agent.
        selected: usize,
    },
    /// Choosing which provider to reset and remove credentials for.
    ResetProvider {
        /// Index of the highlighted provider in [`PROVIDER_LIST`].
        selected: usize,
    },
    // ── GitLab setup steps ────────────────────────────────────────────
    /// Multi-field GitLab configuration: instance URL, PAT, username.
    ///
    /// Tab cycles between fields; Enter validates and saves.
    GitLabSetup {
        /// Instance URL entered so far (e.g. `https://gitlab.com`).
        url_input: String,
        /// Cursor position inside `url_input`.
        url_cursor: usize,
        /// Personal Access Token entered so far.
        token_input: String,
        /// Cursor position inside `token_input`.
        token_cursor: usize,
        /// Which field is currently focused (0 = URL, 1 = Token).
        active_field: u8,
        /// Optional error message from a previous attempt.
        error: Option<String>,
    },
    /// GitLab token validation in progress (async background task).
    GitLabValidating {
        /// Instance URL being validated.
        instance_url: String,
        /// Token being validated.
        token: String,
    },
}

/// Information about a configured provider.
#[derive(Debug, Clone)]
pub struct ConfiguredProvider {
    /// Provider identifier (e.g. "anthropic").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// How the key was found.
    pub source: ProviderSource,
}

/// Where a provider key came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSource {
    /// From an environment variable.
    EnvVar,
    /// From the ragent database.
    Database,
    /// Auto-discovered (e.g. Copilot IDE config).
    AutoDiscovered,
}

/// A registered slash command.
#[derive(Debug, Clone)]
pub struct SlashCommandDef {
    /// The trigger word (without the leading `/`).
    pub trigger: &'static str,
    /// Short description shown in the menu.
    pub description: &'static str,
}

/// All available slash commands.
pub const SLASH_COMMANDS: &[SlashCommandDef] = &[
    SlashCommandDef {
        trigger: "about",
        description: "Show application info, version, and authors",
    },
    SlashCommandDef {
        trigger: "agent",
        description: "Switch the active agent",
    },
    SlashCommandDef {
        trigger: "agents",
        description: "List all agents — built-in and custom",
    },
    SlashCommandDef {
        trigger: "browse_refresh",
        description: "Refresh the @ file-picker project index",
    },
    SlashCommandDef {
        trigger: "clear",
        description: "Clear message history for the current session",
    },
    SlashCommandDef {
        trigger: "cancel",
        description: "Cancel a background task (/cancel <task_id_prefix>)",
    },
    SlashCommandDef {
        trigger: "context",
        description: "Manage context cache: /context refresh",
    },
    SlashCommandDef {
        trigger: "compact",
        description: "Summarise and compact the conversation history",
    },
    SlashCommandDef {
        trigger: "cost",
        description: "Show session token usage and estimated cost",
    },
    SlashCommandDef {
        trigger: "help",
        description: "Show available slash commands",
    },
    SlashCommandDef {
        trigger: "history",
        description: "Browse and re-use previous inputs (↑/↓ to select, Enter to insert)",
    },
    SlashCommandDef {
        trigger: "inputdiag",
        description: "Dump input/cursor/selection diagnostics for troubleshooting",
    },
    SlashCommandDef {
        trigger: "log",
        description: "Toggle the log panel on/off",
    },
    SlashCommandDef {
        trigger: "llmstats",
        description: "Show average LLM response time and token throughput",
    },
    SlashCommandDef {
        trigger: "model",
        description: "Switch the active model on the current provider",
    },
    SlashCommandDef {
        trigger: "provider",
        description: "Change the LLM provider (re-enters setup flow)",
    },
    SlashCommandDef {
        trigger: "provider_reset",
        description: "Reset the current provider and remove stored credentials",
    },
    SlashCommandDef {
        trigger: "quit",
        description: "Exit ragent",
    },
    SlashCommandDef {
        trigger: "exit",
        description: "Exit ragent (alias of /quit)",
    },
    SlashCommandDef {
        trigger: "reload",
        description: "Reload customizations (/reload [all|config|mcp|skills|agents])",
    },
    SlashCommandDef {
        trigger: "resume",
        description: "Resume the agent from where it was halted",
    },
    SlashCommandDef {
        trigger: "system",
        description: "Override the agent system prompt (/system <prompt>)",
    },
    SlashCommandDef {
        trigger: "tools",
        description: "List all available tools (built-in and MCP)",
    },
    SlashCommandDef {
        trigger: "skills",
        description: "List all registered skills and their descriptions",
    },
    SlashCommandDef {
        trigger: "opt",
        description: "Prompt optimization helpers: /opt help or /opt <method> <prompt>",
    },
    SlashCommandDef {
        trigger: "tasks",
        description: "Show background task status and cancel tasks",
    },
    SlashCommandDef {
        trigger: "lsp",
        description: "Show LSP server status (/lsp discover | /lsp edit | /lsp connect <id> | /lsp disconnect <id>)",
    },
    SlashCommandDef {
        trigger: "mcp",
        description: "Show MCP server status (/mcp discover | /mcp connect <id> | /mcp disconnect <id>)",
    },
    SlashCommandDef {
        trigger: "todos",
        description: "Show TODO items for the current session",
    },
    SlashCommandDef {
        trigger: "team",
        description: "Team management (/team help|status|show [name]|create/open/delete <name>|close|message <id> <text>|tasks|clear|cleanup)",
    },
    SlashCommandDef {
        trigger: "teams",
        description: "Alias of /team (supports /teams show <name>)",
    },
    SlashCommandDef {
        trigger: "swarm",
        description: "Auto-decompose a goal into parallel subtasks (/swarm <prompt> | /swarm status | /swarm help)",
    },
    SlashCommandDef {
        trigger: "bash",
        description: "Manage bash command lists: /bash add|remove allow|deny <entry> [--global] | show | help",
    },
    SlashCommandDef {
        trigger: "dirs",
        description: "Manage directory/file permission lists: /dirs add|remove allow|deny <pattern> [--global] | show | help",
    },
    SlashCommandDef {
        trigger: "yolo",
        description: "Toggle YOLO mode — bypass all command validation and tool restrictions",
    },
    SlashCommandDef {
        trigger: "autopilot",
        description: "Autonomous operation: /autopilot on [--max-tokens N] [--max-time N] | off | status",
    },
    SlashCommandDef {
        trigger: "plan",
        description: "Delegate planning to the plan agent: /plan <task description>",
    },
    SlashCommandDef {
        trigger: "mode",
        description: "Set agent role mode: /mode architect|coder|reviewer|debugger|tester|off",
    },
    SlashCommandDef {
        trigger: "memory",
        description: "Memory browser: /memory | /memory show | /memory read <label> | /memory search <query>",
    },
    SlashCommandDef {
        trigger: "github",
        description: "GitHub integration: /github login | logout | status",
    },
    SlashCommandDef {
        trigger: "gitlab",
        description: "GitLab integration: /gitlab setup | logout | status",
    },
    SlashCommandDef {
        trigger: "update",
        description: "Check for or install updates: /update | /update install",
    },
    SlashCommandDef {
        trigger: "doctor",
        description: "Run system diagnostics (providers, git, ripgrep, MCP, memory)",
    },
    SlashCommandDef {
        trigger: "webapi",
        description: "Manage the HTTP REST API: /webapi enable | disable | help",
    },
    SlashCommandDef {
        trigger: "init",
        description: "Analyse the project and write a summary to .ragent/memory/PROJECT_ANALYSIS.md",
    },
    SlashCommandDef {
        trigger: "codeindex",
        description: "Manage codebase index: /codeindex on|off|show|reindex|help",
    },
    SlashCommandDef {
        trigger: "theme",
        description: "Switch theme: /theme default|high-contrast",
    },
    SlashCommandDef {
        trigger: "journal",
        description: "Journal viewer: /journal | /journal search <query> | /journal add <title>",
    },
    SlashCommandDef {
        trigger: "status",
        description: "Show status message history: /status [clear]",
    },
    SlashCommandDef {
        trigger: "mouse",
        description: "Toggle mouse support: /mouse on | off",
    },
    SlashCommandDef {
        trigger: "aiwiki",
        description: "AIWiki: /aiwiki init | on | off | show | reset | sync [--force] | status | help",
    },
];

/// A single entry in the slash-command autocomplete menu.
#[derive(Debug, Clone)]
pub struct SlashMenuEntry {
    /// The trigger word (without the leading `/`).
    pub trigger: String,
    /// Short description shown in the menu.
    pub description: String,
    /// Whether this entry is a skill (vs. a builtin command).
    pub is_skill: bool,
    /// Suggested completions for this command (e.g., team names, agent names).
    pub suggestions: Vec<String>,
    /// Parameter hint shown after command (e.g., "<query>" or "[clear]").
    pub parameter_hint: Option<String>,
}
/// State of the slash-command autocomplete menu.
#[derive(Debug, Clone)]
pub struct SlashMenuState {
    /// Entries that match the current filter.
    pub matches: Vec<SlashMenuEntry>,
    /// Currently highlighted index within `matches`.
    pub selected: usize,
    /// The filter text typed after `/` (e.g. `"mo"` for `/mo`).
    pub filter: String,
}

/// Pending confirmation for a destructive force-cleanup operation.
#[derive(Debug, Clone)]
pub struct PendingForceCleanup {
    /// The name of the active team (for display).
    pub team_name: String,
    /// Active teammate display names (for modal listing).
    pub active_members: Vec<String>,
}

/// State of the `/history` picker overlay.
#[derive(Debug, Clone)]
pub struct HistoryPickerState {
    /// A snapshot of the history entries, newest first.
    pub entries: Vec<String>,
    /// Currently highlighted row (0 = top = newest).
    pub selected: usize,
    /// Scroll offset for the list (rows from the top).
    pub scroll_offset: usize,
}

/// An entry in the `@` file reference autocomplete menu.
#[derive(Debug, Clone)]
pub struct FileMenuEntry {
    /// Display string shown in the menu.
    pub display: String,
    /// Relative path to the file or directory.
    pub path: std::path::PathBuf,
    /// Whether this entry is a directory.
    pub is_dir: bool,
}

/// State of the `@` file reference autocomplete menu.
#[derive(Debug, Clone)]
pub struct FileMenuState {
    /// Entries that match the current query.
    pub matches: Vec<FileMenuEntry>,
    /// Currently highlighted index within `matches`.
    pub selected: usize,
    /// Scroll offset for long result lists.
    pub scroll_offset: usize,
    /// The query text typed after `@` (e.g. `"main"` for `@main`).
    pub query: String,
    /// If set, the menu is currently showing the contents of this directory
    /// (relative to the project root). `None` means fuzzy/global mode.
    pub current_dir: Option<std::path::PathBuf>,
}

/// Identifies which pane a scrollbar drag is acting on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarDragPane {
    /// Dragging the messages pane scrollbar.
    Messages,
    /// Dragging the log pane scrollbar.
    Log,
}

/// Identifies which pane a text selection lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionPane {
    /// Selection in the messages pane.
    Messages,
    /// Selection in the log pane.
    Log,
    /// Selection in the chat-screen input widget.
    Input,
}

/// A mouse-driven text selection within a pane.
#[derive(Debug, Clone)]
pub struct TextSelection {
    /// Which pane the selection is in.
    pub pane: SelectionPane,
    /// Anchor point (where the mouse was first pressed), screen coordinates.
    pub anchor: (u16, u16),
    /// Current endpoint (where the mouse is now), screen coordinates.
    pub endpoint: (u16, u16),
}

impl TextSelection {
    /// Return `(start, end)` with start ≤ end in row-major order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ragent_tui::app::{TextSelection, SelectionPane};
    ///
    /// let sel = TextSelection {
    ///     pane: SelectionPane::Messages,
    ///     anchor: (10, 5),
    ///     endpoint: (3, 2),
    /// };
    /// let ((start_col, start_row), (end_col, end_row)) = sel.normalized();
    /// assert_eq!((start_col, start_row), (3, 2));
    /// assert_eq!((end_col, end_row), (10, 5));
    /// ```
    pub fn normalized(&self) -> ((u16, u16), (u16, u16)) {
        if self.anchor.1 < self.endpoint.1
            || (self.anchor.1 == self.endpoint.1 && self.anchor.0 <= self.endpoint.0)
        {
            (self.anchor, self.endpoint)
        } else {
            (self.endpoint, self.anchor)
        }
    }
}

/// Which action the context menu item represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    /// Copy selected text then delete it from the input.
    Cut,
    /// Copy selected text to the clipboard.
    Copy,
    /// Insert clipboard text at the current input cursor.
    Paste,
}

/// State for the right-click context menu.
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    /// Screen column where the menu top-left should appear.
    pub x: u16,
    /// Screen row where the menu top-left should appear.
    pub y: u16,
    /// The pane that was right-clicked.
    pub pane: SelectionPane,
    /// Currently highlighted item index.
    pub selected: usize,
    /// Items available in this context (enabled/disabled).
    pub items: Vec<(ContextAction, bool)>,
}

/// Target represented by the output overlay.
#[derive(Debug, Clone)]
pub enum OutputViewTarget {
    /// Show output for a concrete session.
    Session {
        /// Session id to display.
        session_id: String,
        /// Human-friendly label shown in the title.
        label: String,
    },
    /// Show output for a team member (with optional linked session).
    TeamMember {
        /// Team name used in log prefixes.
        team_name: String,
        /// Teammate id (e.g. `tm-001`).
        agent_id: String,
        /// Human-friendly teammate name.
        teammate_name: String,
        /// Optional linked session id.
        session_id: Option<String>,
    },
}

/// State for the scrollable output overlay panel.
#[derive(Debug, Clone)]
pub struct OutputViewState {
    /// Selected output target.
    pub target: OutputViewTarget,
    /// Vertical scroll offset from top.
    pub scroll_offset: u16,
    /// Maximum scroll value computed during render.
    pub max_scroll: u16,
}

///
/// Shown as an overlay that lists discovered language servers with numbered
/// rows. The user types a number and presses Enter to enable a server, or
/// presses Esc to dismiss.
#[derive(Debug, Clone)]
pub struct LspDiscoverState {
    /// Servers found during discovery.
    pub servers: Vec<DiscoveredServer>,
    /// Number being typed by the user (e.g. `"2"`).
    pub number_input: String,
    /// Cursor position (char index) inside `number_input`.
    pub number_cursor: usize,
    /// Feedback message shown after an enable action or on error.
    pub feedback: Option<String>,
    /// Vertical scroll offset for the server list (rows scrolled past the top).
    pub scroll_offset: u16,
}

/// State for the interactive `/lsp edit` dialog.
///
/// Shows all configured LSP servers (from ragent.json) with their enabled/disabled
/// status. Arrow keys move the cursor; Space or Enter toggles enabled/disabled.
#[derive(Debug, Clone)]
pub struct LspEditState {
    /// Configured servers: (id, disabled flag).
    pub servers: Vec<(String, bool)>,
    /// Index of the currently highlighted row.
    pub selected: usize,
    /// Vertical scroll offset.
    pub scroll_offset: u16,
    /// Feedback message shown after a toggle action.
    pub feedback: Option<String>,
}

/// State for the interactive `/mcp discover` dialog.
///
/// Shown as an overlay that lists discovered MCP servers with numbered
/// rows. The user types a number and presses Enter to enable a server, or
/// presses Esc to dismiss.
#[derive(Debug, Clone)]
pub struct McpDiscoverState {
    /// Servers found during discovery.
    pub servers: Vec<DiscoveredMcpServer>,
    /// Number being typed by the user (e.g. `"2"`).
    pub number_input: String,
    /// Cursor position (char index) inside `number_input`.
    pub number_cursor: usize,
    /// Feedback message shown after an enable action or on error.
    pub feedback: Option<String>,
}

/// Core TUI application state.
///
/// Holds the message list, input buffer, scroll offset, permission dialogs,
/// token usage counters, and a reference to the shared [`EventBus`].
pub struct App {
    /// Flag indicating whether a UI redraw is required.
    pub needs_redraw: bool,
    /// Chat message history.
    pub messages: Vec<Message>,
    /// Current text input buffer.
    pub input: String,
    /// Scroll offset for the message view (lines from bottom).
    pub scroll_offset: u16,
    /// Whether the event loop should keep running.
    pub is_running: bool,
    /// Shared event bus for agent communication.
    pub event_bus: Arc<EventBus>,
    /// Persistent storage for provider auth and sessions.
    pub storage: Arc<Storage>,
    /// Current session identifier.
    pub session_id: Option<String>,
    /// Name of the active agent.
    pub agent_name: String,
    /// Human-readable status string shown in the status bar.
    pub status: String,
    /// Queue of pending permission requests awaiting user resolution.
    /// The front of the queue is the currently displayed dialog; subsequent
    /// requests are shown one-at-a-time as earlier ones are resolved.
    pub permission_queue: VecDeque<PermissionRequest>,
    /// Text typed by the user in response to a `question`-type permission dialog.
    /// Only active when the front permission request has `permission == "question"`.
    pub pending_question_input: String,
    /// Cumulative (input, output) token counts.
    pub token_usage: (u64, u64),
    /// Completed LLM request samples used to compute `/llmstats`.
    pub llm_request_stats: Vec<LlmRequestStat>,
    /// Input token count from the most recent LLM request (used for context-window % display).
    pub last_input_tokens: u64,
    /// Bytes received from the current LLM streaming response (reset per request).
    pub stream_bytes: u64,
    /// Latest quota usage percentage from provider rate-limit headers (0.0–100.0).
    /// `None` if the provider has not returned rate-limit information yet.
    pub quota_percent: Option<f32>,
    /// Which screen is currently displayed.
    pub current_screen: ScreenMode,
    /// Randomly selected tip shown on the home screen.
    pub tip: &'static str,
    /// Current working directory displayed on the home screen.
    pub cwd: String,
    /// Shell working directory as reported after each bash command.
    ///
    /// Updated by `ShellCwdChanged` events. `None` until the first bash
    /// command is executed in this session.
    pub shell_cwd: Option<String>,
    /// Git branch name if the cwd is inside a git repository.
    pub git_branch: Option<String>,
    /// Provider setup dialog state, if the dialog is open.
    pub provider_setup: Option<ProviderSetupStep>,
    /// Currently configured provider, if any.
    pub configured_provider: Option<ConfiguredProvider>,
    /// Provider registry for querying available models.
    pub provider_registry: Arc<ProviderRegistry>,
    /// Currently selected model in `"provider/model"` format, if any.
    pub selected_model: Option<String>,
    /// Context window size (tokens) for the currently selected model.
    /// Set during model selection; used when `resolve_model()` cannot find the model
    /// (e.g. dynamically discovered ollama/ollama_cloud models).
    pub selected_model_ctx_window: Option<usize>,
    /// Session processor for sending messages to the LLM.
    pub session_processor: Arc<SessionProcessor>,
    /// Resolved agent configuration.
    pub agent_info: AgentInfo,
    /// Non-hidden agents available for cycling via Shift+Tab.
    pub cycleable_agents: Vec<AgentInfo>,
    /// Index into `cycleable_agents` for the currently active agent.
    pub current_agent_index: usize,
    /// Whether the configured provider/model is reachable.
    /// `0` = not yet checked, `1` = available, `2` = unavailable.
    pub provider_health: Arc<AtomicU8>,
    /// Slash-command autocomplete menu, shown when the input starts with `/`.
    pub slash_menu: Option<SlashMenuState>,
    /// File reference autocomplete menu, shown when `@` is typed.
    pub file_menu: Option<FileMenuState>,
    /// Whether directory mode should include hidden files/dirs.
    pub file_menu_show_hidden: bool,
    /// Cached project files for `@` autocomplete (lazily populated).
    pub project_files_cache: Option<Vec<std::path::PathBuf>>,
    /// Working directory used to build `project_files_cache`.
    pub project_files_cache_cwd: Option<std::path::PathBuf>,
    /// Last refresh timestamp for the `@` picker cache.
    pub project_files_cache_refreshed_at: Option<std::time::SystemTime>,
    /// Number of indexed entries from the last cache refresh.
    pub project_files_cache_count: usize,
    /// Previously submitted input lines (oldest first).
    pub input_history: Vec<String>,
    /// Current position when navigating history (`None` = new input).
    pub history_index: Option<usize>,
    /// Saved in-progress input while browsing history.
    pub history_draft: String,
    /// Cursor position (character index) within the input line.
    pub input_cursor: usize,
    /// Keyboard selection anchor (character index). When `Some(n)`, the region
    /// between `n` and `input_cursor` forms the active keyboard selection.
    pub kb_select_anchor: Option<usize>,
    /// Whether the log panel is visible.
    pub show_log: bool,
    /// Log entries displayed in the log panel.
    pub log_entries: Vec<LogEntry>,
    /// Scroll offset for the log panel (lines from bottom).
    pub log_scroll_offset: u16,
    /// Cached area of the messages pane (set during render for mouse hit-testing).
    pub message_area: Rect,
    /// Cached area of the log panel (set during render for mouse hit-testing).
    pub log_area: Rect,
    /// Maximum scroll value for the messages pane (set during render).
    pub message_max_scroll: u16,
    /// Maximum scroll value for the log pane (set during render).
    pub log_max_scroll: u16,
    /// Scroll offset for the active-agents subpanel (lines from top).
    pub active_agents_scroll_offset: u16,
    /// Maximum scroll value for the active-agents subpanel (set during render).
    pub active_agents_max_scroll: u16,
    /// Cached area of the active-agents subpanel.
    pub active_agents_area: Rect,
    /// Active scrollbar drag, if any.
    pub scrollbar_drag: Option<ScrollbarDragPane>,
    /// Active text selection, if any.
    pub text_selection: Option<TextSelection>,
    /// Plain-text lines from the last message pane render (for copy).
    pub message_content_lines: Vec<String>,
    /// Plain-text lines from the last log pane render (for copy).
    pub log_content_lines: Vec<String>,
    /// Cached area of the chat-screen input widget (set during render).
    pub input_area: Rect,
    /// Cached area of the teams subpanel.
    pub teams_area: Rect,
    /// Cached area of the output overlay.
    pub output_view_area: Rect,
    /// Cached area of the Agents button beside chat input.
    pub agents_button_area: Rect,
    /// Cached area of the Teams button beside chat input.
    pub teams_button_area: Rect,
    /// Cached area of the AIWiki status indicator (for click-to-open-browser).
    pub aiwiki_status_area: Rect,
    /// Whether the Agents popup window is visible.
    pub show_agents_window: bool,
    /// Whether the Teams popup window is visible.
    pub show_teams_window: bool,
    /// Cached click target for Agents popup close button.
    pub agents_close_button_area: Rect,
    /// Cached click target for Teams popup close button.
    pub teams_close_button_area: Rect,
    /// Snapshot of discovered MCP servers (populated by `/mcp discover`).
    pub mcp_servers: Vec<McpServer>,
    /// Snapshot of LSP server descriptors (populated via `LspStatusChanged` events).
    pub lsp_servers: Vec<LspServer>,
    /// Handle to the running LSP manager (kept alive for the lifetime of the TUI).
    pub lsp_manager: Option<SharedLspManager>,
    /// Optional code index for codebase search and symbol lookup.
    pub code_index: Option<Arc<ragent_codeindex::CodeIndex>>,
    /// Whether code indexing is enabled in configuration.
    pub code_index_enabled: bool,
    /// Cached code index stats for the status bar (refreshed every few seconds).
    pub code_index_stats_cache: Option<ragent_codeindex::types::IndexStats>,
    /// When the cached stats were last refreshed.
    pub code_index_stats_last_refresh: std::time::Instant,
    /// True when the background indexer holds the store/FTS locks.
    pub code_index_busy: bool,
    /// Active file watcher + background worker session for the code index.
    pub code_index_watch_session: Option<ragent_codeindex::WatchSession>,
    /// Optional AIWiki instance for project knowledge base.
    pub aiwiki: Option<ragent_aiwiki::Aiwiki>,
    /// Whether AIWiki is enabled for the current project.
    pub aiwiki_enabled: bool,
    /// Cached AIWiki stats for the status bar.
    pub aiwiki_stats_cache: Option<(usize, usize, usize)>, // (raw_sources, ref_sources, pages)
    /// When the cached AIWiki stats were last refreshed.
    pub aiwiki_stats_last_refresh: std::time::Instant,
    /// Handle for the spawned AIWiki web server task.
    pub aiwiki_web_server: Option<tokio::task::JoinHandle<()>>,
    /// Port the AIWiki web server is listening on.
    pub aiwiki_web_port: u16,
    /// Handle for a background AIWiki sync task.
    pub aiwiki_sync_handle: Option<tokio::task::JoinHandle<super::AiwikiSyncOutcome>>,
    /// Shared progress counter for the active sync, if any.
    pub aiwiki_sync_progress: Option<std::sync::Arc<ragent_aiwiki::sync::SyncProgress>>,
    /// Active file watcher session for AIWiki source folders.
    pub aiwiki_watch_session: Option<ragent_aiwiki::sync::AiwikiWatchSession>,
    /// Whether AIWiki should auto-sync on startup and watch for changes.
    pub aiwiki_autosync: bool,
    /// Active LSP discovery dialog, if any.
    pub lsp_discover: Option<LspDiscoverState>,
    /// Active LSP edit dialog (enable/disable configured servers), if any.
    pub lsp_edit: Option<LspEditState>,
    /// Active MCP discovery dialog, if any.
    pub mcp_discover: Option<McpDiscoverState>,
    /// When true, the next assistant text delta starts a new message instead
    /// of appending to the current one. Set by `MessageEnd` events to
    /// separate init-exchange output from the main response.
    pub force_new_message: bool,
    /// Saved agent stack for returning from sub-agents (e.g. plan → general).
    pub agent_stack: Vec<AgentInfo>,
    /// Pending plan delegation: `(task, context)` set by `AgentSwitchRequested`,
    /// consumed by `MessageEnd` to auto-send the task to the plan agent.
    pub pending_plan_task: Option<(String, String)>,
    /// Pending agent restore: summary from `AgentRestoreRequested`,
    /// consumed by `MessageEnd` to pop the agent stack and inject the summary.
    pub pending_plan_restore: Option<String>,
    /// Pending confirmation for destructive force-cleanup modal.
    pub pending_forcecleanup: Option<PendingForceCleanup>,
    /// Whether the agent is currently processing a message.
    pub is_processing: bool,
    /// Cancellation flag shared with the processor task; set to `true` on ESC.
    pub cancel_flag: Option<Arc<AtomicBool>>,
    /// True while an automatic pre-send compaction run is active.
    pub auto_compact_in_progress: bool,
    /// True while any compaction run (manual or auto) is active.
    /// Used to trigger message-history replacement when the LLM finishes.
    pub compact_in_progress: bool,
    /// Set when an auto-compaction run returns an error.
    pub auto_compact_failed: bool,
    /// User message queued while auto-compaction runs: `(text, image_paths)`.
    pub pending_send_after_compact: Option<(String, Vec<std::path::PathBuf>)>,
    /// Whether the last agent run was halted by the user (ESC).
    pub agent_halted: bool,
    /// Maps tool call IDs to their `(short_session_id, step_number, sub_step)` for log/message correlation.
    /// Step number comes from EventBus; sub_step is per-tool-call within a step.
    pub tool_step_map: HashMap<String, (String, u32, u32)>,
    /// Pending tool call args received before the ToolCallStart event. Some providers
    /// may emit args/result events before the start event; store them here and apply
    /// when the ToolCallStart arrives.
    pub pending_tool_args: HashMap<String, String>,
    /// Tracks the last seen step number for each session (to detect step changes).
    pub last_step_per_session: HashMap<String, u32>,
    /// Tracks the current sub-step counter for each session (resets when step changes).
    pub substep_counter_per_session: HashMap<String, u32>,
    /// Maps short session IDs (`short_sid`) to display agent names.
    /// Display names are "ag[nnn]" (auto-allocated) or the actual agent name if available.
    pub sid_to_display_name: HashMap<String, String>,
    /// Counter for auto-allocating "ag[nnn]" display names.
    pub next_agent_index: u32,
    /// Active background sub-agent tasks (F14).
    pub active_tasks: Vec<ragent_core::task::TaskEntry>,
    /// Whether the keybindings help panel is currently visible.
    pub show_shortcuts: bool,
    /// Whether Ctrl+C has armed a guarded keyboard exit sequence.
    pub quit_armed: bool,
    /// Active right-click context menu, if any.
    pub context_menu: Option<ContextMenuState>,
    /// Image files staged to be sent with the next message (populated by Alt+V).
    pub pending_attachments: Vec<std::path::PathBuf>,
    /// Path to the persistent input history file.
    pub history_file_path: Option<std::path::PathBuf>,
    /// Active history picker dialog, if any.
    pub history_picker: Option<HistoryPickerState>,
    /// Session ID of the currently selected agent in the agents panel.
    /// When set, messages and logs are filtered to show only from this session.
    /// When `None`, shows primary session messages/logs.
    pub selected_agent_session_id: Option<String>,
    /// Index of the selected agent in the agents panel (for keyboard/mouse navigation).
    /// 0 = primary agent, 1+ = sub-agents in order.
    /// When `None`, no agent is selected (or selection is disabled).
    pub selected_agent_index: Option<usize>,
    /// Custom agent definitions loaded from disk at startup.
    pub custom_agent_defs: Vec<CustomAgentDef>,
    /// Diagnostics from custom agent loading (parse errors, validation failures, collisions).
    pub custom_agent_diagnostics: Vec<String>,
    /// The currently active team config, if the lead is managing a team.
    pub active_team: Option<TeamConfig>,
    /// Current members of the active team (updated from events).
    pub team_members: Vec<TeamMember>,
    /// Per-teammate message counters: `agent_id -> (sent, received)`.
    pub team_message_counts: HashMap<String, (u32, u32)>,
    /// Whether the Teams panel is visible in the sidebar.
    pub show_teams: bool,
    /// Scroll offset for the Teams panel.
    pub teams_scroll_offset: u16,
    /// Max scroll for the Teams panel.
    pub teams_max_scroll: u16,
    /// Currently focused teammate (agent_id). When set, the status
    /// bar shows a focus indicator and the input box routes messages
    /// to this teammate's mailbox instead of the lead session.
    pub focused_teammate: Option<String>,
    /// Active swarm state (if a /swarm is running).
    pub swarm_state: Option<SwarmState>,
    /// Pending result from an async `/swarm` LLM decomposition call.
    pub swarm_result: Arc<std::sync::Mutex<Option<Result<String, String>>>>,
    /// Active output overlay state.
    pub output_view: Option<OutputViewState>,
    /// Pending result from an async `/opt` LLM call.
    pub opt_result: Arc<std::sync::Mutex<Option<Result<String, String>>>>,
    /// Whether input history has been modified since last save.
    pub history_dirty: bool,
    /// Deadline after which a dirty history should be flushed to disk.
    /// Set on the first modification; cleared after each flush.
    pub history_save_deadline: Option<std::time::Instant>,
    /// Cache for rendered markdown output, keyed by FNV-style hash of input text.
    /// Cleared when messages change.
    pub md_render_cache: LruCache<u64, String>,

    // ── Autopilot (M2 Task 2.1) ─────────────────────────────────────────────
    /// True when autopilot mode is active. Agent continues autonomously until
    /// task_complete is called, limits are hit, or the user runs /autopilot off.
    pub autopilot_enabled: bool,
    /// Maximum number of tokens to consume before stopping autopilot.
    pub autopilot_token_budget: Option<u64>,
    /// Maximum wall-clock seconds to run before stopping autopilot.
    pub autopilot_time_limit_secs: Option<u64>,
    /// Wall-clock instant when autopilot was started (for time-limit enforcement).
    pub autopilot_started_at: Option<std::time::Instant>,
    /// Pending autopilot continuation: when Some, the next render tick will
    /// auto-send this text to the agent to continue processing.
    pub autopilot_pending_continue: Option<String>,

    // ── Processing timing (for log breakdown) ───────────────────────────────
    /// Wall-clock instant when the current prompt was sent (for total elapsed time).
    pub prompt_start_time: Option<std::time::Instant>,
    /// Cumulative time spent in tool calls during this processing cycle.
    pub tool_time_ms: u64,
    /// Cumulative time spent waiting for LLM responses during this processing cycle.
    pub llm_time_ms: u64,

    // ── Plan approval (M2 Task 2.2) ��────────────────────────────────────────    /// When Some, the plan approval overlay is shown. Holds the plan text and
    /// the agent to restore on approval.
    pub plan_approval_pending: Option<PlanApprovalState>,

    // ── Agent role mode (M2 Task 2.3) ───────────────────────────────────────
    /// Currently active role mode. None = normal (general-purpose) mode.
    pub role_mode: Option<RoleMode>,
    /// Running HTTP API server handle. `None` when the server is disabled (default).
    pub webapi_server: Option<tokio::task::JoinHandle<()>>,
    /// Address the HTTP API server is bound to.
    pub webapi_addr: String,
    /// Bearer token for the HTTP API. Randomly generated on `/webapi enable`.
    pub webapi_token: Option<String>,

    // ── Memory browser (M7-T1) ────────────────────────────────────────────────
    /// Active memory browser overlay, if visible.
    pub memory_browser: Option<crate::panels::MemoryBrowserState>,
    /// Cached click target for memory browser close button.
    pub memory_browser_close_area: Rect,
    /// Render area for memory browser content.
    pub memory_browser_area: Rect,

    // ── Journal viewer (M7-T2) ───────────────────────────────────────────────
    /// Active journal viewer overlay, if visible.
    pub journal_viewer: Option<crate::panels::JournalViewerState>,
    /// Cached click target for journal viewer close button.
    pub journal_viewer_close_area: Rect,
    /// Render area for journal viewer content.
    pub journal_viewer_area: Rect,

    // ── Memory status (M7-T3) ─────────────────────────────────────────────────
    /// Cached count of memory blocks (global + project).
    pub memory_block_count: usize,
    /// Cached count of structured memories (SQLite).
    pub memory_entry_count: u64,
    /// Cached count of journal entries.
    pub journal_entry_count: u64,
    /// Timestamp of the last memory update event (for relative time display).
    pub memory_last_updated: Option<std::time::Instant>,

    /// Current theme mode (default or high-contrast for accessibility)
    pub theme_mode: crate::theme::ThemeMode,
    /// Whether mouse input is enabled (default: true). Set to false for
    /// keyboard-only accessibility mode.
    pub mouse_enabled: bool,
    /// Status message history for tracking recent status messages
    pub status_history: StatusHistory,
}
/// State held while waiting for the user to approve or reject a plan.
#[derive(Debug, Clone)]
pub struct PlanApprovalState {
    /// The plan text produced by the plan agent.
    pub plan_text: String,
    /// Whether the dialog cursor is on Approve (true) or Reject (false).
    pub cursor_approve: bool,
}

/// Specialised agent behaviour modes (M2 Task 2.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleMode {
    /// Focus on architecture, design, and high-level planning. Read-only posture.
    Architect,
    /// Focus on implementation with full tool access.
    Coder,
    /// Focus on code review and suggestions. Read-only posture.
    Reviewer,
    /// Focus on root-cause analysis and targeted fixes.
    Debugger,
    /// Focus on writing and running tests.
    Tester,
}

impl RoleMode {
    /// The display name shown in the status bar.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Architect => "architect",
            Self::Coder => "coder",
            Self::Reviewer => "reviewer",
            Self::Debugger => "debugger",
            Self::Tester => "tester",
        }
    }

    /// An emoji indicator for the status bar.
    #[must_use]
    pub fn icon(&self) -> &str {
        match self {
            Self::Architect => "🏛",
            Self::Coder => "💻",
            Self::Reviewer => "🔍",
            Self::Debugger => "🐛",
            Self::Tester => "🧪",
        }
    }

    /// Additional system-prompt text injected when this mode is active.
    #[must_use]
    pub fn system_prompt_addition(&self) -> &str {
        match self {
            Self::Architect => {
                "You are in ARCHITECT mode. Focus exclusively on design, architecture, \
                 and high-level planning. Produce written plans and diagrams. \
                 Do NOT modify any files — use only read-only tools (read, list, glob, grep, bash \
                 for read-only commands). When you have produced a plan, summarise it clearly."
            }
            Self::Coder => {
                "You are in CODER mode. Focus on implementation. Write clean, tested, idiomatic \
                 code. Use all available tools. Follow existing conventions in the codebase."
            }
            Self::Reviewer => {
                "You are in REVIEWER mode. Review the code for correctness, security, performance, \
                 and style. Do NOT modify files — read and report only. Provide specific, actionable \
                 feedback with file and line references."
            }
            Self::Debugger => {
                "You are in DEBUGGER mode. Systematically investigate the reported issue. \
                 Identify root causes with evidence. Make targeted, minimal fixes. \
                 Add regression tests where appropriate."
            }
            Self::Tester => {
                "You are in TESTER mode. Write comprehensive tests covering edge cases, \
                 error paths, and happy paths. Follow the existing test style and conventions. \
                 Run tests and report results."
            }
        }
    }

    /// Parse a role mode from a string (case-insensitive).
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "architect" => Some(Self::Architect),
            "coder" => Some(Self::Coder),
            "reviewer" => Some(Self::Reviewer),
            "debugger" => Some(Self::Debugger),
            "tester" => Some(Self::Tester),
            _ => None,
        }
    }
}
impl App {
    /// Set the path for persisting input history.
    pub fn set_history_file(&mut self, path: std::path::PathBuf) {
        self.history_file_path = Some(path);
    }

    /// Load input history from the configured file (if it exists).
    ///
    /// # Errors
    ///
    /// Returns an error if the history file cannot be read.
    pub fn load_history(&mut self) -> Result<(), std::io::Error> {
        if let Some(ref path) = self.history_file_path {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                self.input_history.clear();
                for line in content.lines() {
                    if !line.is_empty() {
                        // Unescape: literal "\n" → newline, "\\" → backslash
                        let entry = line.replace("\\n", "\n").replace("\\\\", "\\");
                        self.input_history.push(entry);
                    }
                }
                // Trim to 100 entries
                if self.input_history.len() > 100 {
                    self.input_history
                        .drain(0..(self.input_history.len() - 100));
                }
                tracing::debug!(
                    "Loaded {} history entries from {:?}",
                    self.input_history.len(),
                    path
                );
            }
        }
        Ok(())
    }

    /// Save input history to the configured file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Parent directories cannot be created
    /// - The history file cannot be written
    pub fn save_history(&self) -> Result<(), std::io::Error> {
        if let Some(ref path) = self.history_file_path {
            // Create parent directories if needed
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let content = history_entries_to_string(&self.input_history);
            std::fs::write(path, content)?;
            tracing::debug!(
                "Saved {} history entries to {:?}",
                self.input_history.len(),
                path
            );
        }
        Ok(())
    }

    /// Flush history to disk in a background thread if the debounce deadline
    /// has elapsed.  Called from the TUI main loop (~50 ms cadence).
    ///
    /// This avoids blocking the UI thread on every keystroke while still
    /// persisting history within a few seconds of a change.
    pub fn flush_history_if_due(&mut self) {
        if !self.history_dirty {
            return;
        }
        if let Some(deadline) = self.history_save_deadline {
            if std::time::Instant::now() < deadline {
                return;
            }
        }
        let Some(ref path) = self.history_file_path else {
            return;
        };
        let path = path.clone();
        let content = history_entries_to_string(&self.input_history);
        let entry_count = self.input_history.len();

        tokio::task::spawn_blocking(move || {
            if let Some(parent) = path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    tracing::warn!("Failed to create history directory: {e}");
                    return;
                }
            }
            if let Err(e) = std::fs::write(&path, content) {
                tracing::warn!("Failed to save history (async): {e}");
            } else {
                tracing::debug!("Saved {entry_count} history entries to {path:?}");
            }
        });

        self.history_dirty = false;
        self.history_save_deadline = None;
    }
}

/// Serialise history entries to a newline-separated string.
///
/// Each entry has its backslashes escaped (`\` → `\\`) and embedded newlines
/// escaped (`\n` → `\n` literal two-char sequence) so that multiline entries
/// survive a round-trip through the file format without being split.
fn history_entries_to_string(entries: &[String]) -> String {
    entries
        .iter()
        .map(|e| e.replace('\\', "\\\\").replace('\n', "\\n"))
        .collect::<Vec<_>>()
        .join("\n")
}

//! Core shared state for the TUI application.
//!
//! This module contains the primary `App` state struct, related UI state enums,
//! and small helpers used by the TUI renderer and input handler.

use anyhow::Result;
use arboard::ImageData;
use image::{ImageBuffer, Rgba};
use ratatui::layout::Rect;
use std::collections::HashMap;
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
use ragent_core::team::{TeamConfig, TeamMember};

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
pub fn percent_decode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(a), Some(b)) = (h1, h2) {
                if let Ok(byte) = u8::from_str_radix(&format!("{a}{b}"), 16) {
                    out.push(byte as char);
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

/// Encode `arboard::ImageData` (raw RGBA pixels) as a PNG saved to a temp file.
///
/// Returns the path of the written file.
///
/// # Errors
///
/// Returns an error if:
/// - The image dimensions don't match the pixel buffer size
/// - The PNG file cannot be written to the temp directory
pub fn save_clipboard_image_to_temp(
    img_data: &ImageData<'_>,
) -> Result<std::path::PathBuf> {
    let width = img_data.width as u32;
    let height = img_data.height as u32;
    let bytes = img_data.bytes.as_ref().to_vec();

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, bytes)
        .ok_or_else(|| anyhow::anyhow!("clipboard image dimensions mismatch pixel buffer"))?;

    let tmp_dir = std::env::temp_dir();
    let filename = format!(
        "ragent_paste_{}.png",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let path = tmp_dir.join(filename);
    img.save(&path)?;
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
}

/// Which screen the TUI is currently showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenMode {
    /// Centered landing page with logo, prompt, and tips.
    Home,
    /// Three-panel chat layout with status bar, messages, and input.
    Chat,
}

/// Providers that ragent can connect to.
pub const PROVIDER_LIST: &[(&str, &str)] = &[
    ("anthropic", "Anthropic (Claude)"),
    ("openai", "OpenAI (GPT)"),
    ("generic_openai", "Generic OpenAI API"),
    ("copilot", "GitHub Copilot"),
    ("ollama", "Ollama (Local)"),
];

/// State of the interactive provider-setup dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        /// Available models as `(model_id, display_name)` pairs.
        models: Vec<(String, String)>,
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
        trigger: "compact",
        description: "Summarise and compact the conversation history",
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
        trigger: "tasks",
        description: "Show background task status and cancel tasks",
    },
    SlashCommandDef {
        trigger: "lsp",
        description: "Show LSP server status (/lsp discover | /lsp connect <id> | /lsp disconnect <id>)",
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
        description: "Team management (/team status | create/open/delete <name> | close | message <id> <text> | tasks | clear | cleanup)",
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
    /// Selection in the home-screen input widget.
    HomeInput,
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
    /// Active permission request overlay, if any.
    pub permission_pending: Option<PermissionRequest>,
    /// Cumulative (input, output) token counts.
    pub token_usage: (u64, u64),
    /// Input token count from the most recent LLM request (used for context-window % display).
    pub last_input_tokens: u64,
    /// Latest quota usage percentage from provider rate-limit headers (0.0–100.0).
    /// `None` if the provider has not returned rate-limit information yet.
    pub quota_percent: Option<f32>,
    /// Which screen is currently displayed.
    pub current_screen: ScreenMode,
    /// Randomly selected tip shown on the home screen.
    pub tip: &'static str,
    /// Current working directory displayed on the home screen.
    pub cwd: String,
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
    /// Cached area of the home-screen input widget (set during render).
    pub home_input_area: Rect,
    /// Snapshot of discovered MCP servers (populated by `/mcp discover`).
    pub mcp_servers: Vec<McpServer>,
    /// Snapshot of LSP server descriptors (populated via `LspStatusChanged` events).
    pub lsp_servers: Vec<LspServer>,
    /// Handle to the running LSP manager (kept alive for the lifetime of the TUI).
    pub lsp_manager: Option<SharedLspManager>,
    /// Active LSP discovery dialog, if any.
    pub lsp_discover: Option<LspDiscoverState>,
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
    /// Whether the agent is currently processing a message.
    pub is_processing: bool,
    /// Cancellation flag shared with the processor task; set to `true` on ESC.
    pub cancel_flag: Option<Arc<AtomicBool>>,
    /// True while an automatic pre-send compaction run is active.
    pub auto_compact_in_progress: bool,
    /// Set when an auto-compaction run returns an error.
    pub auto_compact_failed: bool,
    /// User message queued while auto-compaction runs: `(text, image_paths)`.
    pub pending_send_after_compact: Option<(String, Vec<std::path::PathBuf>)>,
    /// Whether the last agent run was halted by the user (ESC).
    pub agent_halted: bool,
    /// Maps tool call IDs to their `(short_session_id, step_number)` for log/message correlation.
    /// Step number comes from EventBus, which is the single source of truth.
    pub tool_step_map: HashMap<String, (String, u32)>,
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
                /// Whether the Teams panel is visible in the sidebar.
                pub show_teams: bool,
                /// Scroll offset for the Teams panel.
                pub teams_scroll_offset: u16,
                /// Max scroll for the Teams panel.
                pub teams_max_scroll: u16,
            }impl App {
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
                        self.input_history.push(line.to_string());
                    }
                }
                // Trim to 100 entries
                if self.input_history.len() > 100 {
                    self.input_history.drain(0..(self.input_history.len() - 100));
                }
                tracing::debug!("Loaded {} history entries from {:?}", self.input_history.len(), path);
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
            let content = self.input_history.join("\n");
            std::fs::write(path, content)?;
            tracing::debug!("Saved {} history entries to {:?}", self.input_history.len(), path);
        }
        Ok(())
    }
}

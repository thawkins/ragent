//! Core shared state for the TUI application.
//!
//! This module contains the primary `App` state struct, related UI state enums,
//! and small helpers used by the TUI renderer and input handler.

use anyhow::Result;
use lru::LruCache;
use ratatui::layout::Rect;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8};

use ragent_agent::ToolVisibilityConfig;
use ragent_agent::agent::{AgentInfo, CustomAgentDef};
use ragent_agent::event::EventBus;
use ragent_agent::mcp::{McpServer, discovery::DiscoveredMcpServer};
use ragent_agent::message::Message;
use ragent_agent::permission::PermissionRequest;
use ragent_agent::provider::ProviderRegistry;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::trigger::TriggerRuntime;
use ragent_config::OtelProtocol;
use ragent_team::team::{SwarmState, TeamConfig, TeamMember};
use serde::Serialize;

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

/// Encode `arboard::ImageData` (raw RGBA pixels) as a PNG saved to a
/// securely-created temp file.
///
/// This is a backward-compatible wrapper around [`crate::clipboard::clipboard_image_to_temp`].
pub fn save_clipboard_image_to_temp(
    img_data: &arboard::ImageData<'_>,
) -> Result<std::path::PathBuf> {
    crate::clipboard::clipboard_image_to_temp(img_data)
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

/// A background shell task spawned via the `bg` tool, as displayed in the TUI.
#[derive(Debug, Clone)]
pub struct BgTaskView {
    /// Unique task identifier.
    pub id: String,
    /// Session this task belongs to.
    pub session_id: String,
    /// Shell command being executed.
    pub command: String,
    /// Current status: `running`, `completed`, `failed`, or `cancelled`.
    pub status: String,
    /// When the task was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When the task completed, if finished.
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
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
///
/// Local/keyless providers are listed first so users can quickly find them,
/// followed by cloud providers in rough priority order.
pub const PROVIDER_LIST: &[(&str, &str)] = &[
    ("ollama", "Ollama (Local)"),
    ("ollama_cloud", "Ollama Cloud"),
    ("anthropic", "Anthropic (Claude)"),
    ("openai", "OpenAI (GPT)"),
    ("gemini", "Google Gemini"),
    ("xai", "xAI (Grok)"),
    ("huggingface", "Hugging Face"),
    ("generic_openai", "Generic OpenAI API"),
    ("azure_foundry", "Azure AI Foundry"),
    ("azure_resource", "Azure Resource (File)"),
    ("bedrock", "Amazon Bedrock"),
    ("copilot", "GitHub Copilot"),
    ("router", "Model Router"),
];

/// Entry in the model picker with full metadata for table display.
#[derive(Debug, Clone, PartialEq, Serialize)]
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
    /// Supported thinking levels for this model.
    pub thinking_levels: Vec<ragent_types::ThinkingLevel>,
    /// Effective default thinking configuration for this model after config overlays.
    pub thinking_config: Option<ragent_types::ThinkingConfig>,
    /// Cost tier label (e.g., "Free", "Low", "Medium", "High", "Premium").
    /// For Copilot, this shows the premium request tier based on multiplier.
    pub cost_tier: String,
    /// Cost multiplier relative to baseline (e.g., "0x", "1x", "3x", "10x").
    /// For Copilot, this is the premium request multiplier from GitHub docs.
    /// For other providers, this is relative to the least expensive model.
    pub cost_multiplier: String,
}

/// Spinner state shown while a provider is fetching its model list.
#[derive(Debug, Clone)]
pub struct ModelLoadingState {
    /// Provider identifier.
    pub provider_id: String,
    /// Human-readable provider name.
    pub provider_name: String,
    /// When the loading started (for spinner animation).
    pub started_at: std::time::Instant,
}

/// Progress state shown while a provider is downloading a model.
#[derive(Debug, Clone)]
pub struct ModelDownloadState {
    /// Provider identifier.
    pub provider_id: String,
    /// Model identifier being downloaded.
    pub model_id: String,
    /// Current download progress (0.0–100.0).
    pub percent: f32,
    /// When the download started (for elapsed time display).
    pub started_at: std::time::Instant,
}

/// Which product is running a GitHub OAuth device flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceFlowKind {
    /// GitHub Copilot provider setup.
    Copilot,
    /// GitHub VCS tools setup (`/github login`).
    GitHub,
}

/// State of the interactive provider-setup dialog.
#[derive(Debug, Clone)]
pub enum ProviderSetupStep {
    /// Choosing which provider to configure.
    SelectProvider {
        /// Index of the highlighted provider in [`PROVIDER_LIST`].
        selected: usize,
        /// When `true` (invoked by `/provider`) the API-key prompt is always
        /// shown after selecting a provider, even when a key is already
        /// registered, so the user can edit it. When `false` (invoked by
        /// `/model`) selecting an already-configured provider skips straight
        /// to the model list.
        force_key_entry: bool,
    },
    /// Entering an API key for the chosen provider.
    EnterKey {
        /// The provider id (e.g. "anthropic").
        provider_id: String,
        /// Human-readable display name.
        provider_name: String,
        /// The key text entered so far.
        key_field: crate::input_field::InputField,
        /// Optional API base URL (used by Generic OpenAI API provider).
        endpoint_field: crate::input_field::InputField,
        /// Which field is currently focused (0 = key, 1 = endpoint).
        active_field: u8,
        /// Optional error message from a previous attempt.
        error: Option<String>,
    },
    /// Waiting for the user to complete GitHub OAuth device flow authorisation.
    DeviceFlowPending {
        /// Which product flow this dialog represents.
        flow: DeviceFlowKind,
        /// Short code the user enters at the verification URL.
        user_code: String,
        /// URL the user must visit (e.g. `https://github.com/login/device`).
        verification_uri: String,
    },
    /// Choosing an Azure deployment from the user's azureresources.json file.
    SelectAzureResource {
        /// Parsed entries from the file.
        entries: Vec<ragent_agent::provider::azure_resource::AzureResourceEntry>,
        /// Index of the highlighted entry.
        selected: usize,
        /// Optional error message (e.g. file not found).
        error: Option<String>,
    },
    /// Loading the model list for a provider (shows a spinner popup).
    LoadingModels {
        /// The provider id (e.g. "anthropic").
        provider_id: String,
        /// Human-readable provider display name.
        provider_name: String,
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
    /// Choosing which thinking level to use for the selected model.
    SelectThinkingLevel {
        /// The provider id (e.g. "anthropic").
        provider_id: String,
        /// Human-readable provider display name.
        provider_name: String,
        /// The selected model entry.
        model: ModelPickerEntry,
        /// Index of the highlighted thinking level.
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
    /// Choosing which already-configured provider to switch to.
    ///
    /// Distinct from [`SelectProvider`] which lists *all* providers for first-time
    /// setup. This variant lists only those that have usable credentials, and is
    /// used by the `/model` slash-command flow.
    SelectConfiguredProvider {
        /// Configurable providers that have usable credentials.
        providers: Vec<ConfiguredProvider>,
        /// Index of the highlighted provider.
        selected: usize,
    },
    /// Choosing which provider to show configuration for.
    ShowProviderConfig {
        /// Configurable providers that have usable credentials.
        providers: Vec<ConfiguredProvider>,
        /// Index of the highlighted provider.
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
    // ── Router (Model Router) setup steps ─────────────────────────────
    /// Configure the router virtual provider cluster: provider multi-selection
    /// and tier-bucket assignment (FR-003).
    SetupRouter {
        /// Configured concrete providers that can feed the router buckets.
        providers: Vec<ConfiguredProvider>,
        /// IDs of providers that have been selected for the cluster palette.
        selected_provider_ids: Vec<String>,
        /// Currently highlighted provider in the multi-selection list.
        selected_provider_index: usize,
        /// In-memory draft [`RouterConfig`] being edited.
        draft_config: ragent_llm::providers::router_config::RouterConfig,
        /// Which of the four buckets is active when the right pane is focused.
        active_bucket: ragent_llm::providers::router_config::Tier,
        /// Which item is active in the active bucket (0 = first model).
        active_bucket_index: usize,
        /// Whether the left provider pane (true) or right bucket pane (false)
        /// has input focus.
        left_pane_focused: bool,
        /// Optional error or validation message shown in the panel footer.
        error: Option<String>,
    },
    /// Model picker shown after selecting a provider in the router setup flow.
    /// The chosen model is assigned to the currently selected bucket.
    SelectRouterModel {
        /// Provider being configured for the active bucket.
        provider_id: String,
        /// Provider display name.
        provider_name: String,
        /// Available models with full metadata.
        models: Vec<ModelPickerEntry>,
        /// Index of the highlighted model.
        selected: usize,
        /// Which tier bucket the model will be assigned to.
        target_tier: ragent_llm::providers::router_config::Tier,
    },
    // ── Telemetry setup step ─────────────────────────────────────────
    /// Multi-field telemetry (OpenTelemetry) configuration: endpoint, protocol,
    /// export interval, timeout, and internal Prometheus port.
    ///
    /// Tab cycles between fields; Enter validates and saves.
    TelemetrySetup {
        /// OTLP endpoint URL.
        endpoint_field: crate::input_field::InputField,
        /// Transport protocol (HTTP or gRPC).
        protocol: OtelProtocol,
        /// Metric export interval in seconds.
        interval_field: crate::input_field::InputField,
        /// Per-export request timeout in seconds.
        timeout_field: crate::input_field::InputField,
        /// Optional internal Prometheus port (`""` means disabled).
        port_field: crate::input_field::InputField,
        /// Currently focused field index (0..=4).
        active_field: u8,
        /// Optional validation or save error message.
        error: Option<String>,
    },
}

/// Information about a configured provider.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        trigger: "bench",
        description: "Benchmark runner: /bench list|init <suite-or-all-or-full>|show|run <target>|status|open last|cancel",
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
        trigger: "config",
        description: "Show application paths and configuration files: /config show",
    },
    SlashCommandDef {
        trigger: "context",
        description: "Manage context cache: /context refresh",
    },
    SlashCommandDef {
        trigger: "cron",
        description: "Schedule agent runs: /cron add <cronname> <agent> <schedule> \"<prompt>\"|remove|enable|disable|list|log|help",
    },
    SlashCommandDef {
        trigger: "compact",
        description: "Summarise and compact the conversation history",
    },
    SlashCommandDef {
        trigger: "inbox",
        description: "Triage inbox: /inbox list|claim <id>|dismiss <id>|clear|help",
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
        trigger: "profile",
        description: "Toggle the agent-loop profiler panel (/profile on|off)",
    },
    SlashCommandDef {
        trigger: "perf",
        description: "Alias for /profile — toggle the agent-loop perf panel (/perf on|off)",
    },
    SlashCommandDef {
        trigger: "llmstats",
        description: "Show average LLM response time and token throughput",
    },
    SlashCommandDef {
        trigger: "model",
        description: "Switch the active model, or show metadata with /model show",
    },
    SlashCommandDef {
        trigger: "thinking",
        description: "Switch the current thinking level: /thinking auto|off|low|medium|high",
    },
    SlashCommandDef {
        trigger: "provider",
        description: "Change provider, show config, or configure model router: /provider [show|router]",
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
        trigger: "template",
        description: "List and apply reusable prompt templates: /template [name] [args]",
    },
    SlashCommandDef {
        trigger: "goal",
        description: "Goal-based autonomous stop: /goal set|clear|show|test",
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
        description: "List task items for the current session (alias: /task list)",
    },
    SlashCommandDef {
        trigger: "mcp",
        description: "Show MCP server status (/mcp discover | /mcp connect <id> | /mcp disconnect <id>)",
    },
    SlashCommandDef {
        trigger: "task",
        description: "Toggle the TASKS side panel, or list/help tasks: /task [list|help]",
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
        trigger: "spec",
        description: "Specification management: /spec create|add|delete|list|search|validate|status|task|help",
    },
    SlashCommandDef {
        trigger: "research",
        description: "Research system: /research create|list|open|search|show|delete|archive",
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
        description: "Memory panel (Alt+M): /memory | /memory show | /memory init | /memory read <label> | /memory search <query>",
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
        trigger: "websearch",
        description: "Web search engine diagnostics: /websearch show | test | help",
    },
    SlashCommandDef {
        trigger: "init",
        description: "Analyse the project and write a summary, or create a default config: /init [config]",
    },
    SlashCommandDef {
        trigger: "codeindex",
        description: "Manage codebase index: /codeindex on|off|show|lang|reindex|help",
    },
    SlashCommandDef {
        trigger: "theme",
        description: "Switch theme: /theme default|high-contrast",
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
        trigger: "telemetry",
        description: "Telemetry management: /telemetry help|on|off|setup|counters",
    },
    SlashCommandDef {
        trigger: "telemetry_panel",
        description: "Toggle the telemetry side panel (Alt+O alias); use `/telemetry counters` to list values",
    },
    SlashCommandDef {
        trigger: "tools",
        description: "Toggle tool visibility: /tools [office|github|gitlab|teams|agents|plan|codeindex] [on|off]",
    },
    SlashCommandDef {
        trigger: "router",
        description: "Model router management: /router on|off|status|tiers|weights|boundaries|test|stats|reload|help",
    },
    SlashCommandDef {
        trigger: "startup",
        description: "Show startup timing breakdown for the current session",
    },
    SlashCommandDef {
        trigger: "editlog",
        description: "Edit-operation logging: /editlog on|off|status|show|analyse|clear",
    },
    SlashCommandDef {
        trigger: "actionloop",
        description: "Agent action-loop timing: /actionloop [help|clip]",
    },
    SlashCommandDef {
        trigger: "bug-report",
        description: "Generate diagnostic bug report with redacted session data (output to log/)",
    },
    SlashCommandDef {
        trigger: "triggers",
        description: "Manage trigger rules: /triggers [list|enable|disable|remove|status|help]",
    },
    SlashCommandDef {
        trigger: "undo",
        description: "Remove the last user/assistant turn pair from the conversation",
    },
    SlashCommandDef {
        trigger: "name",
        description: "Set a human-readable display name for the session: /name <display-name>",
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

/// State of the `/config list` save-picker overlay.
///
/// Lists timestamped backup files discovered in the global config `saves/`
/// subdirectory. The user navigates with `Up`/`Down`, restores the highlighted
/// entry with `Enter`, or cancels with `Esc`.
#[derive(Debug, Clone)]
pub struct ConfigSavePickerState {
    /// Backup file paths discovered in the `saves/` directory, newest first.
    pub entries: Vec<std::path::PathBuf>,
    /// Currently highlighted row (0 = top = newest).
    pub selected: usize,
    /// Scroll offset for the list (rows from the top).
    pub scroll_offset: usize,
    /// Resolved global config directory (for the title display and restore target).
    pub config_dir: std::path::PathBuf,
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
    /// Dragging the profile pane scrollbar.
    Profile,
    /// Dragging the Tasks pane scrollbar.
    Tasks,
    /// Dragging the Memory pane scrollbar.
    Memory,
    /// Dragging the Telemetry pane scrollbar.
    Telemetry,
}

/// Identifies which pane a text selection lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionPane {
    /// Selection in the messages pane.
    Messages,
    /// Selection in the log pane.
    Log,
    /// Selection in the profile pane.
    Profile,
    /// Selection in the Tasks pane.
    Tasks,
    /// Selection in the Memory pane.
    Memory,
    /// Selection in the Telemetry pane.
    Telemetry,
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
#[derive(Debug, Clone)]
pub struct QuestionRequest {
    /// Unique id for this question request.
    pub id: String,
    /// Session the question belongs to.
    pub session_id: String,
    /// Prompt shown to the user.
    pub question: String,
    /// Optional multiple-choice options.
    pub options: Vec<String>,
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
    /// When a slash command last set the status, and the value it set.
    ///
    /// After a short delay (see [`STATUS_EXPIRY_MS`]), if the status hasn't
    /// changed, it auto-transitions to `"ready"` so the indicator reflects
    /// that the system has finished the command and is ready for input.
    pub status_set_at: Option<std::time::Instant>,
    /// Snapshot of the status value recorded in [`App::status_set_at`].
    ///
    /// The expiry only fires when the current status still matches this
    /// snapshot — if anything else changed the status in the meantime the
    /// timer is silently cleared without overwriting the new status.
    pub status_snapshot: String,
    /// Queue of pending permission requests awaiting user resolution.
    /// The front of the queue is the currently displayed dialog; subsequent
    /// requests are shown one-at-a-time as earlier ones are resolved.
    pub permission_queue: VecDeque<PermissionRequest>,
    /// Queue of pending direct question requests awaiting user answers.
    pub question_queue: VecDeque<QuestionRequest>,
    /// Text typed by the user in response to the active question dialog.
    pub pending_question_input: String,
    /// Selected index for multiple-choice question dialogs.
    pub question_selected_index: usize,
    /// Cumulative (input, output) token counts.
    pub token_usage: (u64, u64),
    /// Completed LLM request samples used to compute `/llmstats`.
    pub llm_request_stats: Vec<LlmRequestStat>,
    /// Input token count from the most recent LLM request (used for context-window % display).
    pub last_input_tokens: u64,
    /// Bytes received from the current LLM streaming response (reset per request).
    pub stream_in_bytes: u64,
    /// Bytes sent in the current LLM request payload (reset per request).
    pub stream_out_bytes: u64,
    /// Latest quota usage percentage from provider rate-limit headers (0.0–100.0).
    /// `None` if the provider has not returned rate-limit information yet.
    pub quota_percent: Option<f32>,
    /// Active provider model-list loading state, if any (spinner popup).
    pub model_loading_state: Option<ModelLoadingState>,
    /// Active model download state, if any (progress bar popup).
    pub model_download_state: Option<ModelDownloadState>,
    /// Current persisted tool-family visibility switches.
    pub tool_visibility: ToolVisibilityConfig,
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
    /// Persisted thinking level for the currently selected model.
    pub selected_thinking_level: Option<ragent_types::ThinkingLevel>,
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
    /// Optional spec manager for reading and updating specifications.
    /// Set when the user activates a spec via /spec activate.
    pub spec_manager: Option<Arc<ragent_specs::SpecManager>>,
    /// Startup timing measurements collected during main() and run_tui().
    pub startup_timings: Option<ragent_agent::StartupTimings>,
    /// Currently active spec ID for context injection.
    pub active_spec: Option<String>,
    /// Whether to show hidden files in the file menu.
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
    /// Whether the realtime profiling panel is visible.
    pub show_profile: bool,
    /// Whether the TODO panel is visible.
    pub show_tasks_panel: bool,
    /// Whether the Memory panel is visible (toggled via Alt+M).
    pub show_memory: bool,
    /// Whether the Telemetry panel is visible (toggled via Alt+O).
    pub show_telemetry: bool,
    /// Log entries displayed in the log panel.
    pub log_entries: Vec<LogEntry>,
    /// Optional file path used to spool log-panel contents when it is visible.
    pub log_window_path: Option<std::path::PathBuf>,
    /// Scroll offset for the log panel (lines from bottom).
    pub log_scroll_offset: u16,
    /// Scroll offset for the profile panel (lines from bottom).
    pub profile_scroll_offset: u16,
    /// Scroll offset for the TODO panel (lines from top).
    pub tasks_scroll_offset: u16,
    /// Scroll offset for the Memory panel (lines from top).
    pub memory_scroll_offset: u16,
    /// Scroll offset for the Telemetry panel (lines from top).
    pub telemetry_scroll_offset: u16,
    /// Cached area of the messages pane (set during render for mouse hit-testing).
    pub message_area: Rect,
    /// Cached area of the log panel (set during render for mouse hit-testing).
    pub log_area: Rect,
    /// Cached area of the profiler panel.
    pub profile_area: Rect,
    /// Cached area of the TODO panel (set during render for mouse hit-testing).
    pub tasks_area: Rect,
    /// Cached area of the Memory panel (set during render for mouse hit-testing).
    pub memory_area: Rect,
    /// Cached area of the Telemetry panel (set during render for mouse hit-testing).
    pub telemetry_area: Rect,
    /// Maximum scroll value for the messages pane (set during render).
    pub message_max_scroll: u16,
    /// Maximum scroll value for the log pane (set during render).
    pub log_max_scroll: u16,
    /// Maximum scroll value for the profile pane (set during render).
    pub profile_max_scroll: u16,
    /// Maximum scroll value for the TODO pane (set during render).
    pub tasks_max_scroll: u16,
    /// Maximum scroll value for the Memory pane (set during render).
    pub memory_max_scroll: u16,
    /// Maximum scroll value for the Telemetry pane (set during render).
    pub telemetry_max_scroll: u16,
    /// Scroll offset for the active-agents subpanel (lines from top).
    pub active_agents_scroll_offset: u16,
    /// Maximum scroll value for the active-agents subpanel (set during render).
    pub active_agents_max_scroll: u16,
    /// Cached area of the active-agents subpanel.
    pub active_agents_area: Rect,
    /// Per-row click targets for Play/Stop buttons in the agents dialog.
    pub agent_row_button_areas: Vec<Rect>,
    /// Parallel task IDs for the agent button click targets.
    pub agent_row_button_task_ids: Vec<String>,
    /// Per-row click targets for Kill buttons in the agents dialog.
    pub agent_row_kill_areas: Vec<Rect>,
    /// Parallel task IDs for the agent kill click targets.
    pub agent_row_kill_task_ids: Vec<String>,
    /// Active scrollbar drag, if any.
    pub scrollbar_drag: Option<ScrollbarDragPane>,
    /// Active text selection, if any.
    pub text_selection: Option<TextSelection>,
    /// Plain-text lines from the last message pane render (for copy).
    pub message_content_lines: Vec<String>,
    /// Plain-text lines from the last log pane render (for copy).
    pub log_content_lines: Vec<String>,
    /// Plain-text lines from the last profile pane render (for copy).
    pub profile_content_lines: Vec<String>,
    /// Plain-text lines from the last TODO pane render (for copy).
    pub tasks_content_lines: Vec<String>,
    /// Plain-text lines from the last Memory pane render (for copy).
    pub memory_content_lines: Vec<String>,
    /// Plain-text lines from the last Telemetry pane render (for copy).
    pub telemetry_content_lines: Vec<String>,
    /// Cached area of the chat-screen input widget (set during render).
    pub input_area: Rect,
    /// Cached area of the teams subpanel.
    pub teams_area: Rect,
    /// Cached area of the output overlay.
    pub output_view_area: Rect,
    /// Cached area of the research markdown viewer overlay.
    pub research_view_area: Rect,
    /// Cached area of the Agents button beside chat input.
    pub agents_button_area: Rect,
    /// Cached area of the Teams button beside chat input.
    pub teams_button_area: Rect,
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
    /// True while a `/compress` compression pipeline is running.
    pub compress_in_progress: bool,
    /// Set when an auto-compaction run returns an error.
    pub auto_compact_failed: bool,
    /// Path to the SQLite storage database.
    pub db_path: std::path::PathBuf,
    /// User message queued while auto-compaction runs: `(text, image_paths)`.
    pub pending_send_after_compact: Option<(String, Vec<std::path::PathBuf>)>,
    /// Whether the last agent run was halted by the user (ESC).
    pub agent_halted: bool,
    /// Maps tool call IDs to their `(short_session_id, step_number, sub_step)` for log/message correlation.    /// Step number comes from EventBus; sub_step is per-tool-call within a step.
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
    pub active_tasks: Vec<ragent_agent::task::TaskEntry>,
    /// Active background shell tasks spawned via the `bg` tool (M3).
    pub bg_tasks: Vec<BgTaskView>,
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
    /// Active config-save picker dialog (`/config list`), if any.
    pub config_save_picker: Option<ConfigSavePickerState>,
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
    /// Per-row click targets for Play/Stop buttons in the teams dialog.
    pub team_row_button_areas: Vec<Rect>,
    /// Parallel agent IDs for the team button click targets.
    pub team_row_button_agent_ids: Vec<String>,
    /// Per-row click targets for Kill buttons in the teams dialog.
    pub team_row_kill_areas: Vec<Rect>,
    /// Parallel agent IDs for the team kill click targets.
    pub team_row_kill_agent_ids: Vec<String>,
    /// Currently focused teammate (agent_id). When set, the status
    /// bar shows a focus indicator and the input box routes messages
    /// to this teammate's mailbox instead of the lead session.
    pub focused_teammate: Option<String>,
    /// Active swarm state (if a /swarm is running).
    pub swarm_state: Option<SwarmState>,
    /// Pending result from an async `/swarm` LLM decomposition call.
    pub swarm_result: Arc<std::sync::Mutex<Option<Result<String, String>>>>,
    /// Pending result from a background `/bench run`.
    pub bench_result: Arc<std::sync::Mutex<Option<Result<ragent_bench::BenchRunOutcome, String>>>>,
    /// Active output overlay state.
    pub output_view: Option<OutputViewState>,
    /// Active `/research open` markdown viewer overlay state.
    pub research_view: Option<ResearchViewState>,
    /// Active benchmark task ID.
    pub active_bench_task_id: Option<String>,
    /// Human-readable summary for the active benchmark task.
    pub active_bench_summary: Option<String>,
    /// UTC timestamp when the active benchmark task started.
    pub active_bench_started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Cancellation flag for the active benchmark run.
    pub active_bench_cancel: Option<Arc<AtomicBool>>,
    /// Shared progress snapshot for the active benchmark run.
    pub active_bench_progress: Option<ragent_bench::BenchProgressHandle>,
    /// Last benchmark status summary.
    pub bench_last_summary: Option<String>,
    /// Last workbook paths produced by `/bench run`.
    pub bench_last_workbooks: Vec<std::path::PathBuf>,
    /// UTC timestamp for the most recent completed benchmark run.
    pub bench_last_finished_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Mock benchmark outputs used by tests to avoid live provider calls.
    pub bench_mock_outputs: Option<Vec<String>>,
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
    /// agent_complete is called, limits are hit, or the user runs /autopilot off.
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
    /// Set to `true` inside a single event-loop wake once an autopilot
    /// continuation has been dispatched, so multiple events arriving in the
    /// same drain cannot re-enter `dispatch_user_message`.
    pub autopilot_continued_this_wake: bool,
    /// Timestamp of the last `TaskCompleted` event received for this session.
    /// Used to suppress the autopilot auto-continue when the agent already
    /// signalled completion during the current turn.
    pub last_task_completed_at: Option<std::time::Instant>,

    // ── /spec impl sequential driver ────────────────────────────────────────
    /// Active `/spec impl` run, if any. Drives tasks one at a time: after each
    /// agent turn ends, the TUI checks the just-run task's status and, if
    /// completed, dispatches the next task's prompt.
    pub spec_impl_state: Option<SpecImplState>,

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

    // ── Memory status (M7-T3) ─────────────────────────────────────────────────
    /// Cached count of structured memories (SQLite).
    pub memory_entry_count: u64,
    /// Atomic cache updated by the off-thread refresh query.
    /// The TUI main loop copies this into `memory_entry_count` once per tick.
    pub memory_entry_count_pending: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Timestamp of the last memory update event (for relative time display).
    pub memory_last_updated: Option<std::time::Instant>,
    /// When the cached memory stats were last refreshed.
    pub memory_stats_last_refresh: std::time::Instant,
    /// When swarm unblock was last polled (debounces filesystem I/O).
    pub swarm_unblock_last_poll: std::time::Instant,
    /// When swarm completion was last polled (debounces filesystem I/O).
    pub swarm_completion_last_poll: std::time::Instant,
    /// Current theme mode (default or high-contrast for accessibility)
    pub theme_mode: crate::theme::ThemeMode,
    /// Whether mouse input is enabled (default: true). Set to false for
    /// keyboard-only accessibility mode.
    pub mouse_enabled: bool,
    /// Status message history for tracking recent status messages
    pub status_history: StatusHistory,
    /// Paths of configuration files that were loaded at startup (displayed in message window).
    pub config_paths: Vec<std::path::PathBuf>,

    // ── Router status (FR-044–FR-049) ────────────────────────────────────────
    /// Whether the router provider is the active provider and routing is enabled.
    pub router_enabled: bool,
    /// The last tier selected by the router for the most recent request.
    /// `None` when no request has been routed yet or the router is not active.
    pub router_current_tier: Option<String>,
    /// The last downstream model selected by the router (format "provider:model"),
    /// for the most recent request.
    /// `None` when no request has been routed yet or the router is not active.
    pub router_current_model: Option<String>,
    /// Stashed router draft configuration used to preserve cluster edits while
    /// a sub-dialog (e.g. the model picker) is open.
    pub router_draft_config: Option<ragent_llm::providers::router_config::RouterConfig>,
    /// Stashed provider list used to repopulate the router setup left pane after
    /// returning from the model picker sub-dialog.
    pub router_draft_providers: Vec<ConfiguredProvider>,
    /// Stashed selected provider IDs used to preserve the router setup palette
    /// after returning from the model picker sub-dialog.
    pub router_draft_selected_ids: Vec<String>,
    /// Pending confirmation for saving the current router draft configuration.
    pub pending_router_save: Option<ragent_llm::providers::router_config::RouterConfig>,

    // ── Research progress (`/research create`) ───────────────────────────────
    /// Live progress trackers for all running/completed `/research create`
    /// runs. Each `/research create` invocation pushes a new tracker here so
    /// the results of older research runs remain visible in the message
    /// window instead of being overwritten by the latest run.
    pub research_progress: Vec<crate::research_progress::ResearchProgress>,

    // ── Skill-registry cache (slash autocomplete hot path) ──────────────────
    /// Cached skill registry, lazily populated by [`App::skill_registry`].
    pub skill_registry_cache: Option<ragent_agent::skill::SkillRegistry>,
    /// `skill_dirs` from the last config load, paired with the cache above.
    pub skill_dirs_cache: Vec<String>,
    /// When the skill-registry cache was last refreshed from disk.
    pub skill_registry_last_refresh: std::time::Instant,

    // ── Run-cost summary banner (FR-012) ────────────────────────────────────
    /// Transient one-line run-complete banner shown after an agent run ends.
    ///
    /// Populated from `Event::RunCostSummary` and dismissed on the next
    /// keypress or after `RUN_COST_BANNER_EXPIRY_SECS` seconds. The full
    /// summary is always logged to the log panel regardless of this banner's
    /// visibility.
    pub run_cost_banner: Option<String>,
    /// When the run-cost banner was first shown, for auto-dismissal.
    pub run_cost_banner_at: Option<std::time::Instant>,

    // ── Trigger runtime (FR-002, FR-003) ───────────────────────────────────
    /// Shared trigger runtime for dynamic trigger rules and MCP notification
    /// push events. `None` until the trigger system is initialised (which
    /// happens lazily when the first session is created or when the user
    /// issues `/triggers`).
    pub trigger_runtime: Option<TriggerRuntime>,
}

/// State for the `/research open` markdown viewer overlay.
#[derive(Debug, Clone)]
pub struct ResearchViewState {
    /// Research item name displayed in the title.
    pub name: String,
    /// Absolute path to the RESEARCH.md file (for resolving relative images).
    pub path: std::path::PathBuf,
    /// Base directory for resolving relative image/link paths.
    pub base_dir: std::path::PathBuf,
    /// Full markdown text (frontmatter stripped).
    pub markdown: String,
    /// Vertical scroll offset from top.
    pub scroll_offset: u16,
    /// Maximum scroll value computed during render.
    pub max_scroll: u16,
}

/// State held while waiting for the user to approve or reject a plan.
#[derive(Debug, Clone)]
pub struct PlanApprovalState {
    /// The plan text produced by the plan agent.
    pub plan_text: String,
    /// Whether the dialog cursor is on Approve (true) or Reject (false).
    pub cursor_approve: bool,
}

/// State for a `/spec impl` run that drives tasks one at a time.
///
/// After each agent turn ends, the TUI checks the just-run task's status via
/// `SpecManager` and, if it is `Completed`, dispatches the next task's prompt.
/// If the task is not completed, the run stops and the user can re-run
/// `/spec impl` to resume.
#[derive(Debug, Clone)]
pub struct SpecImplState {
    /// The spec ID being implemented.
    pub spec_id: String,
    /// Root directory of the specs folder (`<cwd>/specs`).
    pub specs_root: std::path::PathBuf,
    /// Task IDs in execution order (after resume filtering).
    pub task_ids: Vec<String>,
    /// 1-based rank of the task currently being worked on.
    pub current_rank: usize,
    /// Total number of tasks to execute (`task_ids.len()`).
    pub total: usize,
    /// Snapshot of the runner used to build per-task prompts. Kept so the
    /// sequential driver can advance through the original execution order
    /// without rebuilding the runner after every task.
    pub runner: ragent_specs::SpecImplRunner,
    /// Mapping from milestone name to the parent session task ID created for it.
    pub milestone_parent_tasks: std::collections::HashMap<String, String>,
    /// Mapping from spec task ID to the session subtask ID created for it.
    pub spec_task_to_session_task: std::collections::HashMap<String, String>,
    /// Mapping from session subtask ID back to milestone name.
    pub session_task_to_milestone: std::collections::HashMap<String, String>,
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

    /// Arm the status auto-expiry timer for the current status value.
    ///
    /// Called after a slash command completes (synchronously or when an
    /// async slash-command poll produces its final status). Records the
    /// current status and the instant it was set so [`App::poll_status_expiry`]
    /// can transition it to `"ready"` once the grace period elapses — but only
    /// if nothing else changed the status in the meantime.
    ///
    /// No-op for statuses that should persist: async-in-progress (`⏳`) and
    /// error/warning (`⚠`) states are left untouched.
    pub fn arm_status_expiry(&mut self) {
        // Never auto-clear async-in-progress or error states — those need to
        // stay visible until their own completion handler updates them.
        if self.status.starts_with('⏳') || self.status.starts_with('⚠') {
            return;
        }
        // "ready" is already the idle state — nothing to transition to.
        if self.status.eq_ignore_ascii_case("ready") {
            return;
        }
        self.status_snapshot = self.status.clone();
        self.status_set_at = Some(std::time::Instant::now());
    }

    /// Poll the status auto-expiry timer and transition to `"ready"` if the
    /// grace period has elapsed and the status is unchanged.
    ///
    /// Called from the TUI main loop (~50 ms cadence). If the recorded status
    /// snapshot still matches the current status and the delay has passed, the
    /// status is set to `"ready"`. If the status changed since the timer was
    /// armed, the timer is silently cleared without overwriting the new status.
    pub fn poll_status_expiry(&mut self) {
        let Some(set_at) = self.status_set_at else {
            return;
        };
        // Clear the timer first — either we transition below, or the status
        // changed and we no longer own it.
        self.status_set_at = None;
        let snapshot = std::mem::take(&mut self.status_snapshot);

        if std::time::Instant::now().duration_since(set_at)
            < std::time::Duration::from_millis(STATUS_EXPIRY_MS)
        {
            // Not enough time has passed — re-arm for the next poll.
            self.status_set_at = Some(set_at);
            self.status_snapshot = snapshot;
            return;
        }

        // Only transition to "ready" if the status hasn't been changed by
        // something else (e.g. agent processing started and set "busy").
        if self.status == snapshot && !self.status.is_empty() {
            self.status = "ready".to_string();
            self.needs_redraw = true;
        }
    }

    /// Auto-dismiss the transient run-cost banner after
    /// `RUN_COST_BANNER_EXPIRY_SECS` seconds so it does not obstruct the
    /// status bar during long unattended runs (e.g. `/spec impl`).
    ///
    /// Called from the TUI main loop (~50 ms cadence).
    pub fn poll_run_cost_banner_expiry(&mut self) {
        if let Some(shown_at) = self.run_cost_banner_at {
            if shown_at.elapsed() >= std::time::Duration::from_secs(RUN_COST_BANNER_EXPIRY_SECS) {
                self.run_cost_banner = None;
                self.run_cost_banner_at = None;
                self.needs_redraw = true;
            }
        }
    }
}

/// Auto-dismiss timeout (in seconds) for the transient run-cost banner.
const RUN_COST_BANNER_EXPIRY_SECS: u64 = 15;

/// Grace period (in milliseconds) before a slash-command status auto-clears to
/// `"ready"`. Long enough to read the status, short enough to feel responsive.
pub const STATUS_EXPIRY_MS: u64 = 2000;

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

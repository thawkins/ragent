//! Application state and event handling for the TUI.
//!
//! The [`App`] struct holds the current session, message history, input buffer,
//! scroll position, and permission state. It processes both terminal key events
//! and agent bus events to drive the UI.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use ragent_core::{
    agent::{AgentInfo, ModelRef},
    event::{Event, EventBus},
    message::{Message, MessagePart, Role},
    permission::PermissionRequest,
    provider::ProviderRegistry,
    session::processor::SessionProcessor,
    storage::Storage,
};

use crate::input::{self, InputAction};
use crate::tips;

/// Severity level for a log entry.
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
        /// The provider id (e.g. `"anthropic"`).
        provider_id: String,
        /// Human-readable display name.
        provider_name: String,
        /// The key text entered so far.
        key_input: String,
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
        /// The provider id (e.g. `"anthropic"`).
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
        /// Available agent names and descriptions.
        agents: Vec<(String, String)>,
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
    /// Provider identifier (e.g. `"anthropic"`).
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
        trigger: "agent",
        description: "Switch the active agent",
    },
    SlashCommandDef {
        trigger: "clear",
        description: "Clear message history for the current session",
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
        trigger: "system",
        description: "Override the agent system prompt (/system <prompt>)",
    },
];

/// State of the slash-command autocomplete menu.
#[derive(Debug, Clone)]
pub struct SlashMenuState {
    /// Indices into [`SLASH_COMMANDS`] that match the current filter.
    pub matches: Vec<usize>,
    /// Currently highlighted index within `matches`.
    pub selected: usize,
    /// The filter text typed after `/` (e.g. `"mo"` for `/mo`).
    pub filter: String,
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
    /// Previously submitted input lines (oldest first).
    pub input_history: Vec<String>,
    /// Current position when navigating history (`None` = new input).
    pub history_index: Option<usize>,
    /// Saved in-progress input while browsing history.
    pub history_draft: String,
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
}

impl App {
    /// Create a new [`App`] with default state and the given event bus.
    pub fn new(
        event_bus: Arc<EventBus>,
        storage: Arc<Storage>,
        provider_registry: Arc<ProviderRegistry>,
        session_processor: Arc<SessionProcessor>,
        agent_info: AgentInfo,
        show_log: bool,
    ) -> Self {
        let cwd = std::env::current_dir()
            .map(|p| {
                let path = p.display().to_string();
                if let Some(home) = std::env::var_os("HOME") {
                    let home = home.to_string_lossy();
                    if let Some(rest) = path.strip_prefix(home.as_ref()) {
                        return format!("~{rest}");
                    }
                }
                path
            })
            .unwrap_or_default();

        let git_branch = Self::detect_git_branch();

        let configured_provider = Self::detect_provider(&storage);
        let agent_name = agent_info.name.clone();

        let cycleable_agents: Vec<AgentInfo> = ragent_core::agent::create_builtin_agents()
            .into_iter()
            .filter(|a| !a.hidden)
            .collect();
        let current_agent_index = cycleable_agents
            .iter()
            .position(|a| a.name == agent_info.name)
            .unwrap_or(0);

        // Load persisted model selection
        let selected_model = storage.get_setting("selected_model").ok().flatten();

        Self {
            messages: Vec::new(),
            input: String::new(),
            scroll_offset: 0,
            is_running: true,
            event_bus,
            storage,
            session_id: None,
            agent_name,
            status: "ready".to_string(),
            permission_pending: None,
            token_usage: (0, 0),
            current_screen: ScreenMode::Home,
            tip: tips::random_tip(),
            cwd,
            git_branch,
            provider_setup: None,
            configured_provider,
            provider_registry,
            selected_model,
            session_processor,
            agent_info,
            cycleable_agents,
            current_agent_index,
            provider_health: Arc::new(AtomicU8::new(0)),
            slash_menu: None,
            input_history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            show_log,
            log_entries: Vec::new(),
            log_scroll_offset: 0,
            message_area: Rect::default(),
            log_area: Rect::default(),
        }
    }

    /// Detect the first configured provider by checking env vars and the database.
    pub fn detect_provider(storage: &Storage) -> Option<ConfiguredProvider> {
        // Helper: returns true when the user has explicitly reset this provider.
        let is_disabled = |pid: &str| -> bool {
            storage
                .get_setting(&format!("provider_{pid}_disabled"))
                .ok()
                .flatten()
                .is_some()
        };

        // Check Anthropic
        if !is_disabled("anthropic") {
            if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
                if !key.is_empty() {
                    return Some(ConfiguredProvider {
                        id: "anthropic".into(),
                        name: "Anthropic (Claude)".into(),
                        source: ProviderSource::EnvVar,
                    });
                }
            }
        }
        // Check OpenAI
        if !is_disabled("openai") {
            if let Ok(key) = std::env::var("OPENAI_API_KEY") {
                if !key.is_empty() {
                    return Some(ConfiguredProvider {
                        id: "openai".into(),
                        name: "OpenAI (GPT)".into(),
                        source: ProviderSource::EnvVar,
                    });
                }
            }
        }
        // Check Copilot env var
        if !is_disabled("copilot") {
            if let Ok(key) = std::env::var("GITHUB_COPILOT_TOKEN") {
                if !key.is_empty() {
                    return Some(ConfiguredProvider {
                        id: "copilot".into(),
                        name: "GitHub Copilot".into(),
                        source: ProviderSource::EnvVar,
                    });
                }
            }
            // Check Copilot auto-discover (IDE config)
            if ragent_core::provider::copilot::find_copilot_token().is_some() {
                return Some(ConfiguredProvider {
                    id: "copilot".into(),
                    name: "GitHub Copilot".into(),
                    source: ProviderSource::AutoDiscovered,
                });
            }
            // Check Copilot via gh CLI
            if ragent_core::provider::copilot::find_gh_cli_token().is_some() {
                return Some(ConfiguredProvider {
                    id: "copilot".into(),
                    name: "GitHub Copilot".into(),
                    source: ProviderSource::AutoDiscovered,
                });
            }
        }
        // Check Ollama (always available locally)
        if !is_disabled("ollama") {
            if let Ok(host) = std::env::var("OLLAMA_HOST") {
                if !host.is_empty() {
                    return Some(ConfiguredProvider {
                        id: "ollama".into(),
                        name: "Ollama (Local)".into(),
                        source: ProviderSource::EnvVar,
                    });
                }
            }
        }

        // Check database for any stored provider auth
        for (pid, pname) in PROVIDER_LIST {
            if is_disabled(pid) {
                continue;
            }
            if let Ok(Some(_key)) = storage.get_provider_auth(pid) {
                return Some(ConfiguredProvider {
                    id: pid.to_string(),
                    name: pname.to_string(),
                    source: ProviderSource::Database,
                });
            }
        }

        None
    }

    /// Refresh the configured-provider detection (e.g. after storing a new key).
    pub fn refresh_provider(&mut self) {
        self.configured_provider = Self::detect_provider(&self.storage);
    }

    /// Load an existing session from storage and restore its state.
    ///
    /// Sets the session ID, loads all persisted messages, switches to the
    /// chat screen, and updates the status bar. Returns an error if the
    /// session is not found or the storage query fails.
    ///
    /// # Errors
    ///
    /// Returns [`anyhow::Error`] if the session ID is not found in storage
    /// or if a database query fails.
    pub fn load_session(&mut self, session_id: &str) -> anyhow::Result<()> {
        let session = self
            .storage
            .get_session(session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        let messages = self.storage.get_messages(session_id)?;
        let msg_count = messages.len();

        self.session_id = Some(session_id.to_string());
        self.messages = messages;
        self.current_screen = ScreenMode::Chat;
        self.status = format!("resumed ({} messages)", msg_count);

        // Update cwd to match the session's working directory
        if !session.directory.is_empty() {
            self.cwd = session.directory.clone();
        }

        self.push_log(
            LogLevel::Info,
            format!(
                "Resumed session {} ({} messages)",
                &session_id[..8.min(session_id.len())],
                msg_count
            ),
        );

        Ok(())
    }

    /// Detect the current git branch, if the cwd is inside a git repository.
    fn detect_git_branch() -> Option<String> {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if branch.is_empty() {
                None
            } else {
                Some(branch)
            }
        } else {
            None
        }
    }

    /// Returns the list of available models for a provider as `(id, display_name)` pairs.
    ///
    /// For Ollama, queries the running server to discover actual models.
    /// Falls back to the provider's static defaults if discovery fails.
    pub fn models_for_provider(&self, provider_id: &str) -> Vec<(String, String)> {
        if provider_id == "ollama" {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let result = tokio::task::block_in_place(|| {
                    handle.block_on(ragent_core::provider::ollama::list_ollama_models(None))
                });
                if let Ok(models) = result {
                    if !models.is_empty() {
                        return models.into_iter().map(|m| (m.id, m.name)).collect();
                    }
                }
            }
        }
        if provider_id == "copilot" {
            // Prefer DB-stored device flow token (works for token exchange),
            // then fall back to other token sources for model discovery.
            let token = self
                .storage
                .get_provider_auth("copilot")
                .ok()
                .flatten()
                .filter(|k| !k.is_empty())
                .or_else(|| {
                    let storage = self.storage.clone();
                    let db_lookup = move || -> Option<String> { None }; // already checked
                    ragent_core::provider::copilot::resolve_copilot_github_token(Some(&db_lookup))
                });
            if let Some(token) = token {
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    let result = tokio::task::block_in_place(|| {
                        handle.block_on(ragent_core::provider::copilot::list_copilot_models(&token))
                    });
                    if let Ok(models) = result {
                        if !models.is_empty() {
                            return models.into_iter().map(|m| (m.id, m.name)).collect();
                        }
                    }
                }
            }
        }
        self.provider_registry
            .get(provider_id)
            .map(|p| {
                p.default_models()
                    .into_iter()
                    .map(|m| (m.id, m.name))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns a human-readable `"provider / model"` label, or `None` if no model is selected.
    pub fn provider_model_label(&self) -> Option<String> {
        let provider_name = self.configured_provider.as_ref()?.name.clone();
        let model_str = self.selected_model.as_ref()?;
        let model_id = model_str
            .split_once('/')
            .map(|(_, m)| m)
            .unwrap_or(model_str);
        Some(format!("{} / {}", provider_name, model_id))
    }

    /// Spawn an async health check for the currently configured provider.
    ///
    /// Sets `provider_health` to `0` (checking) immediately, then spawns
    /// a background task that updates it to `1` (available) or `2` (unavailable).
    pub fn check_provider_health(&mut self) {
        let provider = match &self.configured_provider {
            Some(p) => p.clone(),
            None => {
                self.provider_health.store(0, Ordering::Relaxed);
                return;
            }
        };
        self.provider_health.store(0, Ordering::Relaxed);
        let health = self.provider_health.clone();

        // Pre-resolve the copilot token using the centralized resolver:
        // env var → IDE auto-discover → gh CLI → database.
        let copilot_token = if provider.id == "copilot" {
            let storage = self.storage.clone();
            let db_lookup = move || {
                storage
                    .get_provider_auth("copilot")
                    .ok()
                    .flatten()
                    .filter(|k| !k.is_empty())
            };
            ragent_core::provider::copilot::resolve_copilot_github_token(Some(&db_lookup))
        } else {
            None
        };

        tokio::spawn(async move {
            let available = match provider.id.as_str() {
                "ollama" => ragent_core::provider::ollama::list_ollama_models(None)
                    .await
                    .is_ok(),
                "copilot" => {
                    if let Some(token) = copilot_token {
                        ragent_core::provider::copilot::check_copilot_health(&token).await
                    } else {
                        false
                    }
                }
                // For API-key providers we trust the key is present
                _ => true,
            };

            health.store(if available { 1 } else { 2 }, Ordering::Relaxed);
        });
    }

    /// Returns the provider health status: `None` = checking, `Some(true)` = up, `Some(false)` = down.
    pub fn provider_health_status(&self) -> Option<bool> {
        match self.provider_health.load(Ordering::Relaxed) {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        }
    }

    /// Update the slash-command autocomplete menu based on the current input buffer.
    ///
    /// Shows the menu when input starts with `/`, filtering commands by the text
    /// after the slash. Closes the menu when input no longer starts with `/`.
    pub fn update_slash_menu(&mut self) {
        if let Some(filter) = self.input.strip_prefix('/') {
            let needle = filter.to_lowercase();
            let matches: Vec<usize> = SLASH_COMMANDS
                .iter()
                .enumerate()
                .filter(|(_, cmd)| {
                    needle.is_empty()
                        || cmd.trigger.starts_with(&needle)
                        || cmd.description.to_lowercase().contains(&needle)
                })
                .map(|(i, _)| i)
                .collect();

            let prev_selected = self.slash_menu.as_ref().map(|m| m.selected).unwrap_or(0);
            let selected = if matches.is_empty() {
                0
            } else {
                prev_selected.min(matches.len() - 1)
            };

            self.slash_menu = Some(SlashMenuState {
                matches,
                selected,
                filter: filter.to_string(),
            });
        } else {
            self.slash_menu = None;
        }
    }

    /// Execute a slash command by trigger name (e.g. `"/model"` or `"model"`).
    pub fn execute_slash_command(&mut self, raw: &str) {
        let stripped = raw.strip_prefix('/').unwrap_or(raw).trim();
        self.input.clear();
        self.slash_menu = None;

        // Split into command and optional argument text.
        let (cmd, args) = stripped
            .split_once(char::is_whitespace)
            .map_or((stripped, ""), |(c, a)| (c, a.trim()));

        match cmd {
            "agent" => {
                if args.is_empty() {
                    // Open the agent picker dialog
                    let agents: Vec<(String, String)> = self
                        .cycleable_agents
                        .iter()
                        .map(|a| (a.name.clone(), a.description.clone()))
                        .collect();
                    let selected = self.current_agent_index;
                    self.provider_setup =
                        Some(ProviderSetupStep::SelectAgent { agents, selected });
                } else {
                    // Direct switch: /agent <name>
                    if let Some(idx) = self
                        .cycleable_agents
                        .iter()
                        .position(|a| a.name == args)
                    {
                        let prev = self.agent_name.clone();
                        self.current_agent_index = idx;
                        self.agent_info = self.cycleable_agents[idx].clone();
                        self.agent_name = self.agent_info.name.clone();
                        self.status = format!("agent: {}", self.agent_name);
                        self.push_log(
                            LogLevel::Info,
                            format!(
                                "Switched to: {} ({})",
                                self.agent_name, self.agent_info.description
                            ),
                        );
                        if let Some(ref sid) = self.session_id {
                            self.event_bus.publish(Event::AgentSwitched {
                                session_id: sid.clone(),
                                from: prev,
                                to: self.agent_name.clone(),
                            });
                        }
                    } else {
                        let available: Vec<&str> =
                            self.cycleable_agents.iter().map(|a| a.name.as_str()).collect();
                        self.status =
                            format!("Unknown agent '{}'. Available: {}", args, available.join(", "));
                        self.push_log(LogLevel::Warn, format!("Unknown agent: {}", args));
                    }
                }
            }
            "clear" => {
                self.messages.clear();
                self.scroll_offset = 0;
                self.status = "messages cleared".to_string();
                self.push_log(LogLevel::Info, "Message history cleared".to_string());
            }
            "compact" => {
                if self.session_id.is_none() {
                    self.status = "⚠ No active session to compact".to_string();
                    return;
                }
                if self.messages.is_empty() {
                    self.status = "⚠ No messages to compact".to_string();
                    return;
                }

                let sid = self.session_id.clone().unwrap();
                let compaction_agent =
                    ragent_core::agent::resolve_agent("compaction", &Default::default())
                        .unwrap_or_else(|_| self.agent_info.clone());

                // Override model to match current selection
                let mut agent = compaction_agent;
                if let Some(ref model_str) = self.selected_model {
                    if let Some((provider, model)) = model_str.split_once('/') {
                        agent.model = Some(ModelRef {
                            provider_id: provider.to_string(),
                            model_id: model.to_string(),
                        });
                    }
                }

                // Build a summary prompt from the current messages
                let summary_prompt =
                    "Summarise the conversation so far into a concise representation that \
                     preserves all important context, decisions, code changes, file paths, \
                     and outstanding tasks. Output only the summary."
                        .to_string();

                self.status = "compacting…".to_string();
                self.push_log(LogLevel::Info, "Compaction started".to_string());

                let processor = self.session_processor.clone();
                let event_bus = self.event_bus.clone();
                tokio::spawn(async move {
                    match processor
                        .process_message(&sid, &summary_prompt, &agent)
                        .await
                    {
                        Ok(_) => {
                            tracing::info!(session_id = %sid, "Compaction completed");
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Compaction failed");
                            event_bus.publish(Event::AgentError {
                                session_id: sid,
                                error: format!("Compaction failed: {e}"),
                            });
                        }
                    }
                });
            }
            "help" => {
                let mut help_lines = String::from("Available commands:\n");
                for cmd_def in SLASH_COMMANDS {
                    help_lines.push_str(&format!("  /{:<18} {}\n", cmd_def.trigger, cmd_def.description));
                }
                self.append_assistant_text(&help_lines);
                if self.current_screen == ScreenMode::Home {
                    self.current_screen = ScreenMode::Chat;
                }
                self.status = "help".to_string();
            }
            "log" => {
                self.show_log = !self.show_log;
                self.status = if self.show_log {
                    "log panel visible".to_string()
                } else {
                    "log panel hidden".to_string()
                };
            }
            "model" => {
                if let Some(ref prov) = self.configured_provider {
                    let models = self.models_for_provider(&prov.id.clone());
                    let prov_name = prov.name.clone();
                    let prov_id = prov.id.clone();
                    self.provider_setup = Some(ProviderSetupStep::SelectModel {
                        provider_id: prov_id,
                        provider_name: prov_name,
                        models,
                        selected: 0,
                    });
                } else {
                    self.status = "⚠ No provider configured — use /provider first".to_string();
                }
            }
            "provider" => {
                self.provider_setup = Some(ProviderSetupStep::SelectProvider { selected: 0 });
            }
            "provider_reset" => {
                self.provider_setup = Some(ProviderSetupStep::ResetProvider { selected: 0 });
            }
            "quit" => {
                self.is_running = false;
            }
            "system" => {
                if args.is_empty() {
                    // Show current system prompt
                    if let Some(ref prompt) = self.agent_info.prompt {
                        self.append_assistant_text(&format!("Current system prompt:\n{prompt}"));
                        if self.current_screen == ScreenMode::Home {
                            self.current_screen = ScreenMode::Chat;
                        }
                    } else {
                        self.status = "No system prompt set".to_string();
                    }
                } else {
                    self.agent_info.prompt = Some(args.to_string());
                    self.status = "system prompt updated".to_string();
                    self.push_log(
                        LogLevel::Info,
                        format!(
                            "System prompt set ({} chars)",
                            args.len()
                        ),
                    );
                }
            }
            _ => {
                self.status = format!("Unknown command: /{}", cmd);
                self.push_log(LogLevel::Warn, format!("Unknown command: /{}", cmd));
            }
        }
    }

    /// Process a terminal mouse event (scroll wheel, etc.).
    pub fn handle_mouse_event(&mut self, event: MouseEvent) {
        match event.kind {
            MouseEventKind::ScrollUp => {
                if self.show_log && self.log_area.contains((event.column, event.row).into()) {
                    self.log_scroll_offset = self.log_scroll_offset.saturating_add(3);
                } else if self.message_area.contains((event.column, event.row).into()) {
                    self.scroll_offset = self.scroll_offset.saturating_add(3);
                }
            }
            MouseEventKind::ScrollDown => {
                if self.show_log && self.log_area.contains((event.column, event.row).into()) {
                    self.log_scroll_offset = self.log_scroll_offset.saturating_sub(3);
                } else if self.message_area.contains((event.column, event.row).into()) {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                }
            }
            _ => {}
        }
    }

    /// Process a terminal key event and execute the resulting [`InputAction`], if any.
    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if let Some(action) = input::handle_key(self, key) {
            match action {
                InputAction::SendMessage(text) => {
                    // Block sending if no provider/model is configured
                    if self.configured_provider.is_none() {
                        self.status =
                            "⚠ No provider configured — use /provider to set up".to_string();
                        return;
                    }
                    if self.selected_model.is_none() {
                        self.status = "⚠ No model selected — use /model to choose".to_string();
                        return;
                    }
                    // Transition from Home to Chat on first message
                    if self.current_screen == ScreenMode::Home {
                        self.current_screen = ScreenMode::Chat;
                    }
                    // Create session if needed
                    if self.session_id.is_none() {
                        let dir = std::env::current_dir().unwrap_or_default();
                        match self.session_processor.session_manager.create_session(dir) {
                            Ok(session) => {
                                self.session_id = Some(session.id);
                            }
                            Err(e) => {
                                self.status = format!("error: {}", e);
                                return;
                            }
                        }
                    }

                    let sid = self.session_id.clone().unwrap();
                    let msg = Message::user_text(&sid, &text);
                    self.messages.push(msg);
                    self.input_history.push(text.clone());
                    self.history_index = None;
                    self.history_draft.clear();
                    self.input.clear();
                    self.status = "processing...".to_string();

                    // Log the prompt
                    let truncated = if text.len() > 120 {
                        format!("{}…", &text[..120])
                    } else {
                        text.clone()
                    };
                    self.push_log(LogLevel::Info, format!("prompt sent: {}", truncated));

                    // Build agent with the selected model override
                    let mut agent = self.agent_info.clone();
                    if let Some(ref model_str) = self.selected_model {
                        if let Some((provider, model)) = model_str.split_once('/') {
                            agent.model = Some(ModelRef {
                                provider_id: provider.to_string(),
                                model_id: model.to_string(),
                            });
                        }
                    }

                    // Spawn async task to process the message
                    let processor = self.session_processor.clone();
                    tokio::spawn(async move {
                        if let Err(e) = processor.process_message(&sid, &text, &agent).await {
                            // Error is already surfaced via Event::AgentError;
                            // only trace at debug level to avoid duplicating
                            // output below the TUI.
                            tracing::debug!(error = %e, "Failed to process message");
                        }
                    });
                }
                InputAction::Quit => {
                    self.is_running = false;
                }
                InputAction::ScrollUp => {
                    self.scroll_offset = self.scroll_offset.saturating_add(3);
                }
                InputAction::ScrollDown => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                }
                InputAction::LogScrollUp => {
                    self.log_scroll_offset = self.log_scroll_offset.saturating_add(3);
                }
                InputAction::LogScrollDown => {
                    self.log_scroll_offset = self.log_scroll_offset.saturating_sub(3);
                }
                InputAction::HistoryUp => {
                    if self.input_history.is_empty() {
                        return;
                    }
                    match self.history_index {
                        None => {
                            self.history_draft = self.input.clone();
                            let idx = self.input_history.len() - 1;
                            self.history_index = Some(idx);
                            self.input = self.input_history[idx].clone();
                        }
                        Some(idx) if idx > 0 => {
                            let idx = idx - 1;
                            self.history_index = Some(idx);
                            self.input = self.input_history[idx].clone();
                        }
                        _ => {}
                    }
                }
                InputAction::HistoryDown => match self.history_index {
                    Some(idx) if idx + 1 < self.input_history.len() => {
                        let idx = idx + 1;
                        self.history_index = Some(idx);
                        self.input = self.input_history[idx].clone();
                    }
                    Some(_) => {
                        self.history_index = None;
                        self.input = self.history_draft.clone();
                        self.history_draft.clear();
                    }
                    None => {}
                },
                InputAction::SwitchAgent => {
                    if self.cycleable_agents.len() > 1 {
                        let prev = self.agent_name.clone();
                        self.current_agent_index =
                            (self.current_agent_index + 1) % self.cycleable_agents.len();
                        self.agent_info = self.cycleable_agents[self.current_agent_index].clone();
                        self.agent_name = self.agent_info.name.clone();
                        self.status = format!("agent: {}", self.agent_name);
                        self.push_log(
                            LogLevel::Info,
                            format!("Switched to: {} ({})", self.agent_name, self.agent_info.description),
                        );

                        if let Some(ref sid) = self.session_id {
                            self.event_bus.publish(Event::AgentSwitched {
                                session_id: sid.clone(),
                                from: prev,
                                to: self.agent_name.clone(),
                            });
                        }
                    }
                }
                InputAction::SlashCommand(cmd) => {
                    self.execute_slash_command(&cmd);
                }
            }
        }
    }

    /// Handle an [`Event`] from the agent event bus and update application state.
    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::SessionCreated { ref session_id } => {
                if self.session_id.is_none() {
                    self.session_id = Some(session_id.clone());
                    self.push_log(
                        LogLevel::Info,
                        format!(
                            "session created: {}",
                            &session_id[..8.min(session_id.len())]
                        ),
                    );
                }
            }
            Event::TextDelta {
                ref session_id,
                ref text,
            } => {
                if self.is_current_session(session_id) {
                    self.append_assistant_text(text);
                }
            }
            Event::ReasoningDelta {
                ref session_id,
                ref text,
            } => {
                if self.is_current_session(session_id) {
                    self.append_reasoning_text(text);
                }
            }
            Event::ToolCallStart {
                ref session_id,
                ref call_id,
                ref tool,
            } => {
                if self.is_current_session(session_id) {
                    self.add_tool_call_part(tool, call_id);
                    self.status = format!("running: {}", tool);
                    self.push_log(
                        LogLevel::Tool,
                        format!("tool call: {} ({})", tool, &call_id[..8.min(call_id.len())]),
                    );
                }
            }
            Event::ToolCallEnd {
                ref session_id,
                ref call_id,
                ref tool,
                ref error,
                duration_ms,
            } => {
                if self.is_current_session(session_id) {
                    self.update_tool_call_status(call_id, error.is_none());
                    if let Some(err) = error {
                        self.push_log(
                            LogLevel::Error,
                            format!("tool {} failed: {} ({}ms)", tool, err, duration_ms),
                        );
                    } else {
                        self.push_log(
                            LogLevel::Tool,
                            format!("tool {} completed ({}ms)", tool, duration_ms),
                        );
                    }
                }
            }
            Event::MessageStart {
                ref session_id,
                ref message_id,
            } => {
                if self.is_current_session(session_id) {
                    self.status = "processing...".to_string();
                    self.push_log(
                        LogLevel::Info,
                        format!(
                            "response started ({})",
                            &message_id[..8.min(message_id.len())]
                        ),
                    );
                }
            }
            Event::MessageEnd {
                ref session_id,
                ref reason,
                ..
            } => {
                if self.is_current_session(session_id) {
                    self.status = "ready".to_string();
                    self.push_log(LogLevel::Info, format!("response finished ({reason:?})"));
                }
            }
            Event::PermissionRequested {
                ref session_id,
                ref request_id,
                ref permission,
                ref description,
            } => {
                if self.is_current_session(session_id) {
                    self.permission_pending = Some(PermissionRequest {
                        id: request_id.clone(),
                        session_id: session_id.clone(),
                        permission: permission.clone(),
                        patterns: vec![description.clone()],
                        metadata: serde_json::Value::Null,
                        tool_call_id: None,
                    });
                    self.status = "awaiting permission".to_string();
                    self.push_log(
                        LogLevel::Warn,
                        format!("permission requested: {} — {}", permission, description),
                    );
                }
            }
            Event::PermissionReplied {
                ref session_id,
                allowed,
                ..
            } => {
                if self.is_current_session(session_id) {
                    self.permission_pending = None;
                    self.status = "processing...".to_string();
                    self.push_log(
                        LogLevel::Info,
                        format!("permission {}", if allowed { "granted" } else { "denied" }),
                    );
                }
            }
            Event::AgentSwitched {
                ref session_id,
                ref from,
                ref to,
            } => {
                if self.is_current_session(session_id) {
                    self.agent_name = to.clone();
                    self.push_log(LogLevel::Info, format!("agent switched: {} → {}", from, to));
                }
            }
            Event::AgentError {
                ref session_id,
                ref error,
            } => {
                if self.is_current_session(session_id) {
                    // Full details go to the log panel only
                    self.push_log(LogLevel::Error, format!("agent error: {}", error));
                    // Clean summary for the status bar and chat panel
                    let summary = summarise_error(error);
                    self.status = format!("error: {}", summary);
                    self.append_assistant_text(&format!("⚠ {}", summary));
                }
            }
            Event::TokenUsage {
                ref session_id,
                input_tokens,
                output_tokens,
            } => {
                if self.is_current_session(session_id) {
                    self.token_usage.0 += input_tokens;
                    self.token_usage.1 += output_tokens;
                    self.push_log(
                        LogLevel::Info,
                        format!(
                            "tokens: +{}in +{}out (total {}in {}out)",
                            input_tokens, output_tokens, self.token_usage.0, self.token_usage.1
                        ),
                    );
                }
            }
            Event::ToolsSent {
                ref session_id,
                ref tools,
            } => {
                if self.is_current_session(session_id) {
                    self.push_log(
                        LogLevel::Info,
                        format!("tools sent: [{}]", tools.join(", ")),
                    );
                }
            }
            Event::ModelResponse {
                ref session_id,
                ref text,
            } => {
                if self.is_current_session(session_id) {
                    self.push_log(LogLevel::Info, format!("model response: {}", text));
                }
            }
            Event::ToolCallArgs {
                ref session_id,
                ref tool,
                ref args,
                ..
            } => {
                if self.is_current_session(session_id) {
                    self.push_log(LogLevel::Tool, format!("→ {}({})", tool, args));
                }
            }
            Event::ToolResult {
                ref session_id,
                ref tool,
                ref content,
                success,
                ..
            } => {
                if self.is_current_session(session_id) {
                    let icon = if success { "✓" } else { "✗" };
                    self.push_log(LogLevel::Tool, format!("← {} {} {}", tool, icon, content));
                }
            }
            _ => {}
        }

        // Handle device flow completion outside the match to avoid
        // borrow issues (we need &mut self for storage + UI updates).
        if let Event::CopilotDeviceFlowComplete {
            ref token,
            ref api_base,
        } = event
        {
            let _ = self.storage.set_provider_auth("copilot", token);

            let _ = self.storage.set_setting("copilot_api_base", api_base);
            let _ = self.storage.delete_setting("provider_copilot_disabled");
            self.push_log(
                LogLevel::Info,
                format!("Copilot authorised (api: {api_base})"),
            );
            self.refresh_provider();
            let models = self.models_for_provider("copilot");
            self.provider_setup = Some(ProviderSetupStep::SelectModel {
                provider_id: "copilot".to_string(),
                provider_name: "GitHub Copilot".to_string(),
                models,
                selected: 0,
            });
        }
    }

    /// Append a log entry to the log buffer.
    pub fn push_log(&mut self, level: LogLevel, message: String) {
        self.log_entries.push(LogEntry {
            timestamp: chrono::Utc::now(),
            level,
            message,
        });
    }

    fn is_current_session(&self, session_id: &str) -> bool {
        self.session_id.as_deref() == Some(session_id)
    }

    fn append_assistant_text(&mut self, text: &str) {
        if let Some(last) = self.messages.last_mut()
            && last.role == Role::Assistant
        {
            // Append to the last text part if it exists
            for part in last.parts.iter_mut().rev() {
                if let MessagePart::Text { text: t } = part {
                    t.push_str(text);
                    return;
                }
            }
            // No text part yet, add one
            last.parts.push(MessagePart::Text {
                text: text.to_string(),
            });
            return;
        }
        // Create new assistant message
        if let Some(ref sid) = self.session_id {
            let msg = Message::new(
                sid,
                Role::Assistant,
                vec![MessagePart::Text {
                    text: text.to_string(),
                }],
            );
            self.messages.push(msg);
        }
    }

    fn append_reasoning_text(&mut self, text: &str) {
        if let Some(last) = self.messages.last_mut()
            && last.role == Role::Assistant
        {
            for part in last.parts.iter_mut().rev() {
                if let MessagePart::Reasoning { text: t } = part {
                    t.push_str(text);
                    return;
                }
            }
            last.parts.push(MessagePart::Reasoning {
                text: text.to_string(),
            });
            return;
        }
        if let Some(ref sid) = self.session_id {
            let msg = Message::new(
                sid,
                Role::Assistant,
                vec![MessagePart::Reasoning {
                    text: text.to_string(),
                }],
            );
            self.messages.push(msg);
        }
    }

    fn add_tool_call_part(&mut self, tool: &str, call_id: &str) {
        use ragent_core::message::{ToolCallState, ToolCallStatus};

        if let Some(last) = self.messages.last_mut()
            && last.role == Role::Assistant
        {
            last.parts.push(MessagePart::ToolCall {
                tool: tool.to_string(),
                call_id: call_id.to_string(),
                state: ToolCallState {
                    status: ToolCallStatus::Running,
                    input: serde_json::Value::Null,
                    output: None,
                    error: None,
                    duration_ms: None,
                },
            });
            return;
        }
        if let Some(ref sid) = self.session_id {
            let msg = Message::new(
                sid,
                Role::Assistant,
                vec![MessagePart::ToolCall {
                    tool: tool.to_string(),
                    call_id: call_id.to_string(),
                    state: ToolCallState {
                        status: ToolCallStatus::Running,
                        input: serde_json::Value::Null,
                        output: None,
                        error: None,
                        duration_ms: None,
                    },
                }],
            );
            self.messages.push(msg);
        }
    }

    fn update_tool_call_status(&mut self, call_id: &str, success: bool) {
        use ragent_core::message::ToolCallStatus;

        for msg in self.messages.iter_mut().rev() {
            for part in msg.parts.iter_mut() {
                if let MessagePart::ToolCall {
                    call_id: cid,
                    state,
                    ..
                } = part
                    && cid == call_id
                {
                    state.status = if success {
                        ToolCallStatus::Completed
                    } else {
                        ToolCallStatus::Error
                    };
                    return;
                }
            }
        }
    }
}

/// Produces a short, user-facing summary from a raw error string.
///
/// Strips JSON payloads, HTTP status codes, and verbose context so the
/// chat panel only shows the essential message.
fn summarise_error(raw: &str) -> String {
    // Try to extract just the human-readable message from common patterns
    // e.g. "LLM call failed: Unknown model: claude-haiku-4.5"
    let cleaned = raw.trim().strip_prefix("LLM call failed: ").unwrap_or(raw);

    // Truncate to a reasonable length for the status bar
    if cleaned.len() > 120 {
        format!("{}…", &cleaned[..120])
    } else {
        cleaned.to_string()
    }
}

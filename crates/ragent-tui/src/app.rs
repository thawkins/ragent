//! Application state and event handling for the TUI.
//!
//! The [`App`] struct holds the current session, message history, input buffer,
//! scroll position, and permission state. It processes both terminal key events
//! and agent bus events to drive the UI.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crossterm::event::KeyEvent;

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
        trigger: "model",
        description: "Switch the active model on the current provider",
    },
    SlashCommandDef {
        trigger: "provider",
        description: "Change the LLM provider (re-enters setup flow)",
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
}

impl App {
    /// Create a new [`App`] with default state and the given event bus.
    pub fn new(
        event_bus: Arc<EventBus>,
        storage: Arc<Storage>,
        provider_registry: Arc<ProviderRegistry>,
        session_processor: Arc<SessionProcessor>,
        agent_info: AgentInfo,
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
        let selected_model = storage
            .get_setting("selected_model")
            .ok()
            .flatten();

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
        }
    }

    /// Detect the first configured provider by checking env vars and the database.
    pub fn detect_provider(storage: &Storage) -> Option<ConfiguredProvider> {
        // Check Anthropic
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            if !key.is_empty() {
                return Some(ConfiguredProvider {
                    id: "anthropic".into(),
                    name: "Anthropic (Claude)".into(),
                    source: ProviderSource::EnvVar,
                });
            }
        }
        // Check OpenAI
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            if !key.is_empty() {
                return Some(ConfiguredProvider {
                    id: "openai".into(),
                    name: "OpenAI (GPT)".into(),
                    source: ProviderSource::EnvVar,
                });
            }
        }
        // Check Copilot env var
        if let Ok(key) = std::env::var("GITHUB_COPILOT_TOKEN") {
            if !key.is_empty() {
                return Some(ConfiguredProvider {
                    id: "copilot".into(),
                    name: "GitHub Copilot".into(),
                    source: ProviderSource::EnvVar,
                });
            }
        }
        // Check Copilot auto-discover
        if ragent_core::provider::copilot::find_copilot_token().is_some() {
            return Some(ConfiguredProvider {
                id: "copilot".into(),
                name: "GitHub Copilot".into(),
                source: ProviderSource::AutoDiscovered,
            });
        }
        // Check Ollama (always available locally)
        if let Ok(host) = std::env::var("OLLAMA_HOST") {
            if !host.is_empty() {
                return Some(ConfiguredProvider {
                    id: "ollama".into(),
                    name: "Ollama (Local)".into(),
                    source: ProviderSource::EnvVar,
                });
            }
        }

        // Check database for any stored provider auth
        for (pid, pname) in PROVIDER_LIST {
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

    /// Detect the current git branch, if the cwd is inside a git repository.
    fn detect_git_branch() -> Option<String> {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if branch.is_empty() { None } else { Some(branch) }
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

        tokio::spawn(async move {
            let available = match provider.id.as_str() {
                "ollama" => {
                    ragent_core::provider::ollama::list_ollama_models(None)
                        .await
                        .is_ok()
                }
                "copilot" => {
                    if let Some(token) = ragent_core::provider::copilot::find_copilot_token() {
                        ragent_core::provider::copilot::list_copilot_models(&token, None)
                            .await
                            .is_ok()
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

            let prev_selected = self
                .slash_menu
                .as_ref()
                .map(|m| m.selected)
                .unwrap_or(0);
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
        let trigger = raw.strip_prefix('/').unwrap_or(raw).trim();
        self.input.clear();
        self.slash_menu = None;

        match trigger {
            "agent" => {
                let agents: Vec<(String, String)> = self
                    .cycleable_agents
                    .iter()
                    .map(|a| (a.name.clone(), a.description.clone()))
                    .collect();
                let selected = self.current_agent_index;
                self.provider_setup = Some(ProviderSetupStep::SelectAgent { agents, selected });
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
                    self.status =
                        "⚠ No provider configured — use /provider first".to_string();
                }
            }
            "provider" => {
                self.provider_setup =
                    Some(ProviderSetupStep::SelectProvider { selected: 0 });
            }
            _ => {
                self.status = format!("Unknown command: /{}", trigger);
            }
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
                            "⚠ No provider configured — press p to set up".to_string();
                        return;
                    }
                    if self.selected_model.is_none() {
                        self.status =
                            "⚠ No model selected — press p to choose a model".to_string();
                        return;
                    }
                    // Transition from Home to Chat on first message
                    if self.current_screen == ScreenMode::Home {
                        self.current_screen = ScreenMode::Chat;
                    }
                    // Create session if needed
                    if self.session_id.is_none() {
                        let dir = std::env::current_dir().unwrap_or_default();
                        match self
                            .session_processor
                            .session_manager
                            .create_session(dir)
                        {
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
                        if let Err(e) = processor
                            .process_message(&sid, &text, &agent)
                            .await
                        {
                            tracing::error!(error = %e, "Failed to process message");
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
                InputAction::HistoryDown => {
                    match self.history_index {
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
                    }
                }
                InputAction::SwitchAgent => {
                    if !self.cycleable_agents.is_empty() {
                        self.current_agent_index =
                            (self.current_agent_index + 1) % self.cycleable_agents.len();
                        self.agent_info =
                            self.cycleable_agents[self.current_agent_index].clone();
                        self.agent_name = self.agent_info.name.clone();
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
                }
            }
            Event::ToolCallEnd {
                ref session_id,
                ref call_id,
                ref error,
                ..
            } => {
                if self.is_current_session(session_id) {
                    self.update_tool_call_status(call_id, error.is_none());
                }
            }
            Event::MessageStart { ref session_id, .. } => {
                if self.is_current_session(session_id) {
                    self.status = "processing...".to_string();
                }
            }
            Event::MessageEnd { ref session_id, .. } => {
                if self.is_current_session(session_id) {
                    self.status = "ready".to_string();
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
                }
            }
            Event::PermissionReplied { ref session_id, .. } => {
                if self.is_current_session(session_id) {
                    self.permission_pending = None;
                    self.status = "processing...".to_string();
                }
            }
            Event::AgentSwitched {
                ref session_id,
                ref to,
                ..
            } => {
                if self.is_current_session(session_id) {
                    self.agent_name = to.clone();
                }
            }
            Event::AgentError {
                ref session_id,
                ref error,
            } => {
                if self.is_current_session(session_id) {
                    self.status = format!("error: {}", error);
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
                }
            }
            _ => {}
        }
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

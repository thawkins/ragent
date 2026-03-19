//! Application state and event handling for the TUI.
//!
//! The [`App`] struct holds the current session, message history, input buffer,
//! scroll position, and permission state. It processes both terminal key events
//! and agent bus events to drive the UI.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use ragent_core::{
    agent::{AgentInfo, ModelRef},
    event::{Event, EventBus, FinishReason},
    lsp::{LspManager, LspServer, LspStatus, SharedLspManager, discovery::DiscoveredServer},
    mcp::{McpClient, McpServer, discovery::DiscoveredMcpServer},
    message::{Message, MessagePart, Role},
    permission::PermissionRequest,
    provider::ProviderRegistry,
    session::processor::SessionProcessor,
    storage::Storage,
    team::{
        Mailbox, MailboxMessage, MemberStatus, MessageType, TaskStatus, TeamManager, TeamMember,
        TeamStore,
    },
};

use crate::input::{self, InputAction};
use crate::tips;

mod state;
pub use self::state::*;


impl App {
    /// Create a new [`App`] with default state and the given event bus.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use ragent_core::event::EventBus;
    /// # use ragent_core::provider::ProviderRegistry;
    /// # use ragent_core::session::processor::SessionProcessor;
    /// # use ragent_core::storage::Storage;
    /// # use ragent_core::agent::AgentInfo;
    /// # fn example(
    /// #     event_bus: Arc<EventBus>,
    /// #     storage: Arc<Storage>,
    /// #     registry: Arc<ProviderRegistry>,
    /// #     processor: Arc<SessionProcessor>,
    /// # ) {
    /// let agent = AgentInfo::new("general", "General-purpose agent");
    /// let app = ragent_tui::App::new(
    ///     event_bus, storage, registry, processor, agent, false,
    /// );
    /// # }
    /// ```
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

        let cwd_path = std::env::current_dir().unwrap_or_default();
        let builtin_agents = ragent_core::agent::create_builtin_agents();
        let builtin_names: std::collections::HashSet<String> =
            builtin_agents.iter().map(|a| a.name.clone()).collect();

        let (custom_defs, mut all_diagnostics) =
            ragent_core::agent::custom::load_custom_agents(&cwd_path);

        let cycleable_agents: Vec<AgentInfo> = builtin_agents
            .into_iter()
            .filter(|a| !a.hidden)
            .chain(custom_defs.iter().filter(|d| !d.agent_info.hidden).map(|d| {
                let mut info = d.agent_info.clone();
                if builtin_names.contains(&info.name) {
                    let new_name = format!("custom:{}", info.name);
                    all_diagnostics.push(format!(
                        "custom agent '{}' collides with a built-in agent name — loaded as '{}'",
                        info.name, new_name
                    ));
                    info.name = new_name;
                }
                info
            }))
            .collect();
        let current_agent_index = cycleable_agents
            .iter()
            .position(|a| a.name == agent_info.name)
            .unwrap_or(0);

        // Load persisted model selection
        let selected_model = storage.get_setting("selected_model").ok().flatten();

        let mut app = Self {
            messages: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            scroll_offset: 0,
            is_running: true,
            event_bus,
            storage,
            session_id: None,
            agent_name,
            status: "ready".to_string(),
            permission_pending: None,
            token_usage: (0, 0),
            last_input_tokens: 0,
            quota_percent: None,
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
            file_menu: None,
            project_files_cache: None,
            input_history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            show_log,
            log_entries: Vec::new(),
            log_scroll_offset: 0,
            message_area: Rect::default(),
            log_area: Rect::default(),
            message_max_scroll: 0,
            log_max_scroll: 0,
            active_agents_scroll_offset: 0,
            active_agents_max_scroll: 0,
            scrollbar_drag: None,
            text_selection: None,
            message_content_lines: Vec::new(),
            log_content_lines: Vec::new(),
            input_area: Rect::default(),
            home_input_area: Rect::default(),
            mcp_servers: Vec::new(),
            lsp_servers: Vec::new(),
            lsp_manager: None,
            lsp_discover: None,
            mcp_discover: None,
            force_new_message: false,
            agent_stack: Vec::new(),
            pending_plan_task: None,
            pending_plan_restore: None,
            is_processing: false,
            cancel_flag: None,
            auto_compact_in_progress: false,
            auto_compact_failed: false,
            pending_send_after_compact: None,
                          agent_halted: false,
                          tool_step_map: HashMap::new(),
                          active_tasks: Vec::new(),
                          show_shortcuts: false,
                          context_menu: None,
                          pending_attachments: Vec::new(),
                          history_file_path: None,
                          history_picker: None,
                          selected_agent_session_id: None,
                          selected_agent_index: None,
                          custom_agent_defs: custom_defs,
                          custom_agent_diagnostics: all_diagnostics.clone(),
                          active_team: None,
                          team_members: Vec::new(),
                          show_teams: false,
                          teams_scroll_offset: 0,
                          teams_max_scroll: 0,
                      };

        // Log any warnings from custom agent loading into the log panel
        for diag in &all_diagnostics {
            app.push_log(LogLevel::Warn, format!("[custom agents] {}", diag));
        }

        app
    }

    /// Add a user message to the input history and save it.
    fn add_to_history(&mut self, text: String) {
        // Don't add empty or duplicate entries
        if text.is_empty() || self.input_history.last() == Some(&text) {
            return;
        }
        self.input_history.push(text);
        // Trim to 100 entries
        if self.input_history.len() > 100 {
            self.input_history.remove(0);
        }
        // Save after adding
        let _ = self.save_history();
        self.history_index = None;
        self.history_draft.clear();
    }

    fn selected_model_context_window(&self) -> Option<usize> {
        let model = self.selected_model.as_deref()?;
        let (provider_id, model_id) = model.split_once('/')?;
        self.provider_registry
            .resolve_model(provider_id, model_id)
            .map(|m| m.context_window)
            .filter(|w| *w > 0)
    }

    fn should_auto_compact_before_send(&self) -> bool {
        if self.auto_compact_in_progress
            || self.auto_compact_failed
            || self.pending_send_after_compact.is_some()
        {
            return false;
        }
        if self.session_id.is_none() || self.messages.is_empty() || self.last_input_tokens == 0 {
            return false;
        }
        let Some(context_window) = self.selected_model_context_window() else {
            return false;
        };

        // Start compaction before hitting hard limits.
        let threshold = (context_window as f32 * 0.92) as u64;
        self.last_input_tokens >= threshold
    }

    fn start_compaction(&mut self, auto_triggered: bool) -> bool {
        if self.session_id.is_none() {
            self.status = "⚠ No active session to compact".to_string();
            return false;
        }
        if self.messages.is_empty() {
            self.status = "⚠ No messages to compact".to_string();
            return false;
        }

        let sid = self.session_id.clone().unwrap_or_default();
        let compaction_agent = ragent_core::agent::resolve_agent("compaction", &Default::default())
            .unwrap_or_else(|_| self.agent_info.clone());

        // Override model to match current selection
        let mut agent = compaction_agent;
        if let Some(ref model_str) = self.selected_model
            && let Some((provider, model)) = model_str.split_once('/')
        {
            agent.model = Some(ModelRef {
                provider_id: provider.to_string(),
                model_id: model.to_string(),
            });
        }

        let summary_prompt =
            "Summarise the conversation so far into a concise representation that \
             preserves all important context, decisions, code changes, file paths, \
             and outstanding tasks. Output only the summary."
                .to_string();

        self.auto_compact_in_progress = auto_triggered;
        if auto_triggered {
            self.auto_compact_failed = false;
            self.status = "compacting before send…".to_string();
            self.push_log(
                LogLevel::Warn,
                "Auto-compaction triggered (context near limit)".to_string(),
            );
        } else {
            self.status = "compacting…".to_string();
            self.push_log(LogLevel::Info, "Compaction started".to_string());
        }

        let processor = self.session_processor.clone();
        let event_bus = self.event_bus.clone();
        tokio::spawn(async move {
            match processor
                .process_message(&sid, &summary_prompt, &agent, Arc::new(AtomicBool::new(false)))
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
        true
    }

    fn dispatch_user_message(
        &mut self,
        text: String,
        image_paths: Vec<std::path::PathBuf>,
    ) {
        self.auto_compact_failed = false;
        let Some(sid) = self.session_id.clone() else {
            self.status = "⚠ No active session".to_string();
            return;
        };

        let display_text = if image_paths.is_empty() {
            text.clone()
        } else {
            let names: Vec<String> = image_paths
                .iter()
                .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
                .collect();
            format!("[📎 {}] {}", names.join(", "), text)
        };
        let msg = Message::user_text(&sid, &display_text);
        self.messages.push(msg);
        self.input_history.push(text.clone());
        self.history_index = None;
        self.history_draft.clear();
        self.input.clear();
        self.input_cursor = 0;
        self.file_menu = None;
        self.status = "processing...".to_string();

        let has_refs = !ragent_core::reference::parse::parse_refs(&text).is_empty();
        if has_refs {
            let ref_names: Vec<String> = ragent_core::reference::parse::parse_refs(&text)
                .iter()
                .map(|r| r.raw.clone())
                .collect();
            self.push_log(
                LogLevel::Info,
                format!("resolving refs: {}", ref_names.join(", ")),
            );
        }

        let truncated = if text.len() > 120 {
            let mut end = 120;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}…", &text[..end])
        } else {
            text.clone()
        };
        let model_tag = if let Some(ref model_str) = self.selected_model {
            format!(" [{}]", model_str)
        } else {
            String::new()
        };
        self.push_log(
            LogLevel::Info,
            format!("prompt sent{}: {}", model_tag, truncated),
        );

        let mut agent = self.agent_info.clone();
        if let Some(ref model_str) = self.selected_model
            && let Some((provider, model)) = model_str.split_once('/')
        {
            agent.model = Some(ModelRef {
                provider_id: provider.to_string(),
                model_id: model.to_string(),
            });
        }

        let processor = self.session_processor.clone();
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(flag.clone());
        tokio::spawn(async move {
            let final_text = if has_refs {
                let wd = std::env::current_dir().unwrap_or_default();
                match ragent_core::reference::resolve::resolve_all_refs(&text, &wd).await {
                    Ok((resolved, _)) => resolved,
                    Err(e) => {
                        tracing::warn!(error = %e, "ref resolution failed, using original text");
                        text.clone()
                    }
                }
            } else {
                text.clone()
            };

            if image_paths.is_empty() {
                if let Err(e) = processor.process_message(&sid, &final_text, &agent, flag).await {
                    tracing::debug!(error = %e, "Failed to process message");
                }
            } else {
                let mut parts: Vec<ragent_core::message::MessagePart> = image_paths
                    .into_iter()
                    .filter(|p| p.exists())
                    .map(|p| {
                        let mime = if p
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("png"))
                            .unwrap_or(false)
                        {
                            "image/png"
                        } else if p
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("gif"))
                            .unwrap_or(false)
                        {
                            "image/gif"
                        } else {
                            "image/jpeg"
                        };
                        ragent_core::message::MessagePart::Image {
                            mime_type: mime.to_string(),
                            path: p,
                        }
                    })
                    .collect();
                parts.push(ragent_core::message::MessagePart::Text { text: final_text });
                let user_msg = ragent_core::message::Message::new(
                    &sid,
                    ragent_core::message::Role::User,
                    parts,
                );
                if let Err(e) = processor
                    .process_user_message(&sid, user_msg, &agent, flag)
                    .await
                {
                    tracing::debug!(error = %e, "Failed to process message with images");
                }
            }
        });
    }

    /// Detect the first configured provider by checking env vars and the database.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use ragent_core::storage::Storage;
    /// # use ragent_tui::App;
    /// # fn example(storage: &Storage) {
    /// if let Some(provider) = App::detect_provider(storage) {
    ///     println!("Found provider: {}", provider.name);
    /// }
    /// # }
    /// ```
    pub fn detect_provider(storage: &Storage) -> Option<ConfiguredProvider> {
        // Helper: returns true when the user has explicitly reset this provider.
        let is_disabled = |pid: &str| -> bool {
            storage
                .get_setting(&format!("provider_{pid}_disabled"))
                .ok()
                .flatten()
                .is_some()
        };

        // Check for an explicit user preference first — this overrides auto-discovery
        // so that e.g. selecting Ollama doesn't get overwritten by Copilot IDE tokens.
        if let Ok(Some(preferred)) = storage.get_setting("preferred_provider") {
            if !preferred.is_empty() && !is_disabled(&preferred) {
                if let Some(&(pid, pname)) = PROVIDER_LIST.iter().find(|(id, _)| *id == preferred)
                {
                    return Some(ConfiguredProvider {
                        id: pid.to_string(),
                        name: pname.to_string(),
                        source: ProviderSource::Database,
                    });
                }
            }
        }

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
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use ragent_tui::App;
    /// # fn example(app: &mut App) {
    /// app.refresh_provider();
    /// # }
    /// ```
    pub fn refresh_provider(&mut self) {
        self.configured_provider = Self::detect_provider(&self.storage);
    }

    /// Return the current input length in characters.
    pub(crate) fn input_len_chars(&self) -> usize {
        self.input.chars().count()
    }

    /// Return the byte offset corresponding to the current cursor position.
    pub(crate) fn cursor_byte_pos(&self) -> usize {
        self.cursor_byte_pos_at_char_index(self.input_cursor)
    }

    /// Return the byte offset corresponding to a character index.
    pub(crate) fn cursor_byte_pos_at_char_index(&self, char_index: usize) -> usize {
        if char_index == 0 {
            return 0;
        }
        if char_index >= self.input_len_chars() {
            return self.input.len();
        }
        // Walk the chars to determine the byte offset.
        self.input
            .char_indices()
            .nth(char_index)
            .map(|(byte, _)| byte)
            .unwrap_or_else(|| self.input.len())
    }

    /// Move the cursor one character to the left (if possible).
    pub(crate) fn cursor_move_left(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor -= 1;
        }
    }

    /// Move the cursor one character to the right (if possible).
    pub(crate) fn cursor_move_right(&mut self) {
        if self.input_cursor < self.input_len_chars() {
            self.input_cursor += 1;
        }
    }

    /// Move the cursor to the beginning of the input line.
    pub(crate) fn cursor_move_home(&mut self) {
        self.input_cursor = 0;
    }

    /// Move the cursor to the end of the input line.
    pub(crate) fn cursor_move_end(&mut self) {
        self.input_cursor = self.input_len_chars();
    }

    /// Attach an [`LspManager`] to the app.
    ///
    /// Called from `run_tui()` after the manager has been created and initial
    /// server connections have been started. The app keeps the manager alive
    /// and uses it for `/lsp` command operations.
    pub fn set_lsp_manager(&mut self, manager: SharedLspManager) {
        self.lsp_manager = Some(manager);
    }

    /// Add a discovered server to the `lsp` section in `ragent.json` and
    /// enable it. Returns `Ok(())` on success or an error description.
    pub fn enable_discovered_server(&self, server: &DiscoveredServer) -> Result<String, String> {
        use ragent_core::config::{Config, LspServerConfig};
        use std::collections::HashMap;

        // Load (or default-construct) the current config.
        let mut config = Config::load().unwrap_or_default();

        if config.lsp.contains_key(&server.id) {
            return Err(format!(
                "'{}' is already in ragent.json. Edit it manually to change settings.",
                server.id
            ));
        }

        let cfg = LspServerConfig {
            command: Some(server.executable.to_string_lossy().into_owned()),
            args: server.args.clone(),
            env: HashMap::new(),
            extensions: server.extensions.clone(),
            disabled: false,
            timeout_ms: LspServerConfig::default_timeout_ms(),
        };
        config.lsp.insert(server.id.clone(), cfg);

        // Persist back to ragent.json in the working directory.
        let config_path = std::env::current_dir()
            .unwrap_or_default()
            .join("ragent.json");

        match std::fs::read_to_string(&config_path) {
            Ok(existing) => {
                // Merge: parse existing JSON, insert/update the lsp key.
                let mut json: serde_json::Value =
                    serde_json::from_str(&existing).map_err(|e| e.to_string())?;
                let lsp_entry = serde_json::json!({
                    "command": server.executable.to_string_lossy(),
                    "args": server.args,
                    "extensions": server.extensions,
                    "disabled": false,
                });
                json["lsp"][&server.id] = lsp_entry;
                let out = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
                std::fs::write(&config_path, out).map_err(|e| e.to_string())?;
            }
            Err(_) => {
                // No existing file — write a minimal config.
                let out = serde_json::to_string_pretty(&serde_json::json!({ "lsp": {
                    &server.id: {
                        "command": server.executable.to_string_lossy(),
                        "args": server.args,
                        "extensions": server.extensions,
                        "disabled": false,
                    }
                }}))
                .map_err(|e| e.to_string())?;
                std::fs::write(&config_path, out).map_err(|e| e.to_string())?;
            }
        }

        Ok(format!(
            "✓ '{}' added to ragent.json. Restart ragent to activate the LSP server.",
            server.id
        ))
    }

    /// Add a discovered MCP server to the `mcp` section in `ragent.json` and
    /// enable it. Returns `Ok(())` on success or an error description.
    pub fn enable_discovered_mcp_server(
        &self,
        server: &DiscoveredMcpServer,
    ) -> Result<String, String> {
        use ragent_core::config::Config;

        // Load (or default-construct) the current config.
        let mut config = Config::load().unwrap_or_default();

        if config.mcp.contains_key(&server.id) {
            return Err(format!(
                "'{}' is already in ragent.json. Edit it manually to change settings.",
                server.id
            ));
        }

        let cfg = server.to_config();
        // Enable the server (discovery sets disabled=true by default)
        let mut cfg = cfg;
        cfg.disabled = false;
        config.mcp.insert(server.id.clone(), cfg.clone());

        // Persist back to ragent.json in the working directory.
        let config_path = std::env::current_dir()
            .unwrap_or_default()
            .join("ragent.json");

        match std::fs::read_to_string(&config_path) {
            Ok(existing) => {
                // Merge: parse existing JSON, insert/update the mcp key.
                let mut json: serde_json::Value =
                    serde_json::from_str(&existing).map_err(|e| e.to_string())?;
                let mcp_entry = serde_json::json!({
                    "type": "stdio",
                    "command": server.executable.to_string_lossy(),
                    "args": server.args,
                    "env": server.env,
                    "disabled": false,
                });
                json["mcp"][&server.id] = mcp_entry;
                let out = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
                std::fs::write(&config_path, out).map_err(|e| e.to_string())?;
            }
            Err(_) => {
                // No existing file — write a minimal config.
                let out = serde_json::to_string_pretty(&serde_json::json!({ "mcp": {
                    &server.id: {
                        "type": "stdio",
                        "command": server.executable.to_string_lossy(),
                        "args": server.args,
                        "env": server.env,
                        "disabled": false,
                    }
                }}))
                .map_err(|e| e.to_string())?;
                std::fs::write(&config_path, out).map_err(|e| e.to_string())?;
            }
        }

        Ok(format!(
            "✓ '{}' added to ragent.json. Restart ragent to activate the MCP server.",
            server.id
        ))
    }

    /// Ensure a session exists, creating one if needed.
    ///
    /// Returns `false` and sets `self.status` if session creation fails.
    fn ensure_session(&mut self) -> bool {
        if self.session_id.is_some() {
            return true;
        }
        let dir = std::env::current_dir().unwrap_or_default();
        match self.session_processor.session_manager.create_session(dir) {
            Ok(session) => {
                self.session_id = Some(session.id);
                true
            }
            Err(e) => {
                self.status = format!("error: {}", e);
                false
            }
        }
    }

    /// Lazily initialise the TeamManager for the current session/team.
    fn ensure_team_manager_for_team(
        &mut self,
        team_name: &str,
        known_team_dir: Option<std::path::PathBuf>,
    ) {
        if self.session_processor.team_manager.get().is_some() {
            return;
        }
        let Some(lead_session_id) = self.session_id.clone() else {
            return;
        };

        let team_dir = if let Some(dir) = known_team_dir {
            dir
        } else {
            let working_dir = std::env::current_dir().unwrap_or_default();
            match TeamStore::load_by_name(team_name, &working_dir) {
                Ok(store) => store.dir,
                Err(e) => {
                    self.push_log(
                        LogLevel::Warn,
                        format!("TeamManager init skipped: cannot load team '{team_name}': {e}"),
                    );
                    return;
                }
            }
        };

        let manager = Arc::new(TeamManager::new(
            team_name.to_string(),
            lead_session_id,
            team_dir,
            self.session_processor.clone(),
            self.event_bus.clone(),
        ));

        if self.session_processor.team_manager.set(manager).is_ok() {
            self.push_log(
                LogLevel::Info,
                format!("TeamManager initialised for team '{team_name}'"),
            );
        }
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
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use ragent_tui::App;
    /// # fn example(app: &mut App) {
    /// if let Err(e) = app.load_session("session-abc-123") {
    ///     eprintln!("Failed to load session: {e}");
    /// }
    /// # }
    /// ```
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

        // Rebuild tool_step_map from restored tool calls and populate log
        // (step count comes from event_bus, not local counter)
        self.tool_step_map.clear();
        let mut restored_logs: Vec<(u64, String, String)> = Vec::new();
        let mut step_counter = 0u64;
        for msg in &self.messages {
            for part in &msg.parts {
                if let MessagePart::ToolCall {
                    call_id,
                    tool,
                    state,
                } = part
                {
                    step_counter += 1;
                    let short_sid = self
                        .session_id
                        .as_deref()
                        .map(short_session_id)
                        .unwrap_or_default();
                    self.tool_step_map
                        .insert(call_id.clone(), (short_sid, step_counter as u32));
                    let icon = match state.status {
                        ragent_core::message::ToolCallStatus::Completed => "✓",
                        ragent_core::message::ToolCallStatus::Error => "✗",
                        _ => "…",
                    };
                    restored_logs.push((step_counter, tool.clone(), icon.to_string()));
                }
            }
        }
        for (step, tool, icon) in restored_logs {
            let short_sid = self
                .session_id
                .as_deref()
                .map(short_session_id)
                .unwrap_or_default();
            self.push_log(
                LogLevel::Tool,
                format!("[{short_sid}:{step}] {tool} {icon} (restored)"),
            );
        }

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
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use ragent_tui::App;
    /// # fn example(app: &App) {
    /// let models = app.models_for_provider("anthropic");
    /// for (id, name) in &models {
    ///     println!("{id}: {name}");
    /// }
    /// # }
    /// ```
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
                    let _storage = self.storage.clone();
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
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use ragent_tui::App;
    /// # fn example(app: &App) {
    /// if let Some(label) = app.provider_model_label() {
    ///     println!("Using: {label}");
    /// }
    /// # }
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use ragent_tui::App;
    /// # fn example(app: &mut App) {
    /// app.check_provider_health();
    /// // Later, query the result:
    /// match app.provider_health_status() {
    ///     Some(true)  => println!("Provider is available"),
    ///     Some(false) => println!("Provider is unavailable"),
    ///     None        => println!("Still checking..."),
    /// }
    /// # }
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use ragent_tui::App;
    /// # fn example(app: &App) {
    /// match app.provider_health_status() {
    ///     Some(true)  => println!("healthy"),
    ///     Some(false) => println!("unhealthy"),
    ///     None        => println!("checking"),
    /// }
    /// # }
    /// ```
    pub fn provider_health_status(&self) -> Option<bool> {
        match self.provider_health.load(Ordering::Relaxed) {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        }
    }

    /// Returns a `(text, is_unknown)` pair for the provider usage display in the status bar.
    ///
    /// For GitHub Copilot, returns the plan label (e.g. `"Pro"`) inferred from the
    /// cached session token, combined with the context-window utilisation percentage
    /// computed from the most recent request's input token count.
    ///
    /// For all other providers, `is_unknown` is `true` and the text is `"unknown"`,
    /// indicating that usage information is not available.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use ragent_tui::App;
    /// # fn example(app: &App) {
    /// let (label, unknown) = app.usage_display();
    /// println!("usage: {} (unknown={})", label, unknown);
    /// # }
    /// ```
    pub fn usage_display(&self) -> (String, bool) {
        let provider_id = self
            .configured_provider
            .as_ref()
            .map(|p| p.id.as_str())
            .unwrap_or("");

        // Provider rate-limit quota % takes priority when available.
        if let Some(quota) = self.quota_percent {
            let label = if provider_id == "copilot" {
                let plan = ragent_core::provider::copilot::cached_copilot_plan()
                    .unwrap_or_else(|| "Copilot".to_string());
                format!("{} quota: {:.1}%", plan, quota)
            } else {
                format!("quota: {:.1}%", quota)
            };
            return (label, false);
        }

        // Compute context-window usage % from last request's input token count.
        let context_pct: Option<f32> = self
            .selected_model
            .as_deref()
            .and_then(|m| {
                let mut parts = m.splitn(2, '/');
                let pid = parts.next()?;
                let mid = parts.next()?;
                self.provider_registry.resolve_model(pid, mid)
            })
            .filter(|m| m.context_window > 0)
            .map(|m| (self.last_input_tokens as f32 / m.context_window as f32 * 100.0).min(100.0));

        if provider_id == "copilot" {
            let plan = ragent_core::provider::copilot::cached_copilot_plan()
                .unwrap_or_else(|| "Copilot".to_string());
            let text = match context_pct {
                Some(p) => format!("{} ctx: {:.0}%", plan, p),
                None => plan,
            };
            (text, false)
        } else if provider_id == "ollama" {
            match context_pct {
                Some(p) => (format!("local ctx: {:.0}%", p), false),
                None => ("local".to_string(), false),
            }
        } else if let Some(p) = context_pct {
            (format!("ctx: {:.0}%", p), false)
        } else {
            ("unknown".to_string(), true)
        }
    }

    /// Update the slash-command autocomplete menu based on the current input buffer.
    ///
    /// Shows the menu when input starts with `/`, filtering commands by the text
    /// after the slash. Closes the menu when input no longer starts with `/`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use ragent_tui::App;
    /// # fn example(app: &mut App) {
    /// app.input = "/mod".to_string();
    /// app.update_slash_menu();
    /// // The slash menu now shows commands matching "mod"
    /// # }
    /// ```
    pub fn update_slash_menu(&mut self) {
        if let Some(filter) = self.input.strip_prefix('/') {
            // If the user has typed a space after the command (i.e., they are
            // entering subcommand arguments like "/lsp discover"), close the
            // menu so it doesn't obstruct the input.
            if filter.contains(' ') {
                self.slash_menu = None;
                return;
            }

            let needle = filter.to_lowercase();

            // Collect builtin command matches
            let mut matches: Vec<SlashMenuEntry> = SLASH_COMMANDS
                .iter()
                .filter(|cmd| {
                    needle.is_empty()
                        || cmd.trigger.starts_with(&needle)
                        || cmd.description.to_lowercase().contains(&needle)
                })
                .map(|cmd| SlashMenuEntry {
                    trigger: cmd.trigger.to_string(),
                    description: cmd.description.to_string(),
                    is_skill: false,
                })
                .collect();

            // Collect user-invocable skill matches
            let working_dir = std::env::current_dir().unwrap_or_default();
            let skill_dirs = ragent_core::config::Config::load()
                .map(|c| c.skill_dirs)
                .unwrap_or_default();
            let registry = ragent_core::skill::SkillRegistry::load(&working_dir, &skill_dirs);
            for skill in registry.list_user_invocable() {
                let desc = skill
                    .description
                    .as_deref()
                    .unwrap_or("(skill)")
                    .to_string();
                let hint = skill
                    .argument_hint
                    .as_deref()
                    .map(|h| format!(" — {h}"))
                    .unwrap_or_default();

                // Skip if a builtin command has the same trigger
                if matches.iter().any(|m| m.trigger == skill.name) {
                    continue;
                }

                if needle.is_empty()
                    || skill.name.starts_with(&needle)
                    || desc.to_lowercase().contains(&needle)
                {
                    matches.push(SlashMenuEntry {
                        trigger: skill.name.clone(),
                        description: format!("{desc}{hint}"),
                        is_skill: true,
                    });
                }
            }

            // Sort alphabetically by trigger so the list is predictable.
            matches.sort_by(|a, b| a.trigger.cmp(&b.trigger));

            // Select the entry whose trigger best matches the typed input:
            // prefer an exact match, then the first entry whose trigger starts
            // with the needle, then fall back to index 0.
            let selected = if matches.is_empty() {
                0
            } else if let Some(exact) = matches.iter().position(|m| m.trigger == needle) {
                exact
            } else if let Some(prefix) = matches.iter().position(|m| m.trigger.starts_with(&needle))
            {
                prefix
            } else {
                0
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

    /// Update the `@` file reference autocomplete menu based on current input.
    ///
    /// Detects the last `@` token in the input, extracts the query after it,
    /// and populates `file_menu` with matching project files.
    pub fn update_file_menu(&mut self) {
        // Find the last '@' that's a valid reference trigger
        let input = &self.input;
        if let Some(at_pos) = input.rfind('@') {
            // '@' must be at start or preceded by whitespace
            if at_pos > 0 {
                let prev_byte = input.as_bytes()[at_pos - 1];
                if prev_byte.is_ascii_alphanumeric() || prev_byte == b'.' {
                    self.file_menu = None;
                    return;
                }
            }

            let query = &input[at_pos + 1..];

            // Close menu if query contains whitespace (user finished typing ref)
            if query.contains(char::is_whitespace) {
                self.file_menu = None;
                return;
            }

            // Lazily populate the project file cache
            if self.project_files_cache.is_none() {
                let wd = std::env::current_dir().unwrap_or_default();
                self.project_files_cache = Some(
                    ragent_core::reference::fuzzy::collect_project_files(&wd, 10_000),
                );
            }

            if let Some(ref candidates) = self.project_files_cache {
                let matches = ragent_core::reference::fuzzy::fuzzy_match(query, candidates);

                let entries: Vec<FileMenuEntry> = matches
                    .into_iter()
                    .take(15)
                    .map(|m| {
                        let is_dir = m.path.to_string_lossy().ends_with('/');
                        FileMenuEntry {
                            display: m.path.to_string_lossy().to_string(),
                            path: m.path,
                            is_dir,
                        }
                    })
                    .collect();

                if entries.is_empty() {
                    self.file_menu = None;
                } else {
                    let prev_selected = self.file_menu.as_ref().map(|m| m.selected).unwrap_or(0);
                    self.file_menu = Some(FileMenuState {
                        selected: prev_selected.min(entries.len().saturating_sub(1)),
                        matches: entries,
                        query: query.to_string(),
                        current_dir: None,
                    });
                }
            } else {
                self.file_menu = None;
            }
        } else {
            self.file_menu = None;
        }
    }

    /// Populate the file menu with the immediate contents of `dir_rel`.
    fn populate_directory_menu(&mut self, dir_rel: &std::path::Path) {
        let wd = std::env::current_dir().unwrap_or_default();
        let abs = wd.join(dir_rel);
        let mut entries: Vec<FileMenuEntry> = Vec::new();

        if abs.is_dir() {
            // Read the directory contents from disk (sorted)
            if let Ok(rd) = std::fs::read_dir(&abs) {
                let mut sorted: Vec<_> = rd.filter_map(|e| e.ok()).collect();
                sorted.sort_by_key(|e| e.file_name());
                for entry in sorted {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Skip hidden
                    if name.starts_with('.') {
                        continue;
                    }
                    let path_abs = entry.path();
                    let is_dir = path_abs.is_dir();
                    let rel = path_abs
                        .strip_prefix(&wd)
                        .unwrap_or(&path_abs)
                        .to_path_buf();
                    let display = if is_dir {
                        format!("{}/", rel.to_string_lossy())
                    } else {
                        rel.to_string_lossy().to_string()
                    };
                    entries.push(FileMenuEntry {
                        display,
                        path: rel,
                        is_dir,
                    });
                }
            }

            // Add parent entry if not at project root
            if !dir_rel.as_os_str().is_empty() {
                let parent = dir_rel.parent().unwrap_or(std::path::Path::new(""));
                let parent_display = if parent.as_os_str().is_empty() {
                    "../".to_string()
                } else {
                    format!("{}/", parent.to_string_lossy())
                };
                entries.insert(
                    0,
                    FileMenuEntry {
                        display: parent_display,
                        path: parent.to_path_buf(),
                        is_dir: true,
                    },
                );
            }
        }

        if entries.is_empty() {
            self.file_menu = None;
        } else {
            self.file_menu = Some(FileMenuState {
                selected: 0,
                matches: entries,
                query: String::new(),
                current_dir: Some(dir_rel.to_path_buf()),
            });
        }
    }

    /// Accept the currently selected file menu entry. If the selected entry is
    /// a directory, navigate into it and show its contents. Returns `true` if a
    /// file was inserted into the input (menu closed), or `false` if the menu
    /// remains open due to directory navigation.
    pub fn accept_file_menu_selection(&mut self) -> bool {
        // Clone the selected entry out of the menu to avoid holding an
        // immutable borrow of self while we call mutating methods below.
        let selected_entry: Option<FileMenuEntry> = self
            .file_menu
            .as_ref()
            .and_then(|m| m.matches.get(m.selected).cloned());

        if let Some(entry) = selected_entry {
            if entry.is_dir {
                // Navigate into the directory instead of inserting it.
                self.populate_directory_menu(&entry.path);
                return false;
            } else {
                // Insert file path into the input and close the menu.
                let path = entry.display.clone();
                if let Some(at_pos) = self.input.rfind('@') {
                    self.input.truncate(at_pos + 1);
                    self.input.push_str(&path);
                    self.input_cursor = self.input_len_chars();
                }
                self.file_menu = None;
                return true;
            }
        }

        self.file_menu = None;
        false
    }

    /// Execute a slash command by trigger name (e.g. `"/model"` or `"model"`).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use ragent_tui::App;
    /// # fn example(app: &mut App) {
    /// app.execute_slash_command("/help");
    /// # }
    /// ```
    pub fn execute_slash_command(&mut self, raw: &str) {
        let stripped = raw.strip_prefix('/').unwrap_or(raw).trim();
        self.input.clear();
        self.input_cursor = 0;
        self.slash_menu = None;
        self.scroll_offset = 0;
        self.force_new_message = true;

        // Split into command and optional argument text.
        let (cmd, args) = stripped
            .split_once(char::is_whitespace)
            .map_or((stripped, ""), |(c, a)| (c, a.trim()));

        match cmd {
            "about" => {
                // Ensure a session exists so the about text can be displayed
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
                let about = format!(
                    "  ragent — AI Coding Agent\n\
                     \n\
                     \x20 An interactive TUI-based AI coding agent\n\
                     \x20 supporting multiple LLM providers.\n\
                     \n\
                     \x20 Version:     {}\n\
                     \x20 Built:       {}\n\
                     \x20 Repository:  https://github.com/thawkins/ragent\n\
                     \x20 License:     MIT\n\
                     \n\
                     \x20 Authors:\n\
                     \x20   Tim Hawkins <tim.thawkins@gmail.com>\n",
                    env!("CARGO_PKG_VERSION"),
                    env!("BUILD_TIMESTAMP"),
                );
                self.append_assistant_text(&format!("From: /about\n{about}"));
                if self.current_screen == ScreenMode::Home {
                    self.current_screen = ScreenMode::Chat;
                }
                self.status = "about".to_string();
            }
            "agent" => {
                if args.is_empty() {
                    // Open the agent picker dialog
                    let custom_names: std::collections::HashSet<String> = self
                        .custom_agent_defs
                        .iter()
                        .map(|d| d.agent_info.name.clone())
                        .collect();
                    let agents: Vec<(String, String, bool)> = self
                        .cycleable_agents
                        .iter()
                        .map(|a| {
                            let is_custom = custom_names.contains(&a.name)
                                || a.name.starts_with("custom:");
                            (a.name.clone(), a.description.clone(), is_custom)
                        })
                        .collect();
                    let selected = self.current_agent_index;
                    self.provider_setup = Some(ProviderSetupStep::SelectAgent { agents, selected });
                } else {
                    // Direct switch: /agent <name>
                    if let Some(idx) = self.cycleable_agents.iter().position(|a| a.name == args) {
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
                        let available: Vec<&str> = self
                            .cycleable_agents
                            .iter()
                            .map(|a| a.name.as_str())
                            .collect();
                        self.status = format!(
                            "Unknown agent '{}'. Available: {}",
                            args,
                            available.join(", ")
                        );
                        self.push_log(LogLevel::Warn, format!("Unknown agent: {}", args));
                    }
                }
            }
            "agents" => {
                // Ensure a session exists so output can be displayed
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

                let mut output = String::from("From: /agents\n\nBuilt-in Agents:\n\n");

                let custom_names: std::collections::HashSet<String> = self
                    .custom_agent_defs
                    .iter()
                    .map(|d| d.agent_info.name.clone())
                    .collect();

                for agent in &self.cycleable_agents {
                    let is_custom = custom_names.contains(&agent.name)
                        || agent.name.starts_with("custom:");
                    if !is_custom {
                        let active = if agent.name == self.agent_name { " ●" } else { "" };
                        output.push_str(&format!(
                            "  {:<18} {}{}\n",
                            agent.name, agent.description, active
                        ));
                    }
                }

                if self.custom_agent_defs.is_empty() {
                    output.push_str(
                        "\nCustom Agents:\n\n  (none — place .json files in .ragent/agents/ or ~/.ragent/agents/)\n",
                    );
                } else {
                    output.push_str("\nCustom Agents:\n\n");
                    for def in &self.custom_agent_defs {
                        let scope = if def.is_project_local { "project" } else { "global" };
                        let name = &def.agent_info.name;
                        let desc = &def.agent_info.description;
                        let active = if *name == self.agent_name { " ●" } else { "" };
                        output.push_str(&format!(
                            "  {:<18} {} [{}]{}\n",
                            name, desc, scope, active
                        ));
                    }
                }

                if !self.custom_agent_diagnostics.is_empty() {
                    output.push_str("\nDiagnostics:\n\n");
                    for diag in &self.custom_agent_diagnostics {
                        output.push_str(&format!("  ⚠ {}\n", diag));
                    }
                }

                self.append_assistant_text(&output);
                if self.current_screen == ScreenMode::Home {
                    self.current_screen = ScreenMode::Chat;
                }
                self.status = "agents".to_string();
            }
            "clear" => {
                self.messages.clear();
                self.scroll_offset = 0;
                self.tool_step_map.clear();
                self.status = "messages cleared".to_string();
                self.push_log(LogLevel::Info, "Message history cleared".to_string());
            }
            "cancel" => {
                if args.is_empty() {
                    self.status = "⚠ Please provide a task ID prefix: /cancel <id>".to_string();
                    self.push_log(LogLevel::Warn, "No task ID provided".to_string());
                    return;
                }

                if let Some(task) = self.active_tasks.iter().find(|t| t.id.starts_with(args)) {
                    let task_id = task.id.clone();
                    let agent = task.agent_name.clone();
                    if let Some(idx) = self.active_tasks.iter().position(|t| t.id == task_id) {
                        self.active_tasks.remove(idx);
                    }
                    self.status = format!(
                        "Cancelled task {} ({})",
                        &task_id[..8.min(task_id.len())],
                        agent
                    );
                    self.push_log(
                        LogLevel::Info,
                        format!(
                            "Task cancelled: {}... ({})",
                            &task_id[..8.min(task_id.len())],
                            agent
                        ),
                    );
                } else {
                    self.status = format!("No task found with ID starting with '{}'", args);
                    self.push_log(LogLevel::Warn, format!("Task not found: {}", args));
                }
            }
            "compact" => {
                let _ = self.start_compaction(false);
            }
            "help" => {
                // Ensure a session exists so the help text can be appended
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
                let mut help_lines = String::from("From: /help\nAvailable commands:\n");
                for cmd_def in SLASH_COMMANDS {
                    help_lines.push_str(&format!(
                        "  /{:<18} {}\n",
                        cmd_def.trigger, cmd_def.description
                    ));
                }

                // Append user-invocable skills
                let working_dir = std::env::current_dir().unwrap_or_default();
                let skill_dirs = ragent_core::config::Config::load()
                    .map(|c| c.skill_dirs)
                    .unwrap_or_default();
                let registry = ragent_core::skill::SkillRegistry::load(&working_dir, &skill_dirs);
                let skills = registry.list_user_invocable();
                if !skills.is_empty() {
                    help_lines.push_str("\nSkills:\n");
                    for skill in &skills {
                        let desc = skill.description.as_deref().unwrap_or("(no description)");
                        let hint = skill
                            .argument_hint
                            .as_deref()
                            .map(|h| format!(" {h}"))
                            .unwrap_or_default();
                        help_lines.push_str(&format!(
                            "  /{:<18} {}\n",
                            format!("{}{}", skill.name, hint),
                            desc
                        ));
                    }
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
            "history" => {
                if self.input_history.is_empty() {
                    self.status = "No input history yet".to_string();
                } else {
                    // Show newest entries first
                    let entries: Vec<String> =
                        self.input_history.iter().rev().cloned().collect();
                    self.history_picker = Some(crate::app::state::HistoryPickerState {
                        entries,
                        selected: 0,
                        scroll_offset: 0,
                    });
                    self.input.clear();
                    self.input_cursor = 0;
                }
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
            "reload" => {
                let sub = args.split_whitespace().next().unwrap_or("all");
                let do_agents = matches!(sub, "all" | "agents");
                let do_config = matches!(sub, "all" | "config");
                let do_mcp = matches!(sub, "all" | "mcp");
                let do_skills = matches!(sub, "all" | "skills");

                let mut report = String::from("From: /reload\n\n");

                // ── reload agents ──────────────────────────────────────────────────
                if do_agents {
                    let cwd_path = std::env::current_dir().unwrap_or_default();
                    let builtin_agents = ragent_core::agent::create_builtin_agents();
                    let builtin_names: std::collections::HashSet<String> =
                        builtin_agents.iter().map(|a| a.name.clone()).collect();

                    let (new_defs, mut diags) =
                        ragent_core::agent::custom::load_custom_agents(&cwd_path);

                    // Rebuild cycleable list: builtins (non-hidden) + custom
                    let mut new_cycleable: Vec<_> = builtin_agents
                        .into_iter()
                        .filter(|a| !a.hidden)
                        .collect();
                    for def in &new_defs {
                        let mut info = def.agent_info.clone();
                        if builtin_names.contains(&info.name) {
                            let new_name = format!("custom:{}", info.name);
                            diags.push(format!(
                                "custom agent '{}' collides with a built-in — loaded as '{}'",
                                info.name, new_name
                            ));
                            info.name = new_name;
                        }
                        if !info.hidden {
                            new_cycleable.push(info);
                        }
                    }

                    let prev_count = self.custom_agent_defs.len();
                    self.custom_agent_defs = new_defs;
                    self.custom_agent_diagnostics = diags.clone();
                    // Preserve current_agent_index if possible
                    let current_name = self.agent_name.clone();
                    self.current_agent_index = new_cycleable
                        .iter()
                        .position(|a| a.name == current_name)
                        .unwrap_or(0);
                    self.cycleable_agents = new_cycleable;

                    for d in &diags {
                        self.push_log(LogLevel::Warn, format!("[reload agents] {}", d));
                    }
                    report.push_str(&format!(
                        "✓ Agents reloaded — {} custom agent(s) (was {})\n",
                        self.custom_agent_defs.len(),
                        prev_count,
                    ));
                    self.push_log(
                        LogLevel::Info,
                        format!(
                            "reload agents: {} custom agent(s) loaded",
                            self.custom_agent_defs.len()
                        ),
                    );
                }

                // ── reload config ──────────────────────────────────────────────────
                if do_config {
                    match ragent_core::config::Config::load() {
                        Ok(_cfg) => {
                            // Refresh cached provider and model selections
                            self.configured_provider = Self::detect_provider(&self.storage);
                            self.selected_model =
                                self.storage.get_setting("selected_model").ok().flatten();
                            report.push_str("✓ Config reloaded (ragent.json)\n");
                            self.push_log(LogLevel::Info, "reload config: ragent.json reloaded".to_string());
                        }
                        Err(e) => {
                            report.push_str(&format!("✗ Config reload failed: {}\n", e));
                            self.push_log(LogLevel::Warn, format!("reload config failed: {}", e));
                        }
                    }
                }

                // ── reload mcp ─────────────────────────────────────────────────────
                if do_mcp {
                    match ragent_core::config::Config::load() {
                        Ok(cfg) => {
                            // Rebuild the display list from config, preserving connected status
                            let mut new_servers: Vec<ragent_core::mcp::McpServer> = Vec::new();
                            for (id, mcp_cfg) in &cfg.mcp {
                                let existing_status = self
                                    .mcp_servers
                                    .iter()
                                    .find(|s| &s.id == id)
                                    .map(|s| s.status.clone())
                                    .unwrap_or(if mcp_cfg.disabled {
                                        ragent_core::mcp::McpStatus::Disabled
                                    } else {
                                        ragent_core::mcp::McpStatus::Disabled
                                    });
                                let existing_tools = self
                                    .mcp_servers
                                    .iter()
                                    .find(|s| &s.id == id)
                                    .map(|s| s.tools.clone())
                                    .unwrap_or_default();
                                new_servers.push(ragent_core::mcp::McpServer {
                                    id: id.clone(),
                                    config: mcp_cfg.clone(),
                                    status: existing_status,
                                    tools: existing_tools,
                                });
                            }
                            let prev = self.mcp_servers.len();
                            self.mcp_servers = new_servers;
                            report.push_str(&format!(
                                "✓ MCP reloaded — {} server(s) in config (was {})\n",
                                self.mcp_servers.len(),
                                prev,
                            ));
                            self.push_log(
                                LogLevel::Info,
                                format!(
                                    "reload mcp: {} server(s) in config",
                                    self.mcp_servers.len()
                                ),
                            );
                        }
                        Err(e) => {
                            report.push_str(&format!("✗ MCP reload failed: {}\n", e));
                            self.push_log(LogLevel::Warn, format!("reload mcp failed: {}", e));
                        }
                    }
                }

                // ── reload skills ──────────────────────────────────────────────────
                if do_skills {
                    // Skills are loaded on-demand from disk each time they are needed;
                    // there is no persistent cache to clear.  Just confirm to the user.
                    report.push_str(
                        "✓ Skills will be reloaded from disk on next use (no cache to clear)\n",
                    );
                    self.push_log(LogLevel::Info, "reload skills: confirmed (on-demand)".to_string());
                }

                if !matches!(sub, "all" | "agents" | "config" | "mcp" | "skills") {
                    report.push_str(&format!(
                        "Unknown subcommand '{}'. Usage: /reload [all|config|mcp|skills|agents]\n",
                        sub
                    ));
                }

                self.append_assistant_text(&report);
                if self.current_screen == ScreenMode::Home {
                    self.current_screen = ScreenMode::Chat;
                }
                self.status = "reload".to_string();
            }
            "resume" => {
                if !self.agent_halted {
                    self.status = "Nothing to resume — agent was not halted".to_string();
                    self.push_log(LogLevel::Warn, "Nothing to resume".to_string());
                    return;
                }
                if self.session_id.is_none() {
                    self.status = "No active session".to_string();
                    return;
                }

                self.agent_halted = false;
                let sid = self.session_id.clone().unwrap();
                let resume_text = "You were previously interrupted by the user. Continue the task from where you left off.";
                let msg = Message::user_text(&sid, resume_text);
                self.messages.push(msg);
                self.status = "processing...".to_string();
                self.push_log(LogLevel::Info, "Resuming halted agent".to_string());

                if self.current_screen == ScreenMode::Home {
                    self.current_screen = ScreenMode::Chat;
                }

                let mut agent = self.agent_info.clone();
                if let Some(ref model_str) = self.selected_model {
                    if let Some((provider, model)) = model_str.split_once('/') {
                        agent.model = Some(ModelRef {
                            provider_id: provider.to_string(),
                            model_id: model.to_string(),
                        });
                    }
                }

                let processor = self.session_processor.clone();
                let flag = Arc::new(AtomicBool::new(false));
                self.cancel_flag = Some(flag.clone());
                tokio::spawn(async move {
                    if let Err(e) = processor
                        .process_message(&sid, resume_text, &agent, flag)
                        .await
                    {
                        tracing::debug!(error = %e, "Failed to resume agent");
                    }
                });
            }
            "system" => {
                if args.is_empty() {
                    // Show current system prompt
                    if let Some(ref prompt) = self.agent_info.prompt {
                        self.append_assistant_text(&format!(
                            "From: /system\nCurrent system prompt:\n{prompt}"
                        ));
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
                        format!("System prompt set ({} chars)", args.len()),
                    );
                }
            }
            "tools" => {
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

                let tool_defs = self.session_processor.tool_registry.definitions();
                let mut output = String::from("From: /tools\nBuilt-in Tools:\n\n");
                for def in &tool_defs {
                    output.push_str(&format!("  {:<16} {}\n", def.name, def.description));
                    // Show parameter actions indented under each tool
                    if let Some(props) =
                        def.parameters.get("properties").and_then(|v| v.as_object())
                    {
                        for (param, schema) in props {
                            let desc = schema
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            output.push_str(&format!("    {:<14} {}\n", param, desc));
                        }
                    }
                }

                let connected_servers: Vec<&McpServer> = self
                    .mcp_servers
                    .iter()
                    .filter(|s| s.status == ragent_core::mcp::McpStatus::Connected)
                    .collect();

                if connected_servers.is_empty() {
                    output.push_str("\nMCP Tools:\n\n  (no MCP servers connected)\n");
                } else {
                    output.push_str("\nMCP Tools:\n\n");
                    for server in &connected_servers {
                        for tool in &server.tools {
                            output.push_str(&format!(
                                "  {:<16} [{}] {}\n",
                                tool.name, server.id, tool.description
                            ));
                        }
                    }
                    if connected_servers.iter().all(|s| s.tools.is_empty()) {
                        output.push_str("  (no tools advertised)\n");
                    }
                }

                self.append_assistant_text(&output);
                if self.current_screen == ScreenMode::Home {
                    self.current_screen = ScreenMode::Chat;
                }
                self.status = "tools".to_string();
            }
            "skills" => {
                // Ensure a session exists so the output can be displayed
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
                let working_dir = std::env::current_dir().unwrap_or_default();
                let skill_dirs = ragent_core::config::Config::load()
                    .map(|c| c.skill_dirs)
                    .unwrap_or_default();
                let registry = ragent_core::skill::SkillRegistry::load(&working_dir, &skill_dirs);
                let skills = registry.list_all();

                let mut output = String::from("From: /skills\nRegistered Skills:\n\n");

                if skills.is_empty() {
                    output.push_str("  (no skills found)\n\n");
                    output.push_str("  Skills are loaded from:\n");
                    output.push_str("    Personal:  ~/.ragent/skills/<name>/SKILL.md\n");
                    output.push_str("    Project:   .ragent/skills/<name>/SKILL.md\n");
                } else {
                    // Compute column widths from data
                    let col_cmd = skills
                        .iter()
                        .map(|s| {
                            let hint_len = s.argument_hint.as_ref().map_or(0, |h| h.len() + 1);
                            s.name.len() + 1 + hint_len // +1 for leading '/'
                        })
                        .max()
                        .unwrap_or(7)
                        .max(7); // "Command"
                    let col_scope = 10; // "Scope" header is 5, but values up to 10
                    let col_access = 10; // "Access" header is 6, values up to 10

                    // Header
                    output.push_str(&format!(
                        "  {:<col_cmd$}  {:<col_scope$}  {:<col_access$}  Description\n",
                        "Command",
                        "Scope",
                        "Access",
                        col_cmd = col_cmd,
                        col_scope = col_scope,
                        col_access = col_access,
                    ));
                    // Separator
                    output.push_str(&format!(
                        "  {:-<col_cmd$}  {:-<col_scope$}  {:-<col_access$}  {:-<11}\n",
                        "",
                        "",
                        "",
                        "",
                        col_cmd = col_cmd,
                        col_scope = col_scope,
                        col_access = col_access,
                    ));

                    for skill in &skills {
                        let hint = skill
                            .argument_hint
                            .as_deref()
                            .map(|h| format!(" {h}"))
                            .unwrap_or_default();
                        let cmd_col = format!("/{}{}", skill.name, hint);
                        let scope = format!("{}", skill.scope);
                        let access = match (skill.user_invocable, !skill.disable_model_invocation) {
                            (true, true) => "both",
                            (true, false) => "user-only",
                            (false, true) => "agent-only",
                            (false, false) => "disabled",
                        };
                        let desc = skill.description.as_deref().unwrap_or("(no description)");
                        output.push_str(&format!(
                            "  {:<col_cmd$}  {:<col_scope$}  {:<col_access$}  {}\n",
                            cmd_col,
                            scope,
                            access,
                            desc,
                            col_cmd = col_cmd,
                            col_scope = col_scope,
                            col_access = col_access,
                        ));
                    }
                    output.push_str(&format!("\n  {} skill(s) registered\n", skills.len()));
                }

                self.append_assistant_text(&output);
                if self.current_screen == ScreenMode::Home {
                    self.current_screen = ScreenMode::Chat;
                }
                self.status = "skills".to_string();
            }
            "tasks" => {
                if self.active_tasks.is_empty() {
                    self.status = "No active background tasks".to_string();
                    self.push_log(LogLevel::Info, "No active tasks".to_string());
                    return;
                }

                let mut output = String::from("From: /tasks\nActive Background Tasks:\n\n");
                output.push_str(&format!(
                    "  {:<12}  {:<20}  {:<12}  Description\n",
                    "Task ID", "Agent", "Status"
                ));
                output.push_str(&format!(
                    "  {:-<12}  {:-<20}  {:-<12}  {:-<20}\n",
                    "", "", "", ""
                ));

                for task in &self.active_tasks {
                    let task_id = format!("{}...", &task.id[..8.min(task.id.len())]);
                    let status_str = format!("{}", task.status);
                    output.push_str(&format!(
                        "  {:<12}  {:<20}  {:<12}  {}\n",
                        task_id,
                        task.agent_name,
                        status_str,
                        task.result.as_deref().unwrap_or("(running)")
                    ));
                }

                output.push_str(&format!(
                    "\nTo cancel a task, use: /cancel <task_id_prefix>\n"
                ));
                output.push_str(&format!(
                    "{} task(s) running, {} completed\n",
                    self.active_tasks
                        .iter()
                        .filter(|t| t.status == ragent_core::task::TaskStatus::Running)
                        .count(),
                    self.active_tasks
                        .iter()
                        .filter(|t| t.status == ragent_core::task::TaskStatus::Completed)
                        .count()
                ));

                self.append_assistant_text(&output);
                if self.current_screen == ScreenMode::Home {
                    self.current_screen = ScreenMode::Chat;
                }
                self.status = "tasks".to_string();
            }
            "lsp" => {
                if !self.ensure_session() {
                    return;
                }
                let lsp_args: Vec<&str> = args.split_whitespace().collect();
                let sub = lsp_args.first().copied().unwrap_or("");
                match sub {
                    "discover" => {
                        // Run discovery synchronously using block_in_place.
                        let found = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(LspManager::discover())
                        });
                        // Show interactive discover dialog.
                        self.lsp_discover = Some(LspDiscoverState {
                            servers: found,
                            number_input: String::new(),
                            feedback: None,
                        });
                        if self.current_screen == ScreenMode::Home {
                            self.current_screen = ScreenMode::Chat;
                        }
                        return;
                    }
                    "connect" => {
                        if let Some(&id) = lsp_args.get(1) {
                            if let Some(ref mgr) = self.lsp_manager {
                                let mgr = mgr.clone();
                                let id = id.to_string();
                                let config = ragent_core::config::Config::load()
                                    .ok()
                                    .and_then(|c| c.lsp.get(id.as_str()).cloned());
                                if let Some(cfg) = config {
                                    let id_clone = id.clone();
                                    tokio::spawn(async move {
                                        mgr.write().await.connect(&id_clone, &id_clone, cfg).await;
                                    });
                                    self.status = format!("lsp connecting {}", id);
                                } else {
                                    self.status = format!("LSP '{}' not found in config", id);
                                }
                            } else {
                                self.status = "LSP manager not initialised".to_string();
                            }
                        } else {
                            self.status = "Usage: /lsp connect <id>".to_string();
                        }
                        if self.current_screen == ScreenMode::Home {
                            self.current_screen = ScreenMode::Chat;
                        }
                        return;
                    }
                    "disconnect" => {
                        if let Some(&id) = lsp_args.get(1) {
                            if let Some(ref mgr) = self.lsp_manager {
                                let mgr = mgr.clone();
                                let id = id.to_string();
                                let id_clone = id.clone();
                                tokio::spawn(async move {
                                    let _ = mgr.write().await.disconnect(&id_clone).await;
                                });
                                self.status = format!("lsp disconnecting {}", id);
                            } else {
                                self.status = "LSP manager not initialised".to_string();
                            }
                        } else {
                            self.status = "Usage: /lsp disconnect <id>".to_string();
                        }
                        if self.current_screen == ScreenMode::Home {
                            self.current_screen = ScreenMode::Chat;
                        }
                        return;
                    }
                    _ => {
                        // Show all registered servers and status.
                        let mut out = String::from("From: /lsp\nLSP Servers:\n\n");
                        if self.lsp_servers.is_empty() {
                            out.push_str("  (no LSP servers configured)\n\n");
                            out.push_str("Run /lsp discover to scan for available servers.\n");
                            out.push_str("Then add them to 'lsp' in ragent.json to activate.\n");
                        } else {
                            for s in &self.lsp_servers {
                                let status_icon = match &s.status {
                                    LspStatus::Connected => "🟢 connected",
                                    LspStatus::Starting => "🟡 starting",
                                    LspStatus::Disabled => "⚪ disabled",
                                    LspStatus::Failed { error } => &format!("🔴 failed: {}", error),
                                };
                                out.push_str(&format!("  {:<18} {}\n", s.id, status_icon));
                                if let Some(ref caps) = s.capabilities_summary {
                                    out.push_str(&format!("    capabilities: {}\n", caps));
                                }
                                if !s.config.extensions.is_empty() {
                                    out.push_str(&format!(
                                        "    extensions:   {}\n",
                                        s.config.extensions.join(", ")
                                    ));
                                }
                            }
                            let connected = self
                                .lsp_servers
                                .iter()
                                .filter(|s| s.status == LspStatus::Connected)
                                .count();
                            out.push_str(&format!(
                                "\n{}/{} server(s) connected\n",
                                connected,
                                self.lsp_servers.len()
                            ));
                        }
                        out.push_str("\nSubcommands: /lsp discover  /lsp connect <id>  /lsp disconnect <id>\n");
                        self.append_assistant_text(&out);
                    }
                }
                if self.current_screen == ScreenMode::Home {
                    self.current_screen = ScreenMode::Chat;
                }
                self.status = "lsp".to_string();
            }
            "mcp" => {
                if !self.ensure_session() {
                    return;
                }
                let mcp_args: Vec<&str> = args.split_whitespace().collect();
                let sub = mcp_args.first().copied().unwrap_or("");
                match sub {
                    "discover" => {
                        // Run discovery synchronously using block_in_place.
                        let found = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(McpClient::discover())
                        });
                        // Show interactive discover dialog.
                        self.mcp_discover = Some(McpDiscoverState {
                            servers: found,
                            number_input: String::new(),
                            feedback: None,
                        });
                        if self.current_screen == ScreenMode::Home {
                            self.current_screen = ScreenMode::Chat;
                        }
                        return;
                    }
                    "connect" => {
                        if let Some(&id) = mcp_args.get(1) {
                            let config = ragent_core::config::Config::load()
                                .ok()
                                .and_then(|c| c.mcp.get(id).cloned());
                            if let Some(_cfg) = config {
                                self.status =
                                    format!("MCP connect not yet implemented for '{}'", id);
                            } else {
                                self.status = format!("MCP '{}' not found in config", id);
                            }
                        } else {
                            self.status = "Usage: /mcp connect <id>".to_string();
                        }
                        if self.current_screen == ScreenMode::Home {
                            self.current_screen = ScreenMode::Chat;
                        }
                        return;
                    }
                    "disconnect" => {
                        if let Some(&id) = mcp_args.get(1) {
                            self.status =
                                format!("MCP disconnect not yet implemented for '{}'", id);
                        } else {
                            self.status = "Usage: /mcp disconnect <id>".to_string();
                        }
                        if self.current_screen == ScreenMode::Home {
                            self.current_screen = ScreenMode::Chat;
                        }
                        return;
                    }
                    _ => {
                        // Show all registered servers and status.
                        let mut out = String::from("From: /mcp\nMCP Servers:\n\n");
                        if self.mcp_servers.is_empty() {
                            out.push_str("  (no MCP servers configured)\n\n");
                            out.push_str("Run /mcp discover to scan for available servers.\n");
                            out.push_str("Then add them to 'mcp' in ragent.json to activate.\n");
                        } else {
                            for s in &self.mcp_servers {
                                let status_icon = match &s.status {
                                    ragent_core::mcp::McpStatus::Connected => "🟢 connected",
                                    ragent_core::mcp::McpStatus::Disabled => "⚪ disabled",
                                    ragent_core::mcp::McpStatus::NeedsAuth => "🟡 needs auth",
                                    ragent_core::mcp::McpStatus::Failed { error } => {
                                        &format!("🔴 failed: {}", error)
                                    }
                                };
                                out.push_str(&format!("  {:<18} {}\n", s.id, status_icon));
                                if !s.tools.is_empty() {
                                    out.push_str(&format!("    tools: {}\n", s.tools.len()));
                                }
                            }
                            let connected = self
                                .mcp_servers
                                .iter()
                                .filter(|s| s.status == ragent_core::mcp::McpStatus::Connected)
                                .count();
                            out.push_str(&format!(
                                "\n{}/{} server(s) connected\n",
                                connected,
                                self.mcp_servers.len()
                            ));
                        }
                        out.push_str("\nSubcommands: /mcp discover  /mcp connect <id>  /mcp disconnect <id>\n");
                        self.append_assistant_text(&out);
                    }
                }
                if self.current_screen == ScreenMode::Home {
                    self.current_screen = ScreenMode::Chat;
                }
                self.status = "mcp".to_string();
            }
            "team" => {
                // Split "subcommand rest-of-args"
                let (sub, rest) = args
                    .split_once(char::is_whitespace)
                    .map_or((args, ""), |(s, r)| (s.trim(), r.trim()));
                let sub = if sub.is_empty() { "status" } else { sub };

                match sub {
                    "status" | "" => {
                        let mut output = String::from("From: /team status\n");
                        if let Some(team) = self.active_team.clone() {
                            self.ensure_team_manager_for_team(&team.name, None);
                            output.push_str(&format!(
                                "## Team: {} ({})\n\n",
                                team.name,
                                format!("{:?}", team.status).to_lowercase()
                            ));
                            output.push_str(&format!(
                                "  ● lead (you)  session: {}\n",
                                team.lead_session_id
                            ));
                            if self.team_members.is_empty() {
                                output.push_str(
                                    "  (no teammates yet — use team_spawn tool or /team create)\n",
                                );
                            } else {
                                for m in &self.team_members {
                                    let status = format!("{:?}", m.status).to_lowercase();
                                    let task =
                                        m.current_task_id.as_deref().unwrap_or("—").to_string();
                                    output.push_str(&format!(
                                        "  └ {:<18} {:<10} task:{}\n",
                                        m.name, status, task
                                    ));
                                }
                            }
                            output.push_str(&format!(
                                "\n{} teammate(s)\n",
                                self.team_members.len()
                            ));
                        } else {
                            output.push_str("No active team.\n\nUse `/team create <name>` to start a team.");
                        }
                        self.append_assistant_text(&output);
                        self.status = "team: status".to_string();
                    }
                    "create" => {
                        if !self.ensure_session() {
                            return;
                        }
                        if rest.is_empty() {
                            self.status = "Usage: /team create <name>".to_string();
                            return;
                        }
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let sid = self.session_id.clone().unwrap_or_default();
                        match TeamStore::create(rest, &sid, &working_dir, true) {
                            Ok(store) => {
                                let name = store.config.name.clone();
                                let team_dir = store.dir.clone();
                                self.active_team = Some(store.config);
                                self.team_members.clear();
                                self.show_teams = true;
                                self.ensure_team_manager_for_team(&name, Some(team_dir));
                                self.push_log(
                                    LogLevel::Info,
                                    format!("🤝 Team '{}' created", name),
                                );
                                self.append_assistant_text(&format!(
                                    "From: /team create\nTeam '{}' created.\n\n\
                                     Use the `team_spawn` tool to add teammates.",
                                    name
                                ));
                                self.status = format!("team: {}", name);
                            }
                            Err(e) => {
                                self.status = format!("Failed to create team: {}", e);
                                self.push_log(
                                    LogLevel::Error,
                                    format!("team create failed: {}", e),
                                );
                            }
                        }
                    }
                    "open" => {
                        if !self.ensure_session() {
                            return;
                        }
                        if rest.is_empty() {
                            self.status = "Usage: /team open <name>".to_string();
                            return;
                        }
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        match TeamStore::load_by_name(rest, &working_dir) {
                            Ok(store) => {
                                let name = store.config.name.clone();
                                let team_dir = store.dir.clone();
                                self.team_members = store.config.members.clone();
                                self.active_team = Some(store.config);
                                self.show_teams = true;
                                self.ensure_team_manager_for_team(&name, Some(team_dir));
                                self.push_log(LogLevel::Info, format!("🤝 Team '{}' opened", name));
                                self.append_assistant_text(&format!(
                                    "From: /team open\nTeam '{}' opened.\n\nUse `/team status` to inspect teammates and tasks.",
                                    name
                                ));
                                self.status = format!("team: {}", name);
                            }
                            Err(e) => {
                                self.status = format!("Failed to open team: {}", e);
                                self.push_log(
                                    LogLevel::Error,
                                    format!("team open failed for '{}': {}", rest, e),
                                );
                            }
                        }
                    }
                    "close" => {
                        if let Some(team) = self.active_team.as_ref() {
                            let team_name = team.name.clone();
                            self.active_team = None;
                            self.team_members.clear();
                            self.show_teams = false;
                            self.push_log(
                                LogLevel::Info,
                                format!("🤝 Team '{}' closed for this session", team_name),
                            );
                            self.append_assistant_text(&format!(
                                "From: /team close\nTeam '{}' closed for this session.",
                                team_name
                            ));
                            self.status = "team closed".to_string();
                        } else {
                            self.status = "No active team to close".to_string();
                        }
                    }
                    "delete" => {
                        if rest.is_empty() {
                            self.status = "Usage: /team delete <name>".to_string();
                            return;
                        }
                        let deleting_active = self
                            .active_team
                            .as_ref()
                            .is_some_and(|team| team.name == rest);
                        if deleting_active {
                            let active_count = self
                                .team_members
                                .iter()
                                .filter(|m| matches!(m.status, MemberStatus::Working))
                                .count();
                            if active_count > 0 {
                                self.status = format!(
                                    "{} teammate(s) still active — shut them down first",
                                    active_count
                                );
                                return;
                            }
                        }
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        match TeamStore::load_by_name(rest, &working_dir) {
                            Ok(store) => match std::fs::remove_dir_all(&store.dir) {
                                Ok(_) => {
                                    if deleting_active {
                                        self.active_team = None;
                                        self.team_members.clear();
                                        self.show_teams = false;
                                    }
                                    self.push_log(
                                        LogLevel::Info,
                                        format!("🗑️  Team '{}' deleted", rest),
                                    );
                                    self.append_assistant_text(&format!(
                                        "From: /team delete\nTeam '{}' deleted.",
                                        rest
                                    ));
                                    self.status = "team deleted".to_string();
                                }
                                Err(e) => {
                                    self.status = format!("Failed to delete team: {e}");
                                    self.push_log(
                                        LogLevel::Error,
                                        format!("team delete failed for '{}': {}", rest, e),
                                    );
                                }
                            },
                            Err(e) => {
                                self.status = format!("Failed to load team: {e}");
                                self.push_log(
                                    LogLevel::Error,
                                    format!("team delete failed for '{}': {}", rest, e),
                                );
                            }
                        }
                    }
                    "message" => {
                        let (name, text) = rest
                            .split_once(char::is_whitespace)
                            .map_or((rest, ""), |(n, t)| (n.trim(), t.trim()));
                        if name.is_empty() || text.is_empty() {
                            self.status =
                                "Usage: /team message <teammate-name> <text>".to_string();
                            return;
                        }
                        let member = self.team_members.iter().find(|m| m.name == name).cloned();
                        match (self.active_team.clone(), member) {
                            (Some(team), Some(member)) => {
                                let working_dir = std::env::current_dir().unwrap_or_default();
                                match TeamStore::load_by_name(&team.name, &working_dir) {
                                    Ok(store) => {
                                        match Mailbox::open(&store.dir, &member.agent_id) {
                                            Ok(mb) => {
                                                let msg = MailboxMessage::new(
                                                    "lead",
                                                    &member.agent_id,
                                                    MessageType::Message,
                                                    text,
                                                );
                                                match mb.push(msg) {
                                                    Ok(_) => {
                                                        self.push_log(
                                                            LogLevel::Info,
                                                            format!(
                                                                "📨 lead → {name}: {text}"
                                                            ),
                                                        );
                                                        self.status =
                                                            format!("message sent to {name}");
                                                    }
                                                    Err(e) => {
                                                        self.status = format!(
                                                            "Failed to send message: {e}"
                                                        );
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                self.status =
                                                    format!("Failed to open mailbox: {e}");
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        self.status = format!("Failed to load team: {e}");
                                    }
                                }
                            }
                            (None, _) => {
                                self.status = "No active team".to_string();
                            }
                            (Some(_), None) => {
                                self.status = format!("Teammate '{name}' not found");
                            }
                        }
                    }
                    "tasks" => {
                        let team_opt = self.active_team.clone();
                        if let Some(team) = team_opt {
                            let working_dir = std::env::current_dir().unwrap_or_default();
                            match TeamStore::load_by_name(&team.name, &working_dir) {
                                Ok(store) => match store.task_store() {
                                    Ok(task_store) => match task_store.read() {
                                        Ok(task_list) => {
                                            let mut output = format!(
                                                "From: /team tasks\n## Tasks — team '{}'\n\n",
                                                team.name
                                            );
                                            if task_list.tasks.is_empty() {
                                                output.push_str("  (no tasks)\n");
                                            } else {
                                                output.push_str(&format!(
                                                    "  {:<12}  {:<34}  {:<12}  {}\n",
                                                    "ID", "Title", "Status", "Assignee"
                                                ));
                                                output.push_str(&format!(
                                                    "  {:-<12}  {:-<34}  {:-<12}  {:-<16}\n",
                                                    "", "", "", ""
                                                ));
                                                for task in &task_list.tasks {
                                                    let status = match task.status {
                                                        TaskStatus::Pending => "pending",
                                                        TaskStatus::InProgress => "in-progress",
                                                        TaskStatus::Completed => "completed",
                                                        TaskStatus::Cancelled => "cancelled",
                                                    };
                                                    let assignee = task
                                                        .assigned_to
                                                        .as_deref()
                                                        .unwrap_or("—");
                                                    output.push_str(&format!(
                                                        "  {:<12}  {:<34}  {:<12}  {}\n",
                                                        task.id, task.title, status, assignee
                                                    ));
                                                }
                                            }
                                            self.append_assistant_text(&output);
                                            self.status =
                                                format!("{} task(s)", task_list.tasks.len());
                                        }
                                        Err(e) => {
                                            self.status = format!("Failed to read tasks: {e}");
                                        }
                                    },
                                    Err(e) => {
                                        self.status = format!("Failed to open task store: {e}");
                                    }
                                },
                                Err(e) => {
                                    self.status = format!("Failed to load team: {e}");
                                }
                            }
                        } else {
                            self.append_assistant_text(
                                "From: /team tasks\nNo active team.",
                            );
                            self.status = "no active team".to_string();
                        }
                    }
                    "clear" => {
                        let team_opt = self.active_team.clone();
                        if let Some(team) = team_opt {
                            let working_dir = std::env::current_dir().unwrap_or_default();
                            match TeamStore::load_by_name(&team.name, &working_dir) {
                                Ok(store) => {
                                    let tasks_path = store.dir.join("tasks.json");
                                    let cleared_count = store
                                        .task_store()
                                        .ok()
                                        .and_then(|s| s.read().ok())
                                        .map(|l| l.tasks.len())
                                        .unwrap_or(0);
                                    let clear_result = if tasks_path.exists() {
                                        std::fs::remove_file(&tasks_path)
                                    } else {
                                        Ok(())
                                    };
                                    match clear_result {
                                        Ok(_) => {
                                            self.append_assistant_text(&format!(
                                                "From: /team clear\nCleared {} task(s) for team '{}'.",
                                                cleared_count, team.name
                                            ));
                                            self.push_log(
                                                LogLevel::Info,
                                                format!(
                                                    "🧹 Cleared {} task(s) from team '{}'",
                                                    cleared_count, team.name
                                                ),
                                            );
                                            self.status = "team tasks cleared".to_string();
                                        }
                                        Err(e) => {
                                            self.status = format!("Failed to clear tasks: {e}");
                                            self.push_log(
                                                LogLevel::Error,
                                                format!("team clear failed for '{}': {}", team.name, e),
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    self.status = format!("Failed to load team: {e}");
                                }
                            }
                        } else {
                            self.append_assistant_text(
                                "From: /team clear\nNo active team.",
                            );
                            self.status = "no active team".to_string();
                        }
                    }
                    "cleanup" => {
                        let team_opt = self.active_team.clone();
                        if let Some(team) = team_opt {
                            // Guard: ensure no teammates are still working.
                            let active_count = self
                                .team_members
                                .iter()
                                .filter(|m| matches!(m.status, MemberStatus::Working))
                                .count();
                            if active_count > 0 {
                                self.status = format!(
                                    "{} teammate(s) still active — shut them down first",
                                    active_count
                                );
                                return;
                            }
                            let working_dir = std::env::current_dir().unwrap_or_default();
                            let team_name = team.name.clone();
                            let removed = match TeamStore::load_by_name(&team_name, &working_dir) {
                                Ok(store) => std::fs::remove_dir_all(&store.dir).is_ok(),
                                Err(_) => false,
                            };
                            self.active_team = None;
                            self.team_members.clear();
                            self.show_teams = false;
                            if removed {
                                self.push_log(
                                    LogLevel::Info,
                                    format!("🗑️  Team '{team_name}' cleaned up"),
                                );
                                self.append_assistant_text(&format!(
                                    "From: /team cleanup\nTeam '{team_name}' cleaned up."
                                ));
                            } else {
                                self.push_log(
                                    LogLevel::Warn,
                                    format!("Team '{team_name}' state cleared (dir not found)"),
                                );
                                self.append_assistant_text(&format!(
                                    "From: /team cleanup\nTeam '{team_name}' state cleared."
                                ));
                            }
                            self.status = "team cleaned up".to_string();
                        } else {
                            self.status = "No active team to clean up".to_string();
                        }
                    }
                    _ => {
                        self.status = format!(
                            "Unknown /team subcommand '{}'. Usage: /team [status|create <name>|open <name>|close|delete <name>|message <name> <text>|tasks|clear|cleanup]",
                            sub
                        );
                        self.push_log(
                            LogLevel::Warn,
                            format!("unknown /team subcommand: {}", sub),
                        );
                    }
                }
                if self.current_screen == ScreenMode::Home {
                    self.current_screen = ScreenMode::Chat;
                }
            }
            "todos" => {
                if !self.ensure_session() {
                    return;
                }
                let session_id = self.session_id.clone().unwrap();
                let storage = self.session_processor.session_manager.storage();

                // Fetch todos from storage
                let status_filter = if args.is_empty() { None } else { Some(args) };
                match storage.get_todos(&session_id, status_filter) {
                    Ok(todos) => {
                        let mut output = String::from("From: /todo_list\n");
                        if todos.is_empty() {
                            output.push_str("No TODO items found");
                            if let Some(filter) = status_filter {
                                output.push_str(&format!(" with status '{filter}'"));
                            }
                            output.push_str(".\n");
                        } else {
                            output.push_str(&format!("## TODOs ({} items)\n\n", todos.len()));
                            for todo in &todos {
                                let status_icon = match todo.status.as_str() {
                                    "pending" => "⏳",
                                    "in_progress" => "🔄",
                                    "done" => "✅",
                                    "blocked" => "🚫",
                                    _ => "❓",
                                };
                                output.push_str(&format!(
                                    "- {} **{}** — {} `[{}]`\n",
                                    status_icon, todo.id, todo.title, todo.status
                                ));
                                if !todo.description.is_empty() {
                                    output.push_str(&format!("  {}\n", todo.description));
                                }
                            }
                        }
                        self.append_assistant_text(&output);
                        if self.current_screen == ScreenMode::Home {
                            self.current_screen = ScreenMode::Chat;
                        }
                        self.status = format!("{} todo(s)", todos.len());
                    }
                    Err(e) => {
                        self.status = format!("Failed to read todos: {}", e);
                        self.push_log(LogLevel::Error, format!("todo_list error: {}", e));
                    }
                }
            }
            _ => {
                // Check if this is a skill invocation before reporting unknown command.
                let working_dir = std::env::current_dir().unwrap_or_default();
                let skill_dirs = ragent_core::config::Config::load()
                    .map(|c| c.skill_dirs)
                    .unwrap_or_default();
                let registry = ragent_core::skill::SkillRegistry::load(&working_dir, &skill_dirs);
                if let Some(skill) = registry.get(cmd) {
                    if !skill.user_invocable {
                        self.status = format!("Skill '{}' is not user-invocable", cmd);
                        self.push_log(
                            LogLevel::Warn,
                            format!("Skill /{} is not user-invocable", cmd),
                        );
                        return;
                    }
                    // Check provider/model are configured
                    if self.configured_provider.is_none() {
                        self.status =
                            "⚠ No provider configured — use /provider to set up".to_string();
                        return;
                    }
                    if self.selected_model.is_none() {
                        self.status = "⚠ No model selected — use /model to choose".to_string();
                        return;
                    }
                    // Ensure a session exists
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
                    if self.current_screen == ScreenMode::Home {
                        self.current_screen = ScreenMode::Chat;
                    }

                    let sid = self.session_id.clone().unwrap_or_default();
                    let skill = skill.clone();
                    let args_owned = args.to_string();
                    let processor = self.session_processor.clone();

                    let mut agent = self.agent_info.clone();
                    // Apply skill model override if present, otherwise use selected model
                    if let Some(ref model_str) =
                        skill.model.as_ref().or(self.selected_model.as_ref())
                    {
                        if let Some((provider, model)) = model_str.split_once('/') {
                            agent.model = Some(ModelRef {
                                provider_id: provider.to_string(),
                                model_id: model.to_string(),
                            });
                        }
                    }

                    self.status = format!("invoking skill /{}…", cmd);
                    self.push_log(
                        LogLevel::Info,
                        format!("Invoking skill /{} with args: {}", cmd, args),
                    );

                    // Show the skill invocation as a user message in the chat
                    let display_text = if args.is_empty() {
                        format!("/{}", cmd)
                                          } else {
                                              format!("/{} {}", cmd, args)
                                          };
                                          let user_msg = Message::user_text(&sid, &display_text);
                                          self.messages.push(user_msg);
                                          self.add_to_history(display_text);
                    
                                          let flag = Arc::new(AtomicBool::new(false));                    self.cancel_flag = Some(flag.clone());

                    tokio::spawn(async move {
                        match ragent_core::skill::invoke::invoke_skill(
                            &skill,
                            &args_owned,
                            &sid,
                            &working_dir,
                        )
                        .await
                        {
                            Ok(invocation) => {
                                if invocation.forked {
                                    // Execute in an isolated sub-session
                                    match ragent_core::skill::invoke::invoke_forked_skill(
                                        &invocation,
                                        &processor,
                                        &sid,
                                        &working_dir,
                                        flag,
                                    )
                                    .await
                                    {
                                        Ok(result) => {
                                            tracing::info!(
                                                skill = %result.skill_name,
                                                forked_session = %result.forked_session_id,
                                                "Forked skill completed"
                                            );
                                            // The forked result is already displayed via events;
                                            // no additional process_message call needed.
                                        }
                                        Err(e) => {
                                            tracing::debug!(
                                                error = %e,
                                                "Failed to execute forked skill"
                                            );
                                        }
                                    }
                                } else {
                                    let message = ragent_core::skill::invoke::format_skill_message(
                                        &invocation,
                                    );
                                    if let Err(e) = processor
                                        .process_message(&sid, &message, &agent, flag)
                                        .await
                                    {
                                        tracing::debug!(
                                            error = %e,
                                            "Failed to process skill message"
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::debug!(error = %e, "Failed to invoke skill");
                            }
                        }
                    });
                } else {
                    self.status = format!("Unknown command: /{}", cmd);
                    self.push_log(LogLevel::Warn, format!("Unknown command: /{}", cmd));
                }
            }
        }
    }

    /// Handle a key event while the history picker overlay is open.
    fn handle_history_picker_key(&mut self, key: KeyEvent) {
        use crossterm::event::KeyCode;
        let picker = match self.history_picker.as_mut() {
            Some(p) => p,
            None => return,
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.history_picker = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if picker.selected > 0 {
                    picker.selected -= 1;
                    if picker.selected < picker.scroll_offset {
                        picker.scroll_offset = picker.selected;
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if picker.selected + 1 < picker.entries.len() {
                    picker.selected += 1;
                }
            }
            KeyCode::Enter => {
                let chosen = picker.entries[picker.selected].clone();
                self.history_picker = None;
                self.input = chosen;
                self.input_cursor = self.input.len();
            }
            _ => {}
        }
    }

    /// Process a terminal mouse event (scroll wheel, scrollbar drag, text selection).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use crossterm::event::{MouseEvent, MouseEventKind, MouseButton};
    /// # use ragent_tui::App;
    /// # fn example(app: &mut App) {
    /// let event = MouseEvent {
    ///     kind: MouseEventKind::ScrollUp,
    ///     column: 10,
    ///     row: 5,
    ///     modifiers: crossterm::event::KeyModifiers::NONE,
    /// };
    /// app.handle_mouse_event(event);
    /// # }
    /// ```
    pub fn handle_mouse_event(&mut self, event: MouseEvent) {
        // If context menu is open, intercept clicks.
        if self.context_menu.is_some() {
            if let MouseEventKind::Down(MouseButton::Left) = event.kind {
                self.handle_context_menu_click(event.column, event.row);
            } else if let MouseEventKind::Down(MouseButton::Right) = event.kind {
                // Second right-click dismisses the menu.
                self.context_menu = None;
            }
            return;
        }

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
            MouseEventKind::Down(MouseButton::Left) => {
                let pos = (event.column, event.row);
                // Scrollbar drag takes priority (rightmost column of pane)
                if self.message_area.height > 0
                    && event.column == self.message_area.right().saturating_sub(1)
                    && self.message_area.contains(pos.into())
                    && self.message_max_scroll > 0
                {
                    self.scrollbar_drag = Some(ScrollbarDragPane::Messages);
                    self.text_selection = None;
                    self.apply_scrollbar_drag(event.row, ScrollbarDragPane::Messages);
                } else if self.show_log
                    && self.log_area.height > 0
                    && event.column == self.log_area.right().saturating_sub(1)
                    && self.log_area.contains(pos.into())
                    && self.log_max_scroll > 0
                {
                    self.scrollbar_drag = Some(ScrollbarDragPane::Log);
                    self.text_selection = None;
                    self.apply_scrollbar_drag(event.row, ScrollbarDragPane::Log);
                                } else {
                                      // If the file menu is open and the click falls within its popup,
                                      // handle file/directory selection via mouse.
                                      if let Some(_menu_state) = self.file_menu.as_ref() {
                                          // Compute popup rect used by the renderer so clicks map to rows.
                                      }
                
                                      if self.file_menu.is_some() {
                                          // Recompute popup geometry identical to layout::render_file_menu
                                          let menu = self.file_menu.as_ref().unwrap();
                                          let item_count = menu.matches.len() as u16;
                                          let height = (item_count + 2).min(self.input_area.y);
                                          let width = self.input_area.width.min(60);
                                          let popup_x = self.input_area.x;
                                          let popup_y = self.input_area.y.saturating_sub(height);
                
                                          // If click is inside the popup, determine which row was clicked.
                                          if event.column >= popup_x
                                              && event.column < popup_x.saturating_add(width)
                                              && event.row >= popup_y
                                              && event.row < popup_y.saturating_add(height)
                                          {
                                              // Content lines start one row below the popup top (inside the border)
                                              let clicked_row = event.row.saturating_sub(popup_y + 1) as usize;
                                              if clicked_row < menu.matches.len() {
                                                  // Set the selected index (drop borrow immediately)
                                                  {
                                                      if let Some(ref mut m) = self.file_menu.as_mut() {
                                                          m.selected = clicked_row;
                                                      }
                                                  }
                
                                                  // Accept the selection: this will navigate into directories
                                                  // or insert a file path into the input. We do not auto-send
                                                  // the message on mouse click; pressing Enter still sends.
                                                  let _ = self.accept_file_menu_selection();
                                                  return;
                                              }
                                          }
                                      }
                
                                      // Start text selection in whichever pane the click is in
                                      let pane = self.pane_at(event.column, event.row);
                                      if let Some(pane) = pane {
                                          self.text_selection = Some(TextSelection {
                                              pane,
                                              anchor: pos,
                                              endpoint: pos,
                                          });
                                      } else {
                                          self.text_selection = None;
                                      }
                                  }            }
                          MouseEventKind::Drag(MouseButton::Left) => {
                              if let Some(pane) = self.scrollbar_drag {
                                  self.apply_scrollbar_drag(event.row, pane);
                              } else if let Some(ref mut sel) = self.text_selection {
                                  sel.endpoint = (event.column, event.row);
                              }
                          }
            
                          // Mouse move -> used for hover highlighting of file menu entries
                          MouseEventKind::Moved => {
                              // If file menu is open, update the highlighted row under the cursor.
                              if self.file_menu.is_some() {
                                  // Snapshot needed values without holding immutable borrows while mutating.
                                  let item_count = self.file_menu.as_ref().map(|m| m.matches.len()).unwrap_or(0) as u16;
                                  let height = (item_count + 2).min(self.input_area.y);
                                  let width = self.input_area.width.min(60);
                                  let popup_x = self.input_area.x;
                                  let popup_y = self.input_area.y.saturating_sub(height);
            
                                  if event.column >= popup_x
                                      && event.column < popup_x.saturating_add(width)
                                      && event.row >= popup_y
                                      && event.row < popup_y.saturating_add(height)
                                  {
                                      let hovered_row = event.row.saturating_sub(popup_y + 1) as usize;
                                      if hovered_row < (item_count as usize) {
                                          // Update selection if changed.
                                          if let Some(ref mut m) = self.file_menu.as_mut() {
                                              if m.selected != hovered_row {
                                                  m.selected = hovered_row;
                                              }
                                          }
                                      }
                                  }
                              }
                          }
            
                          MouseEventKind::Up(MouseButton::Left) => {
                              self.scrollbar_drag = None;
                              // Keep text_selection alive so it stays highlighted until right-click or next click
                          }
                          MouseEventKind::Down(MouseButton::Right) => {
                // Right-click copies the active selection (if any) and clears it.
                if self.text_selection.is_some() {
                    self.copy_selection();
                    return;
                }

                let col = event.column;
                let row = event.row;
                let pane = self.pane_at(col, row).unwrap_or(SelectionPane::Messages);

                // Determine available actions based on context.
                let has_selection = self.text_selection.is_some();
                let in_input = matches!(pane, SelectionPane::Input | SelectionPane::HomeInput);
                let has_clipboard = Self::get_clipboard().map_or(false, |s| !s.is_empty());

                // Cut: only in input panes with selection
                // Copy: anywhere with selection
                // Paste: only in input panes with clipboard content
                let items = vec![
                    (ContextAction::Cut, has_selection && in_input),
                    (ContextAction::Copy, has_selection),
                    (ContextAction::Paste, in_input && has_clipboard),
                ];

                // Find first enabled item as default selection
                let selected = items.iter().position(|(_, en)| *en).unwrap_or(0);

                self.context_menu = Some(ContextMenuState {
                    x: col,
                    y: row,
                    pane,
                    selected,
                    items,
                });
            }
            _ => {}
        }
    }

    /// Determine which selection pane a screen position falls in.
    fn pane_at(&self, col: u16, row: u16) -> Option<SelectionPane> {
        let pos = (col, row).into();
        if self.message_area.area() > 0 && self.message_area.contains(pos) {
            Some(SelectionPane::Messages)
        } else if self.show_log && self.log_area.area() > 0 && self.log_area.contains(pos) {
            Some(SelectionPane::Log)
        } else if self.input_area.area() > 0 && self.input_area.contains(pos) {
            Some(SelectionPane::Input)
        } else if self.home_input_area.area() > 0 && self.home_input_area.contains(pos) {
            Some(SelectionPane::HomeInput)
        } else {
            None
        }
    }

    /// Copy the currently selected text to the system clipboard.
    fn copy_selection(&mut self) {
        let sel = match self.text_selection.take() {
            Some(s) => s,
            None => return,
        };
        let ((start_col, start_row), (end_col, end_row)) = sel.normalized();

        let lines: &[String] = match sel.pane {
            SelectionPane::Messages => &self.message_content_lines,
            SelectionPane::Log => &self.log_content_lines,
            SelectionPane::Input | SelectionPane::HomeInput => {
                // For input widgets, build a single-line content from app.input
                let input_text = format!("> {}", self.input);
                let area = if sel.pane == SelectionPane::Input {
                    self.input_area
                } else {
                    self.home_input_area
                };
                let inner_x = area.x + 1; // inside border
                let inner_y = area.y + 1;
                let inner_w = area.width.saturating_sub(2).max(1) as usize;
                // Wrap the input text into display lines
                let wrapped: Vec<String> = input_text
                    .as_bytes()
                    .chunks(inner_w)
                    .map(|c| String::from_utf8_lossy(c).into_owned())
                    .collect();
                let text = Self::extract_text_from_lines(
                    &wrapped, inner_x, inner_y, start_col, start_row, end_col, end_row,
                );
                if !text.is_empty() {
                    Self::set_clipboard(&text);
                    self.push_log(LogLevel::Info, format!("Copied {} chars", text.len()));
                }
                return;
            }
        };

        let area = match sel.pane {
            SelectionPane::Messages => self.message_area,
            SelectionPane::Log => self.log_area,
            _ => unreachable!(),
        };

        // Inner area (accounting for borders)
        let inner_x = if sel.pane == SelectionPane::Messages {
            area.x + 1 // LEFT border only
        } else {
            area.x + 1 // ALL borders
        };
        let inner_y = if sel.pane == SelectionPane::Messages {
            area.y // no top border on messages (LEFT|RIGHT only)
        } else {
            area.y + 1 // ALL borders on log panel
        };

        let text = Self::extract_text_from_lines(
            lines, inner_x, inner_y, start_col, start_row, end_col, end_row,
        );

        if !text.is_empty() {
            Self::set_clipboard(&text);
            self.push_log(LogLevel::Info, format!("Copied {} chars", text.len()));
        }
    }

    /// Extract text from cached content lines given screen coordinates.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ragent_tui::App;
    ///
    /// let lines = vec![
    ///     "Hello, world!".to_string(),
    ///     "Second line".to_string(),
    /// ];
    /// let text = App::extract_text_from_lines(&lines, 0, 0, 0, 0, 4, 0);
    /// assert_eq!(text, "Hello");
    /// ```
    pub fn extract_text_from_lines(
        lines: &[String],
        inner_x: u16,
        inner_y: u16,
        start_col: u16,
        start_row: u16,
        end_col: u16,
        end_row: u16,
    ) -> String {
        let mut result = String::new();
        for screen_row in start_row..=end_row {
            let line_idx = screen_row.saturating_sub(inner_y) as usize;
            let line = lines.get(line_idx).map(|s| s.as_str()).unwrap_or("");
            let line_start = if screen_row == start_row {
                let byte_idx = start_col.saturating_sub(inner_x) as usize;
                // Snap to a valid char boundary
                let mut idx = byte_idx.min(line.len());
                while idx < line.len() && !line.is_char_boundary(idx) {
                    idx += 1;
                }
                idx
            } else {
                0
            };
            let line_end = if screen_row == end_row {
                let byte_idx = (end_col.saturating_sub(inner_x) as usize + 1).min(line.len());
                let mut idx = byte_idx;
                while idx < line.len() && !line.is_char_boundary(idx) {
                    idx += 1;
                }
                idx
            } else {
                line.len()
            };
            if line_start < line.len() {
                result.push_str(&line[line_start..line_end.min(line.len())]);
            }
            if screen_row < end_row {
                result.push('\n');
            }
        }
        result
    }

    /// Copy text to the system clipboard (spawns thread to avoid blocking).
    fn set_clipboard(text: &str) {
        let text = text.to_owned();
        std::thread::spawn(move || {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                #[cfg(target_os = "linux")]
                {
                    use arboard::SetExtLinux;
                    let _ = clipboard.set().wait().text(&text);
                }
                #[cfg(not(target_os = "linux"))]
                {
                    let _ = clipboard.set_text(&text);
                }
            }
        });
    }

    /// Read the current clipboard contents synchronously.
    fn get_clipboard() -> Option<String> {
        arboard::Clipboard::new()
            .ok()
            .and_then(|mut cb| cb.get_text().ok())
    }

    /// Attempt to paste an image from the clipboard and stage it as an attachment.
    ///
    /// Two clipboard formats are handled:
    /// 1. **Text containing a `file://` URI or an absolute path** pointing to an existing
    ///    image file — the path is used directly (no copy needed).
    /// 2. **Raw RGBA pixel data** (`arboard::ImageData`) — encoded as a PNG and saved to a
    ///    temporary file which is then staged.
    ///
    /// If neither format yields an image the caller is notified via the log.
    pub fn paste_image_from_clipboard(&mut self) {
        // --- Phase 1: look for a file reference in the text clipboard ---
        if let Some(text) = Self::get_clipboard() {
            let trimmed = text.trim();

            // Resolve file:// URI
            let candidate = if let Some(rest) = trimmed.strip_prefix("file://") {
                Some(std::path::PathBuf::from(percent_decode_path(rest)))
            } else if trimmed.starts_with('/') || trimmed.starts_with('.') {
                // Plain absolute or relative path
                Some(std::path::PathBuf::from(trimmed))
            } else {
                None
            };

            if let Some(path) = candidate {
                if path.exists() && is_image_path(&path) {
                    self.push_log(
                        LogLevel::Info,
                        format!("📎 Image attached from clipboard path: {}", path.display()),
                    );
                    self.pending_attachments.push(path);
                    return;
                }
            }
        }

        // --- Phase 2: try raw pixel data ---
        let img_result = arboard::Clipboard::new()
            .ok()
            .and_then(|mut cb| cb.get_image().ok());

        if let Some(img_data) = img_result {
            match save_clipboard_image_to_temp(&img_data) {
                Ok(path) => {
                    self.push_log(
                        LogLevel::Info,
                        format!("📎 Image saved from clipboard: {}", path.display()),
                    );
                    self.pending_attachments.push(path);
                }
                Err(e) => {
                    self.push_log(
                        LogLevel::Warn,
                        format!("Failed to save clipboard image: {e}"),
                    );
                }
            }
        } else {
            self.push_log(
                LogLevel::Info,
                "No image data found in clipboard".to_string(),
            );
        }
    }

    /// Execute a context menu action (Cut / Copy / Paste) and close the menu.
    pub fn execute_context_action(&mut self, action: ContextAction) {
        let pane = self.context_menu.as_ref().map(|m| m.pane);
        self.context_menu = None;

        match action {
            ContextAction::Copy => {
                self.copy_selection();
            }
            ContextAction::Cut => {
                // Copy selected text then clear the input (cut only makes sense in input panes).
                self.copy_selection();
                if matches!(
                    pane,
                    Some(SelectionPane::Input) | Some(SelectionPane::HomeInput)
                ) {
                    self.input.clear();
                    self.slash_menu = None;
                    self.file_menu = None;
                }
            }
            ContextAction::Paste => {
                // Only paste into input panes.
                if matches!(
                    pane,
                    Some(SelectionPane::Input) | Some(SelectionPane::HomeInput)
                ) {
                    if let Some(text) = Self::get_clipboard() {
                        // Strip newlines for single-line input.
                        let clean: String =
                            text.chars().filter(|&c| c != '\n' && c != '\r').collect();
                        self.input.push_str(&clean);
                        if self.input.starts_with('/') {
                            self.update_slash_menu();
                        }
                        if self.input.contains('@') {
                            self.update_file_menu();
                        }
                    }
                }
            }
        }
    }

    /// Handle a left-click when the context menu is open.
    ///
    /// Clicks within the menu bounds activate the item under the cursor.
    /// Clicks outside dismiss the menu.
    fn handle_context_menu_click(&mut self, col: u16, row: u16) {
        if let Some(menu) = self.context_menu.clone() {
            // Menu geometry: x, y is top-left; rows are y+1..y+1+items.len()
            let menu_x = menu.x;
            let menu_y = menu.y;
            let menu_w = 12u16; // matches render_context_menu width
            let item_count = menu.items.len() as u16;
            let menu_h = item_count + 2; // border top + items + border bottom

            if col >= menu_x && col < menu_x + menu_w && row >= menu_y && row < menu_y + menu_h {
                // Row inside border
                if row > menu_y && row < menu_y + menu_h - 1 {
                    let item_idx = (row - menu_y - 1) as usize;
                    if item_idx < menu.items.len() {
                        let (action, enabled) = menu.items[item_idx];
                        if enabled {
                            self.execute_context_action(action);
                        } else {
                            self.context_menu = None;
                        }
                    }
                }
            } else {
                // Click outside menu dismisses it.
                self.context_menu = None;
            }
        }
    }

    /// Map a mouse Y position to a scroll offset for the given pane.
    fn apply_scrollbar_drag(&mut self, mouse_y: u16, pane: ScrollbarDragPane) {
        let (area, max_scroll) = match pane {
            ScrollbarDragPane::Messages => (self.message_area, self.message_max_scroll),
            ScrollbarDragPane::Log => (self.log_area, self.log_max_scroll),
        };

        if area.height <= 1 || max_scroll == 0 {
            return;
        }

        // Clamp mouse_y to the pane area
        let y = mouse_y.clamp(area.y, area.bottom().saturating_sub(1));
        let relative = y.saturating_sub(area.y) as f32;
        let track_height = (area.height.saturating_sub(1)) as f32;
        let fraction = (relative / track_height).clamp(0.0, 1.0);

        // fraction 0.0 = top of content, 1.0 = bottom of content
        // scroll_offset is "lines from bottom": top → max_scroll, bottom → 0
        let offset = ((1.0 - fraction) * max_scroll as f32).round() as u16;

        match pane {
            ScrollbarDragPane::Messages => self.scroll_offset = offset.min(max_scroll),
            ScrollbarDragPane::Log => self.log_scroll_offset = offset.min(max_scroll),
        }
    }

    /// Process a terminal key event and execute the resulting [`InputAction`], if any.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    /// # use ragent_tui::App;
    /// # fn example(app: &mut App) {
    /// let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    /// app.handle_key_event(key);
    /// # }
    /// ```
    pub fn handle_key_event(&mut self, key: KeyEvent) {
        // History picker intercepts all keys while it is open
        if self.history_picker.is_some() {
            self.handle_history_picker_key(key);
            return;
        }
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

                    // Drain image attachments once; either queue for auto-compaction
                    // or send immediately.
                    let image_paths: Vec<std::path::PathBuf> =
                        self.pending_attachments.drain(..).collect();
                    if self.should_auto_compact_before_send() {
                        self.pending_send_after_compact = Some((text, image_paths));
                        if !self.start_compaction(true) {
                            // If compaction could not start, fall back to direct send.
                            if let Some((queued_text, queued_images)) =
                                self.pending_send_after_compact.take()
                            {
                                self.dispatch_user_message(queued_text, queued_images);
                            }
                        }
                        return;
                    }
                    self.dispatch_user_message(text, image_paths);
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
                    self.input_cursor = self.input_len_chars();
                }
                InputAction::HistoryDown => match self.history_index {
                    Some(idx) if idx + 1 < self.input_history.len() => {
                        let idx = idx + 1;
                        self.history_index = Some(idx);
                        self.input = self.input_history[idx].clone();
                        self.input_cursor = self.input_len_chars();
                    }
                    Some(_) => {
                        self.history_index = None;
                        self.input = self.history_draft.clone();
                        self.history_draft.clear();
                        self.input_cursor = self.input_len_chars();
                    }
                    None => {}
                },
                InputAction::MoveCursorLeft => {
                    self.cursor_move_left();
                }
                InputAction::MoveCursorRight => {
                    self.cursor_move_right();
                }
                InputAction::MoveCursorHome => {
                    self.cursor_move_home();
                }
                InputAction::MoveCursorEnd => {
                    self.cursor_move_end();
                }
                InputAction::Delete => {
                    if self.input_cursor < self.input_len_chars() {
                        let del_pos = self.cursor_byte_pos();
                        self.input.remove(del_pos);
                    }
                }
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
                    }
                }
                InputAction::SlashCommand(cmd) => {
                    self.execute_slash_command(&cmd);
                }
                InputAction::CancelAgent => {
                    if let Some(ref flag) = self.cancel_flag {
                        flag.store(true, Ordering::Relaxed);
                        self.status = "halting agent…".to_string();
                        self.push_log(
                            LogLevel::Warn,
                            "User pressed ESC — halting agent".to_string(),
                        );
                    }
                }
            }
        }
    }

    /// Execute a plan agent delegation.
    ///
    /// Pushes the current agent onto the agent stack, switches to the plan
    /// agent, and spawns an async task to send the task to the plan agent.
    fn execute_plan_delegation(&mut self, session_id: &str, task: String, context: String) {
        // Push current agent to stack so plan_exit can restore it
        self.agent_stack.push(self.agent_info.clone());

        // Find and switch to the plan agent
        let plan_agent = self
            .cycleable_agents
            .iter()
            .find(|a| a.name == "plan")
            .cloned();

        if let Some(mut plan) = plan_agent {
            let prev_name = self.agent_name.clone();

            // Apply current model override to plan agent
            if let Some(ref model_str) = self.selected_model {
                if let Some((provider, model)) = model_str.split_once('/') {
                    plan.model = Some(ModelRef {
                        provider_id: provider.to_string(),
                        model_id: model.to_string(),
                    });
                }
            }

            self.agent_info = plan.clone();
            self.agent_name = "plan".to_string();
            self.status = format!("agent: plan (delegated from {})", prev_name);
            self.push_log(
                LogLevel::Info,
                format!("plan delegation: {} → plan", prev_name),
            );

            // Publish the switch event
            self.event_bus.publish(Event::AgentSwitched {
                session_id: session_id.to_string(),
                from: prev_name,
                to: "plan".to_string(),
            });

            // Build the task message
            let full_task = if context.is_empty() {
                task
            } else {
                format!("{}\n\nContext:\n{}", task, context)
            };

            // Add user message to UI
            let sid = session_id.to_string();
            let msg = Message::user_text(&sid, &full_task);
            self.messages.push(msg);

            // Spawn async processing
            let processor = self.session_processor.clone();
            let agent = self.agent_info.clone();
            let task_text = full_task;
            tokio::spawn(async move {
                if let Err(e) = processor
                    .process_message(&sid, &task_text, &agent, Arc::new(AtomicBool::new(false)))
                    .await
                {
                    tracing::debug!(error = %e, "Plan agent failed");
                }
            });
        } else {
            self.push_log(LogLevel::Error, "plan agent not found".to_string());
            // Pop the agent we just pushed since we can't delegate
            self.agent_stack.pop();
        }
    }

    /// Restore the previous agent after plan_exit.
    ///
    /// Pops the agent stack, switches back to the previous agent, publishes
    /// an `AgentSwitched` event, and injects the plan summary into the
    /// conversation as an assistant message.
    fn execute_plan_restore(&mut self, session_id: &str, summary: &str) {
        if let Some(prev_agent) = self.agent_stack.pop() {
            let from_name = self.agent_name.clone();
            let to_name = prev_agent.name.clone();

            self.agent_info = prev_agent;
            self.agent_name = to_name.clone();
            self.status = format!("agent: {}", to_name);
            self.push_log(LogLevel::Info, format!("plan restore: plan → {}", to_name));

            self.event_bus.publish(Event::AgentSwitched {
                session_id: session_id.to_string(),
                from: from_name,
                to: to_name,
            });

            // Inject the plan summary into the chat so the restored agent
            // can see it in context.
            let plan_text = format!("📋 **Plan summary:**\n{}", summary);
            self.append_assistant_text(&plan_text);
            self.force_new_message = true;
        } else {
            self.push_log(
                LogLevel::Error,
                "plan_exit called but agent stack is empty".to_string(),
            );
        }
    }

    /// Handle an [`Event`] from the agent event bus and update application state.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use ragent_core::event::Event;
    /// # use ragent_tui::App;
    /// # fn example(app: &mut App) {
    /// let event = Event::SessionCreated {
    ///     session_id: "sess-001".to_string(),
    /// };
    /// app.handle_event(event);
    /// # }
    /// ```
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
                    // Get the current step count from the event bus (single source of truth)
                    let step = self.event_bus.current_step(session_id);
                    let short_sid = short_session_id(session_id);
                    self.tool_step_map
                        .insert(call_id.clone(), (short_sid.clone(), step as u32));
                    self.add_tool_call_part(tool, call_id);
                    self.status = format!("running: {}", tool);
                    self.push_log(
                        LogLevel::Tool,
                        format!("[{short_sid}:{step}] tool call: {}", tool),
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
                    self.update_tool_call_status(call_id, error.is_none(), error.as_deref());
                    let step_tag = self
                        .tool_step_map
                        .get(call_id)
                        .map(|(sid, s)| format!("[{sid}:{s}] "))
                        .unwrap_or_default();
                    if let Some(err) = error {
                        self.push_log(
                            LogLevel::Error,
                            format!(
                                "{}tool {} failed: {} ({}ms)",
                                step_tag, tool, err, duration_ms
                            ),
                        );
                    } else {
                        self.push_log(
                            LogLevel::Tool,
                            format!("{}tool {} completed ({}ms)", step_tag, tool, duration_ms),
                        );
                    }
                }
            }
            Event::MessageStart {
                ref session_id,
                ref message_id,
            } => {
                if self.is_current_session(session_id) {
                    self.is_processing = true;
                    self.agent_halted = false;
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
                    let was_auto_compaction = self.auto_compact_in_progress;
                    self.is_processing = false;
                    self.cancel_flag = None;
                    if *reason == FinishReason::Cancelled {
                        self.agent_halted = true;
                        self.status = "halted — /resume to continue".to_string();
                        self.push_log(LogLevel::Warn, "Agent halted by user".to_string());
                    } else {
                        self.agent_halted = false;
                        self.status = "ready".to_string();
                    }
                    self.force_new_message = true;
                    self.push_log(LogLevel::Info, format!("response finished ({reason:?})"));

                    // Handle pending plan delegation: switch agent and auto-send task
                    if let Some((task, context)) = self.pending_plan_task.take() {
                        self.execute_plan_delegation(session_id, task, context);
                    }

                    // Handle pending agent restore: pop stack and inject summary
                    if let Some(summary) = self.pending_plan_restore.take() {
                        self.execute_plan_restore(session_id, &summary);
                    }

                    if was_auto_compaction {
                        self.auto_compact_in_progress = false;
                        self.push_log(LogLevel::Info, "Auto-compaction completed".to_string());
                        if let Some((queued_text, queued_images)) =
                            self.pending_send_after_compact.take()
                        {
                            self.dispatch_user_message(queued_text, queued_images);
                        }
                    }
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
            Event::AgentSwitchRequested {
                ref session_id,
                ref to,
                ref task,
                ref context,
            } => {
                if self.is_current_session(session_id) {
                    self.push_log(
                        LogLevel::Info,
                        format!("agent switch requested → {} ({})", to, task),
                    );
                    self.pending_plan_task = Some((task.clone(), context.clone()));
                }
            }
            Event::AgentRestoreRequested {
                ref session_id,
                ref summary,
            } => {
                if self.is_current_session(session_id) {
                    self.push_log(
                        LogLevel::Info,
                        format!("agent restore requested ({} chars)", summary.len()),
                    );
                    self.pending_plan_restore = Some(summary.clone());
                }
            }
            Event::AgentError {
                ref session_id,
                ref error,
            } => {
                if self.is_current_session(session_id) {
                    if self.auto_compact_in_progress {
                        self.auto_compact_in_progress = false;
                        self.auto_compact_failed = true;
                        self.pending_send_after_compact = None;
                        self.push_log(
                            LogLevel::Warn,
                            "Auto-compaction failed; send blocked for this turn".to_string(),
                        );
                    }
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
                    self.last_input_tokens = input_tokens;
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
            Event::QuotaUpdate {
                ref session_id,
                percent,
            } => {
                if self.is_current_session(session_id) {
                    self.quota_percent = Some(percent);
                    self.push_log(
                        LogLevel::Info,
                        format!("quota: {:.1}% used", percent),
                    );
                }
            }
            Event::ToolsSent {
                ref session_id,
                ref tools,
            } => {
                if self.is_current_session(session_id) {
                    // Only log the list of tools during system initialisation (first step).
                    // The SessionProcessor increments the EventBus step at the start of
                    // each loop iteration; the first LLM request corresponds to step 1.
                    if self.event_bus.current_step(session_id) <= 1 {
                        self.push_log(
                            LogLevel::Info,
                            format!("tools sent: [{}]", tools.join(", ")),
                        );
                    }
                }
            }
            Event::ModelResponse {
                ref session_id,
                ref text,
                elapsed_ms,
            } => {
                if self.is_current_session(session_id) {
                    self.push_log(LogLevel::Info, format!("model response ({elapsed_ms}ms): {text}"));
                }
            }
            Event::ToolCallArgs {
                ref session_id,
                ref call_id,
                ref tool,
                ref args,
            } => {
                if self.is_current_session(session_id) {
                    self.update_tool_call_input(call_id, args);
                    let step_tag = self
                        .tool_step_map
                        .get(call_id)
                        .map(|(sid, s)| format!("[{sid}:{s}] "))
                        .unwrap_or_default();
                    // Pretty-print JSON args across multiple log lines
                    let pretty = serde_json::from_str::<serde_json::Value>(args)
                        .ok()
                        .and_then(|v| serde_json::to_string_pretty(&v).ok());
                    if let Some(formatted) = pretty {
                        let mut first = true;
                        for line in formatted.lines() {
                            if first {
                                self.push_log(
                                    LogLevel::Tool,
                                    format!("{}→ {} {}", step_tag, tool, line),
                                );
                                first = false;
                            } else {
                                self.push_log(LogLevel::Tool, format!("  {}", line));
                            }
                        }
                    } else {
                        self.push_log(LogLevel::Tool, format!("{}→ {}({})", step_tag, tool, args));
                    }
                }
            }
            Event::ToolResult {
                ref session_id,
                ref call_id,
                ref tool,
                ref content,
                content_line_count,
                ref metadata,
                success,
                ..
            } => {
                if self.is_current_session(session_id) {
                    self.update_tool_call_output(call_id, content_line_count, metadata.as_ref());
                    let step_tag = self
                        .tool_step_map
                        .get(call_id)
                        .map(|(sid, s)| format!("[{sid}:{s}] "))
                        .unwrap_or_default();
                    let icon = if success { "✓" } else { "✗" };
                    self.push_log(
                        LogLevel::Tool,
                        format!("{}← {} {} {}", step_tag, tool, icon, content),
                    );
                }
            }
            Event::SubagentStart {
                ref session_id,
                ref task_id,
                ref child_session_id,
                ref agent,
                ref task,
                background,
                ..
            } => {
                if self.is_current_session(session_id) {
                    // Add to active_tasks so the agent panel shows it immediately.
                    let entry = ragent_core::task::TaskEntry {
                        id: task_id.clone(),
                        parent_session_id: session_id.clone(),
                        child_session_id: child_session_id.clone(),
                        agent_name: agent.clone(),
                        task_prompt: task.clone(),
                        background,
                        status: ragent_core::task::TaskStatus::Running,
                        result: None,
                        error: None,
                        created_at: chrono::Utc::now(),
                        completed_at: None,
                        reported: false,
                    };
                    self.active_tasks.push(entry);

                    let (icon, kind) = if background {
                        ("⚙️", "Background")
                    } else {
                        ("🔄", "Foreground")
                    };
                    self.push_log(
                        LogLevel::Info,
                        format!(
                            "{} {} task started: {} ({})",
                            icon,
                            kind,
                            &task_id[..8.min(task_id.len())],
                            agent
                        ),
                    );
                }
            }
            Event::SubagentComplete {
                ref session_id,
                ref task_id,
                ref summary,
                success,
                ..
            } => {
                if self.is_current_session(session_id) {
                    if let Some(idx) = self.active_tasks.iter().position(|t| t.id == *task_id) {
                        self.active_tasks.remove(idx);
                    }
                    let icon = if success { "✅" } else { "❌" };
                    self.push_log(
                        LogLevel::Info,
                        format!(
                            "{} Task completed ({}): {}",
                            icon,
                            &task_id[..8.min(task_id.len())],
                            summary
                        ),
                    );
                }
            }
            Event::SubagentCancelled {
                ref session_id,
                ref task_id,
            } => {
                if self.is_current_session(session_id) {
                    if let Some(idx) = self.active_tasks.iter().position(|t| t.id == *task_id) {
                        self.active_tasks.remove(idx);
                    }
                    self.push_log(
                        LogLevel::Info,
                        format!("🚫 Task cancelled ({})", &task_id[..8.min(task_id.len())]),
                    );
                }
            }
            Event::LspStatusChanged {
                ref server_id,
                ref status,
            } => {
                // Update or insert the server descriptor for status display.
                if let Some(s) = self.lsp_servers.iter_mut().find(|s| s.id == *server_id) {
                    s.status = status.clone();
                } else {
                    // New server — create a minimal descriptor. Full descriptor
                    // is populated when the LspManager initialises.
                    let mut s = LspServer::unknown(server_id.clone());
                    s.status = status.clone();
                    self.lsp_servers.push(s);
                }
                let icon = match status {
                    LspStatus::Connected => "🟢",
                    LspStatus::Starting => "🟡",
                    LspStatus::Disabled => "⚪",
                    LspStatus::Failed { .. } => "🔴",
                };
                self.push_log(
                    LogLevel::Info,
                    format!("{icon} LSP '{}' → {:?}", server_id, status),
                );
            }
            Event::TeammateSpawned {
                ref session_id,
                ref team_name,
                ref teammate_name,
                ref agent_id,
            } => {
                if self.is_current_session(session_id) {
                    // Add new member to team_members if not already present.
                    if !self.team_members.iter().any(|m| m.agent_id == *agent_id) {
                        let member = TeamMember::new(
                            teammate_name.as_str(),
                            agent_id.as_str(),
                            "teammate",
                        );
                        self.team_members.push(member);
                    }
                    self.show_teams = true;
                    self.push_log(
                        LogLevel::Info,
                        format!(
                            "🤝 [{team_name}] Spawned teammate '{teammate_name}' ({agent_id})"
                        ),
                    );
                }
            }
            Event::TeammateMessage {
                ref session_id,
                ref team_name,
                ref from,
                ref to,
                ref preview,
            } => {
                if self.is_current_session(session_id) {
                    self.push_log(
                        LogLevel::Info,
                        format!("📨 [{team_name}] {from} → {to}: {preview}"),
                    );
                }
            }
            Event::TeammateIdle {
                ref session_id,
                ref team_name,
                ref agent_id,
            } => {
                if self.is_current_session(session_id) {
                    if let Some(m) = self.team_members.iter_mut().find(|m| m.agent_id == *agent_id) {
                        m.status = MemberStatus::Idle;
                    }
                    self.push_log(
                        LogLevel::Info,
                        format!("💤 [{team_name}] Teammate {agent_id} is idle"),
                    );
                }
            }
            Event::TeamTaskClaimed {
                ref session_id,
                ref team_name,
                ref agent_id,
                ref task_id,
            } => {
                if self.is_current_session(session_id) {
                    if let Some(m) = self.team_members.iter_mut().find(|m| m.agent_id == *agent_id) {
                        m.status = MemberStatus::Working;
                        m.current_task_id = Some(task_id.clone());
                    }
                    self.push_log(
                        LogLevel::Info,
                        format!("📋 [{team_name}] {agent_id} claimed task {task_id}"),
                    );
                }
            }
            Event::TeamTaskCompleted {
                ref session_id,
                ref team_name,
                ref agent_id,
                ref task_id,
            } => {
                if self.is_current_session(session_id) {
                    if let Some(m) = self.team_members.iter_mut().find(|m| m.agent_id == *agent_id) {
                        m.current_task_id = None;
                    }
                    self.push_log(
                        LogLevel::Info,
                        format!("✅ [{team_name}] {agent_id} completed task {task_id}"),
                    );
                }
            }
            Event::TeamCleanedUp {
                ref session_id,
                ref team_name,
            } => {
                if self.is_current_session(session_id) {
                    self.active_team = None;
                    self.team_members.clear();
                    self.show_teams = false;
                    self.push_log(
                        LogLevel::Info,
                        format!("🗑️  Team '{team_name}' cleaned up"),
                    );
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
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use ragent_tui::App;
    /// # use ragent_tui::app::LogLevel;
          /// # fn example(app: &mut App) {
          /// app.push_log(LogLevel::Info, "Session started".to_string());
          /// # }
          /// ```
          pub fn push_log(&mut self, level: LogLevel, message: String) {
              self.log_entries.push(LogEntry {
                  timestamp: chrono::Utc::now(),
                  level,
                  message,
                  session_id: self.session_id.clone(),
              });
          }
    fn is_current_session(&self, session_id: &str) -> bool {
        self.session_id.as_deref() == Some(session_id)
    }

    fn append_assistant_text(&mut self, text: &str) {
        if !self.force_new_message {
            if let Some(last) = self.messages.last_mut()
                && last.role == Role::Assistant
            {
                // Only append to the last part if it is a Text part;
                // otherwise start a new Text part so text after tool calls
                // appears in the correct position.
                if let Some(MessagePart::Text { text: t }) = last.parts.last_mut() {
                    t.push_str(text);
                } else {
                    last.parts.push(MessagePart::Text {
                        text: text.to_string(),
                    });
                }
                return;
            }
        }
        self.force_new_message = false;
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
            if let Some(MessagePart::Reasoning { text: t }) = last.parts.last_mut() {
                t.push_str(text);
            } else {
                last.parts.push(MessagePart::Reasoning {
                    text: text.to_string(),
                });
            }
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

    fn update_tool_call_status(&mut self, call_id: &str, success: bool, error: Option<&str>) {
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
                    if let Some(err) = error {
                        state.error = Some(err.to_string());
                    }
                    return;
                }
            }
        }
    }

    fn update_tool_call_input(&mut self, call_id: &str, args_json: &str) {
        if let Ok(input) = serde_json::from_str::<serde_json::Value>(args_json) {
            for msg in self.messages.iter_mut().rev() {
                for part in msg.parts.iter_mut() {
                    if let MessagePart::ToolCall {
                        call_id: cid,
                        state,
                        ..
                    } = part
                        && cid == call_id
                    {
                        state.input = input;
                        return;
                    }
                }
            }
        }
    }

    fn update_tool_call_output(
        &mut self,
        call_id: &str,
        content_line_count: usize,
        metadata: Option<&serde_json::Value>,
    ) {
        let mut value = serde_json::json!({ "line_count": content_line_count });
        // Merge tool metadata fields into the output for the TUI
        if let Some(meta) = metadata {
            if let Some(obj) = meta.as_object() {
                for (k, v) in obj {
                    value[k] = v.clone();
                }
            }
        }
        for msg in self.messages.iter_mut().rev() {
            for part in msg.parts.iter_mut() {
                if let MessagePart::ToolCall {
                    call_id: cid,
                    state,
                    ..
                } = part
                    && cid == call_id
                {
                    state.output = Some(value);
                    return;
                }
            }
        }
    }
}

/// Produces a short, user-facing summary from a raw error string.
///
/// Returns the last 8 characters of a session ID as the short display form.
fn short_session_id(session_id: &str) -> String {
    let start = session_id.len().saturating_sub(8);
    session_id[start..].to_string()
}

/// Strips JSON payloads, HTTP status codes, and verbose context so the
/// chat panel only shows the essential message.
fn summarise_error(raw: &str) -> String {
    // Try to extract just the human-readable message from common patterns
    // e.g. "LLM call failed: Unknown model: claude-haiku-4.5"
    let cleaned = raw.trim().strip_prefix("LLM call failed: ").unwrap_or(raw);

    if cleaned.contains("not accessible via the /chat/completions endpoint")
        || cleaned.contains("unsupported_api_for_model")
    {
        return "Selected model is not available for chat/completions; use /model and pick a non-Codex chat model".to_string();
    }

    // Truncate to a reasonable length for the status bar
    if cleaned.len() > 120 {
        let mut end = 120;
        while end > 0 && !cleaned.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &cleaned[..end])
    } else {
        cleaned.to_string()
    }
}

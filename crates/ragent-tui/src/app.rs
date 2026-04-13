//! Application state and event handling for the TUI.
//!
//! The [`App`] struct holds the current session, message history, input buffer,
//! scroll position, and permission state. It processes both terminal key events
//! and agent bus events to drive the UI.

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use lru::LruCache;

use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use pulldown_cmark::{Options, Parser, html};
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
    tool::TeamManagerInterface,
};

use crate::input::{self, InputAction};
use crate::tips;

// Prompt optimization templates
use prompt_opt::{Completer, OptMethod, optimize};

mod state;
pub use self::state::*;

/// Connects the `prompt_opt` crate to the session's active LLM provider.
///
/// `RagentCompleter` implements [`Completer`] by building an [`LlmClient`] from
/// the configured provider, sending the system+user message pair, and collecting
/// the streaming `TextDelta` events into a single `String`.
struct RagentCompleter {
    registry: Arc<ragent_core::provider::ProviderRegistry>,
    storage: Arc<ragent_core::storage::Storage>,
    provider_id: String,
    model_id: String,
}

#[async_trait::async_trait]
impl Completer for RagentCompleter {
    async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String> {
        use anyhow::Context as _;
        use futures::StreamExt as _;
        use ragent_core::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};

        let api_key = self
            .storage
            .get_provider_auth(&self.provider_id)
            .context("reading API key")?
            .unwrap_or_default();

        let provider = self
            .registry
            .get(&self.provider_id)
            .with_context(|| format!("provider '{}' not found", self.provider_id))?;

        let client = provider
            .create_client(&api_key, None, &Default::default())
            .await
            .context("creating LLM client")?;

        let request = ChatRequest {
            model: self.model_id.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text(user.to_string()),
            }],
            tools: vec![],
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: Some(system.to_string()),
            options: Default::default(),
            session_id: None,
            request_id: None,
        };

        let mut stream = client.chat(request).await.context("starting LLM stream")?;
        let mut result = String::new();
        while let Some(event) = stream.next().await {
            if let StreamEvent::TextDelta { text } = event {
                result.push_str(&text);
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy)]
struct MentionSpan {
    at_start: usize,
    token_start: usize,
    token_end: usize,
}

impl MentionSpan {
    fn query<'a>(&self, input: &'a str) -> &'a str {
        &input[self.token_start..self.token_end]
    }
}

impl App {
    fn is_ascii_table_line(line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false;
        }
        trimmed.contains('│')
            || (trimmed.contains('─')
                && trimmed
                    .chars()
                    .all(|c| matches!(c, '─' | '┬' | '┼' | '┴' | ' ')))
    }

    fn table_row_cells(line: &str) -> Vec<String> {
        line.split('│').map(|c| c.trim().to_string()).collect()
    }

    fn table_border(widths: &[usize]) -> String {
        let mut out = String::from("+");
        for w in widths {
            out.push_str(&"-".repeat(*w + 2));
            out.push('+');
        }
        out
    }

    /// Normalises ASCII table lines in a rendered string, collapsing separator rows
    /// and aligning column widths for consistent terminal display.
    pub fn normalize_ascii_tables(&self, rendered: &str) -> String {
        let lines: Vec<&str> = rendered.lines().collect();
        let mut out: Vec<String> = Vec::new();
        let mut i = 0usize;

        while i < lines.len() {
            if !Self::is_ascii_table_line(lines[i]) {
                out.push(lines[i].to_string());
                i += 1;
                continue;
            }

            let start = i;
            while i < lines.len() && Self::is_ascii_table_line(lines[i]) {
                i += 1;
            }
            let block = &lines[start..i];

            let mut rows: Vec<Vec<String>> = Vec::new();
            let mut separators: Vec<bool> = Vec::new();
            let mut col_count = 0usize;
            for line in block {
                let trimmed = line.trim();
                if trimmed.contains('│') {
                    let cells = Self::table_row_cells(trimmed);
                    col_count = col_count.max(cells.len());
                    rows.push(cells);
                    separators.push(false);
                } else {
                    separators.push(true);
                    rows.push(Vec::new());
                }
            }
            if col_count == 0 {
                out.extend(block.iter().map(|s| s.to_string()));
                continue;
            }

            let mut widths = vec![0usize; col_count];
            for row in &rows {
                for (idx, cell) in row.iter().enumerate() {
                    widths[idx] = widths[idx].max(cell.chars().count());
                }
            }

            let border = Self::table_border(&widths);
            let mut wrote_top = false;
            for (idx, row) in rows.iter().enumerate() {
                if separators[idx] {
                    if !wrote_top {
                        out.push(border.clone());
                        wrote_top = true;
                    } else {
                        out.push(border.clone());
                    }
                    continue;
                }
                if !wrote_top {
                    out.push(border.clone());
                    wrote_top = true;
                }
                let mut line = String::from("|");
                for col in 0..col_count {
                    let cell = row.get(col).cloned().unwrap_or_default();
                    let pad = widths[col].saturating_sub(cell.chars().count());
                    line.push(' ');
                    line.push_str(&cell);
                    line.push_str(&" ".repeat(pad));
                    line.push(' ');
                    line.push('|');
                }
                out.push(line);
            }
            if wrote_top {
                out.push(border);
            }
        }
        out.join("\n")
    }

    /// Renders markdown-formatted slash command output to plain ASCII text.
    ///
    /// Only processes strings that begin with `"From: /"` (slash command responses).
    /// Plain runtime assistant text is returned unchanged.
    pub fn render_markdown_to_ascii(&mut self, text: &str) -> String {
        // Only convert markdown-like slash output; preserve plain runtime text.
        if !text.starts_with("From: /") {
            return text.to_string();
        }

        // Check cache using FNV-1a hash of input.
        let hash = {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for b in text.as_bytes() {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
            h
        };
        if let Some(cached) = self.md_render_cache.get(&hash) {
            return cached.clone();
        }
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_TABLES);
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        opts.insert(Options::ENABLE_TASKLISTS);

        let parser = Parser::new_ext(text, opts);
        let mut html_buf = String::new();
        html::push_html(&mut html_buf, parser);

        let rendered = html2text::from_read(html_buf.as_bytes(), 120).unwrap_or_else(|_| {
            // Fallback to original text when markdown conversion fails.
            text.to_string()
        });
        let cleaned = rendered
            .lines()
            .map(|l| l.trim_end())
            .collect::<Vec<&str>>()
            .join("\n");
        let result = self.normalize_ascii_tables(&cleaned);

        // Limit cache size to avoid unbounded growth.
        if self.md_render_cache.len() >= 256 {
            self.md_render_cache.clear(); // LRU handles eviction
        }
        self.md_render_cache.put(hash, result.clone());
        result
    }

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
            .chain(
                custom_defs
                    .iter()
                    .filter(|d| !d.agent_info.hidden)
                    .map(|d| {
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
                    }),
            )
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
            kb_select_anchor: None,
            scroll_offset: 0,
            is_running: true,
            event_bus,
            storage,
            session_id: None,
            agent_name,
            status: "ready".to_string(),
            permission_pending: None,
            pending_question_input: String::new(),
            token_usage: (0, 0),
            llm_request_stats: Vec::new(),
            last_input_tokens: 0,
            quota_percent: None,
            current_screen: ScreenMode::Chat,
            tip: tips::random_tip(),
            cwd,
            shell_cwd: None,
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
            file_menu_show_hidden: false,
            project_files_cache: None,
            project_files_cache_cwd: None,
            project_files_cache_refreshed_at: None,
            project_files_cache_count: 0,
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
            active_agents_area: Rect::default(),
            scrollbar_drag: None,
            text_selection: None,
            message_content_lines: Vec::new(),
            log_content_lines: Vec::new(),
            input_area: Rect::default(),
            teams_area: Rect::default(),
            output_view_area: Rect::default(),
            agents_button_area: Rect::default(),
            teams_button_area: Rect::default(),
            show_agents_window: false,
            show_teams_window: false,
            agents_close_button_area: Rect::default(),
            teams_close_button_area: Rect::default(),
            mcp_servers: Vec::new(),
            lsp_servers: Vec::new(),
            lsp_manager: None,
            lsp_discover: None,
            lsp_edit: None,
            mcp_discover: None,
            force_new_message: false,
            agent_stack: Vec::new(),
            pending_plan_task: None,
            pending_plan_restore: None,
            pending_forcecleanup: None,
            is_processing: false,
            cancel_flag: None,
            auto_compact_in_progress: false,
            compact_in_progress: false,
            auto_compact_failed: false,
            pending_send_after_compact: None,
            agent_halted: false,
            tool_step_map: HashMap::new(),
            last_step_per_session: HashMap::new(),
            substep_counter_per_session: HashMap::new(),
            active_tasks: Vec::new(),
            show_shortcuts: false,
            quit_armed: false,
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
            team_message_counts: HashMap::new(),
            show_teams: false,
            teams_scroll_offset: 0,
            teams_max_scroll: 0,
            focused_teammate: None,
            swarm_state: None,
            swarm_result: Arc::new(std::sync::Mutex::new(None)),
            output_view: None,
            opt_result: Arc::new(std::sync::Mutex::new(None)),
            history_dirty: false,
            history_save_deadline: None,
            md_render_cache: LruCache::new(NonZeroUsize::new(256).unwrap()),
            autopilot_enabled: false,
            autopilot_token_budget: None,
            autopilot_time_limit_secs: None,
            autopilot_started_at: None,
            autopilot_pending_continue: None,
            sid_to_display_name: HashMap::new(),
            next_agent_index: 1,
            prompt_start_time: None,
            tool_time_ms: 0,
            llm_time_ms: 0,
                          plan_approval_pending: None,
                          role_mode: None,
                          webapi_server: None,
                          webapi_addr: "127.0.0.1:3000".to_string(),
                          webapi_token: None,
                          needs_redraw: true,
                          code_index: None,
                      };        // Log any warnings from custom agent loading into the log panel
        for diag in &all_diagnostics {
            app.push_log_no_agent(LogLevel::Warn, format!("[custom agents] {}", diag));
        }

        // Initialise the bash allowlist/denylist from config
        ragent_core::bash_lists::load_from_config();

        app
    }

    /// Poll for a completed `/opt` LLM result and display it.
    ///
    /// Called from the TUI main loop (~50 ms cadence). If the background
    /// optimize task has deposited a result, this method retrieves it, appends
    /// the text to the message list, and updates the status bar.
    pub fn poll_pending_opt(&mut self) {
        let outcome = {
            let mut guard = match self.opt_result.lock() {
                Ok(g) => g,
                Err(poisoned) => {
                    tracing::error!("opt_result mutex poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            guard.take()
        };
        if let Some(outcome) = outcome {
            match outcome {
                Ok(text) => {
                    let lines = text.lines().count();
                    self.append_assistant_text(&text);
                    self.status = "opt: done".to_string();
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("Finished /opt — {} lines output", lines),
                    );
                }
                Err(msg) => {
                    self.status = format!("⚠ opt failed: {}", msg);
                    self.push_log_no_agent(LogLevel::Warn, format!("opt error: {}", msg));
                }
            }
        }
    }

    /// Poll for a completed `/swarm` LLM decomposition.  When the async
    /// decomposition task has deposited a result, this method parses it,
    /// creates the ephemeral team, seeds tasks, and spawns teammates.
    pub fn poll_pending_swarm(&mut self) {
        let outcome = {
            let mut guard = match self.swarm_result.lock() {
                Ok(g) => g,
                Err(poisoned) => {
                    tracing::error!("swarm_result mutex poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            guard.take()
        };
        let Some(outcome) = outcome else { return };
        match outcome {
            Ok(raw_json) => match ragent_core::team::parse_decomposition(&raw_json) {
                Ok(decomposition) => {
                    self.execute_swarm_decomposition(decomposition);
                }
                Err(msg) => {
                    self.status = "⚠ swarm: decomposition parse error".to_string();
                    self.append_assistant_text(&format!(
                        "From: /swarm\n## ❌ Decomposition Failed\n\n{}\n",
                        msg
                    ));
                    self.push_log_no_agent(LogLevel::Warn, format!("Swarm parse error: {}", msg));
                }
            },
            Err(msg) => {
                self.status = format!("⚠ swarm failed: {}", msg);
                self.append_assistant_text(&format!(
                    "From: /swarm\n## ❌ Swarm Error\n\n{}\n",
                    msg
                ));
                self.push_log_no_agent(LogLevel::Warn, format!("Swarm error: {}", msg));
            }
        }
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
        // Mark dirty; the main loop will flush after the debounce window.
        self.history_dirty = true;
        if self.history_save_deadline.is_none() {
            self.history_save_deadline =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
        }
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

    fn ollama_cloud_api_key(&self) -> Option<String> {
        self.storage
            .get_provider_auth("ollama_cloud")
            .ok()
            .flatten()
            .filter(|k| !k.is_empty())
            .or_else(|| {
                std::env::var("OLLAMA_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
            })
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

        // Use the current session's provider/model for compaction so it works
        // with any provider (Copilot, OpenAI, Ollama, etc.), not just Anthropic.
        // Priority: selected_model setting → current agent_info model → built-in Haiku.
        let mut agent = compaction_agent;
        let resolved_model = self
            .selected_model
            .as_deref()
            .and_then(|s| s.split_once('/'))
            .map(|(p, m)| ModelRef {
                provider_id: p.to_string(),
                model_id: m.to_string(),
            })
            .or_else(|| self.agent_info.model.clone());
        if let Some(model_ref) = resolved_model {
            agent.model = Some(model_ref);
        }

        let summary_prompt =
            "Summarise the conversation so far into a concise representation that \
             preserves all important context, decisions, code changes, file paths, \
             and outstanding tasks. Output only the summary — no preamble."
                .to_string();

        self.auto_compact_in_progress = auto_triggered;
        self.compact_in_progress = true;
        if auto_triggered {
            self.auto_compact_failed = false;
            self.status = "compacting before send…".to_string();
            self.push_log_no_agent(
                LogLevel::Warn,
                "Auto-compaction triggered (context near limit)".to_string(),
            );
        } else {
            self.status = "compacting…".to_string();
            self.push_log_no_agent(LogLevel::Info, "Compaction started".to_string());
        }

        let processor = self.session_processor.clone();
        let event_bus = self.event_bus.clone();
        tokio::spawn(async move {
            match processor
                .process_message(
                    &sid,
                    &summary_prompt,
                    &agent,
                    Arc::new(AtomicBool::new(false)),
                )
                .await
            {
                Ok(_) => {
                    tracing::info!(session_id = %sid, "Compaction LLM call completed");
                    // Signal the TUI to replace the session history with the summary.
                    // The actual replacement is performed in the Event::MessageEnd handler
                    // when was_auto_compaction is true (or after MessageEnd for manual compact).
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

    fn dispatch_user_message(&mut self, text: String, image_paths: Vec<std::path::PathBuf>) {
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
        self.add_to_history(text.clone());
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
            self.push_log_no_agent(
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
        self.push_log_no_agent(
            LogLevel::Info,
            format!("prompt sent{}: {}", model_tag, truncated),
        );

        let mut agent = self.agent_info.clone();
        // Apply the globally-selected model when:
        //   1. The agent has no model at all, OR
        //   2. The agent's model was not explicitly pinned by a custom profile
        //      (built-in agents carry an anthropic default that should be
        //      overridden by the user's /provider selection).
        if (!agent.model_pinned || agent.model.is_none())
            && let Some(ref model_str) = self.selected_model
            && let Some((provider, model)) = model_str.split_once('/')
        {
            agent.model = Some(ModelRef {
                provider_id: provider.to_string(),
                model_id: model.to_string(),
            });
        }

        // Inject role-mode system prompt addition when a role mode is active.
        if let Some(ref mode) = self.role_mode {
            let addition = mode.system_prompt_addition();
            if !addition.is_empty() {
                let existing = agent.prompt.clone().unwrap_or_default();
                agent.prompt = Some(format!("{}\n\n{}", existing, addition));
            }
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
                if let Err(e) = processor
                    .process_message(&sid, &final_text, &agent, flag)
                    .await
                {
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
                if let Some(&(pid, pname)) = PROVIDER_LIST.iter().find(|(id, _)| *id == preferred) {
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
        // Check Generic OpenAI API
        if !is_disabled("generic_openai") {
            if let Ok(key) = std::env::var("GENERIC_OPENAI_API_KEY") {
                if !key.is_empty() {
                    return Some(ConfiguredProvider {
                        id: "generic_openai".into(),
                        name: "Generic OpenAI API".into(),
                        source: ProviderSource::EnvVar,
                    });
                }
            } else if let Ok(key) = std::env::var("OPENAI_API_KEY")
                && !key.is_empty()
            {
                return Some(ConfiguredProvider {
                    id: "generic_openai".into(),
                    name: "Generic OpenAI API".into(),
                    source: ProviderSource::EnvVar,
                });
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
        if !is_disabled("ollama_cloud") {
            if let Ok(key) = std::env::var("OLLAMA_API_KEY") {
                if !key.is_empty() {
                    return Some(ConfiguredProvider {
                        id: "ollama_cloud".into(),
                        name: "Ollama Cloud".into(),
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
    pub fn input_len_chars(&self) -> usize {
        self.input.chars().count()
    }

    #[inline]
    fn assert_input_cursor_invariant(&self) {
        debug_assert!(self.input_cursor <= self.input_len_chars());
    }

    #[inline]
    fn pane_area(&self, pane: SelectionPane) -> Rect {
        match pane {
            SelectionPane::Messages => self.message_area,
            SelectionPane::Log => self.log_area,
            SelectionPane::Input => self.input_area,
        }
    }

    #[inline]
    fn assert_ui_invariants(&self) {
        self.assert_input_cursor_invariant();
        if let Some(sel) = &self.text_selection {
            debug_assert!(
                self.pane_area(sel.pane).area() > 0,
                "selection pane {:?} has no active area",
                sel.pane
            );
        }
        if let Some(menu) = &self.context_menu {
            debug_assert!(
                self.pane_area(menu.pane).area() > 0,
                "context menu pane {:?} has no active area",
                menu.pane
            );
        }
    }

    #[allow(unused_variables)]
    fn debug_log_input_transition(&self, source: &str, before_input: &str, before_cursor: usize) {
        #[cfg(debug_assertions)]
        {
            if before_input != self.input || before_cursor != self.input_cursor {
                tracing::debug!(
                    source,
                    before_chars = before_input.chars().count(),
                    before_cursor,
                    after_chars = self.input_len_chars(),
                    after_cursor = self.input_cursor,
                    screen = ?self.current_screen,
                    slash_menu = self.slash_menu.is_some(),
                    file_menu = self.file_menu.is_some(),
                    "input transition"
                );
            }
        }
    }

    /// Set cursor to a clamped character index.
    pub(crate) fn set_cursor_char_index_clamped(&mut self, index: usize) {
        self.input_cursor = index.min(self.input_len_chars());
        self.assert_input_cursor_invariant();
    }

    /// Refresh slash/file menus based on current input.
    pub(crate) fn refresh_input_menus(&mut self) {
        if self.input.starts_with('/') {
            self.update_slash_menu();
        } else {
            self.slash_menu = None;
        }
        if self.input.contains('@') {
            self.update_file_menu();
        } else {
            self.file_menu = None;
        }
    }

    fn refresh_project_files_cache(&mut self) {
        let wd = std::env::current_dir().unwrap_or_default();
        let files = ragent_core::reference::fuzzy::collect_project_files(&wd, 10_000);
        self.project_files_cache_count = files.len();
        self.project_files_cache = Some(files);
        self.project_files_cache_cwd = Some(wd);
        self.project_files_cache_refreshed_at = Some(std::time::SystemTime::now());
    }

    /// Insert a single character at the current cursor position.
    pub fn insert_char_at_cursor(&mut self, c: char) {
        let insert_pos = self.cursor_byte_pos();
        self.input.insert(insert_pos, c);
        self.cursor_move_right();
        self.refresh_input_menus();
        self.assert_input_cursor_invariant();
    }

    /// Insert text at the current cursor position.
    pub fn insert_text_at_cursor(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let insert_pos = self.cursor_byte_pos();
        let added = text.chars().count();
        self.input.insert_str(insert_pos, text);
        self.set_cursor_char_index_clamped(self.input_cursor + added);
        self.refresh_input_menus();
        self.assert_input_cursor_invariant();
    }

    /// Delete the character before the cursor, if any.
    pub fn delete_prev_char(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let delete_pos = self.cursor_byte_pos_at_char_index(self.input_cursor - 1);
        self.input.remove(delete_pos);
        self.cursor_move_left();
        self.refresh_input_menus();
        self.assert_input_cursor_invariant();
    }

    /// Delete the character at the cursor, if any.
    pub fn delete_next_char(&mut self) {
        if self.input_cursor >= self.input_len_chars() {
            return;
        }
        let delete_pos = self.cursor_byte_pos();
        self.input.remove(delete_pos);
        self.refresh_input_menus();
        self.assert_input_cursor_invariant();
    }

    /// Remove a char-index range from input and place cursor at the range start.
    pub fn remove_input_char_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let clamped_start = start.min(self.input_len_chars());
        let clamped_end = end.min(self.input_len_chars());
        if clamped_start >= clamped_end {
            return;
        }
        let byte_start = self.cursor_byte_pos_at_char_index(clamped_start);
        let byte_end = self.cursor_byte_pos_at_char_index(clamped_end);
        let _removed = clamped_end - clamped_start;
        self.input.replace_range(byte_start..byte_end, "");
        self.set_cursor_char_index_clamped(clamped_start);
        self.refresh_input_menus();
        self.assert_input_cursor_invariant();
    }

    /// Compute selected input char range for input/home-input panes.
    fn input_selection_char_range(&self, sel: &TextSelection) -> Option<(usize, usize)> {
        if !matches!(sel.pane, SelectionPane::Input) {
            return None;
        }
        let area = self.input_area;
        if area.width < 2 || area.height < 2 {
            return None;
        }
        let inner_x = area.x + 1;
        let inner_y = area.y + 1;
        let inner_w = area.width.saturating_sub(2).max(1) as usize;
        let ((start_col, start_row), (end_col, end_row)) = sel.normalized();
        let start_disp = start_row.saturating_sub(inner_y) as usize * inner_w
            + start_col.saturating_sub(inner_x) as usize;
        let end_disp_exclusive = end_row.saturating_sub(inner_y) as usize * inner_w
            + end_col.saturating_sub(inner_x) as usize
            + 1;
        let display_len = self.input_len_chars() + 2; // "> " prefix
        let start_disp = start_disp.min(display_len);
        let end_disp_exclusive = end_disp_exclusive.min(display_len);
        if end_disp_exclusive <= start_disp {
            return None;
        }
        let start_input = start_disp.saturating_sub(2).min(self.input_len_chars());
        let end_input = end_disp_exclusive
            .saturating_sub(2)
            .min(self.input_len_chars());
        if end_input <= start_input {
            None
        } else {
            Some((start_input, end_input))
        }
    }

    /// Return the currently active input widget area for overlay geometry.
    fn active_input_widget_area(&self) -> Rect {
        self.input_area
    }

    /// Return the byte offset corresponding to the current cursor position.
    pub fn cursor_byte_pos(&self) -> usize {
        self.cursor_byte_pos_at_char_index(self.input_cursor)
    }

    /// Return the byte offset corresponding to a character index.
    pub fn cursor_byte_pos_at_char_index(&self, char_index: usize) -> usize {
        if char_index == 0 {
            return 0;
        }
        // Single pass: nth() returns None when char_index is past the end.
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
        self.assert_input_cursor_invariant();
    }

    /// Move the cursor one character to the right (if possible).
    pub(crate) fn cursor_move_right(&mut self) {
        if self.input_cursor < self.input_len_chars() {
            self.input_cursor += 1;
        }
        self.assert_input_cursor_invariant();
    }

    /// Move cursor one word to the left.
    pub(crate) fn cursor_move_word_left(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let chars: Vec<char> = self.input.chars().collect();
        let mut i = self.input_cursor.min(chars.len());
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        self.set_cursor_char_index_clamped(i);
    }

    /// Move cursor one word to the right.
    pub(crate) fn cursor_move_word_right(&mut self) {
        let chars: Vec<char> = self.input.chars().collect();
        let len = chars.len();
        let mut i = self.input_cursor.min(len);
        while i < len && !chars[i].is_whitespace() {
            i += 1;
        }
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
        self.set_cursor_char_index_clamped(i);
    }

    /// Move the cursor to the beginning of the input line.
    pub(crate) fn cursor_move_home(&mut self) {
        self.input_cursor = 0;
        self.assert_input_cursor_invariant();
    }

    /// Move the cursor to the end of the input line.
    pub(crate) fn cursor_move_end(&mut self) {
        self.input_cursor = self.input_len_chars();
        self.assert_input_cursor_invariant();
    }

    /// Returns `true` if the cursor is on the first logical line (no `\n` before it).
    pub(crate) fn cursor_on_first_logical_line(&self) -> bool {
        let byte = self.cursor_byte_pos();
        !self.input[..byte].contains('\n')
    }

    /// Returns `true` if the cursor is on the last logical line (no `\n` after it).
    pub(crate) fn cursor_on_last_logical_line(&self) -> bool {
        let byte = self.cursor_byte_pos();
        !self.input[byte..].contains('\n')
    }

    /// Move cursor up one logical line (split on `\n`), staying in the same column.
    /// Does nothing if already on the first line.
    pub(crate) fn cursor_move_up_logical_line(&mut self) {
        let byte = self.cursor_byte_pos();
        let before = &self.input[..byte];
        let Some(nl_pos) = before.rfind('\n') else {
            return;
        };

        // Column (char count) within current line
        let line_start_byte = nl_pos + 1;
        let col = before[line_start_byte..].chars().count();

        // Previous line spans from after its preceding '\n' (or 0) to nl_pos
        let prev_line_start = before[..nl_pos].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let prev_line_len = before[prev_line_start..nl_pos].chars().count();

        let target_col = col.min(prev_line_len);
        let new_char = self.input[..prev_line_start].chars().count() + target_col;
        self.set_cursor_char_index_clamped(new_char);
    }

    /// Move cursor down one logical line (split on `\n`), staying in the same column.
    /// Does nothing if already on the last line.
    pub(crate) fn cursor_move_down_logical_line(&mut self) {
        let byte = self.cursor_byte_pos();
        let after = &self.input[byte..];
        let Some(nl_offset) = after.find('\n') else {
            return;
        };

        // Column within current line
        let before = &self.input[..byte];
        let line_start_byte = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
        let col = before[line_start_byte..].chars().count();

        // Next line
        let next_start = byte + nl_offset + 1;
        let next_line = &self.input[next_start..];
        let next_line_end = next_line.find('\n').unwrap_or(next_line.len());
        let next_line_len = next_line[..next_line_end].chars().count();

        let target_col = col.min(next_line_len);
        let new_char = self.input[..next_start].chars().count() + target_col;
        self.set_cursor_char_index_clamped(new_char);
    }

    /// Return the `[start, end)` char-index range for the active keyboard
    /// selection, or `None` when no selection is active or when anchor equals
    /// cursor (zero-width selection).
    pub fn kb_selection_char_range(&self) -> Option<(usize, usize)> {
        let anchor = self.kb_select_anchor?;
        let cursor = self.input_cursor;
        if anchor == cursor {
            None
        } else if anchor < cursor {
            Some((anchor, cursor))
        } else {
            Some((cursor, anchor))
        }
    }

    /// Copy the active keyboard selection to the system clipboard.
    /// Does nothing when no selection is active.
    pub(crate) fn copy_kb_selection(&mut self) {
        if let Some((start, end)) = self.kb_selection_char_range() {
            let selected: String = self.input.chars().skip(start).take(end - start).collect();
            Self::set_clipboard(&selected);
        }
    }

    /// Cut the active keyboard selection: copies to clipboard then removes it.
    /// Does nothing when no selection is active.
    pub(crate) fn cut_kb_selection(&mut self) {
        if let Some((start, end)) = self.kb_selection_char_range() {
            let selected: String = self.input.chars().skip(start).take(end - start).collect();
            Self::set_clipboard(&selected);
            self.remove_input_char_range(start, end);
            self.kb_select_anchor = None;
        }
    }

    /// Paste text from the system clipboard at the cursor (replacing the
    /// keyboard selection if one is active). Newlines are preserved; carriage
    /// returns are stripped.
    pub(crate) fn paste_text_from_clipboard(&mut self) {
        if let Some(text) = Self::get_clipboard() {
            // Replace selection if one is active.
            if let Some((start, end)) = self.kb_selection_char_range() {
                self.remove_input_char_range(start, end);
                self.kb_select_anchor = None;
            }
            let clean: String = text.chars().filter(|&c| c != '\r').collect();
            self.insert_text_at_cursor(&clean);
        }
    }

    /// Paste text into the active provider-setup input field.
    pub fn paste_text_into_provider_setup(&mut self, text: &str) {
        let clean: String = text.chars().filter(|&c| c != '\r').collect();
        let Some(step) = self.provider_setup.as_mut() else {
            return;
        };
        if let ProviderSetupStep::EnterKey {
            key_input,
            key_cursor,
            endpoint_input,
            endpoint_cursor,
            editing_endpoint,
            ..
        } = step
        {
            if *editing_endpoint {
                let insert_pos = endpoint_input
                    .char_indices()
                    .nth(*endpoint_cursor)
                    .map(|(byte, _)| byte)
                    .unwrap_or_else(|| endpoint_input.len());
                endpoint_input.insert_str(insert_pos, &clean);
                *endpoint_cursor += clean.chars().count();
            } else {
                let insert_pos = key_input
                    .char_indices()
                    .nth(*key_cursor)
                    .map(|(byte, _)| byte)
                    .unwrap_or_else(|| key_input.len());
                key_input.insert_str(insert_pos, &clean);
                *key_cursor += clean.chars().count();
            }
        }
    }

    /// Paste clipboard text into the active provider-setup input field.
    pub(crate) fn paste_provider_setup_from_clipboard(&mut self) {
        if let Some(text) = Self::get_clipboard() {
            self.paste_text_into_provider_setup(&text);
        }
    }

    /// Clear the keyboard selection anchor without moving the cursor.
    #[inline]
    pub(crate) fn clear_kb_selection(&mut self) {
        self.kb_select_anchor = None;
    }

    /// Delete the word immediately before the cursor.
    pub(crate) fn delete_prev_word(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let end = self.input_cursor;
        self.cursor_move_word_left();
        let start = self.input_cursor;
        self.remove_input_char_range(start, end);
    }

    /// Delete from cursor to end of line.
    pub(crate) fn delete_to_end_of_line(&mut self) {
        let end = self.input_len_chars();
        self.remove_input_char_range(self.input_cursor, end);
    }

    /// Attach an [`LspManager`] to the app.
    ///
    /// Called from `run_tui()` after the manager has been created and initial
    /// server connections have been started. The app keeps the manager alive
    /// and uses it for `/lsp` command operations.
    pub fn set_lsp_manager(&mut self, manager: SharedLspManager) {
        self.lsp_manager = Some(manager);
    }

    /// Set the code index reference.
    ///
    /// Called from `run_tui()` after the code index has been initialized
    /// (if enabled in config). The app keeps the reference alive and uses it
    /// for `/codeindex show` to display real-time statistics.
    pub fn set_code_index(&mut self, code_index: Option<Arc<ragent_code::CodeIndex>>) {
        self.code_index = code_index;
    }

    /// Register the primary session's short_sid → agent_name mapping.
    ///
    /// This must be called after setting `self.session_id` so that tool call
    /// step tags display the agent name (e.g. `[general:5]`) instead of the
    /// raw session ID suffix.
    pub fn register_primary_session_mapping(&mut self) {
        if let Some(ref sid) = self.session_id {
            let short_sid = short_session_id(sid);
            self.sid_to_display_name
                .insert(short_sid, self.agent_name.clone());
        }
    }

    /// Toggle the `disabled` flag for a configured LSP server in ragent.json.
    pub fn toggle_lsp_server_enabled(&self, id: &str) -> Result<String, String> {
        let config_path = std::env::current_dir()
            .unwrap_or_default()
            .join("ragent.json");

        let config = ragent_core::config::Config::load().unwrap_or_default();
        let currently_disabled = config.lsp.get(id).map(|c| c.disabled).unwrap_or(false);
        let new_disabled = !currently_disabled;
        let id_owned = id.to_string();

        atomic_config_update(&config_path, |json| {
            json["lsp"][&id_owned]["disabled"] = serde_json::json!(new_disabled);
            Ok(())
        })?;

        if new_disabled {
            Ok(format!("⚪ '{}' disabled. Restart ragent to apply.", id))
        } else {
            Ok(format!("🟢 '{}' enabled. Restart ragent to apply.", id))
        }
    }

    /// Add a discovered server to the `lsp` section in `ragent.json` and
    /// enable it. Returns `Ok(())` on success or an error description.
    pub fn enable_discovered_server(&self, server: &DiscoveredServer) -> Result<String, String> {
        use ragent_core::config::{Config, LspServerConfig};
        use std::collections::HashMap;

        // Load (or default-construct) the current config.
        let config = Config::load().unwrap_or_default();

        if config.lsp.contains_key(&server.id) {
            return Err(format!(
                "'{}' is already in ragent.json. Edit it manually to change settings.",
                server.id
            ));
        }

        let _cfg = LspServerConfig {
            command: Some(server.executable.to_string_lossy().into_owned()),
            args: server.args.clone(),
            env: HashMap::new(),
            extensions: server.extensions.clone(),
            disabled: false,
            timeout_ms: LspServerConfig::default_timeout_ms(),
        };

        // Persist back to ragent.json in the working directory.
        let config_path = std::env::current_dir()
            .unwrap_or_default()
            .join("ragent.json");

        let server_id = server.id.clone();
        let lsp_entry = serde_json::json!({
            "command": server.executable.to_string_lossy(),
            "args": server.args,
            "extensions": server.extensions,
            "disabled": false,
        });

        atomic_config_update(&config_path, |json| {
            json["lsp"][&server_id] = lsp_entry;
            Ok(())
        })?;

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
        let config = Config::load().unwrap_or_default();

        if config.mcp.contains_key(&server.id) {
            return Err(format!(
                "'{}' is already in ragent.json. Edit it manually to change settings.",
                server.id
            ));
        }

        // Persist back to ragent.json in the working directory.
        let config_path = std::env::current_dir()
            .unwrap_or_default()
            .join("ragent.json");

        let server_id = server.id.clone();
        let mcp_entry = serde_json::json!({
            "type": "stdio",
            "command": server.executable.to_string_lossy(),
            "args": server.args,
            "env": server.env,
            "disabled": false,
        });

        atomic_config_update(&config_path, |json| {
            json["mcp"][&server_id] = mcp_entry;
            Ok(())
        })?;

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
                self.session_id = Some(session.id.clone());
                // Map the primary session's short_sid to the current agent name
                let short_sid = short_session_id(&session.id);
                self.sid_to_display_name
                    .insert(short_sid, self.agent_name.clone());
                true
            }
            Err(e) => {
                // Surface a visible assistant message so slash commands don't fail silently.
                self.status = format!("error: {}", e);
                let msg = format!("From: /system\nFailed to create session: {}", e);
                self.append_assistant_text(&msg);
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
        self.ensure_team_manager_for_team_inner(team_name, known_team_dir, false);
    }

    /// Internal helper: optionally trigger reconciliation of queued `Spawning` members.
    ///
    /// Pass `reconcile = true` only when the team was created via an LLM tool call
    /// (where the TeamManager didn't exist during blueprint seeding) so that the
    /// queued members are started now that the manager is available.
    fn ensure_team_manager_for_team_inner(
        &mut self,
        team_name: &str,
        known_team_dir: Option<std::path::PathBuf>,
        reconcile: bool,
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
                    self.push_log_no_agent(
                        LogLevel::Warn,
                        format!("TeamManager init skipped: cannot load team '{team_name}': {e}"),
                    );
                    return;
                }
            }
        };

        // Parse the currently selected model so teammates inherit it in the reconcile loop.
        let active_model: Option<ragent_core::agent::ModelRef> =
            self.selected_model.as_deref().and_then(|s| {
                s.split_once('/')
                    .map(|(pid, mid)| ragent_core::agent::ModelRef {
                        provider_id: pid.to_string(),
                        model_id: mid.to_string(),
                    })
            });

        let mut manager = TeamManager::new(
            team_name.to_string(),
            lead_session_id,
            team_dir,
            self.session_processor.clone(),
            self.event_bus.clone(),
        );
        manager.active_model = active_model;
        let manager = Arc::new(manager);

        if self
            .session_processor
            .team_manager
            .set(manager.clone())
            .is_ok()
        {
            self.push_log_no_agent(
                LogLevel::Info,
                format!("TeamManager initialised for team '{team_name}'"),
            );
            // Only reconcile when explicitly requested (i.e. when the team was seeded
            // via the LLM tool path and members may be queued in Spawning state).
            if reconcile {
                manager.reconcile_spawning_members();
            }
        }
    }

    /// Best-effort hydration for teammate session IDs from on-disk team config.
    ///
    /// Some events (e.g. spawn) may arrive before session IDs are fully reflected
    /// in-memory. This keeps UI activity metrics accurate.
    pub fn refresh_team_member_session_ids(&mut self) {
        let Some(team_name) = self.active_team.as_ref().map(|t| t.name.clone()) else {
            return;
        };
        let working_dir = std::env::current_dir().unwrap_or_default();
        let Ok(store) = TeamStore::load_by_name(&team_name, &working_dir) else {
            return;
        };

        for member in &mut self.team_members {
            // If a stored entry exists for this agent, copy session_id, status,
            // and current_task_id so the UI reflects the authoritative on-disk state.
            if let Some(stored_member) = store
                .config
                .members
                .iter()
                .find(|m| m.agent_id == member.agent_id)
            {
                if member.session_id.is_none() {
                    if let Some(sid) = &stored_member.session_id {
                        member.session_id = Some(sid.clone());
                    }
                }
                // Always sync status and current task from the store so races
                // between disk hydration and spawn events don't leave the UI
                // showing an outdated "spawning" state.
                member.status = stored_member.status.clone();
                member.current_task_id = stored_member.current_task_id.clone();
            }
        }
        // Register session_id → teammate name mappings for log display.
        for member in &self.team_members {
            if let Some(ref sid) = member.session_id {
                let short_sid = short_session_id(sid);
                self.sid_to_display_name
                    .entry(short_sid)
                    .or_insert_with(|| member.name.clone());
            }
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
        // Map the primary session's short_sid to the current agent name
        let short_sid = short_session_id(session_id);
        self.sid_to_display_name
            .insert(short_sid, self.agent_name.clone());
        self.messages = messages;
        self.current_screen = ScreenMode::Chat;
        self.status = format!("resumed ({} messages)", msg_count);

        // Rebuild tool_step_map from restored tool calls and populate log
        // (step count comes from event_bus, not local counter)
        self.tool_step_map.clear();
        self.last_step_per_session.clear();
        self.substep_counter_per_session.clear();
        self.sid_to_display_name.clear();
        // Map the primary session's short_sid to the current agent name
        let short_sid = short_session_id(session_id);
        self.sid_to_display_name
            .insert(short_sid, self.agent_name.clone());
        let mut restored_logs: Vec<(u32, u32, String, String)> = Vec::new();
        let mut step_counter = 0u32;
        for msg in &self.messages {
            for part in &msg.parts {
                if let MessagePart::ToolCall {
                    call_id,
                    tool,
                    state,
                } = part
                {
                    // For restoration, treat each tool call as a unique step.1
                    step_counter += 1;
                    let substep = 1u32;
                    let short_sid = self
                        .session_id
                        .as_deref()
                        .map(short_session_id)
                        .unwrap_or_default();
                    self.tool_step_map
                        .insert(call_id.clone(), (short_sid, step_counter, substep));
                    let icon = match state.status {
                        ragent_core::message::ToolCallStatus::Completed => "✓",
                        ragent_core::message::ToolCallStatus::Error => "✗",
                        _ => "…",
                    };
                    restored_logs.push((step_counter, substep, tool.clone(), icon.to_string()));
                }
            }
        }
        for (step, substep, tool, icon) in restored_logs {
            let short_sid = self
                .session_id
                .as_deref()
                .map(short_session_id)
                .unwrap_or_default();
            self.push_log_no_agent(
                LogLevel::Tool,
                format!("[{short_sid}:{step}.{substep}] {tool} {icon} (restored)"),
            );
        }

        // Update cwd to match the session's working directory
        if !session.directory.is_empty() {
            self.cwd = session.directory.clone();
        }

        self.push_log_no_agent(
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
        if provider_id == "ollama_cloud" {
            if let Some(token) = self.ollama_cloud_api_key() {
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    let result = tokio::task::block_in_place(|| {
                        handle.block_on(
                            ragent_core::provider::ollama_cloud::list_ollama_cloud_models(
                                &token, None,
                            ),
                        )
                    });
                    if let Ok(models) = result
                        && !models.is_empty()
                    {
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

    /// Returns the active `provider/model` identifier for the current session, if any.
    fn active_model_ref_string(&self) -> Option<String> {
        self.selected_model.clone().or_else(|| {
            self.agent_info
                .model
                .as_ref()
                .map(|model| format!("{}/{}", model.provider_id, model.model_id))
        })
    }

    /// Summarise `/llmstats` samples for the currently active model.
    ///
    /// The summary is based on completed request samples recorded from the
    /// current session only.
    pub fn llm_stats_summary(&self) -> Option<LlmStatsSummary> {
        let model_ref = self.active_model_ref_string()?;
        let samples: Vec<&LlmRequestStat> = self
            .llm_request_stats
            .iter()
            .filter(|sample| sample.model_ref == model_ref)
            .collect();
        if samples.is_empty() {
            return None;
        }

        let count = samples.len() as f64;
        let avg_elapsed_ms = samples.iter().map(|s| s.elapsed_ms as f64).sum::<f64>() / count;
        let avg_prompt_tps = samples
            .iter()
            .map(|s| Self::tokens_per_second(s.input_tokens, s.elapsed_ms))
            .sum::<f64>()
            / count;
        let avg_output_tps = samples
            .iter()
            .map(|s| Self::tokens_per_second(s.output_tokens, s.elapsed_ms))
            .sum::<f64>()
            / count;

        Some(LlmStatsSummary {
            samples: samples.len(),
            avg_elapsed_ms,
            avg_prompt_tps,
            avg_output_tps,
        })
    }

    /// Build a cost summary for the current session.
    pub fn cost_summary(&self) -> Option<String> {
        if self.llm_request_stats.is_empty() {
            return None;
        }

        #[derive(Default)]
        struct ProviderCost {
            input_tokens: u64,
            output_tokens: u64,
            samples: usize,
            cost_usd: f64,
        }

        let mut total_input_tokens = 0u64;
        let mut total_output_tokens = 0u64;
        let mut total_cost = 0.0f64;
        let mut by_provider: std::collections::HashMap<String, ProviderCost> =
            std::collections::HashMap::new();

        for sample in &self.llm_request_stats {
            let (provider_id, model_id) = sample
                .model_ref
                .split_once('/')
                .unwrap_or((&sample.model_ref, ""));
            let model = self.provider_registry.resolve_model(provider_id, model_id);
            let cost = model
                .map(|m| {
                    (sample.input_tokens as f64 * m.cost.input / 1_000_000.0)
                        + (sample.output_tokens as f64 * m.cost.output / 1_000_000.0)
                })
                .unwrap_or(0.0);

            total_input_tokens += sample.input_tokens;
            total_output_tokens += sample.output_tokens;
            total_cost += cost;

            let entry = by_provider.entry(provider_id.to_string()).or_default();
            entry.input_tokens += sample.input_tokens;
            entry.output_tokens += sample.output_tokens;
            entry.samples += 1;
            entry.cost_usd += cost;
        }

        let session_duration = self
            .session_id
            .as_deref()
            .and_then(|sid| {
                self.session_processor
                    .session_manager
                    .get_session(sid)
                    .ok()
                    .flatten()
            })
            .map(|session| chrono::Utc::now() - session.created_at)
            .map(|duration| {
                let seconds = duration.num_seconds().max(0);
                let hours = seconds / 3600;
                let minutes = (seconds % 3600) / 60;
                let secs = seconds % 60;
                if hours > 0 {
                    format!("{hours}h {minutes}m {secs}s")
                } else if minutes > 0 {
                    format!("{minutes}m {secs}s")
                } else {
                    format!("{secs}s")
                }
            })
            .unwrap_or_else(|| "unknown".to_string());

        let mut providers: Vec<_> = by_provider.into_iter().collect();
        providers.sort_by(|a, b| a.0.cmp(&b.0));

        let mut out = String::from("From: /cost\n");
        out.push_str(&format!("Samples: {}\n", self.llm_request_stats.len()));
        out.push_str(&format!("Session duration: {}\n", session_duration));
        out.push_str(&format!(
            "Total tokens: {} input / {} output\n",
            total_input_tokens, total_output_tokens
        ));
        out.push_str(&format!("Estimated cost: ${:.6}\n", total_cost));
        if !providers.is_empty() {
            out.push_str("\nBy provider:\n");
            for (provider, summary) in providers {
                out.push_str(&format!(
                    "  - {}: ${:.6} ({} in / {} out, {} samples)\n",
                    provider,
                    summary.cost_usd,
                    summary.input_tokens,
                    summary.output_tokens,
                    summary.samples
                ));
            }
        }

        Some(out)
    }

    /// Convert token counts and elapsed milliseconds into tokens per second.
    fn tokens_per_second(tokens: u64, elapsed_ms: u64) -> f64 {
        if elapsed_ms == 0 {
            return 0.0;
        }
        tokens as f64 / (elapsed_ms as f64 / 1000.0)
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
        let context_pct: Option<f32> = self.selected_model_context_window().map(|context_window| {
            (self.last_input_tokens as f32 / context_window as f32 * 100.0).min(100.0)
        });

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
        } else if provider_id == "ollama_cloud" {
            match context_pct {
                Some(p) => (format!("cloud ctx: {:.0}%", p), false),
                None => ("cloud".to_string(), false),
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
        let Some(active) = self.active_mention_span() else {
            self.file_menu = None;
            return;
        };
        let query = active.query(&self.input).to_string();

        if let Some(dir) = self.file_menu.as_ref().and_then(|m| m.current_dir.clone()) {
            self.populate_directory_menu(&dir, Some(&query));
            return;
        }

        // Lazily populate or refresh the project file cache when cwd changes.
        let wd = std::env::current_dir().unwrap_or_default();
        let cache_stale = self
            .project_files_cache_cwd
            .as_ref()
            .is_none_or(|cached| *cached != wd);
        if self.project_files_cache.is_none() || cache_stale {
            self.refresh_project_files_cache();
        }

        if let Some(ref candidates) = self.project_files_cache {
            let matches = ragent_core::reference::fuzzy::fuzzy_match(&query, candidates);

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

            let prev_selected = self.file_menu.as_ref().map(|m| m.selected).unwrap_or(0);
            self.file_menu = Some(FileMenuState {
                selected: prev_selected.min(entries.len().saturating_sub(1)),
                matches: entries,
                scroll_offset: 0,
                query,
                current_dir: None,
            });
        } else {
            self.file_menu = None;
        }
    }

    /// Populate the file menu with the immediate contents of `dir_rel`.
    fn populate_directory_menu(&mut self, dir_rel: &std::path::Path, filter: Option<&str>) {
        let wd = std::env::current_dir().unwrap_or_default();
        let abs = wd.join(dir_rel);
        let mut entries: Vec<FileMenuEntry> = Vec::new();
        let filter_lower = filter
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase());

        if abs.is_dir() {
            // Read the directory contents from disk (sorted)
            if let Ok(rd) = std::fs::read_dir(&abs) {
                let mut sorted: Vec<_> = rd.filter_map(|e| e.ok()).collect();
                sorted.sort_by_key(|e| e.file_name());
                for entry in sorted {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Skip hidden
                    if name.starts_with('.') && !self.file_menu_show_hidden {
                        continue;
                    }
                    if let Some(ref f) = filter_lower
                        && !name.to_lowercase().contains(f)
                    {
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

            // Add explicit "back to fuzzy search" action.
            entries.insert(
                0,
                FileMenuEntry {
                    display: "<back to fuzzy>".to_string(),
                    path: std::path::PathBuf::new(),
                    is_dir: true,
                },
            );
        }

        if entries.is_empty() {
            self.file_menu = None;
        } else {
            self.file_menu = Some(FileMenuState {
                selected: 0,
                matches: entries,
                scroll_offset: 0,
                query: filter.unwrap_or_default().to_string(),
                current_dir: Some(dir_rel.to_path_buf()),
            });
        }
    }

    /// Accept the currently selected file menu entry. If the selected entry is
    /// a directory, navigate into it and show its contents. Returns `true` if a
    /// file was inserted into the input (menu closed), or `false` if the menu
    /// remains open due to directory navigation.
    pub fn accept_file_menu_selection(&mut self) -> bool {
        if self
            .file_menu
            .as_ref()
            .is_some_and(|m| m.matches.is_empty())
        {
            return false;
        }
        // Clone the selected entry out of the menu to avoid holding an
        // immutable borrow of self while we call mutating methods below.
        let selected_entry: Option<FileMenuEntry> = self
            .file_menu
            .as_ref()
            .and_then(|m| m.matches.get(m.selected).cloned());

        if let Some(entry) = selected_entry {
            if entry.is_dir {
                if entry.display == "<back to fuzzy>" {
                    self.update_file_menu();
                    return false;
                }
                // Navigate into the directory instead of inserting it.
                self.populate_directory_menu(&entry.path, None);
                return false;
            } else {
                // Insert file path into the input and close the menu.
                let path = entry.display.clone();
                if let Some(active) = self.active_mention_span() {
                    let replacement = format!("@{path}");
                    self.input
                        .replace_range(active.at_start..active.token_end, &replacement);
                    let cursor_chars =
                        self.input[..active.at_start].chars().count() + replacement.chars().count();
                    self.set_cursor_char_index_clamped(cursor_chars);
                } else {
                    self.file_menu = None;
                    return false;
                }
                self.file_menu = None;
                return true;
            }
        }

        self.file_menu = None;
        false
    }

    fn mention_spans(&self) -> Vec<MentionSpan> {
        let bytes = self.input.as_bytes();
        let mut spans = Vec::new();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'@' {
                if i > 0 {
                    let prev = bytes[i - 1];
                    if prev.is_ascii_alphanumeric() || prev == b'.' {
                        i += 1;
                        continue;
                    }
                }
                let at_start = i;
                i += 1;
                let token_start = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i > token_start {
                    spans.push(MentionSpan {
                        at_start,
                        token_start,
                        token_end: i,
                    });
                }
                continue;
            }
            i += 1;
        }
        spans
    }

    fn active_mention_span(&self) -> Option<MentionSpan> {
        let cursor = self.cursor_byte_pos();
        let spans = self.mention_spans();
        spans
            .iter()
            .find(|span| cursor >= span.at_start && cursor <= span.token_end)
            .copied()
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
        // Top-level wrapper: single entry and single exit. Log invocation and
        // call the inner implementation which may return early. On return,
        // log completion and number of assistant output lines added.
        let stripped = raw.strip_prefix('/').unwrap_or(raw).trim();
        let (cmd, args) = stripped
            .split_once(char::is_whitespace)
            .map_or((stripped, ""), |(c, a)| (c, a.trim()));
        let start_lines = self.assistant_output_lines();
        self.push_log_no_agent(LogLevel::Info, format!("Executing /{} {}", cmd, args));

        // Retain the raw slash command in input history so users can recall it later.
        self.add_to_history(raw.to_string());

        // Call the original implementation moved to an inner function.
        self.execute_slash_command_inner(raw);

        // If the command spawned an async task (status begins with ⏳), defer
        // the "Finished" log entry — poll_pending_opt will emit it once the
        // background work completes.
        if self.status.starts_with('⏳') {
            return;
        }

        let end_lines = self.assistant_output_lines();
        let added = end_lines.saturating_sub(start_lines);
        self.push_log_no_agent(
            LogLevel::Info,
            format!("Finished /{} {} — {} lines output", cmd, args, added),
        );
    }

    // Original implementation moved to an inner function. Keep its signature
    // private so the public API has a single-entry single-exit wrapper.
    fn execute_slash_command_inner(&mut self, raw: &str) {
        let stripped = raw.strip_prefix('/').unwrap_or(raw).trim();
        self.input.clear();
        self.input_cursor = 0;
        self.slash_menu = None;
        self.scroll_offset = 0;
        self.force_new_message = true;
        self.assert_ui_invariants();

        // Split into command and optional argument text.
        let (cmd, args) = stripped
            .split_once(char::is_whitespace)
            .map_or((stripped, ""), |(c, a)| (c, a.trim()));

        // Central session gate for slash commands.
        // Commands may still choose to bypass this (e.g. quit/exit).
        if !matches!(cmd, "quit" | "exit") && !self.ensure_session() {
            return;
        }

        match cmd {
            "about" => {
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
                            let is_custom =
                                custom_names.contains(&a.name) || a.name.starts_with("custom:");
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
                        self.push_log_no_agent(
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
                        self.push_log_no_agent(LogLevel::Warn, format!("Unknown agent: {}", args));
                    }
                }
            }
            "agents" => {
                let mut output = String::from("From: /agents\n\nBuilt-in Agents:\n\n");

                let custom_names: std::collections::HashSet<String> = self
                    .custom_agent_defs
                    .iter()
                    .map(|d| d.agent_info.name.clone())
                    .collect();

                for agent in &self.cycleable_agents {
                    let is_custom =
                        custom_names.contains(&agent.name) || agent.name.starts_with("custom:");
                    if !is_custom {
                        let active = if agent.name == self.agent_name {
                            " ●"
                        } else {
                            ""
                        };
                        output.push_str(&format!(
                            "  {:<18} {}{}\n",
                            agent.name, agent.description, active
                        ));
                    }
                }

                if self.custom_agent_defs.is_empty() {
                    output.push_str(
                        "\nCustom Agents:\n\n  (none — place .json or .md files in .ragent/agents/ or ~/.ragent/agents/)\n",
                    );
                } else {
                    output.push_str("\nCustom Agents:\n\n");
                    for def in &self.custom_agent_defs {
                        let scope = if def.is_project_local {
                            "project"
                        } else {
                            "global"
                        };
                        let name = &def.agent_info.name;
                        let desc = &def.agent_info.description;
                        let active = if *name == self.agent_name { " ●" } else { "" };
                        let fmt =
                            if def.source_path.extension().and_then(|e| e.to_str()) == Some("md") {
                                "profile"
                            } else {
                                "oasf"
                            };
                        output.push_str(&format!(
                            "  {:<18} {} [{}/{}]{}\n",
                            name, desc, scope, fmt, active
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

                self.status = "agents".to_string();
            }
            "context" => match args.trim() {
                "refresh" => {
                    ragent_core::agent::clear_prompt_context_cache();
                    self.append_assistant_text(
                            "From: /context\n🔄 Context cache cleared — next message will recompute file tree, git status, and README."
                        );
                    self.push_log_no_agent(LogLevel::Info, "context cache cleared".to_string());
                    self.status = "context refreshed".to_string();
                }
                _ => {
                    self.append_assistant_text(
                            "From: /context\nUsage: `/context refresh` — clears cached file tree, git status, and README context"
                        );
                }
            },

            // ── /init ────────────────────────────────────────────────────────
            "init" => {
                let sid = self.session_id.clone().unwrap_or_default();
                self.append_assistant_text(
                    "From: /init\n🔍 **Analysing project…**\n\n\
                     The explore agent will examine the project structure, README, build files, \
                     and test layout, then write a summary to `.ragent/memory/PROJECT_ANALYSIS.md`. \
                     Future sessions will automatically load this context."
                );
                self.push_log_no_agent(
                    LogLevel::Info,
                    "init: starting project analysis".to_string(),
                );

                // Find the explore agent and dispatch the analysis task directly
                // (no agent-stack push — init runs as a one-shot subagent that writes memory).
                let explore_agent = self
                    .cycleable_agents
                    .iter()
                    .find(|a| a.name == "explore")
                    .cloned();

                let mut agent = explore_agent.unwrap_or_else(|| {
                    // Fallback: use current agent with a suitable prompt
                    self.agent_info.clone()
                });

                // Apply current model override
                if let Some(ref model_str) = self.selected_model {
                    if let Some((provider, model)) = model_str.split_once('/') {
                        agent.model = Some(ModelRef {
                            provider_id: provider.to_string(),
                            model_id: model.to_string(),
                        });
                    }
                }

                // Allow file writes so the agent can call memory_write
                agent.permission = ragent_core::agent::default_permissions();

                let task = "\
You are performing a one-time project analysis to build persistent memory for this codebase.\n\n\
Analyse the following aspects of the project:\n\
1. Programming language(s), frameworks, and key dependencies\n\
2. Overall architecture and module structure\n\
3. Entry points and main execution flow\n\
4. Build system and how to build/test the project\n\
5. Key conventions: naming, error handling, testing patterns\n\
6. Important files a developer should know about\n\n\
After your analysis, call the `memory_write` tool with:\n\
- scope: \"project\"\n\
- path: \"PROJECT_ANALYSIS.md\"\n\
- content: a well-structured markdown summary of your findings\n\n\
Be concise but comprehensive. This will be injected into future agent sessions automatically.\
"
                .to_string();

                let msg = Message::user_text(&sid, &task);
                self.messages.push(msg);

                let processor = self.session_processor.clone();
                let flag = Arc::new(AtomicBool::new(false));
                self.cancel_flag = Some(flag.clone());
                self.is_processing = true;
                self.status = "init: analysing project…".to_string();

                let event_bus = self.event_bus.clone();
                tokio::spawn(async move {
                    if let Err(e) = processor.process_message(&sid, &task, &agent, flag).await {
                        tracing::warn!(error = %e, "init: analysis failed");
                        event_bus.publish(ragent_core::event::Event::AgentError {
                            session_id: sid,
                            error: format!("init analysis failed: {e}"),
                        });
                    }
                });
            }
            "clear" => {
                self.messages.clear();
                self.scroll_offset = 0;
                self.tool_step_map.clear();
                self.last_step_per_session.clear();
                self.substep_counter_per_session.clear();
                ragent_core::agent::clear_prompt_context_cache();
                self.status = "messages cleared".to_string();
                self.push_log_no_agent(LogLevel::Info, "Message history cleared".to_string());
            }
            "browse_refresh" => {
                self.refresh_project_files_cache();
                self.status = format!(
                    "browse index refreshed ({})",
                    self.project_files_cache_count
                );
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!(
                        "@ picker index refreshed ({} entries)",
                        self.project_files_cache_count
                    ),
                );
            }
            "cancel" => {
                if args.is_empty() {
                    self.status = "⚠ Please provide a task ID prefix: /cancel <id>".to_string();
                    self.push_log_no_agent(LogLevel::Warn, "No task ID provided".to_string());
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
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!(
                            "Task cancelled: {}... ({})",
                            &task_id[..8.min(task_id.len())],
                            agent
                        ),
                    );
                } else {
                    self.status = format!("No task found with ID starting with '{}'", args);
                    self.push_log_no_agent(LogLevel::Warn, format!("Task not found: {}", args));
                }
            }
            "compact" => {
                let _ = self.start_compaction(false);
            }
            "cost" => {
                let Some(output) = self.cost_summary() else {
                    self.append_assistant_text(
                        "From: /cost\nNo completed LLM responses yet for this session.\n",
                    );

                    self.status = "cost unavailable".to_string();
                    return;
                };
                self.append_assistant_text(&output);

                self.status = "cost summary".to_string();
            }
            "help" => {
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

                self.status = "help".to_string();
            }
            "opt" => {
                // /opt help => show markdown table of available optimization methods
                if args.is_empty() || args == "help" {
                    let table = OptMethod::help_table();
                    self.append_assistant_text(&format!("From: /opt help\n\n{}", table));

                    self.status = "opt help".to_string();
                    return;
                }

                // /opt <method> <prompt>
                let (method_str, rest) = args
                    .split_once(char::is_whitespace)
                    .map_or((args, ""), |(m, r)| (m, r.trim()));

                if rest.is_empty() {
                    self.status = "⚠ Please provide a prompt: /opt <method> <prompt>".to_string();
                    return;
                }

                let method = match method_str.parse::<OptMethod>() {
                    Ok(m) => m,
                    Err(_) => {
                        self.status = format!("⚠ Unknown optimization method: {}", method_str);
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            format!("opt: unknown method '{}'", method_str),
                        );
                        return;
                    }
                };

                // Resolve provider / model from session config
                let (provider_id, model_id) = match self
                    .selected_model
                    .as_deref()
                    .and_then(|s| s.split_once('/'))
                    .map(|(p, m)| (p.to_string(), m.to_string()))
                {
                    Some(pair) => pair,
                    None => {
                        self.status =
                            "⚠ /opt requires a configured model (use /provider)".to_string();
                        return;
                    }
                };

                let registry = Arc::clone(&self.provider_registry);
                let storage = Arc::clone(&self.storage);
                let opt_result = Arc::clone(&self.opt_result);
                let user_prompt = rest.to_string();
                let method_name = method.name().to_string();

                self.status = format!("⏳ opt/{}: optimizing…", method_name);

                tokio::spawn(async move {
                    let completer = RagentCompleter {
                        registry,
                        storage,
                        provider_id,
                        model_id,
                    };
                    let outcome = optimize(method, &user_prompt, &completer)
                        .await
                        .map(|text| format!("[opt: {}]\n\n{}", method_name, text))
                        .map_err(|e| e.to_string());
                    if let Ok(mut guard) = opt_result.lock() {
                        *guard = Some(outcome);
                    } else {
                        tracing::error!("opt_result mutex poisoned, result dropped");
                    }
                });
            }
            "inputdiag" => {
                let selection = self
                    .text_selection
                    .as_ref()
                    .map(|s| format!("{:?} {:?}->{:?}", s.pane, s.anchor, s.endpoint))
                    .unwrap_or_else(|| "none".to_string());
                let context_menu = self
                    .context_menu
                    .as_ref()
                    .map(|m| format!("{:?} selected={}", m.pane, m.selected))
                    .unwrap_or_else(|| "none".to_string());
                let diag = format!(
                    "From: /inputdiag\n\
                                       Input diagnostics:\n\
                                         screen: {:?}\n\
                                         input chars: {}\n\
                                         input cursor: {}\n\
                                         slash menu: {}\n\
                                         file menu: {}\n\
                                         history picker: {}\n\
                                         selection: {}\n\
                                         context menu: {}\n\
                                         message area: {:?}\n\
                                         log area: {:?}\n\
                                         input area: {:?}\n\
                                         browse cache cwd: {:?}\n\
                                         browse cache entries: {}\n\
                                         browse cache refreshed: {:?}\n\
                                         browse menu state: {}",
                    self.current_screen,
                    self.input_len_chars(),
                    self.input_cursor,
                    self.slash_menu.is_some(),
                    self.file_menu.is_some(),
                    self.history_picker.is_some(),
                    selection,
                    context_menu,
                    self.message_area,
                    self.log_area,
                    self.input_area,
                    self.project_files_cache_cwd,
                    self.project_files_cache_count,
                    self.project_files_cache_refreshed_at,
                    self.file_menu
                        .as_ref()
                        .map(|m| format!(
                            "query='{}' dir={:?} selected={} offset={} results={}",
                            m.query,
                            m.current_dir,
                            m.selected,
                            m.scroll_offset,
                            m.matches.len()
                        ))
                        .unwrap_or_else(|| "none".to_string())
                );
                self.append_assistant_text(&diag);

                self.status = "inputdiag".to_string();
            }
            "log" => {
                self.show_log = !self.show_log;
                self.status = if self.show_log {
                    "log panel visible".to_string()
                } else {
                    "log panel hidden".to_string()
                };
            }
            "llmstats" => {
                let Some(model_ref) = self.active_model_ref_string() else {
                    self.status = "⚠ No active model selected".to_string();
                    self.push_log_no_agent(LogLevel::Warn, "llmstats: no active model".to_string());
                    return;
                };

                let Some(summary) = self.llm_stats_summary() else {
                    self.append_assistant_text(&format!(
                        "From: /llmstats\nNo completed LLM responses yet for {}.\n",
                        model_ref
                    ));

                    self.status = "llm stats unavailable".to_string();
                    return;
                };

                let output = format!(
                    "From: /llmstats\n\
                     Model: {}\n\
                     Samples: {}\n\
                     Average round-trip: {:.1} ms\n\
                     Average prompt parsing tokens/sec: {:.2}\n\
                     Average output tokens/sec: {:.2}\n",
                    model_ref,
                    summary.samples,
                    summary.avg_elapsed_ms,
                    summary.avg_prompt_tps,
                    summary.avg_output_tps
                );
                self.append_assistant_text(&output);

                self.status = "llm stats".to_string();
            }
            "history" => {
                if self.input_history.is_empty() {
                    self.status = "No input history yet".to_string();
                } else {
                    // Show newest entries first
                    let entries: Vec<String> = self.input_history.iter().rev().cloned().collect();
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
            "quit" | "exit" => {
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
                    let mut new_cycleable: Vec<_> =
                        builtin_agents.into_iter().filter(|a| !a.hidden).collect();
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
                        self.push_log_no_agent(LogLevel::Warn, format!("[reload agents] {}", d));
                    }
                    report.push_str(&format!(
                        "✓ Agents reloaded — {} custom agent(s) (was {})\n",
                        self.custom_agent_defs.len(),
                        prev_count,
                    ));
                    self.push_log_no_agent(
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
                            self.push_log_no_agent(
                                LogLevel::Info,
                                "reload config: ragent.json reloaded".to_string(),
                            );
                        }
                        Err(e) => {
                            report.push_str(&format!("✗ Config reload failed: {}\n", e));
                            self.push_log_no_agent(
                                LogLevel::Warn,
                                format!("reload config failed: {}", e),
                            );
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
                            self.push_log_no_agent(
                                LogLevel::Info,
                                format!(
                                    "reload mcp: {} server(s) in config",
                                    self.mcp_servers.len()
                                ),
                            );
                        }
                        Err(e) => {
                            report.push_str(&format!("✗ MCP reload failed: {}\n", e));
                            self.push_log_no_agent(
                                LogLevel::Warn,
                                format!("reload mcp failed: {}", e),
                            );
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
                    self.push_log_no_agent(
                        LogLevel::Info,
                        "reload skills: confirmed (on-demand)".to_string(),
                    );
                }

                if !matches!(sub, "all" | "agents" | "config" | "mcp" | "skills") {
                    report.push_str(&format!(
                        "Unknown subcommand '{}'. Usage: /reload [all|config|mcp|skills|agents]\n",
                        sub
                    ));
                }

                self.append_assistant_text(&report);

                self.status = "reload".to_string();
                // Reload bash lists alongside other config
                ragent_core::bash_lists::load_from_config();
            }
            "resume" => {
                if !self.agent_halted {
                    self.status = "Nothing to resume — agent was not halted".to_string();
                    self.push_log_no_agent(LogLevel::Warn, "Nothing to resume".to_string());
                    return;
                }
                if self.session_id.is_none() {
                    self.status = "No active session".to_string();
                    return;
                }

                self.agent_halted = false;
                let Some(sid) = self.session_id.clone() else {
                    self.status = "No active session".to_string();
                    return;
                };
                let resume_text = "You were previously interrupted by the user. Continue the task from where you left off.";
                let msg = Message::user_text(&sid, resume_text);
                self.messages.push(msg);
                self.status = "processing...".to_string();
                self.push_log_no_agent(LogLevel::Info, "Resuming halted agent".to_string());

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
                    } else {
                        self.status = "No system prompt set".to_string();
                    }
                } else {
                    self.agent_info.prompt = Some(args.to_string());
                    self.status = "system prompt updated".to_string();
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("System prompt set ({} chars)", args.len()),
                    );
                }
            }
            "tools" => {
                let tool_defs = self.session_processor.tool_registry.definitions();
                let mut output = String::from("From: /tools\nBuilt-in Tools:\n\n");

                // Prepare built-in tools table
                let built_tools: Vec<_> = tool_defs.iter().collect();
                if built_tools.is_empty() {
                    output.push_str("  (no built-in tools)\n");
                } else {
                    // Compute parameter strings and column widths
                    let mut param_strs: Vec<String> = Vec::new();
                    let mut name_w = "Name".len();
                    let mut params_w = "Params".len();
                    for def in &built_tools {
                        name_w = name_w.max(def.name.len());
                        let params = if let Some(props) =
                            def.parameters.get("properties").and_then(|v| v.as_object())
                        {
                            let keys: Vec<&str> = props.keys().map(|k| k.as_str()).collect();
                            keys.join(", ")
                        } else {
                            String::new()
                        };
                        params_w = params_w.max(params.len());
                        param_strs.push(params);
                    }
                    // Cap params column for readability
                    params_w = params_w.min(40);

                    // Helper: wrap text into lines no longer than `w` (word wrap)
                    fn wrap_text(s: &str, w: usize) -> Vec<String> {
                        if w == 0 {
                            return vec![s.to_string()];
                        }
                        let mut lines: Vec<String> = Vec::new();
                        let mut cur = String::new();
                        for word in s.split_whitespace() {
                            if cur.is_empty() {
                                if word.len() <= w {
                                    cur.push_str(word);
                                } else {
                                    // split long word
                                    let mut rem = word;
                                    while !rem.is_empty() {
                                        let take = rem.chars().take(w).collect::<String>();
                                        lines.push(take.clone());
                                        rem = &rem[take.len()..];
                                    }
                                }
                            } else if cur.len() + 1 + word.len() <= w {
                                cur.push(' ');
                                cur.push_str(word);
                            } else {
                                lines.push(cur);
                                cur = String::new();
                                if word.len() <= w {
                                    cur.push_str(word);
                                } else {
                                    let mut rem = word;
                                    while !rem.is_empty() {
                                        let take = rem.chars().take(w).collect::<String>();
                                        lines.push(take.clone());
                                        rem = &rem[take.len()..];
                                    }
                                }
                            }
                        }
                        if !cur.is_empty() {
                            lines.push(cur);
                        }
                        if lines.is_empty() {
                            lines.push(String::new());
                        }
                        lines
                    }

                    // Add a leading "No" column for incremental row numbers
                    let mut no_w = "No".len();
                    // Update name width to account for extra padding
                    no_w = no_w.max(2);
                    name_w = name_w.max(4);

                    // Compute description column width from a reasonable total width
                    let total_w = 120usize;
                    // +4 accounts for the extra spaces and separators between columns
                    let desc_w = total_w.saturating_sub(no_w + name_w + params_w + 8).max(20);

                    // Build wrapped table rows with a leading number column
                    let mut table_buf = String::new();
                    // Header
                    table_buf.push_str(&format!(
                        "  {:<no_w$}  {:<name_w$}  {:<params_w$}  {:<desc_w$}\n",
                        "No",
                        "Name",
                        "Params",
                        "Description",
                        no_w = no_w,
                        name_w = name_w,
                        params_w = params_w,
                        desc_w = desc_w
                    ));
                    table_buf.push_str(&format!(
                        "  {:-<no_w$}  {:-<name_w$}  {:-<params_w$}  {:-<desc_w$}\n",
                        "",
                        "",
                        "",
                        "",
                        no_w = no_w,
                        name_w = name_w,
                        params_w = params_w,
                        desc_w = desc_w
                    ));

                    for (i, def) in built_tools.iter().enumerate() {
                        let params = &param_strs[i];
                        let params_display = params.clone();
                        let name_lines = wrap_text(&def.name, name_w);
                        let params_lines = wrap_text(&params_display, params_w);
                        let desc_lines = wrap_text(&def.description, desc_w);
                        let row_lines = name_lines
                            .len()
                            .max(params_lines.len())
                            .max(desc_lines.len());
                        for r in 0..row_lines {
                            let no_cell = if r == 0 {
                                format!("{}", i + 1)
                            } else {
                                String::new()
                            };
                            let name_cell = name_lines.get(r).cloned().unwrap_or_default();
                            let params_cell = params_lines.get(r).cloned().unwrap_or_default();
                            let desc_cell = desc_lines.get(r).cloned().unwrap_or_default();
                            table_buf.push_str(&format!(
                                "  {:<no_w$}  {:<name_w$}  {:<params_w$}  {:<desc_w$}\n",
                                no_cell,
                                name_cell,
                                params_cell,
                                desc_cell,
                                no_w = no_w,
                                name_w = name_w,
                                params_w = params_w,
                                desc_w = desc_w
                            ));
                        }
                        table_buf.push_str("\n");
                    }

                    output.push_str("```text\n");
                    output.push_str(&table_buf);
                    if !output.ends_with('\n') {
                        output.push('\n');
                    }
                    output.push_str("```\n\n");
                }

                // MCP tools table
                let connected_servers: Vec<&McpServer> = self
                    .mcp_servers
                    .iter()
                    .filter(|s| s.status == ragent_core::mcp::McpStatus::Connected)
                    .collect();

                output.push_str("\nMCP Tools:\n\n");
                if connected_servers.is_empty() {
                    output.push_str("  (no MCP servers connected)\n");
                } else {
                    // Compute widths
                    let mut name_w = "Name".len();
                    let mut server_w = "Server".len();
                    for server in &connected_servers {
                        server_w = server_w.max(server.id.len() + 2); // account for brackets
                        for tool in &server.tools {
                            name_w = name_w.max(tool.name.len());
                        }
                    }
                    name_w = name_w.max(4);
                    server_w = server_w.max(6);

                    // Helper wrap (reuse same as above)
                    fn wrap_text(s: &str, w: usize) -> Vec<String> {
                        if w == 0 {
                            return vec![s.to_string()];
                        }
                        let mut lines: Vec<String> = Vec::new();
                        let mut cur = String::new();
                        for word in s.split_whitespace() {
                            if cur.is_empty() {
                                if word.len() <= w {
                                    cur.push_str(word);
                                } else {
                                    let mut rem = word;
                                    while !rem.is_empty() {
                                        let take = rem.chars().take(w).collect::<String>();
                                        lines.push(take.clone());
                                        rem = &rem[take.len()..];
                                    }
                                }
                            } else if cur.len() + 1 + word.len() <= w {
                                cur.push(' ');
                                cur.push_str(word);
                            } else {
                                lines.push(cur);
                                cur = String::new();
                                if word.len() <= w {
                                    cur.push_str(word);
                                } else {
                                    let mut rem = word;
                                    while !rem.is_empty() {
                                        let take = rem.chars().take(w).collect::<String>();
                                        lines.push(take.clone());
                                        rem = &rem[take.len()..];
                                    }
                                }
                            }
                        }
                        if !cur.is_empty() {
                            lines.push(cur);
                        }
                        if lines.is_empty() {
                            lines.push(String::new());
                        }
                        lines
                    }

                    // Add No column for MCP table
                    let mut no_w = "No".len();
                    no_w = no_w.max(2);
                    name_w = name_w.max(4);

                    let total_w = 120usize;
                    let desc_w = total_w.saturating_sub(no_w + name_w + server_w + 8).max(20);

                    let mut table_buf = String::new();
                    // Header with No
                    table_buf.push_str(&format!(
                        "  {:<no_w$}  {:<name_w$}  {:<server_w$}  {:<desc_w$}\n",
                        "No",
                        "Name",
                        "Server",
                        "Description",
                        no_w = no_w,
                        name_w = name_w,
                        server_w = server_w,
                        desc_w = desc_w
                    ));
                    table_buf.push_str(&format!(
                        "  {:-<no_w$}  {:-<name_w$}  {:-<server_w$}  {:-<desc_w$}\n",
                        "",
                        "",
                        "",
                        "",
                        no_w = no_w,
                        name_w = name_w,
                        server_w = server_w,
                        desc_w = desc_w
                    ));

                    let mut idx = 1usize;
                    for server in &connected_servers {
                        if server.tools.is_empty() {
                            continue;
                        }
                        for tool in &server.tools {
                            let name_lines = wrap_text(&tool.name, name_w);
                            let server_lines = wrap_text(&format!("[{}]", server.id), server_w);
                            let desc_lines = wrap_text(&tool.description, desc_w);
                            let row_lines = name_lines
                                .len()
                                .max(server_lines.len())
                                .max(desc_lines.len());
                            for r in 0..row_lines {
                                let no_cell = if r == 0 {
                                    format!("{}", idx)
                                } else {
                                    String::new()
                                };
                                let name_cell = name_lines.get(r).cloned().unwrap_or_default();
                                let server_cell = server_lines.get(r).cloned().unwrap_or_default();
                                let desc_cell = desc_lines.get(r).cloned().unwrap_or_default();
                                table_buf.push_str(&format!(
                                    "  {:<no_w$}  {:<name_w$}  {:<server_w$}  {:<desc_w$}\n",
                                    no_cell,
                                    name_cell,
                                    server_cell,
                                    desc_cell,
                                    no_w = no_w,
                                    name_w = name_w,
                                    server_w = server_w,
                                    desc_w = desc_w
                                ));
                            }
                            table_buf.push_str("\n");
                            idx += 1;
                        }
                    }

                    output.push_str("```text\n");
                    output.push_str(&table_buf);
                    if !output.ends_with('\n') {
                        output.push('\n');
                    }
                    output.push_str("```\n");
                }

                self.append_assistant_text(&output);

                self.status = "tools".to_string();
            }
            "skills" => {
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

                self.status = "skills".to_string();
            }
            "tasks" => {
                if self.active_tasks.is_empty() {
                    self.status = "No active background tasks".to_string();
                    self.push_log_no_agent(LogLevel::Info, "No active tasks".to_string());
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

                self.status = "tasks".to_string();
            }
            "lsp" => {
                let lsp_args: Vec<&str> = args.split_whitespace().collect();
                let sub = lsp_args.first().copied().unwrap_or("");
                match sub {
                    "edit" => {
                        // Build list of configured servers from config.
                        let mut servers: Vec<(String, bool)> = ragent_core::config::Config::load()
                            .unwrap_or_default()
                            .lsp
                            .into_iter()
                            .map(|(id, cfg)| (id, cfg.disabled))
                            .collect();
                        servers.sort_by(|a, b| a.0.cmp(&b.0));

                        if servers.is_empty() {
                            let msg = "No LSP servers configured in ragent.json.\nRun /lsp discover to find and enable servers first.";
                            self.append_assistant_text(&format!("From: /lsp edit\n{}", msg));
                        } else {
                            self.lsp_edit = Some(LspEditState {
                                servers,
                                selected: 0,
                                scroll_offset: 0,
                                feedback: None,
                            });
                        }

                        return;
                    }
                    "discover" => {
                        // Run discovery synchronously using block_in_place.
                        let found = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(LspManager::discover())
                        });
                        // Show interactive discover dialog.
                        self.lsp_discover = Some(LspDiscoverState {
                            servers: found,
                            number_input: String::new(),
                            number_cursor: 0,
                            feedback: None,
                            scroll_offset: 0,
                        });

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
                        out.push_str("\nSubcommands: /lsp discover  /lsp edit  /lsp connect <id>  /lsp disconnect <id>\n");
                        self.append_assistant_text(&out);
                    }
                }

                self.status = "lsp".to_string();
            }
            "mcp" => {
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
                            number_cursor: 0,
                            feedback: None,
                        });

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

                        return;
                    }
                    "disconnect" => {
                        if let Some(&id) = mcp_args.get(1) {
                            self.status =
                                format!("MCP disconnect not yet implemented for '{}'", id);
                        } else {
                            self.status = "Usage: /mcp disconnect <id>".to_string();
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

                self.status = "mcp".to_string();
            }
            "team" | "teams" => {
                // Split "subcommand rest-of-args"
                let (sub, rest) = args
                    .split_once(char::is_whitespace)
                    .map_or((args, ""), |(s, r)| (s.trim(), r.trim()));
                let sub = if sub.is_empty() { "status" } else { sub };
                match sub {
                    "help" => {
                        let output = "From: /team help
## /team command reference

| Command | Arguments | Description |
|---|---|---|
| `/team help` | none | Show this command reference table. |
| `/team status` | none | Show the currently active team in this session. |
| `/team show [name]` | optional `name` | Show one team in detail, or all registered teams when no name is given. |
| `/team create <blueprint> [name]` | required `blueprint`, optional `name` | Create a new project-local team (blueprint mandatory) and set it active. |
| `/team close` | none | Close the active team in this session (does not delete on disk). |
| `/team delete <name>` | required `name` | Delete a team from disk (also clears active state if it is active). |
| `/team blueprint [name]` | optional `name` | List all installed blueprints, or show details of a specific blueprint. |
| `/team message <teammate-name> <text>` | required `teammate-name`, required `text` | Send a mailbox message from lead to a teammate. |
| `/team tasks` | none | Show the task table for the active team. |
| `/team clear` | none | Clear/remove the active team task list file. |
| `/team cleanup` | none | Clean up the active team (requires no working teammates). |
| `/team focus [name]` | optional `name` | Focus on a teammate (shows output, routes input). No arg clears focus. |

Alias: `/teams ...` routes to `/team ...` (for example `/teams help`, `/teams show`).";
                        self.append_assistant_text(output);
                        self.status = "team: help".to_string();
                    }
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
                                    let model_str = m
                                        .model_override
                                        .as_ref()
                                        .map(|mr| format!("{}/{}", mr.provider_id, mr.model_id))
                                        .unwrap_or_else(|| "(inherited)".to_string());
                                    output.push_str(&format!(
                                        "  └ {:<18} {:<10} model:{:<30} task:{}\n",
                                        m.name, status, model_str, task
                                    ));
                                }
                            }
                            output
                                .push_str(&format!("\n{} teammate(s)\n", self.team_members.len()));
                        } else {
                            output.push_str("No active team.\n\nUse `/team create <blueprint> [name]` to start a team (blueprint required).");
                        }
                        self.append_assistant_text(&output);
                        self.status = "team: status".to_string();
                    }
                    "create" => {
                        if rest.is_empty() {
                            self.status = "Usage: /team create <blueprint> [name]".to_string();
                            return;
                        }

                        // Parse blueprint (mandatory) then optional name
                        let mut parts = rest.split_whitespace();
                        let blueprint = parts.next().unwrap_or("").to_string();
                        let mut name = parts.next().map(|s| s.to_string());

                        if blueprint.is_empty() {
                            self.status = "Usage: /team create <blueprint> [name]".to_string();
                            return;
                        }

                        // If no name provided, generate one from blueprint + timestamp
                        if name.is_none() {
                            let generated_name = format!(
                                "{}-{}",
                                blueprint,
                                chrono::Utc::now().format("%Y%m%d-%H-%M-%S")
                            );
                            name = Some(generated_name);
                        }
                        let name = name.expect("name guaranteed Some above");

                        let working_dir = std::env::current_dir().unwrap_or_default();
                        let sid = self.session_id.clone().unwrap_or_default();
                        match TeamStore::create(&name, &sid, &working_dir, true) {
                            Ok(store) => {
                                let name = store.config.name.clone();
                                let team_dir = store.dir.clone();
                                self.active_team = Some(store.config);
                                self.team_members.clear();
                                self.team_message_counts.clear();
                                self.show_teams = true;
                                self.ensure_team_manager_for_team(&name, Some(team_dir));
                                self.push_log_no_agent(
                                    LogLevel::Info,
                                    format!("🤝 Team '{}' created", name),
                                );
                                self.append_assistant_text(&format!(
                                    "From: /team create\nTeam '{}' created.\n\nUse the `team_spawn` tool to add teammates.",
                                    name
                                ));
                                self.status = format!("team: {}", name);

                                // If blueprint provided, invoke the team_create tool to apply seeding asynchronously
                                let bp = blueprint.clone();
                                if !bp.is_empty() {
                                    let session_processor = self.session_processor.clone();
                                    let event_bus = self.event_bus.clone();
                                    let storage = self.storage.clone();
                                    let lsp_manager = self.lsp_manager.clone();
                                    let working_dir_clone = working_dir.clone();
                                    let sid_clone = sid.clone();
                                    let name_clone = name.clone();
                                    // Capture the currently selected model so teammates inherit it.
                                    let active_model_clone: Option<ragent_core::agent::ModelRef> =
                                        self.selected_model.as_deref().and_then(|s| {
                                            s.split_once('/').map(|(pid, mid)| {
                                                ragent_core::agent::ModelRef {
                                                    provider_id: pid.to_string(),
                                                    model_id: mid.to_string(),
                                                }
                                            })
                                        });
                                    std::thread::spawn(move || {
                                        // Create a small runtime for seeding if there is no existing Tokio runtime
                                        let rt = match tokio::runtime::Runtime::new() {
                                            Ok(rt) => rt,
                                            Err(e) => {
                                                tracing::error!(
                                                    "Failed to create tokio runtime for team seed: {e}"
                                                );
                                                return;
                                            }
                                        };
                                        rt.block_on(async move {
                                                let registry = ragent_core::tool::create_default_registry();
                                                if let Some(tool) = registry.get("team_create") {
                                                    let input = serde_json::json!({
                                                        "name": name_clone,
                                                        "project_local": true,
                                                        "blueprint": bp,
                                                    });
                                                    let ctx = ragent_core::tool::ToolContext {
                                                        session_id: sid_clone.clone(),
                                                        working_dir: working_dir_clone.clone(),
                                                        event_bus: event_bus.clone(),
                                                        storage: Some(storage.clone()),
                                                        task_manager: None,
                                                        lsp_manager: lsp_manager.clone(),
                                                        active_model: active_model_clone,
                                                        team_context: None,
                                                        team_manager: session_processor.team_manager.get().cloned().map(|tm| tm as Arc<dyn ragent_core::tool::TeamManagerInterface>),
                                                        code_index: None,
                                                    };
                                                    let _ = tool.execute(input, &ctx).await;
                                                }
                                            });
                                    });
                                }
                            }
                            Err(e) => {
                                self.status = format!("Failed to create team: {}", e);
                                self.push_log_no_agent(
                                    LogLevel::Error,
                                    format!("team create failed: {}", e),
                                );
                            }
                        }
                    }

                    "show" => {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        if rest.is_empty() {
                            let teams = TeamStore::list_teams(&working_dir);
                            let mut output = String::from("From: /team show\n");
                            if teams.is_empty() {
                                output.push_str("No registered teams found.");
                                self.status = "team: show all (0)".to_string();
                            } else {
                                output.push_str("## Registered teams\n\n");
                                for (name, dir, is_project_local) in teams {
                                    match TeamStore::load(&dir) {
                                        Ok(store) => {
                                            let team = store.config;
                                            let scope = if is_project_local {
                                                "project"
                                            } else {
                                                "global"
                                            };
                                            output.push_str(&format!(
                                                "  ● {:<18} {:<10} lead:{} teammates:{}\n",
                                                team.name,
                                                format!("{:?}", team.status).to_lowercase(),
                                                team.lead_session_id,
                                                team.members.len()
                                            ));
                                            output.push_str(&format!(
                                                "    scope:{} path:{}\n",
                                                scope,
                                                dir.display()
                                            ));
                                        }
                                        Err(e) => {
                                            output.push_str(&format!(
                                                "  ● {} (failed to load: {})\n",
                                                name, e
                                            ));
                                        }
                                    }
                                }
                                self.status = "team: show all".to_string();
                            }
                            self.append_assistant_text(&output);
                            return;
                        }
                        match TeamStore::load_by_name(rest, &working_dir) {
                            Ok(store) => {
                                let team = store.config.clone();
                                self.ensure_team_manager_for_team(
                                    &team.name,
                                    Some(store.dir.clone()),
                                );

                                let mut output = String::from("From: /team show\n");
                                output.push_str(&format!(
                                    "## Team: {} ({})\n\n",
                                    team.name,
                                    format!("{:?}", team.status).to_lowercase()
                                ));
                                output.push_str(&format!(
                                    "  ● lead-session: {}\n",
                                    team.lead_session_id
                                ));
                                if team.members.is_empty() {
                                    output.push_str("  (no teammates yet)\n");
                                } else {
                                    for m in &team.members {
                                        let status = format!("{:?}", m.status).to_lowercase();
                                        let task =
                                            m.current_task_id.as_deref().unwrap_or("—").to_string();
                                        let sid = m.session_id.as_deref().unwrap_or("—");
                                        output.push_str(&format!(
                                            "  └ {:<18} {:<10} agent:{} session:{} task:{}\n",
                                            m.name, status, m.agent_id, sid, task
                                        ));
                                    }
                                }
                                output.push_str(&format!("\n{} teammate(s)\n", team.members.len()));
                                self.append_assistant_text(&output);
                                self.status = format!("team: show {}", team.name);
                            }
                            Err(e) => {
                                self.status = format!("Failed to load team: {e}");
                                self.push_log_no_agent(
                                    LogLevel::Error,
                                    format!("team show failed for '{}': {}", rest, e),
                                );
                            }
                        }
                    }
                    "close" => {
                        if let Some(team) = self.active_team.as_ref() {
                            let team_name = team.name.clone();
                            self.active_team = None;
                            self.team_members.clear();
                            self.team_message_counts.clear();
                            self.show_teams = false;
                            self.focused_teammate = None;
                            if self
                                .swarm_state
                                .as_ref()
                                .is_some_and(|s| s.team_name == team_name)
                            {
                                self.swarm_state = None;
                            }
                            self.push_log_no_agent(
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
                                        self.team_message_counts.clear();
                                        self.show_teams = false;
                                        self.focused_teammate = None;
                                        if self
                                            .swarm_state
                                            .as_ref()
                                            .is_some_and(|s| s.team_name == rest)
                                        {
                                            self.swarm_state = None;
                                        }
                                    }
                                    self.push_log_no_agent(
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
                                    self.push_log_no_agent(
                                        LogLevel::Error,
                                        format!("team delete failed for '{}': {}", rest, e),
                                    );
                                }
                            },
                            Err(e) => {
                                self.status = format!("Failed to load team: {e}");
                                self.push_log_no_agent(
                                    LogLevel::Error,
                                    format!("team delete failed for '{}': {}", rest, e),
                                );
                            }
                        }
                    }
                    "blueprint" | "blueprints" => {
                        let working_dir = std::env::current_dir().unwrap_or_default();

                        // Collect all blueprint directories from project-local and global paths.
                        let mut blueprint_dirs: Vec<(String, std::path::PathBuf, String)> =
                            Vec::new();
                        let mut seen_names: std::collections::HashSet<String> =
                            std::collections::HashSet::new();

                        // Walk up to find project .ragent/blueprints/teams/
                        let mut cur_opt: Option<&std::path::Path> = Some(working_dir.as_path());
                        while let Some(cur) = cur_opt {
                            let bp_root = cur.join(".ragent").join("blueprints").join("teams");
                            if bp_root.is_dir() {
                                if let Ok(entries) = std::fs::read_dir(&bp_root) {
                                    for entry in entries.flatten() {
                                        if entry.path().is_dir() {
                                            let name =
                                                entry.file_name().to_string_lossy().to_string();
                                            if seen_names.insert(name.clone()) {
                                                blueprint_dirs.push((
                                                    name,
                                                    entry.path(),
                                                    "project".to_string(),
                                                ));
                                            }
                                        }
                                    }
                                }
                                break;
                            }
                            cur_opt = cur.parent();
                        }
                        // Global blueprints
                        if let Some(home) = dirs::home_dir() {
                            let bp_root = home.join(".ragent").join("blueprints").join("teams");
                            if bp_root.is_dir() {
                                if let Ok(entries) = std::fs::read_dir(&bp_root) {
                                    for entry in entries.flatten() {
                                        if entry.path().is_dir() {
                                            let name =
                                                entry.file_name().to_string_lossy().to_string();
                                            if seen_names.insert(name.clone()) {
                                                blueprint_dirs.push((
                                                    name,
                                                    entry.path(),
                                                    "global".to_string(),
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        blueprint_dirs.sort_by(|a, b| a.0.cmp(&b.0));

                        if rest.is_empty() {
                            // List all blueprints as a markdown table.
                            let mut output = String::from(
                                "From: /team blueprint\n\n## Installed Team Blueprints\n\n",
                            );
                            if blueprint_dirs.is_empty() {
                                output.push_str("No blueprints found.\n\nInstall blueprints to:\n- `[project]/.ragent/blueprints/teams/<name>/`\n- `~/.ragent/blueprints/teams/<name>/`\n");
                            } else {
                                output.push_str(
                                    "| Blueprint | Scope | Teammates | Tasks | Description |\n",
                                );
                                output.push_str(
                                    "|-----------|-------|-----------|-------|-------------|\n",
                                );
                                for (name, path, scope) in &blueprint_dirs {
                                    // Count teammates from spawn-prompts.json
                                    let teammate_count =
                                        std::fs::read_to_string(path.join("spawn-prompts.json"))
                                            .ok()
                                            .and_then(|raw| {
                                                serde_json::from_str::<serde_json::Value>(&raw).ok()
                                            })
                                            .and_then(|v| v.as_array().map(|a| a.len()))
                                            .unwrap_or(0);
                                    // Count tasks from task-seed.json
                                    let task_count =
                                        std::fs::read_to_string(path.join("task-seed.json"))
                                            .ok()
                                            .and_then(|raw| {
                                                serde_json::from_str::<serde_json::Value>(&raw).ok()
                                            })
                                            .and_then(|v| v.as_array().map(|a| a.len()))
                                            .unwrap_or(0);
                                    // Description from first line of README.md (skip heading)
                                    let desc = std::fs::read_to_string(path.join("README.md"))
                                        .ok()
                                        .and_then(|raw| {
                                            raw.lines()
                                                .find(|l| {
                                                    !l.trim().is_empty() && !l.starts_with('#')
                                                })
                                                .map(|l| l.trim().to_string())
                                        })
                                        .unwrap_or_else(|| "-".to_string());
                                    output.push_str(&format!(
                                        "| `{}` | {} | {} | {} | {} |\n",
                                        name, scope, teammate_count, task_count, desc
                                    ));
                                }
                            }
                            self.append_assistant_text(&output);
                            self.status = "team: blueprints".to_string();
                        } else {
                            // Show detailed summary for a specific blueprint.
                            let bp_name = rest.trim();
                            let found = blueprint_dirs.iter().find(|(n, _, _)| n == bp_name);
                            if let Some((name, path, scope)) = found {
                                let mut output = format!(
                                    "From: /team blueprint {name}\n\n## Blueprint: `{name}`\n\n**Scope:** {scope}  \n**Path:** `{}`\n\n",
                                    path.display()
                                );

                                // README.md
                                if let Ok(readme) = std::fs::read_to_string(path.join("README.md"))
                                {
                                    output.push_str("### Description\n\n");
                                    output.push_str(&readme);
                                    output.push_str("\n\n");
                                }

                                // Teammates from spawn-prompts.json
                                if let Ok(raw) =
                                    std::fs::read_to_string(path.join("spawn-prompts.json"))
                                {
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw)
                                    {
                                        if let Some(items) = val.as_array() {
                                            output.push_str("### Teammates\n\n");
                                            output.push_str("| Name | Type | Prompt |\n");
                                            output.push_str("|------|------|--------|\n");
                                            for item in items {
                                                let tname = item
                                                    .get("teammate_name")
                                                    .or_else(|| {
                                                        item.get("args")
                                                            .and_then(|a| a.get("teammate_name"))
                                                    })
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("auto");
                                                let atype = item
                                                    .get("agent_type")
                                                    .or_else(|| {
                                                        item.get("args")
                                                            .and_then(|a| a.get("agent_type"))
                                                    })
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("general");
                                                let prompt = item
                                                    .get("prompt")
                                                    .or_else(|| {
                                                        item.get("args")
                                                            .and_then(|a| a.get("prompt"))
                                                    })
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("-");
                                                // Truncate long prompts for the table
                                                let prompt_short = if prompt.len() > 80 {
                                                    format!("{}…", &prompt[..77])
                                                } else {
                                                    prompt.to_string()
                                                };
                                                output.push_str(&format!(
                                                    "| `{}` | {} | {} |\n",
                                                    tname, atype, prompt_short
                                                ));
                                            }
                                            output.push('\n');
                                        }
                                    }
                                }

                                // Tasks from task-seed.json
                                if let Ok(raw) =
                                    std::fs::read_to_string(path.join("task-seed.json"))
                                {
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw)
                                    {
                                        if let Some(items) = val.as_array() {
                                            output.push_str("### Seed Tasks\n\n");
                                            output.push_str("| Title | Description |\n");
                                            output.push_str("|-------|-------------|\n");
                                            for item in items {
                                                let title = item
                                                    .get("title")
                                                    .or_else(|| {
                                                        item.get("input")
                                                            .and_then(|a| a.get("title"))
                                                    })
                                                    .or_else(|| {
                                                        item.get("args")
                                                            .and_then(|a| a.get("title"))
                                                    })
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("-");
                                                let desc = item
                                                    .get("description")
                                                    .or_else(|| {
                                                        item.get("input")
                                                            .and_then(|a| a.get("description"))
                                                    })
                                                    .or_else(|| {
                                                        item.get("args")
                                                            .and_then(|a| a.get("description"))
                                                    })
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("-");
                                                output.push_str(&format!(
                                                    "| {} | {} |\n",
                                                    title, desc
                                                ));
                                            }
                                            output.push('\n');
                                        }
                                    }
                                }

                                output.push_str(&format!("**Usage:** `/team create {name}`\n"));
                                self.append_assistant_text(&output);
                                self.status = format!("team: blueprint {name}");
                            } else {
                                self.status = format!("Blueprint '{}' not found", bp_name);
                            }
                        }
                    }
                    "message" => {
                        let (name, text) = rest
                            .split_once(char::is_whitespace)
                            .map_or((rest, ""), |(n, t)| (n.trim(), t.trim()));
                        if name.is_empty() || text.is_empty() {
                            self.status = "Usage: /team message <teammate-name> <text>".to_string();
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
                                                        self.push_log_no_agent(
                                                            LogLevel::Info,
                                                            format!("📨 lead → {name}: {text}"),
                                                        );
                                                        self.status =
                                                            format!("message sent to {name}");
                                                    }
                                                    Err(e) => {
                                                        self.status =
                                                            format!("Failed to send message: {e}");
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
                                                    let assignee =
                                                        task.assigned_to.as_deref().unwrap_or("—");
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
                            self.append_assistant_text("From: /team tasks\nNo active team.");
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
                                            self.push_log_no_agent(
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
                                            self.push_log_no_agent(
                                                LogLevel::Error,
                                                format!(
                                                    "team clear failed for '{}': {}",
                                                    team.name, e
                                                ),
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    self.status = format!("Failed to load team: {e}");
                                }
                            }
                        } else {
                            self.append_assistant_text("From: /team clear\nNo active team.");
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
                                // Build list of active teammate names
                                let active_names: Vec<String> = self
                                    .team_members
                                    .iter()
                                    .filter(|m| m.status != MemberStatus::Stopped)
                                    .map(|m| format!("{} ({})", m.name, m.agent_id))
                                    .collect();

                                // Log a warning with the list of active teammates
                                self.push_log_no_agent(
                                    LogLevel::Warn,
                                    format!(
                                        "Cannot clean up team '{}': {} teammate(s) still active: {}",
                                        team.name,
                                        active_names.len(),
                                        active_names.join(", ")
                                    ),
                                );

                                // Also show a message in the chat window listing active teammates
                                let mut msg = format!(
                                    "From: /team cleanup\nCannot clean up team '{}' because the following teammate(s) are still active:\n\n",
                                    team.name
                                );
                                for name in &active_names {
                                    msg.push_str(&format!("  - {}\n", name));
                                }
                                msg.push_str("\nYou can shut them down individually with team_shutdown_teammate, or run `/team forcecleanup` to deactivate and remove them.");
                                self.append_assistant_text(&msg);

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
                            self.team_message_counts.clear();
                            self.show_teams = false;
                            self.focused_teammate = None;
                            if self
                                .swarm_state
                                .as_ref()
                                .is_some_and(|s| s.team_name == team_name)
                            {
                                self.swarm_state = None;
                            }
                            if removed {
                                self.push_log_no_agent(
                                    LogLevel::Info,
                                    format!("🗑️  Team '{team_name}' cleaned up"),
                                );
                                self.append_assistant_text(&format!(
                                    "From: /team cleanup\nTeam '{team_name}' cleaned up."
                                ));
                            } else {
                                self.push_log_no_agent(
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
                    "forcecleanup" | "force-cleanup" => {
                        // Confirm with the user before destructive operation.
                        let confirm_msg = "Are you sure you want to force-cleanup the active team (deactivate teammates and remove on-disk resources)? Press Enter to confirm or Esc to cancel.";
                        let args_lower = args.to_lowercase();
                        let confirmed = args_lower.split_whitespace().any(|s| s == "confirm");
                        if !confirmed {
                            // Show interactive confirmation modal state with active members listed.
                            // Build list of active teammate names for display in modal.
                            let active_names: Vec<String> = self
                                .team_members
                                .iter()
                                .filter(|m| m.status != MemberStatus::Stopped)
                                .map(|m| format!("{} ({})", m.name, m.agent_id))
                                .collect();

                            let team_name = self
                                .active_team
                                .as_ref()
                                .map(|t| t.name.clone())
                                .unwrap_or_else(|| "<unknown>".to_string());

                            self.pending_forcecleanup = Some(PendingForceCleanup {
                                team_name: team_name.clone(),
                                active_members: active_names.clone(),
                            });

                            // Append assistant text instructing user to press Enter/Esc
                            let mut msg = format!("From: /team forcecleanup\n{}\n\n", confirm_msg);
                            if !active_names.is_empty() {
                                msg.push_str("Active teammates:\n\n");
                                for n in &active_names {
                                    msg.push_str(&format!("  - {}\n", n));
                                }
                                msg.push_str("\n");
                            }
                            msg.push_str("Press Enter to confirm or Esc to cancel.");

                            self.append_assistant_text(&msg);
                            self.push_log_no_agent(
                                LogLevel::Info,
                                "forcecleanup confirmation required (modal)".to_string(),
                            );
                            self.status = "forcecleanup confirmation required".to_string();
                            return;
                        }

                        // If confirmed, perform the force cleanup
                        let team_opt = self.active_team.clone();
                        if let Some(team) = team_opt {
                            let working_dir = std::env::current_dir().unwrap_or_default();
                            let team_name = team.name.clone();
                            match TeamStore::load_by_name(&team_name, &working_dir) {
                                Ok(mut store) => {
                                    // Attempt graceful shutdown of active teammate sessions first
                                    let mut deactivated: Vec<String> = Vec::new();
                                    for m in &mut store.config.members {
                                        if m.status != MemberStatus::Stopped {
                                            // Try to contact team manager to request shutdown if available
                                            if self.session_processor.team_manager.get().is_some() {
                                                // best-effort: ignore errors
                                                // Best-effort: request teammate to shutdown asynchronously.
                                                // Fire-and-forget via tokio::spawn; ignore result.
                                                let team_name_clone = store.config.name.clone();
                                                let m_name = m.name.clone();
                                                let m_agent_type = m.agent_type.clone();
                                                let working_dir_clone = store.dir.clone();
                                                let active_model: Option<
                                                    &ragent_core::agent::ModelRef,
                                                > = None;
                                                if let Some(tm_arc) =
                                                    self.session_processor.team_manager.get()
                                                {
                                                    let tm = tm_arc.clone();
                                                    tokio::spawn(async move {
                                                        let _ = tm
                                                            .spawn_teammate(
                                                                &team_name_clone,
                                                                &m_name,
                                                                &m_agent_type,
                                                                "shutdown",
                                                                active_model,
                                                                None,
                                                                &working_dir_clone,
                                                            )
                                                            .await;
                                                    });
                                                }
                                            }
                                            let desc = format!("{} ({})", m.name, m.agent_id);
                                            m.status = MemberStatus::Stopped;
                                            deactivated.push(desc);
                                        }
                                    }
                                    // Persist best-effort
                                    if let Err(e) = store.save() {
                                        self.push_log_no_agent(
                                            LogLevel::Warn,
                                            format!("Failed to persist team member status before force cleanup: {}", e),
                                        );
                                    }

                                    // Remove directory
                                    let removed = std::fs::remove_dir_all(&store.dir).is_ok();

                                    // Update TUI state
                                    self.active_team = None;
                                    self.team_members.clear();
                                    self.team_message_counts.clear();
                                    self.show_teams = false;
                                    self.focused_teammate = None;
                                    if self
                                        .swarm_state
                                        .as_ref()
                                        .is_some_and(|s| s.team_name == team_name)
                                    {
                                        self.swarm_state = None;
                                    }

                                    if !deactivated.is_empty() {
                                        self.push_log_no_agent(
                                            LogLevel::Info,
                                            format!(
                                                "Deactivated teammates: {}",
                                                deactivated.join(", ")
                                            ),
                                        );
                                    }

                                    if removed {
                                        self.push_log_no_agent(
                                            LogLevel::Info,
                                            format!("🗑️  Team '{team_name}' force cleaned up"),
                                        );
                                        let mut msg = format!(
                                            "From: /team forcecleanup\nTeam '{team_name}' force cleaned up. The following teammate(s) were deactivated and removed:\n\n"
                                        );
                                        for d in &deactivated {
                                            msg.push_str(&format!("  - {}\n", d));
                                        }
                                        self.append_assistant_text(&msg);
                                    } else {
                                        self.push_log_no_agent(
                                            LogLevel::Warn,
                                            format!(
                                                "Team '{team_name}' state cleared (dir not found)"
                                            ),
                                        );
                                        let mut msg = format!(
                                            "From: /team forcecleanup\nTeam '{team_name}' state cleared. The following teammate(s) were deactivated:\n\n"
                                        );
                                        for d in &deactivated {
                                            msg.push_str(&format!("  - {}\n", d));
                                        }
                                        self.append_assistant_text(&msg);
                                    }

                                    self.status = "team force cleaned up".to_string();
                                }
                                Err(e) => {
                                    self.status = format!("Failed to force cleanup team: {}", e);
                                    self.push_log_no_agent(
                                        LogLevel::Error,
                                        format!("forcecleanup failed for '{}': {}", team_name, e),
                                    );
                                }
                            }
                        } else {
                            self.status = "No active team to clean up".to_string();
                        }
                    }
                    "focus" => {
                        if self.active_team.is_none() {
                            self.status = "No active team".to_string();
                            return;
                        }
                        if rest.is_empty() {
                            // No arg → clear focus
                            self.focused_teammate = None;
                            self.output_view = None;
                            self.append_assistant_text("From: /team focus\nTeammate focus cleared. Input returns to lead session.");
                            self.status = "team: focus cleared".to_string();
                        } else {
                            match self.focus_teammate_by_name(rest) {
                                Ok(()) => {
                                    let name = self
                                        .focused_teammate
                                        .as_ref()
                                        .and_then(|id| {
                                            self.team_members.iter().find(|m| m.agent_id == *id)
                                        })
                                        .map(|m| m.name.clone())
                                        .unwrap_or_default();
                                    self.append_assistant_text(&format!(
                                        "From: /team focus\nFocused on **{}**. Messages will be routed to this teammate.\n\nUse `/team focus` (no args) or Alt+Up/Down to change focus.\nPress Esc to close the output view.",
                                        name
                                    ));
                                }
                                Err(e) => {
                                    self.append_assistant_text(&format!(
                                        "From: /team focus\n{e}\n\nAvailable teammates: {}",
                                        self.team_members
                                            .iter()
                                            .map(|m| m.name.as_str())
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    ));
                                    self.status = format!("team focus: {e}");
                                }
                            }
                        }
                    }
                    _ => {
                        self.status = format!(
                            "Unknown /team subcommand '{}'. Usage: /team [help|status|show|create|close|delete|message|tasks|clear|cleanup|focus]",
                            sub
                        );
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            format!("unknown /team subcommand: {}", sub),
                        );
                    }
                }
            }
            "todos" => {
                if !self.ensure_session() {
                    return;
                }
                let Some(session_id) = self.session_id.clone() else {
                    self.status = "No active session".to_string();
                    return;
                };
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

                        self.status = format!("{} todo(s)", todos.len());
                    }
                    Err(e) => {
                        self.status = format!("Failed to read todos: {}", e);
                        self.push_log_no_agent(LogLevel::Error, format!("todo_list error: {}", e));
                    }
                }
            }
            // ── /bash ────────────────────────────────────────────────────────
            "bash" => {
                let (sub, rest) = args
                    .split_once(char::is_whitespace)
                    .map_or((args, ""), |(s, r)| (s.trim(), r.trim()));

                match sub {
                    "help" | "" => {
                        let help = "\
From: /bash help

## /bash — Bash command list management

Manage the user-defined **allowlist** and **denylist** that complement the
built-in safety rules.

### Subcommands

| Command | Description |
|---------|-------------|
| `/bash add allow <cmd>` | Allow a banned command prefix (e.g. `curl`) |
| `/bash add deny <pattern>` | Block any command containing `<pattern>` |
| `/bash remove allow <cmd>` | Remove a command from the allowlist |
| `/bash remove deny <pattern>` | Remove a pattern from the denylist |
| `/bash show` | Show the current allowlist and denylist |
| `/bash help` | Show this help text |

Append `--global` to write the change to the global config
(`~/.config/ragent/ragent.json`) instead of the project `ragent.json`.

### How it works

- **allowlist**: command prefixes that bypass the built-in banned-command \
check.  Use this to re-enable tools like `curl` without entering YOLO mode.
- **denylist**: substring patterns that always reject a command, \
supplementing the built-in denied-patterns list.

Changes are persisted immediately to `ragent.json` and take effect at once.
";
                        self.append_assistant_text(help);
                    }
                    "show" => {
                        let allowlist = ragent_core::bash_lists::get_allowlist();
                        let denylist = ragent_core::bash_lists::get_denylist();

                        let mut out = String::from("From: /bash show\n\n## Bash command lists\n\n");

                        out.push_str("### Allowlist (user-defined)\n");
                        if allowlist.is_empty() {
                            out.push_str("  *(empty)*\n");
                        } else {
                            for entry in &allowlist {
                                out.push_str(&format!("  - `{entry}`\n"));
                            }
                        }

                        out.push_str("\n### Denylist (user-defined)\n");
                        if denylist.is_empty() {
                            out.push_str("  *(empty)*\n");
                        } else {
                            for entry in &denylist {
                                out.push_str(&format!("  - `{entry}`\n"));
                            }
                        }

                        out.push_str(
                            "\n*Built-in lists are not shown here. \
                            Use `/bash help` for more information.*\n",
                        );
                        self.append_assistant_text(&out);
                    }
                    "add" | "remove" => {
                        // Parse: [allow|deny] <entry> [--global]
                        let (list_type, entry_with_flag) = rest
                            .split_once(char::is_whitespace)
                            .map_or((rest, ""), |(l, e)| (l.trim(), e.trim()));

                        let is_global = entry_with_flag.ends_with("--global");
                        let entry = if is_global {
                            entry_with_flag.trim_end_matches("--global").trim()
                        } else {
                            entry_with_flag
                        };

                        if entry.is_empty() {
                            self.append_assistant_text(&format!(
                                "From: /bash {sub}\n\nUsage: `/bash {sub} allow|deny <entry> [--global]`"
                            ));

                            return;
                        }

                        let scope = if is_global {
                            ragent_core::bash_lists::Scope::Global
                        } else {
                            ragent_core::bash_lists::Scope::Project
                        };
                        let scope_label = if is_global { "global" } else { "project" };
                        let config_file = if is_global {
                            "~/.config/ragent/ragent.json"
                        } else {
                            "ragent.json"
                        };

                        match (sub, list_type) {
                            ("add", "allow") => {
                                match ragent_core::bash_lists::add_allowlist(entry, scope) {
                                    Ok(()) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash add allow\n\n\
                                            ✅ Added `{entry}` to the **allowlist** \
                                            ({scope_label}: `{config_file}`).\n\n\
                                            Commands starting with `{entry}` will no longer \
                                            be blocked by the banned-command check."
                                        ));
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash add allow\n\n❌ Error: {e}"
                                        ));
                                    }
                                }
                            }
                            ("add", "deny") => {
                                match ragent_core::bash_lists::add_denylist(entry, scope) {
                                    Ok(()) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash add deny\n\n\
                                            ✅ Added `{entry}` to the **denylist** \
                                            ({scope_label}: `{config_file}`).\n\n\
                                            Any command containing `{entry}` will be rejected."
                                        ));
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash add deny\n\n❌ Error: {e}"
                                        ));
                                    }
                                }
                            }
                            ("remove", "allow") => {
                                match ragent_core::bash_lists::remove_allowlist(entry, scope) {
                                    Ok(true) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash remove allow\n\n\
                                            ✅ Removed `{entry}` from the **allowlist** \
                                            ({scope_label}: `{config_file}`)."
                                        ));
                                    }
                                    Ok(false) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash remove allow\n\n\
                                            ⚠️ `{entry}` was not in the {scope_label} allowlist."
                                        ));
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash remove allow\n\n❌ Error: {e}"
                                        ));
                                    }
                                }
                            }
                            ("remove", "deny") => {
                                match ragent_core::bash_lists::remove_denylist(entry, scope) {
                                    Ok(true) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash remove deny\n\n\
                                            ✅ Removed `{entry}` from the **denylist** \
                                            ({scope_label}: `{config_file}`)."
                                        ));
                                    }
                                    Ok(false) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash remove deny\n\n\
                                            ⚠️ `{entry}` was not in the {scope_label} denylist."
                                        ));
                                    }
                                    Err(e) => {
                                        self.append_assistant_text(&format!(
                                            "From: /bash remove deny\n\n❌ Error: {e}"
                                        ));
                                    }
                                }
                            }
                            _ => {
                                self.append_assistant_text(&format!(
                                    "From: /bash {sub}\n\n\
                                    Unknown list type `{list_type}`. Use `allow` or `deny`.\n\n\
                                    Usage: `/bash {sub} allow|deny <entry> [--global]`"
                                ));
                            }
                        }
                    }
                    _ => {
                        self.append_assistant_text(&format!(
                            "From: /bash\n\nUnknown subcommand `{sub}`. \
                            Run `/bash help` for usage."
                        ));
                    }
                }
            }
            "yolo" => {
                let new_state = ragent_core::yolo::toggle();
                let label = if new_state {
                    "ENABLED ⚠️"
                } else {
                    "disabled"
                };
                let mut output = format!("From: /yolo\n## YOLO mode {label}\n\n");
                if new_state {
                    output.push_str(
                        "All command validation is now **bypassed**:\n\
                         - Bash denied-pattern checks — **off**\n\
                         - Dynamic context allowlist — **off**\n\
                         - MCP config validation — **off**\n\
                         - Obfuscation detection — **off**\n\n\
                         ⚠️  The agent can now execute **any** command without restriction.\n\
                         Use `/yolo` again to re-enable safety checks.\n",
                    );
                } else {
                    output.push_str("All safety checks have been **re-enabled**.\n");
                }
                self.append_assistant_text(&output);

                self.status = format!("YOLO mode {label}");
                self.push_log_no_agent(
                    if new_state {
                        LogLevel::Warn
                    } else {
                        LogLevel::Info
                    },
                    format!("YOLO mode {label}"),
                );
            }
            // ── /swarm ──────────────────────────────────────────────────────
            "swarm" => {
                let (sub, rest) = args
                    .split_once(char::is_whitespace)
                    .map_or((args, ""), |(s, r)| (s.trim(), r.trim()));

                match sub {
                    "help" => {
                        let help = "\
From: /swarm help\n\
## Swarm — Fleet-Style Auto-Decomposition\n\n\
| Command | Description |\n\
|---------|-------------|\n\
| `/swarm <prompt>` | Decompose a goal into parallel subtasks and spawn a team |\n\
| `/swarm status` | Show live progress of the active swarm |\n\
| `/swarm cancel` | Cancel the active swarm and clean up |\n\
| `/swarm help` | Show this help |\n\n\
The swarm analyses your prompt, breaks it into independent subtasks with dependency \
edges, creates an ephemeral team, and orchestrates parallel execution.\n";
                        self.append_assistant_text(help);
                    }
                    "status" => {
                        self.handle_swarm_status();
                    }
                    "cancel" => {
                        self.handle_swarm_cancel();
                    }
                    _ => {
                        // /swarm <prompt> — decompose and execute
                        let full_prompt = if sub.is_empty() {
                            String::new()
                        } else if rest.is_empty() {
                            sub.to_string()
                        } else {
                            format!("{} {}", sub, rest)
                        };

                        if full_prompt.is_empty() {
                            let help = "From: /swarm\n\n\
Usage: `/swarm <prompt>` — describe what you want the swarm to accomplish.\n\
Type `/swarm help` for more info.\n";
                            self.append_assistant_text(help);

                            return;
                        }

                        if self.swarm_state.is_some() {
                            self.status =
                                "⚠ A swarm is already active — use /swarm cancel first".to_string();
                            return;
                        }

                        // Require provider + model
                        let (provider_id, model_id) = match self
                            .selected_model
                            .as_deref()
                            .and_then(|s| s.split_once('/'))
                            .map(|(p, m)| (p.to_string(), m.to_string()))
                        {
                            Some(pair) => pair,
                            None => {
                                self.status =
                                    "⚠ /swarm requires a configured model — use /model".to_string();
                                return;
                            }
                        };

                        self.status = "⏳ swarm: decomposing goal…".to_string();
                        self.push_log_no_agent(
                            LogLevel::Info,
                            format!(
                                "Swarm: decomposing — {}",
                                &full_prompt[..full_prompt.len().min(80)]
                            ),
                        );

                        // Show user message in chat
                        self.append_assistant_text(&format!(
                            "From: /swarm\n## 🐝 Swarm Decomposition\n\n\
                            Analysing your goal and breaking it into parallel subtasks…\n\n\
                            > {}\n",
                            full_prompt
                        ));

                        // Store prompt for later use when decomposition returns
                        {
                            // We'll create the swarm state after decomposition returns
                            // For now just store the prompt in a temporary field
                        }

                        // Spawn async LLM call for decomposition
                        let registry = Arc::clone(&self.provider_registry);
                        let storage_clone = Arc::clone(&self.storage);
                        let swarm_result = Arc::clone(&self.swarm_result);

                        tokio::spawn(async move {
                            let completer = RagentCompleter {
                                registry,
                                storage: storage_clone,
                                provider_id,
                                model_id,
                            };

                            let system = ragent_core::team::DECOMPOSITION_SYSTEM_PROMPT;
                            let user =
                                ragent_core::team::build_decomposition_user_prompt(&full_prompt);

                            let outcome = match completer.complete(system, &user).await {
                                Ok(text) => Ok(text),
                                Err(e) => Err(e.to_string()),
                            };

                            if let Ok(mut guard) = swarm_result.lock() {
                                *guard = Some(outcome);
                            }
                        });
                    }
                }
            }

            // ── /autopilot ──────────────────────────────────────────────────
            "autopilot" => {
                let sub = args.split_whitespace().next().unwrap_or("").to_lowercase();
                match sub.as_str() {
                    "on" => {
                        // Parse optional flags: --max-tokens N  --max-time N
                        let mut token_budget: Option<u64> = None;
                        let mut time_secs: Option<u64> = None;
                        let parts: Vec<&str> = args.split_whitespace().collect();
                        let mut i = 1; // skip "on"
                        while i < parts.len() {
                            match parts[i] {
                                "--max-tokens" if i + 1 < parts.len() => {
                                    token_budget = parts[i + 1].parse().ok();
                                    i += 2;
                                }
                                "--max-time" if i + 1 < parts.len() => {
                                    time_secs = parts[i + 1].parse().ok();
                                    i += 2;
                                }
                                _ => {
                                    i += 1;
                                }
                            }
                        }
                        self.autopilot_enabled = true;
                        self.autopilot_token_budget = token_budget;
                        self.autopilot_time_limit_secs = time_secs;
                        self.autopilot_started_at = Some(std::time::Instant::now());
                        let mut msg =
                            "⚡ **Autopilot ON** — agent will run autonomously.".to_string();
                        if let Some(t) = token_budget {
                            msg.push_str(&format!(" Token budget: {t}."));
                        }
                        if let Some(s) = time_secs {
                            msg.push_str(&format!(" Time limit: {s}s."));
                        }
                        msg.push_str("\nCall `task_complete` to signal completion, or `/autopilot off` to stop.");
                        self.append_assistant_text(&format!("From: /autopilot\n{msg}"));
                        self.status = "⚡ autopilot".to_string();
                        self.push_log_no_agent(LogLevel::Info, "autopilot enabled".to_string());
                    }
                    "off" => {
                        self.autopilot_enabled = false;
                        self.autopilot_token_budget = None;
                        self.autopilot_time_limit_secs = None;
                        self.autopilot_started_at = None;
                        self.autopilot_pending_continue = None;
                        self.append_assistant_text("From: /autopilot\n⚡ **Autopilot OFF** — returning to interactive mode.");
                        self.status = "ready".to_string();
                        self.push_log_no_agent(LogLevel::Info, "autopilot disabled".to_string());
                    }
                    "status" => {
                        let state = if self.autopilot_enabled {
                            let elapsed = self
                                .autopilot_started_at
                                .map(|s| s.elapsed().as_secs())
                                .unwrap_or(0);
                            format!("⚡ Autopilot: **ON** (running for {}s)", elapsed)
                        } else {
                            "⚡ Autopilot: **OFF**".to_string()
                        };
                        self.append_assistant_text(&format!("From: /autopilot status\n{state}"));
                    }
                    _ => {
                        self.append_assistant_text(
                            "From: /autopilot\n\
                             Usage: `/autopilot on [--max-tokens N] [--max-time N]` | `off` | `status`"
                        );
                    }
                }
            }

            // ── /plan ────────────────────────────────────────────────────────
            "plan" => {
                if args.is_empty() {
                    self.append_assistant_text(
                        "From: /plan\n\
                         Usage: `/plan <task description>`\n\n\
                         The plan agent will analyse the codebase and produce a plan for your task. \
                         You will be asked to approve or reject the plan before implementation begins."
                    );
                } else {
                    let sid = self.session_id.clone().unwrap_or_default();
                    self.execute_plan_delegation(&sid, args.to_string(), String::new());
                }
            }

            // ── /mode ────────────────────────────────────────────────────────
            "mode" => {
                let sub = args.trim().to_lowercase();
                if sub.is_empty() || sub == "status" {
                    let current = self
                        .role_mode
                        .as_ref()
                        .map(|m| format!("{} {}", m.icon(), m.label()))
                        .unwrap_or_else(|| "normal (no role mode active)".to_string());
                    self.append_assistant_text(&format!(
                        "From: /mode\nCurrent mode: **{current}**\n\n\
                         Available modes: `architect` `coder` `reviewer` `debugger` `tester`\n\
                         Use `/mode off` to return to normal mode."
                    ));
                } else if sub == "off" || sub == "normal" {
                    self.role_mode = None;
                    self.status = "mode: normal".to_string();
                    self.append_assistant_text(
                        "From: /mode\n✅ Role mode cleared — back to normal mode.",
                    );
                    self.push_log_no_agent(LogLevel::Info, "role mode cleared".to_string());
                } else if let Some(mode) = RoleMode::from_str(&sub) {
                    let label = mode.label().to_string();
                    let icon = mode.icon().to_string();
                    self.role_mode = Some(mode);
                    self.status = format!("{icon} mode: {label}");
                    self.append_assistant_text(&format!(
                        "From: /mode\n{icon} **{label} mode** activated.\n\
                         The agent will now focus on {} tasks.",
                        label
                    ));
                    self.push_log_no_agent(LogLevel::Info, format!("role mode: {label}"));
                } else {
                    self.append_assistant_text(&format!(
                        "From: /mode\nUnknown mode '{}'. \
                         Available: `architect` `coder` `reviewer` `debugger` `tester` `off`",
                        sub
                    ));
                }
            }

            "memory" => {
                let project_mem = std::env::current_dir()
                    .unwrap_or_default()
                    .join(".ragent")
                    .join("memory")
                    .join("MEMORY.md");
                let project_analysis = std::env::current_dir()
                    .unwrap_or_default()
                    .join(".ragent")
                    .join("memory")
                    .join("PROJECT_ANALYSIS.md");
                let user_mem =
                    dirs::home_dir().map(|h| h.join(".ragent").join("memory").join("MEMORY.md"));

                match args.trim() {
                    "show" | "" => {
                        let mut output = String::from("From: /memory show\n\n");

                        let proj_content = std::fs::read_to_string(&project_mem)
                            .unwrap_or_else(|_| "(no project memory)".to_string());
                        output.push_str(&format!(
                            "## Project Memory ({})\n{}\n\n",
                            project_mem.display(),
                            proj_content
                        ));

                        if project_analysis.exists() {
                            let analysis =
                                std::fs::read_to_string(&project_analysis).unwrap_or_default();
                            output.push_str(&format!("## Project Analysis\n{}\n\n", analysis));
                        }

                        if let Some(path) = user_mem {
                            let user_content = std::fs::read_to_string(&path)
                                .unwrap_or_else(|_| "(no user memory)".to_string());
                            output.push_str(&format!(
                                "## User Memory ({})\n{}\n\n",
                                path.display(),
                                user_content
                            ));
                        }

                        self.append_assistant_text(&output);
                    }
                    sub if sub.starts_with("clear") => {
                        let scope = sub.strip_prefix("clear").unwrap_or("").trim();
                        let path = match scope {
                            "user" => dirs::home_dir()
                                .map(|h| h.join(".ragent").join("memory").join("MEMORY.md")),
                            _ => Some(
                                std::env::current_dir()
                                    .unwrap_or_default()
                                    .join(".ragent")
                                    .join("memory")
                                    .join("MEMORY.md"),
                            ),
                        };
                        if let Some(p) = path {
                            if p.exists() {
                                let _ = std::fs::remove_file(&p);
                                self.append_assistant_text(&format!(
                                    "From: /memory clear\nMemory cleared: {}",
                                    p.display()
                                ));
                            } else {
                                self.append_assistant_text(
                                    "From: /memory clear\nNo memory file found.",
                                );
                            }
                        }
                    }
                    _ => {
                        self.append_assistant_text(
                            "From: /memory\nUsage: `/memory show` | `/memory clear [project|user]`",
                        );
                    }
                }
            }

            "github" => match args.trim() {
                "login" => {
                    self.append_assistant_text(
                            "From: /github login\n🔐 Starting GitHub OAuth device flow…\n\nPlease wait for the authorization URL…",
                        );
                    let event_bus = self.event_bus.clone();
                    let sid = self.session_id.clone().unwrap_or_default();
                    tokio::spawn(async move {
                        let client_id = ragent_core::github::GitHubClient::client_id();
                        let result = ragent_core::github::auth::device_flow_login(
                                &client_id,
                                |user_code, verification_uri| {
                                    event_bus.publish(ragent_core::event::Event::AgentError {
                                        session_id: sid.clone(),
                                        error: format!(
                                            "GitHub Login — visit: {verification_uri}\nEnter code: {user_code}\n\nWaiting for authorization…"
                                        ),
                                    });
                                },
                            )
                            .await;

                        match result {
                            Ok(token) => match ragent_core::github::auth::save_token(&token) {
                                Ok(_) => {
                                    event_bus.publish(
                                                ragent_core::event::Event::AgentError {
                                                    session_id: sid,
                                                    error: "✅ GitHub authentication successful! Token saved to ~/.ragent/github_token.".to_string(),
                                                },
                                            );
                                }
                                Err(e) => {
                                    event_bus.publish(ragent_core::event::Event::AgentError {
                                        session_id: sid,
                                        error: format!("Failed to save GitHub token: {e}"),
                                    });
                                }
                            },
                            Err(e) => {
                                event_bus.publish(ragent_core::event::Event::AgentError {
                                    session_id: sid,
                                    error: format!("GitHub login failed: {e}"),
                                });
                            }
                        }
                    });
                }
                "logout" => match ragent_core::github::auth::delete_token() {
                    Ok(_) => self.append_assistant_text("From: /github\n✅ GitHub token removed."),
                    Err(e) => self.append_assistant_text(&format!(
                        "From: /github\n❌ Failed to remove token: {e}"
                    )),
                },
                "status" | "" => match ragent_core::github::auth::load_token() {
                    Some(_) => {
                        self.append_assistant_text(
                                    "From: /github\n✅ GitHub token configured. (GITHUB_TOKEN env or ~/.ragent/github_token)",
                                );
                    }
                    None => {
                        self.append_assistant_text(
                                    "From: /github\n❌ No GitHub token configured.\n\nRun `/github login` to authenticate via OAuth device flow.",
                                );
                    }
                },
                _ => {
                    self.append_assistant_text(
                            "From: /github\nUsage: `/github login` | `/github logout` | `/github status`",
                        );
                }
            },

            "update" => match args.trim() {
                "install" => {
                    self.append_assistant_text(
                        "From: /update install\n⬇️ Downloading latest release…",
                    );
                    let event_bus = self.event_bus.clone();
                    let sid = self.session_id.clone().unwrap_or_default();
                    tokio::spawn(async move {
                        match ragent_core::updater::check_for_update().await {
                            Some(info) => match info.download_url {
                                Some(ref url) => {
                                    match ragent_core::updater::download_and_replace(url).await {
                                        Ok(()) => {
                                            event_bus.publish(
                                                    ragent_core::event::Event::AgentError {
                                                        session_id: sid,
                                                        error: format!(
                                                            "✅ Updated to v{}! Please restart ragent to use the new version.",
                                                            info.version
                                                        ),
                                                    },
                                                );
                                        }
                                        Err(e) => {
                                            event_bus.publish(
                                                ragent_core::event::Event::AgentError {
                                                    session_id: sid,
                                                    error: format!("❌ Install failed: {e}"),
                                                },
                                            );
                                        }
                                    }
                                }
                                None => {
                                    event_bus.publish(ragent_core::event::Event::AgentError {
                                            session_id: sid,
                                            error: format!(
                                                "⚠️  Update v{} found but no binary available for this platform.\n\nVisit https://github.com/thawkins/ragent/releases to download manually.",
                                                info.version
                                            ),
                                        });
                                }
                            },
                            None => {
                                event_bus.publish(ragent_core::event::Event::AgentError {
                                    session_id: sid,
                                    error: format!(
                                        "✅ Already up to date (v{}).",
                                        ragent_core::updater::CURRENT_VERSION
                                    ),
                                });
                            }
                        }
                    });
                }
                _ => {
                    self.append_assistant_text("From: /update\n🔍 Checking for updates…");
                    let event_bus = self.event_bus.clone();
                    let sid = self.session_id.clone().unwrap_or_default();
                    tokio::spawn(async move {
                        match ragent_core::updater::check_for_update().await {
                            Some(info) => {
                                let notes = if info.body.is_empty() {
                                    "No release notes.".to_string()
                                } else {
                                    info.body.chars().take(500).collect::<String>()
                                };
                                event_bus.publish(ragent_core::event::Event::AgentError {
                                        session_id: sid,
                                        error: format!(
                                            "🆕 Update available: **v{}**\n\n{}\n\nRun `/update install` to install.",
                                            info.version, notes
                                        ),
                                    });
                            }
                            None => {
                                event_bus.publish(ragent_core::event::Event::AgentError {
                                    session_id: sid,
                                    error: format!(
                                        "✅ ragent is up to date (v{}).",
                                        ragent_core::updater::CURRENT_VERSION
                                    ),
                                });
                            }
                        }
                    });
                }
            },

            "doctor" => {
                self.append_assistant_text("From: /doctor\n🩺 Running diagnostics…");
                let event_bus = self.event_bus.clone();
                let sid = self.session_id.clone().unwrap_or_default();
                let working_dir = std::env::current_dir().unwrap_or_default();
                tokio::spawn(async move {
                    let mut lines = vec!["From: /doctor\n# Diagnostic Report\n".to_string()];

                    // Check git
                    let git_ok = std::process::Command::new("git")
                        .args(["--version"])
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);
                    lines.push(format!("{} git", if git_ok { "✅" } else { "❌" }));

                    // Check ripgrep
                    let rg_ok = std::process::Command::new("rg")
                        .arg("--version")
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);
                    lines.push(format!(
                        "{} ripgrep (rg)",
                        if rg_ok {
                            "✅"
                        } else {
                            "❌ ripgrep not found — install at https://github.com/BurntSushi/ripgrep"
                        }
                    ));

                    // Check GitHub token
                    let gh_ok = ragent_core::github::auth::load_token().is_some();
                    lines.push(format!(
                        "{} GitHub token",
                        if gh_ok {
                            "✅"
                        } else {
                            "⚠️  no GitHub token — run /github login"
                        }
                    ));

                    // Check memory dirs
                    let memory_dir_ok = if let Some(home) = dirs::home_dir() {
                        let p = home.join(".ragent").join("memory");
                        std::fs::create_dir_all(&p).is_ok()
                    } else {
                        false
                    };
                    lines.push(format!(
                        "{} memory directory (~/.ragent/memory/)",
                        if memory_dir_ok { "✅" } else { "❌" }
                    ));

                    // Check project .ragent dir
                    let project_ragent_ok =
                        std::fs::create_dir_all(working_dir.join(".ragent")).is_ok();
                    lines.push(format!(
                        "{} project .ragent/ directory",
                        if project_ragent_ok { "✅" } else { "❌" }
                    ));

                    // Check MCP config (field is `mcp`)
                    let mcp_configured = ragent_core::config::Config::load()
                        .map(|c| !c.mcp.is_empty())
                        .unwrap_or(false);
                    lines.push(format!(
                        "{} MCP servers configured",
                        if mcp_configured {
                            "✅"
                        } else {
                            "ℹ️  no MCP servers configured (optional)"
                        }
                    ));

                    // Check for update
                    lines.push("\n**Checking for updates…**".to_string());
                    let update_msg = match ragent_core::updater::check_for_update().await {
                        Some(info) => format!("⚠️  Update available: v{}", info.version),
                        None => {
                            format!("✅ Up to date (v{})", ragent_core::updater::CURRENT_VERSION)
                        }
                    };
                    lines.push(update_msg);

                    lines.push("\n*Diagnostics complete.*".to_string());

                    event_bus.publish(ragent_core::event::Event::AgentError {
                        session_id: sid,
                        error: lines.join("\n"),
                    });
                });
            }

            "webapi" => match args.trim() {
                "enable" | "start" => {
                    if self.webapi_server.is_some() {
                        let addr = self.webapi_addr.clone();
                        self.append_assistant_text(&format!(
                                "⚠️ Web API is already running at http://{addr}\n\nRun `/webapi disable` to stop it."
                            ));
                    } else {
                        use rand::Rng;
                        use rand::distr::Alphanumeric;
                        let token: String = rand::rng()
                            .sample_iter(&Alphanumeric)
                            .take(40)
                            .map(char::from)
                            .collect();
                        self.webapi_token = Some(token.clone());
                        let addr = self.webapi_addr.clone();

                        let config = ragent_core::config::Config::load().unwrap_or_default();
                        let app_state = ragent_server::routes::AppState {
                            event_bus: self.event_bus.clone(),
                            config: std::sync::Arc::new(tokio::sync::RwLock::new(config)),
                            storage: self.storage.clone(),
                            session_processor: self.session_processor.clone(),
                            auth_token: token.clone(),
                            rate_limiter: std::sync::Arc::new(tokio::sync::Mutex::new(
                                std::collections::HashMap::new(),
                            )),
                            coordinator: None,
                        };

                        let addr_clone = addr.clone();
                        let handle = tokio::spawn(async move {
                            if let Err(e) =
                                ragent_server::routes::start_server(&addr_clone, app_state).await
                            {
                                tracing::error!("Web API server error: {e}");
                            }
                        });
                        self.webapi_server = Some(handle);

                        self.append_assistant_text(&format!(
                            "✅ **Web API enabled** at `http://{addr}`\n\n\
                                **Bearer Token:**\n```\n{token}\n```\n\
                                Include this token in all API requests (except `/health`):\n\
                                ```\nAuthorization: Bearer {token}\n```\n\n\
                                Run `/webapi help` to see all endpoints."
                        ));
                    }
                }
                "disable" | "stop" => {
                    if let Some(handle) = self.webapi_server.take() {
                        handle.abort();
                        self.webapi_token = None;
                        self.append_assistant_text("🛑 **Web API disabled.**");
                    } else {
                        self.append_assistant_text(
                            "ℹ️ Web API is not running. Use `/webapi enable` to start it.",
                        );
                    }
                }
                "help" | "status" | "" => {
                    let base = format!("http://{}", self.webapi_addr);
                    let status = if self.webapi_server.is_some() {
                        format!("🟢 **Running** — {base}")
                    } else {
                        "🔴 **Disabled** — run `/webapi enable` to start".to_string()
                    };
                    let auth_note = if let Some(ref tok) = self.webapi_token {
                        format!(
                            "\n**Bearer Token:** `{tok}`\n\
                                Add `Authorization: Bearer {tok}` to all requests (except `/health`)."
                        )
                    } else {
                        "\n*No token set — start the server with `/webapi enable`.*".to_string()
                    };
                    self.append_assistant_text(&format!(
                            "## 🌐 Web API\n\n\
                            **Status:** {status}{auth_note}\n\n\
                            ### Endpoints\n\n\
                            | Method | Path | Description |\n\
                            |--------|------|-------------|\n\
                            | `GET` | [{base}/health]({base}/health) | Health check — no auth required |\n\
                            | `GET` | [{base}/config]({base}/config) | Get application configuration |\n\
                            | `GET` | [{base}/providers]({base}/providers) | List available LLM providers |\n\
                            | `GET` | [{base}/sessions]({base}/sessions) | List all sessions |\n\
                            | `POST` | [{base}/sessions]({base}/sessions) | Create session · body: `{{\"directory\": \"/path\"}}` |\n\
                            | `GET` | [{base}/sessions/{{id}}]({base}/sessions) | Get session details |\n\
                            | `DELETE` | [{base}/sessions/{{id}}]({base}/sessions) | Archive a session |\n\
                            | `GET` | [{base}/sessions/{{id}}/messages]({base}/sessions) | List session messages |\n\
                            | `POST` | [{base}/sessions/{{id}}/messages]({base}/sessions) | Send message · body: `{{\"content\": \"...\", \"attachments\": []}}` |\n\
                            | `POST` | [{base}/sessions/{{id}}/abort]({base}/sessions) | Abort current operation |\n\
                            | `POST` | [{base}/sessions/{{id}}/permission/{{req_id}}]({base}/sessions) | Reply to permission · body: `{{\"allow\": true}}` |\n\
                            | `GET` | [{base}/sessions/{{id}}/tasks]({base}/sessions) | List background tasks |\n\
                            | `POST` | [{base}/sessions/{{id}}/tasks]({base}/sessions) | Spawn a background task |\n\
                            | `GET` | [{base}/sessions/{{id}}/tasks/{{tid}}]({base}/sessions) | Get task status |\n\
                            | `DELETE` | [{base}/sessions/{{id}}/tasks/{{tid}}]({base}/sessions) | Cancel a task |\n\
                            | `GET` | [{base}/events]({base}/events) | SSE stream for real-time events |\n\
                            | `POST` | [{base}/opt]({base}/opt) | Optimise a prompt |\n\
                            | `GET` | [{base}/orchestrator/metrics]({base}/orchestrator/metrics) | Orchestration metrics |\n\
                            | `POST` | [{base}/orchestrator/start]({base}/orchestrator/start) | Start orchestration job |\n\
                            | `GET` | [{base}/orchestrator/jobs/{{id}}]({base}/orchestrator/jobs) | Get job status |\n\n\
                            ### Quick start\n\
                            ```bash\n\
                            # Health check\n\
                            curl {base}/health\n\n\
                            # List sessions (replace TOKEN)\n\
                            curl -H 'Authorization: Bearer TOKEN' {base}/sessions\n\n\
                            # Send a message\n\
                            curl -X POST -H 'Authorization: Bearer TOKEN' \\\n\
                              -H 'Content-Type: application/json' \\\n\
                              -d '{{\"content\": \"Hello!\"}}' \\\n\
                              {base}/sessions/SESSION_ID/messages\n\
                            ```"
                        ));
                }
                _ => {
                    self.append_assistant_text(
                        "Usage: `/webapi enable` · `/webapi disable` · `/webapi help`",
                    );
                }
            },

            "codeindex" => {
                let sub = args.split_whitespace().next().unwrap_or("");
                match sub {
                    "on" | "enable" => {
                        self.append_assistant_text(
                            "ℹ️ **Code index:** enabling is handled by the session processor at startup. \
                             The code index will be active on the next session if the project root contains source files.",
                        );
                        self.status = "codeindex: on".to_string();
                    }
                    "off" | "disable" => {
                        self.append_assistant_text(
                            "ℹ️ **Code index:** disabling removes the index from the tool context. \
                             Tools will return \"not available\" and suggest fallback tools.",
                        );
                        self.status = "codeindex: off".to_string();
                    }
                    "show" | "status" | "" => {
                        // Check if we have an active code index with real stats
                        if let Some(ref idx) = self.code_index {
                            match idx.status() {
                                Ok(stats) => {
                                    let mut output = String::from("## Code Index Status\n\n");
                                    output.push_str(&format!("**Files indexed:** {}\n", stats.files_indexed));
                                    output.push_str(&format!("**Total symbols:** {}\n", stats.total_symbols));
                                    output.push_str(&format!(
                                        "**Total size:** {:.1} KB\n",
                                        stats.total_bytes as f64 / 1024.0
                                    ));

                                    if !stats.languages.is_empty() {
                                        output.push_str("**Languages:** ");
                                        let langs: Vec<String> = stats
                                            .languages
                                            .iter()
                                            .map(|(lang, count)| format!("{lang} ({count})"))
                                            .collect();
                                        output.push_str(&langs.join(", "));
                                        output.push('\n');
                                    }

                                    if let Some(ts) = &stats.last_full_index {
                                        output.push_str(&format!("**Last full index:** {ts}\n"));
                                    }
                                    if let Some(ts) = &stats.last_incremental_update {
                                        output.push_str(&format!("**Last incremental:** {ts}\n"));
                                    }
                                    output.push_str(&format!(
                                        "**Index size:** {:.1} KB\n",
                                        stats.index_size_bytes as f64 / 1024.0
                                    ));
                                    self.append_assistant_text(&output);
                                    self.status = format!(
                                        "codeindex: {} files, {} symbols",
                                        stats.files_indexed, stats.total_symbols
                                    );
                                }
                                Err(e) => {
                                    self.append_assistant_text(&format!(
                                        "## Code Index Status\n\n⚠️ Error reading index stats: {e}"
                                    ));
                                    self.status = "codeindex: error".to_string();
                                }
                            }
                        } else {
                            // No active code index
                            self.append_assistant_text(
                                "## Code Index Status\n\n\
                                 Code index is not available. It may be disabled or not yet initialised.\n\n\
                                 Use `/codeindex on` to enable indexing, \
                                 or run `/codeindex help` for available sub-commands.",
                            );
                            self.status = "codeindex: not available".to_string();
                        }
                    }
                    "reindex" => {
                        self.append_assistant_text(
                            "ℹ️ **Code index:** to trigger a full re-index, use the `codeindex_reindex` tool from the agent.",
                        );
                        self.status = "codeindex: reindex".to_string();
                    }
                    "help" => {
                        self.append_assistant_text(
                            "## /codeindex — Codebase Index Management\n\n\
                             | Sub-command | Description |\n\
                             |-------------|-------------|\n\
                             | `/codeindex on` | Enable codebase indexing |\n\
                             | `/codeindex off` | Disable codebase indexing |\n\
                             | `/codeindex show` | Show index status and statistics |\n\
                             | `/codeindex reindex` | Trigger a full re-index |\n\
                             | `/codeindex help` | Show this help |\n\n\
                             When enabled, the agent has access to these tools:\n\
                             - `codeindex_search` — Full-text search for symbols and docs\n\
                             - `codeindex_symbols` — Structured symbol query\n\
                             - `codeindex_references` — Find all references to a symbol\n\
                             - `codeindex_dependencies` — File dependency graph\n\
                             - `codeindex_status` — Index statistics\n\
                             - `codeindex_reindex` — Trigger full re-index",
                        );
                        self.status = "codeindex: help".to_string();
                    }
                    _ => {
                        self.append_assistant_text(
                            "Usage: `/codeindex on|off|show|reindex|help`",
                        );
                    }
                }
            }

            _ => {
                let working_dir = std::env::current_dir().unwrap_or_default();
                let skill_dirs = ragent_core::config::Config::load()
                    .map(|c| c.skill_dirs)
                    .unwrap_or_default();
                let registry = ragent_core::skill::SkillRegistry::load(&working_dir, &skill_dirs);
                if let Some(skill) = registry.get(cmd) {
                    if !skill.user_invocable {
                        self.status = format!("Skill '{}' is not user-invocable", cmd);
                        self.push_log_no_agent(
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
                                self.session_id = Some(session.id.clone());
                                // Map the primary session's short_sid to the current agent name
                                let short_sid = short_session_id(&session.id);
                                self.sid_to_display_name
                                    .insert(short_sid, self.agent_name.clone());
                            }
                            Err(e) => {
                                self.status = format!("error: {}", e);
                                return;
                            }
                        }
                    }

                    let sid = self.session_id.clone().unwrap_or_default();
                    let skill = skill.clone();
                    let args_owned = args.to_string();
                    let processor = self.session_processor.clone();

                    let mut agent = self.agent_info.clone();
                    // Skill model takes highest priority, then agent-defined
                    // model, then global selected_model as fallback.
                    let override_model = skill.model.as_ref().or_else(|| {
                        if agent.model.is_none() {
                            self.selected_model.as_ref()
                        } else {
                            None
                        }
                    });
                    if let Some(ref model_str) = override_model {
                        if let Some((provider, model)) = model_str.split_once('/') {
                            agent.model = Some(ModelRef {
                                provider_id: provider.to_string(),
                                model_id: model.to_string(),
                            });
                        }
                    }

                    self.status = format!("invoking skill /{}…", cmd);
                    self.push_log_no_agent(
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

                    let flag = Arc::new(AtomicBool::new(false));
                    self.cancel_flag = Some(flag.clone());
                    let working_dir = std::env::current_dir().unwrap_or_default();

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
                    self.push_log_no_agent(LogLevel::Warn, format!("Unknown command: /{}", cmd));
                }
            }
        }
        self.assert_ui_invariants();
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
                self.set_cursor_char_index_clamped(self.input_len_chars());
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
        let before_input = self.input.clone();
        let before_cursor = self.input_cursor;
        // If context menu is open, intercept clicks.
        if self.context_menu.is_some() {
            if let MouseEventKind::Down(MouseButton::Left) = event.kind {
                self.handle_context_menu_click(event.column, event.row);
            } else if let MouseEventKind::Down(MouseButton::Right) = event.kind {
                // Second right-click dismisses the menu.
                self.context_menu = None;
            }
            self.assert_ui_invariants();
            self.debug_log_input_transition("mouse-context", &before_input, before_cursor);
            return;
        }

        match event.kind {
            MouseEventKind::ScrollUp => {
                if self.output_view.is_some()
                    && self
                        .output_view_area
                        .contains((event.column, event.row).into())
                {
                    self.scroll_output_view_by(-3);
                } else if self.show_log && self.log_area.contains((event.column, event.row).into())
                {
                    self.log_scroll_offset = self.log_scroll_offset.saturating_add(3);
                } else if self.message_area.contains((event.column, event.row).into()) {
                    self.scroll_offset = self.scroll_offset.saturating_add(3);
                }
            }
            MouseEventKind::ScrollDown => {
                if self.output_view.is_some()
                    && self
                        .output_view_area
                        .contains((event.column, event.row).into())
                {
                    self.scroll_output_view_by(3);
                } else if self.show_log && self.log_area.contains((event.column, event.row).into())
                {
                    self.log_scroll_offset = self.log_scroll_offset.saturating_sub(3);
                } else if self.message_area.contains((event.column, event.row).into()) {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let pos = (event.column, event.row);
                if self.agents_button_area.contains(pos.into()) {
                    if self.active_tasks.is_empty() {
                        return;
                    }
                    self.show_agents_window = !self.show_agents_window;
                    if self.show_agents_window {
                        self.show_teams_window = false;
                    }
                    return;
                }
                if self.teams_button_area.contains(pos.into()) {
                    if self.active_team.is_none() {
                        return;
                    }
                    self.show_teams_window = !self.show_teams_window;
                    if self.show_teams_window {
                        self.show_agents_window = false;
                    }
                    return;
                }
                if self.agents_close_button_area.contains(pos.into()) {
                    self.show_agents_window = false;
                    return;
                }
                if self.teams_close_button_area.contains(pos.into()) {
                    self.show_teams_window = false;
                    return;
                }
                if self.output_view.is_some()
                    && self
                        .output_view_area
                        .contains((event.column, event.row).into())
                {
                    return;
                }
                if self.output_view.is_some() {
                    self.output_view = None;
                    self.selected_agent_session_id = None;
                    self.selected_agent_index = None;
                }
                if self
                    .active_agents_area
                    .contains((event.column, event.row).into())
                {
                    let row = event
                        .row
                        .saturating_sub(self.active_agents_area.y.saturating_add(1));
                    let absolute_row =
                        row.saturating_add(self.active_agents_scroll_offset) as usize;
                    if absolute_row == 1 {
                        if let Some(ref sid) = self.session_id {
                            self.selected_agent_index = Some(0);
                            self.open_output_view_session(sid.clone(), "primary".to_string());
                        }
                        return;
                    }
                    if absolute_row >= 2 {
                        let idx = absolute_row - 2;
                        if let Some(task) = self.active_tasks.get(idx).cloned() {
                            self.selected_agent_index = Some(idx + 1);
                            self.open_output_view_session(
                                task.child_session_id.clone(),
                                format!("{} [{}]", task.agent_name, short_session_id(&task.id)),
                            );
                        }
                        return;
                    }
                }
                if self.teams_area.contains((event.column, event.row).into()) {
                    let row = event
                        .row
                        .saturating_sub(self.teams_area.y.saturating_add(1));
                    let absolute_row = row.saturating_add(self.teams_scroll_offset) as usize;
                    if absolute_row == 1 {
                        // Lead row clicked — unfocus any teammate
                        self.focused_teammate = None;
                        self.status = "focus: lead (you)".to_string();
                        return;
                    }
                    if absolute_row >= 2 {
                        let idx = absolute_row - 2;
                        if let Some(member) = self.team_members.get(idx).cloned() {
                            // Focus this teammate (same as /team focus <name>)
                            self.focus_teammate_by_id(&member.agent_id);
                        }
                        return;
                    }
                }
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
                        let Some(menu) = self.file_menu.as_ref() else {
                            return;
                        };
                        let input_area = self.active_input_widget_area();
                        let item_count = menu.matches.len() as u16;
                        let visible_items = item_count.max(1).min(8);
                        let height = (visible_items + 1 + 2).min(input_area.y);
                        let width = input_area.width.min(60);
                        let popup_x = input_area.x;
                        let popup_y = input_area.y.saturating_sub(height);

                        // If click is inside the popup, determine which row was clicked.
                        if event.column >= popup_x
                            && event.column < popup_x.saturating_add(width)
                            && event.row >= popup_y
                            && event.row < popup_y.saturating_add(height)
                        {
                            // Content lines start one row below the popup top (inside the border)
                            let clicked_row = event.row.saturating_sub(popup_y + 1) as usize;
                            let absolute_row = menu.scroll_offset + clicked_row;
                            if absolute_row < menu.matches.len() {
                                // Set the selected index (drop borrow immediately)
                                {
                                    if let Some(ref mut m) = self.file_menu.as_mut() {
                                        m.selected = absolute_row;
                                    }
                                }

                                // Accept the selection: this will navigate into directories
                                // or insert a file path into the input. We do not auto-send
                                // the message on mouse click; pressing Enter still sends.
                                let _ = self.accept_file_menu_selection();
                                return;
                            }
                        } else {
                            // Click outside popup dismisses the file menu.
                            self.file_menu = None;
                            return;
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
                }
            }
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
                    let input_area = self.active_input_widget_area();
                    let item_count = self
                        .file_menu
                        .as_ref()
                        .map(|m| m.matches.len())
                        .unwrap_or(0) as u16;
                    let visible_items = item_count.max(1).min(8);
                    let height = (visible_items + 1 + 2).min(input_area.y);
                    let width = input_area.width.min(60);
                    let popup_x = input_area.x;
                    let popup_y = input_area.y.saturating_sub(height);

                    if event.column >= popup_x
                        && event.column < popup_x.saturating_add(width)
                        && event.row >= popup_y
                        && event.row < popup_y.saturating_add(height)
                    {
                        let hovered_row = event.row.saturating_sub(popup_y + 1) as usize;
                        let absolute_row = self
                            .file_menu
                            .as_ref()
                            .map(|m| m.scroll_offset)
                            .unwrap_or(0)
                            + hovered_row;
                        if absolute_row < (item_count as usize) {
                            // Update selection if changed.
                            if let Some(ref mut m) = self.file_menu.as_mut() {
                                if m.selected != absolute_row {
                                    m.selected = absolute_row;
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
                // Right-click contract: always open context menu when inside a pane.
                // Actions are enabled only when valid for that pane + selection context.
                let col = event.column;
                let row = event.row;
                let Some(pane) = self.pane_at(col, row) else {
                    self.context_menu = None;
                    return;
                };

                let selection_for_pane =
                    self.text_selection.as_ref().is_some_and(|s| s.pane == pane);
                let in_input = matches!(pane, SelectionPane::Input);
                let has_clipboard = Self::get_clipboard().is_some_and(|s| !s.is_empty());
                let provider_setup_input = matches!(
                    self.provider_setup,
                    Some(ProviderSetupStep::EnterKey { .. })
                );

                let items = vec![
                    (ContextAction::Cut, selection_for_pane && in_input),
                    (ContextAction::Copy, selection_for_pane),
                    (
                        ContextAction::Paste,
                        if provider_setup_input {
                            true
                        } else {
                            in_input && has_clipboard
                        },
                    ),
                ];
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
        self.assert_ui_invariants();
        self.debug_log_input_transition("mouse", &before_input, before_cursor);
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
        } else {
            None
        }
    }

    /// Copy the currently selected text to the system clipboard.
    ///
    /// When `consume_selection` is true, clears the active selection after copying.
    fn copy_selection(&mut self, consume_selection: bool) {
        let sel = match self.text_selection.clone() {
            Some(s) => s,
            None => return,
        };
        if consume_selection {
            self.text_selection = None;
        }
        let ((start_col, start_row), (end_col, end_row)) = sel.normalized();

        let lines: &[String] = match sel.pane {
            SelectionPane::Messages => &self.message_content_lines,
            SelectionPane::Log => &self.log_content_lines,
            SelectionPane::Input => {
                // For input widgets, build a single-line content from app.input
                let input_text = format!("> {}", self.input);
                let area = self.input_area;
                let inner_x = area.x + 1; // inside border
                let inner_y = area.y + 1;
                let inner_w = area.width.saturating_sub(2).max(1) as usize;
                // Wrap the input text into display lines (character-width based).
                let chars: Vec<char> = input_text.chars().collect();
                let mut wrapped: Vec<String> = Vec::new();
                let mut start = 0usize;
                while start < chars.len() {
                    let end = (start + inner_w).min(chars.len());
                    wrapped.push(chars[start..end].iter().collect::<String>());
                    start = end;
                }
                if wrapped.is_empty() {
                    wrapped.push(String::new());
                }
                let text = Self::extract_text_from_lines(
                    &wrapped, inner_x, inner_y, start_col, start_row, end_col, end_row,
                );
                if !text.is_empty() {
                    Self::set_clipboard(&text);
                    self.push_log_no_agent(LogLevel::Info, format!("Copied {} chars", text.len()));
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
            self.push_log_no_agent(LogLevel::Info, format!("Copied {} chars", text.len()));
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
                start_col.saturating_sub(inner_x) as usize
            } else {
                0
            };
            let line_end = if screen_row == end_row {
                end_col.saturating_sub(inner_x) as usize + 1
            } else {
                line.chars().count()
            };
            let line_char_count = line.chars().count();
            let start = line_start.min(line_char_count);
            let end = line_end.min(line_char_count);
            if start < end {
                result.extend(line.chars().skip(start).take(end - start));
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
                Some(percent_decode_path(rest))
            } else if trimmed.starts_with('/') || trimmed.starts_with('.') {
                // Plain absolute or relative path
                Some(std::path::PathBuf::from(trimmed))
            } else {
                None
            };

            if let Some(path) = candidate {
                if path.exists() && is_image_path(&path) {
                    self.push_log_no_agent(
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
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("📎 Image saved from clipboard: {}", path.display()),
                    );
                    self.pending_attachments.push(path);
                }
                Err(e) => {
                    self.push_log_no_agent(
                        LogLevel::Warn,
                        format!("Failed to save clipboard image: {e}"),
                    );
                }
            }
        } else {
            self.push_log_no_agent(
                LogLevel::Info,
                "No image data found in clipboard".to_string(),
            );
        }
    }

    /// Execute a context menu action (Cut / Copy / Paste) and close the menu.
    pub fn execute_context_action(&mut self, action: ContextAction) {
        let pane = self.context_menu.as_ref().map(|m| m.pane);
        let selection = self.text_selection.clone();
        self.context_menu = None;

        match action {
            ContextAction::Copy => {
                self.copy_selection(false);
            }
            ContextAction::Cut => {
                // Copy selected text then remove only the selected span in input pane.
                self.copy_selection(true);
                if matches!(pane, Some(SelectionPane::Input)) {
                    if let Some(sel) = selection.as_ref()
                        && let Some((start, end)) = self.input_selection_char_range(sel)
                    {
                        self.remove_input_char_range(start, end);
                    }
                }
            }
            ContextAction::Paste => {
                if matches!(
                    self.provider_setup,
                    Some(ProviderSetupStep::EnterKey { .. })
                ) {
                    self.paste_provider_setup_from_clipboard();
                } else if matches!(pane, Some(SelectionPane::Input)) {
                    if let Some(text) = Self::get_clipboard() {
                        // Strip carriage returns but keep newlines (multiline input supported).
                        let clean: String = text.chars().filter(|&c| c != '\r').collect();
                        if let Some(sel) = selection.as_ref()
                            && let Some((start, end)) = self.input_selection_char_range(sel)
                        {
                            self.remove_input_char_range(start, end);
                        }
                        self.insert_text_at_cursor(&clean);
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
        let before_input = self.input.clone();
        let before_cursor = self.input_cursor;
        // History picker intercepts all keys while it is open
        if self.history_picker.is_some() {
            self.handle_history_picker_key(key);
            self.assert_ui_invariants();
            self.debug_log_input_transition("key-history-picker", &before_input, before_cursor);
            return;
        }
        if let Some(action) = input::handle_key(self, key) {
            match action {
                InputAction::SendMessage(text) => {
                    // When a teammate is focused, route the message to their
                    // mailbox instead of the lead session.
                    if let Some(ref focused_id) = self.focused_teammate.clone() {
                        if let Some(member) = self
                            .team_members
                            .iter()
                            .find(|m| m.agent_id == *focused_id)
                            .cloned()
                        {
                            let team_name = self
                                .active_team
                                .as_ref()
                                .map(|t| t.name.clone())
                                .unwrap_or_default();
                            self.send_teammate_message(&team_name, &member.name, &text);
                            self.input.clear();
                            self.input_cursor = 0;
                            self.history_index = None;
                            self.push_log_no_agent(
                                LogLevel::Info,
                                format!(
                                    "→ {} (focused): {}",
                                    member.name,
                                    &text[..text.len().min(60)]
                                ),
                            );
                            return;
                        }
                    }
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

                    // Create session if needed
                    if self.session_id.is_none() {
                        let dir = std::env::current_dir().unwrap_or_default();
                        match self.session_processor.session_manager.create_session(dir) {
                            Ok(session) => {
                                self.session_id = Some(session.id.clone());
                                // Map the primary session's short_sid to the current agent name
                                let short_sid = short_session_id(&session.id);
                                self.sid_to_display_name
                                    .insert(short_sid, self.agent_name.clone());
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
                    self.quit_armed = true;
                    self.status = "Press Ctrl+D to quit (or use /quit or /exit)".to_string();
                }
                InputAction::ConfirmQuit => {
                    if self.quit_armed {
                        self.is_running = false;
                    } else {
                        self.status = "Press Ctrl+C first, then Ctrl+D to quit".to_string();
                    }
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
                InputAction::ToggleLog => {
                    self.show_log = !self.show_log;
                    self.status = if self.show_log {
                        "log panel visible".to_string()
                    } else {
                        "log panel hidden".to_string()
                    };
                }
                InputAction::OutputViewPageUp => {
                    self.scroll_output_view_by(-5);
                }
                InputAction::OutputViewPageDown => {
                    self.scroll_output_view_by(5);
                }
                InputAction::OutputViewToStart => {
                    self.jump_output_view_start();
                }
                InputAction::OutputViewToEnd => {
                    self.jump_output_view_end();
                }
                InputAction::HistoryUp => {
                    // Within a multiline input, Up moves to the previous logical line.
                    // Only navigate history when already on the first logical line.
                    if self.history_index.is_none() && !self.cursor_on_first_logical_line() {
                        self.cursor_move_up_logical_line();
                        return;
                    }
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
                InputAction::HistoryDown => {
                    // Within a multiline input (while not browsing history), Down moves
                    // to the next logical line before navigating history.
                    if self.history_index.is_none() && !self.cursor_on_last_logical_line() {
                        self.cursor_move_down_logical_line();
                        return;
                    }
                    match self.history_index {
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
                    }
                }
                InputAction::MoveCursorLeft => {
                    // Standard: if selection active, jump to left boundary and clear.
                    if let Some((start, _)) = self.kb_selection_char_range() {
                        self.kb_select_anchor = None;
                        self.set_cursor_char_index_clamped(start);
                    } else {
                        self.cursor_move_left();
                    }
                }
                InputAction::MoveCursorRight => {
                    // Standard: if selection active, jump to right boundary and clear.
                    if let Some((_, end)) = self.kb_selection_char_range() {
                        self.kb_select_anchor = None;
                        self.set_cursor_char_index_clamped(end);
                    } else {
                        self.cursor_move_right();
                    }
                }
                InputAction::MoveCursorWordLeft => {
                    self.clear_kb_selection();
                    self.cursor_move_word_left();
                }
                InputAction::MoveCursorWordRight => {
                    self.clear_kb_selection();
                    self.cursor_move_word_right();
                }
                InputAction::MoveCursorHome => {
                    self.clear_kb_selection();
                    self.cursor_move_home();
                }
                InputAction::MoveCursorEnd => {
                    self.clear_kb_selection();
                    self.cursor_move_end();
                }
                InputAction::Delete => {
                    if let Some((start, end)) = self.kb_selection_char_range() {
                        self.remove_input_char_range(start, end);
                        self.kb_select_anchor = None;
                    } else {
                        self.delete_next_char();
                    }
                }
                InputAction::DeletePrevWord => {
                    self.clear_kb_selection();
                    self.delete_prev_word();
                }
                InputAction::DeleteToLineEnd => {
                    self.clear_kb_selection();
                    self.delete_to_end_of_line();
                }
                InputAction::SelectAll => {
                    self.kb_select_anchor = Some(0);
                    self.cursor_move_end();
                }
                InputAction::SelectCharLeft => {
                    if self.kb_select_anchor.is_none() {
                        self.kb_select_anchor = Some(self.input_cursor);
                    }
                    self.cursor_move_left();
                }
                InputAction::SelectCharRight => {
                    if self.kb_select_anchor.is_none() {
                        self.kb_select_anchor = Some(self.input_cursor);
                    }
                    self.cursor_move_right();
                }
                InputAction::SelectWordLeft => {
                    if self.kb_select_anchor.is_none() {
                        self.kb_select_anchor = Some(self.input_cursor);
                    }
                    self.cursor_move_word_left();
                }
                InputAction::SelectWordRight => {
                    if self.kb_select_anchor.is_none() {
                        self.kb_select_anchor = Some(self.input_cursor);
                    }
                    self.cursor_move_word_right();
                }
                InputAction::CopyToClipboard => {
                    self.copy_kb_selection();
                }
                InputAction::CutToClipboard => {
                    self.cut_kb_selection();
                }
                InputAction::PasteFromClipboard => {
                    self.paste_text_from_clipboard();
                }
                InputAction::SwitchAgent => {
                    if self.cycleable_agents.len() > 1 {
                        let prev = self.agent_name.clone();
                        self.current_agent_index =
                            (self.current_agent_index + 1) % self.cycleable_agents.len();
                        self.agent_info = self.cycleable_agents[self.current_agent_index].clone();
                        self.agent_name = self.agent_info.name.clone();
                        self.status = format!("agent: {}", self.agent_name);
                        self.push_log_no_agent(
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
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            "User pressed ESC — halting agent".to_string(),
                        );
                    }
                }
                InputAction::ConfirmForceCleanup => {
                    if self.pending_forcecleanup.is_some() {
                        // Clear pending modal state and invoke forcecleanup with confirm arg.
                        self.pending_forcecleanup = None;
                        self.execute_slash_command("/team forcecleanup confirm");
                    }
                }
                InputAction::CancelForceCleanup => {
                    if self.pending_forcecleanup.is_some() {
                        self.pending_forcecleanup = None;
                        self.append_assistant_text(
                            "From: /team forcecleanup\nForce-cleanup cancelled.",
                        );
                        self.push_log_no_agent(
                            LogLevel::Info,
                            "forcecleanup cancelled".to_string(),
                        );
                        self.status = "forcecleanup cancelled".to_string();
                    }
                }
                InputAction::ApprovePlan => {
                    if let Some(state) = self.plan_approval_pending.take() {
                        if let Some(ref session_id) = self.session_id.clone() {
                            self.push_log_no_agent(LogLevel::Info, "plan approved".to_string());
                            self.execute_plan_restore(session_id, &state.plan_text);
                        }
                    }
                }
                InputAction::RejectPlan => {
                    if let Some(state) = self.plan_approval_pending.take() {
                        if let Some(ref session_id) = self.session_id.clone() {
                            self.push_log_no_agent(
                                LogLevel::Info,
                                "plan rejected — re-delegating".to_string(),
                            );
                            self.append_assistant_text(
                                "From: /plan\n🔄 **Plan rejected** — re-delegating to plan agent for revision.\n",
                            );
                            self.execute_plan_delegation(
                                session_id,
                                "Revise the plan based on this feedback: please provide an improved plan".to_string(),
                                state.plan_text,
                            );
                        }
                    }
                }
                InputAction::TogglePlanCursor => {
                    if let Some(ref mut state) = self.plan_approval_pending {
                        state.cursor_approve = !state.cursor_approve;
                    }
                }
                InputAction::FocusNextTeammate => {
                    self.cycle_focused_teammate(true);
                }
                InputAction::FocusPrevTeammate => {
                    self.cycle_focused_teammate(false);
                }
                // InsertNewline is handled directly in handle_key (returns None),
                // so this arm should not be reached in normal operation.
                InputAction::InsertNewline => {
                    self.insert_char_at_cursor('\n');
                }
            }
        }
        self.assert_ui_invariants();
        self.debug_log_input_transition("key", &before_input, before_cursor);
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
            self.push_log_no_agent(
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
            self.push_log_no_agent(LogLevel::Error, "plan agent not found".to_string());
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
            self.push_log_no_agent(LogLevel::Info, format!("plan restore: plan → {}", to_name));

            self.event_bus.publish(Event::AgentSwitched {
                session_id: session_id.to_string(),
                from: from_name,
                to: to_name,
            });

            // Inject the plan summary into the chat so the restored agent
            // can see it in context.
            let plan_text = format!("📋 **Plan summary:**\n{}", summary);
            self.append_assistant_text(&plan_text);

            // Offer /swarm as an execution option after plan completion
            self.append_assistant_text(
                "\n💡 **Tip:** You can execute this plan in parallel with `/swarm <goal>`, \
                 or implement it step-by-step.\n",
            );
            self.force_new_message = true;
        } else {
            self.push_log_no_agent(
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
        // Mark UI dirty for any event handling
        self.needs_redraw = true;
        match event {
            Event::SessionCreated { ref session_id } => {
                if self.session_id.is_none() {
                    self.session_id = Some(session_id.clone());
                    // Map the primary session's short_sid to the current agent name
                    let short_sid = short_session_id(session_id);
                    self.sid_to_display_name
                        .insert(short_sid, self.agent_name.clone());
                    self.push_log_no_agent(
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
                    let step = self.event_bus.current_step(session_id) as u32;
                    let short_sid = short_session_id(session_id);
                    // Check if step changed - if so, reset substep counter to 0
                    let last_step = self
                        .last_step_per_session
                        .get(session_id)
                        .copied()
                        .unwrap_or(0);
                    if step != last_step {
                        self.substep_counter_per_session
                            .insert(session_id.clone(), 0);
                        self.last_step_per_session.insert(session_id.clone(), step);
                    }
                    // Increment sub-step counter for this session
                    let substep = self
                        .substep_counter_per_session
                        .entry(session_id.clone())
                        .or_insert(0);
                    *substep += 1;
                    let current_substep = *substep;
                    self.tool_step_map
                        .insert(call_id.clone(), (short_sid.clone(), step, current_substep));
                    self.add_tool_call_part(tool, call_id);
                    self.status = format!("running: {}", tool);
                    let display_name = self
                        .sid_to_display_name
                        .get(&short_sid)
                        .cloned()
                        .unwrap_or(short_sid);
                    self.push_log_no_agent(
                        LogLevel::Tool,
                        format!(
                            "[{display_name}:{step}.{current_substep}] tool call: {}",
                            tool
                        ),
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
                    self.status = "processing...".to_string();
                    let step_tag = self
                        .tool_step_map
                        .get(call_id)
                        .map(|(sid, step, substep)| {
                            let name = self
                                .sid_to_display_name
                                .get(sid)
                                .cloned()
                                .unwrap_or_else(|| sid.clone());
                            format!("[{name}:{step}.{substep}] ")
                        })
                        .unwrap_or_default();
                    if let Some(err) = error {
                        self.push_log_no_agent(
                            LogLevel::Error,
                            format!(
                                "{}tool {} failed: {} ({}ms)",
                                step_tag, tool, err, duration_ms
                            ),
                        );
                    } else {
                        self.push_log_no_agent(
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
                    self.push_log_no_agent(
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
                ref message_id,
                ref reason,
            } => {
                if self.is_current_session(session_id) {
                    // The "init" message_id is used exclusively by the AGENTS.md
                    // acknowledgment exchange that runs before the main agent loop.
                    // It must NOT reset processing state — the main loop hasn't
                    // started yet.  Only set force_new_message so the real response
                    // starts in a fresh message block.
                    if message_id == "init" {
                        self.force_new_message = true;
                        return;
                    }
                    let was_auto_compaction = self.auto_compact_in_progress;
                    self.is_processing = false;
                    self.cancel_flag = None;
                    if *reason == FinishReason::Cancelled {
                        self.agent_halted = true;
                        self.status = "halted — /resume to continue".to_string();
                        self.push_log_no_agent(LogLevel::Warn, "Agent halted by user".to_string());
                    } else {
                        self.agent_halted = false;
                        self.status = "ready".to_string();
                    }
                    self.force_new_message = true;
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("response finished ({reason:?})"),
                    );

                    // After compaction: replace session message history with just the summary.
                    // The summary is the last assistant message in self.messages.
                    if self.compact_in_progress && *reason != FinishReason::Cancelled {
                        self.compact_in_progress = false;
                        let summary_text = self
                            .messages
                            .iter()
                            .rev()
                            .find(|m| m.role == Role::Assistant)
                            .map(|m| m.text_content());
                        if let Some(summary) = summary_text {
                            if !summary.trim().is_empty() {
                                let sid = session_id.clone();
                                let summary_msg = Message::new(
                                    &sid,
                                    Role::Assistant,
                                    vec![MessagePart::Text {
                                        text: format!("[Conversation compacted]\n\n{}", summary),
                                    }],
                                );
                                if let Err(e) = self.storage.delete_messages(&sid) {
                                    self.push_log_no_agent(
                                        LogLevel::Warn,
                                        format!("Compaction: failed to clear messages: {e}"),
                                    );
                                } else if let Err(e) = self.storage.create_message(&summary_msg) {
                                    self.push_log_no_agent(
                                        LogLevel::Warn,
                                        format!("Compaction: failed to save summary: {e}"),
                                    );
                                } else {
                                    self.messages = vec![summary_msg];
                                    self.push_log_no_agent(
                                        LogLevel::Info,
                                        "Compaction: session history replaced with summary"
                                            .to_string(),
                                    );
                                }
                            }
                        }
                    } else {
                        self.compact_in_progress = false;
                    }

                    // Handle pending plan delegation: switch agent and auto-send task
                    if let Some((task, context)) = self.pending_plan_task.take() {
                        self.execute_plan_delegation(session_id, task, context);
                    }

                    // Autopilot auto-continue: after agent completes a turn without calling
                    // task_complete, automatically send a continuation prompt so the agent
                    // keeps working towards its goal.
                    if self.autopilot_enabled && *reason != FinishReason::Cancelled {
                        // Check time limit
                        let time_exceeded = self
                            .autopilot_time_limit_secs
                            .and_then(|limit| {
                                self.autopilot_started_at
                                    .map(|s| s.elapsed().as_secs() >= limit)
                            })
                            .unwrap_or(false);
                        if time_exceeded {
                            self.autopilot_enabled = false;
                            self.autopilot_started_at = None;
                            self.autopilot_pending_continue = None;
                            self.status = "autopilot: time limit reached".to_string();
                            self.append_assistant_text(
                                "⚡ **Autopilot stopped** — time limit reached.",
                            );
                            self.push_log_no_agent(
                                LogLevel::Warn,
                                "autopilot stopped: time limit".to_string(),
                            );
                        } else {
                            // Schedule a continuation on the next render tick
                            self.autopilot_pending_continue = Some(
                                "Continue working on the task. When fully done, call task_complete with a summary.".to_string()
                            );
                            self.status = "⚡ autopilot".to_string();
                        }
                    }

                    if was_auto_compaction {
                        self.auto_compact_in_progress = false;
                        self.push_log_no_agent(
                            LogLevel::Info,
                            "Auto-compaction completed".to_string(),
                        );
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
                    self.push_log_no_agent(
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
                    self.pending_question_input.clear();
                    self.status = "processing...".to_string();
                    self.push_log_no_agent(
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
                    // Update the display name mapping for the current session
                    if let Some(ref sid) = self.session_id {
                        let short_sid = short_session_id(sid);
                        self.sid_to_display_name.insert(short_sid, to.clone());
                    }
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("agent switched: {} → {}", from, to),
                    );
                }
            }
            Event::AgentSwitchRequested {
                ref session_id,
                ref to,
                ref task,
                ref context,
            } => {
                if self.is_current_session(session_id) {
                    self.push_log_no_agent(
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
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("agent restore requested ({} chars)", summary.len()),
                    );
                    // Show plan approval dialog instead of immediately restoring.
                    // The user presses Approve/Reject (Enter/r) to proceed.
                    self.plan_approval_pending = Some(PlanApprovalState {
                        plan_text: summary.clone(),
                        cursor_approve: true,
                    });
                }
            }
            Event::TaskCompleted {
                ref session_id,
                ref summary,
            } => {
                if self.is_current_session(session_id) {
                    self.push_log_no_agent(LogLevel::Info, "task_complete signalled".to_string());
                    // Exit autopilot mode on task completion
                    if self.autopilot_enabled {
                        self.autopilot_enabled = false;
                        self.autopilot_started_at = None;
                        self.autopilot_pending_continue = None;
                        self.status = "task complete".to_string();
                        self.push_log_no_agent(
                            LogLevel::Info,
                            "autopilot stopped: task complete".to_string(),
                        );
                    }
                    self.append_assistant_text(&format!("✅ **Task Complete**\n\n{}", summary));
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
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            "Auto-compaction failed; send blocked for this turn".to_string(),
                        );
                    }
                    self.compact_in_progress = false;
                    // Full details go to the log panel only
                    self.push_log_no_agent(LogLevel::Error, format!("agent error: {}", error));
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
                    self.push_log_no_agent(
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
                    self.push_log_no_agent(LogLevel::Info, format!("quota: {:.1}% used", percent));
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
                        self.push_log_no_agent(
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
                input_tokens,
                output_tokens,
            } => {
                if self.is_current_session(session_id) {
                    if let Some(model_ref) = self.active_model_ref_string() {
                        self.llm_request_stats.push(LlmRequestStat {
                            model_ref,
                            elapsed_ms,
                            input_tokens,
                            output_tokens,
                        });
                    }
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("model response ({elapsed_ms}ms): {text}"),
                    );
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
                        .map(|(sid, step, substep)| {
                            let display = self
                                .sid_to_display_name
                                .get(sid)
                                .cloned()
                                .unwrap_or_else(|| sid.clone());
                            format!("[{display}:{step}.{substep}] ")
                        })
                        .unwrap_or_default();
                    // Pretty-print JSON args across multiple log lines
                    let pretty = serde_json::from_str::<serde_json::Value>(args)
                        .ok()
                        .and_then(|v| serde_json::to_string_pretty(&v).ok());
                    if let Some(formatted) = pretty {
                        let mut first = true;
                        for line in formatted.lines() {
                            if first {
                                self.push_log_no_agent(
                                    LogLevel::Tool,
                                    format!("{}→ {} {}", step_tag, tool, line),
                                );
                                first = false;
                            } else {
                                self.push_log_no_agent(LogLevel::Tool, format!("  {}", line));
                            }
                        }
                    } else {
                        self.push_log_no_agent(
                            LogLevel::Tool,
                            format!("{}→ {}({})", step_tag, tool, args),
                        );
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
                    if *tool == "team_create"
                        && success
                        && let Some(meta) = metadata
                        && let Some(team_name) = meta.get("team_name").and_then(|v| v.as_str())
                    {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        if let Ok(store) = TeamStore::load_by_name(team_name, &working_dir) {
                            let name = store.config.name.clone();
                            let team_dir = store.dir.clone();
                            self.team_members = store.config.members.clone();
                            self.active_team = Some(store.config);
                            self.show_teams = true;
                            // Reconcile is needed here: team was created via LLM tool path,
                            // so the TeamManager didn't exist during blueprint seeding and
                            // members may have been queued in Spawning state.
                            self.ensure_team_manager_for_team_inner(&name, Some(team_dir), true);
                        }
                    }
                    let step_tag = self
                        .tool_step_map
                        .get(call_id)
                        .map(|(sid, step, substep)| {
                            let display = self
                                .sid_to_display_name
                                .get(sid)
                                .cloned()
                                .unwrap_or_else(|| sid.clone());
                            format!("[{display}:{step}.{substep}] ")
                        })
                        .unwrap_or_default();
                    let icon = if success { "✓" } else { "✗" };
                    self.push_log_no_agent(
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
                    // Map the child session's short_sid to the agent name for display
                    let short_sid = short_session_id(child_session_id);
                    self.sid_to_display_name.insert(short_sid, agent.clone());

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
                    self.push_log_no_agent(
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
                    self.push_log_no_agent(
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
                    self.push_log_no_agent(
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
                self.push_log_no_agent(
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
                        let member =
                            TeamMember::new(teammate_name.as_str(), agent_id.as_str(), "teammate");
                        self.team_members.push(member);
                        self.team_message_counts
                            .entry(agent_id.clone())
                            .or_insert((0, 0));
                    }
                    // Always refresh the stored values (session id, status, current task)
                    // from disk so races between UI hydration and spawn events don't
                    // leave the UI showing an outdated state.
                    if let Some(m) = self
                        .team_members
                        .iter_mut()
                        .find(|m| m.agent_id == *agent_id)
                    {
                        let working_dir = std::env::current_dir().unwrap_or_default();
                        if let Ok(store) = TeamStore::load_by_name(team_name, &working_dir)
                            && let Some(stored) = store
                                .config
                                .members
                                .iter()
                                .find(|x| x.agent_id == *agent_id)
                        {
                            m.session_id = stored.session_id.clone();
                            m.status = stored.status.clone();
                            m.current_task_id = stored.current_task_id.clone();
                            // Map this teammate's session short_sid → name for log display
                            if let Some(ref sid) = stored.session_id {
                                let short_sid = short_session_id(sid);
                                self.sid_to_display_name
                                    .insert(short_sid, teammate_name.clone());
                            }
                        }
                    }
                    self.show_teams = true;
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("🤝 [{team_name}] Spawned teammate '{teammate_name}' ({agent_id})"),
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
                    if from.as_str() != "lead" {
                        let counts = self
                            .team_message_counts
                            .entry(from.clone())
                            .or_insert((0, 0));
                        counts.0 = counts.0.saturating_add(1);
                    }
                    if to.as_str() != "lead" {
                        let counts = self.team_message_counts.entry(to.clone()).or_insert((0, 0));
                        counts.1 = counts.1.saturating_add(1);
                    }
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("📨 [{team_name}] {from} → {to}: {preview}"),
                    );
                }
            }
            Event::TeammateP2PMessage {
                ref session_id,
                ref team_name,
                ref from,
                ref to,
                ref preview,
            } => {
                if self.is_current_session(session_id) {
                    // Track sent count for sender.
                    let from_counts = self
                        .team_message_counts
                        .entry(from.clone())
                        .or_insert((0, 0));
                    from_counts.0 = from_counts.0.saturating_add(1);
                    // Track received count for recipient.
                    let to_counts = self.team_message_counts.entry(to.clone()).or_insert((0, 0));
                    to_counts.1 = to_counts.1.saturating_add(1);
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("🔀 [{team_name}] P2P {from} → {to}: {preview}"),
                    );
                }
            }
            Event::TeammateIdle {
                ref session_id,
                ref team_name,
                ref agent_id,
            } => {
                if self.is_current_session(session_id) {
                    if let Some(m) = self
                        .team_members
                        .iter_mut()
                        .find(|m| m.agent_id == *agent_id)
                    {
                        m.status = MemberStatus::Idle;
                    }
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("💤 [{team_name}] Teammate {agent_id} is idle"),
                    );
                }
            }
            Event::TeammateFailed {
                ref session_id,
                ref team_name,
                ref agent_id,
                ref error,
            } => {
                if self.is_current_session(session_id) {
                    if let Some(m) = self
                        .team_members
                        .iter_mut()
                        .find(|m| m.agent_id == *agent_id)
                    {
                        m.status = MemberStatus::Failed;
                        m.last_spawn_error = Some(error.clone());
                    }
                    let short_err = if error.len() > 200 {
                        let mut end = 200;
                        while end > 0 && !error.is_char_boundary(end) {
                            end -= 1;
                        }
                        format!("{}…", &error[..end])
                    } else {
                        error.to_string()
                    };
                    self.push_log_no_agent(
                        LogLevel::Error,
                        format!("❌ [{team_name}] Teammate {agent_id} failed: {short_err}"),
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
                    if let Some(m) = self
                        .team_members
                        .iter_mut()
                        .find(|m| m.agent_id == *agent_id)
                    {
                        m.status = MemberStatus::Working;
                        m.current_task_id = Some(task_id.clone());
                    }
                    self.push_log_no_agent(
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
                    if let Some(m) = self
                        .team_members
                        .iter_mut()
                        .find(|m| m.agent_id == *agent_id)
                    {
                        m.current_task_id = None;
                    }
                    self.push_log_no_agent(
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
                    self.team_message_counts.clear();
                    self.show_teams = false;
                    self.focused_teammate = None;
                    if self
                        .swarm_state
                        .as_ref()
                        .is_some_and(|s| &s.team_name == team_name)
                    {
                        self.swarm_state = None;
                    }
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("🗑️  Team '{team_name}' cleaned up"),
                    );
                }
            }
            Event::ShellCwdChanged {
                ref session_id,
                ref cwd,
            } => {
                if self.is_current_session(session_id) {
                    self.shell_cwd = Some(cwd.clone());
                }
            }
            Event::UserInput { ref session_id, .. } => {
                if self.is_current_session(session_id) {
                    // The tool is unblocked; clear the question UI and resume.
                    self.permission_pending = None;
                    self.pending_question_input.clear();
                    self.status = "processing...".to_string();
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
            self.push_log_no_agent(
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
    /// app.push_log(LogLevel::Info, "Session started".to_string(), None);
    /// # }
    /// ```
    pub fn push_log(&mut self, level: LogLevel, message: String, agent_id: Option<String>) {
        self.log_entries.push(LogEntry {
            timestamp: chrono::Utc::now(),
            level,
            message,
            session_id: self.session_id.clone(),
            agent_id,
        });
    }

    /// Helper: push log with no agent_id (for backwards compatibility during transition).
    #[allow(dead_code)]
    fn push_log_no_agent(&mut self, level: LogLevel, message: String) {
        self.push_log(level, message, None);
    }

    fn open_output_view_session(&mut self, session_id: String, label: String) {
        self.selected_agent_session_id = Some(session_id.clone());
        self.output_view = Some(OutputViewState {
            target: OutputViewTarget::Session { session_id, label },
            scroll_offset: 0,
            max_scroll: 0,
        });
    }

    fn open_output_view_team_member(
        &mut self,
        team_name: String,
        agent_id: String,
        teammate_name: String,
        session_id: Option<String>,
    ) {
        self.selected_agent_session_id = session_id.clone();
        self.output_view = Some(OutputViewState {
            target: OutputViewTarget::TeamMember {
                team_name,
                agent_id,
                teammate_name,
                session_id,
            },
            scroll_offset: 0,
            max_scroll: 0,
        });
    }

    fn scroll_output_view_by(&mut self, delta: i16) {
        if let Some(ref mut view) = self.output_view {
            if delta >= 0 {
                view.scroll_offset = (view.scroll_offset + delta as u16).min(view.max_scroll);
            } else {
                view.scroll_offset = view.scroll_offset.saturating_sub((-delta) as u16);
            }
        }
    }

    fn jump_output_view_start(&mut self) {
        if let Some(ref mut view) = self.output_view {
            view.scroll_offset = 0;
        }
    }

    fn jump_output_view_end(&mut self) {
        if let Some(ref mut view) = self.output_view {
            view.scroll_offset = view.max_scroll;
        }
    }

    /// Cycle the focused teammate forward or backward.
    ///
    /// Cycling order: None → first teammate → second → … → last → None → …
    fn cycle_focused_teammate(&mut self, forward: bool) {
        if self.team_members.is_empty() {
            self.focused_teammate = None;
            return;
        }
        let ids: Vec<String> = self
            .team_members
            .iter()
            .map(|m| m.agent_id.clone())
            .collect();
        let current_idx = self
            .focused_teammate
            .as_ref()
            .and_then(|f| ids.iter().position(|id| id == f));
        let next = match (current_idx, forward) {
            (None, true) => Some(0),
            (None, false) => Some(ids.len() - 1),
            (Some(i), true) => {
                if i + 1 >= ids.len() {
                    None
                } else {
                    Some(i + 1)
                }
            }
            (Some(i), false) => {
                if i == 0 {
                    None
                } else {
                    Some(i - 1)
                }
            }
        };
        match next {
            Some(idx) => {
                let agent_id = ids[idx].clone();
                self.focus_teammate_by_id(&agent_id);
            }
            None => {
                self.focused_teammate = None;
                self.output_view = None;
                self.status = "team: focus cleared".to_string();
            }
        }
    }

    /// Focus a specific teammate by agent_id, opening their output view.
    fn focus_teammate_by_id(&mut self, agent_id: &str) {
        let member = self
            .team_members
            .iter()
            .find(|m| m.agent_id == agent_id)
            .cloned();
        if let Some(m) = member {
            self.focused_teammate = Some(m.agent_id.clone());
            let team_name = self
                .active_team
                .as_ref()
                .map(|t| t.name.clone())
                .unwrap_or_default();
            self.open_output_view_team_member(
                team_name,
                m.agent_id.clone(),
                m.name.clone(),
                m.session_id.clone(),
            );
            self.status = format!("team: focused → {}", m.name);
        }
    }

    /// Focus a teammate by name (partial match supported).
    fn focus_teammate_by_name(&mut self, name: &str) -> Result<(), String> {
        let lower = name.to_lowercase();
        let matches: Vec<_> = self
            .team_members
            .iter()
            .filter(|m| {
                m.name.to_lowercase().contains(&lower) || m.agent_id.to_lowercase().contains(&lower)
            })
            .cloned()
            .collect();
        match matches.len() {
            0 => Err(format!("No teammate matching '{name}'")),
            1 => {
                let agent_id = matches[0].agent_id.clone();
                self.focus_teammate_by_id(&agent_id);
                Ok(())
            }
            _ => {
                let names: Vec<_> = matches.iter().map(|m| m.name.as_str()).collect();
                Err(format!("Ambiguous: matches {}", names.join(", ")))
            }
        }
    }

    /// Send a message to a teammate's mailbox.
    fn send_teammate_message(&mut self, team_name: &str, teammate_name: &str, text: &str) {
        let member = self
            .team_members
            .iter()
            .find(|m| m.name == teammate_name)
            .cloned();
        let working_dir = std::env::current_dir().unwrap_or_default();
        match (self.active_team.as_ref(), member) {
            (Some(_team), Some(member)) => match TeamStore::load_by_name(team_name, &working_dir) {
                Ok(store) => match Mailbox::open(&store.dir, &member.agent_id) {
                    Ok(mb) => {
                        let msg = MailboxMessage::new(
                            "lead",
                            &member.agent_id,
                            MessageType::Message,
                            text,
                        );
                        match mb.push(msg) {
                            Ok(_) => {
                                self.push_log_no_agent(
                                    LogLevel::Info,
                                    format!("📨 lead → {teammate_name}: {text}"),
                                );
                                self.status = format!("message sent to {teammate_name}");
                            }
                            Err(e) => {
                                self.status = format!("Failed to send message: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        self.status = format!("Failed to open mailbox: {e}");
                    }
                },
                Err(e) => {
                    self.status = format!("Failed to load team: {e}");
                }
            },
            (None, _) => {
                self.status = "No active team".to_string();
            }
            (Some(_), None) => {
                self.status = format!("Teammate '{teammate_name}' not found");
            }
        }
    }

    // ── Swarm helpers ───────────────────────────────────────────────────────

    /// Process a successful decomposition and create the ephemeral swarm team.
    fn execute_swarm_decomposition(
        &mut self,
        decomposition: ragent_core::team::SwarmDecomposition,
    ) {
        use ragent_core::team::task::Task;
        use ragent_core::team::{SwarmState, TaskStore, TeamStore};

        let task_count = decomposition.tasks.len();
        if task_count == 0 {
            self.status = "⚠ swarm: LLM returned 0 subtasks".to_string();
            self.append_assistant_text(
                "From: /swarm\n## ⚠ No subtasks\n\nThe LLM returned an empty task list.\n",
            );
            return;
        }

        // Create ephemeral team name
        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let team_name = format!("swarm-{ts}");
        let working_dir = std::env::current_dir().unwrap_or_default();
        let lead_sid = self
            .session_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        // Create the team
        match TeamStore::create(&team_name, &lead_sid, &working_dir, true) {
            Ok(store) => {
                // Seed tasks into tasks.json
                if let Ok(task_store) = TaskStore::open(&store.dir) {
                    for st in &decomposition.tasks {
                        let mut task = Task::new(&st.id, &st.title);
                        task.description = st.description.clone();
                        task.depends_on = st.depends_on.clone();
                        if let Err(e) = task_store.add_task(task) {
                            self.push_log_no_agent(
                                LogLevel::Warn,
                                format!("Swarm: failed to add task {}: {e}", st.id),
                            );
                        }
                    }
                }

                // Set up active team state
                self.active_team = Some(store.config.clone());
                self.team_members.clear();
                self.show_teams = true;
                self.ensure_team_manager_for_team(&team_name, Some(store.dir.clone()));

                // Build display table
                let mut output = format!(
                    "From: /swarm\n## 🐝 Swarm Created: {team_name}\n\n\
                    **{task_count} subtasks** decomposed and seeded.\n\n\
                    | ID | Title | Dependencies |\n\
                    |----|-------|--------------|\n"
                );
                for st in &decomposition.tasks {
                    let deps = if st.depends_on.is_empty() {
                        "—".to_string()
                    } else {
                        st.depends_on.join(", ")
                    };
                    output.push_str(&format!("| {} | {} | {} |\n", st.id, st.title, deps));
                }
                output.push_str("\nSpawning teammates…\n");
                self.append_assistant_text(&output);

                // Record swarm state (prompt is blank for now — it was consumed in the slash command)
                let swarm_prompt = String::new();
                self.swarm_state = Some(SwarmState {
                    team_name: team_name.clone(),
                    prompt: swarm_prompt,
                    decomposition: decomposition.clone(),
                    spawned: false,
                    completed: false,
                });

                // Spawn one teammate per subtask
                self.spawn_swarm_teammates(&team_name, &decomposition, &store.dir);
            }
            Err(e) => {
                self.status = format!("⚠ swarm: team creation failed: {e}");
                self.push_log_no_agent(LogLevel::Warn, format!("Swarm team creation failed: {e}"));
            }
        }
    }

    /// Spawn one teammate per swarm subtask using the team_spawn tool pattern.
    fn spawn_swarm_teammates(
        &mut self,
        team_name: &str,
        decomposition: &ragent_core::team::SwarmDecomposition,
        team_dir: &std::path::Path,
    ) {
        let working_dir = std::env::current_dir().unwrap_or_default();

        for subtask in &decomposition.tasks {
            let teammate_name = format!("swarm-{}", subtask.id);
            let agent_type = subtask
                .agent_type
                .as_deref()
                .unwrap_or("general")
                .to_string();

            // Build a rich prompt with task context
            let prompt = format!(
                "## Swarm Task: {}\n\n\
                **Task ID:** {}\n\
                **Title:** {}\n\n\
                {}\n\n\
                You are part of a swarm team. Complete this specific task.\n\n\
                IMPORTANT: Your VERY FIRST action must be a tool call. \
                Call `team_read_messages` with team_name set to the team name from your context. \
                Do NOT respond with text first — call the tool immediately.\n\n\
                After reading messages, do the work described above using tool calls \
                (glob, read, bash, etc.). \
                When done, call `team_task_complete` to mark task \"{}\" as completed.\n\
                Focus only on your assigned task — other teammates are handling other parts.",
                subtask.title, subtask.id, subtask.title, subtask.description, subtask.id
            );

            // Parse per-subtask model override
            let teammate_model: Option<ragent_core::agent::ModelRef> =
                subtask.model.as_deref().and_then(|s| {
                    s.split_once('/')
                        .or_else(|| s.split_once(':'))
                        .map(|(p, m)| ragent_core::agent::ModelRef {
                            provider_id: p.to_string(),
                            model_id: m.to_string(),
                        })
                });

            // Tasks with unresolved dependencies start as Blocked; others as Spawning
            let has_deps = !subtask.depends_on.is_empty();
            let initial_status = if has_deps {
                MemberStatus::Blocked
            } else {
                MemberStatus::Spawning
            };

            // Record member in config
            {
                if let Ok(mut store) =
                    ragent_core::team::TeamStore::load_by_name(team_name, &working_dir)
                {
                    if store.config.member_by_name(&teammate_name).is_none() {
                        let agent_id = store.next_agent_id();
                        let mut member = ragent_core::team::TeamMember::new(
                            &teammate_name,
                            &agent_id,
                            &agent_type,
                        );
                        member.spawn_prompt = Some(prompt.clone());
                        member.model_override = teammate_model.clone();
                        member.status = initial_status;
                        store.config.members.push(member.clone());
                        let _ = store.save();

                        // Add to local state
                        self.team_members.push(member);
                    }
                }
            }

            let status_label = if has_deps {
                "blocked (deps)"
            } else {
                "spawning"
            };
            self.push_log_no_agent(
                LogLevel::Info,
                format!(
                    "🐝 Swarm teammate: {} ({}) — {}",
                    teammate_name, subtask.id, status_label
                ),
            );
        }

        // Trigger reconcile — the manager picks up Spawning members and spawns them.
        // Blocked members are skipped by reconcile (they aren't MemberStatus::Spawning).
        if let Some(manager) = self.session_processor.team_manager.get() {
            manager.clone().reconcile_spawning_members();
        } else {
            self.ensure_team_manager_for_team_inner(team_name, Some(team_dir.to_path_buf()), true);
        }

        if let Some(ref mut swarm) = self.swarm_state {
            swarm.spawned = true;
        }

        let ready = decomposition
            .tasks
            .iter()
            .filter(|t| t.depends_on.is_empty())
            .count();
        let blocked = decomposition.tasks.len() - ready;
        self.status = format!("🐝 swarm: {ready} spawning, {blocked} blocked");
    }

    /// Handle `/swarm status` — display progress of active swarm.
    fn handle_swarm_status(&mut self) {
        let Some(ref swarm) = self.swarm_state else {
            self.append_assistant_text(
                "From: /swarm status\n\nNo active swarm. Use `/swarm <prompt>` to start one.\n",
            );
            return;
        };

        let mut output = format!("From: /swarm status\n## 🐝 Swarm: {}\n\n", swarm.team_name);

        // Load tasks from disk for current status
        let working_dir = std::env::current_dir().unwrap_or_default();
        let tasks = if let Ok(store) =
            ragent_core::team::TeamStore::load_by_name(&swarm.team_name, &working_dir)
        {
            if let Ok(ts) = ragent_core::team::TaskStore::open(&store.dir) {
                ts.read().ok()
            } else {
                None
            }
        } else {
            None
        };

        let total = swarm.decomposition.tasks.len();
        let (completed, in_progress, pending) = if let Some(ref tl) = tasks {
            let c = tl
                .tasks
                .iter()
                .filter(|t| t.status == ragent_core::team::TaskStatus::Completed)
                .count();
            let ip = tl
                .tasks
                .iter()
                .filter(|t| t.status == ragent_core::team::TaskStatus::InProgress)
                .count();
            let p = tl
                .tasks
                .iter()
                .filter(|t| t.status == ragent_core::team::TaskStatus::Pending)
                .count();
            (c, ip, p)
        } else {
            (0, 0, total)
        };

        // Progress bar
        let bar_width = 30;
        let filled = if total > 0 {
            (completed * bar_width) / total
        } else {
            0
        };
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
        output.push_str(&format!(
            "**Progress:** [{bar}] {completed}/{total} ({} in progress, {} pending)\n\n",
            in_progress, pending
        ));

        // Task table
        output.push_str("| ID | Title | Status | Assigned | Dependencies |\n");
        output.push_str("|----|-------|--------|----------|-------------|\n");

        if let Some(ref tl) = tasks {
            for task in &tl.tasks {
                let status_icon = match task.status {
                    ragent_core::team::TaskStatus::Completed => "✅",
                    ragent_core::team::TaskStatus::InProgress => "🔄",
                    ragent_core::team::TaskStatus::Pending => "⏳",
                    ragent_core::team::TaskStatus::Cancelled => "❌",
                };
                let assigned = task.assigned_to.as_deref().unwrap_or("—");
                let deps = if task.depends_on.is_empty() {
                    "—".to_string()
                } else {
                    task.depends_on.join(", ")
                };
                output.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    task.id, task.title, status_icon, assigned, deps
                ));
            }
        } else {
            for st in &swarm.decomposition.tasks {
                let deps = if st.depends_on.is_empty() {
                    "—".to_string()
                } else {
                    st.depends_on.join(", ")
                };
                output.push_str(&format!(
                    "| {} | {} | ⏳ | — | {} |\n",
                    st.id, st.title, deps
                ));
            }
        }

        // Teammate status
        output.push_str("\n**Teammates:**\n");
        if self.team_members.is_empty() {
            output.push_str("  (spawning…)\n");
        } else {
            for m in &self.team_members {
                let status = format!("{:?}", m.status).to_lowercase();
                output.push_str(&format!("  • {} — {}\n", m.name, status));
            }
        }

        if completed == total && total > 0 {
            output.push_str("\n🎉 **All tasks complete!** Use `/swarm cancel` to clean up.\n");
        }

        self.append_assistant_text(&output);
    }

    /// Handle `/swarm cancel` — tear down the swarm team.
    fn handle_swarm_cancel(&mut self) {
        let Some(swarm) = self.swarm_state.take() else {
            self.append_assistant_text("From: /swarm cancel\n\nNo active swarm to cancel.\n");
            return;
        };

        // Reuse the existing team cleanup path
        let team_name = swarm.team_name.clone();

        // Trigger team cleanup
        self.execute_slash_command(&format!("/team close {}", team_name));

        self.append_assistant_text(&format!(
            "From: /swarm cancel\n## 🐝 Swarm Cancelled\n\n\
            Swarm **{team_name}** has been shut down.\n"
        ));
        self.status = "swarm: cancelled".to_string();
        self.push_log_no_agent(LogLevel::Info, format!("Swarm cancelled: {team_name}"));
    }

    /// Check if any blocked swarm tasks can be unblocked now that deps have completed.
    ///
    /// A blocked member becomes Spawning when all its dependency tasks (by task ID)
    /// have been completed by their respective teammates (member status Idle/Stopped,
    /// or task status Completed in the TaskStore).
    pub fn poll_swarm_unblock(&mut self) {
        let Some(ref swarm) = self.swarm_state else {
            return;
        };
        if swarm.completed {
            return;
        }

        // Clone what we need from swarm_state to avoid borrow issues
        let team_name = swarm.team_name.clone();
        let decomp_tasks = swarm.decomposition.tasks.clone();

        // Find blocked members
        let blocked_members: Vec<(String, String)> = self
            .team_members
            .iter()
            .filter(|m| m.status == MemberStatus::Blocked)
            .map(|m| (m.name.clone(), m.agent_id.clone()))
            .collect();

        if blocked_members.is_empty() {
            return;
        }

        // Build set of completed task IDs from member status.
        // A task ID is the suffix after "swarm-" in the teammate name.
        let completed_task_ids: std::collections::HashSet<String> = self
            .team_members
            .iter()
            .filter(|m| matches!(m.status, MemberStatus::Idle | MemberStatus::Stopped))
            .filter_map(|m| m.name.strip_prefix("swarm-").map(|s| s.to_string()))
            .collect();

        // Also check TaskStore for explicitly completed tasks
        let working_dir = std::env::current_dir().unwrap_or_default();
        let task_completed_ids: std::collections::HashSet<String> = if let Ok(store) =
            ragent_core::team::TeamStore::load_by_name(&team_name, &working_dir)
        {
            if let Ok(ts) = ragent_core::team::TaskStore::open(&store.dir) {
                if let Ok(tl) = ts.read() {
                    tl.tasks
                        .iter()
                        .filter(|t| t.status == ragent_core::team::TaskStatus::Completed)
                        .map(|t| t.id.clone())
                        .collect()
                } else {
                    std::collections::HashSet::new()
                }
            } else {
                std::collections::HashSet::new()
            }
        } else {
            std::collections::HashSet::new()
        };

        let all_completed: std::collections::HashSet<String> = completed_task_ids
            .union(&task_completed_ids)
            .cloned()
            .collect();

        // Check each blocked member's dependencies
        let mut unblocked = Vec::new();
        for (member_name, agent_id) in &blocked_members {
            let task_id = member_name.strip_prefix("swarm-").unwrap_or(member_name);
            // Find the task's depends_on from decomposition
            let deps = decomp_tasks
                .iter()
                .find(|t| t.id == task_id)
                .map(|t| &t.depends_on);

            if let Some(deps) = deps {
                let missing: Vec<_> = deps
                    .iter()
                    .filter(|d| !all_completed.contains(*d))
                    .cloned()
                    .collect();
                tracing::debug!(
                    task = %task_id,
                    deps = ?deps,
                    missing = ?missing,
                    completed_ids = ?all_completed,
                    "Checking swarm dependency resolution"
                );
                if missing.is_empty() && !deps.is_empty() {
                    unblocked.push((member_name.clone(), agent_id.clone(), task_id.to_string()));
                } else if deps.is_empty() {
                    // No deps — should have been Spawning, but unblock anyway
                    unblocked.push((member_name.clone(), agent_id.clone(), task_id.to_string()));
                }
            }
        }

        if unblocked.is_empty() {
            return;
        }

        // Transition unblocked members from Blocked → Spawning
        for (member_name, agent_id, task_id) in &unblocked {
            // Update local state
            if let Some(m) = self
                .team_members
                .iter_mut()
                .find(|m| m.agent_id == *agent_id)
            {
                m.status = MemberStatus::Spawning;
            }
            // Update persisted config
            if let Ok(mut store) =
                ragent_core::team::TeamStore::load_by_name(&team_name, &working_dir)
            {
                if let Some(m) = store.config.member_by_id_mut(agent_id) {
                    m.status = MemberStatus::Spawning;
                }
                let _ = store.save();
            }
            // Log with actual deps for debugging
            let dep_info = decomp_tasks
                .iter()
                .find(|t| t.id == *task_id)
                .map(|t| t.depends_on.join(", "))
                .unwrap_or_default();
            self.push_log_no_agent(
                LogLevel::Info,
                format!(
                    "🔓 Unblocking {} ({}) — deps [{}] all in {:?}",
                    member_name, task_id, dep_info, all_completed
                ),
            );
        }

        // Trigger reconcile to spawn newly-unblocked members
        if let Some(manager) = self.session_processor.team_manager.get() {
            manager.clone().reconcile_spawning_members();
        }

        let remaining_blocked = blocked_members.len() - unblocked.len();
        if remaining_blocked > 0 {
            self.status = format!(
                "🐝 swarm: {} unblocked, {} still blocked",
                unblocked.len(),
                remaining_blocked
            );
        } else {
            self.status = format!("🐝 swarm: all teammates spawned");
        }
    }

    /// Check if the active swarm has completed all tasks and auto-summarise.
    ///
    /// Completion is detected in two ways:
    /// 1. All tasks in the TaskStore are Completed/Cancelled (normal path via `team_task_complete`)
    /// 2. All teammates are idle/failed/stopped — they finished their agent loop but may not have
    ///    called `team_task_complete`. In this case we auto-complete their tasks.
    pub fn poll_swarm_completion(&mut self) {
        let Some(ref swarm) = self.swarm_state else {
            return;
        };
        if swarm.completed || !swarm.spawned {
            return;
        }
        let team_name = swarm.team_name.clone();

        let working_dir = std::env::current_dir().unwrap_or_default();

        // Check member status — if all non-lead members are terminal (idle/failed/stopped),
        // the swarm is effectively done regardless of task store state.
        let members: Vec<_> = self
            .team_members
            .iter()
            .filter(|m| m.name != "lead" && !m.name.is_empty())
            .collect();
        let has_members = !members.is_empty();
        let all_members_terminal = has_members
            && members.iter().all(|m| {
                matches!(
                    m.status,
                    MemberStatus::Idle | MemberStatus::Failed | MemberStatus::Stopped
                )
            });

        // If all members are terminal, auto-complete any non-completed tasks in the task store
        if all_members_terminal {
            if let Ok(store) = ragent_core::team::TeamStore::load_by_name(&team_name, &working_dir)
            {
                if let Ok(ts) = ragent_core::team::TaskStore::open(&store.dir) {
                    if let Ok(tl) = ts.read() {
                        for task in &tl.tasks {
                            if task.status != ragent_core::team::TaskStatus::Completed
                                && task.status != ragent_core::team::TaskStatus::Cancelled
                            {
                                let agent_id = task.assigned_to.as_deref().unwrap_or("swarm");
                                if let Err(e) = ts.complete(&task.id, agent_id) {
                                    tracing::warn!(task = %task.id, error = %e, "failed to auto-complete swarm task");
                                }
                            }
                        }
                    }
                }
            }
        }

        // Now check task store for final tally
        let tasks = if let Ok(store) =
            ragent_core::team::TeamStore::load_by_name(&team_name, &working_dir)
        {
            if let Ok(ts) = ragent_core::team::TaskStore::open(&store.dir) {
                ts.read().ok()
            } else {
                None
            }
        } else {
            None
        };

        let Some(ref tl) = tasks else {
            // No task store — fall back to member-only check
            if all_members_terminal {
                self.finalize_swarm_completion(&team_name, 0, 0, 0);
            }
            return;
        };
        let total = tl.tasks.len();
        if total == 0 && all_members_terminal {
            self.finalize_swarm_completion(&team_name, 0, 0, 0);
            return;
        }
        if total == 0 {
            return;
        }

        let completed = tl
            .tasks
            .iter()
            .filter(|t| t.status == ragent_core::team::TaskStatus::Completed)
            .count();
        let cancelled = tl
            .tasks
            .iter()
            .filter(|t| t.status == ragent_core::team::TaskStatus::Cancelled)
            .count();
        let failed_members = members
            .iter()
            .filter(|m| m.status == MemberStatus::Failed)
            .count();

        if completed + cancelled >= total {
            self.finalize_swarm_completion(&team_name, total, completed, cancelled);
        } else if all_members_terminal {
            // Members done but tasks not all completed — report partial completion
            self.finalize_swarm_completion(
                &team_name,
                total,
                completed,
                cancelled + failed_members,
            );
        }
    }

    /// Build the swarm completion summary and mark the swarm as done.
    fn finalize_swarm_completion(
        &mut self,
        team_name: &str,
        total: usize,
        completed: usize,
        cancelled: usize,
    ) {
        let working_dir = std::env::current_dir().unwrap_or_default();

        let mut output = format!(
            "From: /swarm\n## 🎉 Swarm Complete: {team_name}\n\n\
            All **{total}** subtasks have finished ({completed} completed, {cancelled} failed/cancelled).\n\n"
        );

        // Include task table if we have tasks
        if total > 0 {
            if let Ok(store) = ragent_core::team::TeamStore::load_by_name(team_name, &working_dir) {
                if let Ok(ts) = ragent_core::team::TaskStore::open(&store.dir) {
                    if let Ok(tl) = ts.read() {
                        output.push_str("| ID | Title | Status |\n|----|-------|--------|\n");
                        for task in &tl.tasks {
                            let icon = match task.status {
                                ragent_core::team::TaskStatus::Completed => "✅",
                                ragent_core::team::TaskStatus::Cancelled => "❌",
                                _ => "⚠️",
                            };
                            output.push_str(&format!(
                                "| {} | {} | {} |\n",
                                task.id, task.title, icon
                            ));
                        }
                        output.push('\n');
                    }
                }
            }
        }

        output.push_str("Use `/swarm cancel` to clean up the ephemeral team.\n");

        self.append_assistant_text(&output);
        self.status = format!("🎉 swarm complete: {team_name}");
        self.push_log_no_agent(
            LogLevel::Info,
            format!("Swarm complete: {team_name} — {completed}/{total} tasks done"),
        );

        if let Some(ref mut s) = self.swarm_state {
            s.completed = true;
        }
    }

    fn is_current_session(&self, session_id: &str) -> bool {
        self.session_id.as_deref() == Some(session_id)
    }

    /// Count the total number of output lines produced by assistant messages.
    fn assistant_output_lines(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .map(|m| m.text_content().lines().count())
            .sum()
    }

    fn append_assistant_text(&mut self, text: &str) {
        let rendered = self.render_markdown_to_ascii(text);
        if !self.force_new_message {
            if let Some(last) = self.messages.last_mut()
                && last.role == Role::Assistant
            {
                // Only append to the last part if it is a Text part;
                // otherwise start a new Text part so text after tool calls
                // appears in the correct position.
                if let Some(MessagePart::Text { text: t }) = last.parts.last_mut() {
                    t.push_str(&rendered);
                } else {
                    last.parts.push(MessagePart::Text {
                        text: rendered.clone(),
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
                vec![MessagePart::Text { text: rendered }],
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

    /// Fire a pending autopilot continuation if the agent is idle.
    ///
    /// Called every render tick; sends the queued continuation text to the
    /// agent so it keeps working autonomously.
    pub fn poll_autopilot_continue(&mut self) {
        if !self.autopilot_enabled || self.is_processing {
            self.autopilot_pending_continue = None;
            return;
        }
        if let Some(text) = self.autopilot_pending_continue.take() {
            self.dispatch_user_message(text, vec![]);
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

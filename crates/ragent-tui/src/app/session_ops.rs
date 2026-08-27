//! Session, team, and miscellaneous operations for the TUI.
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ratatui::layout::Rect;

use ragent_agent::{
    event::Event,
    mcp::discovery::DiscoveredMcpServer,
    message::{Message, MessagePart, Role},
};
use ragent_team::team::TeamStore;

// Prompt optimization templates

// State types from app/state.rs
use crate::app::state::{
    App, ContextAction, FileMenuEntry, FileMenuState, LlmRequestStat, LlmStatsSummary, LogEntry,
    LogLevel, OutputViewState, OutputViewTarget, ProviderSetupStep, ScreenMode, ScrollbarDragPane,
    SelectionPane, TextSelection, atomic_config_update, is_image_path, percent_decode_path,
    save_clipboard_image_to_temp,
};

// Helpers
use crate::app::helpers::{MentionSpan, short_session_id};

// Re-export status types from theme
use crate::theme::{StatusCategory, StatusMessage};

impl App {
    /// Clone the current agent info, apply the selected model/thinking settings,
    /// and inject the role-mode system prompt addition when active.
    fn prepare_agent_for_dispatch(&self) -> ragent_agent::agent::AgentInfo {
        let mut agent = self.agent_info.clone();
        self.apply_selected_model_and_thinking(&mut agent);
        if let Some(ref mode) = self.role_mode {
            let addition = mode.system_prompt_addition();
            if !addition.is_empty() {
                let existing = agent.prompt.clone().unwrap_or_default();
                agent.prompt = Some(Arc::from(format!("{existing}\n\n{addition}")));
            }
        }
        agent
    }

    pub(crate) fn ollama_cloud_api_key(&self) -> Option<String> {
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

    pub(crate) fn should_auto_compact_before_send(&self) -> bool {
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

        // Use the shared estimator logic so the TUI pre-send check matches the
        // agent/server path. The threshold is a fraction of the model's context
        // window (default 70%), which makes the trigger independent of the
        // model's absolute context size.
        let threshold = ragent_agent::compaction::compaction_threshold(
            context_window,
            0,
            self.current_config().compaction.buffer,
            self.current_config().compaction.threshold,
        ) as u64;
        self.last_input_tokens > threshold
    }

    pub(crate) fn start_compaction(&mut self, auto_triggered: bool) -> bool {
        if self.session_id.is_none() {
            self.status = "⚠ No active session to compact".to_string();
            return false;
        }
        if self.messages.is_empty() {
            self.status = "⚠ No messages to compact".to_string();
            return false;
        }
        let sid = self.session_id.clone().unwrap_or_default();
        self.start_provider_compaction_for_session(&sid, auto_triggered)
    }

    /// Run a shell command (input started with `!`) and render its output in
    /// the chat panel, then dispatch the output to the model for review.
    ///
    /// The command is executed via `sh -c` in a spawned tokio task so the UI
    /// stays responsive. The command and its combined stdout/stderr are shown
    /// in the chat — first as a user message showing the command, then the
    /// output is published as an agent notice so the user can see what the
    /// command produced — and the model is asked to review the output for
    /// errors and resolve them as required.
    pub(crate) fn dispatch_bang_command(&mut self, raw: String) {
        // Strip the leading `!` and trim surrounding whitespace.
        let command = raw.strip_prefix('!').unwrap_or(&raw).trim().to_string();
        if command.is_empty() {
            self.status = "⚠ Empty shell command".to_string();
            return;
        }

        self.auto_compact_failed = false;
        let Some(sid) = self.session_id.clone() else {
            self.status = "⚠ No active session".to_string();
            return;
        };

        // Show the command in the chat as a user message.
        let display_text = format!("$ {command}");
        let msg = Message::user_text(&sid, &display_text);
        self.messages.push(msg);
        self.messages_version = self.messages_version.wrapping_add(1);
        self.add_to_history(raw.clone());
        self.input.clear();
        self.input_cursor = 0;
        self.file_menu = None;
        self.set_status_working("running command");
        self.stream_in_bytes = 0;
        self.stream_out_bytes = 0;
        // R-10: trim messages to bound memory.
        self.trim_messages_if_needed();

        self.push_log_no_agent(LogLevel::Info, format!("bang command: {command}"));

        let agent = self.prepare_agent_for_dispatch();
        let command_for_prompt = command.clone();
        let processor = self.session_processor.clone();
        let event_bus = self.event_bus.clone();
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(flag.clone());
        tokio::spawn(async move {
            // Run the command on a blocking thread to avoid stalling the
            // async runtime.
            let output = tokio::task::spawn_blocking(move || {
                std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&command)
                    .output()
            })
            .await;

            let combined = match output {
                Ok(Ok(out)) => {
                    ragent_agent::bang_command::combine_command_output(&out.stdout, &out.stderr)
                }
                Ok(Err(e)) => format!("failed to execute command: {e}"),
                Err(e) => format!("internal error: {e}"),
            };

            // Render the shell command output in the chat panel so the user
            // can see what the command produced before the model reviews it.
            event_bus.publish(Event::AgentNotice {
                session_id: sid.clone(),
                message: format!("```\n{combined}\n```"),
            });

            let prompt = ragent_agent::bang_command::build_bang_command_prompt(
                &command_for_prompt,
                &combined,
            );

            if let Err(e) = processor.process_message(&sid, &prompt, &agent, flag).await {
                tracing::debug!(error = %e, "Failed to process bang command output");
            }
        });
    }

    pub(crate) fn dispatch_user_message(
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
        self.messages_version = self.messages_version.wrapping_add(1);
        self.add_to_history(text.clone());
        self.input.clear();
        self.input_cursor = 0;
        self.file_menu = None;
        self.set_status_working("processing");
        self.stream_in_bytes = 0;
        self.stream_out_bytes = 0;
        // R-10: trim messages to bound memory.
        self.trim_messages_if_needed();

        let has_refs = !ragent_agent::reference::parse::parse_refs(&text).is_empty();
        if has_refs {
            let ref_names: Vec<String> = ragent_agent::reference::parse::parse_refs(&text)
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

        let agent = self.prepare_agent_for_dispatch();

        let processor = self.session_processor.clone();
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(flag.clone());
        tokio::spawn(async move {
            let final_text = if has_refs {
                let wd = std::env::current_dir().unwrap_or_default();
                match ragent_agent::reference::resolve::resolve_all_refs(&text, &wd).await {
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
                let mut parts: Vec<ragent_agent::message::MessagePart> = image_paths
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
                        ragent_agent::message::MessagePart::Image(Box::new(
                            ragent_agent::message::ImageData {
                                mime_type: mime.to_string(),
                                path: p,
                            },
                        ))
                    })
                    .collect();
                parts.push(ragent_agent::message::MessagePart::Text { text: final_text });
                let user_msg = ragent_agent::message::Message::new(
                    &sid,
                    ragent_agent::message::Role::User,
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

    /// Set the status line to an informational message and record it in history.
    #[allow(dead_code)]
    pub(crate) fn set_status_info(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.status = StatusCategory::Info.format(&msg);
        self.status_history.push(StatusMessage::info(msg));
        self.needs_redraw = true;
    }

    /// Set the status line to a success message and record it in history.
    #[allow(dead_code)]
    pub(crate) fn set_status_success(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.status = StatusCategory::Success.format(&msg);
        self.status_history.push(StatusMessage::success(msg));
        self.needs_redraw = true;
    }

    /// Set the status line to a warning message and record it in history.
    #[allow(dead_code)]
    pub(crate) fn set_status_warning(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.status = StatusCategory::Warning.format(&msg);
        self.status_history.push(StatusMessage::warning(msg));
        self.needs_redraw = true;
    }

    /// Set the status line to an error message and record it in history.
    #[allow(dead_code)]
    pub(crate) fn set_status_error(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.status = StatusCategory::Error.format(&msg);
        self.status_history.push(StatusMessage::error(msg));
        self.needs_redraw = true;
    }

    /// Set the status line to a working/in-progress message and record it in history.
    pub(crate) fn set_status_working(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.status = StatusCategory::Working.format(&msg);
        self.status_history.push(StatusMessage::working(msg));
        self.needs_redraw = true;
    }

    /// Return whether user input is currently blocked (agent is processing,
    /// compression is running, or a post-compact send is pending).
    pub(crate) fn is_input_blocked(&self) -> bool {
        self.is_processing
            || self.compact_in_progress
            || self.auto_compact_in_progress
            || self.pending_send_after_compact.is_some()
    }

    /// Return the number of Unicode code points currently in the input buffer.
    pub fn input_len_chars(&self) -> usize {
        self.input.chars().count()
    }

    pub(crate) fn assert_input_cursor_invariant(&self) {
        debug_assert!(self.input_cursor <= self.input_len_chars());
    }

    pub(crate) fn pane_area(&self, pane: SelectionPane) -> Rect {
        match pane {
            SelectionPane::Messages => self.message_area,
            SelectionPane::Log => self.log_area,
            SelectionPane::Profile => self.profile_area,
            SelectionPane::Tasks => self.tasks_area,
            SelectionPane::Memory => self.memory_area,
            SelectionPane::Telemetry => self.telemetry_area,
            SelectionPane::Input => self.input_area,
        }
    }

    /// Clear any text selection or context menu anchored on a pane whose
    /// layout area has collapsed to zero (e.g. a side panel that was just
    /// dismissed by a toggle such as Alt+T). The render pass zeroes the
    /// `*_area` of hidden panels, but the `text_selection`/`context_menu`
    /// state may still reference that pane — left stale, the next
    /// [`Self::assert_ui_invariants`] call would panic. This self-heals
    /// that state whenever input arrives.
    pub(crate) fn prune_stale_selection(&mut self) {
        if let Some(sel) = &self.text_selection {
            if self.pane_area(sel.pane).area() == 0 {
                self.text_selection = None;
            }
        }
        if let Some(menu) = &self.context_menu {
            if self.pane_area(menu.pane).area() == 0 {
                self.context_menu = None;
            }
        }
    }

    pub(crate) fn assert_ui_invariants(&self) {
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
    pub(crate) fn debug_log_input_transition(
        &self,
        source: &str,
        before_input: &str,
        before_cursor: usize,
    ) {
        #[cfg(debug_assertions)]
        {
            if before_input != self.input || before_cursor != self.input_cursor {
                tracing::debug!(
                    source = source,
                    before_chars = before_input.chars().count(),
                    before_cursor = before_cursor,
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

    pub(crate) fn set_cursor_char_index_clamped(&mut self, index: usize) {
        self.input_cursor = index.min(self.input_len_chars());
        self.assert_input_cursor_invariant();
    }

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

    pub(crate) fn get_parameter_hint(&self, trigger: &str) -> Option<String> {
        match trigger {
            "team" => Some("<subcommand>".to_string()),
            "memory" => Some("<subcommand> [<arg>]".to_string()),
            "agent" => Some("[<name>]".to_string()),
            "codeindex" => Some("[on|off|sync]".to_string()),
            "tools" => Some(
                "[show|help|office|github|gitlab|teams|agents|plan|codeindex] [on|off]".to_string(),
            ),
            "model" => Some("[show]".to_string()),
            "spec" => Some("[create|add|delete|list|search|validate|status|task|activate|deactivate|coverage|impl|jtbd|help]".to_string()),
            "router" => {
                Some("[on|off|status|tiers|weights|boundaries|test|stats|reload|help]".to_string())
            }
            "config" => Some("[show]".to_string()),
            "triggers" => Some("[list|enable|disable|remove|status|help]".to_string()),
            "websearch" => Some("[show|help]".to_string()),
            "init" => Some("[config]".to_string()),
            "thinking" => Some("[auto|off|low|medium|high]".to_string()),
            "theme" => Some("[toggle|light|dark]".to_string()),
            "mouse" => Some("[on|off]".to_string()),
            "status" => Some("[clear]".to_string()),
            "alog" => Some("[help|on|off|config|list|status|delete <run-id> --yes|export <run-id> --yes]".to_string()),
            "log" => Some("[clear subagents|panics|research|editlog|help]".to_string()),
            "help" => Some("[<command>]".to_string()),
            "quit" | "exit" => None,
            "clear" => None,
            "undo" => None,
            "redo" => None,
            "compact" | "compress" => None,
            "halt" => None,
            "resume" => None,
            _ => Some("<arg>".to_string()),
        }
    }

    pub(crate) fn refresh_project_files_cache(&mut self) {
        let wd = std::env::current_dir().unwrap_or_default();
        let files = ragent_agent::reference::fuzzy::collect_project_files(&wd, 10_000);
        self.project_files_cache_count = files.len();
        self.project_files_cache = Some(files);
        self.project_files_cache_cwd = Some(wd);
        self.project_files_cache_refreshed_at = Some(std::time::SystemTime::now());
    }

    /// Insert a single character at the cursor and refresh autocomplete menus.
    pub fn insert_char_at_cursor(&mut self, c: char) {
        let insert_pos = self.cursor_byte_pos();
        self.input.insert(insert_pos, c);
        self.cursor_move_right();
        self.refresh_input_menus();
        self.assert_input_cursor_invariant();
    }

    /// Insert a string at the cursor and refresh autocomplete menus.
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

    /// Delete the character before the cursor (backspace).
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

    /// Delete the character at the cursor (delete).
    pub fn delete_next_char(&mut self) {
        if self.input_cursor >= self.input_len_chars() {
            return;
        }
        let delete_pos = self.cursor_byte_pos();
        self.input.remove(delete_pos);
        self.refresh_input_menus();
        self.assert_input_cursor_invariant();
    }

    /// Remove the inclusive character range `[start, end)` from the input
    /// buffer, clamping to the current length. No-op when `start >= end`.
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

    pub(crate) fn input_selection_char_range(&self, sel: &TextSelection) -> Option<(usize, usize)> {
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

    pub(crate) fn active_input_widget_area(&self) -> Rect {
        self.input_area
    }

    /// Return the byte offset in `self.input` corresponding to the current
    /// cursor character index.
    pub(crate) fn cursor_byte_pos(&self) -> usize {
        self.cursor_byte_pos_at_char_index(self.input_cursor)
    }

    /// Return the byte offset in `self.input` for a given character index.
    /// `char_index == 0` short-circuits to `0`.
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

    pub(crate) fn cursor_move_left(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor -= 1;
        }
        self.assert_input_cursor_invariant();
    }

    pub(crate) fn cursor_move_right(&mut self) {
        if self.input_cursor < self.input_len_chars() {
            self.input_cursor += 1;
        }
        self.assert_input_cursor_invariant();
    }

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

    pub(crate) fn cursor_move_home(&mut self) {
        self.input_cursor = 0;
        self.assert_input_cursor_invariant();
    }

    pub(crate) fn cursor_move_end(&mut self) {
        self.input_cursor = self.input_len_chars();
        self.assert_input_cursor_invariant();
    }

    pub(crate) fn cursor_on_first_logical_line(&self) -> bool {
        let byte = self.cursor_byte_pos();
        !self.input[..byte].contains('\n')
    }

    pub(crate) fn cursor_on_last_logical_line(&self) -> bool {
        let byte = self.cursor_byte_pos();
        !self.input[byte..].contains('\n')
    }

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

    /// Return the `(start, end)` character range of the current keyboard
    /// selection, or `None` when the anchor and cursor coincide.
    pub(crate) fn kb_selection_char_range(&self) -> Option<(usize, usize)> {
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

    pub(crate) fn copy_kb_selection(&mut self) {
        if let Some((start, end)) = self.kb_selection_char_range() {
            let selected: String = self.input.chars().skip(start).take(end - start).collect();
            Self::set_clipboard(&selected);
        }
    }

    pub(crate) fn cut_kb_selection(&mut self) {
        if let Some((start, end)) = self.kb_selection_char_range() {
            let selected: String = self.input.chars().skip(start).take(end - start).collect();
            Self::set_clipboard(&selected);
            self.remove_input_char_range(start, end);
            self.kb_select_anchor = None;
        }
    }

    pub(crate) fn paste_text_from_clipboard(&mut self) {
        if let Some(text) = Self::get_clipboard() {
            self.handle_paste_text(&text);
        }
    }

    /// Insert pasted text at the cursor, stripping `\r` and replacing any
    /// active keyboard or mouse selection.
    pub fn handle_paste_text(&mut self, text: &str) {
        let clean: String = text.chars().filter(|&c| c != '\r').collect();
        if clean.is_empty() {
            return;
        }

        // Replace a mouse-driven selection first.
        if let Some(sel) = self.text_selection.clone() {
            if let Some((start, end)) = self.input_selection_char_range(&sel) {
                self.remove_input_char_range(start, end);
            }
            self.text_selection = None;
        }

        // Replace the keyboard-driven selection, if any.
        if let Some((start, end)) = self.kb_selection_char_range() {
            self.remove_input_char_range(start, end);
            self.kb_select_anchor = None;
        }

        self.insert_text_at_cursor(&clean);
    }

    pub(crate) fn clear_kb_selection(&mut self) {
        self.kb_select_anchor = None;
    }

    pub(crate) fn delete_prev_word(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let end = self.input_cursor;
        self.cursor_move_word_left();
        let start = self.input_cursor;
        self.remove_input_char_range(start, end);
    }

    pub(crate) fn delete_to_end_of_line(&mut self) {
        let end = self.input_len_chars();
        self.remove_input_char_range(self.input_cursor, end);
    }

    /// Set the active code index handle and trigger stats refresh.
    pub(crate) fn set_code_index(&mut self, code_index: Option<Arc<ragent_codeindex::CodeIndex>>) {
        self.code_index = code_index;
    }

    /// Refresh cached code-index stats (file/symbol counts and busy flag)
    /// on a throttled interval (1s while busy, 5s otherwise).
    pub(crate) fn refresh_code_index_stats(&mut self) {
        let interval = if self.code_index_busy {
            std::time::Duration::from_secs(1)
        } else {
            std::time::Duration::from_secs(5)
        };
        if self.code_index_stats_last_refresh.elapsed() < interval {
            return;
        }
        if let Some(ref idx) = self.code_index {
            // Check progress atomics (lock-free) to detect active reindex
            // even when locks are momentarily free between chunks.
            let (_done, total) = idx.reindex_progress();
            let reindex_active = total > 0;

            if let Some(stats) = idx.try_status() {
                self.code_index_stats_cache = Some(stats);
                self.code_index_stats_last_refresh = std::time::Instant::now();
                if self.code_index_busy && !reindex_active {
                    self.code_index_busy = false;
                    self.needs_redraw = true;
                }
            } else {
                // Locks busy — indexing in progress
                if !self.code_index_busy {
                    self.code_index_busy = true;
                    self.needs_redraw = true;
                }
            }
            // If reindex counters indicate active work, keep busy flag set.
            if reindex_active && !self.code_index_busy {
                self.code_index_busy = true;
                self.needs_redraw = true;
            }
        } else {
            self.code_index_stats_cache = None;
            self.code_index_stats_last_refresh = std::time::Instant::now();
            self.code_index_busy = false;
        }
    }

    /// Refresh cached structured-memory stats on a throttled 5s interval.
    pub(crate) fn refresh_memory_stats(&mut self) {
        if self.memory_stats_last_refresh.elapsed() < std::time::Duration::from_secs(5) {
            return;
        }
        self.memory_stats_last_refresh = std::time::Instant::now();
        let storage = self.storage.clone();
        let project_dir = std::env::current_dir().unwrap_or_default();
        let pending = self.memory_entry_count_pending.clone();
        tokio::task::spawn_blocking(move || {
            let count = storage
                .count_memories_for_project(&project_dir)
                .unwrap_or(0);
            pending.store(count, std::sync::atomic::Ordering::Relaxed);
        });
    }
    /// Map the primary session's short id to the current agent name for log display.
    pub(crate) fn register_primary_session_mapping(&mut self) {
        if let Some(ref sid) = self.session_id {
            let short_sid = short_session_id(sid);
            self.sid_to_display_name
                .insert(short_sid, self.agent_name.clone());
        }
    }

    /// Persist a discovered MCP server into `ragent.json`. Returns an error
    /// message when the server is already configured.
    pub(crate) fn enable_discovered_mcp_server(
        &self,
        server: &DiscoveredMcpServer,
    ) -> Result<String, String> {
        use ragent_agent::Config;

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
            .join(".ragent")
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

    pub(crate) fn ensure_session(&mut self) -> bool {
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

    /// Sync the in-memory `team_members` list with the on-disk team store,
    /// copying session ids, status, and current task ids so the UI reflects the
    /// authoritative persisted state. Also registers session_id → teammate
    /// name mappings for log display.
    ///
    /// **Note:** This method is no longer called from the render path
    /// (FR-009).  Team member state is kept in sync by event handlers
    /// (`TeammateSpawned`, `TeammateIdle`, `TeamTaskClaimed`, etc.).  The
    /// method is retained for explicit one-shot refreshes when a team is
    /// first opened.
    #[allow(dead_code)]
    pub(crate) fn refresh_team_member_session_ids(&mut self) {
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

    /// Load a session by id, replacing the current messages and updating the
    /// session_id mapping. Returns an error if the session cannot be found.
    pub fn load_session(&mut self, session_id: &str) -> anyhow::Result<()> {
        let session = self
            .storage
            .get_session(session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        let messages = self.storage.get_messages(session_id)?;
        let msg_count = messages.len();

        self.session_id = Some(session_id.to_string());
        // Cache the resumed session's creation timestamp for the teams panel
        // elapsed-time display (FR-009: avoids per-frame storage reads).
        self.lead_session_created_at = chrono::DateTime::parse_from_rfc3339(&session.created_at)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc));
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
                        ragent_agent::message::ToolCallStatus::Completed => "✓",
                        ragent_agent::message::ToolCallStatus::Error => "✗",
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

    pub(crate) fn detect_git_branch() -> Option<String> {
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

    pub(crate) fn current_config(&self) -> ragent_agent::Config {
        ragent_agent::Config::load().unwrap_or_default()
    }

    /// Compute an [`LlmStatsSummary`] (averages of elapsed time, prompt and
    /// output tokens/sec) for samples belonging to the currently active model.
    /// Returns `None` when no samples are recorded for that model.
    pub(crate) fn llm_stats_summary(&self) -> Option<LlmStatsSummary> {
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

    pub(crate) fn tokens_per_second(tokens: u64, elapsed_ms: u64) -> f64 {
        if elapsed_ms == 0 {
            return 0.0;
        }
        tokens as f64 / (elapsed_ms as f64 / 1000.0)
    }

    /// Build the usage display string (provider quota % or token-rate info)
    /// and a flag indicating whether the value is a rate limit.
    pub fn usage_display(&self) -> (String, bool) {
        let provider_id = self
            .configured_provider
            .as_ref()
            .map(|p| p.id.as_str())
            .unwrap_or("");

        // Provider rate-limit quota % takes priority when available.
        if let Some(quota) = self.quota_percent {
            let label = if provider_id == "copilot" {
                let plan = ragent_agent::provider::copilot::cached_copilot_plan()
                    .unwrap_or_else(|| "Copilot".to_string());
                format!("{} quota: {:.1}%", plan, quota)
            } else {
                format!("quota: {:.1}%", quota)
            };
            return (label, false);
        }

        let ctx_detail = self.context_window_display();
        let ctx_label = |prefix: &str| -> String {
            match ctx_detail.as_deref() {
                Some(detail) if prefix.is_empty() => format!("ctx: {detail}"),
                Some(detail) => format!("{prefix} ctx: {detail}"),
                None => prefix.to_string(),
            }
        };

        if provider_id == "copilot" {
            let plan = ragent_agent::provider::copilot::cached_copilot_plan()
                .unwrap_or_else(|| "Copilot".to_string());
            (ctx_label(&plan), false)
        } else if provider_id == "ollama" || provider_id == "ollama_cloud" {
            let label = ctx_label("");
            if label.is_empty() {
                (
                    if provider_id == "ollama" {
                        "local"
                    } else {
                        "ollama"
                    }
                    .to_string(),
                    false,
                )
            } else {
                (label, false)
            }
        } else {
            let label = ctx_label("");
            if label.is_empty() {
                ("unknown".to_string(), true)
            } else {
                (label, false)
            }
        }
    }

    /// Build the `"pct usedK/contextK"` context-window usage display, or
    /// `None` when no model context window is configured.
    pub(crate) fn context_window_display(&self) -> Option<String> {
        let ctx_window = self.selected_model_context_window()?;
        let pct = (self.last_input_tokens as f32 / ctx_window as f32 * 100.0).min(100.0);
        Some(format!(
            "{pct:.0}% {}K/{}K",
            self.last_input_tokens / 1000,
            ctx_window / 1000
        ))
    }

    /// Refresh the file-mention autocomplete menu based on the active mention
    /// span. Populates directory listings when navigating into a directory,
    /// otherwise fuzzy-matches against the cached project file list.
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
            let matches = ragent_agent::reference::fuzzy::fuzzy_match(&query, candidates);

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

    /// Accept the currently-selected file-menu entry: navigates into a
    /// directory or inserts the chosen file path into the input buffer.
    /// Returns `false` when the menu is empty or no selection can be made.
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

    pub(crate) fn mention_spans(&self) -> Vec<MentionSpan> {
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

    pub(crate) fn active_mention_span(&self) -> Option<MentionSpan> {
        let cursor = self.cursor_byte_pos();
        let spans = self.mention_spans();
        spans
            .iter()
            .find(|span| cursor >= span.at_start && cursor <= span.token_end)
            .copied()
    }

    /// Map a screen coordinate to the side-panel pane it lands in.
    ///
    /// Returns the [`SelectionPane`] (Messages / Profile / Log / Todo /
    /// Memory / Input) whose cached area contains `(col, row)`, or `None`
    /// when the coordinate is outside every active pane. Side-panel panes
    /// (Profile / Log / Todo / Memory) are only reported when their
    /// corresponding `show_*` flag is true, so hidden panels never win
    /// hit-testing even if their cached rect is stale. This is the single
    /// mouse hit-testing entry point used by `handle_mouse_event` for
    /// left-click selection start, right-click context-menu open, and
    /// scrollbar-gutter detection (FR-013).
    pub fn pane_at(&self, col: u16, row: u16) -> Option<SelectionPane> {
        let pos = (col, row).into();
        if self.message_area.area() > 0 && self.message_area.contains(pos) {
            Some(SelectionPane::Messages)
        } else if self.show_profile
            && self.profile_area.area() > 0
            && self.profile_area.contains(pos)
        {
            Some(SelectionPane::Profile)
        } else if self.show_log && self.log_area.area() > 0 && self.log_area.contains(pos) {
            Some(SelectionPane::Log)
        } else if self.show_tasks_panel
            && self.tasks_area.area() > 0
            && self.tasks_area.contains(pos)
        {
            Some(SelectionPane::Tasks)
        } else if self.show_memory && self.memory_area.area() > 0 && self.memory_area.contains(pos)
        {
            Some(SelectionPane::Memory)
        } else if self.show_telemetry
            && self.telemetry_area.area() > 0
            && self.telemetry_area.contains(pos)
        {
            Some(SelectionPane::Telemetry)
        } else if self.input_area.area() > 0 && self.input_area.contains(pos) {
            Some(SelectionPane::Input)
        } else {
            None
        }
    }

    /// Extract a text selection spanning `[start_col, start_row]` to
    /// `[end_col, end_row]` from `lines`, which are positioned at the inner
    /// origin `(inner_x, inner_y)`. Joins multi-line selections with `\n`.
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

    pub(crate) fn set_clipboard(text: &str) {
        crate::clipboard::set_clipboard_text(text);
    }

    pub(crate) fn get_clipboard() -> Option<String> {
        crate::clipboard::get_clipboard_text()
    }

    /// Paste an image (or image file path) from the clipboard into the pending
    /// attachments. Checks the text clipboard first for a `file://` URI or
    /// path, then falls back to raw pixel data which is saved to
    /// `target/temp/` with restrictive permissions.
    pub(crate) fn paste_image_from_clipboard(&mut self) {
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
                    self.warn_if_path_outside_safe_scope(&path);
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
        if let Some(img_data) = crate::clipboard::get_clipboard_image() {
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

    /// Log a warning when a clipboard-resolved image path lies outside the
    /// current working directory or the user's home directory. The file is
    /// still attached (the user may intentionally want a screenshot or asset
    /// from elsewhere), but the warning provides a visible security nudge.
    fn warn_if_path_outside_safe_scope(&mut self, path: &std::path::Path) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let home = dirs::home_dir().unwrap_or_default();
        let inside_cwd = path.strip_prefix(&cwd).is_ok();
        let inside_home = path.strip_prefix(&home).is_ok();
        if !inside_cwd && !inside_home {
            tracing::warn!(
                path = %path.display(),
                cwd = %cwd.display(),
                "clipboard image path is outside the working directory and home directory"
            );
            self.push_log_no_agent(
                LogLevel::Warn,
                format!(
                    "⚠ Clipboard image path is outside the working directory and home: {}. \
                     Attaching anyway.",
                    path.display()
                ),
            );
        }
    }

    /// Execute a context-menu action (copy/cut/paste/etc.) against the current
    /// text selection, dismissing the menu afterwards.
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
                        | Some(ProviderSetupStep::GitLabSetup { .. })
                        | Some(ProviderSetupStep::TelemetrySetup { .. })
                ) {
                    self.paste_provider_setup_from_clipboard();
                } else if matches!(pane, Some(SelectionPane::Input)) {
                    if let Some(text) = Self::get_clipboard() {
                        self.handle_paste_text(&text);
                    }
                }
            }
        }
    }

    pub(crate) fn apply_scrollbar_drag(&mut self, mouse_y: u16, pane: ScrollbarDragPane) {
        let (area, max_scroll) = match pane {
            ScrollbarDragPane::Messages => (self.message_area, self.message_max_scroll),
            ScrollbarDragPane::Log => (self.log_area, self.log_max_scroll),
            ScrollbarDragPane::Profile => (self.profile_area, self.profile_max_scroll),
            ScrollbarDragPane::Tasks => (self.tasks_area, self.tasks_max_scroll),
            ScrollbarDragPane::Memory => (self.memory_area, self.memory_max_scroll),
            ScrollbarDragPane::Telemetry => (self.telemetry_area, self.telemetry_max_scroll),
        };
        if area.height <= 1 || max_scroll == 0 {
            return;
        }

        // Clamp mouse_y to the pane area
        let y = mouse_y.clamp(area.y, area.bottom().saturating_sub(1));
        let relative = y.saturating_sub(area.y) as f32;
        let track_height = (area.height.saturating_sub(1)) as f32;
        let fraction = (relative / track_height).clamp(0.0, 1.0);

        // fraction 0.0 = top of scrollbar track, 1.0 = bottom.
        // Messages, Log, and Profile use "lines from bottom" semantics
        // (scroll_offset = 0 → bottom of content, max_scroll → top), so
        // dragging to the top of the track must produce offset = max_scroll:
        // offset = (1.0 - fraction) * max_scroll.
        // Tasks, Memory, and Telemetry use "lines from top" semantics
        // (scroll_offset = 0 → top of content, max_scroll → bottom), so
        // dragging to the top must produce offset = 0 and dragging to the
        // bottom must produce offset = max_scroll: offset = fraction *
        // max_scroll.  Using a single formula for all panels inverts the
        // drag direction for the "lines from top" family, which is the
        // erratic-scrollbar bug.
        let offset = match pane {
            ScrollbarDragPane::Messages | ScrollbarDragPane::Log | ScrollbarDragPane::Profile => {
                ((1.0 - fraction) * max_scroll as f32).round() as u16
            }
            ScrollbarDragPane::Tasks | ScrollbarDragPane::Memory | ScrollbarDragPane::Telemetry => {
                (fraction * max_scroll as f32).round() as u16
            }
        };

        match pane {
            ScrollbarDragPane::Messages => self.scroll_offset = offset.min(max_scroll),
            ScrollbarDragPane::Log => self.log_scroll_offset = offset.min(max_scroll),
            ScrollbarDragPane::Profile => self.profile_scroll_offset = offset.min(max_scroll),
            ScrollbarDragPane::Tasks => self.tasks_scroll_offset = offset.min(max_scroll),
            ScrollbarDragPane::Memory => self.memory_scroll_offset = offset.min(max_scroll),
            ScrollbarDragPane::Telemetry => self.telemetry_scroll_offset = offset.min(max_scroll),
        }
    }

    pub(crate) fn execute_plan_restore(&mut self, session_id: &str, summary: &str) {
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

    /// Push a log entry at the given level, tagging it with an optional agent id
    /// and the current session id. When the log panel is visible, the entry is
    /// also appended to the log-window spool file under the project's `log/`
    /// directory.
    pub(crate) fn push_log(&mut self, level: LogLevel, message: String, agent_id: Option<String>) {
        let entry = LogEntry {
            timestamp: chrono::Utc::now(),
            level,
            message: message.clone(),
            session_id: self.session_id.clone(),
            agent_id,
        };
        self.log_entries.push(entry);
        self.log_version = self.log_version.wrapping_add(1);
        // R-11: Cap log entries with FIFO eviction so the Vec (and its
        // mirror `log_line_cache`) do not grow without bound over a long
        // session.
        self.trim_log_entries_if_needed();
        if self.show_log {
            if let Some(ref path) = self.log_window_path {
                self.append_log_entry_to_spool(path, level, &message);
            }
        }
    }

    /// R-10: Trim `messages` and `message_line_cache` to `MAX_TUI_MESSAGES`
    /// using FIFO eviction so long sessions do not accumulate every message
    /// (with all `MessagePart`s up to 12 KB each) without bound.
    pub(crate) fn trim_messages_if_needed(&mut self) {
        const MAX_TUI_MESSAGES: usize = 500;
        if self.messages.len() > MAX_TUI_MESSAGES {
            let drop_count = self.messages.len() - MAX_TUI_MESSAGES;
            self.messages.drain(0..drop_count);
            // Trim the line cache to match.
            if self.message_line_cache.len() > MAX_TUI_MESSAGES {
                let drop_cache = self.message_line_cache.len() - MAX_TUI_MESSAGES;
                self.message_line_cache.drain(0..drop_cache);
            }
            self.messages_version = self.messages_version.wrapping_add(1);
        }
    }

    /// R-11: Trim `log_entries` and `log_line_cache` to `MAX_LOG_ENTRIES`
    /// using FIFO eviction.
    pub(crate) fn trim_log_entries_if_needed(&mut self) {
        const MAX_LOG_ENTRIES: usize = 1000;
        if self.log_entries.len() > MAX_LOG_ENTRIES {
            let drop_count = self.log_entries.len() - MAX_LOG_ENTRIES;
            self.log_entries.drain(0..drop_count);
            if self.log_line_cache.len() > MAX_LOG_ENTRIES {
                let drop_cache = self.log_line_cache.len() - MAX_LOG_ENTRIES;
                self.log_line_cache.drain(0..drop_cache);
            }
        }
    }

    /// Append a single formatted log line to the log-window spool file.
    fn append_log_entry_to_spool(&self, path: &std::path::Path, level: LogLevel, message: &str) {
        use std::io::Write;
        let level_str = match level {
            LogLevel::Info => "INF",
            LogLevel::Tool => "TUL",
            LogLevel::Warn => "WRN",
            LogLevel::Error => "ERR",
        };
        let ts = chrono::Utc::now().to_rfc3339();
        let line = format!("{ts} {level_str} {message}\n");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = file.write_all(line.as_bytes());
        }
    }

    /// Flush every current log entry to the log-window spool file. Called when
    /// the log panel is toggled on so the file contains the full history that
    /// is currently visible in the panel.
    pub(crate) fn spool_log_window_history(&mut self) {
        if !self.show_log {
            return;
        }
        let Some(ref path) = self.log_window_path else {
            return;
        };
        for entry in &self.log_entries {
            self.append_log_entry_to_spool(path, entry.level, &entry.message);
        }
    }

    /// Convenience wrapper for [`push_log`](Self::push_log) with no agent id.
    pub(crate) fn push_log_no_agent(&mut self, level: LogLevel, message: String) {
        self.push_log(level, message, None);
    }

    pub(crate) fn open_output_view_session(&mut self, session_id: String, label: String) {
        self.selected_agent_session_id = Some(session_id.clone());
        self.output_view = Some(OutputViewState {
            target: OutputViewTarget::Session { session_id, label },
            scroll_offset: 0,
            max_scroll: 0,
        });
    }

    pub(crate) fn assistant_output_lines(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .map(|m| m.text_content().lines().count())
            .sum()
    }
}

//! Keyboard input handling for the TUI.
//!
//! Maps terminal key events to high-level [`InputAction`]s, handling both
//! normal editing mode and the permission dialog intercept.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ragent_types::ThinkingLevel;

use crate::app::{
    App, ConfiguredProvider, ContextAction, ModelPickerEntry, PROVIDER_LIST, ProviderSetupStep,
    ProviderSource,
};
use crate::utils::is_ollama_family;
use ragent_llm::providers::router_config::Tier;

fn cursor_byte_pos(s: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    let len_chars = s.chars().count();
    if char_index >= len_chars {
        return s.len();
    }
    s.char_indices()
        .nth(char_index)
        .map(|(byte, _)| byte)
        .unwrap_or(s.len())
}

/// A high-level action produced by interpreting a key event.
#[derive(Debug)]
pub enum InputAction {
    /// Submit the input buffer as a user message.
    SendMessage(String),
    /// Run a shell command (input started with `!`) and ask the model to
    /// review the output for errors.
    BangCommand(String),
    /// Exit the application.
    Quit,
    /// Confirm guarded keyboard quit (Ctrl+D after Ctrl+C).
    ConfirmQuit,
    /// Scroll the message view upward.
    ScrollUp,
    /// Scroll the message view downward.
    ScrollDown,
    /// Scroll the log panel upward.
    LogScrollUp,
    /// Scroll the log panel downward.
    LogScrollDown,
    /// Scroll active output view upward.
    OutputViewPageUp,
    /// Scroll active output view downward.
    OutputViewPageDown,
    /// Jump active output view to the start.
    OutputViewToStart,
    /// Jump active output view to the end.
    OutputViewToEnd,
    /// Recall the previous entry from input history.
    HistoryUp,
    /// Recall the next entry from input history.
    HistoryDown,
    /// Move the cursor left within the input line.
    MoveCursorLeft,
    /// Move the cursor right within the input line.
    MoveCursorRight,
    /// Move the cursor to the start of the input line.
    MoveCursorHome,
    /// Move the cursor to the end of the input line.
    MoveCursorEnd,
    /// Move the cursor one word left.
    MoveCursorWordLeft,
    /// Move the cursor one word right.
    MoveCursorWordRight,
    /// Delete the character under the cursor.
    Delete,
    /// Delete the previous word.
    DeletePrevWord,
    /// Delete from cursor to end of line.
    DeleteToLineEnd,
    /// Clear entire line.
    ClearLine,
    /// Cycle to the next configured agent.
    SwitchAgent,
    /// Execute a `/`-prefixed command.
    SlashCommand(String),
    /// Cancel the currently running agent (Esc or Ctrl+X while processing).
    CancelAgent,
    /// Confirm a pending forcecleanup modal (Enter -> confirm).
    ConfirmForceCleanup,
    /// Cancel a pending forcecleanup modal (Esc -> cancel).
    CancelForceCleanup,
    /// Confirm the router save confirmation modal (Enter -> save).
    ConfirmRouterSave,
    /// Cancel the router save confirmation modal (Esc -> cancel).
    CancelRouterSave,
    /// Confirm the plan approval dialog (Enter when cursor_approve = true).
    ApprovePlan,
    /// Reject the plan approval dialog (Enter when cursor_approve = false, or `r`/Esc).
    RejectPlan,
    /// Toggle the plan approval dialog cursor left/right (←/→ arrow keys).
    TogglePlanCursor,
    /// Cycle focus to the next teammate (Alt+Down).
    FocusNextTeammate,
    /// Cycle focus to the previous teammate (Alt+Up).
    FocusPrevTeammate,
    /// Insert a literal newline at cursor (Shift+Enter — multiline input).
    InsertNewline,
    /// Select all input text (Ctrl+A).
    SelectAll,
    /// Extend keyboard selection one character to the left (Shift+Left).
    SelectCharLeft,
    /// Extend keyboard selection one character to the right (Shift+Right).
    SelectCharRight,
    /// Extend keyboard selection one word to the left (Ctrl+Shift+Left).
    SelectWordLeft,
    /// Extend keyboard selection one word to the right (Ctrl+Shift+Right).
    SelectWordRight,
    /// Copy the active keyboard selection to the clipboard (Ctrl+C when selection active).
    CopyToClipboard,
    /// Cut the active keyboard selection to the clipboard (Ctrl+X).
    CutToClipboard,
    /// Paste text from the clipboard at the cursor (Ctrl+V).
    PasteFromClipboard,
    /// Toggle the log panel visibility (Alt+L).
    ToggleLog,
    /// Toggle the profiler panel visibility and profiler state (Alt+P).
    ToggleProfile,
    /// Toggle the Tasks panel visibility (Alt+T).
    ToggleTasksPanel,
    /// Toggle the Memory side panel visibility (Alt+M).
    ToggleMemory,
    /// Move the Memory panel cursor up.
    MemoryCursorUp,
    /// Move the Memory panel cursor down.
    MemoryCursorDown,
    /// Open the selected memory in a full overlay.
    OpenMemoryView,
    /// Prompt to delete the selected memory (shows confirmation).
    PromptMemoryDelete,
    /// Confirm the pending memory delete (Enter on confirmation dialog).
    ConfirmMemoryDelete,
    /// Cancel the pending memory delete (Esc on confirmation dialog).
    CancelMemoryDelete,
    /// Scroll the memory view overlay upward.
    MemoryViewPageUp,
    /// Scroll the memory view overlay downward.
    MemoryViewPageDown,
    /// Jump the memory view overlay to the start.
    MemoryViewToStart,
    /// Jump the memory view overlay to the end.
    MemoryViewToEnd,
    /// Scroll the memory view overlay up by one line.
    MemoryViewLineUp,
    /// Scroll the memory view overlay down by one line.
    MemoryViewLineDown,
    /// Toggle the Telemetry side panel visibility (Alt+O).
    ToggleTelemetry,
    /// Toggle the Context side panel visibility (Alt+X).
    ToggleContextPanel,
    /// Toggle YOLO mode (Alt+Y).
    ToggleYolo,
    /// Toggle edit-operation logging (Alt+E).
    ToggleEditLog,
    /// Scroll the research markdown viewer up.
    ResearchViewPageUp,
    /// Scroll the research markdown viewer down.
    ResearchViewPageDown,
    /// Jump the research markdown viewer to the start.
    ResearchViewToStart,
    /// Jump the research markdown viewer to the end.
    ResearchViewToEnd,
    /// Scroll the research markdown viewer up by one line.
    ResearchViewLineUp,
    /// Scroll the research markdown viewer down by one line.
    ResearchViewLineDown,
}

/// Translate a [`KeyEvent`] into an optional [`InputAction`].
///
/// When a permission dialog is active, only `y` / `a` / `n` keys are handled
/// (publishing a permission reply). When the provider setup dialog is active,
/// keys are routed to the dialog. When the slash-command menu is active,
/// arrow keys navigate and Enter selects. Otherwise normal editing and
/// navigation keys are processed.
///
/// # Examples
///
/// ```rust,no_run
/// # use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// # use ragent_tui::App;
/// # use ragent_tui::input::handle_key;
/// # fn example(app: &mut App) {
/// let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
/// if let Some(action) = handle_key(app, key) {
///     println!("Action: {action:?}");
/// }
/// # }
/// ```
pub fn handle_key(app: &mut App, key: KeyEvent) -> Option<InputAction> {
    if matches!(key.kind, KeyEventKind::Release) {
        return None;
    }

    // Always check for quit commands first, before any modal interception.
    // This ensures Ctrl+C (arm quit) and Ctrl+D (confirm quit) work globally.
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        // Copy if a keyboard selection is active; otherwise arm quit.
        if app.kb_select_anchor.is_some() {
            return Some(InputAction::CopyToClipboard);
        }
        return Some(InputAction::Quit);
    }
    if key.code == KeyCode::Char('d') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(InputAction::ConfirmQuit);
    }

    // If a memory delete confirmation modal is active, intercept Enter/Esc
    // before any other dialog so the confirmation always takes precedence.
    if app.pending_memory_delete.is_some() {
        match key.code {
            KeyCode::Enter => return Some(InputAction::ConfirmMemoryDelete),
            KeyCode::Esc => return Some(InputAction::CancelMemoryDelete),
            _ => return None,
        }
    }

    // If a router save confirmation modal is active, intercept Enter/Esc
    // before any other dialog so the confirmation always takes precedence.
    if app.pending_router_save.is_some() {
        match key.code {
            KeyCode::Enter => return Some(InputAction::ConfirmRouterSave),
            KeyCode::Esc => return Some(InputAction::CancelRouterSave),
            _ => return None,
        }
    }

    // If context menu is active, route all keys there.
    if app.context_menu.is_some() {
        handle_context_menu_key(app, key);
        return None;
    }

    // If shortcuts panel is active, only Esc or '?' dismiss it.
    if app.show_shortcuts {
        if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
            app.show_shortcuts = false;
        }
        return None;
    }

    // If provider setup dialog is active, route all keys there.
    if app.provider_setup.is_some() {
        handle_provider_setup_key(app, key);
        return None;
    }

    // If MCP discover dialog is active, route all keys there
    if app.mcp_discover.is_some() {
        handle_mcp_discover_key(app, key);
        return None;
    }

    // If question dialog is active, intercept keys.
    if !app.question_queue.is_empty() {
        let has_options = app
            .question_queue
            .front()
            .map(|r| !r.options.is_empty())
            .unwrap_or(false);

        if has_options {
            return match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if app.question_selected_index > 0 {
                        app.question_selected_index -= 1;
                    }
                    None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(req) = app.question_queue.front() {
                        if app.question_selected_index + 1 < req.options.len() {
                            app.question_selected_index += 1;
                        }
                    }
                    None
                }
                KeyCode::Enter => {
                    if let Some(req) = app.question_queue.front().cloned() {
                        let response = req
                            .options
                            .get(app.question_selected_index)
                            .cloned()
                            .unwrap_or_default();
                        app.event_bus
                            .publish(ragent_agent::event::Event::QuestionAnswered {
                                session_id: req.session_id.clone(),
                                request_id: req.id.clone(),
                                response,
                            });
                        app.question_queue.pop_front();
                        app.question_selected_index = 0;
                    }
                    None
                }
                KeyCode::Esc => {
                    if let Some(req) = app.question_queue.front().cloned() {
                        app.event_bus
                            .publish(ragent_agent::event::Event::QuestionAnswered {
                                session_id: req.session_id.clone(),
                                request_id: req.id.clone(),
                                response: "[User dismissed question]".to_string(),
                            });
                    }
                    app.question_queue.pop_front();
                    app.question_selected_index = 0;
                    None
                }
                _ => None,
            };
        }

        // Free-text question: accept typed input, submit on Enter, cancel on Esc.
        return match key.code {
            KeyCode::Enter => {
                if let Some(req) = app.question_queue.front().cloned() {
                    let response = app.pending_question_input.trim().to_string();
                    if !response.is_empty() {
                        app.event_bus
                            .publish(ragent_agent::event::Event::QuestionAnswered {
                                session_id: req.session_id.clone(),
                                request_id: req.id.clone(),
                                response,
                            });
                        app.question_queue.pop_front();
                        app.pending_question_input.clear();
                    }
                }
                None
            }
            KeyCode::Esc => {
                if let Some(req) = app.question_queue.front().cloned() {
                    app.event_bus
                        .publish(ragent_agent::event::Event::QuestionAnswered {
                            session_id: req.session_id.clone(),
                            request_id: req.id.clone(),
                            response: "[User dismissed question]".to_string(),
                        });
                }
                app.question_queue.pop_front();
                app.pending_question_input.clear();
                None
            }
            KeyCode::Backspace => {
                app.pending_question_input.pop();
                None
            }
            KeyCode::Char(c) => {
                app.pending_question_input.push(c);
                None
            }
            _ => None,
        };
    }

    // If permission dialog is active, intercept keys
    if !app.permission_queue.is_empty() {
        // Standard permission dialog: y/a/n only.
        return match key.code {
            KeyCode::Char('y') => {
                if let Some(ref req) = app.permission_queue.front() {
                    tracing::info!(
                        session_id = %req.session_id,
                        request_id = %req.id,
                        "User pressed 'y' to grant permission"
                    );
                    app.event_bus
                        .publish(ragent_agent::event::Event::PermissionReplied {
                            session_id: req.session_id.clone(),
                            request_id: req.id.clone(),
                            allowed: true,
                            decision: ragent_agent::permission::PermissionDecision::Once,
                        });
                }
                None
            }
            KeyCode::Char('a') => {
                if let Some(ref req) = app.permission_queue.front() {
                    tracing::info!(
                        session_id = %req.session_id,
                        request_id = %req.id,
                        "User pressed 'a' to grant permission (always)"
                    );
                    app.event_bus
                        .publish(ragent_agent::event::Event::PermissionReplied {
                            session_id: req.session_id.clone(),
                            request_id: req.id.clone(),
                            allowed: true,
                            decision: ragent_agent::permission::PermissionDecision::Always,
                        });
                }
                None
            }
            KeyCode::Char('n') => {
                if let Some(ref req) = app.permission_queue.front() {
                    tracing::info!(
                        session_id = %req.session_id,
                        request_id = %req.id,
                        "User pressed 'n' to deny permission"
                    );
                    app.event_bus
                        .publish(ragent_agent::event::Event::PermissionReplied {
                            session_id: req.session_id.clone(),
                            request_id: req.id.clone(),
                            allowed: false,
                            decision: ragent_agent::permission::PermissionDecision::Deny,
                        });
                }
                None
            }
            _ => None,
        };
    }

    // If a router save confirmation modal is active, intercept Enter/Esc
    if app.pending_router_save.is_some() {
        match key.code {
            KeyCode::Enter => return Some(InputAction::ConfirmRouterSave),
            KeyCode::Esc => return Some(InputAction::CancelRouterSave),
            _ => return None,
        }
    }

    // If a forcecleanup confirmation modal is active, intercept Enter/Esc
    if app.pending_forcecleanup.is_some() {
        match key.code {
            KeyCode::Enter => return Some(InputAction::ConfirmForceCleanup),
            KeyCode::Esc => return Some(InputAction::CancelForceCleanup),
            _ => return None,
        }
    }

    // If a plan approval dialog is active, intercept keys
    if let Some(ref state) = app.plan_approval_pending {
        let cursor_approve = state.cursor_approve;
        match key.code {
            KeyCode::Enter => {
                if cursor_approve {
                    return Some(InputAction::ApprovePlan);
                } else {
                    return Some(InputAction::RejectPlan);
                }
            }
            KeyCode::Left | KeyCode::Right => return Some(InputAction::TogglePlanCursor),
            KeyCode::Char('r') | KeyCode::Esc => return Some(InputAction::RejectPlan),
            _ => return None,
        }
    }

    // If slash menu is active, intercept navigation keys
    if app.slash_menu.is_some() {
        match key.code {
            KeyCode::Up => {
                if let Some(ref mut menu) = app.slash_menu {
                    if !menu.matches.is_empty() {
                        menu.selected = if menu.selected == 0 {
                            menu.matches.len() - 1
                        } else {
                            menu.selected - 1
                        };
                    }
                }
                return None;
            }
            KeyCode::Down => {
                if let Some(ref mut menu) = app.slash_menu {
                    if !menu.matches.is_empty() {
                        menu.selected = (menu.selected + 1) % menu.matches.len();
                    }
                }
                return None;
            }
            KeyCode::Enter => {
                // Select the highlighted command, or use the typed text.
                // If the user typed more than just the trigger, preserve the full
                // input so subcommands and arguments are not lost.
                let command = {
                    let menu = app.slash_menu.as_ref()?;
                    let raw = app.input.trim_end().to_string();
                    if let Some(entry) = menu.matches.get(menu.selected) {
                        let with_slash = format!("/{}", entry.trigger);
                        // Raw input extends beyond the matched trigger with a space → use raw
                        if raw.starts_with(&with_slash)
                            && raw.len() > with_slash.len()
                            && raw.as_bytes().get(with_slash.len()) == Some(&b' ')
                        {
                            raw
                        } else {
                            format!("/{}", entry.trigger)
                        }
                    } else {
                        menu.filter.clone()
                    }
                };
                return Some(InputAction::SlashCommand(command));
            }
            KeyCode::Esc => {
                app.slash_menu = None;
                app.set_cursor_char_index_clamped(app.input_len_chars());
                return None;
            }
            KeyCode::Char(c) => {
                app.insert_char_at_cursor(c);
                return None;
            }
            KeyCode::Backspace => {
                app.delete_prev_char();
                return None;
            }
            _ => return None,
        }
    }

    // If file menu is active, intercept navigation keys while still allowing
    // normal in-line editing and cursor motion.
    if app.file_menu.is_some() {
        match key.code {
            KeyCode::Up => {
                if let Some(ref mut menu) = app.file_menu
                    && !menu.matches.is_empty()
                {
                    menu.selected = if menu.selected == 0 {
                        menu.matches.len() - 1
                    } else {
                        menu.selected - 1
                    };
                    const FILE_MENU_VISIBLE_ROWS: usize = 8;
                    if menu.selected < menu.scroll_offset {
                        menu.scroll_offset = menu.selected;
                    } else if menu.selected + 1 < FILE_MENU_VISIBLE_ROWS {
                        menu.scroll_offset = 0;
                    }
                }
                return None;
            }
            KeyCode::Down => {
                if let Some(ref mut menu) = app.file_menu
                    && !menu.matches.is_empty()
                {
                    menu.selected = (menu.selected + 1) % menu.matches.len();
                    const FILE_MENU_VISIBLE_ROWS: usize = 8;
                    if menu.selected >= menu.scroll_offset + FILE_MENU_VISIBLE_ROWS {
                        menu.scroll_offset = menu.selected + 1 - FILE_MENU_VISIBLE_ROWS;
                    }
                }
                return None;
            }
            KeyCode::Tab => {
                // If the menu is showing a directory, Tab navigates into it;
                // if it is a file, insert it and close the menu.
                let _ = app.accept_file_menu_selection();
                return None;
            }
            KeyCode::Enter => {
                // Accept selection only. Sending is a separate Enter after menu closes.
                let _ = app.accept_file_menu_selection();
                return None;
            }
            KeyCode::Esc => {
                app.file_menu = None;
                return None;
            }
            KeyCode::Char('\\') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.file_menu_show_hidden = !app.file_menu_show_hidden;
                app.refresh_input_menus();
                return None;
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(InputAction::MoveCursorHome);
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(InputAction::MoveCursorEnd);
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(InputAction::MoveCursorLeft);
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(InputAction::MoveCursorRight);
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(InputAction::DeletePrevWord);
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(InputAction::DeleteToLineEnd);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(InputAction::ClearLine);
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(InputAction::MoveCursorWordLeft);
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(InputAction::MoveCursorWordRight);
            }
            KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(InputAction::MoveCursorHome);
            }
            KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(InputAction::MoveCursorEnd);
            }
            KeyCode::Left => return Some(InputAction::MoveCursorLeft),
            KeyCode::Right => return Some(InputAction::MoveCursorRight),
            KeyCode::Home => return Some(InputAction::MoveCursorHome),
            KeyCode::End => return Some(InputAction::MoveCursorEnd),
            KeyCode::Delete => return Some(InputAction::Delete),
            KeyCode::Char(c) => {
                app.insert_char_at_cursor(c);
                return None;
            }
            KeyCode::Backspace => {
                app.delete_prev_char();
                return None;
            }
            _ => return None,
        }
    }

    // Research markdown viewer: intercept scroll/close keys.
    if app.research_view.is_some() {
        return match key.code {
            KeyCode::PageUp if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputAction::ResearchViewToStart)
            }
            KeyCode::PageDown if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputAction::ResearchViewToEnd)
            }
            KeyCode::PageUp => Some(InputAction::ResearchViewPageUp),
            KeyCode::PageDown => Some(InputAction::ResearchViewPageDown),
            KeyCode::Up => Some(InputAction::ResearchViewLineUp),
            KeyCode::Down => Some(InputAction::ResearchViewLineDown),
            KeyCode::Esc => {
                app.research_view = None;
                None
            }
            _ => None,
        };
    }

    if app.memory_view.is_some() {
        return match key.code {
            KeyCode::PageUp if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputAction::MemoryViewToStart)
            }
            KeyCode::PageDown if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputAction::MemoryViewToEnd)
            }
            KeyCode::PageUp => Some(InputAction::MemoryViewPageUp),
            KeyCode::PageDown => Some(InputAction::MemoryViewPageDown),
            KeyCode::Up => Some(InputAction::MemoryViewLineUp),
            KeyCode::Down => Some(InputAction::MemoryViewLineDown),
            KeyCode::Esc => {
                app.memory_view = None;
                None
            }
            _ => None,
        };
    }

    if app.output_view.is_some() {
        return match key.code {
            KeyCode::PageUp if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputAction::OutputViewToStart)
            }
            KeyCode::PageDown if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(InputAction::OutputViewToEnd)
            }
            KeyCode::PageUp => Some(InputAction::OutputViewPageUp),
            KeyCode::PageDown => Some(InputAction::OutputViewPageDown),
            // Alt+Down/Up cycle teammate focus even while output view is open
            KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
                Some(InputAction::FocusNextTeammate)
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
                Some(InputAction::FocusPrevTeammate)
            }
            KeyCode::Esc => {
                app.output_view = None;
                app.selected_agent_session_id = None;
                app.selected_agent_index = None;
                None
            }
            _ => None,
        };
    }

    match key.code {
        KeyCode::Enter
            if key.modifiers.contains(KeyModifiers::SHIFT)
                || key.modifiers.contains(KeyModifiers::ALT) =>
        {
            // Shift+Enter (Kitty protocol) or Alt+Enter (universal fallback):
            // insert a literal newline without sending the message.
            app.clear_kb_selection();
            app.insert_char_at_cursor('\n');
            None
        }
        KeyCode::Enter if app.show_memory && app.input.is_empty() => {
            // With the Memory side panel visible and an empty prompt, Enter
            // opens the selected memory. When the prompt has text, Enter
            // falls through to the normal send path so the memory panel
            // does not intercept message submission.
            Some(InputAction::OpenMemoryView)
        }
        KeyCode::Enter => {
            if app.is_input_blocked() {
                app.status = "busy - wait for the current turn to finish".to_string();
                return None;
            }
            let text = app.input.clone();
            if text.is_empty() {
                return None;
            }
            if text.starts_with('/') {
                return Some(InputAction::SlashCommand(text));
            }
            if text.starts_with('!') {
                return Some(InputAction::BangCommand(text));
            }
            Some(InputAction::SendMessage(text))
        }
        KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.is_processing {
                Some(InputAction::CancelAgent)
            } else {
                Some(InputAction::CutToClipboard)
            }
        }
        KeyCode::Char('v')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            Some(InputAction::PasteFromClipboard)
        }
        KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::ALT) => {
            // Alt+V: paste image from clipboard as a staged attachment.
            app.paste_image_from_clipboard();
            None
        }
        KeyCode::Char('?') if app.input.is_empty() => {
            // Show keybindings help panel when '?' is typed on an empty input.
            app.show_shortcuts = true;
            None
        }
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::SelectAll)
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::MoveCursorEnd)
        }
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::MoveCursorLeft)
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::MoveCursorRight)
        }
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::DeletePrevWord)
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::DeleteToLineEnd)
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::ClearLine)
        }
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::ALT) => {
            Some(InputAction::ToggleLog)
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::ALT) => {
            Some(InputAction::ToggleProfile)
        }
        // Alt+T toggles the TODO panel (placed before generic char-insert
        // handling so the `t` is never inserted into the input buffer — NFR-002).
        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::ALT) => {
            Some(InputAction::ToggleTasksPanel)
        }
        // Alt+M toggles the Memory side panel (placed before generic char-insert
        // handling so the `m` is never inserted into the input buffer — NFR-002,
        // FR-011).
        KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::ALT) => {
            Some(InputAction::ToggleMemory)
        }
        // Memory panel cursor navigation: when the panel is visible, Up/Down
        // move the block cursor through memories instead of navigating input
        // history (FR-016).
        KeyCode::Up if app.show_memory => Some(InputAction::MemoryCursorUp),
        KeyCode::Down if app.show_memory => Some(InputAction::MemoryCursorDown),
        // Open / delete the selected memory when the Memory panel is focused.
        // Alt+O toggles the Telemetry side panel (placed before generic char-insert
        // handling so the `o` is never inserted into the input buffer — NFR-002).
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::ALT) => {
            Some(InputAction::ToggleTelemetry)
        }
        KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::ALT) => {
            Some(InputAction::ToggleContextPanel)
        }
        KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::ALT) => {
            Some(InputAction::ToggleYolo)
        }
        // Alt+E toggles edit-operation logging.
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::ALT) => {
            Some(InputAction::ToggleEditLog)
        }
        KeyCode::Char(c) => {
            if app.is_input_blocked() {
                app.status = "busy - wait for the current turn to finish".to_string();
                return None;
            }
            // Typing a character replaces the active keyboard selection.
            if let Some((start, end)) = app.kb_selection_char_range() {
                app.remove_input_char_range(start, end);
                app.kb_select_anchor = None;
            }
            app.insert_char_at_cursor(c);
            None
        }
        KeyCode::Backspace => {
            // Backspace deletes the selection when one is active.
            if let Some((start, end)) = app.kb_selection_char_range() {
                app.remove_input_char_range(start, end);
                app.kb_select_anchor = None;
            } else {
                app.delete_prev_char();
            }
            None
        }
        KeyCode::Delete => {
            // Delete removes the active selection, otherwise the character
            // under the cursor (forward delete).  When the Memory panel is
            // focused and the prompt is empty, Delete prompts to delete the
            // selected memory instead.
            if app.show_memory && app.input.is_empty() {
                Some(InputAction::PromptMemoryDelete)
            } else if let Some((start, end)) = app.kb_selection_char_range() {
                app.remove_input_char_range(start, end);
                app.kb_select_anchor = None;
                None
            } else {
                app.delete_next_char();
                None
            }
        }
        KeyCode::Left
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            Some(InputAction::SelectWordLeft)
        }
        KeyCode::Right
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            Some(InputAction::SelectWordRight)
        }
        KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::MoveCursorWordLeft)
        }
        KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::MoveCursorWordRight)
        }
        KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::MoveCursorHome)
        }
        KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::MoveCursorEnd)
        }
        KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
            Some(InputAction::SelectCharLeft)
        }
        KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
            Some(InputAction::SelectCharRight)
        }
        KeyCode::Left => Some(InputAction::MoveCursorLeft),
        KeyCode::Right => Some(InputAction::MoveCursorRight),
        KeyCode::Home => Some(InputAction::MoveCursorHome),
        KeyCode::End => Some(InputAction::MoveCursorEnd),
        KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => Some(InputAction::ScrollUp),
        KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
            Some(InputAction::ScrollDown)
        }
        KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
            Some(InputAction::FocusNextTeammate)
        }
        KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
            Some(InputAction::FocusPrevTeammate)
        }
        KeyCode::Up => Some(InputAction::HistoryUp),
        KeyCode::Down => Some(InputAction::HistoryDown),
        KeyCode::PageUp if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::LogScrollUp)
        }
        KeyCode::PageDown if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::LogScrollDown)
        }
        KeyCode::PageUp => Some(InputAction::ScrollUp),
        KeyCode::PageDown => Some(InputAction::ScrollDown),
        KeyCode::Tab | KeyCode::BackTab => Some(InputAction::SwitchAgent),
        KeyCode::Esc if app.is_processing => Some(InputAction::CancelAgent),
        _ => None,
    }
}

/// Handle key events inside the provider setup dialog.
fn handle_provider_setup_key(app: &mut App, key: KeyEvent) {
    // Escape always closes non-router setup dialogs. Router dialogs handle Esc
    // internally so that the model picker can cancel back to the cluster panel
    // without discarding the whole flow.
    if key.code == KeyCode::Esc
        && !matches!(
            app.provider_setup,
            Some(
                ProviderSetupStep::SetupRouter { .. } | ProviderSetupStep::SelectRouterModel { .. }
            )
        )
    {
        app.provider_setup = None;
        return;
    }

    let Some(step) = app.provider_setup.take() else {
        return;
    };

    match step {
        ProviderSetupStep::SelectProvider {
            selected,
            force_key_entry,
        } => match key.code {
            KeyCode::Up => {
                let new = if selected == 0 {
                    PROVIDER_LIST.len() - 1
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::SelectProvider {
                    selected: new,
                    force_key_entry,
                });
            }
            KeyCode::Down => {
                let new = (selected + 1) % PROVIDER_LIST.len();
                app.provider_setup = Some(ProviderSetupStep::SelectProvider {
                    selected: new,
                    force_key_entry,
                });
            }
            KeyCode::Enter => {
                let (pid, pname) = PROVIDER_LIST[selected];
                // If the provider is already configured and the dialog was opened
                // via `/model` (force_key_entry == false), skip straight to the
                // model picker. When opened via `/provider` (force_key_entry ==
                // true) the API-key prompt is always shown so the user can edit
                // the key.
                //
                // The Model Router is an exception: it is a virtual provider with
                // no API key of its own, so selecting it always opens the cluster
                // setup UI regardless of force_key_entry.
                //
                // Azure Resource is also an exception: it has no API key, only a
                // resource-file picker, so it always opens that picker.
                let already_configured = App::get_configured_providers(&app.storage)
                    .iter()
                    .any(|p| p.id == pid);
                if already_configured && !force_key_entry {
                    if pid == "azure_resource" {
                        app.refresh_provider();
                        let provider =
                            ragent_agent::provider::azure_resource::AzureResourceProvider::new();
                        let entries = provider.entries();
                        if entries.is_empty() {
                            app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                                entries: Vec::new(),
                                selected: 0,
                                error: Some(
                                    "No Azure resources found. Check azureresources.json."
                                        .to_string(),
                                ),
                            });
                        } else {
                            // Try to restore last selection
                            let mut selected = 0usize;
                            if let Ok(Some(last)) =
                                app.storage.get_setting("azure_resource_last_selection")
                            {
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&last)
                                {
                                    if let Some(last_id) = parsed.get("id").and_then(|v| v.as_str())
                                    {
                                        if let Some(idx) =
                                            entries.iter().position(|e| e.id == last_id)
                                        {
                                            selected = idx;
                                        } else {
                                            let _ = app
                                                .storage
                                                .delete_setting("azure_resource_last_selection");
                                        }
                                    }
                                }
                            }
                            app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                                entries,
                                selected,
                                error: None,
                            });
                        }
                        return;
                    }
                    if pid == "router" {
                        let providers = App::get_configured_providers_for_router(&app.storage);
                        if providers.is_empty() {
                            app.status =
                                "[warn] No concrete providers — configure one first".to_string();
                            app.push_log_no_agent(
                                crate::app::LogLevel::Warn,
                                "provider router: no concrete providers configured".to_string(),
                            );
                            app.provider_setup = Some(ProviderSetupStep::SelectProvider {
                                selected,
                                force_key_entry,
                            });
                        } else {
                            app.provider_setup = Some(app.seeded_router_setup_step(providers));
                        }
                        return;
                    }
                    app.refresh_provider();
                    app.provider_setup = Some(ProviderSetupStep::LoadingModels {
                        provider_id: pid.to_string(),
                        provider_name: pname.to_string(),
                    });
                    app.start_model_discovery(pid.to_string(), pname.to_string());
                } else if pid == "ollama" {
                    // Ollama doesn't require a key — store empty and mark configured
                    let _ = app.storage.set_provider_auth(pid, "");
                    let _ = app
                        .storage
                        .delete_setting(&format!("provider_{pid}_disabled"));
                    app.refresh_provider();
                    app.provider_setup = Some(ProviderSetupStep::LoadingModels {
                        provider_id: pid.to_string(),
                        provider_name: pname.to_string(),
                    });
                    app.start_model_discovery(pid.to_string(), pname.to_string());
                } else if pid == "copilot" {
                    // Copilot: try auto-discover and verify token exchange
                    let storage = app.storage.clone();
                    let db_lookup = || {
                        storage
                            .get_provider_auth("copilot")
                            .ok()
                            .flatten()
                            .filter(|k| !k.is_empty())
                    };
                    let token = ragent_agent::provider::copilot::resolve_copilot_github_token(
                        Some(&db_lookup),
                    );
                    if let Some(ref tk) = token {
                        // Token exchange is a network call — run it in a
                        // background task so the UI thread never blocks
                        // (FR-002, FR-004).  Show the loading spinner while we
                        // wait for `CopilotTokenExchangeResult`.
                        if let Ok(handle) = tokio::runtime::Handle::try_current() {
                            app.provider_setup = Some(ProviderSetupStep::LoadingModels {
                                provider_id: pid.to_string(),
                                provider_name: pname.to_string(),
                            });
                            app.model_loading_state = Some(crate::app::ModelLoadingState {
                                provider_id: pid.to_string(),
                                provider_name: pname.to_string(),
                                started_at: std::time::Instant::now(),
                            });
                            let event_bus = app.event_bus.clone();
                            let tk_clone = tk.clone();
                            handle.spawn(async move {
                                let result = ragent_agent::provider::copilot::resolve_copilot_auth(
                                    &tk_clone, None,
                                )
                                .await;
                                let (success, api_base, error) = match result {
                                    Ok(auth) => {
                                        if !auth.base_url.contains("models.inference.ai.azure.com")
                                        {
                                            (true, Some(auth.base_url), None)
                                        } else {
                                            // Azure-hosted Copilot endpoint — treat as
                                            // needing device flow for proper setup.
                                            (false, None, None)
                                        }
                                    }
                                    Err(e) => (false, None, Some(format!("{e:#}"))),
                                };
                                event_bus.publish(
                                    ragent_agent::event::Event::CopilotTokenExchangeResult {
                                        success,
                                        api_base,
                                        error,
                                    },
                                );
                            });
                            return;
                        }
                    }
                    // No token or no runtime — start device flow
                    start_copilot_device_flow_setup(app);
                } else if pid == "azure_resource" {
                    // Azure Resource: load entries from azureresources.json
                    let provider =
                        ragent_agent::provider::azure_resource::AzureResourceProvider::new();
                    let entries = provider.entries();
                    if entries.is_empty() {
                        app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                            entries: Vec::new(),
                            selected: 0,
                            error: Some(
                                "No Azure resources found. Check azureresources.json.".to_string(),
                            ),
                        });
                    } else {
                        // Try to restore last selection
                        let mut selected = 0usize;
                        if let Ok(Some(last)) =
                            app.storage.get_setting("azure_resource_last_selection")
                        {
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&last) {
                                if let Some(last_id) = parsed.get("id").and_then(|v| v.as_str()) {
                                    if let Some(idx) = entries.iter().position(|e| e.id == last_id)
                                    {
                                        selected = idx;
                                    } else {
                                        let _ = app
                                            .storage
                                            .delete_setting("azure_resource_last_selection");
                                    }
                                }
                            }
                        }
                        app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                            entries,
                            selected,
                            error: None,
                        });
                    }
                } else if pid == "router" {
                    // The Model Router is a virtual provider with no API key of
                    // its own. Selecting it from the provider picker opens the
                    // router cluster setup panel, mirroring `/provider router`.
                    let providers = App::get_configured_providers_for_router(&app.storage);
                    if providers.is_empty() {
                        app.status =
                            "[warn] No concrete providers — configure one first".to_string();
                        app.push_log_no_agent(
                            crate::app::LogLevel::Warn,
                            "provider router: no concrete providers configured".to_string(),
                        );
                        // Keep the picker open so the user can choose a concrete
                        // provider to configure first.
                        app.provider_setup = Some(ProviderSetupStep::SelectProvider {
                            selected,
                            force_key_entry,
                        });
                    } else {
                        app.provider_setup = Some(app.seeded_router_setup_step(providers));
                    }
                } else {
                    // Pre-fill the key field with the existing stored key (if
                    // any) so the user can edit it rather than re-entering
                    // from scratch.
                    let existing_key = app.provider_api_key(pid).unwrap_or_default();
                    app.provider_setup = Some(ProviderSetupStep::EnterKey {
                        provider_id: pid.to_string(),
                        provider_name: pname.to_string(),
                        key_field: crate::input_field::InputField::with_text(existing_key),
                        endpoint_field: if pid == "generic_openai" {
                            crate::input_field::InputField::with_text(
                                app.storage
                                    .get_setting("generic_openai_api_base")
                                    .ok()
                                    .flatten()
                                    .unwrap_or_default(),
                            )
                        } else if pid == "azure_foundry" {
                            crate::input_field::InputField::with_text(
                                app.storage
                                    .get_setting("azure_foundry_api_base")
                                    .ok()
                                    .flatten()
                                    .unwrap_or_default(),
                            )
                        } else {
                            crate::input_field::InputField::new()
                        },
                        active_field: 0,
                        error: None,
                    });
                }
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::SelectProvider {
                    selected,
                    force_key_entry,
                });
            }
        },
        ProviderSetupStep::EnterKey {
            provider_id,
            provider_name,
            mut key_field,
            mut endpoint_field,
            mut active_field,
            ..
        } => match key.code {
            KeyCode::Enter => {
                let trimmed = key_field.text().trim().to_string();
                if trimmed.is_empty() {
                    app.provider_setup = Some(ProviderSetupStep::EnterKey {
                        provider_id,
                        provider_name,
                        key_field,
                        endpoint_field,
                        active_field,
                        error: Some("API key cannot be empty.".to_string()),
                    });
                } else if provider_id == "copilot"
                    && ragent_agent::provider::copilot::is_pat_token(&trimmed)
                {
                    app.provider_setup = Some(ProviderSetupStep::EnterKey {
                          provider_id,
                          provider_name,
                          key_field,
                          endpoint_field,
                          active_field,
                          error: Some(
                              "PATs (github_pat_/ghp_) are not supported by                                the Copilot API. Run: gh auth refresh -s copilot"
                                  .to_string(),
                          ),
                      });
                } else {
                    let _ = app.storage.set_provider_auth(&provider_id, &trimmed);
                    if provider_id == "generic_openai" || provider_id == "azure_foundry" {
                        let endpoint = endpoint_field.text().trim();
                        if endpoint.is_empty() {
                            let _ = app
                                .storage
                                .delete_setting(&format!("{provider_id}_api_base"));
                        } else {
                            let _ = app
                                .storage
                                .set_setting(&format!("{provider_id}_api_base"), endpoint);
                        }
                    }
                    let _ = app
                        .storage
                        .delete_setting(&format!("provider_{provider_id}_disabled"));
                    app.refresh_provider();
                    app.provider_setup = Some(ProviderSetupStep::LoadingModels {
                        provider_id: provider_id.clone(),
                        provider_name: provider_name.clone(),
                    });
                    app.start_model_discovery(provider_id, provider_name);
                }
            }
            KeyCode::Tab if provider_id == "generic_openai" || provider_id == "azure_foundry" => {
                active_field = if active_field == 0 { 1 } else { 0 };
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_field,
                    endpoint_field,
                    active_field,
                    error: None,
                });
            }
            _ => {
                let is_endpoint = provider_id == "generic_openai" || provider_id == "azure_foundry";
                let target = if is_endpoint && active_field == 1 {
                    &mut endpoint_field
                } else {
                    &mut key_field
                };
                let consumed = target.handle_key(key);
                if !consumed
                    && matches!(key.code, KeyCode::Char('v') | KeyCode::Char('V'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                {
                    target.paste_text_from_clipboard();
                }
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_field,
                    endpoint_field,
                    active_field,
                    error: None,
                });
            }
        },
        ProviderSetupStep::DeviceFlowPending {
            flow,
            user_code,
            verification_uri,
        } => match key.code {
            KeyCode::Esc => {
                app.provider_setup = None;
            }
            KeyCode::Char('c') => {
                crate::clipboard::set_clipboard_text(&user_code);
                app.status = "[ok] Device code copied to clipboard".to_string();
                app.provider_setup = Some(ProviderSetupStep::DeviceFlowPending {
                    flow,
                    user_code,
                    verification_uri,
                });
            }
            _ => {
                // Keep showing the device flow pending UI — polling happens
                // in a background task and completes via the appropriate
                // device-flow completion event.
                app.provider_setup = Some(ProviderSetupStep::DeviceFlowPending {
                    flow,
                    user_code,
                    verification_uri,
                });
            }
        },
        ProviderSetupStep::SelectModel {
            provider_id,
            provider_name,
            models,
            selected,
        } => match key.code {
            KeyCode::Up => {
                let new = if models.is_empty() {
                    0
                } else if selected == 0 {
                    models.len() - 1
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::SelectModel {
                    provider_id,
                    provider_name,
                    models,
                    selected: new,
                });
            }
            KeyCode::Down => {
                let new = if models.is_empty() {
                    0
                } else {
                    (selected + 1) % models.len()
                };
                app.provider_setup = Some(ProviderSetupStep::SelectModel {
                    provider_id,
                    provider_name,
                    models,
                    selected: new,
                });
            }
            KeyCode::Enter => {
                if let Some(entry) = models.get(selected).cloned() {
                    // Ollama-family providers are boolean thinkers, but model
                    // detection of reasoning support is unreliable. Always
                    // show the reasoning selector so the user can override it.
                    let force_selector =
                        is_ollama_family(&provider_id) || !entry.thinking_levels.is_empty();
                    if force_selector {
                        let default_level = App::default_thinking_level_for_entry(&entry);
                        let thinking_levels =
                            if entry.thinking_levels.is_empty() && is_ollama_family(&provider_id) {
                                ragent_llm::providers::thinking::full_reasoning_levels()
                            } else {
                                entry.thinking_levels.clone()
                            };
                        let selected_level = thinking_levels
                            .iter()
                            .position(|level| *level == default_level)
                            .unwrap_or(0);
                        app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
                            provider_id,
                            provider_name,
                            model: ModelPickerEntry {
                                thinking_levels,
                                ..entry
                            },
                            selected: selected_level,
                        });
                    } else {
                        let model_name = app.finalize_model_selection(
                            provider_id,
                            provider_name.clone(),
                            &entry,
                            ThinkingLevel::Off,
                        );
                        app.provider_setup = Some(ProviderSetupStep::Done {
                            provider_name,
                            model_name: Some(model_name),
                        });
                    }
                } else {
                    app.provider_setup = Some(ProviderSetupStep::Done {
                        provider_name,
                        model_name: None,
                    });
                }
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::SelectModel {
                    provider_id,
                    provider_name,
                    models,
                    selected,
                });
            }
        },
        ProviderSetupStep::SelectAzureResource {
            entries,
            selected,
            error,
        } => match key.code {
            KeyCode::Up => {
                let new = if selected == 0 {
                    entries.len().saturating_sub(1)
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                    entries,
                    selected: new,
                    error,
                });
            }
            KeyCode::Down => {
                let new = if entries.is_empty() {
                    0
                } else {
                    (selected + 1) % entries.len()
                };
                app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                    entries,
                    selected: new,
                    error,
                });
            }
            KeyCode::Enter => {
                if let Some(entry) = entries.get(selected) {
                    // Persist selection (include api_type for re-hydration)
                    let payload = serde_json::json!({
                        "id": entry.id,
                        "endpoint": entry.endpoint,
                        "api_key": entry.api_key,
                        "api_key_env": entry.api_key_env,
                        "api_type": entry.api_type,
                    });
                    let _ = app
                        .storage
                        .set_setting("azure_resource_last_selection", &payload.to_string());
                    // Set active provider to azure_resource with the entry's endpoint and model id
                    let _ = app
                        .storage
                        .set_setting("preferred_provider", "azure_resource");
                    let _ = app
                        .storage
                        .set_setting("azure_resource_api_base", &entry.endpoint);
                    let model_value = format!("azure_resource/{}", entry.id);
                    let _ = app.storage.set_setting("selected_model", &model_value);
                    let _ = app.storage.set_setting(
                        "selected_model_ctx_window",
                        &entry.context_window.unwrap_or(128_000).to_string(),
                    );
                    app.selected_model = Some(model_value);
                    app.selected_model_ctx_window = Some(entry.context_window.unwrap_or(128_000));
                    app.configured_provider = Some(ConfiguredProvider {
                        id: "azure_resource".to_string(),
                        name: "Azure Resource (File)".to_string(),
                        source: ProviderSource::Database,
                    });
                    app.provider_setup = Some(ProviderSetupStep::Done {
                        provider_name: "Azure Resource (File)".to_string(),
                        model_name: Some(entry.name.clone()),
                    });
                }
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                    entries,
                    selected,
                    error,
                });
            }
        },
        ProviderSetupStep::SelectThinkingLevel {
            provider_id,
            provider_name,
            model,
            selected,
        } => match key.code {
            KeyCode::Up => {
                let new = if selected == 0 {
                    model.thinking_levels.len().saturating_sub(1)
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
                    provider_id,
                    provider_name,
                    model,
                    selected: new,
                });
            }
            KeyCode::Down => {
                let new = if model.thinking_levels.is_empty() {
                    0
                } else {
                    (selected + 1) % model.thinking_levels.len()
                };
                app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
                    provider_id,
                    provider_name,
                    model,
                    selected: new,
                });
            }
            KeyCode::Enter => {
                let level = model
                    .thinking_levels
                    .get(selected)
                    .copied()
                    .unwrap_or(ThinkingLevel::Off);
                let model_name =
                    app.finalize_model_selection(provider_id, provider_name.clone(), &model, level);
                app.provider_setup = Some(ProviderSetupStep::Done {
                    provider_name,
                    model_name: Some(model_name),
                });
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
                    provider_id,
                    provider_name,
                    model,
                    selected,
                });
            }
        },
        ProviderSetupStep::Done { .. } => {
            // Any key closes the done screen and triggers a health check
            app.provider_setup = None;
            app.check_provider_health();
        }
        ProviderSetupStep::SelectAgent { agents, selected } => match key.code {
            KeyCode::Up => {
                let new = if agents.is_empty() {
                    0
                } else if selected == 0 {
                    agents.len() - 1
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::SelectAgent {
                    agents,
                    selected: new,
                });
            }
            KeyCode::Down => {
                let new = if agents.is_empty() {
                    0
                } else {
                    (selected + 1) % agents.len()
                };
                app.provider_setup = Some(ProviderSetupStep::SelectAgent {
                    agents,
                    selected: new,
                });
            }
            KeyCode::Enter => {
                if let Some((name, _desc, _is_custom)) = agents.get(selected) {
                    if let Some(idx) = app.cycleable_agents.iter().position(|a| a.name == *name) {
                        app.current_agent_index = idx;
                        app.agent_info = app.cycleable_agents[idx].clone();
                        app.agent_name = app.agent_info.name.clone();
                        app.status = format!("Agent: {}", app.agent_name);
                    }
                }
                app.provider_setup = None;
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::SelectAgent { agents, selected });
            }
        },
        ProviderSetupStep::SelectConfiguredProvider {
            providers,
            selected,
        } => match key.code {
            KeyCode::Up => {
                let new = if selected == 0 {
                    providers.len().saturating_sub(1)
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::SelectConfiguredProvider {
                    providers,
                    selected: new,
                });
            }
            KeyCode::Down => {
                let new = if providers.is_empty() {
                    0
                } else {
                    (selected + 1) % providers.len()
                };
                app.provider_setup = Some(ProviderSetupStep::SelectConfiguredProvider {
                    providers,
                    selected: new,
                });
            }
            KeyCode::Enter => {
                if let Some(prov) = providers.get(selected).cloned() {
                    let prov_id = prov.id.clone();
                    let prov_name = prov.name.clone();
                    // FR-003/FR-004: try persisted model restore, fallback to picker.
                    if app
                        .try_restore_provider_model(&prov_id, &prov_name)
                        .is_some()
                    {
                        // Model was restored — show the Done confirmation.
                        app.provider_setup = Some(ProviderSetupStep::Done {
                            provider_name: prov_name,
                            model_name: app
                                .selected_model
                                .as_ref()
                                .and_then(|m| crate::app::model_part_from_selected_model(m))
                                .map(String::from),
                        });
                    } else {
                        app.provider_setup = Some(ProviderSetupStep::LoadingModels {
                            provider_id: prov_id.clone(),
                            provider_name: prov_name.clone(),
                        });
                        app.start_model_discovery(prov_id, prov_name);
                    }
                } else {
                    app.provider_setup = None;
                }
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::SelectConfiguredProvider {
                    providers,
                    selected,
                });
            }
        },
        ProviderSetupStep::ShowProviderConfig {
            providers,
            selected,
        } => match key.code {
            KeyCode::Up => {
                let new = if selected == 0 {
                    providers.len().saturating_sub(1)
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::ShowProviderConfig {
                    providers,
                    selected: new,
                });
            }
            KeyCode::Down => {
                let new = if providers.is_empty() {
                    0
                } else {
                    (selected + 1) % providers.len()
                };
                app.provider_setup = Some(ProviderSetupStep::ShowProviderConfig {
                    providers,
                    selected: new,
                });
            }
            KeyCode::Enter => {
                if let Some(prov) = providers.get(selected).cloned() {
                    let report = if prov.id == "router" {
                        app.router_config_report(&app.provider_registry.clone())
                    } else {
                        app.provider_config_report(&prov)
                    };
                    app.append_assistant_text(&report);
                    app.status = format!("provider show: {}", prov.name);
                }
                app.provider_setup = None;
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::ShowProviderConfig {
                    providers,
                    selected,
                });
            }
        },

        ProviderSetupStep::ResetProvider { selected } => match key.code {
            KeyCode::Up => {
                let new = if selected == 0 {
                    PROVIDER_LIST.len() - 1
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::ResetProvider { selected: new });
            }
            KeyCode::Down => {
                let new = (selected + 1) % PROVIDER_LIST.len();
                app.provider_setup = Some(ProviderSetupStep::ResetProvider { selected: new });
            }
            KeyCode::Enter => {
                let (pid, pname) = PROVIDER_LIST[selected];
                let _ = app.storage.delete_provider_auth(pid);
                let _ = app
                    .storage
                    .set_setting(&format!("provider_{pid}_disabled"), "true");
                // Clear provider-specific settings
                let _ = app
                    .storage
                    .delete_setting(&format!("provider_{pid}_last_model"));
                if pid == "copilot" {
                    let _ = app.storage.delete_setting("copilot_api_base");
                } else if pid == "generic_openai" {
                    let _ = app.storage.delete_setting("generic_openai_api_base");
                } else if pid == "azure_foundry" {
                    let _ = app.storage.delete_setting("azure_foundry_api_base");
                }
                let is_active = app
                    .configured_provider
                    .as_ref()
                    .map_or(false, |p| p.id == pid);
                if is_active {
                    app.configured_provider = None;
                    app.selected_model = None;
                    app.selected_model_ctx_window = None;
                    app.selected_thinking_level = None;
                    let _ = app.storage.delete_setting("selected_model");
                    let _ = app.storage.delete_setting("selected_model_ctx_window");
                    let _ = app.storage.delete_setting("thinking_level");
                    let _ = app.storage.delete_setting("thinking_level_explicit");
                    app.provider_health
                        .store(0, std::sync::atomic::Ordering::Relaxed);
                }
                app.status = format!("[ok] Provider {} reset — credentials removed", pname);
                app.provider_setup = None;
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::ResetProvider { selected });
            }
        },

        // ── GitLab setup (multi-field form) ──────────────────────────────
        ProviderSetupStep::GitLabSetup {
            mut url_input,
            mut url_cursor,
            mut token_input,
            mut token_cursor,
            mut active_field,
            error,
        } => match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                active_field = if active_field == 0 { 1 } else { 0 };
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error,
                });
            }
            KeyCode::Enter => {
                let url = url_input.trim().to_string();
                let tok = token_input.trim().to_string();
                if url.is_empty() || tok.is_empty() {
                    app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                        url_input,
                        url_cursor,
                        token_input,
                        token_cursor,
                        active_field,
                        error: Some("Both URL and token are required.".to_string()),
                    });
                } else {
                    // Start async validation
                    app.provider_setup = Some(ProviderSetupStep::GitLabValidating {
                        instance_url: url.clone(),
                        token: tok.clone(),
                    });
                    start_gitlab_validation(app, url, tok);
                }
            }
            KeyCode::Char(c) => {
                if active_field == 0 {
                    url_input.insert(url_cursor, c);
                    url_cursor += 1;
                } else {
                    token_input.insert(token_cursor, c);
                    token_cursor += 1;
                }
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error: None,
                });
            }
            KeyCode::Backspace => {
                if active_field == 0 {
                    if url_cursor > 0 {
                        url_cursor -= 1;
                        url_input.remove(url_cursor);
                    }
                } else if token_cursor > 0 {
                    token_cursor -= 1;
                    token_input.remove(token_cursor);
                }
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error: None,
                });
            }
            KeyCode::Left => {
                if active_field == 0 {
                    url_cursor = url_cursor.saturating_sub(1);
                } else {
                    token_cursor = token_cursor.saturating_sub(1);
                }
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error,
                });
            }
            KeyCode::Right => {
                if active_field == 0 {
                    if url_cursor < url_input.len() {
                        url_cursor += 1;
                    }
                } else if token_cursor < token_input.len() {
                    token_cursor += 1;
                }
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error,
                });
            }
            KeyCode::Home => {
                if active_field == 0 {
                    url_cursor = 0;
                } else {
                    token_cursor = 0;
                }
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error,
                });
            }
            KeyCode::End => {
                if active_field == 0 {
                    url_cursor = url_input.len();
                } else {
                    token_cursor = token_input.len();
                }
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error,
                });
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error,
                });
            }
        },

        ProviderSetupStep::GitLabValidating {
            instance_url,
            token,
        } => {
            // Esc cancels and returns to the form
            if key.code == KeyCode::Esc {
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input: instance_url,
                    url_cursor: 0,
                    token_input: token,
                    token_cursor: 0,
                    active_field: 0,
                    error: Some("Validation cancelled.".to_string()),
                });
            } else {
                app.provider_setup = Some(ProviderSetupStep::GitLabValidating {
                    instance_url,
                    token,
                });
            }
        }

        // ── Telemetry setup (multi-field form) ───────────────────────────
        ProviderSetupStep::TelemetrySetup {
            mut endpoint_field,
            mut protocol,
            mut interval_field,
            mut timeout_field,
            mut port_field,
            mut active_field,
            error: _,
        } => {
            let is_text_field = active_field != 1;
            match key.code {
                KeyCode::Tab => {
                    active_field = (active_field + 1) % 5;
                }
                KeyCode::BackTab => {
                    active_field = if active_field == 0 {
                        4
                    } else {
                        active_field - 1
                    };
                }
                KeyCode::Up | KeyCode::Down if active_field == 1 => {
                    protocol = match protocol {
                        ragent_config::OtelProtocol::Http => ragent_config::OtelProtocol::Grpc,
                        ragent_config::OtelProtocol::Grpc => ragent_config::OtelProtocol::Http,
                    };
                }
                KeyCode::Enter => {
                    let endpoint = endpoint_field.text().trim().to_string();
                    let interval_str = interval_field.text().trim();
                    let timeout_str = timeout_field.text().trim();
                    let port_str = port_field.text().trim();

                    let interval = interval_str.parse::<u64>().unwrap_or(0);
                    let timeout = timeout_str.parse::<u64>().unwrap_or(0);
                    let internal_port = if port_str.is_empty() {
                        None
                    } else {
                        match port_str.parse::<u16>() {
                            Ok(p) => Some(p),
                            Err(_) => {
                                app.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
                                    endpoint_field,
                                    protocol,
                                    interval_field,
                                    timeout_field,
                                    port_field,
                                    active_field,
                                    error: Some(format!(
                                        "Internal port must be a number between 0 and {}.",
                                        u16::MAX
                                    )),
                                });
                                return;
                            }
                        }
                    };

                    let draft = ragent_config::OtelConfig {
                        enabled: true,
                        endpoint,
                        protocol,
                        export_interval_seconds: interval,
                        export_timeout_seconds: timeout,
                        service_name: "ragent".to_string(),
                        resource_attributes: std::collections::HashMap::new(),
                        metrics: std::collections::HashMap::new(),
                        internal_port,
                        cardinality_limit: 1000,
                    };

                    let problems = draft.validate();
                    if !problems.is_empty() {
                        app.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
                            endpoint_field,
                            protocol,
                            interval_field,
                            timeout_field,
                            port_field,
                            active_field,
                            error: Some(problems.join("; ")),
                        });
                        return;
                    }

                    match app.save_telemetry_otel(&draft) {
                        Ok(()) => {
                            app.session_processor.invalidate_config_cache();
                            app.provider_setup = None;
                            app.append_assistant_text(
                                "From: /telemetry setup\n[ok] **Telemetry configuration saved.**\n\nSettings take effect on the next ragent start, or immediately if the telemetry subsystem is already running.",
                            );
                            app.status = "telemetry: saved".to_string();
                        }
                        Err(e) => {
                            app.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
                                endpoint_field,
                                protocol,
                                interval_field,
                                timeout_field,
                                port_field,
                                active_field,
                                error: Some(e),
                            });
                        }
                    }
                    return;
                }
                KeyCode::Char(c) if is_text_field => {
                    let target = match active_field {
                        0 => &mut endpoint_field,
                        2 => &mut interval_field,
                        3 => &mut timeout_field,
                        4 => &mut port_field,
                        _ => unreachable!(),
                    };
                    target.insert_char(c);
                }
                KeyCode::Backspace if is_text_field => {
                    let target = match active_field {
                        0 => &mut endpoint_field,
                        2 => &mut interval_field,
                        3 => &mut timeout_field,
                        4 => &mut port_field,
                        _ => unreachable!(),
                    };
                    target.backspace();
                }
                KeyCode::Left if is_text_field => {
                    let target = match active_field {
                        0 => &mut endpoint_field,
                        2 => &mut interval_field,
                        3 => &mut timeout_field,
                        4 => &mut port_field,
                        _ => unreachable!(),
                    };
                    target.move_left();
                }
                KeyCode::Right if is_text_field => {
                    let target = match active_field {
                        0 => &mut endpoint_field,
                        2 => &mut interval_field,
                        3 => &mut timeout_field,
                        4 => &mut port_field,
                        _ => unreachable!(),
                    };
                    target.move_right();
                }
                KeyCode::Home if is_text_field => {
                    let target = match active_field {
                        0 => &mut endpoint_field,
                        2 => &mut interval_field,
                        3 => &mut timeout_field,
                        4 => &mut port_field,
                        _ => unreachable!(),
                    };
                    target.move_home();
                }
                KeyCode::End if is_text_field => {
                    let target = match active_field {
                        0 => &mut endpoint_field,
                        2 => &mut interval_field,
                        3 => &mut timeout_field,
                        4 => &mut port_field,
                        _ => unreachable!(),
                    };
                    target.move_end();
                }
                _ => {}
            }

            app.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
                endpoint_field,
                protocol,
                interval_field,
                timeout_field,
                port_field,
                active_field,
                error: None,
            });
        }

        ProviderSetupStep::LoadingModels { .. } => {
            // Loading state is read-only; any key besides Esc just keeps it alive.
            if key.code == KeyCode::Esc {
                app.provider_setup = None;
                app.model_loading_state = None;
            } else {
                app.provider_setup = Some(step);
            }
        }
        ProviderSetupStep::SetupRouter { .. } => {
            app.provider_setup = Some(step);
            handle_router_setup_key(app, key);
        }
        ProviderSetupStep::SelectRouterModel { .. } => {
            app.provider_setup = Some(step);
            handle_router_model_picker_key(app, key);
        }
    }
}
/// Starts the Copilot device flow and spawns a background polling task.
///
/// The device-flow start (a network call to `github.com/login/device/code`)
/// is itself offloaded to a background task so the UI thread never blocks
/// (FR-002, FR-004).  When the start completes the background task publishes
/// [`Event::CopilotDeviceFlowStartResult`]; the event handler then sets the
/// `DeviceFlowPending` dialog and spawns the polling task.  On failure the
/// event handler reverts to the key-entry form with an error.
pub(crate) fn start_copilot_device_flow_setup(app: &mut App) {
    let handle = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => {
            app.provider_setup = Some(ProviderSetupStep::EnterKey {
                provider_id: "copilot".to_string(),
                provider_name: "GitHub Copilot".to_string(),
                key_field: crate::input_field::InputField::new(),
                endpoint_field: crate::input_field::InputField::new(),
                active_field: 0,
                error: Some("Async runtime not available for device flow.".to_string()),
            });
            return;
        }
    };

    // Show the loading spinner while the device-flow start runs in the
    // background.
    app.provider_setup = Some(ProviderSetupStep::LoadingModels {
        provider_id: "copilot".to_string(),
        provider_name: "GitHub Copilot".to_string(),
    });
    app.model_loading_state = Some(crate::app::ModelLoadingState {
        provider_id: "copilot".to_string(),
        provider_name: "GitHub Copilot".to_string(),
        started_at: std::time::Instant::now(),
    });

    let event_bus = app.event_bus.clone();
    handle.spawn(async move {
        match ragent_agent::provider::copilot::start_copilot_device_flow().await {
            Ok(flow) => {
                event_bus.publish(ragent_agent::event::Event::CopilotDeviceFlowStartResult {
                    user_code: Some(flow.user_code),
                    verification_uri: Some(flow.verification_uri),
                    device_code: Some(flow.device_code),
                    interval: Some(flow.interval),
                    error: None,
                });
            }
            Err(e) => {
                event_bus.publish(ragent_agent::event::Event::CopilotDeviceFlowStartResult {
                    user_code: None,
                    verification_uri: None,
                    device_code: None,
                    interval: None,
                    error: Some(format!("{e:#}")),
                });
            }
        }
    });
}

/// Spawns an async task to validate a GitLab PAT and save credentials on success.
///
/// On completion the task publishes an `AgentError` event with the result,
/// and clears `provider_setup` (or reverts to the form with an error).
fn start_gitlab_validation(app: &mut App, instance_url: String, token: String) {
    let event_bus = app.event_bus.clone();
    let sid = app.session_id.clone().unwrap_or_default();
    let storage = app.storage.clone();

    let handle = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => {
            app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                url_input: instance_url,
                url_cursor: 0,
                token_input: token,
                token_cursor: 0,
                active_field: 0,
                error: Some("No async runtime available.".to_string()),
            });
            return;
        }
    };

    handle.spawn(async move {
        match ragent_agent::gitlab::auth::validate_token(&instance_url, &token).await {
            Ok(username) => {
                // Save token (encrypted) and config to database
                let cfg = ragent_agent::gitlab::auth::GitLabConfig {
                    instance_url: instance_url.clone(),
                    username: username.clone(),
                };
                let mut errors = Vec::new();
                if let Err(e) = ragent_agent::gitlab::auth::save_token(storage.as_ref(), &token) {
                    errors.push(format!("token save: {e}"));
                }
                if let Err(e) = ragent_agent::gitlab::auth::save_config(storage.as_ref(), &cfg) {
                    errors.push(format!("config save: {e}"));
                }
                if errors.is_empty() {
                    event_bus.publish(ragent_agent::event::Event::AgentError {
                        session_id: sid,
                        error: format!(
                            "[ok] GitLab configured successfully!\n\n\
                             **Instance**: {instance_url}\n\
                             **Username**: {username}\n\
                             **Token**: saved (encrypted)"
                        ),
                    });
                } else {
                    event_bus.publish(ragent_agent::event::Event::AgentError {
                        session_id: sid,
                        error: format!(
                            "[warn] GitLab authenticated as {username} but failed to save: {}",
                            errors.join(", ")
                        ),
                    });
                }
                // Signal the TUI to close the dialog
                event_bus.publish(ragent_agent::event::Event::GitLabSetupComplete {
                    success: errors.is_empty(),
                    error: if errors.is_empty() {
                        None
                    } else {
                        Some(errors.join(", "))
                    },
                });
            }
            Err(e) => {
                event_bus.publish(ragent_agent::event::Event::GitLabSetupComplete {
                    success: false,
                    error: Some(format!("{e}")),
                });
            }
        }
    });
}

/// Handle key events when the MCP discover dialog is active.
fn handle_mcp_discover_key(app: &mut App, key: KeyEvent) {
    match key.code {
        // Dismiss on Escape
        KeyCode::Esc => {
            app.mcp_discover = None;
        }

        // Confirm selection on Enter
        KeyCode::Enter => {
            let Some(state) = app.mcp_discover.as_mut() else {
                return;
            };
            let input = state.number_input.trim().to_string();
            if input.is_empty() {
                // Empty input = close dialog
                app.mcp_discover = None;
                return;
            }
            match input.parse::<usize>() {
                Ok(n) if n >= 1 => {
                    // Take the server (avoids borrow issues)
                    let server = app
                        .mcp_discover
                        .as_ref()
                        .and_then(|s| s.servers.get(n - 1).cloned());
                    match server {
                        Some(srv) => {
                            let result = app.enable_discovered_mcp_server(&srv);
                            if let Some(state) = app.mcp_discover.as_mut() {
                                match result {
                                    Ok(msg) => {
                                        state.feedback = Some(msg);
                                    }
                                    Err(e) => {
                                        state.feedback = Some(format!("✗ {e}"));
                                    }
                                }
                                state.number_input.clear();
                                state.number_cursor = 0;
                            }
                        }
                        None => {
                            if let Some(state) = app.mcp_discover.as_mut() {
                                let count = state.servers.len();
                                state.feedback =
                                    Some(format!("✗ Invalid number — enter 1..{count}"));
                                state.number_input.clear();
                                state.number_cursor = 0;
                            }
                        }
                    }
                }
                _ => {
                    if let Some(state) = app.mcp_discover.as_mut() {
                        let count = state.servers.len();
                        state.feedback = Some(format!("✗ Invalid number — enter 1..{count}"));
                        state.number_input.clear();
                        state.number_cursor = 0;
                    }
                }
            }
        }

        // Backspace in number input
        KeyCode::Backspace => {
            if let Some(ref mut state) = app.mcp_discover {
                if state.number_cursor > 0 {
                    let remove_pos = cursor_byte_pos(&state.number_input, state.number_cursor - 1);
                    state.number_input.remove(remove_pos);
                    state.number_cursor -= 1;
                }
            }
        }

        KeyCode::Delete => {
            if let Some(ref mut state) = app.mcp_discover
                && state.number_cursor < state.number_input.chars().count()
            {
                let remove_pos = cursor_byte_pos(&state.number_input, state.number_cursor);
                state.number_input.remove(remove_pos);
            }
        }

        KeyCode::Left => {
            if let Some(ref mut state) = app.mcp_discover {
                state.number_cursor = state.number_cursor.saturating_sub(1);
            }
        }

        KeyCode::Right => {
            if let Some(ref mut state) = app.mcp_discover {
                state.number_cursor =
                    (state.number_cursor + 1).min(state.number_input.chars().count());
            }
        }

        KeyCode::Home => {
            if let Some(ref mut state) = app.mcp_discover {
                state.number_cursor = 0;
            }
        }

        KeyCode::End => {
            if let Some(ref mut state) = app.mcp_discover {
                state.number_cursor = state.number_input.chars().count();
            }
        }

        // Digit character for number input
        KeyCode::Char(c) if c.is_ascii_digit() => {
            if let Some(ref mut state) = app.mcp_discover {
                let insert_pos = cursor_byte_pos(&state.number_input, state.number_cursor);
                state.number_input.insert(insert_pos, c);
                state.number_cursor += 1;
            }
        }

        _ => {}
    }
}

/// Handle key input when the right-click context menu is open.
///
/// Up/Down navigate items; Enter activates the highlighted item; Esc closes
/// without acting; any other key is ignored.
fn handle_context_menu_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.context_menu = None;
        }

        KeyCode::Up => {
            if let Some(ref mut menu) = app.context_menu {
                // Skip disabled items going upward.
                let count = menu.items.len();
                let mut idx = menu.selected;
                for _ in 0..count {
                    idx = (idx + count - 1) % count;
                    if menu.items[idx].1 {
                        menu.selected = idx;
                        break;
                    }
                }
            }
        }

        KeyCode::Down => {
            if let Some(ref mut menu) = app.context_menu {
                let count = menu.items.len();
                let mut idx = menu.selected;
                for _ in 0..count {
                    idx = (idx + 1) % count;
                    if menu.items[idx].1 {
                        menu.selected = idx;
                        break;
                    }
                }
            }
        }

        KeyCode::Enter => {
            if let Some(menu) = app.context_menu.clone() {
                let (action, enabled): (ContextAction, bool) = menu.items[menu.selected];
                if enabled {
                    app.execute_context_action(action);
                } else {
                    app.context_menu = None;
                }
            }
        }

        _ => {}
    }
}

/// Handle keyboard input inside the router cluster setup panel.
/// Navigation is entirely keyboard-driven: Tab switches between the provider
/// list (left) and the tier bucket columns (right). Space toggles a provider in
/// or out of the cluster palette. Enter opens the model picker for the selected
/// provider and assigns the chosen model to the active bucket. Ctrl+S saves the
/// cluster to `ragent.json`, preserving existing classifier weights/boundaries.
fn handle_router_setup_key(app: &mut App, key: KeyEvent) {
    let Some(ProviderSetupStep::SetupRouter {
        providers,
        mut selected_provider_ids,
        mut selected_provider_index,
        mut draft_config,
        mut active_bucket,
        mut active_bucket_index,
        mut left_pane_focused,
        error: _,
    }) = app.provider_setup.take()
    else {
        return;
    };

    let mut error: Option<String> = None;

    match key.code {
        KeyCode::Esc => {
            app.provider_setup = None;
            return;
        }
        KeyCode::Tab => {
            left_pane_focused = !left_pane_focused;
            active_bucket_index = 0;
        }
        KeyCode::Left if !left_pane_focused => {
            let idx = Tier::all()
                .iter()
                .position(|t| *t == active_bucket)
                .unwrap_or(0);
            active_bucket = *Tier::all()
                .iter()
                .cycle()
                .nth(idx + Tier::all().len() - 1)
                .expect("tier list non-empty");
            active_bucket_index = 0;
        }
        KeyCode::Right if !left_pane_focused => {
            let idx = Tier::all()
                .iter()
                .position(|t| *t == active_bucket)
                .unwrap_or(0);
            active_bucket = *Tier::all().get(idx + 1).unwrap_or(&Tier::Simple);
            active_bucket_index = 0;
        }
        KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) && !left_pane_focused => {
            if let Some(tier_config) = draft_config.tiers.get_mut(&active_bucket.to_string()) {
                if active_bucket_index > 0 && active_bucket_index < tier_config.models.len() {
                    tier_config
                        .models
                        .swap(active_bucket_index, active_bucket_index - 1);
                    active_bucket_index -= 1;
                }
            }
        }
        KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) && !left_pane_focused => {
            if let Some(tier_config) = draft_config.tiers.get_mut(&active_bucket.to_string()) {
                if active_bucket_index + 1 < tier_config.models.len() {
                    tier_config
                        .models
                        .swap(active_bucket_index, active_bucket_index + 1);
                    active_bucket_index += 1;
                }
            }
        }
        KeyCode::Up => {
            if left_pane_focused {
                if providers.is_empty() {
                    selected_provider_index = 0;
                } else {
                    selected_provider_index = if selected_provider_index == 0 {
                        providers.len() - 1
                    } else {
                        selected_provider_index - 1
                    };
                }
            } else {
                let models = draft_config
                    .tiers
                    .get(&active_bucket.to_string())
                    .map(|t| t.models.len())
                    .unwrap_or(0);
                if models == 0 {
                    active_bucket_index = 0;
                } else {
                    active_bucket_index = if active_bucket_index == 0 {
                        models - 1
                    } else {
                        active_bucket_index - 1
                    };
                }
            }
        }
        KeyCode::Down => {
            if left_pane_focused {
                if providers.is_empty() {
                    selected_provider_index = 0;
                } else {
                    selected_provider_index = (selected_provider_index + 1) % providers.len();
                }
            } else {
                let models = draft_config
                    .tiers
                    .get(&active_bucket.to_string())
                    .map(|t| t.models.len())
                    .unwrap_or(0);
                if models == 0 {
                    active_bucket_index = 0;
                } else {
                    active_bucket_index = (active_bucket_index + 1) % models;
                }
            }
        }
        KeyCode::Char(' ') if left_pane_focused => {
            if let Some(provider) = providers.get(selected_provider_index) {
                let id = provider.id.clone();
                if selected_provider_ids.contains(&id) {
                    selected_provider_ids.retain(|x| x != &id);
                    // Remove any existing assignments for this provider from
                    // every tier so the cluster stays consistent.
                    for tier_config in draft_config.tiers.values_mut() {
                        tier_config.models.retain(|entry| entry.provider != id);
                    }
                } else {
                    selected_provider_ids.push(id);
                }
            }
        }
        KeyCode::Enter => {
            let provider = if left_pane_focused {
                providers.get(selected_provider_index).cloned()
            } else {
                providers
                    .get(selected_provider_index)
                    .filter(|p| selected_provider_ids.contains(&p.id))
                    .cloned()
            };
            if let Some(provider) = provider {
                if !selected_provider_ids.contains(&provider.id) {
                    error = Some("Select provider with Space first".to_string());
                } else {
                    let models = app.models_for_provider(&provider.id);
                    if models.is_empty() {
                        error = Some(format!("No models available for {}", provider.name));
                    } else {
                        app.router_draft_providers = providers.clone();
                        app.router_draft_selected_ids = selected_provider_ids.clone();
                        app.router_draft_config = Some(draft_config.clone());
                        app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
                            provider_id: provider.id,
                            provider_name: provider.name,
                            models,
                            selected: 0,
                            target_tier: active_bucket,
                        });
                        return;
                    }
                }
            }
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let has_any = draft_config.tiers.values().any(|tc| !tc.models.is_empty());
            if !has_any {
                error = Some("At least one tier must contain a model".to_string());
            } else {
                app.pending_router_save = Some(draft_config.clone());
                app.provider_setup = Some(ProviderSetupStep::SetupRouter {
                    providers,
                    selected_provider_ids,
                    selected_provider_index,
                    draft_config,
                    active_bucket,
                    active_bucket_index,
                    left_pane_focused,
                    error,
                });
                return;
            }
        }
        KeyCode::Delete if !left_pane_focused => {
            if let Some(tier_config) = draft_config.tiers.get_mut(&active_bucket.to_string()) {
                if active_bucket_index < tier_config.models.len() {
                    tier_config.models.remove(active_bucket_index);
                    if active_bucket_index >= tier_config.models.len() && active_bucket_index > 0 {
                        active_bucket_index -= 1;
                    }
                }
            }
        }
        _ => {}
    }

    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
        providers,
        selected_provider_ids,
        selected_provider_index,
        draft_config,
        active_bucket,
        active_bucket_index,
        left_pane_focused,
        error,
    });
}

/// Handle keyboard input inside the router model picker sub-dialog.
/// Esc cancels and returns to the router setup panel without assigning a model.
/// Up/Down navigate the model list. Enter assigns the selected model to the
/// target tier and returns to the bucket pane.
fn handle_router_model_picker_key(app: &mut App, key: KeyEvent) {
    let Some(ProviderSetupStep::SelectRouterModel {
        provider_id,
        provider_name,
        models,
        selected,
        target_tier,
    }) = app.provider_setup.take()
    else {
        return;
    };

    match key.code {
        KeyCode::Esc => {
            // Restore the stashed router setup state, returning focus to the
            // provider palette.
            let draft_config = app.router_draft_config.take().unwrap_or_default();
            app.provider_setup = Some(ProviderSetupStep::SetupRouter {
                providers: app.router_draft_providers.clone(),
                selected_provider_ids: app.router_draft_selected_ids.clone(),
                selected_provider_index: app
                    .router_draft_providers
                    .iter()
                    .position(|p| p.id == provider_id)
                    .unwrap_or(0),
                draft_config,
                active_bucket: target_tier,
                active_bucket_index: 0,
                left_pane_focused: true,
                error: None,
            });
            app.router_draft_providers.clear();
            app.router_draft_selected_ids.clear();
        }
        KeyCode::Up => {
            let new = if models.is_empty() {
                0
            } else if selected == 0 {
                models.len() - 1
            } else {
                selected - 1
            };
            app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
                provider_id,
                provider_name,
                models,
                selected: new,
                target_tier,
            });
        }
        KeyCode::Down => {
            let new = if models.is_empty() {
                0
            } else {
                (selected + 1) % models.len()
            };
            app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
                provider_id,
                provider_name,
                models,
                selected: new,
                target_tier,
            });
        }
        KeyCode::Enter => {
            if let Some(model) = models.get(selected).cloned() {
                let mut draft_config = app.router_draft_config.take().unwrap_or_default();
                let tier_key = target_tier.to_string();
                let tier_config = draft_config.tiers.entry(tier_key).or_default();
                // Prevent recursive routing: the router must never route to itself.
                if provider_id == "router" {
                    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
                        providers: app.router_draft_providers.clone(),
                        selected_provider_ids: app.router_draft_selected_ids.clone(),
                        selected_provider_index: app
                            .router_draft_providers
                            .iter()
                            .position(|p| p.id == provider_id)
                            .unwrap_or(0),
                        draft_config,
                        active_bucket: target_tier,
                        active_bucket_index: 0,
                        left_pane_focused: true,
                        error: Some("The router cannot route to itself".to_string()),
                    });
                    app.router_draft_providers.clear();
                    app.router_draft_selected_ids.clear();
                    return;
                }

                // Avoid duplicate exact provider/model pairs in the same tier.
                let already_present = tier_config
                    .models
                    .iter()
                    .any(|entry| entry.provider == provider_id && entry.model == model.id);
                if !already_present {
                    tier_config
                        .models
                        .push(ragent_llm::providers::router_config::TierEntry {
                            provider: provider_id.clone(),
                            model: model.id.clone(),
                        });
                }
                let new_bucket_index = tier_config.models.len().saturating_sub(1);
                app.provider_setup = Some(ProviderSetupStep::SetupRouter {
                    providers: app.router_draft_providers.clone(),
                    selected_provider_ids: app.router_draft_selected_ids.clone(),
                    selected_provider_index: app
                        .router_draft_providers
                        .iter()
                        .position(|p| p.id == provider_id)
                        .unwrap_or(0),
                    draft_config,
                    active_bucket: target_tier,
                    active_bucket_index: new_bucket_index,
                    left_pane_focused: false,
                    error: None,
                });
                app.router_draft_providers.clear();
                app.router_draft_selected_ids.clear();
            } else {
                app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
                    provider_id,
                    provider_name,
                    models,
                    selected,
                    target_tier,
                });
            }
        }
        _ => {
            app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
                provider_id,
                provider_name,
                models,
                selected,
                target_tier,
            });
        }
    }
}

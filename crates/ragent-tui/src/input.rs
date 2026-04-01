//! Keyboard input handling for the TUI.
//!
//! Maps terminal key events to high-level [`InputAction`]s, handling both
//! normal editing mode and the permission dialog intercept.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, ConfiguredProvider, ContextAction, ProviderSource, PROVIDER_LIST, ProviderSetupStep};

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
    /// Cycle to the next configured agent.
    SwitchAgent,
    /// Execute a `/`-prefixed command.
    SlashCommand(String),
    /// Cancel the currently running agent (ESC while processing).
    CancelAgent,
    /// Confirm a pending forcecleanup modal (Enter -> confirm).
    ConfirmForceCleanup,
    /// Cancel a pending forcecleanup modal (Esc -> cancel).
    CancelForceCleanup,
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

    // If provider setup dialog is active, route all keys there
    if app.provider_setup.is_some() {
        handle_provider_setup_key(app, key);
        return None;
    }

    // If LSP discover dialog is active, route all keys there
    if app.lsp_discover.is_some() {
        handle_lsp_discover_key(app, key);
        return None;
    }

    // If MCP discover dialog is active, route all keys there
    if app.mcp_discover.is_some() {
        handle_mcp_discover_key(app, key);
        return None;
    }

    // If permission dialog is active, intercept keys
    if app.permission_pending.is_some() {
        return match key.code {
            KeyCode::Char('y') => {
                if let Some(ref req) = app.permission_pending {
                    app.event_bus
                        .publish(ragent_core::event::Event::PermissionReplied {
                            session_id: req.session_id.clone(),
                            request_id: req.id.clone(),
                            allowed: true,
                        });
                }
                None
            }
            KeyCode::Char('a') => {
                if let Some(ref req) = app.permission_pending {
                    app.event_bus
                        .publish(ragent_core::event::Event::PermissionReplied {
                            session_id: req.session_id.clone(),
                            request_id: req.id.clone(),
                            allowed: true,
                        });
                }
                None
            }
            KeyCode::Char('n') => {
                if let Some(ref req) = app.permission_pending {
                    app.event_bus
                        .publish(ragent_core::event::Event::PermissionReplied {
                            session_id: req.session_id.clone(),
                            request_id: req.id.clone(),
                            allowed: false,
                        });
                }
                None
            }
            _ => None,
        };
    }

    // If a forcecleanup confirmation modal is active, intercept Enter/Esc
    if app.pending_forcecleanup.is_some() {
        match key.code {
            KeyCode::Enter => return Some(InputAction::ConfirmForceCleanup),
            KeyCode::Esc => return Some(InputAction::CancelForceCleanup),
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
                // If the user typed more than just the trigger (e.g. "/lsp discover"
                // vs. "/lsp"), preserve the full input so subcommands are not lost.
                let command = if let Some(ref menu) = app.slash_menu {
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
                            entry.trigger.clone()
                        }
                    } else {
                        menu.filter.clone()
                    }
                } else {
                    return None;
                };
                return Some(InputAction::SlashCommand(command));
            }
            KeyCode::Esc => {
                app.input.clear();
                app.slash_menu = None;
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
                if let Some(ref mut menu) = app.file_menu && !menu.matches.is_empty() {
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
                if let Some(ref mut menu) = app.file_menu && !menu.matches.is_empty() {
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
        KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
            // Shift+Enter: insert a literal newline without sending.
            app.clear_kb_selection();
            app.insert_char_at_cursor('\n');
            None
        }
        KeyCode::Enter => {
            let text = app.input.clone();
            if text.is_empty() {
                return None;
            }
            if text.starts_with('/') {
                return Some(InputAction::SlashCommand(text));
            }
            Some(InputAction::SendMessage(text))
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Copy if a keyboard selection is active; otherwise arm quit.
            if app.kb_select_anchor.is_some() {
                Some(InputAction::CopyToClipboard)
            } else {
                Some(InputAction::Quit)
            }
        }
        KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::CutToClipboard)
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
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(InputAction::ConfirmQuit)
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
        KeyCode::Char(c) => {
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
        KeyCode::Delete => Some(InputAction::Delete),
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
    // Escape always closes the dialog
    if key.code == KeyCode::Esc {
        app.provider_setup = None;
        return;
    }

    let Some(step) = app.provider_setup.take() else {
        return;
    };

    match step {
        ProviderSetupStep::SelectProvider { selected } => match key.code {
            KeyCode::Up => {
                let new = if selected == 0 {
                    PROVIDER_LIST.len() - 1
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::SelectProvider { selected: new });
            }
            KeyCode::Down => {
                let new = (selected + 1) % PROVIDER_LIST.len();
                app.provider_setup = Some(ProviderSetupStep::SelectProvider { selected: new });
            }
            KeyCode::Enter => {
                let (pid, pname) = PROVIDER_LIST[selected];
                if pid == "ollama" {
                    // Ollama doesn't require a key — store empty and mark configured
                    let _ = app.storage.set_provider_auth(pid, "");
                    let _ = app
                        .storage
                        .delete_setting(&format!("provider_{pid}_disabled"));
                    app.refresh_provider();
                    let models = app.models_for_provider(pid);
                    app.provider_setup = Some(ProviderSetupStep::SelectModel {
                        provider_id: pid.to_string(),
                        provider_name: pname.to_string(),
                        models,
                        selected: 0,
                    });
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
                    let token = ragent_core::provider::copilot::resolve_copilot_github_token(Some(
                        &db_lookup,
                    ));
                    if let Some(ref tk) = token {
                        // Try token exchange to check if we have a working token
                        if let Ok(handle) = tokio::runtime::Handle::try_current() {
                            let tk_clone = tk.clone();
                            let exchange_ok = tokio::task::block_in_place(|| {
                                handle.block_on(
                                    ragent_core::provider::copilot::resolve_copilot_auth(
                                        &tk_clone, None,
                                    ),
                                )
                            });
                            if let Ok(auth) = exchange_ok {
                                if !auth.base_url.contains("models.inference.ai.azure.com") {
                                    let _ =
                                        app.storage.set_setting("copilot_api_base", &auth.base_url);
                                    let _ = app.storage.delete_setting("provider_copilot_disabled");
                                    app.refresh_provider();
                                    let models = app.models_for_provider(pid);
                                    app.provider_setup = Some(ProviderSetupStep::SelectModel {
                                        provider_id: pid.to_string(),
                                        provider_name: pname.to_string(),
                                        models,
                                        selected: 0,
                                    });
                                    return;
                                }
                            }
                        }
                    }
                    // Token exchange failed or no token — start device flow
                    start_copilot_device_flow_setup(app);
                } else {
                    app.provider_setup = Some(ProviderSetupStep::EnterKey {
                        provider_id: pid.to_string(),
                        provider_name: pname.to_string(),
                        key_input: String::new(),
                        key_cursor: 0,
                        endpoint_input: app
                            .storage
                            .get_setting("generic_openai_api_base")
                            .ok()
                            .flatten()
                            .unwrap_or_default(),
                        endpoint_cursor: 0,
                        editing_endpoint: false,
                        error: None,
                    });
                }
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::SelectProvider { selected });
            }
        },
        ProviderSetupStep::EnterKey {
            provider_id,
            provider_name,
            mut key_input,
            mut key_cursor,
            mut endpoint_input,
            mut endpoint_cursor,
            mut editing_endpoint,
            ..
        } => match key.code {
            KeyCode::Enter => {
                let trimmed = key_input.trim().to_string();
                if trimmed.is_empty() {
                    app.provider_setup = Some(ProviderSetupStep::EnterKey {
                        provider_id,
                        provider_name,
                        key_input,
                        key_cursor,
                        endpoint_input,
                        endpoint_cursor,
                        editing_endpoint,
                        error: Some("API key cannot be empty.".to_string()),
                    });
                } else if provider_id == "copilot"
                    && ragent_core::provider::copilot::is_pat_token(&trimmed)
                {
                    app.provider_setup = Some(ProviderSetupStep::EnterKey {
                        provider_id,
                        provider_name,
                        key_input,
                        key_cursor,
                        endpoint_input,
                        endpoint_cursor,
                        editing_endpoint,
                        error: Some(
                            "PATs (github_pat_/ghp_) are not supported by \
                             the Copilot API. Run: gh auth refresh -s copilot"
                                .to_string(),
                        ),
                    });
                } else {
                    let _ = app.storage.set_provider_auth(&provider_id, &trimmed);
                    if provider_id == "generic_openai" {
                        let endpoint = endpoint_input.trim();
                        if endpoint.is_empty() {
                            let _ = app.storage.delete_setting("generic_openai_api_base");
                        } else {
                            let _ = app.storage.set_setting("generic_openai_api_base", endpoint);
                        }
                    }
                    let _ = app
                        .storage
                        .delete_setting(&format!("provider_{provider_id}_disabled"));
                    app.refresh_provider();
                    let models = app.models_for_provider(&provider_id);
                    app.provider_setup = Some(ProviderSetupStep::SelectModel {
                        provider_id,
                        provider_name,
                        models,
                        selected: 0,
                    });
                }
            }
            KeyCode::Tab if provider_id == "generic_openai" => {
                editing_endpoint = !editing_endpoint;
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_input,
                    key_cursor,
                    endpoint_input,
                    endpoint_cursor,
                    editing_endpoint,
                    error: None,
                });
            }
            KeyCode::Char(c) => {
                if provider_id == "generic_openai" && editing_endpoint {
                    let insert_pos = cursor_byte_pos(&endpoint_input, endpoint_cursor);
                    endpoint_input.insert(insert_pos, c);
                    endpoint_cursor += 1;
                } else {
                    let insert_pos = cursor_byte_pos(&key_input, key_cursor);
                    key_input.insert(insert_pos, c);
                    key_cursor += 1;
                }
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_input,
                    key_cursor,
                    endpoint_input,
                    endpoint_cursor,
                    editing_endpoint,
                    error: None,
                });
            }
            KeyCode::Backspace => {
                if provider_id == "generic_openai" && editing_endpoint {
                    if endpoint_cursor > 0 {
                        let remove_pos = cursor_byte_pos(&endpoint_input, endpoint_cursor - 1);
                        endpoint_input.remove(remove_pos);
                        endpoint_cursor -= 1;
                    }
                } else {
                    if key_cursor > 0 {
                        let remove_pos = cursor_byte_pos(&key_input, key_cursor - 1);
                        key_input.remove(remove_pos);
                        key_cursor -= 1;
                    }
                }
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_input,
                    key_cursor,
                    endpoint_input,
                    endpoint_cursor,
                    editing_endpoint,
                    error: None,
                });
            }
            KeyCode::Delete => {
                if provider_id == "generic_openai" && editing_endpoint {
                    if endpoint_cursor < endpoint_input.chars().count() {
                        let remove_pos = cursor_byte_pos(&endpoint_input, endpoint_cursor);
                        endpoint_input.remove(remove_pos);
                    }
                } else if key_cursor < key_input.chars().count() {
                    let remove_pos = cursor_byte_pos(&key_input, key_cursor);
                    key_input.remove(remove_pos);
                }
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_input,
                    key_cursor,
                    endpoint_input,
                    endpoint_cursor,
                    editing_endpoint,
                    error: None,
                });
            }
            KeyCode::Left => {
                if provider_id == "generic_openai" && editing_endpoint {
                    endpoint_cursor = endpoint_cursor.saturating_sub(1);
                } else {
                    key_cursor = key_cursor.saturating_sub(1);
                }
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_input,
                    key_cursor,
                    endpoint_input,
                    endpoint_cursor,
                    editing_endpoint,
                    error: None,
                });
            }
            KeyCode::Right => {
                if provider_id == "generic_openai" && editing_endpoint {
                    endpoint_cursor = (endpoint_cursor + 1).min(endpoint_input.chars().count());
                } else {
                    key_cursor = (key_cursor + 1).min(key_input.chars().count());
                }
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_input,
                    key_cursor,
                    endpoint_input,
                    endpoint_cursor,
                    editing_endpoint,
                    error: None,
                });
            }
            KeyCode::Home => {
                if provider_id == "generic_openai" && editing_endpoint {
                    endpoint_cursor = 0;
                } else {
                    key_cursor = 0;
                }
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_input,
                    key_cursor,
                    endpoint_input,
                    endpoint_cursor,
                    editing_endpoint,
                    error: None,
                });
            }
            KeyCode::End => {
                if provider_id == "generic_openai" && editing_endpoint {
                    endpoint_cursor = endpoint_input.chars().count();
                } else {
                    key_cursor = key_input.chars().count();
                }
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_input,
                    key_cursor,
                    endpoint_input,
                    endpoint_cursor,
                    editing_endpoint,
                    error: None,
                });
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_input,
                    key_cursor,
                    endpoint_input,
                    endpoint_cursor,
                    editing_endpoint,
                    error: None,
                });
            }
        },
        ProviderSetupStep::DeviceFlowPending {
            user_code,
            verification_uri,
        } => match key.code {
            KeyCode::Esc => {
                app.provider_setup = None;
            }
            KeyCode::Char('c') => {
                let code = user_code.clone();
                std::thread::spawn(move || {
                    let mut cb = match arboard::Clipboard::new() {
                        Ok(cb) => cb,
                        Err(_) => return,
                    };
                    #[cfg(target_os = "linux")]
                    {
                        use arboard::SetExtLinux;
                        let _ = cb.set().wait().text(code);
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        let _ = cb.set_text(code);
                    }
                });
                app.status = "✔ Device code copied to clipboard".to_string();
                app.provider_setup = Some(ProviderSetupStep::DeviceFlowPending {
                    user_code,
                    verification_uri,
                });
            }
            _ => {
                // Keep showing the device flow pending UI — polling happens
                // in a background task and completes via CopilotDeviceFlowComplete event.
                app.provider_setup = Some(ProviderSetupStep::DeviceFlowPending {
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
                let model_name = if let Some((mid, mname)) = models.get(selected) {
                    let model_value = format!("{}/{}", provider_id, mid);
                    let _ = app.storage.set_setting("selected_model", &model_value);
                    // Persist the user's explicit provider choice so it survives restarts.
                    let _ = app.storage.set_setting("preferred_provider", &provider_id);
                    app.selected_model = Some(model_value);
                    // Ensure the UI reflects the provider the user just chose.
                    app.configured_provider = Some(ConfiguredProvider {
                        id: provider_id.clone(),
                        name: provider_name.clone(),
                        source: ProviderSource::Database,
                    });
                    Some(mname.clone())
                } else {
                    None
                };
                app.provider_setup = Some(ProviderSetupStep::Done {
                    provider_name,
                    model_name,
                });
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
                if pid == "copilot" {
                    let _ = app.storage.delete_setting("copilot_api_base");
                } else if pid == "generic_openai" {
                    let _ = app.storage.delete_setting("generic_openai_api_base");
                }
                let is_active = app
                    .configured_provider
                    .as_ref()
                    .map_or(false, |p| p.id == pid);
                if is_active {
                    app.configured_provider = None;
                    app.selected_model = None;
                    let _ = app.storage.delete_setting("selected_model");
                    app.provider_health
                        .store(0, std::sync::atomic::Ordering::Relaxed);
                }
                app.status = format!("✔ Provider {} reset — credentials removed", pname);
                app.provider_setup = None;
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::ResetProvider { selected });
            }
        },
    }
}

/// Starts the Copilot device flow and spawns a background polling task.
///
/// On success the polling task publishes [`Event::CopilotDeviceFlowComplete`]
/// which the app event handler picks up to finish the setup.
fn start_copilot_device_flow_setup(app: &mut App) {
    let handle = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => {
            app.provider_setup = Some(ProviderSetupStep::EnterKey {
                provider_id: "copilot".to_string(),
                provider_name: "GitHub Copilot".to_string(),
                key_input: String::new(),
                key_cursor: 0,
                endpoint_input: String::new(),
                endpoint_cursor: 0,
                editing_endpoint: false,
                error: Some("Async runtime not available for device flow.".to_string()),
            });
            return;
        }
    };

    // Initiate the device flow (blocking briefly)
    let start = tokio::task::block_in_place(|| {
        handle.block_on(ragent_core::provider::copilot::start_copilot_device_flow())
    });

    let flow = match start {
        Ok(f) => f,
        Err(e) => {
            app.provider_setup = Some(ProviderSetupStep::EnterKey {
                provider_id: "copilot".to_string(),
                provider_name: "GitHub Copilot".to_string(),
                key_input: String::new(),
                key_cursor: 0,
                endpoint_input: String::new(),
                endpoint_cursor: 0,
                editing_endpoint: false,
                error: Some(format!("Device flow failed: {e}")),
            });
            return;
        }
    };

    let user_code = flow.user_code.clone();
    let verification_uri = flow.verification_uri.clone();
    let device_code = flow.device_code.clone();
    let interval = std::time::Duration::from_secs(flow.interval.max(5));
    let event_bus = app.event_bus.clone();

    app.push_log(
        crate::app::LogLevel::Info,
        format!(
            "Copilot device flow started — enter code {} at {}",
            user_code, verification_uri
        ),
    );

    app.provider_setup = Some(ProviderSetupStep::DeviceFlowPending {
        user_code,
        verification_uri,
    });

    // Background task: poll until authorised or expired
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            match ragent_core::provider::copilot::poll_copilot_device_flow(&device_code).await {
                Ok(Some(token)) => {
                    // Discover the plan API base.
                    // The device flow token may lack scope for copilot_internal/user,
                    // so also try the gh CLI token which has broader scope.
                    let api_base = {
                        let mut base =
                            ragent_core::provider::copilot::discover_copilot_api_base(&token).await;
                        if base.is_none() {
                            if let Some(gh_token) =
                                ragent_core::provider::copilot::find_gh_cli_token()
                            {
                                base = ragent_core::provider::copilot::discover_copilot_api_base(
                                    &gh_token,
                                )
                                .await;
                            }
                        }
                        base.unwrap_or_else(|| "https://api.githubcopilot.com".to_string())
                    };
                    event_bus.publish(ragent_core::event::Event::CopilotDeviceFlowComplete {
                        token,
                        api_base,
                    });
                    break;
                }
                Ok(None) => {
                    // Still waiting — keep polling
                    continue;
                }
                Err(_) => {
                    // Expired or denied — give up silently (user can Esc)
                    break;
                }
            }
        }
    });
}

/// Handle key events when the LSP discover dialog is active.
fn handle_lsp_discover_key(app: &mut App, key: KeyEvent) {
    match key.code {
        // Dismiss on Escape
        KeyCode::Esc => {
            app.lsp_discover = None;
        }

        // Confirm selection on Enter
        KeyCode::Enter => {
            let Some(state) = app.lsp_discover.as_mut() else { return };
            let input = state.number_input.trim().to_string();
            if input.is_empty() {
                // Empty input = close dialog
                app.lsp_discover = None;
                return;
            }
            match input.parse::<usize>() {
                Ok(n) if n >= 1 => {
                    // Take the server (avoids borrow issues)
                    let server = app
                        .lsp_discover
                        .as_ref()
                        .and_then(|s| s.servers.get(n - 1).cloned());
                    match server {
                        Some(srv) => {
                            let result = app.enable_discovered_server(&srv);
                            if let Some(state) = app.lsp_discover.as_mut() {
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
                            if let Some(state) = app.lsp_discover.as_mut() {
                                let count = state.servers.len();
                                state.feedback = Some(format!("✗ Invalid number — enter 1..{count}"));
                                state.number_input.clear();
                                state.number_cursor = 0;
                            }
                        }
                    }
                }
                _ => {
                    if let Some(state) = app.lsp_discover.as_mut() {
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
            if let Some(ref mut state) = app.lsp_discover {
                if state.number_cursor > 0 {
                    let remove_pos = cursor_byte_pos(&state.number_input, state.number_cursor - 1);
                    state.number_input.remove(remove_pos);
                    state.number_cursor -= 1;
                }
            }
        }

        KeyCode::Delete => {
            if let Some(ref mut state) = app.lsp_discover
                && state.number_cursor < state.number_input.chars().count()
            {
                let remove_pos = cursor_byte_pos(&state.number_input, state.number_cursor);
                state.number_input.remove(remove_pos);
            }
        }

        KeyCode::Left => {
            if let Some(ref mut state) = app.lsp_discover {
                state.number_cursor = state.number_cursor.saturating_sub(1);
            }
        }

        KeyCode::Right => {
            if let Some(ref mut state) = app.lsp_discover {
                state.number_cursor = (state.number_cursor + 1).min(state.number_input.chars().count());
            }
        }

        KeyCode::Home => {
            if let Some(ref mut state) = app.lsp_discover {
                state.number_cursor = 0;
            }
        }

        KeyCode::End => {
            if let Some(ref mut state) = app.lsp_discover {
                state.number_cursor = state.number_input.chars().count();
            }
        }

        // Digit character for number input
        KeyCode::Char(c) if c.is_ascii_digit() => {
            if let Some(ref mut state) = app.lsp_discover {
                let insert_pos = cursor_byte_pos(&state.number_input, state.number_cursor);
                state.number_input.insert(insert_pos, c);
                state.number_cursor += 1;
            }
        }

        _ => {}
    }
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
            let Some(state) = app.mcp_discover.as_mut() else { return };
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
                                state.feedback = Some(format!("✗ Invalid number — enter 1..{count}"));
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
                state.number_cursor = (state.number_cursor + 1).min(state.number_input.chars().count());
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

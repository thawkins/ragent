//! Keyboard input handling for the TUI.
//!
//! Maps terminal key events to high-level [`InputAction`]s, handling both
//! normal editing mode and the permission dialog intercept.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, ProviderSetupStep, ScreenMode, PROVIDER_LIST, SLASH_COMMANDS};

/// A high-level action produced by interpreting a key event.
#[derive(Debug)]
pub enum InputAction {
    /// Submit the input buffer as a user message.
    SendMessage(String),
    /// Exit the application.
    Quit,
    /// Scroll the message view upward.
    ScrollUp,
    /// Scroll the message view downward.
    ScrollDown,
    /// Recall the previous entry from input history.
    HistoryUp,
    /// Recall the next entry from input history.
    HistoryDown,
    /// Cycle to the next configured agent.
    SwitchAgent,
    /// Execute a `/`-prefixed command.
    SlashCommand(String),
}

/// Translate a [`KeyEvent`] into an optional [`InputAction`].
///
/// When a permission dialog is active, only `y` / `a` / `n` keys are handled
/// (publishing a permission reply). When the provider setup dialog is active,
/// keys are routed to the dialog. When the slash-command menu is active,
/// arrow keys navigate and Enter selects. Otherwise normal editing and
/// navigation keys are processed.
pub fn handle_key(app: &mut App, key: KeyEvent) -> Option<InputAction> {
    // If provider setup dialog is active, route all keys there
    if app.provider_setup.is_some() {
        handle_provider_setup_key(app, key);
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

    // On the home screen, 'p' opens the provider setup dialog
    if app.current_screen == ScreenMode::Home && key.code == KeyCode::Char('p') {
        app.provider_setup = Some(ProviderSetupStep::SelectProvider { selected: 0 });
        return None;
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
                // Select the highlighted command, or use the typed text
                let trigger = if let Some(ref menu) = app.slash_menu {
                    if let Some(&idx) = menu.matches.get(menu.selected) {
                        SLASH_COMMANDS[idx].trigger.to_string()
                    } else {
                        menu.filter.clone()
                    }
                } else {
                    return None;
                };
                return Some(InputAction::SlashCommand(trigger));
            }
            KeyCode::Esc => {
                app.input.clear();
                app.slash_menu = None;
                return None;
            }
            KeyCode::Char(c) => {
                app.input.push(c);
                app.update_slash_menu();
                return None;
            }
            KeyCode::Backspace => {
                app.input.pop();
                app.update_slash_menu();
                return None;
            }
            _ => return None,
        }
    }

    match key.code {
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
            Some(InputAction::Quit)
        }
        KeyCode::Char(c) => {
            app.input.push(c);
            // If the input now starts with '/', show the slash menu
            if app.input.starts_with('/') {
                app.update_slash_menu();
            }
            None
        }
        KeyCode::Backspace => {
            app.input.pop();
            // Update or close the slash menu
            if app.input.starts_with('/') {
                app.update_slash_menu();
            } else {
                app.slash_menu = None;
            }
            None
        }
        KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => Some(InputAction::ScrollUp),
        KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
            Some(InputAction::ScrollDown)
        }
        KeyCode::Up => Some(InputAction::HistoryUp),
        KeyCode::Down => Some(InputAction::HistoryDown),
        KeyCode::PageUp => Some(InputAction::ScrollUp),
        KeyCode::PageDown => Some(InputAction::ScrollDown),
        KeyCode::Tab | KeyCode::BackTab => Some(InputAction::SwitchAgent),
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

    let step = app.provider_setup.take().unwrap();

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
                    app.refresh_provider();
                    let models = app.models_for_provider(pid);
                    app.provider_setup = Some(ProviderSetupStep::SelectModel {
                        provider_id: pid.to_string(),
                        provider_name: pname.to_string(),
                        models,
                        selected: 0,
                    });
                } else if pid == "copilot" {
                    // Copilot: try auto-discover first
                    if ragent_core::provider::copilot::find_copilot_token().is_some() {
                        app.refresh_provider();
                        let models = app.models_for_provider(pid);
                        app.provider_setup = Some(ProviderSetupStep::SelectModel {
                            provider_id: pid.to_string(),
                            provider_name: pname.to_string(),
                            models,
                            selected: 0,
                        });
                    } else {
                        app.provider_setup = Some(ProviderSetupStep::EnterKey {
                            provider_id: pid.to_string(),
                            provider_name: pname.to_string(),
                            key_input: String::new(),
                            error: Some(
                                "No Copilot IDE config found. Paste a GITHUB_COPILOT_TOKEN:"
                                    .to_string(),
                            ),
                        });
                    }
                } else {
                    app.provider_setup = Some(ProviderSetupStep::EnterKey {
                        provider_id: pid.to_string(),
                        provider_name: pname.to_string(),
                        key_input: String::new(),
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
            ..
        } => match key.code {
            KeyCode::Enter => {
                let trimmed = key_input.trim().to_string();
                if trimmed.is_empty() {
                    app.provider_setup = Some(ProviderSetupStep::EnterKey {
                        provider_id,
                        provider_name,
                        key_input,
                        error: Some("API key cannot be empty.".to_string()),
                    });
                } else {
                    let _ = app.storage.set_provider_auth(&provider_id, &trimmed);
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
            KeyCode::Char(c) => {
                key_input.push(c);
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_input,
                    error: None,
                });
            }
            KeyCode::Backspace => {
                key_input.pop();
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_input,
                    error: None,
                });
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_input,
                    error: None,
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
                    app.selected_model = Some(model_value);
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
                app.provider_setup = Some(ProviderSetupStep::SelectAgent { agents, selected: new });
            }
            KeyCode::Down => {
                let new = if agents.is_empty() {
                    0
                } else {
                    (selected + 1) % agents.len()
                };
                app.provider_setup = Some(ProviderSetupStep::SelectAgent { agents, selected: new });
            }
            KeyCode::Enter => {
                if let Some((name, _desc)) = agents.get(selected) {
                    if let Some(idx) = app
                        .cycleable_agents
                        .iter()
                        .position(|a| a.name == *name)
                    {
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
    }
}

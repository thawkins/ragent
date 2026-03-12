//! Keyboard input handling for the TUI.
//!
//! Maps terminal key events to high-level [`InputAction`]s, handling both
//! normal editing mode and the permission dialog intercept.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, PROVIDER_LIST, ProviderSetupStep, SLASH_COMMANDS};

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
    /// Scroll the log panel upward.
    LogScrollUp,
    /// Scroll the log panel downward.
    LogScrollDown,
    /// Recall the previous entry from input history.
    HistoryUp,
    /// Recall the next entry from input history.
    HistoryDown,
    /// Cycle to the next configured agent.
    SwitchAgent,
    /// Execute a `/`-prefixed command.
    SlashCommand(String),
    /// Cancel the currently running agent (ESC while processing).
    CancelAgent,
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
                } else if provider_id == "copilot"
                    && ragent_core::provider::copilot::is_pat_token(&trimmed)
                {
                    app.provider_setup = Some(ProviderSetupStep::EnterKey {
                        provider_id,
                        provider_name,
                        key_input,
                        error: Some(
                            "PATs (github_pat_/ghp_) are not supported by \
                             the Copilot API. Run: gh auth refresh -s copilot"
                                .to_string(),
                        ),
                    });
                } else {
                    let _ = app.storage.set_provider_auth(&provider_id, &trimmed);
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
                if let Some((name, _desc)) = agents.get(selected) {
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

//! Keyboard input handling for the TUI.
//!
//! Maps terminal key events to high-level [`InputAction`]s, handling both
//! normal editing mode and the permission dialog intercept.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;

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
    /// Cycle to the next configured agent.
    SwitchAgent,
    /// Execute a `/`-prefixed command.
    SlashCommand(String),
}

/// Translate a [`KeyEvent`] into an optional [`InputAction`].
///
/// When a permission dialog is active, only `y` / `a` / `n` keys are handled
/// (publishing a permission reply). Otherwise normal editing and navigation
/// keys are processed.
pub fn handle_key(app: &mut App, key: KeyEvent) -> Option<InputAction> {
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
            None
        }
        KeyCode::Backspace => {
            app.input.pop();
            None
        }
        KeyCode::Up => Some(InputAction::ScrollUp),
        KeyCode::Down => Some(InputAction::ScrollDown),
        KeyCode::Tab => Some(InputAction::SwitchAgent),
        _ => None,
    }
}

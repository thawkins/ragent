use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;

#[derive(Debug)]
pub enum InputAction {
    SendMessage(String),
    Quit,
    ScrollUp,
    ScrollDown,
    SwitchAgent,
    SlashCommand(String),
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> Option<InputAction> {
    // If permission dialog is active, intercept keys
    if app.permission_pending.is_some() {
        return match key.code {
            KeyCode::Char('y') => {
                if let Some(ref req) = app.permission_pending {
                    app.event_bus.publish(ragent_core::event::Event::PermissionReplied {
                        session_id: req.session_id.clone(),
                        request_id: req.id.clone(),
                        allowed: true,
                    });
                }
                None
            }
            KeyCode::Char('a') => {
                if let Some(ref req) = app.permission_pending {
                    app.event_bus.publish(ragent_core::event::Event::PermissionReplied {
                        session_id: req.session_id.clone(),
                        request_id: req.id.clone(),
                        allowed: true,
                    });
                }
                None
            }
            KeyCode::Char('n') => {
                if let Some(ref req) = app.permission_pending {
                    app.event_bus.publish(ragent_core::event::Event::PermissionReplied {
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

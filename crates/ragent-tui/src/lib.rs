//! Terminal user interface for ragent.
//!
//! Provides a ratatui-based interactive TUI that displays agent messages,
//! tool call status, permission dialogs, and a text input prompt. The TUI
//! reacts to real-time events from the ragent [`EventBus`](ragent_core::event::EventBus).

pub mod app;
pub mod input;
pub mod layout;
pub mod widgets;

pub use app::App;

use std::sync::Arc;

use anyhow::Result;
use crossterm::{
    event::{self as ct_event, DisableMouseCapture, EnableMouseCapture, Event as CtEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio_stream::wrappers::BroadcastStream;

use ragent_core::event::EventBus;

/// Run the TUI application.
///
/// Enters the alternate screen, creates an [`App`], and runs the main event
/// loop until the user quits. The terminal is restored on exit.
///
/// # Errors
///
/// Returns an error if terminal setup, drawing, or event reading fails.
pub async fn run_tui(event_bus: Arc<EventBus>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(event_bus.clone());
    let mut bus_stream = BroadcastStream::new(event_bus.subscribe());

    while app.is_running {
        terminal.draw(|frame| layout::render(frame, &app))?;

        tokio::select! {
            // Terminal key/mouse events (polled at 50ms intervals)
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                while ct_event::poll(std::time::Duration::ZERO)? {
                    if let CtEvent::Key(key) = ct_event::read()? {
                        app.handle_key_event(key);
                    }
                }
            }
            // Events from the agent event bus
            Some(Ok(event)) = bus_stream.next() => {
                app.handle_event(event);
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

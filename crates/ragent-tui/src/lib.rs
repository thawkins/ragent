//! Terminal user interface for ragent.
//!
//! Provides a ratatui-based interactive TUI that displays agent messages,
//! tool call status, permission dialogs, and a text input prompt. The TUI
//! reacts to real-time events from the ragent [`EventBus`](ragent_core::event::EventBus).

pub mod app;
pub mod input;
pub mod layout;
pub mod layout_active_agents;
pub mod logo;
pub mod tips;
pub mod widgets;

pub use app::App;

use std::sync::Arc;

use anyhow::Result;
use crossterm::{
    event::{self as ct_event, DisableMouseCapture, EnableMouseCapture, Event as CtEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use ragent_core::agent::AgentInfo;
use ragent_core::config::Config;
use ragent_core::event::EventBus;
use ragent_core::lsp::{LspManager, SharedLspManager};
use ragent_core::provider::ProviderRegistry;
use ragent_core::session::processor::SessionProcessor;
use ragent_core::storage::Storage;

/// Run the TUI application.
///
/// Enters the alternate screen, creates an [`App`], and runs the main event
/// loop until the user quits. The terminal is restored on exit.
///
/// If `resume_session_id` is provided, the TUI loads the existing session
/// and its message history before entering the event loop.
///
/// # Errors
///
/// Returns an error if terminal setup, drawing, or event reading fails.
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
/// # async fn example(
/// #     bus: Arc<EventBus>,
/// #     storage: Arc<Storage>,
/// #     registry: Arc<ProviderRegistry>,
/// #     processor: Arc<SessionProcessor>,
/// # ) -> anyhow::Result<()> {
/// let agent = AgentInfo::new("general", "General-purpose agent");
/// ragent_tui::run_tui(bus, storage, registry, processor, agent, false, None).await?;
/// # Ok(())
/// # }
/// ```
pub async fn run_tui(
    event_bus: Arc<EventBus>,
    storage: Arc<Storage>,
    provider_registry: Arc<ProviderRegistry>,
    session_processor: Arc<SessionProcessor>,
    agent: AgentInfo,
    show_log: bool,
    resume_session_id: Option<String>,
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(
        event_bus.clone(),
        storage,
        provider_registry,
        session_processor.clone(),
        agent,
        show_log,
    );
    app.check_provider_health();

    // Subscribe to the event bus BEFORE starting LSP so no status events are dropped.
    let mut bus_rx = event_bus.subscribe();

    // ── LSP startup ───────────────────────────────────────────────────────────
    // Create the LspManager, start configured servers, and wire events into
    // the app. This runs asynchronously; status changes propagate via events.
    let lsp_manager: SharedLspManager = {
        let cwd = std::env::current_dir().unwrap_or_default();
        let mgr = LspManager::new(cwd, event_bus.clone());
        Arc::new(tokio::sync::RwLock::new(mgr))
    };
    {
        let mut mgr = lsp_manager.write().await;
        let lsp_configs = Config::load()
            .map(|c| c.lsp)
            .unwrap_or_default();
        if !lsp_configs.is_empty() {
            mgr.connect_all(lsp_configs).await;
        }
        // Populate the initial server snapshot in app.
        app.lsp_servers = mgr.servers().to_vec();
    }
    app.set_lsp_manager(lsp_manager.clone());
    // Also wire into the session processor so LSP tools can access the manager.
    let _ = session_processor.lsp_manager.set(lsp_manager);

    if let Some(ref sid) = resume_session_id {
        if let Err(e) = app.load_session(sid) {
            tracing::error!(error = %e, session_id = %sid, "Failed to resume session");
        }
    }

    while app.is_running {
        // Drain ALL pending bus events before rendering so the screen
        // always reflects the latest state.
        loop {
            match bus_rx.try_recv() {
                Ok(event) => app.handle_event(event),
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                    app.push_log(
                        app::LogLevel::Warn,
                        format!("{n} events dropped (event bus lag)"),
                    );
                    // After Lagged, the receiver is reset — continue draining
                }
                Err(_) => break, // Empty or Closed
            }
        }

        terminal.draw(|frame| layout::render(frame, &mut app))?;

        tokio::select! {
            // Terminal key/mouse events (polled at 50ms intervals)
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                while ct_event::poll(std::time::Duration::ZERO)? {
                    match ct_event::read()? {
                        CtEvent::Key(key) => app.handle_key_event(key),
                        CtEvent::Mouse(mouse) => app.handle_mouse_event(mouse),
                        _ => {}
                    }
                }
            }
            // Wake up when a new bus event arrives (handled in drain loop above)
            result = bus_rx.recv() => {
                match result {
                    Ok(event) => app.handle_event(event),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        app.push_log(
                            app::LogLevel::Warn,
                            format!("{n} events dropped (event bus lag)"),
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {}
                }
            }
        }
    }

    // Restore terminal — disable mouse capture first to stop generating
    // escape sequences, then drain any buffered events so they don't leak
    // into the shell after we leave raw mode.
    execute!(terminal.backend_mut(), DisableMouseCapture)?;
    while ct_event::poll(std::time::Duration::ZERO)? {
        let _ = ct_event::read();
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

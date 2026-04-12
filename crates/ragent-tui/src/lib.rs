//! Terminal user interface for ragent.
//!
//! Provides a ratatui-based interactive TUI that displays agent messages,
//! tool call status, permission dialogs, and a text input prompt. The TUI
//! reacts to real-time events from the ragent [`EventBus`](ragent_core::event::EventBus).

pub mod app;
pub mod input;
pub mod layout;
pub mod layout_active_agents;
pub mod layout_teams;
pub mod logo;
pub mod theme;
pub mod tips;
pub mod tracing_layer;
pub mod utils;
pub mod widgets;

pub use app::App;

use std::sync::Arc;

use anyhow::Result;
use crossterm::{
    event::{
        self as ct_event, DisableMouseCapture, EnableMouseCapture, Event as CtEvent,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
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

use tracing_layer::TuiLogReceiver;

/// Run the TUI application.
///
/// Enters the alternate screen, creates an [`App`], and runs the main event
/// loop until the user quits. The terminal is restored on exit.
///
/// `log_rx` receives tracing records captured by [`tracing_layer::TuiTracingLayer`]
/// and routes them into the on-screen log panel so they never corrupt the
/// alternate-screen rendering.
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
/// let (tx, rx) = ragent_tui::tracing_layer::tui_log_channel(512);
/// ragent_tui::run_tui(bus, storage, registry, processor, agent, false, None, rx).await?;
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
    log_rx: TuiLogReceiver,
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    // Enable the Kitty keyboard protocol so terminals that support it will
    // send distinct escape codes for Shift+Enter (vs plain Enter).
    // `execute!` returns an error on unsupporting terminals; we ignore it so
    // the TUI still works on xterm/gnome-terminal/etc.
    let keyboard_enhanced = execute!(
        stdout,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )
    .is_ok();
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

    // Auto-initialize a session at startup if not resuming.
    if resume_session_id.is_none() {
        let dir = std::env::current_dir().unwrap_or_default();
        match app.session_processor.session_manager.create_session(dir) {
            Ok(session) => {
                let session_id = session.id.clone();
                app.session_id = Some(session_id.clone());
                // Register the short_sid → agent_name mapping so tool step tags
                // display "general:5" instead of "fbd5b8a9:5".
                app.register_primary_session_mapping();

                // Kick off the AGENTS.md acknowledgement exchange in the background
                // so it appears before the user types anything (including /swarm, /team, etc.).
                let proc = Arc::clone(&app.session_processor);
                let mut init_agent = app.agent_info.clone();
                // Inject the persisted model selection so the init call works even
                // before the user sends their first message.
                if !init_agent.model_pinned || init_agent.model.is_none() {
                    if let Some(ref model_str) = app.selected_model {
                        if let Some((p, m)) = model_str.split_once('/') {
                            init_agent.model = Some(ragent_core::agent::ModelRef {
                                provider_id: p.to_string(),
                                model_id: m.to_string(),
                            });
                        }
                    }
                }
                let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
                tokio::spawn(async move {
                    if let Err(e) = proc
                        .run_init_exchange(&session_id, &init_agent, cancel)
                        .await
                    {
                        tracing::warn!(error = %e, "Startup init exchange failed");
                    }
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to auto-create session at startup");
            }
        }
    }

    // Set up persistent input history (kept across sessions in the data dir).
    let history_path = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ragent")
        .join("input_history.txt");
    app.set_history_file(history_path);
    if let Err(e) = app.load_history() {
        tracing::warn!("Failed to load input history: {}", e);
    }

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
        let lsp_configs = Config::load().map(|c| c.lsp).unwrap_or_default();
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
                        None,
                    );
                    // After Lagged, the receiver is reset — continue draining
                }
                Err(_) => break, // Empty or Closed
            }
        }

        // Drain tracing records captured by TuiTracingLayer into the log panel.
        while let Ok(record) = log_rx.try_recv() {
            use tracing::Level;
            let level = match record.level {
                Level::ERROR => app::LogLevel::Error,
                Level::WARN => app::LogLevel::Warn,
                _ => app::LogLevel::Info,
            };
            if !record.message.is_empty() {
                app.push_log(level, record.message, None);
            }
        }

        // Check for completed /opt LLM results.
        app.poll_pending_opt();

        // Check for completed /swarm LLM decomposition results.
        app.poll_pending_swarm();

        // Unblock swarm tasks whose dependencies are satisfied.
        app.poll_swarm_unblock();

        // Check if active swarm has completed all tasks.
        app.poll_swarm_completion();

        // Fire any pending autopilot continuation.
        app.poll_autopilot_continue();

        // Flush dirty history to disk (non-blocking, debounced).
        app.flush_history_if_due();

        // Only draw when UI dirty or periodic refresh
        if app.needs_redraw {
            terminal.draw(|frame| layout::render(frame, &mut app))?;
            app.needs_redraw = false;
        }
        tokio::select! {
            // Terminal key/mouse events (polled at 50ms intervals)
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                let mut got_input = false;
                while ct_event::poll(std::time::Duration::ZERO)? {
                    match ct_event::read()? {
                        CtEvent::Key(key) => { app.handle_key_event(key); got_input = true; }
                        CtEvent::Mouse(mouse) => { app.handle_mouse_event(mouse); got_input = true; }
                        _ => {}
                    }
                }
                if got_input {
                    app.needs_redraw = true;
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
                            None,
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {}
                }
            }
        }
    }

    // Final synchronous save if history was modified since last flush.
    if app.history_dirty {
        if let Err(e) = app.save_history() {
            tracing::warn!("Failed to save input history: {}", e);
        }
    }

    // Restore terminal — disable mouse capture first to stop generating
    // escape sequences, then drain any buffered events so they don't leak
    // into the shell after we leave raw mode.
    if keyboard_enhanced {
        let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    }
    execute!(terminal.backend_mut(), DisableMouseCapture)?;
    while ct_event::poll(std::time::Duration::ZERO)? {
        let _ = ct_event::read();
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

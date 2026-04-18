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
pub mod panels;
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
use tokio::signal::unix::{SignalKind, signal};

/// RAII guard that ensures terminal state is restored on drop.
///
/// This struct handles the terminal setup (raw mode, alternate screen, mouse capture)
/// and automatically restores the terminal state when dropped. This is critical
/// for ensuring the terminal is usable after crashes (panic, OOM, segfault, etc.).
pub struct TerminalGuard {
    keyboard_enhanced: bool,
}

impl TerminalGuard {
    /// Create a new terminal guard, setting up the terminal.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal setup fails.
    ///
    /// # Safety
    ///
    /// This function modifies global terminal state. The caller must ensure
    /// that `restore_terminal` is called before the program exits.
    pub fn new() -> Result<Self> {
        // Enable the Kitty keyboard protocol before entering raw mode
        let keyboard_enhanced = execute!(
            std::io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )
        .is_ok();

        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        Ok(Self { keyboard_enhanced })
    }

    /// Restore the terminal to its original state.
    ///
    /// This is called automatically on drop, but can also be called explicitly.
    /// It is safe to call multiple times.
    pub fn restore_terminal(&self) {
        // Disable mouse capture first to stop generating escape sequences
        let _ = execute!(std::io::stdout(), DisableMouseCapture);

        if self.keyboard_enhanced {
            let _ = execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
        }

        // Leave alternate screen and disable raw mode
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
        let _ = disable_raw_mode();

        // Drain any buffered terminal events AFTER leaving raw mode
        // so they don't leak into the shell as garbage characters
        while ct_event::poll(std::time::Duration::from_millis(10)).unwrap_or(false) {
            let _ = ct_event::read();
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.restore_terminal();
    }
}

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
    // Set up panic handler to ensure terminal state is restored on crashes
    // This handles panics, OOM, and segfaults by restoring the terminal before
    // the default panic handler prints the backtrace
    let default_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Restore terminal state before printing panic message
        // This is best-effort; ignore errors
        let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);

        // Call the default panic hook to print the backtrace
        default_panic_hook(info);
    }));

    // Create the terminal guard - it will automatically restore terminal state on drop
    // We don't need to reference it after creation - Drop handles cleanup
    let _terminal_guard = TerminalGuard::new()?;
    // Now create the ratatui terminal
    let stdout = std::io::stdout();
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

    // -- Render the very first frame so the user sees the TUI immediately --
    app.status = "starting up…".to_string();
    app.force_new_message = true;
    app.append_assistant_text("⚙️ **Starting up…**");
    terminal.draw(|frame| layout::render(frame, &mut app))?;

    // -- Provider health check --
    app.check_provider_health();
    app.append_assistant_text("\n✔ Provider health check");
    app.status = "checking provider…".to_string();
    terminal.draw(|frame| layout::render(frame, &mut app))?;

    // -- Auto-initialize a session at startup if not resuming --
    if resume_session_id.is_none() {
        let dir = std::env::current_dir().unwrap_or_default();
        match app.session_processor.session_manager.create_session(dir) {
            Ok(session) => {
                let session_id = session.id.clone();
                app.session_id = Some(session_id.clone());
                app.register_primary_session_mapping();

                app.append_assistant_text(&format!("\n✔ Session created: `{}`", &session_id[..8]));
                app.status = "session created".to_string();
                terminal.draw(|frame| layout::render(frame, &mut app))?;

                // Kick off the AGENTS.md acknowledgement exchange in the background
                let proc = Arc::clone(&app.session_processor);
                let mut init_agent = app.agent_info.clone();
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

    // -- Input history --
    let history_path = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ragent")
        .join("input_history.txt");
    app.set_history_file(history_path);
    if let Err(e) = app.load_history() {
        tracing::warn!("Failed to load input history: {}", e);
    }
    app.append_assistant_text("\n✔ Input history loaded");
    terminal.draw(|frame| layout::render(frame, &mut app))?;

    // Subscribe to the event bus BEFORE starting LSP so no status events are dropped.
    let mut bus_rx = event_bus.subscribe();

    // -- LSP startup --
    app.status = "starting LSP…".to_string();
    terminal.draw(|frame| layout::render(frame, &mut app))?;

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
        app.lsp_servers = mgr.servers().to_vec();
    }
    app.set_lsp_manager(lsp_manager.clone());
    let _ = session_processor.lsp_manager.set(lsp_manager);

    let lsp_count = app.lsp_servers.len();
    app.append_assistant_text(&format!("\n✔ LSP: {lsp_count} server(s)"));
    terminal.draw(|frame| layout::render(frame, &mut app))?;

    // -- Code index startup --
    app.status = "starting code index…".to_string();
    terminal.draw(|frame| layout::render(frame, &mut app))?;

    // Track the fallback reindex thread so we can join it on shutdown
    let mut code_index_fallback_thread: Option<std::thread::JoinHandle<()>> = None;

    let _code_index: Option<Arc<ragent_code::CodeIndex>> = {
        let cwd = std::env::current_dir().unwrap_or_default();
        match ragent_core::config::Config::load() {
            Ok(config) => {
                if config.code_index.enabled {
                    let index_config = ragent_code::types::CodeIndexConfig {
                        enabled: true,
                        project_root: cwd.clone(),
                        index_dir: cwd.join(".ragent/codeindex"),
                        scan_config: ragent_code::types::ScanConfig::default(),
                    };
                    match ragent_code::CodeIndex::open(&index_config) {
                        Ok(idx) => {
                            let arc_idx = Arc::new(idx);
                            // Start the file watcher + background worker.
                            // start_watching() performs an initial full_reindex() and then
                            // watches for filesystem changes to keep the index up to date.
                            match ragent_code::start_watching(
                                arc_idx.clone(),
                                ragent_code::worker::WorkerConfig::default(),
                            ) {
                                Ok(session) => {
                                    app.code_index_watch_session = Some(session);
                                    tracing::info!("Code index watcher started");
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, "Failed to start code index watcher, falling back to one-shot reindex");
                                    // Fall back to one-shot background reindex without watcher
                                    let bg = arc_idx.clone();
                                    let handle = std::thread::spawn(move || {
                                        if let Err(e) = bg.full_reindex() {
                                            tracing::warn!(error = %e, "Background code index reindex failed");
                                        }
                                    });
                                    code_index_fallback_thread = Some(handle);
                                }
                            }
                            app.set_code_index(Some(arc_idx.clone()));
                            let _ = session_processor.code_index.set(arc_idx.clone());
                            app.append_assistant_text("\n✔ Code index: enabled");
                            tracing::info!(
                                "Code index initialized at {:?}",
                                index_config.index_dir
                            );
                            Some(arc_idx)
                        }
                        Err(e) => {
                            app.append_assistant_text("\n✘ Code index: failed to open");
                            tracing::warn!(error = %e, "Failed to initialize code index");
                            None
                        }
                    }
                } else {
                    app.append_assistant_text("\n✔ Code index: disabled");
                    tracing::debug!("Code index is disabled in config");
                    None
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load config for code index check");
                None
            }
        }
    };

    // -- Session resume --
    if let Some(ref sid) = resume_session_id {
        app.status = "resuming session…".to_string();
        terminal.draw(|frame| layout::render(frame, &mut app))?;
        if let Err(e) = app.load_session(sid) {
            tracing::error!(error = %e, session_id = %sid, "Failed to resume session");
        }
    }

    // -- Backfill context window cache for models selected before this feature --
    app.backfill_model_ctx_window();

    // -- Startup complete --
    app.append_assistant_text("\n\n✅ **Ready**");
    app.status = "ready".to_string();
    // Ensure the init exchange response starts a new message bubble
    app.force_new_message = true;
    terminal.draw(|frame| layout::render(frame, &mut app))?;

    // Set up signal handlers for graceful shutdown (SIGINT, SIGTERM)
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;

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

        // Refresh cached code index stats periodically (every 5s, not every frame).
        app.refresh_code_index_stats();

        // Refresh cached memory/journal stats for the status bar.
        app.refresh_memory_stats();

        // Only draw when UI dirty or periodic refresh
        if app.needs_redraw {
            terminal.draw(|frame| layout::render(frame, &mut app))?;
            app.needs_redraw = false;
        }
        tokio::select! {
            // Handle SIGINT (Ctrl+C) - initiate graceful shutdown
            _ = sigint.recv() => {
                tracing::info!("SIGINT received, initiating graceful shutdown");
                app.is_running = false;
            }
            // Handle SIGTERM - initiate graceful shutdown
            _ = sigterm.recv() => {
                tracing::info!("SIGTERM received, initiating graceful shutdown");
                app.is_running = false;
            }
                          // Terminal key/mouse events (polled at 50ms intervals)
                          _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                              let mut got_input = false;
                              while ct_event::poll(std::time::Duration::ZERO)? {
                                  match ct_event::read()? {
                                      CtEvent::Key(key) => { app.handle_key_event(key); got_input = true; }
                                      CtEvent::Mouse(mouse) => {
                                          // Only process mouse events when mouse mode is enabled
                                          if app.mouse_enabled {
                                              app.handle_mouse_event(mouse); got_input = true;
                                          }
                                      }
                                      _ => {}
                                  }
                              }
                              if got_input {
                                  app.needs_redraw = true;
                              }
                          }            // Wake up when a new bus event arrives (handled in drain loop above)
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

    // -- Signal cancellation to abort any active LLM streams --
    if let Some(ref flag) = app.cancel_flag {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    // -- Restore terminal FIRST so the user gets a clean shell immediately --
    // Drop the terminal guard now (before slow cleanup) to leave alternate screen
    // and disable raw mode. This prevents the "stuck in TUI" appearance.
    drop(_terminal_guard);

    // -- Safety-net: force exit after 3 seconds if cleanup hangs --
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        std::process::exit(0);
    });

    // -- Graceful shutdown of background resources --
    // Stop code index watcher (if running) - this has a Drop impl that calls stop()
    if let Some(session) = app.code_index_watch_session.take() {
        drop(session);
    }

    // Disconnect LSP servers gracefully (with timeout)
    if let Some(mgr) = app.lsp_manager.take() {
        let mgr_clone = mgr.clone();
        let lsp_shutdown = async {
            if let Ok(mut guard) = mgr_clone.try_write() {
                guard.disconnect_all().await;
            }
        };
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), lsp_shutdown).await;
    }

    // Wait for fallback reindex thread to complete (with timeout)
    if let Some(handle) = code_index_fallback_thread.take() {
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            tokio::task::spawn_blocking(move || handle.join().ok()),
        )
        .await;
    }

    Ok(())
}

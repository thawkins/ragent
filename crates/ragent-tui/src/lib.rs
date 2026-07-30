//! Terminal user interface for ragent.
//!
//! Provides a ratatui-based interactive TUI that displays agent messages,
//! tool call status, permission dialogs, and a text input prompt. The TUI
//! reacts to real-time events from the ragent [`EventBus`](ragent_agent::event::EventBus).

pub mod app;
pub mod input;
pub mod input_field;
pub mod layout;
pub mod layout_active_agents;
pub mod layout_statusbar;
pub mod layout_teams;
pub mod logo;
pub mod panels;
pub mod research_adapter;
pub mod research_progress;
pub mod theme;
pub mod tips;
pub mod tracing_layer;
pub mod utils;
pub mod widgets;

pub use app::{App, sanitize_for_display};

use std::sync::Arc;

use anyhow::Result;
use crossterm::{
    event::{
        self as ct_event, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste,
        EnableMouseCapture, Event as CtEvent, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
#[cfg(unix)]
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
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableBracketedPaste,
            EnableMouseCapture
        )?;

        Ok(Self { keyboard_enhanced })
    }

    /// Restore the terminal to its original state.
    ///
    /// This is called automatically on drop, but can also be called explicitly.
    /// It is safe to call multiple times.
    pub fn restore_terminal(&self) {
        // Disable mouse capture first to stop generating escape sequences
        let _ = execute!(std::io::stdout(), DisableMouseCapture);

        // Disable bracketed paste mode
        let _ = execute!(std::io::stdout(), DisableBracketedPaste);

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

use ragent_agent::agent::AgentInfo;
use ragent_agent::event::EventBus;
use ragent_agent::provider::ProviderRegistry;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;

use tracing_layer::TuiLogReceiver;

const IDLE_REDRAW_INTERVAL_MS: u64 = 250;

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
/// # use ragent_agent::event::EventBus;
/// # use ragent_agent::provider::ProviderRegistry;
/// # use ragent_agent::session::processor::SessionProcessor;
/// # use ragent_agent::storage::Storage;
/// # use ragent_agent::agent::AgentInfo;
/// # use ragent_agent::StartupTimings;
/// # async fn example(
/// #     bus: Arc<EventBus>,
/// #     storage: Arc<Storage>,
/// #     registry: Arc<ProviderRegistry>,
/// #     processor: Arc<SessionProcessor>,
/// # ) -> anyhow::Result<()> {
/// let agent = AgentInfo::new("general", "General-purpose agent");
/// let (tx, rx) = ragent_tui::tracing_layer::tui_log_channel(512);
/// ragent_tui::run_tui(
///     bus, storage, registry, processor, agent, false, None, rx,
///     std::path::PathBuf::new(),
///     vec![],
///     StartupTimings::new(),
/// ).await?;
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
    db_path: std::path::PathBuf,
    config_paths: Vec<std::path::PathBuf>,
    mut startup: ragent_agent::StartupTimings,
) -> Result<()> {
    use std::time::Instant;
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
    let t0 = Instant::now();
    let terminal_guard = TerminalGuard::new()?;
    // Now create the ratatui terminal
    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    startup.record("Terminal setup", t0.elapsed());

    let t0 = Instant::now();
    let mut app = App::new(
        event_bus.clone(),
        Arc::clone(&storage),
        provider_registry.clone(),
        session_processor.clone(),
        agent,
        show_log,
        db_path,
    );
    let app_new_elapsed = t0.elapsed();

    // Merge sub-stages recorded inside App::new() into the main timings.
    if let Some(ref mut app_sub) = app.startup_timings {
        startup.merge_stages(app_sub);
    }
    startup.record("App::new() (total)", app_new_elapsed);

    // Attach the event bus to providers that publish lifecycle events
    // (e.g. local model download progress).
    provider_registry.set_event_bus_all(Some(event_bus.clone()));
    // Attach storage to the router provider so it can resolve database-backed
    // API keys (e.g. `ragent auth ollama_cloud <key>`) when routing to
    // downstream providers.
    if let Some(router_provider) = provider_registry
        .get_as_any("router")
        .and_then(|p| p.downcast_ref::<ragent_llm::providers::router::RouterProvider>())
    {
        router_provider.set_storage(Arc::clone(&storage));
    }
    // Pass through the config file paths loaded at startup so the TUI
    // can display them in the message window.
    app.config_paths = config_paths;

    // -- Render the very first frame so the user sees the TUI immediately --
    app.status = "starting up…".to_string();
    app.force_new_message = true;
    app.append_assistant_text("⚙️ **Starting up…**");
    terminal.draw(|frame| layout::render(frame, &mut app))?;

    // -- Provider health check --
    let t0 = Instant::now();
    app.check_provider_health();
    app.append_assistant_text("\n✔ Provider health check");
    app.status = "checking provider…".to_string();
    terminal.draw(|frame| layout::render(frame, &mut app))?;
    startup.record("Provider health check", t0.elapsed());

    // Subscribe to the event bus before starting background services.
    //
    // This includes the startup init exchange. If we spawn that exchange before
    // subscribing, streamed startup deltas can be treated as "dropped" and
    // produce a large burst of warning logs.
    // Bridge the broadcast channel to an unbounded mpsc channel so the TUI never
    // loses events.  The broadcast channel can drop events for slow receivers
    // (Lagged error) during burst scenarios (many parallel tool calls, rapid
    // streaming).  The mpsc channel is unbounded, so the TUI always receives
    // every event regardless of drain speed.
    let (event_tx, mut event_rx) =
        tokio::sync::mpsc::unbounded_channel::<ragent_agent::event::Event>();
    {
        let mut bus_rx = event_bus.subscribe();
        tokio::spawn(async move {
            loop {
                match bus_rx.recv().await {
                    Ok(event) => {
                        if event_tx.send(event).is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("{n} broadcast events skipped in TUI bridge task");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    // -- Auto-initialize a session at startup if not resuming --
    let t0 = Instant::now();
    if resume_session_id.is_none() {
        let dir = std::env::current_dir().unwrap_or_default();
        match app.session_processor.session_manager.create_session(dir) {
            Ok(session) => {
                let session_id = session.id.clone();
                app.session_id = Some(session_id.clone());
                app.register_primary_session_mapping();

                // Render the ASCII art banner into the message window now that
                // the session is valid (append_assistant_text requires session_id).
                let banner = crate::logo::LOGO.join("\n");
                app.append_assistant_text(&banner);
                app.append_assistant_text(&format!("\n  Version {}", env!("CARGO_PKG_VERSION")));
                // app.force_new_message = true;
                app.append_assistant_text(&format!("\n✔ Session created: `{}`", &session_id[..8]));
                // Display the loaded configuration file(s)
                if app.config_paths.is_empty() {
                    app.append_assistant_text("\nℹ No config file found; using defaults");
                } else {
                    let mut paths_text = app
                        .config_paths
                        .iter()
                        .map(|p| {
                            let s = p.display().to_string();
                            if let Some(home) = std::env::var_os("HOME") {
                                let home = home.to_string_lossy();
                                if let Some(rest) = s.strip_prefix(home.as_ref()) {
                                    return format!("~{}", rest);
                                }
                            }
                            s
                        })
                        .collect::<Vec<_>>()
                        .join("`\n  ");
                    // Prepend indentation for multi-line alignment
                    if app.config_paths.len() > 1 {
                        paths_text = format!("\n  {paths_text}");
                    }
                    app.append_assistant_text(&format!("\n✔ Loaded config file: `{paths_text}`"));
                }
                app.status = "session created".to_string();
                terminal.draw(|frame| layout::render(frame, &mut app))?;

                // Kick off the AGENTS.md acknowledgement exchange in the background
                let proc = Arc::clone(&app.session_processor);
                let mut init_agent = app.agent_info.clone();
                if !init_agent.model_pinned || init_agent.model.is_none() {
                    if let Some(ref model_str) = app.selected_model {
                        if let Some((p, m)) = model_str.split_once('/') {
                            init_agent.model = Some(ragent_agent::agent::ModelRef {
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
    startup.record("Session create", t0.elapsed());
    let t0 = Instant::now();
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
    startup.record("Input history load", t0.elapsed());

    // -- Code index startup (non-blocking) --
    // The code index open + watcher setup can take several seconds on large
    // projects.  We spawn it in a background task so the TUI event loop starts
    // immediately and the index becomes available when ready.
    app.status = "starting code index…".to_string();
    terminal.draw(|frame| layout::render(frame, &mut app))?;

    /// Result of the background code-index startup, delivered to the main
    /// event loop via an mpsc channel so `&mut App` updates happen on the
    /// main thread.
    struct CodeIndexStartupResult {
        /// The opened code index, if any.
        index: Option<Arc<ragent_codeindex::CodeIndex>>,
        /// The watch session, if the watcher started successfully.
        watch_session: Option<ragent_codeindex::WatchSession>,
        /// Fallback one-shot reindex thread (when watcher failed to start).
        fallback_thread: Option<std::thread::JoinHandle<()>>,
        /// Human-readable message to append to the chat panel.
        message: String,
        /// Wall-clock duration of the background code-index startup.
        elapsed: std::time::Duration,
    }

    let (ci_tx, mut ci_rx) = tokio::sync::mpsc::unbounded_channel::<CodeIndexStartupResult>();

    let sp = Arc::clone(&session_processor);
    tokio::spawn(async move {
        let cwd = std::env::current_dir().unwrap_or_default();
        let ci_inner_start = Instant::now();
        let result = match ragent_agent::Config::load() {
            Ok(config) => {
                if config.code_index.enabled {
                    let index_config = ragent_codeindex::types::CodeIndexConfig {
                        enabled: true,
                        project_root: cwd.clone(),
                        index_dir: cwd.join(".ragent/codeindex"),
                        scan_config: ragent_codeindex::types::ScanConfig::default(),
                    };
                    match ragent_codeindex::CodeIndex::open(&index_config) {
                        Ok(idx) => {
                            let arc_idx = Arc::new(idx);
                            // Start the file watcher + background worker.
                            // start_watching() performs an initial full_reindex() in a
                            // background thread and then watches for filesystem changes.
                            let (watch_session, fallback_thread) =
                                match ragent_codeindex::start_watching(
                                    arc_idx.clone(),
                                    ragent_codeindex::worker::WorkerConfig::default(),
                                ) {
                                    Ok(session) => {
                                        tracing::info!("Code index watcher started");
                                        (Some(session), None)
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, "Failed to start code index watcher, falling back to one-shot reindex");
                                        let bg = arc_idx.clone();
                                        let handle = std::thread::spawn(move || {
                                            if let Err(e) = bg.full_reindex() {
                                                tracing::warn!(error = %e, "Background code index reindex failed");
                                            }
                                        });
                                        (None, Some(handle))
                                    }
                                };
                            // Thread-safe: OnceLock can be set from any thread.
                            let _ = sp.code_index.set(arc_idx.clone());
                            tracing::info!(
                                "Code index initialized at {:?}",
                                index_config.index_dir
                            );
                            CodeIndexStartupResult {
                                index: Some(arc_idx),
                                watch_session,
                                fallback_thread,
                                message: "\n✔ Code index: enabled".to_string(),
                                elapsed: ci_inner_start.elapsed(),
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to initialize code index");
                            CodeIndexStartupResult {
                                index: None,
                                watch_session: None,
                                fallback_thread: None,
                                message: "\n✘ Code index: failed to open".to_string(),
                                elapsed: ci_inner_start.elapsed(),
                            }
                        }
                    }
                } else {
                    tracing::debug!("Code index is disabled in config");
                    CodeIndexStartupResult {
                        index: None,
                        watch_session: None,
                        fallback_thread: None,
                        message: "\n✔ Code index: disabled".to_string(),
                        elapsed: ci_inner_start.elapsed(),
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load config for code index check");
                CodeIndexStartupResult {
                    index: None,
                    watch_session: None,
                    fallback_thread: None,
                    message: String::new(),
                    elapsed: ci_inner_start.elapsed(),
                }
            }
        };
        let _ = ci_tx.send(result);
    });

    // Track the fallback reindex thread so we can join it on shutdown.
    // Populated when the background code-index startup result arrives.
    let mut code_index_fallback_thread: Option<std::thread::JoinHandle<()>> = None;

    // -- Spec manager startup --
    let t0 = Instant::now();
    let specs_root = std::env::current_dir().unwrap_or_default().join("specs");
    let _ = session_processor
        .spec_manager
        .set(Arc::new(ragent_specs::SpecManager::new(&specs_root)));
    app.spec_manager = Some(Arc::new(ragent_specs::SpecManager::new(&specs_root)));

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
    startup.record("Spec mgr & resume & backfill", t0.elapsed());

    // -- Startup complete --
    app.startup_timings = Some(startup);
    app.append_assistant_text("\n✅ **Ready**");
    app.status = "ready".to_string();
    // Ensure the init exchange response starts a new message bubble
    app.force_new_message = true;
    terminal.draw(|frame| layout::render(frame, &mut app))?;

    // Record code-index background startup time when it arrives (stored for
    // the event loop to merge into startup_timings).
    let mut ci_startup_recorded = false;

    // Set up signal handlers for graceful shutdown via a channel
    // This works cross-platform without needing #[cfg] inside tokio::select!
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::unbounded_channel();

    // Spawn a task to listen for signals
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            let mut sigint = signal(SignalKind::interrupt())?;
            let mut sigterm = signal(SignalKind::terminate())?;
            tokio::select! {
                _ = sigint.recv() => {
                    tracing::info!("SIGINT received, initiating graceful shutdown");
                }
                _ = sigterm.recv() => {
                    tracing::info!("SIGTERM received, initiating graceful shutdown");
                }
            }
        }
        #[cfg(windows)]
        {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("Ctrl+C received, initiating graceful shutdown");
        }
        let _ = shutdown_tx.send(());
        anyhow::Result::<()>::Ok(())
    });

    let mut last_draw = std::time::Instant::now();

    while app.is_running {
        // Drain ALL pending events before rendering so the screen
        // always reflects the latest state.
        while let Ok(event) = event_rx.try_recv() {
            app.handle_event(event);
        }

        // Poll the background code-index startup result (non-blocking).
        // When it arrives, wire the index into App state and update the UI.
        if let Ok(result) = ci_rx.try_recv() {
            if let Some(idx) = result.index {
                app.set_code_index(Some(idx));
            }
            if let Some(session) = result.watch_session {
                app.code_index_watch_session = Some(session);
            }
            code_index_fallback_thread = result.fallback_thread;
            if !result.message.is_empty() {
                app.append_assistant_text(&result.message);
            }
            // Record the background code-index startup duration once.
            if !ci_startup_recorded {
                if let Some(ref mut st) = app.startup_timings {
                    st.record("Code index (background)", result.elapsed);
                }
                ci_startup_recorded = true;
            }
            // Clear the "starting code index…" status now that setup is done.
            if app.status == "starting code index…" {
                app.status = "ready".to_string();
            }
            app.needs_redraw = true;
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

        // Check for completed /bench background runs.
        app.poll_pending_bench();

        // Transition slash-command statuses to "ready" after a grace period.
        app.poll_status_expiry();

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

        // Refresh cached memory stats for the status bar.
        app.refresh_memory_stats();

        if app.needs_redraw
            || last_draw.elapsed() >= std::time::Duration::from_millis(IDLE_REDRAW_INTERVAL_MS)
        {
            terminal.draw(|frame| layout::render(frame, &mut app))?;
            app.needs_redraw = false;
            last_draw = std::time::Instant::now();
        }
        tokio::select! {
            // Handle shutdown signal (Ctrl+C/SIGINT/SIGTERM)
            _ = shutdown_rx.recv() => {
                app.is_running = false;
            }
                          // Terminal key/mouse events (polled at 50ms intervals)
                          _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                              let mut got_input = false;
                                                              while ct_event::poll(std::time::Duration::ZERO)? {
                                                                  match ct_event::read()? {
                                                                      CtEvent::Key(key) => { app.handle_key_event(key); got_input = true; }
                                                                      CtEvent::Mouse(mouse)
                                                                          // Only process mouse events when mouse mode is enabled
                                                                          if app.mouse_enabled => {
                                                                              app.handle_mouse_event(mouse); got_input = true;
                                                                          }
                                                                      CtEvent::Paste(text) => {
                                                                          // Insert pasted text as a single operation
                                                                          // Strip carriage returns but preserve newlines
                                                                          let clean: String = text.chars().filter(|&c| c != '\r').collect();
                                                                          app.insert_text_at_cursor(&clean);
                                                                          got_input = true;
                                                                      }
                                                                      _ => {}
                                                                  }
                                                              }                              if got_input {
                                  app.needs_redraw = true;
                              }
                          }            // Wake up when a new event arrives from the lossless mpsc bridge
            event = event_rx.recv() => {
                match event {
                    Some(event) => app.handle_event(event),
                    None => {} // Bridge task exited
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
    drop(terminal_guard);

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

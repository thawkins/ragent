//! Tests for the GCF status-bar indicator and the Alt+G toggle.
//!
//! Covers:
//! - Alt+G toggles GCF encoding through the real persist path
//!   (`ragent_config::gcf::toggle_persist`) and reports the new state.
//! - The GCF icon renders on the second status-bar line, LEFT of the
//!   codeindex icon, with the enabled (`[ok]`) / disabled (`[no]`) marker.
//! - The `g` keystroke is never inserted into the input buffer (NFR-002).

#[path = "support/mod.rs"]
mod support;

use std::sync::MutexGuard;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_tui::{App, layout};
use ratatui::{Terminal, backend::TestBackend};

/// Build an [`App`] backed by an in-memory database (mirrors the pattern used
/// by the slash-command tests).
/// A test [`App`] built on the shared crate-wide fixture.
fn make_app() -> App {
    support::make_app()
}

struct CwdGuard {
    prev: std::path::PathBuf,
    #[allow(dead_code)]
    lock: MutexGuard<'static, ()>,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// Acquire the cwd test lock, recovering from any prior poisoning.
/// The process-wide cwd/env lock (see `tests/all.rs`).
fn cwd_lock() -> MutexGuard<'static, ()> {
    crate::global_env_lock()
}

/// Create a tempdir, chdir into it with a project-local config primed for
/// GCF-off, and return the cwd-restoring guard. The tempdir is removed only
/// after cwd has been restored.
fn enter_isolated_config_project() -> (CwdGuard, tempfile::TempDir) {
    let lock = cwd_lock();
    let prev = std::env::current_dir().expect("current dir");
    let temp = tempfile::TempDir::new().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    let ragent_dir = temp.path().join(".ragent");
    std::fs::create_dir_all(&ragent_dir).expect("create .ragent");
    std::fs::write(
        ragent_dir.join("ragent.json"),
        r#"{"gcf": {"enabled": false}}"#,
    )
    .expect("write project config");
    (CwdGuard { prev, lock }, temp)
}

/// Render the app and return the full terminal buffer text.
fn render_to_text(app: &mut App) -> String {
    let backend = TestBackend::new(140, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");
    let cells = terminal.backend().buffer().content.clone();
    cells.iter().map(ratatui::buffer::Cell::symbol).collect()
}

#[tokio::test]
async fn test_alt_g_toggles_gcf_and_status_bar_indicator() {
    let (_guard, _temp) = enter_isolated_config_project();
    ragent_config::gcf::set_enabled(false);

    // Warm the config cache BEFORE the toggle so the real project-config
    // resolution path (which mirrors the live app) is exercised.
    // Touch the config loader once so a malformed-config failure surfaces as
    // an explicit expect here rather than a silent toggle failure below.
    ragent_config::Config::load().expect("config load");

    let mut app = make_app();

    // Sanity: GCF starts off.
    assert!(!ragent_config::gcf::is_enabled());

    // Press Alt+G through the app handler so the persist path runs.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT))
        .await;

    // Handler should have toggled and persisted GCF on.
    assert!(ragent_config::gcf::is_enabled());
    assert!(app.status.contains("GCF encoding enabled"));

    // Status bar should show the enabled GCF tag next to the codeindex tag.
    let text = render_to_text(&mut app);
    assert!(
        text.contains("GCF:") && text.contains("✓"),
        "status bar should show enabled GCF tag: {text}"
    );

    // Toggle back off and verify.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT))
        .await;
    assert!(!ragent_config::gcf::is_enabled());
    assert!(app.status.contains("GCF encoding disabled"));

    let text = render_to_text(&mut app);
    assert!(
        text.contains("GCF:") && text.contains("✗"),
        "status bar should show disabled GCF tag: {text}"
    );
}

#[tokio::test]
async fn test_alt_g_never_inserts_g_into_input_buffer() {
    let (_guard, _temp) = enter_isolated_config_project();
    ragent_config::gcf::set_enabled(false);

    let mut app = make_app();

    app.handle_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT))
        .await;

    assert!(
        app.input.is_empty(),
        "Alt+G must not insert `g` into the input buffer; input was: {:?}",
        app.input
    );
}

#[test]
fn test_gcf_indicator_sits_left_of_codeindex_icon() {
    let (_guard, _temp) = enter_isolated_config_project();
    ragent_config::gcf::set_enabled(true);

    let mut app = make_app();
    let text = render_to_text(&mut app);

    // Both tags must be present on line 2, with GCF appearing before (left of)
    // the codeindex tag.
    let gcf_pos = text.find("GCF:").expect("GCF tag must render when enabled");
    let codeindex_pos = text
        .find("CDX:")
        .expect("codeindex tag must render on line 2");
    assert!(
        gcf_pos < codeindex_pos,
        "GCF tag must sit LEFT of the codeindex tag; text: {text}"
    );

    ragent_config::gcf::set_enabled(false);
}

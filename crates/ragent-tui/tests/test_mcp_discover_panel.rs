//! `/mcp discover` panel tests.
//!
//! The dialog lists discovered MCP servers under a block cursor. `Up`/`Down`
//! move the cursor (the list scrolls to keep it visible), `Enter` toggles the
//! highlighted server's enabled state in `ragent.json` + the global ledger, and
//! `Esc` closes the dialog.
//!
//! Persistence tests redirect cwd + `XDG_CONFIG_HOME` into a tempdir so a toggle
//! never touches the developer's real config. Env mutation is `unsafe` in
//! edition 2024; the workspace denies `unsafe_code`, so this target opts back in
//! explicitly (env mutation is contained to the binary).
#![allow(unsafe_code)]
// The test serialiser is intentionally held across the async `#[tokio::test]`
// bodies so cwd/env mutations cannot interleave (mirrors `test_backend_command`).
#![allow(clippy::await_holding_lock)]
#![cfg(test)]

use std::sync::{Mutex, OnceLock};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_agent::mcp::discovery::{DiscoveredMcpServer, McpDiscoverySource};
use ragent_tui::App;
use ragent_tui::app::McpDiscoverState;
use ragent_tui::layout;

#[path = "support/mod.rs"]
mod support;

/// The cwd/env mutations below are process-global; one mutex serialises every
/// test in this binary against them.
fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Restores cwd and config env vars on drop.
struct EnvGuard {
    keys: Vec<(&'static str, Option<String>)>,
    cwd: std::path::PathBuf,
}

impl EnvGuard {
    fn new() -> Self {
        Self {
            keys: vec![
                ("XDG_CONFIG_HOME", std::env::var("XDG_CONFIG_HOME").ok()),
                ("RAGENT_CONFIG", std::env::var("RAGENT_CONFIG").ok()),
                (
                    "RAGENT_CONFIG_CONTENT",
                    std::env::var("RAGENT_CONFIG_CONTENT").ok(),
                ),
            ],
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in &self.keys {
            match v {
                Some(val) => unsafe { std::env::set_var(k, val) },
                None => unsafe { std::env::remove_var(k) },
            }
        }
        std::env::set_current_dir(&self.cwd).ok();
    }
}

/// Enter an isolated tempdir project with the given project config contents.
fn enter_project(
    config: &str,
) -> (
    std::sync::MutexGuard<'static, ()>,
    EnvGuard,
    tempfile::TempDir,
) {
    let guard = test_lock();
    let env_guard = EnvGuard::new();
    let temp = tempfile::tempdir().expect("tempdir");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", temp.path().join(".config")) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };
    std::env::set_current_dir(temp.path()).expect("set cwd");

    let ragent_dir = temp.path().join(".ragent");
    std::fs::create_dir_all(&ragent_dir).expect("create .ragent");
    std::fs::write(ragent_dir.join("ragent.json"), config).expect("write project config");
    (guard, env_guard, temp)
}

/// A discovered server with the given id, on `PATH`.
fn discovered(id: &str) -> DiscoveredMcpServer {
    DiscoveredMcpServer {
        id: id.to_string(),
        name: format!("{id} MCP Server"),
        executable: std::path::PathBuf::from(id),
        args: Vec::new(),
        env: std::collections::HashMap::new(),
        source: McpDiscoverySource::SystemPath,
    }
}

/// The dialog state for a set of servers, none enabled, cursor on the first row.
fn dialog(servers: Vec<DiscoveredMcpServer>) -> McpDiscoverState {
    let enabled = vec![false; servers.len()];
    McpDiscoverState {
        servers,
        enabled,
        selected: 0,
        scroll_offset: 0,
        feedback: None,
    }
}

/// Press a key through the full input path.
async fn press(app: &mut App, code: KeyCode) {
    app.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE))
        .await;
}

/// The rendered screen as one string per row.
fn render_rows(app: &mut App) -> Vec<String> {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render app");
    (0..40)
        .map(|y| {
            (0..120)
                .map(|x| {
                    terminal
                        .backend()
                        .buffer()
                        .cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
                .collect::<String>()
        })
        .collect()
}

#[tokio::test]
async fn mcp_discover_down_moves_cursor_and_clamps_at_ends() {
    let _guard = enter_project("{}");
    let mut app = support::make_app();
    app.mcp_discover = Some(dialog(vec![
        discovered("a"),
        discovered("b"),
        discovered("c"),
    ]));

    press(&mut app, KeyCode::Down).await;
    assert_eq!(app.mcp_discover.as_ref().unwrap().selected, 1);

    press(&mut app, KeyCode::Down).await;
    press(&mut app, KeyCode::Down).await;
    // Clamped at the last row (index 2).
    assert_eq!(app.mcp_discover.as_ref().unwrap().selected, 2);

    press(&mut app, KeyCode::Up).await;
    assert_eq!(app.mcp_discover.as_ref().unwrap().selected, 1);

    press(&mut app, KeyCode::Home).await;
    assert_eq!(app.mcp_discover.as_ref().unwrap().selected, 0);

    press(&mut app, KeyCode::End).await;
    assert_eq!(app.mcp_discover.as_ref().unwrap().selected, 2);

    // Up at the top stays put.
    press(&mut app, KeyCode::Home).await;
    press(&mut app, KeyCode::Up).await;
    assert_eq!(app.mcp_discover.as_ref().unwrap().selected, 0);
}

#[tokio::test]
async fn mcp_discover_escape_closes_the_dialog() {
    let _guard = enter_project("{}");
    let mut app = support::make_app();
    app.mcp_discover = Some(dialog(vec![discovered("a")]));

    press(&mut app, KeyCode::Esc).await;
    assert!(app.mcp_discover.is_none());
}

#[tokio::test]
async fn mcp_discover_enter_enables_then_disables_the_highlighted_server() {
    let (_guard, _env, temp) = enter_project("{}");
    let mut app = support::make_app();
    app.mcp_discover = Some(dialog(vec![discovered("filesystem")]));

    // First Enter adds the server to ragent.json and enables it.
    press(&mut app, KeyCode::Enter).await;

    let state = app.mcp_discover.as_ref().expect("dialog open");
    assert!(state.enabled[0], "the toggled row must read enabled");
    assert!(
        state
            .feedback
            .as_deref()
            .is_some_and(|m| m.starts_with("[ok]")),
        "expected an [ok] feedback, got {:?}",
        state.feedback
    );

    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(temp.path().join(".ragent/ragent.json")).expect("read config"),
    )
    .expect("parse config");
    assert_eq!(
        config["mcp"]["filesystem"]["disabled"], false,
        "the entry must be written enabled"
    );

    // Second Enter disables it (persisted in the global ledger).
    press(&mut app, KeyCode::Enter).await;
    let state = app.mcp_discover.as_ref().expect("dialog open");
    assert!(!state.enabled[0], "the toggled row must read disabled");
    assert!(
        state
            .feedback
            .as_deref()
            .is_some_and(|m| m.starts_with("[ok]")),
        "expected an [ok] feedback, got {:?}",
        state.feedback
    );

    let ledger = std::fs::read_to_string(temp.path().join(".config/ragent/mcp_state.json"))
        .expect("read ledger");
    let ledger: serde_json::Value = serde_json::from_str(&ledger).expect("parse ledger");
    assert_eq!(ledger["servers"]["filesystem"], false);
}

#[tokio::test]
async fn mcp_discover_enter_lifts_a_hard_disabled_override() {
    let (_guard, _env, temp) = enter_project(
        r#"{ "mcp": { "filesystem": { "type": "stdio", "command": "custom", "disabled": true } } }"#,
    );
    let mut app = support::make_app();
    app.mcp_discover = Some(dialog(vec![discovered("filesystem")]));

    press(&mut app, KeyCode::Enter).await;

    let state = app.mcp_discover.as_ref().expect("dialog open");
    assert!(state.enabled[0]);

    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(temp.path().join(".ragent/ragent.json")).expect("read config"),
    )
    .expect("parse config");
    assert_eq!(config["mcp"]["filesystem"]["disabled"], false);
    // The user's own command is preserved, not overwritten by the discovery.
    assert_eq!(config["mcp"]["filesystem"]["command"], "custom");
}

#[tokio::test]
async fn mcp_discover_list_scrolls_to_keep_the_cursor_visible() {
    let _guard = enter_project("{}");
    let mut app = support::make_app();
    let servers: Vec<DiscoveredMcpServer> = (0..30)
        .map(|i| discovered(&format!("server-{i:02}")))
        .collect();
    app.mcp_discover = Some(dialog(servers));

    // Render once so the initial scroll window is established.
    let rows = render_rows(&mut app).join("\n");
    assert!(
        rows.contains("server-00"),
        "the first server must be visible"
    );
    assert_eq!(app.mcp_discover.as_ref().unwrap().scroll_offset, 0);

    // Move the cursor to the last row and render.
    for _ in 0..40 {
        press(&mut app, KeyCode::Down).await;
    }
    let rows = render_rows(&mut app).join("\n");
    assert_eq!(app.mcp_discover.as_ref().unwrap().selected, 29);
    assert!(
        app.mcp_discover.as_ref().unwrap().scroll_offset > 0,
        "the list must scroll once the cursor passes the visible window"
    );
    assert!(
        rows.contains("server-29"),
        "the last server must be scrolled into view"
    );
    assert!(
        !rows.contains("server-00"),
        "the first server must have scrolled off the top"
    );
}

#[tokio::test]
async fn mcp_discover_block_cursor_paints_the_selected_row() {
    let _guard = enter_project("{}");
    let mut app = support::make_app();
    app.mcp_discover = Some(dialog(vec![
        discovered("alpha"),
        discovered("bravo"),
        discovered("charlie"),
    ]));
    press(&mut app, KeyCode::Down).await;

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, &mut app))
        .expect("render app");

    let buffer = terminal.backend().buffer();
    // The selected row ("bravo") must carry the block-cursor background
    // somewhere along it; a non-selected row must not.
    let mut bravo_magenta = false;
    let mut alpha_magenta = false;
    for y in 0..40 {
        let row: String = (0..120)
            .map(|x| {
                buffer
                    .cell((x, y))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default()
            })
            .collect();
        if row.contains("bravo") {
            bravo_magenta = (0..120).any(|x| buffer[(x, y)].bg == ratatui::style::Color::Magenta);
        }
        if row.contains("alpha") {
            alpha_magenta = (0..120).any(|x| buffer[(x, y)].bg == ratatui::style::Color::Magenta);
        }
    }
    assert!(
        bravo_magenta,
        "the block-cursor row must carry the cursor background"
    );
    assert!(
        !alpha_magenta,
        "an unselected row must not carry the cursor background"
    );
}

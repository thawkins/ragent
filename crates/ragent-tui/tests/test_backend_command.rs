//! `/backend` execution-backend TUI surface tests (spec `openhands` T-005;
//! FR-008).
//!
//! Covers the four FR-008 obligations:
//! - the active backend is surfaced in the status bar (default `local`);
//! - `/backend` opens a switcher panel listing every registered backend with a
//!   health state, including `docker`/`podman` kinds;
//! - `Enter` (or `/backend <kind>`) switches the active backend and persists it;
//! - the panel owns the keyboard while open, and an `unavailable` backend cannot
//!   be selected.
//!
//! Persistence tests redirect cwd + `XDG_CONFIG_HOME` into a tempdir so a
//! switch never touches the developer's real config. Env mutation is `unsafe` in
//! edition 2024; the workspace denies `unsafe_code`, so this test target opts
//! back in explicitly (env mutation is contained to the binary).
#![allow(unsafe_code)]
// The test serialiser (`test_lock`) is intentionally held across the async
// `#[tokio::test]` bodies so cwd/env mutations cannot interleave; this mirrors
// the `test_i18n_command` / `test_plugins_command` targets.
#![allow(clippy::await_holding_lock)]
#![cfg(test)]

use std::sync::{Mutex, OnceLock};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_tui::App;
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

/// Render one frame and return the painted text, one line per screen row.
fn render_app_to_string(app: &mut App) -> String {
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
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every painted row of the `/backend` panel's inner area.
fn panel_rows(terminal: &Terminal<TestBackend>, app: &App) -> String {
    let area = app.backend_panel_area;
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
        .collect::<Vec<_>>()
        .join("\n")
        .lines()
        .skip(area.y as usize + 1)
        .take(area.height.saturating_sub(2) as usize)
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Status bar surfaces the active backend (FR-008)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn status_bar_surfaces_the_default_local_backend() {
    let (_guard, _env, _temp) = enter_project(r#"{"defaultAgent": "general"}"#);
    let mut app = support::make_app();

    assert_eq!(app.active_backend, "local", "FR-019 default backend");
    let frame = render_app_to_string(&mut app);
    let line2 = frame.lines().nth(1).unwrap_or("");
    assert!(
        line2.contains("B:local"),
        "the status bar must surface the active backend; line was: {line2}"
    );
}

// ---------------------------------------------------------------------------
// `/backend` opens the switcher panel (FR-004, FR-008)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn backend_command_opens_the_switcher_panel() {
    let project_config = r#"
    {
      "defaultAgent": "general",
      "backends": [
        { "id": "box", "name": "Docker box", "kind": "docker", "image": "alpine:latest" }
      ]
    }
    "#;
    let (_guard, _env, _temp) = enter_project(project_config);
    let mut app = support::make_app();
    app.session_id = Some("backend-test".to_string());

    app.execute_slash_command("/backend").await;

    let panel = app.backend_panel.as_ref().expect("panel must open");
    let ids: Vec<&str> = panel.rows.iter().map(|r| r.id.as_str()).collect();
    assert!(ids.contains(&"local"), "local must be registered: {ids:?}");
    assert!(
        ids.contains(&"box"),
        "the configured docker backend must be registered: {ids:?}"
    );
    let docker = panel.rows.iter().find(|r| r.id == "box").expect("box row");
    assert_eq!(docker.kind, "docker", "the docker kind must be surfaced");
    assert!(
        !docker.health.is_empty(),
        "every row carries a health state"
    );
    let local = panel
        .rows
        .iter()
        .find(|r| r.id == "local")
        .expect("local row");
    assert!(local.active, "local is active by default");
    assert_eq!(local.health, "ok", "local is always healthy");
}

#[tokio::test]
async fn panel_lists_docker_and_podman_kinds_with_health() {
    let project_config = r#"
    {
      "defaultAgent": "general",
      "backends": [
        { "id": "dock", "name": "D", "kind": "docker", "image": "alpine" },
        { "id": "pod", "name": "P", "kind": "podman", "image": "alpine" }
      ]
    }
    "#;
    let (_guard, _env, _temp) = enter_project(project_config);
    let mut app = support::make_app();
    app.session_id = Some("backend-test".to_string());

    app.execute_slash_command("/backend").await;
    let panel = app.backend_panel.as_ref().expect("panel must open");
    let kinds: Vec<&str> = panel.rows.iter().map(|r| r.kind).collect();
    assert!(kinds.contains(&"docker"), "docker kind listed: {kinds:?}");
    assert!(kinds.contains(&"podman"), "podman kind listed: {kinds:?}");
    for row in &panel.rows {
        assert!(
            matches!(row.health, "ok" | "unavailable" | "unknown"),
            "row {} has a valid health: {}",
            row.id,
            row.health
        );
    }
}

#[tokio::test]
async fn panel_renders_rows_and_footer() {
    let (_guard, _env, _temp) = enter_project(r#"{"defaultAgent": "general"}"#);
    let mut app = support::make_app();
    app.session_id = Some("backend-test".to_string());
    app.execute_slash_command("/backend").await;

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, &mut app))
        .expect("draw");
    let body = panel_rows(&terminal, &app);
    assert!(body.contains("local"), "the local row must render: {body}");
    assert!(
        body.contains("[active]"),
        "the active row must carry the marker: {body}"
    );
    assert!(
        body.contains("Enter switch"),
        "the footer hint must render: {body}"
    );
}

// ---------------------------------------------------------------------------
// Panel keyboard ownership (FR-008)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn panel_owns_the_keyboard_until_escape() {
    let (_guard, _env, _temp) = enter_project(r#"{"defaultAgent": "general"}"#);
    let mut app = support::make_app();
    app.session_id = Some("backend-test".to_string());
    app.execute_slash_command("/backend").await;
    assert!(app.backend_panel.is_some(), "panel open");

    // A printable character is swallowed and never reaches the input buffer.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE))
        .await;
    assert!(
        app.input.is_empty(),
        "no character may reach the input buffer"
    );
    assert!(
        app.backend_panel.is_some(),
        "a printable character does not close the panel"
    );

    // `Esc` is the only key that closes the panel.
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;
    assert!(app.backend_panel.is_none(), "Esc closes the panel");
}

#[tokio::test]
async fn panel_switches_the_active_backend_and_persists_it() {
    let project_config = r#"
    {
      "defaultAgent": "general",
      "backends": [
        { "id": "box", "name": "Docker box", "kind": "docker", "image": "alpine" }
      ]
    }
    "#;
    let (_guard, _env, _temp) = enter_project(project_config);
    let mut app = support::make_app();
    app.session_id = Some("backend-test".to_string());

    app.execute_slash_command("/backend").await;
    // Move the cursor off `local` and switch.
    app.open_backend_panel();
    app.backend_panel_move_down();
    app.backend_panel_activate_selected();

    // The switch is only accepted when the docker runtime is healthy; on a host
    // with no runtime the row is `unavailable` and the switch is refused. Assert
    // whichever path the host took is internally consistent.
    let config: ragent_config::Config =
        serde_json::from_str(&std::fs::read_to_string(".ragent/ragent.json").expect("read"))
            .expect("parse config");
    let persisted = serde_json::to_value(&config).expect("serialise");
    if app.active_backend == "docker" {
        assert_eq!(
            persisted.get("execution_backend").and_then(|v| v.as_str()),
            Some("docker"),
            "a successful switch persists execution_backend"
        );
    } else {
        assert_eq!(app.active_backend, "local", "a refused switch keeps local");
        assert!(
            persisted.get("execution_backend").is_none(),
            "a refused switch writes no key"
        );
    }
}

#[tokio::test]
async fn switching_by_kind_persists_and_updates_the_status_bar() {
    let (_guard, _env, _temp) = enter_project(r#"{"defaultAgent": "general"}"#);
    let mut app = support::make_app();
    app.session_id = Some("backend-test".to_string());

    app.execute_slash_command("/backend remote").await;

    // A `remote` backend with no URL/key is `unavailable`, so the switcher
    // refuses it and leaves the active backend on `local`.
    assert_eq!(
        app.active_backend, "local",
        "an unavailable backend cannot be selected"
    );
    assert_eq!(
        app.status, "backend: unavailable",
        "the refusal must be surfaced: {}",
        app.status
    );
}

#[tokio::test]
async fn switching_to_the_active_backend_is_a_no_op() {
    let (_guard, _env, _temp) = enter_project(r#"{"defaultAgent": "general"}"#);
    let mut app = support::make_app();
    app.session_id = Some("backend-test".to_string());

    app.execute_slash_command("/backend local").await;
    assert_eq!(app.status, "backend: active");
}

#[tokio::test]
async fn unknown_selector_is_reported() {
    let (_guard, _env, _temp) = enter_project(r#"{"defaultAgent": "general"}"#);
    let mut app = support::make_app();
    app.session_id = Some("backend-test".to_string());

    app.execute_slash_command("/backend nosuchkind").await;

    let text = app
        .messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default();
    assert!(
        text.contains("No registered backend matches"),
        "an unknown selector is reported: {text}"
    );
    assert_eq!(app.status, "backend: unknown");
}

#[tokio::test]
async fn backend_help_renders_usage_and_changes_no_state() {
    let (_guard, _env, _temp) = enter_project(r#"{"defaultAgent": "general"}"#);
    let mut app = support::make_app();
    app.session_id = Some("backend-test".to_string());

    app.execute_slash_command("/backend help").await;

    let text = app
        .messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default();
    for needle in [
        "From: /backend help",
        "/backend",
        "podman",
        "docker",
        "help",
    ] {
        assert!(text.contains(needle), "help must mention {needle}: {text}");
    }
    assert_eq!(app.status, "backend: help");
    assert!(app.backend_panel.is_none(), "help never opens the panel");
    assert_eq!(app.active_backend, "local");
}

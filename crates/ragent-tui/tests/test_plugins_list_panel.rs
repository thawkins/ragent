//! `/plugins list` panel tests (spec `plugins` FR-009, FR-030).
//!
//! `/plugins list [--verbose]` opens an interactive panel: the plugin table is
//! pinned at the top (a fixed column heading and separator) above a scrollable
//! list under a block cursor, and the capability blocks (contributions,
//! unsupported capabilities, errors, summary, and - verbose - telemetry) are
//! listed at the end of the same scrollable body. `Up`/`Down` move the cursor and
//! `Esc` closes the panel.
//!
//! The store is redirected into a tempdir so nothing touches the developer's real
//! project. Env/cwd mutation is `unsafe` in edition 2024; the workspace denies
//! `unsafe_code`, so this target opts back in explicitly (env mutation is
//! contained to the binary).
#![allow(unsafe_code)]
// The test serialiser is intentionally held across the async `#[tokio::test]`
// bodies so cwd/env mutations cannot interleave (mirrors `test_mcp_discover_panel`).
#![allow(clippy::await_holding_lock)]
#![cfg(test)]

use std::sync::{Mutex, OnceLock};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;

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
    dir_name: &str,
    count: usize,
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

    for i in 0..count {
        write_codex_plugin(
            &temp
                .path()
                .join(".ragent")
                .join("plugins")
                .join(format!("{dir_name}-{i:02}")),
            &format!("{dir_name}-{i:02}"),
        );
    }
    (guard, env_guard, temp)
}

/// Write a minimal Codex-dialect plugin that registers one tool and one command.
fn write_codex_plugin(dir: &std::path::Path, id: &str) {
    std::fs::create_dir_all(dir).expect("plugin dir");
    let manifest = format!(
        r#"{{ "id": "{id}", "name": "{id}", "version": "1.0.0", "entry": "index.js",
             "tools": [{{ "name": "ping", "description": "Ping", "parameters": {{ "type": "object" }} }}],
             "commands": [{{ "name": "{id}-go", "description": "Go" }}] }}"#
    );
    std::fs::write(dir.join("codex-plugin.json"), manifest).expect("manifest");
    // The entry registers a tool distinct from the manifest-declared `ping`, so a
    // real enable (which merges manifest and runtime tools) has no duplicate name.
    std::fs::write(
        dir.join("index.js"),
        "ragent.register_tool({ name: \"pong\", description: \"Pong\", parameters: { type: \"object\" }, handler: function(){ return \"pong\"; } });",
    )
    .expect("entry");
}

/// Press one key through the full input path.
async fn press(app: &mut App, code: KeyCode) {
    app.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE))
        .await;
}

/// The rendered screen as one string per row.
fn render_rows(app: &mut App) -> Vec<String> {
    let backend = TestBackend::new(160, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render app");
    (0..40)
        .map(|y| {
            (0..160)
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

/// The rendered screen as a background-colour grid, so a highlight can be found.
fn render_bgs(app: &mut App) -> Vec<Vec<Color>> {
    let backend = TestBackend::new(160, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render app");
    (0..40)
        .map(|y| {
            (0..160)
                .map(|x| {
                    terminal
                        .backend()
                        .buffer()
                        .cell((x, y))
                        .map(|c| c.bg)
                        .unwrap_or(Color::Reset)
                })
                .collect()
        })
        .collect()
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn plugins_list_opens_a_panel_with_the_pinned_table_and_capabilities() {
    let (_lock, _guard, _temp) = enter_project("codex-weather", 1);
    let mut app = support::make_app();
    app.cwd_path = std::env::current_dir().expect("cwd");
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;

    // The panel owns the keyboard; the message window gets no appended report.
    assert!(
        app.plugins_list.is_some(),
        "`/plugins list` must open the interactive panel"
    );
    let rows = render_rows(&mut app);
    let screen = rows.join("\n");
    assert!(
        screen.contains("/plugins list"),
        "the panel title must be visible: {screen}"
    );
    // Pinned table heading and separator.
    assert!(
        rows.iter().any(|r| r.contains("ID")
            && r.contains("Name")
            && r.contains("Version")
            && r.contains("MCP Tools")),
        "the pinned table header must be visible: {screen}"
    );
    // The plugin row and the trailing contributions block. Discovery of a
    // disabled plugin names its manifest-declared tool (`plugin_<id>_<tool>`) and
    // command, so the block is non-empty and proves the trailing section is
    // present at the end of the body.
    assert!(
        screen.contains("codex-weather-00"),
        "the plugin row must be listed: {screen}"
    );
    let body = &app.plugins_list.as_ref().unwrap().view.body;
    assert!(
        body.iter().any(|l| l == "Contributions:"),
        "the capability block must be listed at the end of the body: {body:?}"
    );
    assert!(
        body.iter()
            .any(|l| l.contains("tools [plugin_codex-weather-00_ping]")),
        "the contributed tool must be named in the capability block: {body:?}"
    );
    // The leading body lines are plugin table rows, exposed as `view.rows` so the
    // panel can toggle the highlighted plugin (FR-009).
    assert_eq!(
        app.plugins_list.as_ref().unwrap().view.rows.len(),
        1,
        "one plugin row identity expected"
    );
    assert_eq!(
        app.plugins_list.as_ref().unwrap().view.rows[0].id,
        "codex-weather-00"
    );
    assert!(
        body.iter().any(|l| l.starts_with("- codex-weather-00:")),
        "the contributions line must name the plugin: {body:?}"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn plugins_list_escape_closes_the_panel() {
    let (_lock, _guard, _temp) = enter_project("codex-weather", 1);
    let mut app = support::make_app();
    app.cwd_path = std::env::current_dir().expect("cwd");
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;
    assert!(app.plugins_list.is_some());

    press(&mut app, KeyCode::Esc).await;
    assert!(
        app.plugins_list.is_none(),
        "Esc must close the `/plugins list` panel"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn plugins_list_down_moves_cursor_and_clamps_at_ends() {
    let (_lock, _guard, _temp) = enter_project("codex-weather", 3);
    let mut app = support::make_app();
    app.cwd_path = std::env::current_dir().expect("cwd");
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;
    let last = app
        .plugins_list
        .as_ref()
        .map(|s| s.view.body.len().saturating_sub(1))
        .expect("panel open");

    press(&mut app, KeyCode::Down).await;
    assert_eq!(app.plugins_list.as_ref().unwrap().selected, 1);

    for _ in 0..(last + 5) {
        press(&mut app, KeyCode::Down).await;
    }
    assert_eq!(
        app.plugins_list.as_ref().unwrap().selected,
        last,
        "Down must clamp at the last body line"
    );

    press(&mut app, KeyCode::Up).await;
    assert_eq!(app.plugins_list.as_ref().unwrap().selected, last - 1);

    press(&mut app, KeyCode::Home).await;
    assert_eq!(app.plugins_list.as_ref().unwrap().selected, 0);

    press(&mut app, KeyCode::End).await;
    assert_eq!(app.plugins_list.as_ref().unwrap().selected, last);

    press(&mut app, KeyCode::Up).await;
    press(&mut app, KeyCode::Up).await;
    assert_eq!(app.plugins_list.as_ref().unwrap().selected, last - 2);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn plugins_list_list_scrolls_to_keep_the_cursor_visible() {
    // Enough plugins that the table overflows the panel's body.
    let (_lock, _guard, _temp) = enter_project("codex-plugin", 30);
    let mut app = support::make_app();
    app.cwd_path = std::env::current_dir().expect("cwd");
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;

    let rows = render_rows(&mut app);
    assert!(
        rows.iter().any(|r| r.contains("codex-plugin-00")),
        "the first row must be visible on open: {}",
        rows.join("\n")
    );
    assert_eq!(app.plugins_list.as_ref().unwrap().scroll_offset, 0);

    // Walk the cursor well past one screen.
    for _ in 0..40 {
        press(&mut app, KeyCode::Down).await;
    }
    let selected = app.plugins_list.as_ref().unwrap().selected;
    assert!(selected > 0, "the cursor must have moved");

    // The scroll window is re-clamped on the next render.
    let rows = render_rows(&mut app);
    let screen = rows.join("\n");
    assert!(
        app.plugins_list.as_ref().unwrap().scroll_offset > 0,
        "the list must scroll to follow the cursor"
    );
    assert!(
        screen.contains("codex-plugin-29"),
        "the last table row must stay visible after scrolling: {screen}"
    );
    assert!(
        !screen.contains("codex-plugin-00 "),
        "the first row must have scrolled off: {screen}"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn plugins_list_block_cursor_paints_the_selected_row() {
    let (_lock, _guard, _temp) = enter_project("codex-weather", 2);
    let mut app = support::make_app();
    app.cwd_path = std::env::current_dir().expect("cwd");
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;
    // The block cursor starts on body line 0 (the first plugin table row).
    press(&mut app, KeyCode::Down).await;

    let rows = render_rows(&mut app);
    let bgs = render_bgs(&mut app);
    let selected_y = rows
        .iter()
        .position(|r| r.contains("codex-weather-01"))
        .expect("the second row must be rendered");
    assert!(
        bgs[selected_y].contains(&Color::Magenta),
        "the selected row must carry the block-cursor background"
    );
    let first_y = rows
        .iter()
        .position(|r| r.contains("codex-weather-00"))
        .expect("the first row must be rendered");
    assert!(
        !bgs[first_y].contains(&Color::Magenta),
        "an unselected row must not carry the block-cursor background"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn plugins_list_space_toggles_the_highlighted_plugin() {
    // FR-009/FR-011/FR-012: Space enables the highlighted plugin, a second Space
    // disables it, and the panel refreshes the row state + feedback each time.
    let (_lock, _guard, _temp) = enter_project("codex-weather", 1);
    let mut app = support::make_app();
    app.cwd_path = std::env::current_dir().expect("cwd");
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;
    assert!(
        app.plugins_list
            .as_ref()
            .is_some_and(|s| !s.view.rows[0].enabled),
        "a freshly installed plugin starts disabled"
    );

    // Space enables it.
    press(&mut app, KeyCode::Char(' ')).await;
    let state = app.plugins_list.as_ref().expect("panel open");
    assert!(
        state.view.rows[0].enabled,
        "Space must enable the highlighted plugin"
    );
    assert!(
        state
            .feedback
            .as_deref()
            .is_some_and(|m| m.starts_with("[ok]")),
        "expected an [ok] feedback, got {:?}",
        state.feedback
    );
    let rows = render_rows(&mut app).join("\n");
    assert!(
        rows.contains("Space toggle"),
        "the panel hint must advertise the toggle: {rows}"
    );

    // A second Space disables it again.
    press(&mut app, KeyCode::Char(' ')).await;
    let state = app.plugins_list.as_ref().expect("panel open");
    assert!(
        !state.view.rows[0].enabled,
        "a second Space must disable the plugin"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn plugins_list_feedback_row_shows_the_toggle_outcome() {
    let (_lock, _guard, _temp) = enter_project("codex-weather", 1);
    let mut app = support::make_app();
    app.cwd_path = std::env::current_dir().expect("cwd");
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;
    press(&mut app, KeyCode::Char(' ')).await;

    let rows = render_rows(&mut app).join("\n");
    assert!(
        rows.contains("[ok]") && rows.contains("codex-weather-00"),
        "the feedback line must name the toggled plugin: {rows}"
    );
}

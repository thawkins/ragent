//! Tests for the `ENTER` install path from the plugin-store browser (spec
//! `pluginstores` T-009; FR-006, FR-011, FR-014, FR-022, FR-024, FR-025,
//! FR-026).
//!
//! The install runs through the existing `ragent_plugins::add(force = false)`
//! entry point against a local source directory, so no network is contacted.
//! The off-loop worker deposits its result in `App::plugin_store_install_result`
//! and the UI-thread poll applies it: the footer notice, the re-derived
//! installed set (so the row re-colours), and the message-window report naming
//! the installed id and dialect.

use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ragent_plugins::{StoreEntry, StoreKind};
use ragent_tui::App;
use ragent_tui::app::{PluginStoreInstallResult, PluginStoreStatus};
use ragent_tui::input::handle_key;

#[path = "support/mod.rs"]
mod support;

static TEMP_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A unique scratch directory under `target/temp/` (no `/tmp`, per AGENTS.md).
fn temp_dir(name: &str) -> std::path::PathBuf {
    let unique = TEMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/temp/plugin-store-install/{name}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).expect("temp dir creatable");
    path
}

/// A Codex plugin source directory under `root` with the given id.
fn codex_source(root: &std::path::Path, id: &str) -> std::path::PathBuf {
    let plugin = root.join(format!("{id}-src"));
    std::fs::create_dir_all(&plugin).expect("plugin dir");
    std::fs::write(
        plugin.join("codex-plugin.json"),
        format!(r#"{{ "name": "{id}", "version": "1.0.0", "entry": "index.js" }}"#),
    )
    .expect("manifest");
    std::fs::write(plugin.join("index.js"), "// entry").expect("entry");
    plugin
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn entry(id: &str, source: &str) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: "1.0.0".to_string(),
        source: source.to_string(),
        description: String::new(),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
    }
}

/// An app with the Codex panel open, a fake session, and the given entries.
fn open_with_entries(cwd: std::path::PathBuf, entries: Vec<StoreEntry>) -> App {
    let mut app = support::make_app();
    app.cwd_path = cwd;
    app.session_id = Some("s1".to_string());
    app.open_plugin_store(StoreKind::Codex, "", false);
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_entries(entries);
    app
}

fn last_message(app: &App) -> String {
    app.messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default()
}

/// Bounded wait for the off-loop install worker to deposit its result, then
/// drain it through the poll.
fn drain_install(app: &mut App) {
    let mut delivered = false;
    for _ in 0..500 {
        if app
            .plugin_store_install_result
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
        {
            delivered = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(delivered, "the off-loop install delivered its result");
    app.poll_plugin_store_install_result();
}

// ── ENTER installs and re-colours (FR-006, FR-011) ──────────────────────────

#[test]
fn enter_installs_a_local_source_and_the_row_re_colours() {
    let temp = temp_dir("install-ok");
    let source = codex_source(&temp, "codex-weather");
    let mut app = open_with_entries(
        temp.clone(),
        vec![entry("codex-weather", source.to_str().unwrap())],
    );

    let action = handle_key(&mut app, key(KeyCode::Enter));

    assert!(action.is_none(), "ENTER is routed to the panel");
    assert_eq!(
        app.plugin_store
            .as_ref()
            .expect("panel open")
            .last_install
            .as_deref(),
        Some("installing codex-weather..."),
        "progress is shown while the install runs (FR-011)"
    );

    drain_install(&mut app);

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert!(
        browser.is_installed("codex-weather"),
        "the freshly installed id joins the installed set so the row re-colours (FR-011)"
    );
    assert_eq!(
        browser.last_install.as_deref(),
        Some("installed codex-weather"),
        "the footer reports the installed id"
    );
    let report = last_message(&app);
    assert!(
        report.contains("Installed plugin `codex-weather`") && report.contains("dialect: codex"),
        "the message window reports the installed id and dialect (FR-006), got: {report:?}"
    );
}

#[test]
fn the_installed_plugin_lands_in_the_store_scan() {
    let temp = temp_dir("install-scan");
    let source = codex_source(&temp, "codex-weather");
    let mut app = open_with_entries(
        temp.clone(),
        vec![entry("codex-weather", source.to_str().unwrap())],
    );

    handle_key(&mut app, key(KeyCode::Enter));
    drain_install(&mut app);

    // The install committed into the project store the app's cwd resolves to.
    let installed = temp.join(".ragent").join("plugins").join("codex-weather");
    assert!(
        installed.join("codex-plugin.json").exists(),
        "the plugin files are in the project store: {}",
        installed.display()
    );
}

// ── Double-install guard (FR-014) ───────────────────────────────────────────

#[test]
fn a_second_enter_on_an_installed_result_is_refused_without_writing() {
    let temp = temp_dir("install-double");
    let source = codex_source(&temp, "codex-weather");
    let mut app = open_with_entries(
        temp.clone(),
        vec![entry("codex-weather", source.to_str().unwrap())],
    );

    handle_key(&mut app, key(KeyCode::Enter));
    drain_install(&mut app);
    assert!(app.plugin_store_install_result.lock().unwrap().is_none());

    // Second ENTER on the now-installed row: refused, no install requested.
    handle_key(&mut app, key(KeyCode::Enter));

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(
        browser.last_install.as_deref(),
        Some("plugin codex-weather is already installed"),
        "the re-install is refused (FR-014)"
    );
    assert!(
        app.plugin_store_install_result.lock().unwrap().is_none(),
        "no second install was spawned (FR-014)"
    );
}

// ── Failure reporting (FR-024, FR-025) ──────────────────────────────────────

#[test]
fn a_non_https_source_is_refused_and_reported_without_panic() {
    let temp = temp_dir("install-bad-scheme");
    let mut app = open_with_entries(
        temp.clone(),
        vec![entry("codex-weather", "http://example.com/plugin.zip")],
    );

    handle_key(&mut app, key(KeyCode::Enter));
    drain_install(&mut app);

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert!(
        browser
            .last_install
            .as_deref()
            .is_some_and(|n| n.starts_with("install failed:")),
        "the panel footer names the failure, got: {:?}",
        browser.last_install
    );
    let report = last_message(&app);
    assert!(
        report.contains("[err]"),
        "the message window carries an error report (FR-025), got: {report:?}"
    );
    // The panel stays open and dismissible (FR-013/FR-025).
    assert!(app.plugin_store.is_some());
}

#[test]
fn a_missing_source_is_reported_as_a_failure() {
    let temp = temp_dir("install-missing");
    let mut app = open_with_entries(temp.clone(), vec![entry("codex-weather", "does/not/exist")]);

    handle_key(&mut app, key(KeyCode::Enter));
    drain_install(&mut app);

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert!(
        browser
            .last_install
            .as_deref()
            .is_some_and(|n| n.starts_with("install failed:")),
        "a missing source is a contained failure, got: {:?}",
        browser.last_install
    );
    assert!(last_message(&app).contains("[err]"));
}

// ── Report application semantics (FR-006, FR-011, FR-025) ───────────────────

#[test]
fn a_success_result_re_colours_and_clears_the_installing_notice() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.open_plugin_store(StoreKind::Codex, "", false);
    app.plugin_store.as_mut().expect("panel open").last_install =
        Some("installing codex-weather...".to_string());

    // Deposit a finished success result directly, then poll it (deterministic,
    // no worker thread).
    if let Ok(mut slot) = app.plugin_store_install_result.lock() {
        *slot = Some(PluginStoreInstallResult {
            kind: StoreKind::Codex,
            id: "codex-weather".to_string(),
            notice: "installed codex-weather".to_string(),
            report: "From: /plugins add\n\n[ok] Installed plugin `codex-weather` (dialect: codex)."
                .to_string(),
            succeeded: true,
        });
    }
    app.poll_plugin_store_install_result();

    assert_eq!(
        app.plugin_store
            .as_ref()
            .expect("panel open")
            .last_install
            .as_deref(),
        Some("installed codex-weather")
    );
    assert!(last_message(&app).contains("Installed plugin `codex-weather`"));
}

#[test]
fn a_result_for_a_closed_panel_still_reports_but_touches_nothing() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    // No panel open.
    if let Ok(mut slot) = app.plugin_store_install_result.lock() {
        *slot = Some(PluginStoreInstallResult {
            kind: StoreKind::Codex,
            id: "codex-weather".to_string(),
            notice: "installed codex-weather".to_string(),
            report: "From: /plugins add\n\n[ok] Installed plugin `codex-weather` (dialect: codex)."
                .to_string(),
            succeeded: true,
        });
    }

    app.poll_plugin_store_install_result();

    assert!(app.plugin_store.is_none(), "no panel to fill");
    assert!(
        last_message(&app).contains("Installed plugin `codex-weather`"),
        "the report is still surfaced once the install has happened"
    );
}

#[test]
fn a_foreign_store_result_never_fills_another_stores_panel() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.open_plugin_store(StoreKind::Codex, "", false);

    // A result tagged for the Claude store arrives while the Codex panel is
    // open: it must not set the Codex panel's notice.
    if let Ok(mut slot) = app.plugin_store_install_result.lock() {
        *slot = Some(PluginStoreInstallResult {
            kind: StoreKind::Claude,
            id: "claude-todo".to_string(),
            notice: "installed claude-todo".to_string(),
            report: "From: /plugins add\n\n[ok] Installed plugin `claude-todo` (dialect: claude)."
                .to_string(),
            succeeded: true,
        });
    }
    app.poll_plugin_store_install_result();

    assert_eq!(
        app.plugin_store.as_ref().expect("panel").kind,
        StoreKind::Codex
    );
    assert!(
        app.plugin_store
            .as_ref()
            .expect("panel")
            .last_install
            .is_none(),
        "the Codex panel is untouched by a Claude result"
    );
}

// ── No install without ENTER (FR-022) ───────────────────────────────────────

#[test]
fn typing_and_moving_never_spawn_an_install() {
    let temp = temp_dir("install-no-enter");
    let source = codex_source(&temp, "codex-weather");
    let mut app = open_with_entries(
        temp.clone(),
        vec![entry("codex-weather", source.to_str().unwrap())],
    );

    handle_key(&mut app, key(KeyCode::Char('w')));
    handle_key(&mut app, key(KeyCode::Down));
    handle_key(&mut app, key(KeyCode::Up));
    handle_key(&mut app, key(KeyCode::Backspace));

    assert!(
        app.plugin_store_install_result.lock().unwrap().is_none(),
        "browsing, typing, and moving install nothing (FR-022)"
    );
    assert!(
        !temp.join(".ragent").join("plugins").exists(),
        "no store directory was written (FR-022)"
    );
}

#[test]
fn enter_with_no_result_highlighted_records_a_neutral_notice_and_installs_nothing() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.open_plugin_store(StoreKind::Codex, "", false);
    assert_eq!(
        app.plugin_store.as_ref().expect("panel").status,
        PluginStoreStatus::Loading
    );

    handle_key(&mut app, key(KeyCode::Enter));

    assert_eq!(
        app.plugin_store
            .as_ref()
            .expect("panel")
            .last_install
            .as_deref(),
        Some("no result highlighted")
    );
    assert!(app.plugin_store_install_result.lock().unwrap().is_none());
}

// ── Poll plumbing (FR-025, FR-026) ──────────────────────────────────────────

#[test]
fn polling_with_nothing_pending_is_a_noop() {
    let mut app = support::make_app();
    app.poll_plugin_store_install_result();
    assert!(app.messages.is_empty());
}

#[test]
fn the_poll_recovers_a_poisoned_slot_without_panicking() {
    let mut app = support::make_app();
    let slot = std::sync::Arc::clone(&app.plugin_store_install_result);
    let handle = std::thread::spawn(move || {
        let _guard = slot.lock().expect("slot");
        panic!("poison the slot");
    });
    assert!(handle.join().is_err(), "the thread panicked as intended");

    // Must return without panicking.
    app.poll_plugin_store_install_result();
}

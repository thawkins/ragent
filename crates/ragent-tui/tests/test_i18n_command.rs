//! `/i18n` TUI dispatch + status-bar translation tests (spec `openhands`
//! T-014; FR-027).
//!
//! Verifies the `handle_i18n_command` dispatch arm through the public
//! `App::execute_slash_command` entry point, and that the status bar renders a
//! translated label from an installed locale catalog while falling back to
//! English for untranslated keys.
//!
//! Persistence tests redirect cwd + `XDG_CONFIG_HOME` into a tempdir so the
//! toggle never touches the developer's real global config. Env mutation is
//! `unsafe` in edition 2024; the workspace denies `unsafe_code`, so this test
//! target opts back in explicitly (env mutation is contained to the binary).
#![allow(unsafe_code)]
#![cfg(test)]

use std::sync::{Mutex, OnceLock};

use ragent_tui::{App, layout};
use ratatui::{Terminal, backend::TestBackend};

#[path = "support/mod.rs"]
mod support;

/// The i18n runtime state and the cwd/env mutations below are process-global;
/// one mutex serialises every test in this binary against all of them.
#[allow(clippy::await_holding_lock)]
fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Restores cwd, config env vars, and the i18n runtime on drop.
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
                ("LANG", std::env::var("LANG").ok()),
                ("LC_ALL", std::env::var("LC_ALL").ok()),
                ("LC_MESSAGES", std::env::var("LC_MESSAGES").ok()),
            ],
            // A prior test can momentarily leave the cwd pointing at a
            // just-deleted tempdir; fall back to `.` so setup never panics.
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
        // NOTE: the i18n runtime is reset at the *start* of
        // `enter_isolated_project` (under `test_lock`), not here - a drop-time
        // reset would run after the lock is released and race the next test.
        std::env::set_current_dir(&self.cwd).ok();
    }
}

/// Enter a fresh tempdir project with an isolated global config dir, a scratch
/// project config, and an installed `zz` locale catalog.
fn enter_isolated_project() -> (
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
    // Ensure no ambient locale auto-enables i18n and skews the assertions.
    unsafe { std::env::remove_var("LC_ALL") };
    unsafe { std::env::remove_var("LC_MESSAGES") };
    unsafe { std::env::set_var("LANG", "C") };
    std::env::set_current_dir(temp.path()).expect("set cwd");

    let ragent_dir = temp.path().join(".ragent");
    std::fs::create_dir_all(&ragent_dir).expect("create .ragent");
    std::fs::write(
        ragent_dir.join("ragent.json"),
        r#"{"defaultAgent": "general"}"#,
    )
    .expect("write scratch project config");

    // Partial catalog: `status.project` translated, everything else falls back.
    let locales = ragent_dir.join("locales");
    std::fs::create_dir_all(&locales).expect("create locales dir");
    std::fs::write(
        locales.join("zz.json"),
        r#"{"locale":"zz","messages":{"status.project":"PROJET: "}}"#,
    )
    .expect("write zz catalog");

    ragent_config::i18n::reset();
    (guard, env_guard, temp)
}

/// Return the text of the most recently appended message.
fn last_message_text(app: &App) -> String {
    app.messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default()
}

/// Render the app at 120x40 and return the visible frame text.
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

// ---------------------------------------------------------------------------
// Help / status paths
// ---------------------------------------------------------------------------

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_help_renders_usage() {
    let _guard = test_lock();
    let _env = EnvGuard::new();
    let mut app = support::make_app();
    app.execute_slash_command("/i18n help").await;

    let text = last_message_text(&app);
    assert!(
        text.contains("From: /i18n help") && text.contains("/i18n on"),
        "help must render the usage table, got: {text}"
    );
    assert_eq!(app.status, "i18n: help", "/i18n help status");
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_help_aliases() {
    let _guard = test_lock();
    let _env = EnvGuard::new();
    for input in ["/i18n --help", "/i18n -h"] {
        let mut app = support::make_app();
        app.execute_slash_command(input).await;
        let text = last_message_text(&app);
        assert!(
            text.contains("From: /i18n help"),
            "help form '{input}' must render usage, got: {text}"
        );
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_status_reports_state() {
    let (_guard, _env, _temp) = enter_isolated_project();
    let mut app = support::make_app();
    app.execute_slash_command("/i18n status").await;

    let text = last_message_text(&app);
    assert!(
        text.contains("From: /i18n status") && text.contains("UI internationalisation"),
        "status must report the state, got: {text}"
    );
    assert_eq!(app.status, "i18n: status");
}

// ---------------------------------------------------------------------------
// Enable / disable + persistence
// ---------------------------------------------------------------------------

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_enable_translates_and_statusbar_renders_catalog() {
    let (_guard, _env, _temp) = enter_isolated_project();
    let mut app = support::make_app();

    // Disabled by default: the status bar shows the English label.
    assert!(!ragent_config::i18n::is_enabled());
    let english_frame = render_app_to_string(&mut app);
    assert!(
        english_frame.contains("Project: "),
        "with i18n off the English label must render"
    );

    // Enabling i18n with the `zz` locale applies + persists.
    app.execute_slash_command("/i18n zz").await;
    assert!(
        ragent_config::i18n::is_enabled(),
        "/i18n <locale> must enable i18n"
    );
    assert!(ragent_config::i18n::catalog_loaded());
    assert_eq!(ragent_config::i18n::active_locale(), "zz");

    // The translated key renders; an untranslated key falls back to English.
    let translated_frame = render_app_to_string(&mut app);
    assert!(
        translated_frame.contains("PROJET: "),
        "the translated label must render from the catalog: {translated_frame}"
    );
    assert!(
        !translated_frame.contains("Project: "),
        "the English label must be replaced while i18n is on: {translated_frame}"
    );

    // The change persisted to the project config.
    let persisted = ragent_config::Config::load().expect("load config");
    let section = persisted.i18n.expect("i18n section persisted");
    assert!(section.enabled);
    assert_eq!(section.locale, "zz");
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_off_restores_english() {
    let (_guard, _env, _temp) = enter_isolated_project();
    let mut app = support::make_app();

    app.execute_slash_command("/i18n zz").await;
    assert!(ragent_config::i18n::is_enabled());

    app.execute_slash_command("/i18n off").await;
    assert!(
        !ragent_config::i18n::is_enabled(),
        "/i18n off must disable translation"
    );
    let frame = render_app_to_string(&mut app);
    assert!(
        frame.contains("Project: ") && !frame.contains("PROJET: "),
        "English must be restored after /i18n off: {frame}"
    );

    let persisted = ragent_config::Config::load().expect("load config");
    let section = persisted.i18n.expect("i18n section persisted");
    assert!(!section.enabled);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_missing_catalog_falls_back_to_english() {
    let (_guard, _env, _temp) = enter_isolated_project();
    let mut app = support::make_app();

    // No catalog installed for `qq`.
    app.execute_slash_command("/i18n qq").await;
    assert!(ragent_config::i18n::is_enabled());
    assert!(
        !ragent_config::i18n::catalog_loaded(),
        "a locale with no catalog must not load one"
    );
    let frame = render_app_to_string(&mut app);
    assert!(
        frame.contains("Project: "),
        "a missing catalog must fall back to English, never blank: {frame}"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_list_reports_installed_catalogs() {
    let (_guard, _env, _temp) = enter_isolated_project();
    let mut app = support::make_app();
    app.execute_slash_command("/i18n list").await;

    let text = last_message_text(&app);
    assert!(
        text.contains("From: /i18n list") && text.contains("zz"),
        "list must name the installed `zz` catalog, got: {text}"
    );
}

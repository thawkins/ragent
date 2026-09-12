//! `/gcf` TUI dispatch tests (spec `gcf`, T-010; FR-002, FR-003).
//!
//! Verifies the `handle_gcf_command` dispatch arm through the public
//! `App::execute_slash_command` entry point:
//!
//! - `/gcf on` persists `gcf.enabled = true` to the loaded config source,
//!   applies the runtime flag, and confirms in the transcript (FR-002);
//! - `/gcf off` persists `false` and omits the section again (FR-001/FR-002);
//! - `/gcf help` (plus bare `/gcf`, `--help`, `-h`) renders the usage table
//!   and never changes state (FR-003, AC-8);
//! - `/gcf show` reports the effective state and its source (FR-003);
//! - unknown subcommands are rejected with a usage notice and no state or
//!   config change (FR-003 unwanted path, AC-7).
//!
//! Persistence tests redirect cwd + `XDG_CONFIG_HOME` into a tempdir so the
//! toggle never touches the developer's real global config. Env mutation is
//! `unsafe` in edition 2024; the workspace denies `unsafe_code`, so this test
//! target opts back in explicitly (env mutation is contained to the binary).
#![allow(unsafe_code)]
#![cfg(test)]

use std::sync::{Mutex, OnceLock};

use ragent_tui::App;

#[path = "support/mod.rs"]
mod support;

/// The GCF runtime flag is process-global and the cwd/env mutations below are
/// too; one mutex serialises every test in this binary against both.
fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Toggle the GCF flag for the duration of `f` and always restore it.
fn with_gcf(enabled: bool, f: impl FnOnce()) {
    let previous = ragent_config::gcf::is_enabled();
    ragent_config::gcf::set_enabled(enabled);
    f();
    ragent_config::gcf::set_enabled(previous);
}

/// Restores cwd and config-related env vars on drop.
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
            cwd: std::env::current_dir().expect("cwd"),
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

/// Acquire the test lock and move into a fresh temporary project directory
/// with an isolated global config dir and a scratch project config, so
/// `persist_gcf` writes to the tempdir's `.ragent/ragent.json` and never to
/// the developer's real global config.
fn enter_isolated_config_project() -> (
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
    std::fs::write(
        ragent_dir.join("ragent.json"),
        r#"{"defaultAgent": "general"}"#,
    )
    .expect("write scratch project config");

    (guard, env_guard, temp)
}

/// Return the text of the most recently appended message.
fn last_message_text(app: &App) -> String {
    app.messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default()
}

/// The scratch project config path for the current isolated cwd.
fn scratch_config_path() -> std::path::PathBuf {
    std::env::current_dir()
        .expect("cwd")
        .join(".ragent")
        .join("ragent.json")
}

// ---------------------------------------------------------------------------
// Help / no-op paths (FR-003, AC-8)
// ---------------------------------------------------------------------------

#[test]
fn test_gcf_help_renders_usage_without_state_change() {
    let _guard = test_lock();
    let mut app = support::make_app();
    app.execute_slash_command("/gcf help");

    let text = last_message_text(&app);
    assert!(
        text.contains("/gcf on") && text.contains("/gcf off"),
        "help must advertise the on and off subcommands, got: {text}"
    );
    assert!(
        text.contains("/gcf show"),
        "help must advertise the show subcommand, got: {text}"
    );
    assert_eq!(app.status, "gcf: help", "/gcf help status");
    // Help must not touch the runtime flag or the persisted config.
    assert!(
        !ragent_config::gcf::is_enabled(),
        "help must leave GCF disabled by default"
    );
    let persisted = ragent_config::Config::load().is_ok_and(|c| c.gcf.enabled);
    assert!(!persisted, "help must not persist gcf.enabled");
}

#[test]
fn test_gcf_help_aliases_bare_dashdash_and_h() {
    let _guard = test_lock();
    for input in ["/gcf", "/gcf --help", "/gcf -h"] {
        let mut app = support::make_app();
        app.execute_slash_command(input);
        let text = last_message_text(&app);
        assert!(
            text.contains("/gcf on"),
            "help form '{input}' must render the usage table, got: {text}"
        );
        assert_eq!(app.status, "gcf: help", "help form '{input}' status");
    }
}

// ---------------------------------------------------------------------------
// Unknown subcommand rejection (FR-003 unwanted path, AC-7)
// ---------------------------------------------------------------------------

#[test]
fn test_gcf_unknown_subcommand_rejected_without_state_change() {
    let _guard = test_lock();
    let mut app = support::make_app();
    app.execute_slash_command("/gcf bogus");
    let text = last_message_text(&app);

    assert!(
        text.contains("Unknown subcommand"),
        "unknown subcommand must be rejected with a usage notice, got: {text}"
    );
    assert!(
        text.contains("/gcf on|off|show|help"),
        "rejection must list the valid subcommands, got: {text}"
    );
    assert_eq!(app.status, "gcf: usage", "unknown subcommand status");
    assert!(
        !ragent_config::gcf::is_enabled(),
        "state must be unchanged by an unknown subcommand"
    );
    let persisted = ragent_config::Config::load().is_ok_and(|c| c.gcf.enabled);
    assert!(
        !persisted,
        "config file must be untouched by an unknown subcommand"
    );
}

// ---------------------------------------------------------------------------
// Persist paths (FR-002, AC-2): write to an isolated config source, verify
// the persisted value and the runtime flag; the tempdir isolates the write.
// ---------------------------------------------------------------------------

#[test]
fn test_gcf_on_persists_enabled_to_config_source() {
    let (_guard, _env, _temp) = enter_isolated_config_project();
    with_gcf(false, || {
        let config_path = scratch_config_path();

        let mut app = support::make_app();
        app.execute_slash_command("/gcf on");

        // The runtime flag is applied (live session, FR-002).
        assert!(
            ragent_config::gcf::is_enabled(),
            "in-memory GCF state must be enabled after /gcf on"
        );
        // The persisted config now carries gcf.enabled = true (FR-002).
        let persisted = ragent_config::Config::load()
            .expect("reload config")
            .gcf
            .enabled;
        assert!(persisted, "gcf.enabled must be persisted as true");
        let file_text = std::fs::read_to_string(&config_path).expect("read back");
        assert!(
            file_text.contains("\"gcf\"") && file_text.contains("\"enabled\": true"),
            "config file must contain the gcf section, got: {file_text}"
        );
        // The confirmation notice must report persistence.
        let text = last_message_text(&app);
        assert!(
            text.contains("persisted to the config file"),
            "confirmation notice must mention persistence, got: {text}"
        );
        assert_eq!(app.status, "gcf: on", "/gcf on status");
    });
}

#[test]
fn test_gcf_off_persists_disabled_to_config_source() {
    let (_guard, _env, _temp) = enter_isolated_config_project();
    with_gcf(true, || {
        let config_path = scratch_config_path();

        let mut app = support::make_app();
        app.execute_slash_command("/gcf off");

        assert!(
            !ragent_config::gcf::is_enabled(),
            "in-memory GCF state must be disabled after /gcf off"
        );
        let persisted = ragent_config::Config::load()
            .expect("reload config")
            .gcf
            .enabled;
        assert!(!persisted, "gcf.enabled must be persisted as false");
        let file_text = std::fs::read_to_string(&config_path).expect("read back");
        // The section is omitted while disabled (FR-001 skip rule), so the
        // file must not carry a gcf key at all.
        assert!(
            !file_text.contains("\"gcf\""),
            "disabled state must omit the gcf key from the config file, got: {file_text}"
        );
        assert_eq!(app.status, "gcf: off", "/gcf off status");
    });
}

#[test]
fn test_gcf_show_reports_effective_state_and_source() {
    let (_guard, _env, _temp) = enter_isolated_config_project();
    with_gcf(true, || {
        let mut app = support::make_app();
        app.execute_slash_command("/gcf show");

        let text = last_message_text(&app);
        assert!(
            text.contains("**on**"),
            "/gcf show must report the on state, got: {text}"
        );
        assert!(
            text.contains("in-session change (unsaved)"),
            "show must attribute the runtime-only state to an unsaved in-session change, got: {text}"
        );
        assert_eq!(app.status, "gcf: show", "/gcf show status");
    });
}

/// `/gcf show` with no runtime-flag change and no gcf section in the file
/// must report the default-off source (FR-003).
#[test]
fn test_gcf_show_reports_default_off_source() {
    let (_guard, _env, _temp) = enter_isolated_config_project();
    with_gcf(false, || {
        let mut app = support::make_app();
        app.execute_slash_command("/gcf show");

        let text = last_message_text(&app);
        assert!(
            text.contains("**off**"),
            "/gcf show must report the off state, got: {text}"
        );
        assert!(
            text.contains("default-off"),
            "show must report the default-off source, got: {text}"
        );
    });
}

//! Shared helpers for the `/slash` dispatch test families.
//!
//! Provides an isolated-project harness ([`enter_project`]) and small app/
//! message accessors. Each test binary that needs cwd/env isolation includes
//! this module via `#[path = "slash_support/mod.rs"] mod slash_support;`.
//!
//! Env mutation is `unsafe` in edition 2024; the workspace denies `unsafe_code`,
//! so each test target that pulls this module opts back in with a local
//! `#![allow(unsafe_code)]` (env mutation is contained to the test).

#![allow(dead_code)]

use std::sync::MutexGuard;

use ragent_tui::App;

/// The cwd/env mutations in [`EnvGuard`] are process-global; one mutex
/// serialises every test against them.
///
/// Consolidated test binaries share one process, so this delegates to the
/// binary-wide lock in `tests/all.rs` rather than a module-local one; a
/// module-local mutex would only serialise the children that happen to load
/// this helper. It recovers from poisoning so one panicking test does not
/// cascade.
pub fn test_lock() -> MutexGuard<'static, ()> {
    crate::global_env_lock()
}

/// Restores cwd and config-related env vars on drop.
pub struct EnvGuard {
    keys: Vec<(&'static str, Option<String>)>,
    cwd: std::path::PathBuf,
}

impl EnvGuard {
    /// Capture the current config env vars and cwd for restoration on drop.
    pub fn new() -> Self {
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

impl Default for EnvGuard {
    fn default() -> Self {
        Self::new()
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

/// Enter an isolated tempdir project with the given `.ragent/ragent.json`
/// contents, isolated `XDG_CONFIG_HOME`, and cleared `RAGENT_CONFIG*`.
///
/// The returned guard must be bound (`let (_guard, _env, _temp) = ...`) so the
/// cwd/env restoration survives the test body.
#[allow(clippy::await_holding_lock)]
pub fn enter_project(config: &str) -> (MutexGuard<'static, ()>, EnvGuard, tempfile::TempDir) {
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

/// Build an [`App`] with a live session so `append_assistant_text` records
/// messages into the message window.
pub fn app_with_session() -> App {
    let mut app = super::support::make_app();
    app.session_id = Some("slash-support".to_string());
    app
}

/// Text of the most recent message in the message window.
pub fn last_message(app: &App) -> String {
    app.messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default()
}

/// Join every message in the window into one string (for substring asserts
/// that may span several appended assistant messages).
pub fn all_messages(app: &App) -> String {
    app.messages
        .iter()
        .map(ragent_types::message::Message::text_content)
        .collect::<Vec<_>>()
        .join("\n")
}

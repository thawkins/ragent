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

/// Environment variables that make the TUI treat a provider as configured
/// (`App::detect_provider`): every provider's credential variables plus the
/// Bedrock (`AWS_*`) and Ollama (`OLLAMA_HOST`) probes.
///
/// [`enter_project`] clears them so a fixture that "configures no provider"
/// does not inherit one from the host (a CI runner or cloud container commonly
/// exports `GITHUB_TOKEN` or AWS keys).
fn provider_env_vars() -> Vec<&'static str> {
    let mut vars: Vec<&'static str> = ragent_tui::app::PROVIDER_LIST
        .iter()
        .flat_map(|(pid, _)| ragent_config::credential_env::provider_credential_env_vars(pid))
        .copied()
        .collect();
    vars.extend(["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "OLLAMA_HOST"]);
    vars.sort_unstable();
    vars.dedup();
    vars
}

/// Restores cwd and config- and provider-related env vars on drop.
pub struct EnvGuard {
    keys: Vec<(&'static str, Option<String>)>,
    cwd: std::path::PathBuf,
}

impl EnvGuard {
    /// Capture the current config and provider env vars and cwd for
    /// restoration on drop.
    pub fn new() -> Self {
        let mut keys = vec![
            ("XDG_CONFIG_HOME", std::env::var("XDG_CONFIG_HOME").ok()),
            ("RAGENT_CONFIG", std::env::var("RAGENT_CONFIG").ok()),
            (
                "RAGENT_CONFIG_CONTENT",
                std::env::var("RAGENT_CONFIG_CONTENT").ok(),
            ),
        ];
        keys.extend(
            provider_env_vars()
                .into_iter()
                .map(|k| (k, std::env::var(k).ok())),
        );
        Self {
            keys,
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
/// contents, isolated `XDG_CONFIG_HOME`, cleared `RAGENT_CONFIG*`, and no
/// provider credentials inherited from the host environment.
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
    for key in provider_env_vars() {
        unsafe { std::env::remove_var(key) };
    }
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

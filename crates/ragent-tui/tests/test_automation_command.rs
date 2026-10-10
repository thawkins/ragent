//! `/automation` TUI surface tests (spec `openhands` T-016; FR-013, FR-018).
//!
//! Covers the three TUI obligations: the command lists the configured
//! automations with their trigger and backend, it lists an automation's durable
//! run history, and it reports a disabled service. Persistence tests redirect
//! cwd + `XDG_CONFIG_HOME` into a tempdir so a run never touches the developer's
//! real config.
#![allow(unsafe_code)]
// The test serialiser (`test_lock`) is intentionally held across the async
// `#[tokio::test]` bodies so cwd/env mutations cannot interleave.
#![allow(clippy::await_holding_lock)]
#![cfg(test)]

use std::sync::{Mutex, OnceLock};

use ragent_tui::App;

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

/// The text of the most recent message in the message window.
fn last_message(app: &App) -> String {
    app.messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default()
}

const AUTOMATION_PROJECT: &str = r#"
{
  "defaultAgent": "general",
  "automation": {
    "enabled": true,
    "automations": [
      {
        "id": "on-issue",
        "agent": "general",
        "prompt": "Triage issue {{payload}}",
        "trigger": { "kind": "webhook" },
        "backend": "local"
      },
      {
        "id": "nightly",
        "trigger": { "kind": "schedule", "schedule": "every 2m" }
      }
    ]
  }
}
"#;

#[tokio::test]
async fn list_shows_configured_automations_with_trigger_and_backend() {
    let (_guard, _env, _temp) = enter_project(AUTOMATION_PROJECT);
    let mut app = support::make_app();
    app.session_id = Some("automation-test".to_string());

    app.execute_slash_command("/automation").await;

    let text = last_message(&app);
    assert!(text.contains("From: /automation"), "header: {text}");
    assert!(
        text.contains("on-issue"),
        "webhook automation listed: {text}"
    );
    assert!(
        text.contains("nightly"),
        "schedule automation listed: {text}"
    );
    assert!(text.contains("webhook"), "trigger shown: {text}");
    assert!(text.contains("schedule"), "trigger shown: {text}");
    assert!(text.contains("local"), "backend shown: {text}");
    assert_eq!(app.status, "automation: 2");
}

#[tokio::test]
async fn runs_reports_empty_history() {
    let (_guard, _env, _temp) = enter_project(AUTOMATION_PROJECT);
    let mut app = support::make_app();
    app.session_id = Some("automation-test".to_string());

    app.execute_slash_command("/automation runs on-issue").await;

    let text = last_message(&app);
    assert!(
        text.contains("No run history"),
        "empty history reported: {text}"
    );
    assert_eq!(app.status, "automation: 0 runs");
}

#[tokio::test]
async fn runs_lists_persisted_history() {
    let (_guard, _env, _temp) = enter_project(AUTOMATION_PROJECT);
    let mut app = support::make_app();
    app.session_id = Some("automation-test".to_string());

    // Seed a durable run record directly, then read it back through the command.
    let run = ragent_types::AutomationRun::start(
        "seed-run".to_string(),
        "on-issue".to_string(),
        ragent_types::AutomationTrigger::Webhook,
        "local".to_string(),
    );
    app.storage.insert_automation_run(&run).expect("seed run");
    app.storage
        .finish_automation_run("seed-run", ragent_types::RunOutcome::Error, None)
        .expect("finish run");

    app.execute_slash_command("/automation runs on-issue").await;

    let text = last_message(&app);
    assert!(text.contains("seed-run"), "run id listed: {text}");
    assert!(text.contains("error"), "outcome listed: {text}");
    assert!(text.contains("local"), "backend listed: {text}");
    assert_eq!(app.status, "automation: 1 run(s)");
}

#[tokio::test]
async fn run_reports_a_disabled_service() {
    let (_guard, _env, _temp) = enter_project(
        r#"{ "defaultAgent": "general", "automation": { "enabled": false, "automations": [] } }"#,
    );
    let mut app = support::make_app();
    app.session_id = Some("automation-test".to_string());

    app.execute_slash_command("/automation run on-issue").await;

    let text = last_message(&app);
    assert!(text.contains("disabled"), "disabled reported: {text}");
    assert_eq!(app.status, "automation: disabled");
}

#[tokio::test]
async fn unknown_automation_run_is_refused() {
    let (_guard, _env, _temp) = enter_project(AUTOMATION_PROJECT);
    let mut app = support::make_app();
    app.session_id = Some("automation-test".to_string());

    app.execute_slash_command("/automation run nosuch").await;

    let text = last_message(&app);
    assert!(text.contains("No enabled automation"), "refusal: {text}");
    assert_eq!(app.status, "automation: unknown");
}

#[tokio::test]
async fn help_lists_the_subcommands() {
    let (_guard, _env, _temp) = enter_project(AUTOMATION_PROJECT);
    let mut app = support::make_app();
    app.session_id = Some("automation-test".to_string());

    app.execute_slash_command("/automation help").await;

    let text = last_message(&app);
    for needle in ["From: /automation help", "runs", "run", "help"] {
        assert!(text.contains(needle), "help must mention {needle}: {text}");
    }
    assert_eq!(app.status, "automation: help");
}

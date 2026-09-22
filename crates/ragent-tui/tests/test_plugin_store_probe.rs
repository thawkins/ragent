//! TUI tests for the opt-in `/plugins stores --check` availability probe (spec
//! `pluginstores` FR-031 `--check`).
//!
//! `--check` contacts each store endpoint off the event loop and appends a
//! report naming whether each store is available and how many plugins it
//! advertises. These tests drive that path through the injectable store-fetch
//! seam with a fixture index, so they are deterministic and offline: no live
//! store is contacted (NFR-003). The plain `/plugins stores` output is asserted
//! to stay a pure config read that touches no probe slot.
//!
//! Env mutation (`set_var`/`remove_var`) is `unsafe` in edition 2024 and the
//! workspace denies `unsafe_code`; this test target opts back in explicitly (the
//! mutations are contained to this test binary and serialised by a lock).
//!
//! The env/cwd guard is intentionally held across the bounded poll `.await`
//! (single-threaded tests serialised by a lock), so
//! `clippy::await_holding_lock` is allowed here.

#![allow(unsafe_code)]
#![allow(clippy::await_holding_lock)]
#![cfg(test)]

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use ragent_plugins::{FixtureStoreFetcher, StoreKind};
use ragent_tui::App;

#[path = "support/mod.rs"]
mod support;

/// Serialise the cwd/env-mutating tests in this binary.
fn probe_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Restores the config-related env vars and the working directory on drop.
struct LaunchEnv {
    keys: Vec<(&'static str, Option<String>)>,
    cwd: std::path::PathBuf,
}

impl LaunchEnv {
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

impl Drop for LaunchEnv {
    fn drop(&mut self) {
        for (key, value) in &self.keys {
            // SAFETY: single-threaded test section guarded by `probe_lock`;
            // edition 2024 requires `unsafe` for `set_var`/`remove_var`.
            match value {
                Some(val) => unsafe { std::env::set_var(key, val) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
        std::env::set_current_dir(&self.cwd).ok();
    }
}

/// Enter an isolated empty project (no `.ragent/ragent.json`), with the global
/// config dir redirected into the tempdir so the developer's real config is
/// never read.
fn enter_empty_project() -> (MutexGuard<'static, ()>, LaunchEnv, tempfile::TempDir) {
    let guard = probe_lock().lock().unwrap_or_else(|e| e.into_inner());
    let env = LaunchEnv::new();
    let temp = tempfile::tempdir().expect("tempdir");
    // SAFETY: see `LaunchEnv::drop`; the lock makes this single-threaded.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", temp.path().join(".config")) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };
    std::env::set_current_dir(temp.path()).expect("set cwd");
    std::fs::create_dir_all(temp.path().join(".ragent")).expect("create .ragent");
    (guard, env, temp)
}

/// An app with a session and the given offline store-fetch seam installed.
fn app_with_seam(seam: FixtureStoreFetcher) -> App {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app.set_plugin_store_fetcher(Arc::new(seam));
    app
}

/// The rendered text of the most recent assistant message.
fn last_text(app: &App) -> String {
    app.messages
        .last()
        .expect("an assistant message must have been appended")
        .text_content()
}

/// The rendered text with every whitespace run folded to a single space, so
/// assertions are immune to the markdown re-wrap width applied to the message
/// window (a long availability marker is split across two lines).
fn flat_text(app: &App) -> String {
    last_text(app)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Wait (bounded) for the off-loop probe to deposit its report, then apply it.
async fn drive_probe(app: &mut App) {
    let mut delivered = false;
    for _ in 0..300 {
        if app
            .plugin_store_probe_result
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
        {
            delivered = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(delivered, "the off-loop probe delivered a report");
    app.poll_plugin_store_probe_result();
}

// ── `--check` probes each store off-loop (FR-031 `--check`) ─────────────────

#[tokio::test]
async fn stores_check_probes_both_defaults_and_reports_availability() {
    let (_lock, _env, _temp) = enter_empty_project();
    let mut app = app_with_seam(FixtureStoreFetcher::new().with_default_endpoints());

    app.execute_slash_command("/plugins stores --check");

    // The acknowledgement renders immediately; the probe completes off-loop.
    assert!(
        last_text(&app).contains("Checking store availability"),
        "the immediate acknowledgement is shown: {}",
        last_text(&app)
    );
    assert!(
        app.plugin_store.is_none(),
        "`--check` opens no browse panel"
    );

    drive_probe(&mut app).await;

    let text = flat_text(&app);
    assert!(text.contains("From: /plugins stores"), "{text}");
    for kind in StoreKind::ALL {
        assert!(
            text.contains(&format!("{}: [default]", kind.token())),
            "{} provenance is preserved: {text}",
            kind.token()
        );
    }
    assert!(
        text.contains("[ok] available,"),
        "both stores report availability: {text}"
    );
    assert!(
        text.contains("plugins"),
        "the plugin count is labelled: {text}"
    );
    assert!(text.is_ascii(), "the report must be ASCII only: {text}");
}

#[tokio::test]
async fn stores_check_reports_unavailable_with_no_registered_fixture() {
    let (_lock, _env, _temp) = enter_empty_project();
    // An empty seam: no endpoint is registered, so the probe is unavailable for
    // both stores without any live request (NFR-003).
    let mut app = app_with_seam(FixtureStoreFetcher::new());

    app.execute_slash_command("/plugins stores --check");
    drive_probe(&mut app).await;

    let text = flat_text(&app);
    assert!(
        text.contains("[err] unavailable:"),
        "an unreachable store is flagged: {text}"
    );
    assert!(
        text.contains("no offline fixture"),
        "the contained reason is surfaced: {text}"
    );
}

#[tokio::test]
async fn stores_check_uses_the_injected_seam_for_every_store() {
    // Both compiled defaults are served by the fixture seam, proving the probe
    // routes through the injectable fetcher rather than the live HTTPS one.
    let (_lock, _env, _temp) = enter_empty_project();
    let seam = FixtureStoreFetcher::new().with_default_endpoints();
    let expected = seam.endpoints().len();
    let mut app = app_with_seam(seam);

    app.execute_slash_command("/plugins stores --check");
    drive_probe(&mut app).await;

    assert_eq!(expected, 2, "the seam is seeded for both compiled defaults");
    assert!(
        flat_text(&app).contains("[ok] available,"),
        "the injected seam answered both probes: {}",
        flat_text(&app)
    );
}

// ── Plain `/plugins stores` is unchanged ────────────────────────────────────

#[tokio::test]
async fn plain_stores_touches_no_probe_slot_and_carries_no_availability() {
    let (_lock, _env, _temp) = enter_empty_project();
    let mut app = app_with_seam(FixtureStoreFetcher::new().with_default_endpoints());

    app.execute_slash_command("/plugins stores");

    let text = last_text(&app);
    assert!(text.contains("From: /plugins stores"), "{text}");
    for kind in StoreKind::ALL {
        assert!(
            text.contains(&format!("{}: [default]", kind.token())),
            "{} default provenance is reported: {text}",
            kind.token()
        );
    }
    assert!(
        !text.contains("available") && !text.contains("unavailable"),
        "the plain report carries no probe suffix: {text}"
    );
    assert!(
        app.plugin_store_probe_result.lock().unwrap().is_none(),
        "the plain report starts no probe"
    );
}

// ── No reactor: the probe degrades to the plain config report ───────────────

#[test]
fn stores_check_without_a_reactor_deposits_the_plain_config_report() {
    // A plain `#[test]` has no async reactor, so `begin_plugin_store_probe`
    // cannot run off-loop. It must degrade to the plain config report rather
    // than block or panic, and the poll must still surface it.
    let (_lock, _env, _temp) = enter_empty_project();
    let mut app = app_with_seam(FixtureStoreFetcher::new().with_default_endpoints());

    app.execute_slash_command("/plugins stores --check");
    assert!(
        app.plugin_store_probe_result.lock().unwrap().is_some(),
        "without a reactor the plain report is deposited synchronously"
    );

    app.poll_plugin_store_probe_result();
    let text = flat_text(&app);
    assert!(text.contains("From: /plugins stores"), "{text}");
    for kind in StoreKind::ALL {
        assert!(
            text.contains(&format!("{}: [default]", kind.token())),
            "{} default provenance is still reported: {text}",
            kind.token()
        );
    }
    assert!(
        !text.contains("available") && !text.contains("unavailable"),
        "the degraded report is the plain config view: {text}"
    );
}

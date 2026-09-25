//! Tests for the no-`plugins.stores` launch path and the `/plugins stores`
//! report's default tagging (spec `pluginstores` T-021; FR-036, FR-037, FR-038,
//! NFR-003).
//!
//! Two behaviours are pinned here, both with no `plugins.stores` block present:
//!
//! 1. The `/plugins codex` and `/plugins claude` launch path emits no
//!    configuration error and opens the browse panel for the requested store
//!    (FR-036).
//! 2. The `/plugins stores` report tags each store's effective endpoint as
//!    sourced from the compiled default (FR-038).
//!
//! Every test is deterministic and offline: the launch resolves the endpoint and
//! opens the panel, but no live store is contacted (the app's production HTTPS
//! fetcher is replaced with an offline [`FixtureStoreFetcher`] seeded for both
//! compiled defaults, so even if a fetch were attempted it could not reach the
//! network -- FR-037, NFR-003).
//!
//! Env mutation (`set_var`/`remove_var`) is `unsafe` in edition 2024 and the
//! workspace denies `unsafe_code`; this test target opts back in explicitly (the
//! mutations are contained to this test binary and serialised by a lock).

#![allow(unsafe_code)]
#![cfg(test)]

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use ragent_plugins::{
    DEFAULT_CLAUDE_STORE_URL, DEFAULT_CODEX_STORE_URL, EndpointSource, FixtureStoreFetcher,
    StoreKind,
};
use ragent_tui::App;
use ragent_tui::app::PluginStoreStatus;

#[path = "support/mod.rs"]
mod support;

/// Serialise the cwd/env-mutating tests in this binary.
fn launch_lock() -> &'static Mutex<()> {
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
            // SAFETY: single-threaded test section guarded by `launch_lock`;
            // edition 2024 requires `unsafe` for `set_var`/`remove_var`.
            match value {
                Some(val) => unsafe { std::env::set_var(key, val) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
        std::env::set_current_dir(&self.cwd).ok();
    }
}

/// Enter an isolated empty project (no `.ragent/ragent.json` at all), with the
/// global config dir redirected into the tempdir so the developer's real config
/// is never read.
#[allow(clippy::await_holding_lock)]
fn enter_empty_project() -> (MutexGuard<'static, ()>, LaunchEnv, tempfile::TempDir) {
    let guard = launch_lock().lock().unwrap_or_else(|e| e.into_inner());
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

/// An app with a session and the offline fixture seam installed for both
/// compiled defaults, so no launch can reach the live network (FR-037, NFR-003).
fn app_with_fixture_seam() -> App {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app.set_plugin_store_fetcher(Arc::new(
        FixtureStoreFetcher::new().with_default_endpoints(),
    ));
    app
}

/// The number of assistant messages on `app`.
fn assistant_count(app: &App) -> usize {
    app.messages.len()
}

/// The rendered text of the most recent assistant message.
fn last_text(app: &App) -> String {
    app.messages
        .last()
        .expect("an assistant message must have been appended")
        .text_content()
}

/// The rendered text of the most recent assistant message with every whitespace
/// run folded to a single space, so a long endpoint URL that the message window
/// markdown-wrapped onto its own line can still be asserted as one cell.
fn flat_text(app: &App) -> String {
    last_text(app)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// -- No-config launch emits no error and opens the panel (FR-036) --------------

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn codex_launch_with_no_config_opens_the_panel_and_prints_nothing() {
    let (_lock, _env, _temp) = enter_empty_project();
    let mut app = app_with_fixture_seam();
    let before = assistant_count(&app);

    app.execute_slash_command("/plugins codex").await;

    let browser = app
        .plugin_store
        .as_ref()
        .expect("`/plugins codex` must open the browse panel with no config");
    assert_eq!(browser.kind, StoreKind::Codex);
    assert_eq!(
        browser.status,
        PluginStoreStatus::Loading,
        "the launch must start the fetch and show the loading row"
    );
    assert_eq!(
        assistant_count(&app),
        before,
        "the launch must print no report, so no configuration error can appear"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn claude_launch_with_no_config_opens_the_panel_and_prints_nothing() {
    let (_lock, _env, _temp) = enter_empty_project();
    let mut app = app_with_fixture_seam();
    let before = assistant_count(&app);

    app.execute_slash_command("/plugins claude").await;

    let browser = app
        .plugin_store
        .as_ref()
        .expect("`/plugins claude` must open the browse panel with no config");
    assert_eq!(browser.kind, StoreKind::Claude);
    assert_eq!(browser.status, PluginStoreStatus::Loading);
    assert_eq!(assistant_count(&app), before);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn both_stores_launch_with_no_config_and_emit_no_error() {
    // FR-036: with no `plugins.stores` block, the launch path emits no
    // configuration error and opens the browse panel for BOTH stores.
    let (_lock, _env, _temp) = enter_empty_project();
    let mut app = app_with_fixture_seam();

    for (invocation, kind) in [
        ("/plugins codex", StoreKind::Codex),
        ("/plugins claude", StoreKind::Claude),
    ] {
        app.close_plugin_store();
        let before = assistant_count(&app);
        app.execute_slash_command(invocation).await;
        assert_eq!(
            assistant_count(&app),
            before,
            "`{invocation}` must emit no configuration error"
        );
        let browser = app
            .plugin_store
            .as_ref()
            .unwrap_or_else(|| panic!("`{invocation}` must leave the panel open"));
        assert_eq!(browser.kind, kind, "`{invocation}` opens its own store");
    }
}

// -- `/plugins stores` tags default-sourced endpoints (FR-038) -----------------

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn stores_report_tags_both_endpoints_as_default_with_no_config() {
    // FR-038: with no `plugins.stores` block, the report tags each store's
    // effective endpoint as sourced from the compiled default.
    let (_lock, _env, _temp) = enter_empty_project();
    let mut app = app_with_fixture_seam();

    app.execute_slash_command("/plugins stores").await;
    let raw = last_text(&app);
    let text = flat_text(&app);

    assert!(raw.contains("From: /plugins stores"), "{raw}");
    // The message window re-renders markdown bullets as `*` and wraps a long
    // endpoint onto its own line, so assert on the whitespace-folded
    // `token: [tag] endpoint` cells rather than the bullet glyph.
    for kind in StoreKind::ALL {
        assert!(
            text.contains(&format!(
                "{}: [default] {}",
                kind.token(),
                kind.default_url()
            )),
            "{} must be tagged default with its compiled URL: {text}",
            kind.token()
        );
        assert!(
            !text.contains(&format!("{}: [config]", kind.token())),
            "{} must not be tagged config without an override: {text}",
            kind.token()
        );
    }
    // Both compiled defaults are named (FR-038, NFR-004).
    assert!(text.contains(DEFAULT_CODEX_STORE_URL), "{text}");
    assert!(text.contains(DEFAULT_CLAUDE_STORE_URL), "{text}");
    assert!(raw.is_ascii(), "the report must be ASCII only: {raw}");
    // Reporting the stores opens no panel and touches no store.
    assert!(
        app.plugin_store.is_none(),
        "`/plugins stores` must not open the browse panel"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn stores_report_matches_the_direct_provenance_resolution() {
    // The TUI report and the pure provenance resolver agree: with no config both
    // stores resolve to `EndpointSource::Default` and their compiled URL.
    let (_lock, _env, _temp) = enter_empty_project();
    let mut app = app_with_fixture_seam();

    app.execute_slash_command("/plugins stores").await;
    let text = flat_text(&app);

    let stores = ragent_config::PluginsConfig::default().stores_or_default();
    for kind in StoreKind::ALL {
        let (endpoint, source) = kind
            .effective_endpoint_with_source(&stores)
            .expect("the compiled default is accepted");
        assert_eq!(source, EndpointSource::Default, "{} source", kind.token());
        assert!(
            text.contains(&format!(
                "{}: [{}] {}",
                kind.token(),
                source.tag(),
                endpoint.as_str()
            )),
            "the report must mirror the resolver for {}: {text}",
            kind.token()
        );
    }
}

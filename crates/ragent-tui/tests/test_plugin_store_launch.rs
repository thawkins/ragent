//! Tests for the plugin-store launch path: `/plugins codex` and
//! `/plugins claude` open the browse panel and resolve the compiled default
//! endpoint when the resolved configuration carries no `plugins.stores` block
//! (spec `pluginstores` T-016; FR-027, FR-028, FR-029, FR-030, FR-036).
//!
//! These tests are deterministic and offline: the launch resolves the endpoint
//! and opens the panel, but no live store is contacted (the off-loop fetch has
//! no async reactor in a plain `#[test]`, and the fetch seam that serves the
//! default endpoints is a later task).
//!
//! Env mutation (`set_var`/`remove_var`) is `unsafe` in edition 2024 and the
//! workspace denies `unsafe_code`; this test target opts back in explicitly
//! (the mutations are contained to this test binary and serialised by a lock).
#![allow(unsafe_code)]
#![cfg(test)]

use std::sync::{Mutex, MutexGuard, OnceLock};

use ragent_config::{PluginStoreEndpoint, PluginStoresConfig, PluginsConfig};
use ragent_plugins::{DEFAULT_CLAUDE_STORE_URL, DEFAULT_CODEX_STORE_URL, StoreEndpoint, StoreKind};
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

/// Enter an isolated project whose `.ragent/ragent.json` holds `config_json`,
/// with the global config dir redirected into the tempdir so the developer's
/// real config is never read.
fn enter_project(config_json: &str) -> (MutexGuard<'static, ()>, LaunchEnv, tempfile::TempDir) {
    let guard = launch_lock().lock().unwrap_or_else(|e| e.into_inner());
    let env = LaunchEnv::new();
    let temp = tempfile::tempdir().expect("tempdir");
    // SAFETY: see `LaunchEnv::drop`; the lock makes this single-threaded.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", temp.path().join(".config")) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };
    std::env::set_current_dir(temp.path()).expect("set cwd");
    let ragent_dir = temp.path().join(".ragent");
    std::fs::create_dir_all(&ragent_dir).expect("create .ragent");
    std::fs::write(ragent_dir.join("ragent.json"), config_json).expect("write project config");
    (guard, env, temp)
}

/// An app with a session, so the `/plugins` launch reaches the dispatch arm.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app
}

/// The number of assistant messages on `app`.
fn assistant_count(app: &App) -> usize {
    app.messages.len()
}

/// The project config used by every no-`stores`-block case: the `plugins` key
/// is absent entirely (FR-030 "no `plugins.stores` block is present").
const NO_PLUGINS_BLOCK: &str = r#"{"defaultAgent": "general"}"#;

// ── Launch with no `plugins.stores` block (FR-030, FR-036) ──────────────────

#[test]
fn codex_launch_with_no_stores_block_opens_the_panel() {
    let (_lock, _env, _temp) = enter_project(NO_PLUGINS_BLOCK);
    let mut app = app_with_session();
    let before = assistant_count(&app);

    app.execute_slash_command("/plugins codex");

    let browser = app
        .plugin_store
        .as_ref()
        .expect("`/plugins codex` must open the browse panel with no stores block");
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

#[test]
fn claude_launch_with_no_stores_block_opens_the_panel() {
    let (_lock, _env, _temp) = enter_project(NO_PLUGINS_BLOCK);
    let mut app = app_with_session();
    let before = assistant_count(&app);

    app.execute_slash_command("/plugins claude");

    let browser = app
        .plugin_store
        .as_ref()
        .expect("`/plugins claude` must open the browse panel with no stores block");
    assert_eq!(browser.kind, StoreKind::Claude);
    assert_eq!(browser.status, PluginStoreStatus::Loading);
    assert_eq!(assistant_count(&app), before);
}

#[test]
fn launch_reports_no_configuration_error_for_either_store() {
    let (_lock, _env, _temp) = enter_project(NO_PLUGINS_BLOCK);
    let mut app = app_with_session();

    for invocation in ["/plugins codex", "/plugins claude"] {
        app.close_plugin_store();
        let before = assistant_count(&app);
        app.execute_slash_command(invocation);
        assert_eq!(
            assistant_count(&app),
            before,
            "`{invocation}` must emit no configuration error"
        );
        assert!(
            app.plugin_store.is_some(),
            "`{invocation}` must leave the panel open"
        );
    }
}

// ── Effective endpoint resolution (FR-027, FR-028, FR-029) ──────────────────

#[test]
fn absent_stores_block_resolves_each_store_to_its_compiled_default() {
    // A config with no `plugins` key resolves to `PluginsConfig::default()`,
    // whose `stores_or_default()` is what the launch path hands the resolver.
    let stores = PluginsConfig::default().stores_or_default();

    let codex = StoreKind::Codex
        .effective_endpoint(&stores)
        .expect("the compiled Codex default must satisfy the https guard");
    let claude = StoreKind::Claude
        .effective_endpoint(&stores)
        .expect("the compiled Claude default must satisfy the https guard");

    assert_eq!(codex.as_str(), DEFAULT_CODEX_STORE_URL);
    assert_eq!(claude.as_str(), DEFAULT_CLAUDE_STORE_URL);
}

#[test]
fn the_default_endpoints_are_https_and_pass_the_endpoint_guard() {
    for (kind, raw) in [
        (StoreKind::Codex, DEFAULT_CODEX_STORE_URL),
        (StoreKind::Claude, DEFAULT_CLAUDE_STORE_URL),
    ] {
        assert!(
            raw.starts_with("https://"),
            "{} default endpoint must be https: {raw}",
            kind.label()
        );
        let parsed = StoreEndpoint::parse(raw)
            .unwrap_or_else(|e| panic!("{} default must parse: {e}", kind.label()));
        assert_eq!(parsed.as_str(), raw);
        // The same guard the configured path uses (FR-024, FR-029).
        assert_eq!(
            kind.effective_endpoint(&PluginStoresConfig::default())
                .expect("default passes the guard")
                .as_str(),
            raw
        );
    }
}

#[test]
fn a_configured_url_overrides_the_default_on_the_launch_path() {
    // Isolate env/cwd and serialise against the other env-mutating cases: the
    // launch below reads the resolved config.
    let (_lock, _env, _temp) = enter_project(NO_PLUGINS_BLOCK);

    // The same `PluginsConfig` the launch path resolves: a non-empty Codex URL
    // wins wholesale (FR-028) while the un-overridden Claude store keeps its
    // compiled default (FR-027).
    let stores = PluginsConfig {
        stores: Some(PluginStoresConfig {
            codex: Some(PluginStoreEndpoint {
                url: Some("https://example.org/codex/index.json".to_string()),
            }),
            ..PluginStoresConfig::default()
        }),
        ..PluginsConfig::default()
    }
    .stores_or_default();

    let codex = StoreKind::Codex
        .effective_endpoint(&stores)
        .expect("configured https endpoint is accepted");
    assert_eq!(codex.as_str(), "https://example.org/codex/index.json");
    // The un-overridden store keeps its compiled default.
    assert_eq!(
        StoreKind::Claude
            .effective_endpoint(&stores)
            .expect("default is accepted")
            .as_str(),
        DEFAULT_CLAUDE_STORE_URL
    );

    // The launch itself still opens the panel, now against the override.
    let mut app = app_with_session();
    app.execute_slash_command("/plugins codex");
    assert_eq!(
        app.plugin_store.as_ref().expect("panel open").kind,
        StoreKind::Codex
    );
}

// ── Query and `--refresh` parsing on launch (FR-020, FR-021) ────────────────

#[test]
fn trailing_query_is_prefilled_and_refresh_is_recorded() {
    let (_lock, _env, _temp) = enter_project(NO_PLUGINS_BLOCK);
    let mut app = app_with_session();

    app.execute_slash_command("/plugins codex weather --refresh");

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.query, "weather");
    assert!(browser.refresh, "`--refresh` must be recorded");
}

#[test]
fn a_multi_word_query_is_joined_and_refresh_defaults_off() {
    let (_lock, _env, _temp) = enter_project(NO_PLUGINS_BLOCK);
    let mut app = app_with_session();

    app.execute_slash_command("/plugins claude time helper");

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.query, "time helper");
    assert!(!browser.refresh);
}

// ── Disabled subsystem (SPEC configuration schema) ──────────────────────────

#[test]
fn a_disabled_subsystem_reports_and_opens_no_panel() {
    let (_lock, _env, _temp) =
        enter_project(r#"{"defaultAgent": "general", "plugins": {"enabled": false}}"#);
    let mut app = app_with_session();

    app.execute_slash_command("/plugins codex");

    assert!(
        app.plugin_store.is_none(),
        "a disabled subsystem must open no panel"
    );
    let text = app
        .messages
        .last()
        .expect("a report must be appended")
        .text_content();
    assert!(
        text.contains("disabled"),
        "the report must say the subsystem is disabled: {text}"
    );
}

// ── Non-store subcommands still report (regression) ─────────────────────────

#[test]
fn help_and_unknown_subcommands_still_render_the_usage_block() {
    let (_lock, _env, _temp) = enter_project(NO_PLUGINS_BLOCK);
    let mut app = app_with_session();

    for invocation in ["/plugins help", "/plugins", "/plugins bogus"] {
        app.execute_slash_command(invocation);
        let text = app
            .messages
            .last()
            .expect("usage block appended")
            .text_content();
        assert!(
            text.contains("command reference"),
            "`{invocation}` must render the usage block: {text}"
        );
        assert!(
            app.plugin_store.is_none(),
            "`{invocation}` must not open a panel"
        );
    }
}

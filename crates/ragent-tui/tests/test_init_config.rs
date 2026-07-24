//! Tests for the `/init config` slash command.
//!
//! These tests manipulate `XDG_CONFIG_HOME` (via `std::env::set_var`, which is
//! `unsafe` in Rust 2024) to sandbox the global config directory. The workspace
//! forbids `unsafe_code`, so we override it for this test target only.

#![allow(unsafe_code)]

use std::sync::{Mutex, OnceLock};

#[path = "support/mod.rs"]
mod support;

/// Serialise env-var mutation across parallel tests.
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Point `XDG_CONFIG_HOME` at a temp directory so `dirs::config_dir()`
/// returns a sandboxed path. Returns the previous value (if any).
fn with_temp_xdg_config_home(temp: &tempfile::TempDir) -> Option<String> {
    let original = std::env::var("XDG_CONFIG_HOME").ok();
    let fake = temp.path().join("fake_config");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &fake);
    }
    original
}

fn restore_xdg_config_home(original: Option<String>) {
    unsafe {
        if let Some(v) = original {
            std::env::set_var("XDG_CONFIG_HOME", v);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }
}

/// `/init config` should create a default `ragent.json` inside the global
/// config directory.
#[test]
fn test_slash_init_config_creates_default_global_config() {
    let _lock = env_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    let temp = tempfile::tempdir().expect("tempdir");
    let original_xdg = with_temp_xdg_config_home(&temp);

    // The global config path will be <temp>/fake_config/ragent/ragent.json
    let config_dir = temp.path().join("fake_config").join("ragent");
    let config_path = config_dir.join("ragent.json");
    assert!(!config_path.exists(), "config should not exist pre-test");

    app.execute_slash_command("/init config");

    assert_eq!(app.status, "init config: created");
    assert!(
        config_path.exists(),
        "default global config should have been created at {}",
        config_path.display()
    );

    // The file must be valid JSON that round-trips into a Config.
    let content = std::fs::read_to_string(&config_path).expect("read created config");
    let _: ragent_config::Config =
        serde_json::from_str(&content).expect("created config should be valid JSON");

    // The assistant message should mention success and the path.
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Default config created"),
        "should report creation: {text}"
    );
    assert!(
        text.contains(config_path.to_str().unwrap()),
        "should include the config path: {text}"
    );

    restore_xdg_config_home(original_xdg);
}

/// `/init config` should not overwrite an existing global config.
#[test]
fn test_slash_init_config_skips_when_config_exists() {
    let _lock = env_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    let temp = tempfile::tempdir().expect("tempdir");
    let original_xdg = with_temp_xdg_config_home(&temp);

    let config_dir = temp.path().join("fake_config").join("ragent");
    std::fs::create_dir_all(&config_dir).expect("create config dir");
    let config_path = config_dir.join("ragent.json");
    let existing = r#"{"default_agent":"coder"}"#;
    std::fs::write(&config_path, existing).expect("write existing config");

    app.execute_slash_command("/init config");

    assert_eq!(app.status, "init config: already exists");

    // The file should be untouched.
    let content = std::fs::read_to_string(&config_path).expect("read config");
    assert_eq!(content, existing, "existing config should not be modified");

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("already exists"),
        "should warn about existing config: {text}"
    );

    restore_xdg_config_home(original_xdg);
}

/// `/init config` should produce a user-facing message even when no config
/// directory can be determined.  This is a defensive check — on real platforms
/// `dirs::config_dir()` always returns `Some`, but the branch should still
/// produce a clean status rather than panicking.
#[test]
fn test_slash_init_config_produces_status_message() {
    let _lock = env_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    let temp = tempfile::tempdir().expect("tempdir");
    let original_xdg = with_temp_xdg_config_home(&temp);

    app.execute_slash_command("/init config");

    // Status should be one of the known init-config statuses.
    assert!(
        app.status.starts_with("init config:"),
        "status should start with 'init config:': {}",
        app.status
    );

    // A message should have been produced.
    assert!(
        !app.messages.is_empty(),
        "init config should produce a user-facing message"
    );

    restore_xdg_config_home(original_xdg);
}

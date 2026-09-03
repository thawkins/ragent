//! Tests for activity-log enable/disable persistence.
//!
//! Note: `std::env::set_var` / `remove_var` are `unsafe` in Rust 2024; the
//! workspace denies `unsafe_code`, so this test target opts back in
//! explicitly (env mutation is contained to the test binary).

#![allow(unsafe_code)]

use std::io::Write;

use ragent_config::Config;

fn write_config(dir: &std::path::Path, contents: &str) -> std::path::PathBuf {
    let ragent_dir = dir.join(".ragent");
    std::fs::create_dir_all(&ragent_dir).expect("create .ragent dir");
    let path = ragent_dir.join("ragent.json");
    let mut file = std::fs::File::create(&path).expect("create config file");
    file.write_all(contents.as_bytes())
        .expect("write config file");
    path
}

/// Restores the environment and working directory mutated by a test so the
/// real user config is never clobbered.
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

#[test]
fn test_activity_log_defaults_to_true() {
    // When deserialised from JSON with the field absent, `#[serde(default =
    // "default_true")]` yields `true`. The derived `Default` impl (used by
    // `Config::default()`) zeroes bools to `false`, so we test the serde
    // deserialisation path instead, which is the one used at startup.
    let json = "{}";
    let config: Config = serde_json::from_str(json).expect("parse config");
    assert!(
        config.activity_log,
        "activity_log should default to true when absent from JSON"
    );
}

#[test]
#[serial_test::serial]
fn test_activity_log_runtime_defaults_to_true() {
    // The flag is a process-wide static shared with the other (serial) tests,
    // which may have left it disabled via `sync_from_config`. Restore the
    // documented default before asserting so this test is order-independent.
    ragent_config::activity_log::set_enabled(true);
    assert!(
        ragent_config::activity_log::is_enabled(),
        "runtime activity-log flag should default to true"
    );
}

#[test]
#[serial_test::serial]
fn test_activity_log_round_trips_through_config() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = write_config(temp.path(), r#"{ "activity_log": false }"#);

    unsafe { std::env::set_var("RAGENT_CONFIG", &path) };
    let config = Config::load().expect("load config");

    assert!(!config.activity_log);
    ragent_config::activity_log::sync_from_config();
    assert!(!ragent_config::activity_log::is_enabled());

    // Restore default state for subsequent tests.
    ragent_config::activity_log::set_enabled(true);
}

#[test]
#[serial_test::serial]
fn test_activity_log_persist_helper_updates_config_file() {
    let _guard = EnvGuard::new();
    let temp = tempfile::tempdir().expect("temp dir");
    let project = temp.path().join("project");
    std::fs::create_dir_all(&project).expect("create project dir");

    // Point the global config dir at our temp dir's .config and clear the
    // env overrides so `Config::load` only sees the project file we write.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", temp.path().join(".config")) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };

    // Project config: activity logging enabled.
    let path = write_config(&project, r#"{ "activity_log": true }"#);
    std::env::set_current_dir(&project).expect("cwd into project");

    // `save_to_source` resolves the project `.ragent/ragent.json` (because it
    // was loaded from the project path), so the persisted value lands in the
    // temp project file rather than the user's real global config.
    ragent_config::activity_log::persist_activity_log(false).expect("persist activity_log");

    // Reloading should pick up the persisted value.
    let config = Config::load().expect("reload config");
    assert!(!config.activity_log);
    assert!(!ragent_config::activity_log::is_enabled());

    // The temp project file itself must have been updated.
    let on_disk = std::fs::read_to_string(&path).expect("read persisted config");
    assert!(
        on_disk.contains("\"activity_log\": false"),
        "project config should contain activity_log: false, got:\n{on_disk}"
    );

    // Restore default state for subsequent tests.
    ragent_config::activity_log::set_enabled(true);
}

#[test]
fn test_activity_log_false_is_serialized() {
    let mut config = Config::default();
    config.activity_log = false;
    let json = serde_json::to_string_pretty(&config).expect("serialize config");
    assert!(json.contains("\"activity_log\": false"));
}

#[test]
fn test_activity_log_true_is_serialized() {
    let mut config = Config::default();
    config.activity_log = true;
    let json = serde_json::to_string_pretty(&config).expect("serialize config");
    assert!(json.contains("\"activity_log\": true"));
}

#[test]
fn test_activity_log_merge_overlay_takes_precedence() {
    // Regression: `activity_log` was missing from `Config::merge`, so the
    // merged value always stayed at the derived-`Default` of `false` even
    // when an overlay explicitly set it to `true`. The status bar showed
    // "off" at startup regardless of the configured state.
    let mut base = Config::default();
    base.activity_log = false;

    let mut overlay = Config::default();
    overlay.activity_log = true;

    let merged = Config::merge(base, overlay);
    assert!(
        merged.activity_log,
        "overlay activity_log=true should win over base false"
    );

    // The reverse must also hold so `/alog off` is respected.
    let mut base = Config::default();
    base.activity_log = true;

    let mut overlay = Config::default();
    overlay.activity_log = false;

    let merged = Config::merge(base, overlay);
    assert!(
        !merged.activity_log,
        "overlay activity_log=false should win over base true"
    );
}

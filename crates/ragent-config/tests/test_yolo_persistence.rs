//! Tests for YOLO mode persistence.
//!
//! Note: `std::env::set_var` is `unsafe` in Rust 2024; the workspace denies
//! `unsafe_code`, so this test target opts back in explicitly.

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

/// Restores the env vars these (serially run) tests mutate so the real user
/// config and config-dir resolution are never left clobbered.
struct EnvGuard {
    keys: Vec<(&'static str, Option<String>)>,
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
    }
}

#[test]
fn test_yolo_defaults_to_false() {
    let config = Config::default();
    assert!(!config.yolo);
}

#[test]
#[serial_test::serial]
fn test_yolo_round_trips_through_config() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = write_config(temp.path(), r#"{ "yolo": true }"#);

    unsafe { std::env::set_var("RAGENT_CONFIG", &path) };
    let config = Config::load().expect("load config");

    assert!(config.yolo);
    ragent_config::yolo::sync_from_config();
    assert!(ragent_config::yolo::is_enabled());
}

#[test]
#[serial_test::serial]
fn test_yolo_persist_helper_updates_global_config_file() {
    let _guard = EnvGuard::new();
    let temp = tempfile::tempdir().expect("temp dir");

    // Redirect the user-global config dir into the tempdir and clear the env
    // overrides so `Config::load` resolves the global file we can inspect.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", temp.path().join(".config")) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };

    ragent_config::yolo::persist_yolo(true).expect("persist yolo");

    // YOLO is a user-only toggle: it must land in the global config, never a
    // project file (a project `yolo` is stripped by `merge_project`).
    let global_path = Config::global_config_path().expect("global config path");
    assert_eq!(
        global_path,
        temp.path()
            .join(".config")
            .join("ragent")
            .join("ragent.json"),
        "XDG_CONFIG_HOME redirect should retarget the global config path"
    );
    assert!(
        global_path.exists(),
        "persist_yolo should write the global config at {}",
        global_path.display()
    );
    let on_disk = std::fs::read_to_string(&global_path).expect("read global config");
    assert!(
        on_disk.contains("\"yolo\": true"),
        "global config should contain yolo: true, got:\n{on_disk}"
    );

    // Reloading should pick up the persisted value.
    let config = Config::load().expect("reload config");
    assert!(config.yolo);
    assert!(ragent_config::yolo::is_enabled());
}

#[test]
fn test_yolo_false_is_serialized() {
    let mut config = Config::default();
    config.yolo = false;
    let json = serde_json::to_string_pretty(&config).expect("serialize config");
    assert!(!json.contains("\"yolo\": true"));
}

#[test]
fn test_yolo_true_is_serialized() {
    let mut config = Config::default();
    config.yolo = true;
    let json = serde_json::to_string_pretty(&config).expect("serialize config");
    assert!(json.contains("\"yolo\": true"));
}

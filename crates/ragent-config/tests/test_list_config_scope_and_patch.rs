//! Tests for `ragent_config::list_config` - the shared config-file plumbing
//! used by the runtime list modules (`bash_lists`, `dir_lists`).
//!
//! Covers `Scope::config_path` resolution and the read-modify-write cycle in
//! `patch_config` (section/array creation, preservation of unrelated keys, the
//! absent-file path, and the parse-error path). Mutating tests run inside a
//! private temp working directory because the project scope is a relative path,
//! and they are serialised because the process cwd + env are shared.

#![allow(unsafe_code)]

use std::env;
use std::path::PathBuf;

use ragent_config::list_config::{self, Scope};
use serde_json::Value;

/// Points the process at a private temp working directory and restores the
/// original cwd (plus `XDG_CONFIG_HOME`) on drop.
struct Sandbox {
    original_cwd: PathBuf,
    original_xdg: Option<String>,
    temp: tempfile::TempDir,
    config_home: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let original_cwd = env::current_dir().expect("cwd");
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        let config_home = temp.path().join("config");
        // SAFETY: Rust 2024 makes `set_var` unsafe; this test target opts in
        // and runs serially.
        unsafe { env::set_var("XDG_CONFIG_HOME", &config_home) };
        env::set_current_dir(temp.path()).expect("chdir to sandbox");
        Self {
            original_cwd,
            original_xdg,
            temp,
            config_home,
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        env::set_current_dir(&self.original_cwd).ok();
        match self.original_xdg.take() {
            // SAFETY: see `Sandbox::new`.
            Some(v) => unsafe { env::set_var("XDG_CONFIG_HOME", v) },
            None => unsafe { env::remove_var("XDG_CONFIG_HOME") },
        }
        let _ = &self.temp;
    }
}

#[test]
#[serial_test::serial]
fn test_scope_project_config_path_is_relative() {
    let path = Scope::Project.config_path().expect("project path");
    assert_eq!(path, PathBuf::from(".ragent").join("ragent.json"));
}

#[test]
#[serial_test::serial]
fn test_scope_global_config_path_uses_xdg() {
    let sandbox = Sandbox::new();
    let path = Scope::Global.config_path().expect("global path");
    assert!(
        path.starts_with(&sandbox.config_home),
        "global path {path:?} must live under XDG_CONFIG_HOME {:?}",
        sandbox.config_home
    );
    assert!(path.ends_with("ragent/ragent.json"));
}

#[test]
#[serial_test::serial]
fn test_scope_derives() {
    assert_eq!(Scope::Project, Scope::Project);
    assert_ne!(Scope::Project, Scope::Global);
    assert_eq!(Scope::Global, Scope::Global.clone());
    assert!(format!("{:?}", Scope::Global).contains("Global"));
}

#[test]
#[serial_test::serial]
fn test_patch_config_creates_section_and_default_arrays() {
    let _sandbox = Sandbox::new();

    // No config file exists yet: patch_config must create it with the section
    // and the default arrays, then run the mutation.
    list_config::patch_config(Scope::Project, "bash", &["allowlist", "denylist"], |bash| {
        bash["allowlist"]
            .as_array_mut()
            .expect("allowlist array")
            .push(Value::String("curl".to_string()));
    })
    .expect("patch config");

    let raw = std::fs::read_to_string(".ragent/ragent.json").expect("read config");
    let root: Value = serde_json::from_str(&raw).expect("parse config");
    assert_eq!(root["bash"]["allowlist"], serde_json::json!(["curl"]));
    assert_eq!(root["bash"]["denylist"], serde_json::json!([]));
}

#[test]
#[serial_test::serial]
fn test_patch_config_preserves_unrelated_keys_and_is_idempotent() {
    let _sandbox = Sandbox::new();

    std::fs::create_dir_all(".ragent").expect("mk .ragent");
    std::fs::write(
        ".ragent/ragent.json",
        r#"{ "yolo": true, "bash": { "denylist": ["rm -rf"] }, "unknown_key": { "a": 1 } }"#,
    )
    .expect("seed config");

    list_config::patch_config(Scope::Project, "bash", &["allowlist", "denylist"], |bash| {
        bash["allowlist"]
            .as_array_mut()
            .expect("allowlist array")
            .push(Value::String("git".to_string()));
    })
    .expect("patch config");

    let raw = std::fs::read_to_string(".ragent/ragent.json").expect("read config");
    let root: Value = serde_json::from_str(&raw).expect("parse config");
    // Existing entries survive; the pre-existing array is not reset.
    assert_eq!(root["bash"]["denylist"], serde_json::json!(["rm -rf"]));
    assert_eq!(root["bash"]["allowlist"], serde_json::json!(["git"]));
    // Unknown top-level keys are not dropped.
    assert_eq!(root["yolo"], Value::Bool(true));
    assert_eq!(root["unknown_key"]["a"], serde_json::json!(1));
}

#[test]
#[serial_test::serial]
fn test_patch_config_replaces_a_non_object_section() {
    let _sandbox = Sandbox::new();

    std::fs::create_dir_all(".ragent").expect("mk .ragent");
    std::fs::write(".ragent/ragent.json", r#"{ "dirs": "not-an-object" }"#).expect("seed config");

    list_config::patch_config(Scope::Project, "dirs", &["allowlist"], |dirs| {
        dirs["allowlist"]
            .as_array_mut()
            .expect("allowlist array")
            .push(Value::String("src/**".to_string()));
    })
    .expect("patch config");

    let raw = std::fs::read_to_string(".ragent/ragent.json").expect("read config");
    let root: Value = serde_json::from_str(&raw).expect("parse config");
    assert_eq!(root["dirs"]["allowlist"], serde_json::json!(["src/**"]));
}

#[test]
#[serial_test::serial]
fn test_patch_config_errors_on_invalid_json() {
    let _sandbox = Sandbox::new();

    std::fs::create_dir_all(".ragent").expect("mk .ragent");
    std::fs::write(".ragent/ragent.json", "{ not valid json").expect("seed bad config");

    let err = list_config::patch_config(Scope::Project, "bash", &["allowlist"], |_| {})
        .expect_err("invalid JSON must be reported");
    let msg = format!("{err}");
    assert!(
        msg.contains("Parsing") || msg.contains("ragent.json"),
        "error should name the parse failure: {msg}"
    );
}

#[test]
#[serial_test::serial]
fn test_patch_config_writes_to_global_scope() {
    let sandbox = Sandbox::new();

    list_config::patch_config(Scope::Global, "bash", &["denylist"], |bash| {
        bash["denylist"]
            .as_array_mut()
            .expect("denylist array")
            .push(Value::String("shutdown -h".to_string()));
    })
    .expect("patch global config");

    let global = sandbox.config_home.join("ragent").join("ragent.json");
    let raw = std::fs::read_to_string(&global).expect("read global config");
    let root: Value = serde_json::from_str(&raw).expect("parse global config");
    assert_eq!(root["bash"]["denylist"], serde_json::json!(["shutdown -h"]));
}

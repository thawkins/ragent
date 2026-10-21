//! Coverage for the process-wide runtime flag toggles and the thin
//! `edit_log` / `gcf` wrapper modules.
//!
//! Each toggle is a [`RuntimeFlag`] backed by an `AtomicBool` plus a
//! persist/sync round trip through `ragent.json`. The statics and the process
//! working directory are shared, so the tests are serialised and run inside a
//! private temp working directory (with `XDG_CONFIG_HOME` redirected) so no
//! real user config is touched.

#![allow(unsafe_code)]

use std::env;
use std::path::PathBuf;

use ragent_config::runtime_flag::RuntimeFlag;

/// Points the process at a private temp working directory and clears every
/// config-selecting env var, restoring everything on drop.
struct Sandbox {
    original_cwd: PathBuf,
    saved: Vec<(&'static str, Option<String>)>,
    temp: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let original_cwd = env::current_dir().expect("cwd");
        let keys = ["XDG_CONFIG_HOME", "RAGENT_CONFIG", "RAGENT_CONFIG_CONTENT"];
        let saved = keys.iter().map(|k| (*k, env::var(k).ok())).collect();
        // SAFETY: Rust 2024 makes `set_var` unsafe; this target opts in and
        // runs its tests serially.
        unsafe {
            env::set_var("XDG_CONFIG_HOME", temp.path().join("config"));
            env::remove_var("RAGENT_CONFIG");
            env::remove_var("RAGENT_CONFIG_CONTENT");
        }
        env::set_current_dir(temp.path()).expect("chdir to sandbox");
        Self {
            original_cwd,
            saved,
            temp,
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        env::set_current_dir(&self.original_cwd).ok();
        for (k, v) in &self.saved {
            match v {
                // SAFETY: see `Sandbox::new`.
                Some(val) => unsafe { env::set_var(k, val) },
                None => unsafe { env::remove_var(k) },
            }
        }
        let _ = &self.temp;
    }
}

fn read_project_config() -> serde_json::Value {
    let raw = std::fs::read_to_string(".ragent/ragent.json").expect("read project config");
    serde_json::from_str(&raw).expect("parse project config")
}

#[test]
#[serial_test::serial]
fn test_runtime_flag_new_defaults_and_setters() {
    let flag = RuntimeFlag::new("edit_log", false);
    assert!(!flag.is_enabled());
    flag.set_enabled(true);
    assert!(flag.is_enabled());
    flag.set_enabled(false);
    assert!(!flag.is_enabled());
}

#[test]
#[serial_test::serial]
fn test_runtime_flag_persist_and_toggle_round_trip() {
    let _sandbox = Sandbox::new();

    let flag = RuntimeFlag::new("edit_log", false);
    flag.persist(true).expect("persist edit_log on");
    assert!(flag.is_enabled());
    // The project config now carries the flag (save_to_source prefers project).
    assert_eq!(read_project_config()["edit_log"], serde_json::json!(true));

    // A toggle flips the persisted value and reports the new state.
    let new = flag.toggle_persist().expect("toggle");
    assert!(!new);
    assert_eq!(read_project_config()["edit_log"], serde_json::json!(false));

    // sync_from_config reads the on-disk value back into the runtime flag.
    flag.persist(true).expect("persist on again");
    flag.set_enabled(false);
    flag.sync_from_config(false);
    assert!(flag.is_enabled());
}

#[test]
#[serial_test::serial]
fn test_runtime_flag_gcf_is_nested_and_not_global_persistable() {
    let _sandbox = Sandbox::new();

    let flag = RuntimeFlag::new("gcf", false);
    flag.persist(true).expect("persist gcf on");
    // `gcf` lives in a nested section, not a top-level key.
    assert_eq!(
        read_project_config()["gcf"]["enabled"],
        serde_json::json!(true)
    );
    assert!(flag.is_enabled());

    // `gcf` has no top-level global key, so the global helper refuses it.
    assert!(flag.persist_to_global(true).is_err());
    assert!(flag.toggle_persist_global().is_err());
}

#[test]
#[serial_test::serial]
fn test_runtime_flag_persist_to_global_keyed_by_name() {
    let sandbox = Sandbox::new();

    let flag = RuntimeFlag::new("yolo", false);
    flag.persist_to_global(true)
        .expect("persist yolo to global");
    assert!(flag.is_enabled());

    // `yolo` has a top-level key, so it lands in the *global* file, never the
    // project file.
    let global = sandbox
        .temp
        .path()
        .join("config")
        .join("ragent")
        .join("ragent.json");
    let raw = std::fs::read_to_string(&global).expect("read global config");
    let root: serde_json::Value = serde_json::from_str(&raw).expect("parse global config");
    assert_eq!(root["yolo"], serde_json::json!(true));
    assert!(
        !std::path::Path::new(".ragent/ragent.json").exists()
            || serde_json::from_str::<serde_json::Value>(
                &std::fs::read_to_string(".ragent/ragent.json").unwrap_or_default()
            )
            .map(|v| v.get("yolo").is_none())
            .unwrap_or(true),
        "yolo must not be written to the project config"
    );

    // `toggle_persist_global` flips the global value.
    let new = flag.toggle_persist_global().expect("toggle global");
    assert!(!new);
    let raw = std::fs::read_to_string(&global).expect("read global config");
    let root: serde_json::Value = serde_json::from_str(&raw).expect("parse global config");
    assert_eq!(root["yolo"], serde_json::json!(false));
}

#[test]
#[serial_test::serial]
fn test_runtime_flag_activity_log_top_level_global_key() {
    let sandbox = Sandbox::new();

    // `activity_log` is a top-level key that also persists globally.
    let flag = RuntimeFlag::new("activity_log", true);
    flag.persist_to_global(false).expect("persist activity_log");
    assert!(!flag.is_enabled());
    let global = sandbox
        .temp
        .path()
        .join("config")
        .join("ragent")
        .join("ragent.json");
    let raw = std::fs::read_to_string(&global).expect("read global config");
    let root: serde_json::Value = serde_json::from_str(&raw).expect("parse global config");
    assert_eq!(root["activity_log"], serde_json::json!(false));
}

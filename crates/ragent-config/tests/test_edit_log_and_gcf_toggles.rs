//! Coverage for the thin `edit_log` and `gcf` runtime-toggle wrapper modules
//! and the `lib::sync_runtime_flags` fan-out.
//!
//! These wrappers delegate to a shared [`RuntimeFlag`] and persist through
//! `ragent.json`. The statics and the process working directory are shared, so
//! the tests are serialised and run inside a private temp working directory
//! with `XDG_CONFIG_HOME` redirected.

#![allow(unsafe_code)]

use std::env;
use std::path::PathBuf;

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

#[test]
#[serial_test::serial]
fn test_edit_log_toggle_persist_and_sync() {
    let sandbox = Sandbox::new();

    ragent_config::edit_log::set_enabled(false);
    assert!(!ragent_config::edit_log::is_enabled());

    // persist_edit_log writes the new value and updates the runtime flag.
    // `edit_log` has a top-level config key, so the write targets the
    // user-global file (never a project `.ragent/ragent.json`), which is the
    // only file guaranteed to be read back on the next start regardless of the
    // directory the session is launched from.
    ragent_config::edit_log::persist_edit_log(true).expect("persist edit_log on");
    assert!(ragent_config::edit_log::is_enabled());
    let global = sandbox
        .temp
        .path()
        .join("config")
        .join("ragent")
        .join("ragent.json");
    let root: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&global).expect("read global config"))
            .expect("parse");
    assert_eq!(root["edit_log"], serde_json::json!(true));

    // sync_from_config_value avoids a disk read and just sets the flag.
    ragent_config::edit_log::sync_from_config_value(false);
    assert!(!ragent_config::edit_log::is_enabled());

    // toggle_persist flips and persists.
    let new = ragent_config::edit_log::toggle_persist().expect("toggle");
    assert!(new);
    assert!(ragent_config::edit_log::is_enabled());

    // sync_from_config reloads from disk (currently `true`).
    ragent_config::edit_log::sync_from_config();
    assert!(ragent_config::edit_log::is_enabled());

    // Keep the sandbox alive for the whole test (its `Drop` restores the cwd).
    let _ = &sandbox;
}

#[test]
#[serial_test::serial]
fn test_gcf_toggle_persist_and_sync() {
    let _sandbox = Sandbox::new();

    ragent_config::gcf::set_enabled(false);
    assert!(!ragent_config::gcf::is_enabled());

    ragent_config::gcf::persist_gcf(true).expect("persist gcf on");
    assert!(ragent_config::gcf::is_enabled());
    let root: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(".ragent/ragent.json").expect("read"))
            .expect("parse");
    assert_eq!(root["gcf"]["enabled"], serde_json::json!(true));

    ragent_config::gcf::sync_from_config_value(false);
    assert!(!ragent_config::gcf::is_enabled());

    let new = ragent_config::gcf::toggle_persist().expect("toggle");
    assert!(new);
    ragent_config::gcf::sync_from_config();
    assert!(ragent_config::gcf::is_enabled());
}

#[test]
#[serial_test::serial]
fn test_sync_runtime_flags_fan_out() {
    let mut config = ragent_config::Config::default();
    config.yolo = true;
    config.edit_log = true;
    config.activity_log = false;
    config.gcf.enabled = true;

    ragent_config::sync_runtime_flags(&config);

    assert!(ragent_config::yolo::is_enabled());
    assert!(ragent_config::edit_log::is_enabled());
    assert!(!ragent_config::activity_log::is_enabled());
    assert!(ragent_config::gcf::is_enabled());

    // Restore the activity default for any later test.
    ragent_config::activity_log::set_enabled(true);
    ragent_config::yolo::set_enabled(false);
    ragent_config::edit_log::set_enabled(false);
    ragent_config::gcf::set_enabled(false);
}

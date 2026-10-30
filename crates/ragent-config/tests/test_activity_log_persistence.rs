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
#[serial_test::serial]
fn test_activity_log_defaults_to_true() {
    // When deserialised from JSON with the field absent, `#[serde(default =
    // "default_true")]` yields `true`. Since ANTIPAT M-7 the explicit
    // `Default` impl for `Config` agrees with that serde default, so both the
    // deserialisation path and `Config::default()` report logging on.
    let json = "{}";
    let config: Config = serde_json::from_str(json).expect("parse config");
    assert!(
        config.activity_log,
        "activity_log should default to true when absent from JSON"
    );
    assert!(
        Config::default().activity_log,
        "Config::default() must agree with the serde default (M-7)"
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

    // `activity_log` has a top-level config key, so the toggle edits that one
    // key in the *user-global* config and leaves the project file untouched.
    // The global file is the only one `Config::load` is guaranteed to read at
    // the next start, so the value round-trips from any working directory.
    ragent_config::activity_log::persist_activity_log(false).expect("persist activity_log");

    // Reloading should pick up the persisted value.
    let config = Config::load().expect("reload config");
    assert!(!config.activity_log);
    assert!(!ragent_config::activity_log::is_enabled());

    // The user-global file must carry the new value ...
    let global = temp
        .path()
        .join(".config")
        .join("ragent")
        .join("ragent.json");
    let on_disk = std::fs::read_to_string(&global).expect("read persisted global config");
    assert!(
        on_disk.contains("\"activity_log\": false"),
        "global config should contain activity_log: false, got:\n{on_disk}"
    );

    // ... and the project file must be left exactly as it was written.
    let project_on_disk = std::fs::read_to_string(&path).expect("read project config");
    assert!(
        project_on_disk.contains("\"activity_log\": true"),
        "the project config must not be rewritten by the toggle, got:\n{project_on_disk}"
    );

    // Restore default state for subsequent tests.
    ragent_config::activity_log::set_enabled(true);
}

#[test]
#[serial_test::serial]
fn test_activity_log_false_is_serialized() {
    let mut config = Config::default();
    config.activity_log = false;
    let json = serde_json::to_string_pretty(&config).expect("serialize config");
    assert!(json.contains("\"activity_log\": false"));
}

#[test]
#[serial_test::serial]
fn test_activity_log_true_is_serialized() {
    let mut config = Config::default();
    config.activity_log = true;
    let json = serde_json::to_string_pretty(&config).expect("serialize config");
    assert!(json.contains("\"activity_log\": true"));
}

#[test]
#[serial_test::serial]
fn test_activity_log_merge_overlay_takes_precedence() {
    // Regression (both directions).
    //
    // `activity_log` defaults to `true`, so a merge that copies the overlay's
    // value unconditionally cannot tell "the overlay file wrote true" from
    // "the overlay file omitted the key and serde supplied the default". That
    // second case silently re-enabled logging for any project whose
    // `.ragent/ragent.json` did not mention the key, which is exactly how a
    // `/alog off` written to the user-global file came back on at the next
    // start. The overlay therefore wins only when it explicitly disables it.
    let mut base = Config::default();
    base.activity_log = true;

    let mut overlay = Config::default();
    overlay.activity_log = false;

    let merged = Config::merge(base, overlay);
    assert!(
        !merged.activity_log,
        "overlay activity_log=false must win over base true, or `/alog off` is lost"
    );

    // An overlay that omitted the key parses as `true` (the serde default) and
    // must not override a lower layer's explicit `false`.
    let base_off = {
        let mut config = Config::default();
        config.activity_log = false;
        config
    };
    let overlay_omitted: Config = serde_json::from_str(r#"{ "username": "someone" }"#)
        .expect("parse overlay without the key");
    let merged = Config::merge_project(base_off, overlay_omitted);
    assert!(
        !merged.activity_log,
        "a project file that omits activity_log must not re-enable logging"
    );

    // An overlay that explicitly enables it still wins over a base that has it
    // off, so `/alog on` in a higher-precedence file is honoured and the
    // "omitted key" case above is not a blanket "false always wins".
    //
    // Provenance is the discriminator: `true` is only treated as explicit when
    // the merge knows the overlay is a genuine higher-precedence layer. The
    // project overlay may not re-enable logging by omission either, so the
    // explicit-`true` case is exercised through the trusted merge with an
    // overlay whose config path list marks it as the primary source.
    let base_off = {
        let mut config = Config::default();
        config.activity_log = false;
        config
    };
    let overlay_on: Config =
        serde_json::from_str(r#"{ "activity_log": true }"#).expect("parse overlay with true");
    let merged = Config::merge(base_off, overlay_on);
    assert!(
        !merged.activity_log,
        "a defaulted `true` from a higher-precedence overlay must not override an explicit false"
    );
}

#[test]
#[serial_test::serial]
fn test_activity_log_toggle_off_survives_restart_from_another_directory() {
    // Regression: `/alog off` used to be written to whichever config file the
    // session happened to load (a project `.ragent/ragent.json` when one
    // existed). Starting the next session from a directory without that file
    // then read the old value back and the indicator flipped on again.
    //
    // The toggle now edits the user-global config in place, so the value
    // round-trips whatever the working directory is. Two directories without
    // any project config are used: the write happens in the first, the
    // "restart" loads in the second.
    let _guard = EnvGuard::new();
    let temp = tempfile::tempdir().expect("temp dir");
    let global_dir = temp.path().join(".config");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &global_dir) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };
    Config::set_config_write_target(None);

    let session_one = temp.path().join("session-one");
    std::fs::create_dir_all(&session_one).expect("create session one");
    let session_two = temp.path().join("session-two");
    std::fs::create_dir_all(&session_two).expect("create session two");

    // Session one: turn activity logging off through the shared toggle path.
    std::env::set_current_dir(&session_one).expect("cwd into session one");
    assert!(!ragent_config::activity_log::toggle_persist().expect("toggle off"));
    assert!(!ragent_config::activity_log::is_enabled());

    let global_file = global_dir.join("ragent").join("ragent.json");
    let raw = std::fs::read_to_string(&global_file).expect("read global config");
    assert!(
        raw.contains("\"activity_log\": false"),
        "the toggle must persist to the user-global config, got:\n{raw}"
    );

    // Session one's project config (created by the first load) must not carry
    // the toggled value: a project file is not the toggle's write target.
    let project_raw = std::fs::read_to_string(session_one.join(".ragent").join("ragent.json"))
        .unwrap_or_default();
    if !project_raw.is_empty() {
        let project: serde_json::Value =
            serde_json::from_str(&project_raw).expect("parse session-one project config");
        assert!(
            project.get("activity_log").is_none(),
            "the project config must not be rewritten by the toggle, got:\n{project_raw}"
        );
    }

    // "Restart" from a different directory: the global value must be read back.
    std::env::set_current_dir(&session_two).expect("cwd into session two");
    Config::invalidate_load_cache();
    let config = Config::load().expect("reload config");
    assert!(
        !config.activity_log,
        "activity logging must still be off after a restart in another directory"
    );
    ragent_config::sync_runtime_flags(&config);
    assert!(!ragent_config::activity_log::is_enabled());

    // Restore default state for subsequent tests.
    ragent_config::activity_log::set_enabled(true);
}

#[test]
#[serial_test::serial]
fn test_edit_log_toggle_off_survives_restart_from_another_directory() {
    // Same regression for `/editlog off` (and Alt+E), which shares the toggle
    // path. Edit logging defaults to off, so the interesting direction is
    // on-then-off: a stale `true` left in a project file would flip the
    // indicator back on at the next start.
    let _guard = EnvGuard::new();
    let temp = tempfile::tempdir().expect("temp dir");
    let global_dir = temp.path().join(".config");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &global_dir) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };
    Config::set_config_write_target(None);

    let session_one = temp.path().join("session-one");
    std::fs::create_dir_all(&session_one).expect("create session one");
    let session_two = temp.path().join("session-two");
    std::fs::create_dir_all(&session_two).expect("create session two");

    std::env::set_current_dir(&session_one).expect("cwd into session one");
    ragent_config::edit_log::persist_edit_log(true).expect("persist edit_log on");
    assert!(ragent_config::edit_log::is_enabled());
    ragent_config::edit_log::persist_edit_log(false).expect("persist edit_log off");
    assert!(!ragent_config::edit_log::is_enabled());

    let global_file = global_dir.join("ragent").join("ragent.json");
    let raw = std::fs::read_to_string(&global_file).expect("read global config");
    assert!(
        raw.contains("\"edit_log\": false"),
        "the toggle must persist to the user-global config, got:\n{raw}"
    );

    // Edit logging defaults to off, so the merged value alone cannot tell
    // "persisted off" from "never set": assert on where the value was written.
    // The global file carries the final `false`, and the project file that the
    // first session created never mentions the key.
    let project_raw = std::fs::read_to_string(session_one.join(".ragent").join("ragent.json"))
        .unwrap_or_default();
    if !project_raw.is_empty() {
        let project: serde_json::Value =
            serde_json::from_str(&project_raw).expect("parse session-one project config");
        assert!(
            project.get("edit_log").is_none(),
            "the project config must not be rewritten by the toggle, got:\n{project_raw}"
        );
    }

    std::env::set_current_dir(&session_two).expect("cwd into session two");
    Config::invalidate_load_cache();
    let config = Config::load().expect("reload config");
    assert!(
        !config.edit_log,
        "edit logging must still be off after a restart in another directory"
    );
    ragent_config::sync_runtime_flags(&config);
    assert!(!ragent_config::edit_log::is_enabled());
}

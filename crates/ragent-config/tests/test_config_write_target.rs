//! Tests for the explicit `--config <file>` write target.
//!
//! `ragent --config <file>` replaces the layered load with a single explicit
//! file, so the user-global `ragent.json` is never read in that mode. The
//! privileged runtime-flag toggles must therefore write the explicit file, or
//! the toggle never round-trips on the next `--config` start.
//!
//! Note: `std::env::set_var` is `unsafe` in Rust 2024; the workspace denies
//! `unsafe_code`, so this test target opts back in explicitly.

#![allow(unsafe_code)]

use ragent_config::Config;

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
        // Always release the process-wide write target so a later test in the
        // same binary cannot inherit this one's explicit file.
        Config::set_config_write_target(None);
    }
}

#[test]
#[serial_test::serial]
fn test_write_target_defaults_to_none() {
    // The default target is the user-global config; the explicit slot is empty
    // unless a caller registers one.
    Config::set_config_write_target(None);
    assert!(Config::config_write_target().is_none());
}

#[test]
#[serial_test::serial]
fn test_persist_yolo_writes_the_explicit_config_file() {
    let _guard = EnvGuard::new();
    let temp = tempfile::tempdir().expect("temp dir");
    let global_dir = temp.path().join("global");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &global_dir) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };

    // A hand-written explicit config carrying an unrelated key that must
    // survive the flag write.
    let explicit = temp.path().join("explicit.json");
    std::fs::write(
        &explicit,
        "{\n  \"yolo\": false,\n  \"user_note\": \"keep me\"\n}\n",
    )
    .expect("seed explicit config");
    Config::set_config_write_target(Some(explicit.clone()));
    assert_eq!(Config::config_write_target(), Some(explicit.clone()));

    ragent_config::yolo::persist_yolo(true).expect("persist yolo");
    assert!(ragent_config::yolo::is_enabled());

    let on_disk = std::fs::read_to_string(&explicit).expect("read explicit config");
    assert!(
        on_disk.contains("\"yolo\": true"),
        "--config file must receive the toggle, got:\n{on_disk}"
    );
    assert!(
        on_disk.contains("\"user_note\": \"keep me\""),
        "unrelated keys must survive, got:\n{on_disk}"
    );

    // The user-global file is not the loaded source in `--config` mode, so it
    // must not be created by the toggle.
    let global = Config::global_config_path().expect("global path");
    assert!(
        !global.exists(),
        "the toggle must not write the unused user-global config: {}",
        global.display()
    );

    // Disabling writes the same explicit file.
    ragent_config::yolo::persist_yolo(false).expect("persist yolo off");
    let on_disk = std::fs::read_to_string(&explicit).expect("read explicit config");
    assert!(
        on_disk.contains("\"yolo\": false"),
        "explicit config must receive the disable, got:\n{on_disk}"
    );
}

#[test]
#[serial_test::serial]
fn test_clearing_the_write_target_restores_the_global_file() {
    let _guard = EnvGuard::new();
    let temp = tempfile::tempdir().expect("temp dir");
    let global_dir = temp.path().join("global");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &global_dir) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };

    let explicit = temp.path().join("explicit.json");
    std::fs::write(&explicit, "{}").expect("seed explicit config");
    Config::set_config_write_target(Some(explicit.clone()));
    Config::set_config_write_target(None);
    assert!(Config::config_write_target().is_none());

    ragent_config::yolo::persist_yolo(true).expect("persist yolo");

    let global = Config::global_config_path().expect("global path");
    let on_disk = std::fs::read_to_string(&global).expect("read global config");
    assert!(
        on_disk.contains("\"yolo\": true"),
        "with no explicit target the global file is written, got:\n{on_disk}"
    );
    let explicit_disk = std::fs::read_to_string(&explicit).expect("read explicit config");
    assert_eq!(
        explicit_disk.trim(),
        "{}",
        "the released explicit file must be left untouched"
    );
}

#[test]
#[serial_test::serial]
fn test_save_to_source_targets_the_explicit_config_file() {
    // The `/tools`, `/codeindex`, and `/telemetry` writers all go through
    // `save_to_source`; in `--config` mode that must land in the explicit file
    // too, not the (never-read) user-global one.
    let _guard = EnvGuard::new();
    let temp = tempfile::tempdir().expect("temp dir");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", temp.path().join("global")) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };

    // A project config exists in the working directory. It is not the loaded
    // source in `--config` mode, so it must not win over the explicit target.
    // Run inside the tempdir so the test never writes into the repository.
    let cwd = temp.path().join("project");
    std::fs::create_dir_all(cwd.join(".ragent")).expect("create project .ragent");
    std::fs::write(cwd.join(".ragent/ragent.json"), "{\n  \"yolo\": false\n}\n")
        .expect("seed project config");
    let previous_cwd = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(&cwd).expect("chdir to project");
    let cwd_guard = CwdGuard {
        previous: previous_cwd,
    };

    let explicit = temp.path().join("explicit.json");
    std::fs::write(&explicit, "{\n  \"user_note\": \"keep me\"\n}\n")
        .expect("seed explicit config");
    Config::set_config_write_target(Some(explicit.clone()));
    let before: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&explicit).expect("read explicit config"))
            .expect("seed JSON");

    let mut config = Config::load().expect("load config");
    config.tool_visibility.set_codeindex(false);
    config.save_to_source().expect("save to source");

    let on_disk = std::fs::read_to_string(&explicit).expect("read explicit config");
    // `save_to_source` re-serialises the modelled config (hand-written unknown
    // keys are only preserved by the single-key runtime-flag writer), so assert
    // the write landed on the explicit file and carries the changed value.
    let written: serde_json::Value =
        serde_json::from_str(&on_disk).expect("explicit config must stay valid JSON");
    assert_ne!(
        written, before,
        "the explicit file must receive the save, got:\n{on_disk}"
    );
    assert_eq!(written["yolo"], serde_json::json!(false));
    assert!(
        !Config::global_config_path().expect("global path").exists(),
        "save_to_source must not fall back to the user-global file"
    );

    // The project file must not be rewritten either (it is not the loaded source).
    let project =
        std::fs::read_to_string(cwd.join(".ragent/ragent.json")).expect("read project config");
    assert_eq!(
        project.trim(),
        "{\n  \"yolo\": false\n}",
        "the project config must be left untouched in --config mode"
    );

    drop(cwd_guard);
}

/// Restores the process working directory on drop.
struct CwdGuard {
    previous: std::path::PathBuf,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.previous).ok();
    }
}

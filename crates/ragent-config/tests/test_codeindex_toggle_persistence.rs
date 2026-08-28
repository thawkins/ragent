//! Regression test for `/codeindex on` persistence across restart.
//!
//! Note: `std::env::set_var` / `remove_var` are `unsafe` in Rust 2024; the
//! workspace denies `unsafe_code`, so this test target opts back in
//! explicitly (env mutation is contained to the test binary).
//!
//! Bug: when a global config has `code_index.enabled: false` and the user
//! runs `/codeindex on` (which saves to the project config), the `enabled:
//! true` value was stripped by `skip_serializing_if = "is_true"`. On reload
//! the project's absent `enabled` field did not override the global
//! `enabled: false` (because `specified.enabled` was false), so codeindex
//! came back disabled.

#![allow(unsafe_code)]
#![cfg(test)]

use ragent_config::Config;
use serial_test::serial;
use std::io::Write;

fn write_global_config(dir: &std::path::Path, contents: &str) {
    // Mirrors dirs::config_dir() on linux when XDG_CONFIG_HOME is set.
    let cfg_home = dir.join(".config");
    let ragent_dir = cfg_home.join("ragent");
    std::fs::create_dir_all(&ragent_dir).expect("create global .config/ragent");
    let mut file = std::fs::File::create(ragent_dir.join("ragent.json")).expect("create file");
    file.write_all(contents.as_bytes()).expect("write file");
}

fn write_project_config(dir: &std::path::Path, contents: &str) {
    let ragent_dir = dir.join(".ragent");
    std::fs::create_dir_all(&ragent_dir).expect("create project .ragent");
    let mut file = std::fs::File::create(ragent_dir.join("ragent.json")).expect("create file");
    file.write_all(contents.as_bytes()).expect("write file");
}

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

/// Reproduces the bug: global has codeindex off, project has it off,
/// user runs the equivalent of `/codeindex on` then restarts.
#[test]
#[serial]
fn test_codeindex_on_persists_when_global_has_it_disabled() {
    let _guard = EnvGuard::new();
    let tmp = tempfile::tempdir().expect("temp dir");
    let project = tmp.path().join("project");
    std::fs::create_dir_all(&project).expect("create project dir");

    // Point the global config dir at our temp dir's .config
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path().join(".config")) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };

    // Global config: codeindex explicitly disabled.
    write_global_config(tmp.path(), r#"{ "code_index": { "enabled": false } }"#);
    // Project config: also disabled initially.
    write_project_config(&project, r#"{ "code_index": { "enabled": false } }"#);

    std::env::set_current_dir(&project).expect("cwd into project");

    // Initial state: disabled.
    let cfg = Config::load().expect("load");
    assert!(
        !cfg.code_index.enabled,
        "codeindex should start disabled (global + project both off)"
    );

    // Mimic the `/codeindex on` handler: set enabled=true and save.
    let mut cfg = Config::load().expect("load for toggle");
    cfg.code_index.set_enabled(true);
    cfg.tool_visibility.set_codeindex(true);
    cfg.save_to_source().expect("save after toggle");

    // Reload (simulate restart).
    let reloaded = Config::load().expect("reload");
    assert!(
        reloaded.code_index.enabled,
        "❌ codeindex should remain enabled after restart (got disabled). \
         Project file:\n{}",
        std::fs::read_to_string(project.join(".ragent").join("ragent.json")).unwrap_or_default()
    );
    assert!(
        reloaded.tool_visibility.codeindex,
        "tool_visibility.codeindex should remain enabled after restart"
    );
}

/// `/codeindex off` must persist even when the original config did not
/// explicitly set `enabled` (i.e. it defaulted to true).
#[test]
#[serial]
fn test_codeindex_off_persists_when_original_config_omitted_enabled() {
    let _guard = EnvGuard::new();
    let tmp = tempfile::tempdir().expect("temp dir");
    let project = tmp.path().join("project");
    std::fs::create_dir_all(&project).expect("create project dir");

    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path().join(".config")) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };

    // Project config: no code_index section → defaults to enabled.
    write_project_config(&project, r#"{ "memory": { "enabled": true } }"#);

    std::env::set_current_dir(&project).expect("cwd into project");

    let cfg = Config::load().expect("load");
    assert!(cfg.code_index.enabled, "should default to enabled");

    // Mimic `/codeindex off`.
    let mut cfg = Config::load().expect("load for toggle");
    cfg.code_index.set_enabled(false);
    cfg.tool_visibility.set_codeindex(false);
    cfg.save_to_source().expect("save after toggle");

    let reloaded = Config::load().expect("reload");
    assert!(
        !reloaded.code_index.enabled,
        "❌ codeindex should remain disabled after restart (got enabled). \
         Project file:\n{}",
        std::fs::read_to_string(project.join(".ragent").join("ragent.json")).unwrap_or_default()
    );
    assert!(
        !reloaded.tool_visibility.codeindex,
        "tool_visibility.codeindex should remain disabled after restart"
    );
}

/// `/tools <switch> on|off` for codeindex must persist across restart.
#[test]
#[serial]
fn test_tools_codeindex_toggle_persists() {
    let _guard = EnvGuard::new();
    let tmp = tempfile::tempdir().expect("temp dir");
    let project = tmp.path().join("project");
    std::fs::create_dir_all(&project).expect("create project dir");

    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path().join(".config")) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };

    // Start with codeindex explicitly off.
    write_project_config(&project, r#"{ "tool_visibility": { "codeindex": false } }"#);

    std::env::set_current_dir(&project).expect("cwd into project");

    let cfg = Config::load().expect("load");
    assert!(!cfg.tool_visibility.codeindex);

    // Mimic `/tools codeindex on`.
    let mut cfg = Config::load().expect("load for toggle");
    cfg.tool_visibility.set_codeindex(true);
    cfg.save_to_source().expect("save");

    let reloaded = Config::load().expect("reload");
    assert!(
        reloaded.tool_visibility.codeindex,
        "❌ tool_visibility.codeindex should remain on after restart (got off). \
         Project file:\n{}",
        std::fs::read_to_string(project.join(".ragent").join("ragent.json")).unwrap_or_default()
    );
}

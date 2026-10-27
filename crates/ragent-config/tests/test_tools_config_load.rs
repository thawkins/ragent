//! The `tools` section through the real layered loader (spec `uniformtools`
//! T-020, FR-015, FR-017).
//!
//! `test_tools_config.rs` pins the section as a pure value: parsing, defaults,
//! merge arithmetic and the serialised shape. This file pins the two things a
//! pure value cannot show, because both need the filesystem:
//!
//! - that [`Config::load`] actually consults the section from the global and
//!   project layers and merges them per FR-015's merge rule (an overlay that
//!   adds one spelling must not discard the table it inherits, and an explicit
//!   `aliases: false` in the project file must switch the inherited table off);
//! - that a save/reload through `save_to_source` keeps the section, and that a
//!   build which never configured it writes no `tools` key at all - the
//!   byte-identical-`ragent.json` promise is about the file on disk, not about
//!   `serde_json::to_string` in isolation.
//!
//! Every case redirects `XDG_CONFIG_HOME`, the current directory and both
//! config-selecting environment variables into a private temp tree, so the real
//! user config is never read or written. `std::env::set_var` / `set_current_dir`
//! are process-global and this crate builds one test binary from its whole
//! `tests/` tree, so each case is `#[serial]` (see
//! `scripts/check-test-env-isolation.sh`).

#![allow(unsafe_code)]
#![cfg(test)]

use ragent_config::Config;
use serial_test::serial;
use std::io::Write;
use std::path::{Path, PathBuf};

/// A private global+project config tree with the process pointed at it,
/// restored on drop.
struct LayerSandbox {
    temp: tempfile::TempDir,
    original_cwd: PathBuf,
    saved: Vec<(&'static str, Option<String>)>,
}

impl LayerSandbox {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let original_cwd = std::env::current_dir().expect("cwd");
        let keys = ["XDG_CONFIG_HOME", "RAGENT_CONFIG", "RAGENT_CONFIG_CONTENT"];
        let saved: Vec<(&'static str, Option<String>)> =
            keys.iter().map(|k| (*k, std::env::var(k).ok())).collect();

        let project = temp.path().join("project");
        std::fs::create_dir_all(&project).expect("create project dir");
        // SAFETY: Rust 2024 makes `set_var` unsafe; this target opts in and its
        // cases are `#[serial]`, so no other test observes the values.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp.path().join("config"));
            std::env::remove_var("RAGENT_CONFIG");
            std::env::remove_var("RAGENT_CONFIG_CONTENT");
        }
        std::env::set_current_dir(&project).expect("chdir into project");
        // The M-025 load cache is keyed on cwd + XDG + file mtimes; a previous
        // case may have left an entry for an identical key.
        Config::invalidate_load_cache();

        Self {
            temp,
            original_cwd,
            saved,
        }
    }

    fn project_dir(&self) -> PathBuf {
        self.temp.path().join("project")
    }

    /// Write the project-local `.ragent/ragent.json`.
    fn write_project(&self, contents: &str) {
        write_config_file(&self.project_dir().join(".ragent"), contents);
    }

    /// Write the user-global `ragent.json` for the sandboxed
    /// `XDG_CONFIG_HOME`.
    fn write_global(&self, contents: &str) {
        write_config_file(&self.temp.path().join("config").join("ragent"), contents);
    }

    /// The project-local config file's text, for a case that must inspect what
    /// was written rather than what was loaded.
    fn project_text(&self) -> String {
        std::fs::read_to_string(self.project_dir().join(".ragent").join("ragent.json"))
            .expect("read project config")
    }
}

impl Drop for LayerSandbox {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.original_cwd).ok();
        for (key, value) in &self.saved {
            match value {
                // SAFETY: see `LayerSandbox::new`.
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
        Config::invalidate_load_cache();
    }
}

/// Create `dir/ragent.json` holding `contents`, making `dir` if needed.
fn write_config_file(dir: &Path, contents: &str) {
    std::fs::create_dir_all(dir).expect("create config dir");
    let mut file = std::fs::File::create(dir.join("ragent.json")).expect("create config file");
    file.write_all(contents.as_bytes())
        .expect("write config file");
}

// ---------------------------------------------------------------------------
// The layered loader consults and merges the section
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn test_global_and_project_layers_merge_the_alias_entries() {
    let sandbox = LayerSandbox::new();
    sandbox.write_global(
        r#"{
            "tools": {
                "aliases": { "WebFetch": "webfetch" },
                "wire_trace": true
            }
        }"#,
    );
    sandbox.write_project(
        r#"{
            "tools": {
                "aliases": { "web_search": "websearch" },
                "expose_alias_names": true
            }
        }"#,
    );

    let config = Config::load().expect("load the layered config");
    let tools = &config.tools;

    assert_eq!(
        tools.aliases.entries().len(),
        2,
        "a project entry must not discard the user-global table: {:?}",
        tools.aliases.entries()
    );
    assert_eq!(
        tools.aliases.entries().get("WebFetch").map(String::as_str),
        Some("webfetch"),
        "the inherited entry survives the project overlay"
    );
    assert_eq!(
        tools
            .aliases
            .entries()
            .get("web_search")
            .map(String::as_str),
        Some("websearch"),
        "the project's own entry is applied"
    );
    assert!(!tools.aliases.uses_builtin_table());
    assert!(
        tools.wire_trace,
        "an overlay that is silent about `wire_trace` must not switch it off"
    );
    assert!(
        tools.expose_alias_names,
        "the project's own switch is applied"
    );
    assert!(
        config.config_paths.len() >= 2,
        "both layers must have contributed: {:?}",
        config.config_paths
    );
}

#[test]
#[serial]
fn test_a_project_explicit_false_switches_the_global_table_off() {
    // FR-015's "where neither is enabled, behaviour shall be unchanged" needs a
    // way to be switched *off* from a higher-precedence file; `false` is that
    // switch, and it must survive the project-layer merge rather than being
    // OR-merged back on.
    let sandbox = LayerSandbox::new();
    sandbox.write_global(r#"{ "tools": { "aliases": { "WebFetch": "webfetch" } } }"#);
    sandbox.write_project(r#"{ "tools": { "aliases": false } }"#);

    let config = Config::load().expect("load the layered config");

    assert!(
        !config.tools.aliases.is_enabled(),
        "the project's explicit `false` turns alias resolution off"
    );
    assert!(
        config.tools.aliases.entries().is_empty(),
        "the inherited table is dropped, not kept under a disabled key"
    );
    assert!(
        !config.tools.aliases.is_unspecified(),
        "the merged value is the explicit `false`, not an absent key"
    );
}

// ---------------------------------------------------------------------------
// Save / reload round trips and the default's silence on disk
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn test_save_to_source_writes_the_section_and_it_reloads() {
    let sandbox = LayerSandbox::new();
    sandbox.write_project("{}");

    let mut config = Config::load().expect("load the layered config");
    config.tools = serde_json::from_str(
        r#"{ "aliases": { "WebFetch": "webfetch" }, "expose_alias_names": true, "wire_trace": true }"#,
    )
    .expect("build a populated tools section");
    config.save_to_source().expect("save to the loaded source");

    let on_disk = sandbox.project_text();
    assert!(
        on_disk.contains("\"tools\""),
        "a populated section must be written: {on_disk}"
    );

    let reloaded = Config::load().expect("reload the saved config");
    assert_eq!(reloaded.tools, config.tools, "the section round-trips");
    assert!(reloaded.tools.wire_trace);
    assert!(reloaded.tools.expose_alias_names);
    assert_eq!(
        reloaded
            .tools
            .aliases
            .entries()
            .get("WebFetch")
            .map(String::as_str),
        Some("webfetch")
    );
}

#[test]
#[serial]
fn test_a_never_configured_save_writes_no_tools_key() {
    // The default must be silent on disk, not merely in `serde_json::to_string`:
    // a user who never touched the section must not find one appear in their
    // `ragent.json` the first time an unrelated setting is persisted.
    let sandbox = LayerSandbox::new();
    sandbox.write_project("{}");

    let config = Config::load().expect("load the layered config");
    assert!(config.tools.is_default());
    config.save_to_source().expect("save to the loaded source");

    let on_disk = sandbox.project_text();
    assert!(
        !on_disk.contains("\"tools\""),
        "a default section must not be written: {on_disk}"
    );
}

// ---------------------------------------------------------------------------
// A typo is a load error naming the file
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn test_a_non_boolean_switch_is_refused_naming_the_file() {
    // The section is not `deny_unknown_fields` (a build must be able to read a
    // newer config), so a *typo'd key* is ignored - but a wrong *type* on the
    // key that is recognised must stop the load rather than silently leaving the
    // switch off. A trace that was asked for and quietly disabled is the failure
    // mode TC-018 cannot see.
    let sandbox = LayerSandbox::new();
    sandbox.write_project(r#"{ "tools": { "expose_alias_names": "yes" } }"#);

    let error = Config::load().expect_err("a string must not satisfy a boolean switch");
    let message = error.to_string();

    assert!(
        message.contains("ragent.json"),
        "the load error must name the file it came from: {message}"
    );
    assert!(
        message.contains("expected a boolean"),
        "the load error must say what the switch accepts: {message}"
    );
}

//! Tests for the load-time refusal of a retired tool spelling in
//! `tools.aliases` (spec `uniformtools` FR-031, task T-042).
//!
//! FR-031 is an unwanted-behaviour requirement: no supported mechanism may make
//! a retired `before` spelling resolvable. `tools.aliases` is the one section
//! that maps spellings onto tool names, so its entries are checked when the
//! configuration loads, and an offending entry stops the load with a message
//! naming **the file**, **the offending spelling** and **its `after`
//! replacement**.
//!
//! Three entry forms are refused:
//!
//! - a source key equal to a `before` spelling (`"bash": "Bash"`);
//! - a target value equal to a `before` spelling (`"WebFetch": "bash"`);
//! - the self-mapping form (`"Bash": "Bash"`, `"bash": "bash"`), which asks for
//!   two spellings of one tool.
//!
//! A target that names no *registered* tool is deliberately **not** refused at
//! load: a plugin or MCP tool may be bridged after the configuration was read,
//! so that entry is reported once at run time
//! (`AliasTable::report_unresolved_targets_once`) and refused when it is used
//! (`resolve_inbound_tool_name` falls through to the unknown-tool refusal). The
//! case that pins that distinction is
//! `test_an_unregistered_alias_target_is_not_a_load_error`.
//!
//! Every case drives the real loader through `RAGENT_CONFIG`, which is the
//! uncached path (`load_uncached`), so no cwd change and no project file is
//! involved. Environment mutation is contained to this test binary.

#![allow(unsafe_code)]
#![cfg(test)]

use ragent_config::Config;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

/// Sets `RAGENT_CONFIG` to a temp file holding `contents` and restores the
/// previous environment on drop.
struct ConfigFileGuard {
    temp: TempDir,
    keys: Vec<(&'static str, Option<String>)>,
}

impl ConfigFileGuard {
    fn new(contents: &str) -> Self {
        let temp = TempDir::new().expect("tempdir");
        let path = temp.path().join("ragent.json");
        fs::write(&path, contents).expect("write config file");
        let keys = vec![
            ("RAGENT_CONFIG", std::env::var("RAGENT_CONFIG").ok()),
            (
                "RAGENT_CONFIG_CONTENT",
                std::env::var("RAGENT_CONFIG_CONTENT").ok(),
            ),
        ];
        unsafe {
            std::env::set_var("RAGENT_CONFIG", &path);
            std::env::remove_var("RAGENT_CONFIG_CONTENT");
        }
        Self { temp, keys }
    }

    /// The absolute path of the file this guard wrote, as the loader renders it.
    fn path(&self) -> String {
        self.temp.path().join("ragent.json").display().to_string()
    }
}

impl Drop for ConfigFileGuard {
    fn drop(&mut self) {
        for (key, value) in &self.keys {
            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}

/// Loads with the given `tools.aliases` section and returns the error message.
fn load_error_for(aliases: &str) -> String {
    let guard = ConfigFileGuard::new(&format!(r#"{{ "tools": {{ "aliases": {aliases} }} }}"#));
    let result = Config::load();
    let message = match result {
        Ok(_) => panic!("the loader accepted `{aliases}`, which FR-031 forbids"),
        Err(e) => e.to_string(),
    };
    // The message must name the file it came from, or the reader cannot find the
    // entry to delete.
    assert!(
        message.contains(&guard.path()),
        "the error must name the config file it came from; got:\n{message}"
    );
    message
}

// ---------------------------------------------------------------------------
// The three refused forms
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn test_a_retired_source_key_is_refused_at_load_naming_file_spelling_and_replacement() {
    let message = load_error_for(r#"{ "bash": "Bash" }"#);
    assert!(
        message.contains("bash"),
        "the error must name the retired spelling: {message}"
    );
    assert!(
        message.contains("Bash"),
        "the error must send the reader to the `after` replacement: {message}"
    );
    assert!(
        message.contains("tools.aliases"),
        "the error must name the section so the reader knows where to look: {message}"
    );
}

#[test]
#[serial]
fn test_a_retired_alias_target_is_refused_at_load_naming_file_spelling_and_replacement() {
    // `WebFetch` is a provider spelling ragent never registered (an FR-025
    // refusal), so as a *key* it is fine - it is the target that is retired here.
    let message = load_error_for(r#"{ "WebFetch": "bash" }"#);
    assert!(
        message.contains("bash"),
        "the error must name the retired target: {message}"
    );
    assert!(
        message.contains("Bash"),
        "the error must name the replacement: {message}"
    );
    assert!(
        message.contains("WebFetch"),
        "the error must name the entry that carries the offender: {message}"
    );
}

#[test]
#[serial]
fn test_the_self_mapping_form_of_an_adopted_spelling_is_refused() {
    // `Bash` is a ragent name after the rename, so the entry asks for a second
    // spelling of one tool - which FR-026 forbids and FR-031 refuses.
    let message = load_error_for(r#"{ "Bash": "Bash" }"#);
    assert!(
        message.contains("Bash"),
        "the error must name the spelling: {message}"
    );
    assert!(
        message.contains("itself") || message.contains("its own"),
        "the error must say the entry maps the spelling onto itself: {message}"
    );
}

#[test]
#[serial]
fn test_the_self_mapping_form_of_a_retired_spelling_is_refused() {
    let message = load_error_for(r#"{ "bash": "bash" }"#);
    assert!(
        message.contains("bash") && message.contains("Bash"),
        "a self-mapping of a retired spelling is refused and sent to `Bash`: {message}"
    );
}

#[test]
#[serial]
fn test_every_offending_entry_is_reported_in_one_error() {
    // A table copied from an old dump carries several offenders; fixing them one
    // reload at a time is the failure mode the single message exists to avoid.
    let message = load_error_for(r#"{ "bash": "Bash", "WebFetch": "grep", "Think": "think" }"#);
    assert!(
        message.contains("\"bash\"") || message.contains("`bash`"),
        "the retired key is reported: {message}"
    );
    assert!(
        message.contains("grep"),
        "the retired target is reported: {message}"
    );
    assert!(
        message.contains("Grep"),
        "the retired target's replacement is reported: {message}"
    );
}

// ---------------------------------------------------------------------------
// What is deliberately accepted
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn test_a_provider_spelling_onto_a_registered_tool_loads_cleanly() {
    let guard = ConfigFileGuard::new(r#"{ "tools": { "aliases": { "WebFetch": "webfetch" } } }"#);
    let config = Config::load().expect("a provider spelling is not a retired ragent spelling");
    assert!(
        config.tools.aliases.is_enabled(),
        "the accepted entry must survive the load"
    );
    assert_eq!(
        config.tools.aliases.entries().get("WebFetch"),
        Some(&"webfetch".to_string()),
        "the accepted entry must be the one the author wrote"
    );
    drop(guard);
}

#[test]
#[serial]
fn test_an_unregistered_alias_target_is_not_a_load_error() {
    // FR-031 is explicit: an unregistered target is not a rename problem, because
    // a plugin or MCP tool may be bridged after the config was loaded. It is
    // reported once at run time and refused when used, never refused at load.
    let guard = ConfigFileGuard::new(
        r#"{ "tools": { "aliases": { "WebFetch": "tool_from_a_plugin_bridged_later" } } }"#,
    );
    let config = Config::load().expect("an unregistered target must not stop the load");
    assert_eq!(
        config.tools.aliases.entries().get("WebFetch"),
        Some(&"tool_from_a_plugin_bridged_later".to_string()),
        "the unresolved entry is kept, so it can be reported once and refused when used"
    );
    drop(guard);
}

#[test]
#[serial]
fn test_the_self_mapping_of_an_untouched_spelling_is_accepted() {
    // `think` is not a renamed tool, so `"think": "think"` asks for no second
    // spelling of one. The alias table drops it as a no-op (D7); it is not an
    // FR-031 offender and must not stop the load.
    let guard = ConfigFileGuard::new(r#"{ "tools": { "aliases": { "think": "think" } } }"#);
    Config::load().expect("a no-op self-mapping of an untouched name is not a rename problem");
    drop(guard);
}

#[test]
#[serial]
fn test_both_boolean_forms_are_accepted() {
    for form in ["true", "false"] {
        let guard = ConfigFileGuard::new(&format!(r#"{{ "tools": {{ "aliases": {form} }} }}"#));
        Config::load().unwrap_or_else(|e| {
            panic!("the boolean form `{form}` carries no spelling and must load: {e}")
        });
        drop(guard);
    }
}

// ---------------------------------------------------------------------------
// The inline environment route is refused too
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn test_the_inline_environment_route_is_refused_and_names_itself() {
    // A configuration mechanism is an escape hatch whatever it is called, so
    // `RAGENT_CONFIG_CONTENT` is refused exactly like a file - and names itself
    // in place of the file path.
    let previous = std::env::var("RAGENT_CONFIG_CONTENT").ok();
    let previous_path = std::env::var("RAGENT_CONFIG").ok();
    unsafe {
        std::env::remove_var("RAGENT_CONFIG");
        std::env::set_var(
            "RAGENT_CONFIG_CONTENT",
            r#"{"tools": {"aliases": {"bash": "Bash"}}}"#,
        );
    }
    let result = Config::load();
    match &previous {
        Some(value) => unsafe { std::env::set_var("RAGENT_CONFIG_CONTENT", value) },
        None => unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") },
    }
    if let Some(value) = previous_path {
        unsafe { std::env::set_var("RAGENT_CONFIG", value) };
    }
    let message = result
        .expect_err("the inline route must be refused like any other configuration")
        .to_string();
    assert!(
        message.contains("RAGENT_CONFIG_CONTENT"),
        "the error must name the route it came from: {message}"
    );
    assert!(
        message.contains("bash") && message.contains("Bash"),
        "the error must name the spelling and its replacement: {message}"
    );
}

// ---------------------------------------------------------------------------
// The refusal reads the same table the rename holds
// ---------------------------------------------------------------------------

#[test]
fn test_every_pair_in_the_rename_table_is_refused_as_key_and_as_target() {
    use ragent_config::ToolsConfig;

    // The check must read the FR-023 table rather than a copy of it, or the two
    // drift apart the moment a pair is added. Every `before` is refused in both
    // positions; every `after` is refused as a self-mapping.
    for row in ragent_types::tool_rename::TOOL_RENAMES {
        let as_key: ToolsConfig = serde_json::from_str(&format!(
            r#"{{"aliases": {{"{}": "calculator"}}}}"#,
            row.before
        ))
        .expect("a retired key still parses as a shape");
        let offenders = validate(&as_key);
        assert_eq!(
            offenders.len(),
            1,
            "`{}` as a source key must be refused",
            row.before
        );
        assert_eq!(offenders[0].after, row.after);
        assert!(offenders[0].in_source_key);

        let as_target: ToolsConfig = serde_json::from_str(&format!(
            r#"{{"aliases": {{"SomeProviderSpelling": "{}"}}}}"#,
            row.before
        ))
        .expect("a retired target still parses as a shape");
        let offenders = validate(&as_target);
        assert_eq!(
            offenders.len(),
            1,
            "`{}` as a target must be refused",
            row.before
        );
        assert_eq!(offenders[0].after, row.after);
        assert!(!offenders[0].in_source_key);

        let self_mapped: ToolsConfig = serde_json::from_str(&format!(
            r#"{{"aliases": {{"{}": "{}"}}}}"#,
            row.after, row.after
        ))
        .expect("the self-mapping parses as a shape");
        let offenders = validate(&self_mapped);
        assert_eq!(
            offenders.len(),
            1,
            "`{}` mapped onto itself must be refused",
            row.after
        );
        assert!(offenders[0].self_mapping);
        assert_eq!(offenders[0].after, row.after);
    }
}

/// Runs the section's own validator, so the table-coverage case does not need a
/// config file.
fn validate(tools: &ragent_config::ToolsConfig) -> Vec<ragent_config::RetiredAliasSpelling> {
    tools.validate_alias_spellings()
}

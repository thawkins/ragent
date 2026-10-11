//! Tests for the `/plugins marketplace <store> add|list|remove` sub-subcommands
//! driven through the shared dispatch ladder (spec `pluginstores` FR-048).
//!
//! Every test points `XDG_CONFIG_HOME` at a scratch tree so the user-global
//! config write lands in the sandbox and no real config is touched.

// `std::env::set_var` is `unsafe` in edition 2024; these are single-threaded
// tests that restore the previous value.
#![allow(unsafe_code)]

mod support;

use support::TempTree;

use std::collections::{BTreeMap, BTreeSet};

use ragent_plugins::run_plugin_subcommand;

/// Serialise the tests that mutate the process-global `XDG_CONFIG_HOME`, so two
/// env-mutating tests never clobber each other's config home.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Run a `/plugins marketplace <args>` invocation through the shared dispatch
/// ladder with `XDG_CONFIG_HOME` pointed at `tree`.
fn dispatch(tree: &TempTree, args: &str) -> String {
    run_plugin_subcommand(
        tree.path(),
        "marketplace",
        args,
        BTreeSet::new(),
        BTreeSet::new(),
        &BTreeMap::new(),
    )
    .expect("`marketplace` is a management subcommand")
}

#[test]
fn marketplace_add_records_the_marketplace_in_the_global_config() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tree = TempTree::new("mp-add");
    let home = tree.path().join("xdg-config");
    let saved = std::env::var_os("XDG_CONFIG_HOME");
    // SAFETY: single-threaded via `ENV_LOCK`; restored below.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &home) };

    let added = dispatch(&tree, "claude add anthropics/claude-plugins-community");
    let listed = dispatch(&tree, "claude list");
    let codex = dispatch(&tree, "codex list");

    match saved {
        Some(prev) => unsafe { std::env::set_var("XDG_CONFIG_HOME", prev) },
        None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
    }

    assert!(added.contains("[ok] added Claude marketplace"), "{added}");
    assert!(
        added.contains("claude-plugins-community"),
        "the resolved name is reported: {added}"
    );
    // The `list` report names the added marketplace (tagged `added`).
    assert!(listed.contains("claude-plugins-community"), "{listed}");
    assert!(listed.contains("(added)"), "{listed}");
    // The Codex list is untouched by a Claude add.
    assert!(
        !codex.contains("claude-plugins-community"),
        "a Claude marketplace does not leak into Codex: {codex}"
    );

    // The write landed in the scratch global config, not the project tree.
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.join("ragent/ragent.json")).expect("config"))
            .expect("valid json");
    assert_eq!(
        config["plugins"]["stores"]["marketplaces"][0]["name"],
        "claude-plugins-community"
    );
    assert!(
        !tree.path().join(".ragent/ragent.json").exists(),
        "the add must not write into the project config"
    );
}

#[test]
fn marketplace_remove_drops_a_previously_added_marketplace() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tree = TempTree::new("mp-remove");
    let home = tree.path().join("xdg-config");
    let saved = std::env::var_os("XDG_CONFIG_HOME");
    // SAFETY: single-threaded via `ENV_LOCK`; restored below.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &home) };

    let _ = dispatch(&tree, "codex add openai/plugins");
    let removed = dispatch(&tree, "codex remove plugins");
    let list = dispatch(&tree, "codex list");

    match saved {
        Some(prev) => unsafe { std::env::set_var("XDG_CONFIG_HOME", prev) },
        None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
    }

    assert!(
        removed.contains("[ok] removed Codex marketplace"),
        "{removed}"
    );
    assert!(
        !list.contains("(added)"),
        "no added marketplace remains: {list}"
    );
}

#[test]
fn marketplace_add_without_a_source_is_a_usage_error() {
    let tree = TempTree::new("mp-usage");
    let report = dispatch(&tree, "codex add");
    assert!(report.contains("[err] Missing <source>"), "{report}");
    assert!(report.contains("Usage:"), "{report}");

    let remove = dispatch(&tree, "claude remove");
    assert!(remove.contains("[err] Missing <name>"), "{remove}");
}

#[test]
fn an_unrecognised_marketplace_sub_subcommand_renders_usage() {
    let tree = TempTree::new("mp-unknown");
    for args in ["", "bogus", "codex", "codex frobnicate"] {
        let report = dispatch(&tree, args);
        assert!(
            report.contains("command reference"),
            "`marketplace {args}` must render the usage block: {report}"
        );
        assert!(!report.contains("## Codex marketplace"), "{report}");
    }
}

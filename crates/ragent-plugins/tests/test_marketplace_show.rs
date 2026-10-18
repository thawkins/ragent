//! Tests for the `/plugins marketplace show` report: one section per store
//! (Codex, then Claude), each a markdown table with one row per configured
//! marketplace endpoint (name, url, description).
//!
//! Like the `/plugins stores` report it is a pure config read: these tests are
//! offline, exercise only the renderer and the shared dispatch ladder, and
//! install nothing.

// The shared-dispatch test points the global config directory at a fixture via
// the process environment, which `std::env::set_var` marks `unsafe` in edition
// 2024. It is a single-threaded test that restores the previous value.
#![allow(unsafe_code)]

mod support;

use support::TempTree;

use std::collections::{BTreeMap, BTreeSet};

use ragent_config::{PluginStoreEndpoint, PluginStoresConfig, PluginsConfig};
use ragent_plugins::{
    DEFAULT_CLAUDE_STORE_URL, DEFAULT_CODEX_STORE_URL, StoreKind, render_marketplace_show_report,
    run_plugin_subcommand,
};

/// Serialise the tests that mutate the process-global `XDG_CONFIG_HOME`, so the
/// shared-dispatch test here and the add/remove tests elsewhere never clobber
/// each other's config home.

/// A stores block that overrides `kind` with `url`, leaving every other field
/// at its default.
fn stores_overriding(kind: StoreKind, url: &str) -> PluginStoresConfig {
    let endpoint = Some(PluginStoreEndpoint {
        url: Some(url.to_string()),
    });
    match kind {
        StoreKind::Codex => PluginStoresConfig {
            codex: endpoint,
            ..PluginStoresConfig::default()
        },
        StoreKind::Claude => PluginStoresConfig {
            claude: endpoint,
            ..PluginStoresConfig::default()
        },
    }
}

#[test]
fn report_attributes_to_the_marketplace_show_subcommand() {
    let report = render_marketplace_show_report(&PluginStoresConfig::default());
    assert!(
        report.starts_with("From: /plugins marketplace show\n"),
        "report must carry the attribution header: {report}"
    );
}

#[test]
fn report_renders_one_section_per_store() {
    let report = render_marketplace_show_report(&PluginStoresConfig::default());
    assert!(report.contains("## Codex marketplace"), "{report}");
    assert!(report.contains("## Claude marketplace"), "{report}");
    // Codex renders before Claude (StoreKind::ALL display order).
    let codex = report.find("## Codex marketplace").expect("codex section");
    let claude = report
        .find("## Claude marketplace")
        .expect("claude section");
    assert!(codex < claude, "codex must render before claude: {report}");
}

#[test]
fn report_renders_a_table_row_per_marketplace_with_name_url_description() {
    let report = render_marketplace_show_report(&PluginStoresConfig::default());
    let header = "| Name | Url | Description |";
    assert_eq!(
        report.matches(header).count(),
        2,
        "each section carries the table header: {report}"
    );
    // Default provenance: one row per marketplace with the compiled endpoint.
    assert!(
        report.contains(&format!(
            "| codex | {DEFAULT_CODEX_STORE_URL} | Codex plugin marketplace (default) |"
        )),
        "{report}"
    );
    assert!(
        report.contains(&format!(
            "| claude | {DEFAULT_CLAUDE_STORE_URL} | Claude plugin marketplace (default) |"
        )),
        "{report}"
    );
}

#[test]
fn report_tags_a_configured_override_as_config() {
    let stores = stores_overriding(StoreKind::Codex, "https://cfg.example/codex.json");
    let report = render_marketplace_show_report(&stores);
    assert!(
        report.contains(
            "| codex | https://cfg.example/codex.json | Codex plugin marketplace (config) |"
        ),
        "{report}"
    );
    // The Claude store keeps its compiled default.
    assert!(
        report.contains(&format!(
            "| claude | {DEFAULT_CLAUDE_STORE_URL} | Claude plugin marketplace (default) |"
        )),
        "{report}"
    );
}

#[test]
fn a_refused_override_is_reported_inline_not_omitted() {
    // A non-https override fails the scheme guard; the row stays visible with
    // an `[err]` URL cell rather than dropping the section's only row.
    let stores = stores_overriding(StoreKind::Claude, "http://insecure.example/claude.json");
    let report = render_marketplace_show_report(&stores);
    assert!(report.contains("## Claude marketplace"), "{report}");
    assert!(report.contains("| claude | [err] "), "{report}");
    assert!(report.contains("(invalid) |"), "{report}");
    // Codex is untouched.
    assert!(
        report.contains(&format!(
            "| codex | {DEFAULT_CODEX_STORE_URL} | Codex plugin marketplace (default) |"
        )),
        "{report}"
    );
}

#[test]
fn report_is_pure_and_offline() {
    // The shared dispatch ladder resolves `marketplace show` from config alone
    // (no store access, no session). A global config that overrides just Codex
    // lets us assert both provenance tags end to end without writing a project
    // config.
    let _lock = crate::global_env_lock();
    let tree = TempTree::new("dispatch");
    let config_home = tree.path().join("xdg-config");
    let global_dir = config_home.join("ragent");
    std::fs::create_dir_all(&global_dir).expect("global config dir creatable");
    std::fs::write(
        global_dir.join("ragent.json"),
        r#"{ "plugins": { "stores": { "codex": { "url": "https://cfg.example/codex.json" } } } }"#,
    )
    .expect("global config writable");

    let saved_xdg = std::env::var_os("XDG_CONFIG_HOME");
    // SAFETY: single-threaded test; the value is restored below.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &config_home) };

    let report = run_plugin_subcommand(
        tree.path(),
        "marketplace",
        "show",
        BTreeSet::new(),
        BTreeSet::new(),
        &BTreeMap::new(),
    )
    .expect("`marketplace show` is handled by the shared dispatch ladder");

    match saved_xdg {
        Some(prev) => unsafe { std::env::set_var("XDG_CONFIG_HOME", prev) },
        None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
    }

    assert!(
        report.starts_with("From: /plugins marketplace show\n"),
        "dispatch must render the marketplace report: {report}"
    );
    assert!(report.contains("## Codex marketplace"), "{report}");
    assert!(report.contains("## Claude marketplace"), "{report}");
    assert!(
        report.contains(
            "| codex | https://cfg.example/codex.json | Codex plugin marketplace (config) |"
        ),
        "the configured store is tagged config: {report}"
    );
    assert!(
        report.contains(&format!(
            "| claude | {DEFAULT_CLAUDE_STORE_URL} | Claude plugin marketplace (default) |"
        )),
        "the unconfigured store falls back to its compiled default: {report}"
    );
    assert!(report.is_ascii(), "the report must be ASCII only: {report}");
    assert!(
        !tree.path().join(".ragent").join("plugins").exists(),
        "the report must not create the store directory"
    );
}

#[test]
fn the_read_only_report_ignores_the_master_switch() {
    // Informational reports are not gated on `plugins.enabled`
    // (`plugins.enabled: false` makes execution inert, not config inspection).
    let disabled = PluginsConfig {
        enabled: false,
        ..PluginsConfig::default()
    };
    assert!(!disabled.is_enabled());
    let report = render_marketplace_show_report(&disabled.stores_or_default());
    assert!(report.contains(DEFAULT_CODEX_STORE_URL), "{report}");
    assert!(report.contains("(default)"), "{report}");
}

#[test]
fn a_bare_or_unknown_marketplace_invocation_falls_through_to_usage() {
    let tree = TempTree::new("usage");
    for rest in ["", "bogus"] {
        let report = run_plugin_subcommand(
            tree.path(),
            "marketplace",
            rest,
            BTreeSet::new(),
            BTreeSet::new(),
            &BTreeMap::new(),
        )
        .expect("`marketplace` is a management subcommand");
        assert!(
            report.contains("command reference"),
            "`marketplace {rest}` must render the usage block: {report}"
        );
        assert!(
            !report.contains("## Codex marketplace"),
            "`marketplace {rest}` must not render the show report: {report}"
        );
    }
}

//! Tests for the `/plugins stores` report (spec `pluginstores` T-017; FR-031,
//! FR-038).
//!
//! The report lists, for **both** stores, the effective store-index endpoint and
//! tags it `config` (a non-empty `plugins.stores.<name>.url` override) or
//! `default` (the compiled default). These tests are pure and offline: they
//! exercise the report renderer and the provenance resolver directly, never the
//! network, and install nothing.

// The shared-dispatch test points the global config directory at a fixture via
// the process environment, which `std::env::set_var` marks `unsafe` in edition
// 2024. It is a single-threaded test that restores the previous value.
#![allow(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use ragent_config::{PluginStoreEndpoint, PluginStoresConfig, PluginsConfig};
use ragent_plugins::{
    DEFAULT_CLAUDE_STORE_URL, DEFAULT_CODEX_STORE_URL, EndpointSource, StoreKind,
    render_stores_report, run_plugin_subcommand,
};

/// A stores block that overrides `kind` with `url`, leaving every other field at
/// its default.
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

/// RAII temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(std::path::PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = std::process::id();
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/stores-report-{name}-{unique}"
        ));
        std::fs::create_dir_all(&path).expect("temp tree creatable");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ── Provenance resolution (FR-031) ──────────────────────────────────────────

#[test]
fn effective_endpoint_with_source_tags_an_override_as_config() {
    for kind in StoreKind::ALL {
        let url = format!("https://cfg.example/{}/index.json", kind.token());
        let stores = stores_overriding(kind, &url);

        let (endpoint, source) = kind
            .effective_endpoint_with_source(&stores)
            .expect("a configured https override is accepted");
        assert_eq!(source, EndpointSource::Config, "{kind} override source");
        assert_eq!(source.tag(), "config");
        assert_eq!(endpoint.as_str(), url);
    }
}

#[test]
fn effective_endpoint_with_source_tags_the_fallback_as_default() {
    let stores = PluginStoresConfig::default();
    for kind in StoreKind::ALL {
        let (endpoint, source) = kind
            .effective_endpoint_with_source(&stores)
            .expect("the compiled default is accepted");
        assert_eq!(source, EndpointSource::Default, "{kind} fallback source");
        assert_eq!(source.tag(), "default");
        assert_eq!(endpoint.as_str(), kind.default_url());
    }
}

// ── Report rendering (FR-031, FR-038) ───────────────────────────────────────

#[test]
fn report_attributes_to_the_stores_subcommand() {
    let report = render_stores_report(&PluginStoresConfig::default());
    assert!(
        report.starts_with("From: /plugins stores\n"),
        "report must carry the attribution header: {report}"
    );
}

#[test]
fn report_lists_both_stores_even_when_only_one_is_overridden() {
    // Only Codex is overridden (TC-015 precondition).
    let stores = stores_overriding(StoreKind::Codex, "https://cfg.example/codex.json");
    let report = render_stores_report(&stores);

    assert!(report.contains("- codex: [config]"), "{report}");
    assert!(report.contains("- claude: [default]"), "{report}");
}

#[test]
fn report_tags_each_store_with_its_provenance_and_endpoint() {
    let codex_url = "https://cfg.example/codex.json";
    let stores = stores_overriding(StoreKind::Codex, codex_url);
    let report = render_stores_report(&stores);

    // The overridden store names its configured URL and is tagged `config`.
    assert!(
        report.contains(&format!("- codex: [config] {codex_url}")),
        "codex row must name the configured URL: {report}"
    );
    // The other store names its compiled default and is tagged `default`.
    assert!(
        report.contains(&format!("- claude: [default] {DEFAULT_CLAUDE_STORE_URL}")),
        "claude row must name the compiled default: {report}"
    );
    // The default is not merged into the override.
    assert!(
        !report.contains(&format!("- codex: [config] {DEFAULT_CODEX_STORE_URL}")),
        "the default must not replace the configured URL: {report}"
    );
}

#[test]
fn report_tags_both_stores_as_default_when_no_stores_block_is_present() {
    // FR-038: with no `plugins.stores` block, both stores are tagged `default`
    // and still report a non-empty https endpoint.
    let report = render_stores_report(&PluginStoresConfig::default());

    for kind in StoreKind::ALL {
        assert!(
            report.contains(&format!(
                "- {}: [default] {}",
                kind.token(),
                kind.default_url()
            )),
            "{kind} must be tagged default with its compiled URL: {report}"
        );
        assert!(
            !report.contains(&format!("- {}: [config]", kind.token())),
            "{kind} must not be tagged config without an override: {report}"
        );
    }
    assert!(report.contains(DEFAULT_CODEX_STORE_URL), "{report}");
    assert!(report.contains(DEFAULT_CLAUDE_STORE_URL), "{report}");
    assert!(report.contains("https://"), "{report}");
}

#[test]
fn report_is_ascii_only() {
    let overridden = stores_overriding(StoreKind::Claude, "https://cfg.example/claude.json");
    assert!(render_stores_report(&PluginStoresConfig::default()).is_ascii());
    assert!(render_stores_report(&overridden).is_ascii());
}

#[test]
fn a_refused_override_is_reported_inline_not_omitted() {
    // A non-https override is refused by the endpoint guard (FR-024); the store
    // must still appear, flagged with `[err]`, rather than silently vanish.
    let stores = stores_overriding(StoreKind::Codex, "http://cfg.example/codex.json");
    let report = render_stores_report(&stores);

    assert!(
        report.contains("- codex: [err]"),
        "the refused store must be reported: {report}"
    );
    assert!(
        report.contains("https"),
        "the refusal names the https requirement: {report}"
    );
    // The other store still renders normally.
    assert!(report.contains("- claude: [default]"), "{report}");
}

// ── Shared dispatch ladder ──────────────────────────────────────────────────

#[test]
fn run_plugin_subcommand_handles_stores_without_a_session() {
    // The shared ladder used by the TUI and CLI resolves `stores` from config
    // alone (no store access, no session) and returns a report. A global config
    // that overrides just Codex lets us assert both provenance tags end to end
    // without writing a project config.
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
        "stores",
        "",
        BTreeSet::new(),
        BTreeSet::new(),
        &BTreeMap::new(),
    )
    .expect("stores must be handled by the shared dispatch ladder");

    match saved_xdg {
        Some(prev) => unsafe { std::env::set_var("XDG_CONFIG_HOME", prev) },
        None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
    }

    assert!(
        report.starts_with("From: /plugins stores\n"),
        "dispatch must render the stores report: {report}"
    );
    assert!(
        report.contains("- codex: [config] https://cfg.example/codex.json"),
        "the configured store is tagged config: {report}"
    );
    assert!(
        report.contains(&format!("- claude: [default] {DEFAULT_CLAUDE_STORE_URL}")),
        "the unconfigured store falls back to its compiled default: {report}"
    );
}

#[test]
fn the_read_only_report_ignores_the_master_switch() {
    // `/plugins stores` is informational, so a disabled subsystem does not block
    // it: the report still renders. (The panel launch is guarded by the master
    // switch elsewhere, but the config read is not.)
    let disabled = PluginsConfig {
        enabled: false,
        ..PluginsConfig::default()
    };
    assert!(!disabled.is_enabled());
    let report = render_stores_report(&disabled.stores_or_default());
    assert!(report.contains("[default]"), "{report}");
}

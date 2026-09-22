//! Endpoint-resolution precedence and no-config launch tests (spec
//! `pluginstores` T-018; FR-027, FR-028, FR-029, FR-030, NFR-001, NFR-002).
//!
//! These tests pin down, for **both** stores, how the effective store-index
//! endpoint is chosen:
//!
//! - a non-empty `plugins.stores.<name>.url` wins wholesale over the compiled
//!   default (FR-028);
//! - an absent `plugins.stores` block, an absent per-store key, or an
//!   empty/whitespace-only override yields the compiled default (FR-027);
//! - the default path is run through the same `https` scheme guard as the
//!   configured path (FR-029);
//! - the launch path resolves each store from a config with no `plugins.stores`
//!   block to its compiled default without error (FR-030, NFR-002);
//! - the two default literals appear in exactly one production source file
//!   (NFR-001).
//!
//! Everything here is deterministic and offline: it exercises the pure resolver
//! (`StoreKind::effective_endpoint`), never the network, and installs nothing.

use std::path::{Path, PathBuf};

use ragent_config::{PluginStoreEndpoint, PluginStoresConfig, PluginsConfig};
use ragent_plugins::{
    DEFAULT_CLAUDE_STORE_URL, DEFAULT_CODEX_STORE_URL, StoreEndpoint, StoreEndpointError, StoreKind,
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

/// A stores block that declares `kind`'s key but sets no `url` on it.
fn stores_with_empty_endpoint_for(kind: StoreKind) -> PluginStoresConfig {
    let endpoint = Some(PluginStoreEndpoint { url: None });
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

// ── Precedence: override wins, wholesale (FR-028) ───────────────────────────

#[test]
fn configured_url_overrides_the_default_for_either_store() {
    for kind in StoreKind::ALL {
        let url = format!("https://cfg.example/{}/index.json", kind.token());
        let stores = stores_overriding(kind, &url);

        let effective = kind
            .effective_endpoint(&stores)
            .expect("a configured https override is accepted");
        assert_eq!(
            effective.as_str(),
            url,
            "{kind} must use its configured URL"
        );
        assert_ne!(
            effective.as_str(),
            kind.default_url(),
            "{kind} must not fall back to the default when a URL is configured"
        );
    }
}

#[test]
fn a_configured_override_is_used_wholesale_without_merging_the_default() {
    for kind in StoreKind::ALL {
        // A value on an unrelated host: if resolution appended or merged the
        // default, the result would name the default host (FR-028).
        let url = format!("https://override.invalid/{}/custom.json", kind.token());
        let stores = stores_overriding(kind, &url);

        let effective = kind
            .effective_endpoint(&stores)
            .expect("configured override is accepted");
        assert_eq!(effective.as_str(), url);
        assert!(
            !effective.as_str().contains("raw.githubusercontent.com"),
            "{kind} must use the override verbatim, not the default: {effective}"
        );
    }
}

// ── Precedence: default on absence (FR-027, FR-030) ─────────────────────────

#[test]
fn absent_stores_block_yields_each_compiled_default() {
    // `PluginStoresConfig::default()` is what `stores_or_default()` produces
    // when the `plugins.stores` key is absent entirely (FR-030).
    let stores = PluginStoresConfig::default();
    for kind in StoreKind::ALL {
        let effective = kind
            .effective_endpoint(&stores)
            .expect("the default path must never error");
        assert_eq!(
            effective.as_str(),
            kind.default_url(),
            "{kind} must resolve to its compiled default when no block is present"
        );
    }
}

#[test]
fn a_store_key_without_a_url_yields_each_compiled_default() {
    for kind in StoreKind::ALL {
        let stores = stores_with_empty_endpoint_for(kind);
        let effective = kind
            .effective_endpoint(&stores)
            .expect("an absent per-store url must not error");
        assert_eq!(
            effective.as_str(),
            kind.default_url(),
            "{kind} must fall back to the default when its key carries no url"
        );
    }
}

#[test]
fn empty_and_whitespace_overrides_are_ignored_for_either_store() {
    for kind in StoreKind::ALL {
        for raw in ["", " ", "   ", "\t\n"] {
            let stores = stores_overriding(kind, raw);
            let effective = kind
                .effective_endpoint(&stores)
                .expect("an empty override must fall back, not error");
            assert_eq!(
                effective.as_str(),
                kind.default_url(),
                "{kind} override {raw:?} must be ignored in favour of the default"
            );
        }
    }
}

// ── The default path is guarded like the configured path (FR-029) ────────────

#[test]
fn compiled_defaults_pass_the_scheme_guard_on_the_default_path() {
    let stores = PluginStoresConfig::default();
    for kind in StoreKind::ALL {
        let default = kind.default_url();
        assert!(
            default.starts_with("https://"),
            "{kind} compiled default must be https: {default}"
        );
        // The same `StoreEndpoint` guard the configured path uses (FR-024/FR-029).
        let parsed = StoreEndpoint::parse(default)
            .unwrap_or_else(|e| panic!("{kind} default must pass the guard: {e}"));
        assert_eq!(parsed.as_str(), default);

        let effective = kind
            .effective_endpoint(&stores)
            .expect("the default path must pass the guard, not error");
        assert_eq!(effective.as_str(), default);
        assert_eq!(effective.as_url().scheme(), "https");
    }
}

#[test]
fn a_refused_override_is_refused_for_either_store() {
    // FR-028/FR-029: a configured non-https value is refused rather than used,
    // so the configured path is not a bypass around the scheme guard.
    for kind in StoreKind::ALL {
        let stores = stores_overriding(kind, "http://cfg.example/index.json");
        assert!(
            matches!(
                kind.effective_endpoint(&stores),
                Err(StoreEndpointError::NotHttps { .. })
            ),
            "{kind} must refuse a non-https override"
        );
    }
}

// ── No-config launch path (FR-030) and fresh resolution (NFR-002) ────────────

#[test]
fn no_config_launch_resolves_both_defaults_without_error() {
    // The launch path (`spawn_plugin_store_fetch`) resolves the endpoint from
    // `store_and_config(..).1.stores_or_default()`. A config with no `plugins`
    // key at all yields `PluginsConfig::default()`, so this is exactly what the
    // no-config launch hands the resolver (FR-030, NFR-002).
    let resolved = PluginsConfig::default().stores_or_default();
    assert_eq!(resolved, PluginStoresConfig::default());

    for kind in StoreKind::ALL {
        let endpoint = kind.effective_endpoint(&resolved).unwrap_or_else(|e| {
            panic!(
                "`/plugins {}` launch must resolve without a config error: {e}",
                kind.token()
            )
        });
        assert_eq!(endpoint.as_str(), kind.default_url());
    }
}

#[test]
fn resolution_reads_the_config_afresh_on_every_call() {
    // NFR-002: resolution reads the supplied config each call, so adding or
    // removing an override takes effect without a rebuild.
    for kind in StoreKind::ALL {
        let absent = PluginStoresConfig::default();
        assert_eq!(
            kind.effective_endpoint(&absent)
                .expect("default resolves")
                .as_str(),
            kind.default_url()
        );

        let url = format!("https://late.example/{}/index.json", kind.token());
        let present = stores_overriding(kind, &url);
        assert_eq!(
            kind.effective_endpoint(&present)
                .expect("override resolves")
                .as_str(),
            url
        );

        // And back again: removing the override restores the default.
        assert_eq!(
            kind.effective_endpoint(&absent)
                .expect("default resolves again")
                .as_str(),
            kind.default_url()
        );
    }
}

// ── NFR-001: the default literals live in exactly one source file ────────────

/// Recursively collect every `.rs` file under `root`.
fn rust_sources_under(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources_under(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn the_default_endpoint_literal_appears_only_in_the_store_registry() {
    // NFR-001: the compiled default URLs are declared once in
    // `ragent_plugins::store_index` and are the only place a default-endpoint
    // literal appears in production source.
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    let mut sources = Vec::new();
    for crate_entry in std::fs::read_dir(workspace.join("crates")).expect("crates dir") {
        let src = crate_entry.expect("entry").path().join("src");
        if src.is_dir() {
            rust_sources_under(&src, &mut sources);
        }
    }
    let root_src = workspace.join("src");
    if root_src.is_dir() {
        rust_sources_under(&root_src, &mut sources);
    }
    assert!(
        sources.len() > 100,
        "the workspace source scan must find the crate sources, found {}",
        sources.len()
    );

    let mut hits: Vec<PathBuf> = Vec::new();
    for path in &sources {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        if text.contains(DEFAULT_CODEX_STORE_URL) || text.contains(DEFAULT_CLAUDE_STORE_URL) {
            hits.push(path.clone());
        }
    }

    assert_eq!(
        hits.len(),
        1,
        "NFR-001: the default-endpoint literals must appear in exactly one source file, found: {hits:?}"
    );
    let registry = &hits[0];
    assert!(
        registry.ends_with("crates/ragent-plugins/src/store_index.rs"),
        "the single default-endpoint source must be the store registry, got {registry:?}"
    );

    let text = std::fs::read_to_string(registry).expect("registry readable");
    assert!(
        text.contains("openai/plugins"),
        "the registry declares the Codex official marketplace URL"
    );
    assert!(
        text.contains("anthropics/claude-plugins-official"),
        "the registry declares the Claude official marketplace URL"
    );
    // Both constants are the compiled defaults this suite resolves against.
    assert_eq!(DEFAULT_CODEX_STORE_URL, StoreKind::Codex.default_url());
    assert_eq!(DEFAULT_CLAUDE_STORE_URL, StoreKind::Claude.default_url());
}

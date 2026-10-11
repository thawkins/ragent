//! Tests for the `/connectors marketplace <store> add|remove` source resolver
//! and the durable marketplace ledger (spec `connectors` FR-043).
//!
//! The resolver is pure and offline; the ledger writes go to a scratch
//! `XDG_CONFIG_HOME` so no real user config is touched.

// `std::env::set_var` is `unsafe` in edition 2024; these are single-threaded
// tests that restore the previous value.
#![allow(unsafe_code)]

mod support;

use support::TempTree;

use ragent_connectors::marketplace_ops::{
    MarketplaceSourceError, add_marketplace, remove_marketplace, resolve_source,
};

// -- Source resolution (pure) -------------------------------------------------

#[test]
fn owner_repo_shorthand_resolves_to_the_claude_marketplace_document() {
    let cwd = std::path::Path::new(".");
    let resolved = resolve_source("anthropics/claude-connectors-community", cwd).expect("slug");
    assert_eq!(resolved.name, "claude-connectors-community");
    assert_eq!(
        resolved.url.as_deref(),
        Some(
            "https://raw.githubusercontent.com/anthropics/claude-connectors-community/main/.claude-plugin/marketplace.json"
        )
    );
}

#[test]
fn ssh_remote_resolves_like_the_shorthand() {
    let resolved =
        resolve_source("git@github.com:owner/repo.git", std::path::Path::new(".")).expect("ssh");
    assert_eq!(resolved.name, "repo");
    assert!(
        resolved
            .url
            .expect("url present")
            .ends_with("/.claude-plugin/marketplace.json")
    );
}

#[test]
fn github_repo_url_resolves_to_the_raw_document() {
    let resolved = resolve_source(
        "https://github.com/acme/connectors.git",
        std::path::Path::new("."),
    )
    .expect("github url");
    assert_eq!(resolved.name, "connectors");
    assert_eq!(
        resolved.url.as_deref(),
        Some(
            "https://raw.githubusercontent.com/acme/connectors/main/.claude-plugin/marketplace.json"
        )
    );
}

#[test]
fn https_url_is_used_verbatim() {
    let url = "https://example.org/custom/marketplace.json";
    let resolved = resolve_source(url, std::path::Path::new(".")).expect("url source");
    assert_eq!(resolved.url.as_deref(), Some(url));
}

#[test]
fn a_local_directory_reads_its_manifest_name() {
    let tree = TempTree::new("mp-local");
    let dir = tree.path().join("my-marketplace");
    std::fs::create_dir_all(dir.join(".claude-plugin")).expect("dir creatable");
    std::fs::write(
        dir.join(".claude-plugin/marketplace.json"),
        r#"{ "name": "My Community", "plugins": [] }"#,
    )
    .expect("manifest writable");

    let resolved = resolve_source(dir.to_str().expect("utf-8"), tree.path()).expect("local");
    assert_eq!(resolved.name, "My Community");
    assert!(resolved.url.is_none(), "a local marketplace is not fetched");
}

#[test]
fn an_empty_source_is_refused() {
    let err = resolve_source("   ", std::path::Path::new(".")).unwrap_err();
    assert_eq!(err, MarketplaceSourceError::Empty);
}

#[test]
fn an_unrecognised_source_is_refused() {
    let err = resolve_source("not a source", std::path::Path::new("."));
    assert!(matches!(err, Err(MarketplaceSourceError::Unsupported(_))));
}

// -- Ledger mutate (writes a scratch global config) ---------------------------

/// Point `XDG_CONFIG_HOME` at `dir` for the duration of `body` and restore it.
fn with_config_home<T>(dir: &std::path::Path, body: impl FnOnce() -> T) -> T {
    let saved = std::env::var_os("XDG_CONFIG_HOME");
    // SAFETY: single-threaded test; the value is restored below.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", dir) };
    let out = body();
    match saved {
        Some(prev) => unsafe { std::env::set_var("XDG_CONFIG_HOME", prev) },
        None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
    }
    out
}

#[test]
fn add_then_remove_round_trips_through_the_global_config() {
    let tree = TempTree::new("mp-ledger");
    let home = tree.path().join("xdg-config");

    with_config_home(&home, || {
        let added = add_marketplace(
            "claude",
            "claude-connectors-community",
            Some("https://raw.githubusercontent.com/anthropics/claude-connectors-community/main/.claude-plugin/marketplace.json"),
            "anthropics/claude-connectors-community",
            false,
        )
        .expect("add succeeds");
        assert!(added, "the first add writes a new entry");

        // A duplicate without `--force` is refused and changes nothing.
        let dup = add_marketplace(
            "claude",
            "claude-connectors-community",
            Some("https://raw.githubusercontent.com/anthropics/claude-connectors-community/main/.claude-plugin/marketplace.json"),
            "anthropics/claude-connectors-community",
            false,
        )
        .expect("duplicate add is a contained refusal");
        assert!(!dup, "a duplicate is refused without --force");

        let config: serde_json::Value = serde_json::from_slice(
            &std::fs::read(home.join("ragent/ragent.json")).expect("config written"),
        )
        .expect("config is valid json");
        let entries = config["connectors"]["stores"]["marketplaces"]
            .as_array()
            .expect("marketplaces array");
        assert_eq!(entries.len(), 1, "the duplicate did not append");
        assert_eq!(entries[0]["name"], "claude-connectors-community");

        // `--force` replaces in place without growing the array.
        let forced = add_marketplace(
            "claude",
            "claude-connectors-community",
            Some("https://example.org/other.json"),
            "owner/other",
            true,
        )
        .expect("force add succeeds");
        assert!(forced);
        let config: serde_json::Value = serde_json::from_slice(
            &std::fs::read(home.join("ragent/ragent.json")).expect("config re-read"),
        )
        .expect("config is valid json");
        let entries = config["connectors"]["stores"]["marketplaces"]
            .as_array()
            .expect("marketplaces array");
        assert_eq!(entries.len(), 1, "force replaced, not appended");
        assert_eq!(entries[0]["url"], "https://example.org/other.json");

        // Removing it drops the entry.
        let removed =
            remove_marketplace("claude", "claude-connectors-community").expect("remove succeeds");
        assert!(removed, "the existing entry is removed");

        // Removing a missing entry is a contained no-op.
        let again = remove_marketplace("claude", "claude-connectors-community").expect("remove ok");
        assert!(!again, "a missing entry reports not-removed");
    });
}

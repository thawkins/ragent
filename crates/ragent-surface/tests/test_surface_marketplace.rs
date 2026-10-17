//! Coverage for `ragent-surface::marketplace` source resolution, which the
//! `/plugins` and `/connectors` `marketplace add` families delegate to.
//!
//! `resolve_source` is pure over its inputs (no network), so each accepted
//! source form and each rejection is asserted directly, including the local
//! directory manifest name and the URL-derived name fallbacks.

use std::fs;
use std::path::Path;

use ragent_surface::marketplace::{MarketplaceSource, MarketplaceSourceError, resolve_source};

fn workdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("ragent-surface-mp-{tag}-"))
        .tempdir()
        .expect("tempdir")
}

#[test]
fn resolve_source_rejects_an_empty_or_whitespace_source() {
    let cwd = Path::new("/tmp/work");
    assert_eq!(
        resolve_source("", cwd, ".claude-plugin/marketplace.json"),
        Err(MarketplaceSourceError::Empty)
    );
    assert_eq!(
        resolve_source("   ", cwd, ".claude-plugin/marketplace.json"),
        Err(MarketplaceSourceError::Empty)
    );
}

#[test]
fn resolve_source_accepts_bare_owner_repo_shorthand() {
    let cwd = Path::new("/tmp/work");
    let resolved =
        resolve_source("acme/plugins", cwd, ".claude-plugin/marketplace.json").expect("resolves");
    assert_eq!(
        resolved,
        MarketplaceSource {
            name: "plugins".to_string(),
            url: Some(
                "https://raw.githubusercontent.com/acme/plugins/main/.claude-plugin/marketplace.json"
                    .to_string()
            ),
        }
    );
}

#[test]
fn resolve_source_accepts_https_and_ssh_github_forms() {
    let cwd = Path::new("/tmp/work");
    let doc = ".codex-plugin/marketplace.json";

    // https GitHub URL with a trailing .git and a /tree/main suffix.
    let https = resolve_source("https://github.com/acme/tools.git", cwd, doc).expect("resolves");
    assert_eq!(https.name, "tools");
    assert_eq!(
        https.url.as_deref(),
        Some("https://raw.githubusercontent.com/acme/tools/main/.codex-plugin/marketplace.json")
    );

    let http_host = resolve_source("http://github.com/acme/tools", cwd, doc).expect("resolves");
    assert_eq!(http_host.name, "tools");

    let ssh = resolve_source("git@github.com:acme/widgets.git", cwd, doc).expect("resolves");
    assert_eq!(ssh.name, "widgets");
    assert_eq!(
        ssh.url.as_deref(),
        Some("https://raw.githubusercontent.com/acme/widgets/main/.codex-plugin/marketplace.json")
    );
}

#[test]
fn resolve_source_keeps_an_arbitrary_https_url_as_a_fetchable_index() {
    let cwd = Path::new("/tmp/work");
    let resolved = resolve_source(
        "https://market.example.com/catalog/index.json",
        cwd,
        ".claude-plugin/marketplace.json",
    )
    .expect("resolves");
    assert_eq!(resolved.name, "catalog");
    assert_eq!(
        resolved.url.as_deref(),
        Some("https://market.example.com/catalog/index.json")
    );
}

#[test]
fn resolve_source_names_a_raw_githubusercontent_url_after_the_repo() {
    let cwd = Path::new("/tmp/work");
    let resolved = resolve_source(
        "https://raw.githubusercontent.com/acme/plugins/main/.claude-plugin/marketplace.json",
        cwd,
        ".claude-plugin/marketplace.json",
    )
    .expect("resolves");
    assert_eq!(resolved.name, "plugins");
}

#[test]
fn resolve_source_errors_on_an_unrecognised_source() {
    let cwd = Path::new("/tmp/work");
    let err = resolve_source("just-a-word", cwd, ".claude-plugin/marketplace.json")
        .expect_err("rejected");
    assert_eq!(
        err,
        MarketplaceSourceError::Unsupported("just-a-word".to_string())
    );
}

#[test]
fn resolve_source_reads_a_local_directory_manifest_name() {
    let tmp = workdir("local");
    let marketplace = tmp.path().join("market");
    fs::create_dir_all(marketplace.join(".claude-plugin")).expect("mkdir");
    fs::write(
        marketplace.join(".claude-plugin/marketplace.json"),
        br#"{"name":"My Local Market"}"#,
    )
    .expect("write manifest");

    let resolved = resolve_source(
        marketplace.to_str().expect("utf8 path"),
        tmp.path(),
        ".claude-plugin/marketplace.json",
    )
    .expect("resolves");
    assert_eq!(resolved.name, "My Local Market");
    assert_eq!(resolved.url, None);
}

#[test]
fn resolve_source_falls_back_to_the_directory_name_when_no_manifest_name() {
    let tmp = workdir("fallback");
    let marketplace = tmp.path().join("bare-market");
    fs::create_dir_all(&marketplace).expect("mkdir");
    // No manifest present: the directory name is used.
    let resolved = resolve_source(
        marketplace.to_str().expect("utf8 path"),
        tmp.path(),
        ".claude-plugin/marketplace.json",
    )
    .expect("resolves");
    assert_eq!(resolved.name, "bare-market");
    assert_eq!(resolved.url, None);
}

#[test]
fn resolve_source_uses_marketplace_json_when_the_doc_path_is_absent() {
    let tmp = workdir("legacy");
    let marketplace = tmp.path().join("legacy-market");
    fs::create_dir_all(&marketplace).expect("mkdir");
    // Only the conventional marketplace.json exists, not the doc-path file.
    fs::write(
        marketplace.join("marketplace.json"),
        br#"{"name":"Legacy Name"}"#,
    )
    .expect("write manifest");
    let resolved = resolve_source(
        marketplace.to_str().expect("utf8 path"),
        tmp.path(),
        ".claude-plugin/marketplace.json",
    )
    .expect("resolves");
    assert_eq!(resolved.name, "Legacy Name");
}

#[test]
fn resolve_source_resolves_a_relative_local_path_against_the_cwd() {
    let tmp = workdir("relative");
    let marketplace = tmp.path().join("./rel-market");
    fs::create_dir_all(&marketplace).expect("mkdir");
    let resolved = resolve_source(
        "./rel-market",
        tmp.path(),
        ".claude-plugin/marketplace.json",
    )
    .expect("resolves");
    assert_eq!(resolved.name, "rel-market");
    assert_eq!(resolved.url, None);
}

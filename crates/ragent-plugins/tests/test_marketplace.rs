//! Tests for the marketplace-inline manifest bridge ([`crate::marketplace`]):
//! the registry, the `#<key>` source suffix, and the manifest materialisation
//! that turns a manifest-less marketplace stub (the Claude store's `*-lsp`
//! entries) into an installable plugin (spec `plugins` follow-up; FR-002).

use ragent_plugins::marketplace::{
    document_key, inline_manifest_for, lookup, materialize_at, record_document, split_manifest_key,
};

/// A marketplace document carrying one manifest-less LSP stub and one plain
/// listing entry.
const DOC: &str = r#"{
  "name": "claude-plugins-official",
  "plugins": [
    {
      "name": "rust-analyzer-lsp",
      "description": "Rust language server for code intelligence and analysis",
      "version": "1.0.0",
      "author": { "name": "Anthropic" },
      "source": "./plugins/rust-analyzer-lsp",
      "category": "development",
      "strict": false,
      "lspServers": {
        "rust-analyzer": {
          "command": "rust-analyzer",
          "extensionToLanguage": { ".rs": "rust" }
        }
      }
    },
    {
      "name": "agent-sdk-dev",
      "description": "Development kit for the Claude Agent SDK",
      "source": "./plugins/agent-sdk-dev",
      "category": "development"
    }
  ]
}"#;

#[test]
fn document_key_is_stable_and_origin_bound() {
    let origin = "https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json";
    assert_eq!(
        document_key(origin, DOC.as_bytes()),
        document_key(origin, DOC.as_bytes())
    );
    assert_ne!(
        document_key(origin, DOC.as_bytes()),
        document_key("https://example.com/other.json", DOC.as_bytes())
    );
    assert_ne!(
        document_key(origin, DOC.as_bytes()),
        document_key(origin, b"{}"),
    );
    assert_eq!(document_key(origin, DOC.as_bytes()).len(), 64);
}

#[test]
fn inline_manifest_strips_listing_fields_and_keeps_content() {
    let manifest = inline_manifest_for(DOC, "rust-analyzer-lsp").expect("stub has inline content");
    let value: serde_json::Value = serde_json::from_str(&manifest).expect("valid json");
    // Listing-only fields are stripped...
    for field in ["source", "category", "author", "strict"] {
        assert!(value.get(field).is_none(), "{field} must be stripped");
    }
    // ...manifest content survives.
    assert_eq!(value["name"], "rust-analyzer-lsp");
    assert_eq!(value["version"], "1.0.0");
    assert!(value.get("lspServers").is_some());
    assert!(value.get("description").is_some());
}

#[test]
fn inline_manifest_refuses_plain_listings_and_unknown_names() {
    // A plain listing entry (no hosting-config section) never materialises -
    // the real manifest lives in the clone.
    assert!(inline_manifest_for(DOC, "agent-sdk-dev").is_none());
    assert!(inline_manifest_for(DOC, "not-in-the-doc").is_none());
    assert!(inline_manifest_for("not json", "rust-analyzer-lsp").is_none());
    assert!(inline_manifest_for(r#"{"other": []}"#, "rust-analyzer-lsp").is_none());
}

#[test]
fn record_then_lookup_roundtrips_the_inline_manifest() {
    let origin = "https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json";
    record_document(origin, DOC.as_bytes());
    let key = document_key(origin, DOC.as_bytes());
    let manifest = lookup(&key, "rust-analyzer-lsp").expect("recorded doc must answer");
    assert!(manifest.contains("lspServers"));
    // An unknown key answers nothing.
    assert!(lookup(&"0".repeat(64), "rust-analyzer-lsp").is_none());
}

#[test]
fn split_manifest_key_only_splits_a_sha256_suffix() {
    let key = "a".repeat(64);
    let source = format!("git+https://github.com/o/r#main:plugins/x@{key}");
    assert_eq!(
        split_manifest_key(&source),
        (
            "git+https://github.com/o/r#main:plugins/x",
            Some(key.as_str())
        )
    );
    // No suffix, short suffix, and non-hex suffix are not keys.
    assert_eq!(
        split_manifest_key("git+https://github.com/o/r#main"),
        ("git+https://github.com/o/r#main", None)
    );
    assert_eq!(
        split_manifest_key("https://user@host/x.zip"),
        ("https://user@host/x.zip", None)
    );
    let not_hex = format!("git+x@{}", "z".repeat(64));
    assert_eq!(split_manifest_key(&not_hex).1, None);
}

#[test]
fn materialize_writes_the_manifest_for_a_manifest_less_stub() {
    let origin = "https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json";
    record_document(origin, DOC.as_bytes());
    let key = document_key(origin, DOC.as_bytes());

    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_path_buf();
    // The manifest-less stub layout: README + LICENSE, no manifest.
    std::fs::write(root.join("README.md"), "# stub").unwrap();

    let returned = materialize_at(root.clone(), "rust-analyzer-lsp", Some(&key));
    assert_eq!(returned, root);
    let written = root.join(".claude-plugin/plugin.json");
    assert!(written.is_file(), "manifest materialised");
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&written).unwrap()).expect("valid json");
    assert_eq!(value["name"], "rust-analyzer-lsp");
    assert!(value.get("lspServers").is_some());
}

#[test]
fn materialize_leaves_a_real_manifest_tree_untouched() {
    let origin = "https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json";
    record_document(origin, DOC.as_bytes());
    let key = document_key(origin, DOC.as_bytes());

    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_path_buf();
    let dir = root.join(".claude-plugin");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("plugin.json"),
        r#"{"name": "rust-analyzer-lsp", "version": "9.9.9"}"#,
    )
    .unwrap();

    let returned = materialize_at(root.clone(), "rust-analyzer-lsp", Some(&key));
    assert_eq!(returned, root);
    let body = std::fs::read_to_string(dir.join("plugin.json")).unwrap();
    assert!(body.contains("9.9.9"), "upstream manifest must win: {body}");
}

#[test]
fn materialize_without_key_or_record_writes_nothing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_path_buf();
    std::fs::write(root.join("README.md"), "# stub").unwrap();

    // No key: nothing happens.
    let returned = materialize_at(root.clone(), "rust-analyzer-lsp", None);
    assert_eq!(returned, root);
    assert!(!root.join(".claude-plugin/plugin.json").exists());

    // A key that was never registered: nothing happens.
    let returned = materialize_at(root.clone(), "rust-analyzer-lsp", Some(&"f".repeat(64)));
    assert_eq!(returned, root);
    assert!(!root.join(".claude-plugin/plugin.json").exists());
}

/// End-to-end: the store provider tags a stub entry's source with the
/// manifest key, `add` clones and materialises, and the plugin installs.
#[test]
fn store_provider_tags_stub_sources_so_add_installs_them() {
    use ragent_plugins::{StoreDirs, StoreKind, add, provider_for};
    use std::path::PathBuf;

    // 1. Parse the marketplace document through the real provider; the stub
    //    entry's source must gain the `#<key>` suffix, the plain entry's must
    //    not.
    let origin_url = "https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json";
    let origin = reqwest::Url::parse(origin_url).unwrap();
    let index = provider_for(StoreKind::Claude)
        .parse_index(DOC.as_bytes(), &origin)
        .expect("vendor marketplace parses");
    let stub = index
        .entries
        .iter()
        .find(|e| e.name == "rust-analyzer-lsp")
        .expect("stub entry present");
    let plain = index
        .entries
        .iter()
        .find(|e| e.name == "agent-sdk-dev")
        .expect("plain entry present");
    let expected_key = document_key(origin_url, DOC.as_bytes());
    let expected_suffix = format!("@{expected_key}");
    assert!(
        stub.source.ends_with(&expected_suffix),
        "stub source must carry the manifest key: {}",
        stub.source
    );
    assert!(
        !plain.source.contains('@'),
        "plain listing source must be untouched: {}",
        plain.source
    );
    assert!(stub.source.starts_with(
        "git+https://github.com/anthropics/claude-plugins-official#main:plugins/rust-analyzer-lsp"
    ));

    // 2. Install the stub's source against a local git mirror of the stub
    //    layout, so the test performs no network I/O. `materialize_at` writes
    //    the manifest from the recorded marketplace document.
    let repo = tempfile::tempdir().expect("repo tempdir");
    let plugin_dir = repo.path().join("plugins/rust-analyzer-lsp");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::write(plugin_dir.join("README.md"), "# stub").unwrap();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "test@example.com"]);
    git(&["config", "user.name", "test"]);
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "stub"]);

    let store = tempfile::tempdir().expect("store tempdir");
    let dirs = StoreDirs {
        project: Some(store.path().to_path_buf()),
        global: None,
    };
    let source = format!(
        "git+file://{}#HEAD:plugins/rust-analyzer-lsp{}",
        repo.path().display(),
        expected_suffix
    );
    let outcome = add(&dirs, &PathBuf::from("."), &source, false).expect("stub installs");
    assert_eq!(outcome.parsed.descriptor.id, "rust-analyzer-lsp");
    assert_eq!(
        outcome.parsed.descriptor.dialect,
        ragent_plugins::PluginDialect::Claude
    );
    assert!(
        outcome
            .installed_dir
            .join(".claude-plugin/plugin.json")
            .is_file(),
        "materialised manifest committed to the store"
    );
    // FR-025: the bridged-in lspServers section is reported, never silently
    // dropped.
    assert!(
        outcome
            .parsed
            .descriptor
            .unsupported_capabilities
            .contains(&ragent_plugins::UNSUP_LSP.to_string()),
        "lspServers recorded as unsupported: {:?}",
        outcome.parsed.descriptor.unsupported_capabilities
    );
}

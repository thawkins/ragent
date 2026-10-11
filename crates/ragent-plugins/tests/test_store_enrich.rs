//! Tests for the store-browser manifest-metadata enrichment (spec `pluginstores`
//! FR-003, FR-048).
//!
//! The URL derivation and the manifest parsing are pure and tested directly; the
//! `enrich_into` driver is exercised with an offline [`ManifestSource`] so no
//! network is contacted (NFR-003).

use std::sync::Mutex;

use ragent_plugins::{
    ManifestMeta, ManifestSource, ManifestUpdate, StoreEntry, StoreKind, enrich_into,
    manifest_urls, parse_manifest_meta,
};

fn entry(id: &str, source: &str) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: String::new(),
        source: source.to_string(),
        description: String::new(),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
        category: None,
        marketplace: String::new(),
    }
}

// -- manifest URL derivation --------------------------------------------------

#[test]
fn a_codex_git_subdir_source_yields_dialect_appropriate_manifest_urls() {
    let urls = manifest_urls(
        "git+https://github.com/openai/plugins#main:plugins/linear",
        StoreKind::Codex,
    );
    // The Codex nested manifest is tried first, then a bare plugin.json and the
    // direct codex-plugin.json.
    assert_eq!(
        urls.first().map(String::as_str),
        Some(
            "https://raw.githubusercontent.com/openai/plugins/main/plugins/linear/.codex-plugin/plugin.json"
        )
    );
    assert!(
        urls.contains(
            &"https://raw.githubusercontent.com/openai/plugins/main/plugins/linear/plugin.json"
                .to_string()
        ),
        "the generic manifest name is tried too: {urls:?}"
    );
}

#[test]
fn a_claude_repo_relative_source_yields_the_claude_manifest_first() {
    let urls = manifest_urls(
        "git+https://github.com/anthropics/claude-plugins-official#main:plugins/agent-sdk-dev",
        StoreKind::Claude,
    );
    assert_eq!(
        urls.first().map(String::as_str),
        Some(
            "https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/plugins/agent-sdk-dev/.claude-plugin/plugin.json"
        )
    );
}

#[test]
fn a_whole_repo_source_without_a_subpath_uses_the_repo_root_and_head() {
    let urls = manifest_urls(
        "git+https://github.com/CrowdStrike/foundry-skills#HEAD",
        StoreKind::Codex,
    );
    assert_eq!(
        urls.first().map(String::as_str),
        Some(
            "https://raw.githubusercontent.com/CrowdStrike/foundry-skills/HEAD/.codex-plugin/plugin.json"
        )
    );
}

#[test]
fn a_non_github_or_non_git_source_yields_no_urls() {
    // An archive URL (Claude's `https://.../x.zip` style) has no derivable repo.
    assert_eq!(
        manifest_urls("https://example.org/x.zip", StoreKind::Claude),
        [] as [std::string::String; 0]
    );
    // A git source on a non-GitHub host has no raw.githubusercontent view.
    assert_eq!(
        manifest_urls("git+https://gitlab.com/o/r#main", StoreKind::Codex),
        [] as [std::string::String; 0]
    );
    // A plain local directory is not a git source.
    assert_eq!(
        manifest_urls("./plugins/x", StoreKind::Codex),
        [] as [std::string::String; 0]
    );
}

// -- manifest parsing ---------------------------------------------------------

#[test]
fn a_codex_manifest_maps_the_interface_display_name_and_description() {
    let bytes = br#"{
      "name": "linear",
      "description": "Search, create and update Linear issues.",
      "interface": {
        "displayName": "Linear",
        "shortDescription": "Manage Linear issues"
      }
    }"#;
    let meta = parse_manifest_meta(bytes);
    // The top-level description wins over the interface block's.
    assert_eq!(
        meta.description.as_deref(),
        Some("Search, create and update Linear issues.")
    );
    // This manifest declares no version, so the version stays absent.
    assert!(meta.version.is_none());
}

#[test]
fn an_interface_only_manifest_falls_back_to_the_short_description() {
    let bytes = br#"{
      "interface": {
        "displayName": "Slack",
        "shortDescription": "Work with Slack",
        "longDescription": "A longer Slack description"
      }
    }"#;
    let meta = parse_manifest_meta(bytes);
    assert_eq!(meta.description.as_deref(), Some("Work with Slack"));
}

#[test]
fn a_manifest_with_a_version_keeps_it() {
    let bytes = br#"{"name":"gmail","version":"0.1.9","description":"Work with Gmail"}"#;
    let meta = parse_manifest_meta(bytes);
    assert_eq!(meta.version.as_deref(), Some("0.1.9"));
    assert_eq!(meta.description.as_deref(), Some("Work with Gmail"));
}

#[test]
fn malformed_json_yields_empty_metadata() {
    assert!(parse_manifest_meta(b"{ not json").is_empty());
}

// -- enrichment driver --------------------------------------------------------

/// An offline manifest source that answers from a fixed table and records how
/// many lookups it saw.
struct FakeSource {
    meta_by_id: Vec<(String, ManifestMeta)>,
    seen: Mutex<usize>,
}

impl ManifestSource for FakeSource {
    fn manifest_meta(&self, entry: &StoreEntry, _kind: StoreKind) -> Option<ManifestMeta> {
        *self.seen.lock().expect("seen lock") += 1;
        self.meta_by_id
            .iter()
            .find(|(id, _)| id == &entry.id)
            .map(|(_, meta)| meta.clone())
    }
}

#[test]
fn enrich_into_reports_one_update_per_entry_with_metadata() {
    let entries = vec![
        entry("a", "git+https://github.com/o/r#main:plugins/a"),
        entry("b", "https://example.org/b.zip"), // no derivable manifest -> skipped
        entry("c", "git+https://github.com/o/r#main:plugins/c"),
    ];
    let source = FakeSource {
        meta_by_id: vec![
            (
                "a".to_string(),
                ManifestMeta {
                    version: Some("1.2.3".to_string()),
                    description: Some("Plugin A".to_string()),
                },
            ),
            ("c".to_string(), ManifestMeta::default()),
        ],
        seen: Mutex::new(0),
    };
    let updates = Mutex::new(Vec::new());
    enrich_into(
        &entries,
        StoreKind::Codex,
        &source,
        &|update: ManifestUpdate| {
            updates.lock().expect("updates lock").push(update);
        },
    );

    // The archive-sourced entry was never looked up.
    assert_eq!(*source.seen.lock().expect("seen lock"), 2);
    let updates = updates.into_inner().expect("updates lock");
    let a = updates.iter().find(|u| u.id == "a").expect("a updated");
    assert_eq!(a.meta.version.as_deref(), Some("1.2.3"));
    // An empty metadata value still reports (it is dropped by the caller), so `c`
    // appears with nothing set.
    let c = updates.iter().find(|u| u.id == "c").expect("c reported");
    assert!(c.meta.is_empty());
    assert!(
        updates.iter().all(|u| u.id != "b"),
        "the archive entry is never looked up"
    );
}

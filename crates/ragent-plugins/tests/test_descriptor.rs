//! Tests for the plugin descriptor model and dialect recognition rules
//! (spec `plugins` T-002; FR-002, FR-025).

use std::path::PathBuf;

use ragent_plugins::{
    DialectMatch, PluginDescriptor, PluginDialect, PluginError, detect_dialect, recognise_dialect,
};

/// Pure `read_entry` that serves named manifest bodies from a static map.
fn reader_with(
    bodies: &'static [(&'static str, &'static str)],
) -> impl Fn(&str) -> Option<Vec<u8>> {
    move |name| {
        bodies
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, body)| body.as_bytes().to_vec())
    }
}

fn no_bodies(name: &str) -> Option<Vec<u8>> {
    let _ = name;
    None
}

// ── Pure recognition over directory listings ────────────────────────────────

#[test]
fn recognises_codex_manifest_file() {
    let entries = vec!["codex-plugin.json".to_string(), "index.js".to_string()];
    let result = recognise_dialect(&entries, no_bodies).expect("no ambiguity");
    assert_eq!(
        result,
        Some(DialectMatch {
            dialect: PluginDialect::Codex,
            manifest_rel: PathBuf::from("codex-plugin.json"),
        })
    );
}

#[test]
fn recognises_marked_plugin_json_as_codex() {
    static BODIES: &[(&str, &str)] = &[("plugin.json", r#"{"codex": true, "entry": "index.js"}"#)];
    let entries = vec!["plugin.json".to_string(), "index.js".to_string()];
    let result = recognise_dialect(&entries, reader_with(BODIES)).expect("no ambiguity");
    assert_eq!(
        result,
        Some(DialectMatch {
            dialect: PluginDialect::Codex,
            manifest_rel: PathBuf::from("plugin.json"),
        })
    );
}

#[test]
fn unmarked_plugin_json_is_not_a_plugin() {
    static BODIES: &[(&str, &str)] = &[("plugin.json", r#"{"name": "something-else"}"#)];
    let entries = vec!["plugin.json".to_string()];
    let result = recognise_dialect(&entries, reader_with(BODIES)).expect("no ambiguity");
    assert_eq!(result, None);
}

#[test]
fn unparseable_plugin_json_is_not_a_plugin() {
    static BODIES: &[(&str, &str)] = &[("plugin.json", "{not json")];
    let entries = vec!["plugin.json".to_string()];
    let result = recognise_dialect(&entries, reader_with(BODIES)).expect("no ambiguity");
    assert_eq!(result, None);
}

#[test]
fn recognises_claude_manifest_file() {
    let entries = vec!["claude-plugin.json".to_string(), "main.js".to_string()];
    let result = recognise_dialect(&entries, no_bodies).expect("no ambiguity");
    assert_eq!(
        result,
        Some(DialectMatch {
            dialect: PluginDialect::Claude,
            manifest_rel: PathBuf::from("claude-plugin.json"),
        })
    );
}

#[test]
fn recognises_nested_claude_plugin_manifest() {
    static BODIES: &[(&str, &str)] = &[(
        ".claude-plugin/plugin.json",
        r#"{"name": "todo", "entry": "main.js"}"#,
    )];
    let entries = vec![".claude-plugin".to_string(), "main.js".to_string()];
    let result = recognise_dialect(&entries, reader_with(BODIES)).expect("no ambiguity");
    assert_eq!(
        result,
        Some(DialectMatch {
            dialect: PluginDialect::Claude,
            manifest_rel: PathBuf::from(".claude-plugin/plugin.json"),
        })
    );
}

#[test]
fn nested_marker_without_manifest_file_is_not_a_plugin() {
    // A `.claude-plugin` directory exists but contains no `plugin.json`.
    let entries = vec![".claude-plugin".to_string()];
    let result = recognise_dialect(&entries, no_bodies).expect("no ambiguity");
    assert_eq!(result, None);
}

#[test]
fn recognises_nested_codex_plugin_manifest() {
    static BODIES: &[(&str, &str)] = &[(
        ".codex-plugin/plugin.json",
        r#"{"name": "linear", "version": "5.0.1"}"#,
    )];
    let entries = vec![".codex-plugin".to_string(), "README.md".to_string()];
    let result = recognise_dialect(&entries, reader_with(BODIES)).expect("no ambiguity");
    assert_eq!(
        result,
        Some(DialectMatch {
            dialect: PluginDialect::Codex,
            manifest_rel: PathBuf::from(".codex-plugin/plugin.json"),
        })
    );
}

#[test]
fn nested_codex_marker_without_manifest_file_is_not_a_plugin() {
    // A `.codex-plugin` directory exists but contains no `plugin.json`.
    let entries = vec![".codex-plugin".to_string()];
    let result = recognise_dialect(&entries, no_bodies).expect("no ambiguity");
    assert_eq!(result, None);
}

#[test]
fn multi_target_nested_manifest_pair_resolves_to_claude() {
    // A single upstream tree shipping one manifest per host (the shape
    // `mongodb/agent-skills` uses): both nested manifests present. This is not
    // ambiguous; Claude wins deterministically.
    static BODIES: &[(&str, &str)] = &[
        (
            ".codex-plugin/plugin.json",
            r#"{"name": "mongodb", "version": "1.2.1"}"#,
        ),
        (
            ".claude-plugin/plugin.json",
            r#"{"name": "mongodb", "version": "1.2.1"}"#,
        ),
    ];
    let entries = vec![
        ".codex-plugin".to_string(),
        ".claude-plugin".to_string(),
        "skills".to_string(),
    ];
    let result = recognise_dialect(&entries, reader_with(BODIES)).expect("multi-target resolves");
    assert_eq!(
        result,
        Some(DialectMatch {
            dialect: PluginDialect::Claude,
            manifest_rel: PathBuf::from(".claude-plugin/plugin.json"),
        })
    );
}

#[test]
fn nested_codex_manifest_plus_top_level_claude_manifest_is_ambiguous() {
    // Only the both-nested pairing is multi-target; a nested Codex manifest
    // beside a top-level Claude manifest remains a genuine ambiguity.
    static BODIES: &[(&str, &str)] = &[(".codex-plugin/plugin.json", r#"{"name": "x"}"#)];
    let entries = vec![
        ".codex-plugin".to_string(),
        "claude-plugin.json".to_string(),
    ];
    let result = recognise_dialect(&entries, reader_with(BODIES));
    assert!(matches!(result, Err(PluginError::AmbiguousManifest)));
}

#[test]
fn plain_directory_is_not_a_plugin() {
    let entries = vec!["main.rs".to_string(), "Cargo.toml".to_string()];
    let result = recognise_dialect(&entries, no_bodies).expect("no ambiguity");
    assert_eq!(result, None);
}

#[test]
fn ambiguous_directory_is_rejected() {
    // Matches both dialects: codex-plugin.json and claude-plugin.json.
    let entries = vec![
        "codex-plugin.json".to_string(),
        "claude-plugin.json".to_string(),
    ];
    let result = recognise_dialect(&entries, no_bodies);
    assert!(
        matches!(result, Err(PluginError::AmbiguousManifest)),
        "expected ambiguous-manifest rejection, got {result:?}"
    );
}

#[test]
fn ambiguous_marked_plugin_json_plus_claude_manifest_is_rejected() {
    static BODIES: &[(&str, &str)] = &[("plugin.json", r#"{"codex": {}}"#)];
    let entries = vec!["plugin.json".to_string(), "claude-plugin.json".to_string()];
    let result = recognise_dialect(&entries, reader_with(BODIES));
    assert!(matches!(result, Err(PluginError::AmbiguousManifest)));
}

// ── Filesystem wrapper over real fixture directories ────────────────────────

static FIXTURE_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII tempdir rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct FixtureDir(PathBuf);

impl FixtureDir {
    fn new(name: &str) -> Self {
        let unique = FIXTURE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("fixture dir should be creatable");
        Self(path)
    }

    fn write(&self, rel: &str, body: &str) {
        let path = self.0.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("fixture parent should be creatable");
        }
        std::fs::write(path, body).expect("fixture file should be writable");
    }
}

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn detect_dialect_finds_codex_directory_on_disk() {
    let dir = FixtureDir::new("codex");
    dir.write(
        "codex-plugin.json",
        r#"{"name": "weather", "entry": "index.js"}"#,
    );
    dir.write("index.js", "// plugin code");

    let result = detect_dialect(&dir.0).expect("no ambiguity");
    assert_eq!(
        result,
        Some(DialectMatch {
            dialect: PluginDialect::Codex,
            manifest_rel: PathBuf::from("codex-plugin.json"),
        })
    );
}

#[test]
fn detect_dialect_finds_nested_claude_directory_on_disk() {
    let dir = FixtureDir::new("claude-nested");
    dir.write(".claude-plugin/plugin.json", r#"{"name": "todo"}"#);

    let result = detect_dialect(&dir.0).expect("no ambiguity");
    assert_eq!(
        result,
        Some(DialectMatch {
            dialect: PluginDialect::Claude,
            manifest_rel: PathBuf::from(".claude-plugin/plugin.json"),
        })
    );
}

#[test]
fn detect_dialect_reports_ambiguity_on_disk() {
    let dir = FixtureDir::new("ambiguous");
    dir.write("codex-plugin.json", "{}");
    dir.write("claude-plugin.json", "{}");

    let result = detect_dialect(&dir.0);
    assert!(matches!(result, Err(PluginError::AmbiguousManifest)));
}

#[test]
fn detect_dialect_resolves_multi_target_nested_pair_on_disk() {
    // The `mongodb/agent-skills` layout: both nested manifests on disk.
    let dir = FixtureDir::new("multi-target");
    dir.write(
        ".codex-plugin/plugin.json",
        r#"{"name": "mongodb", "version": "1.2.1"}"#,
    );
    dir.write(
        ".claude-plugin/plugin.json",
        r#"{"name": "mongodb", "version": "1.2.1"}"#,
    );

    let result = detect_dialect(&dir.0).expect("multi-target resolves");
    assert_eq!(
        result,
        Some(DialectMatch {
            dialect: PluginDialect::Claude,
            manifest_rel: PathBuf::from(".claude-plugin/plugin.json"),
        })
    );
}

#[test]
fn detect_dialect_returns_none_for_non_plugin_directory() {
    let dir = FixtureDir::new("not-a-plugin");
    dir.write("README.md", "hello");

    assert_eq!(detect_dialect(&dir.0).expect("no ambiguity"), None);
}

// ── Descriptor model shape (FR-002, FR-025) ─────────────────────────────────

#[test]
fn descriptor_records_unsupported_capabilities() {
    let descriptor = PluginDescriptor {
        id: "codex-weather".to_string(),
        name: "Weather".to_string(),
        version: "1.2.0".to_string(),
        dialect: PluginDialect::Codex,
        entry: Some(PathBuf::from("/plugins/codex-weather/index.js")),
        requested_permissions: vec!["network.outbound".to_string()],
        api_version: 1,
        unsupported_capabilities: vec!["claude-desktop:mcp-server transport".to_string()],
        manifest_path: PathBuf::from("/plugins/codex-weather/codex-plugin.json"),
        root: PathBuf::from("/plugins/codex-weather"),
    };

    assert_eq!(descriptor.dialect.to_string(), "codex");
    assert_eq!(
        descriptor.unsupported_capabilities,
        vec!["claude-desktop:mcp-server transport"]
    );

    let json = serde_json::to_string(&descriptor).expect("descriptor should serialise");
    assert!(json.contains("\"dialect\":\"codex\""));
    assert!(json.contains("unsupported_capabilities"));
    let reparsed: PluginDescriptor = serde_json::from_str(&json).expect("descriptor should parse");
    assert_eq!(reparsed, descriptor);
}

#[test]
fn dialect_display_names_are_stable() {
    assert_eq!(PluginDialect::Codex.to_string(), "codex");
    assert_eq!(PluginDialect::Claude.to_string(), "claude");
}

// ── Error reporting keeps the underlying cause chain ────────────────────────

#[test]
fn ambiguous_manifest_error_names_the_cause() {
    let err = PluginError::AmbiguousManifest;
    assert!(err.to_string().contains("ambiguous"));
}

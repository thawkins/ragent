//! Tests for `dirs` allowlist/denylist merge behavior in config loading.
//!
//! Covers the fix for the issue where `/dirs add allow|deny` persisted to
//! the config file but the values were not merged when loading global +
//! project configs together.

use ragent_config::Config;
use std::fs;
use std::path::Path;

/// Write a config file with the given content.
#[allow(dead_code)]
fn write_config(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(path, contents).expect("write config");
}

#[test]
fn test_dirs_merge_union_from_global_and_project() {
    // Create configs as JSON strings and parse them
    let global_content = r#"{
        "dirs": {
            "allowlist": ["src/**/*.rs", "tests/**/*.rs"],
            "denylist": ["secrets/**", ".env"]
        }
    }"#;

    let project_content = r#"{
        "dirs": {
            "allowlist": ["docs/**/*.md", "target/**"],
            "denylist": ["build/**", "*.tmp"]
        }
    }"#;

    // Parse both configs
    let global_config: Config = serde_json::from_str(global_content).expect("parse global");
    let project_config: Config = serde_json::from_str(project_content).expect("parse project");

    // Merge: global (base) + project (overlay)
    let merged = Config::merge(global_config, project_config);

    // Assert that allowlist contains patterns from BOTH configs
    assert!(
        merged.dirs.allowlist.contains(&"src/**/*.rs".to_string()),
        "merged allowlist should contain global pattern 'src/**/*.rs'"
    );
    assert!(
        merged.dirs.allowlist.contains(&"tests/**/*.rs".to_string()),
        "merged allowlist should contain global pattern 'tests/**/*.rs'"
    );
    assert!(
        merged.dirs.allowlist.contains(&"docs/**/*.md".to_string()),
        "merged allowlist should contain project pattern 'docs/**/*.md'"
    );
    assert!(
        merged.dirs.allowlist.contains(&"target/**".to_string()),
        "merged allowlist should contain project pattern 'target/**'"
    );

    // Assert that denylist contains patterns from BOTH configs
    assert!(
        merged.dirs.denylist.contains(&"secrets/**".to_string()),
        "merged denylist should contain global pattern 'secrets/**'"
    );
    assert!(
        merged.dirs.denylist.contains(&".env".to_string()),
        "merged denylist should contain global pattern '.env'"
    );
    assert!(
        merged.dirs.denylist.contains(&"build/**".to_string()),
        "merged denylist should contain project pattern 'build/**'"
    );
    assert!(
        merged.dirs.denylist.contains(&"*.tmp".to_string()),
        "merged denylist should contain project pattern '*.tmp'"
    );

    // Total counts: 4 allowlist, 4 denylist
    assert_eq!(
        merged.dirs.allowlist.len(),
        4,
        "allowlist should have 4 unique patterns"
    );
    assert_eq!(
        merged.dirs.denylist.len(),
        4,
        "denylist should have 4 unique patterns"
    );
}

#[test]
fn test_dirs_merge_avoids_duplicates() {
    let global_content = r#"{
        "dirs": {
            "allowlist": ["src/**/*.rs", "common/**"],
            "denylist": ["secrets/**"]
        }
    }"#;

    let project_content = r#"{
        "dirs": {
            "allowlist": ["src/**/*.rs", "docs/**/*.md"],
            "denylist": ["secrets/**", "build/**"]
        }
    }"#;

    let global_config: Config = serde_json::from_str(global_content).expect("parse global");
    let project_config: Config = serde_json::from_str(project_content).expect("parse project");

    let merged = Config::merge(global_config, project_config);

    // "src/**/*.rs" appears in both but should only appear once in merged
    let src_count = merged
        .dirs
        .allowlist
        .iter()
        .filter(|p| *p == "src/**/*.rs")
        .count();
    assert_eq!(src_count, 1, "duplicate patterns should be deduplicated");

    // "secrets/**" appears in both but should only appear once in merged
    let secrets_count = merged
        .dirs
        .denylist
        .iter()
        .filter(|p| *p == "secrets/**")
        .count();
    assert_eq!(
        secrets_count, 1,
        "duplicate denylist patterns should be deduplicated"
    );

    // Total: 3 unique allowlist, 2 unique denylist
    assert_eq!(merged.dirs.allowlist.len(), 3);
    assert_eq!(merged.dirs.denylist.len(), 2);
}

#[test]
fn test_dirs_merge_empty_base() {
    let project_content = r#"{
        "dirs": {
            "allowlist": ["src/**/*.rs"],
            "denylist": ["secrets/**"]
        }
    }"#;

    let base = Config::default();
    let overlay: Config = serde_json::from_str(project_content).expect("parse project");

    let merged = Config::merge(base, overlay);

    assert_eq!(merged.dirs.allowlist.len(), 1);
    assert_eq!(merged.dirs.denylist.len(), 1);
    assert!(merged.dirs.allowlist.contains(&"src/**/*.rs".to_string()));
    assert!(merged.dirs.denylist.contains(&"secrets/**".to_string()));
}

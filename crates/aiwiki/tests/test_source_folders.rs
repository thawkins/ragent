//! Unit tests for the AIWiki source folders data model.
//!
//! Tests cover:
//! - SourceFolder struct creation and parsing
//! - AiwikiConfig CRUD operations for sources
//! - FileState backward compatibility
//! - Validation logic

use aiwiki::source_folder::SourceFolder;
use aiwiki::{AiwikiConfig, FileState};

// ============================================================================
// SourceFolder::new() Tests
// ============================================================================

#[test]
fn test_source_folder_new_defaults() {
    let source = SourceFolder::new("docs");

    assert_eq!(source.path, "docs");
    assert!(source.label.is_none());
    assert_eq!(source.patterns, vec!["**/*"]);
    assert!(source.recursive);
    assert!(source.enabled);
}

// ============================================================================
// SourceFolder::from_spec() Tests
// ============================================================================

#[test]
fn test_from_spec_simple_folder() {
    let source = SourceFolder::from_spec("docs").unwrap();

    assert_eq!(source.path, "docs");
    assert_eq!(source.patterns, vec!["**/*"]);
    assert!(source.recursive);
    assert!(source.enabled);
}

#[test]
fn test_from_spec_folder_with_trailing_slash() {
    let source = SourceFolder::from_spec("docs/").unwrap();

    assert_eq!(source.path, "docs");
    assert_eq!(source.patterns, vec!["**/*"]);
}

#[test]
fn test_from_spec_with_glob_pattern() {
    let source = SourceFolder::from_spec("src/*.rs").unwrap();

    assert_eq!(source.path, "src");
    assert_eq!(source.patterns, vec!["**/*.rs"]);
}

#[test]
fn test_from_spec_with_explicit_recursive_glob() {
    let source = SourceFolder::from_spec("tests/**/*.rs").unwrap();

    assert_eq!(source.path, "tests");
    assert_eq!(source.patterns, vec!["**/*.rs"]);
}

#[test]
fn test_from_spec_nested_path_with_pattern() {
    let source = SourceFolder::from_spec("crates/aiwiki/src/*.rs").unwrap();

    assert_eq!(source.path, "crates/aiwiki/src");
    assert_eq!(source.patterns, vec!["**/*.rs"]);
}

#[test]
fn test_from_spec_nested_path_only() {
    let source = SourceFolder::from_spec("crates/aiwiki/src").unwrap();

    assert_eq!(source.path, "crates/aiwiki/src");
    assert_eq!(source.patterns, vec!["**/*"]);
}

#[test]
fn test_from_spec_empty_fails() {
    let result = SourceFolder::from_spec("");
    assert!(result.is_err());
}

#[test]
fn test_from_spec_glob_only_fails() {
    // Pattern without a folder path should fail
    let result = SourceFolder::from_spec("*.rs");
    assert!(result.is_err());
}

// ============================================================================
// AiwikiConfig Tests
// ============================================================================

#[test]
fn test_config_default_has_empty_sources() {
    let config = AiwikiConfig::default();
    assert!(config.sources.is_empty());
}

#[test]
fn test_config_add_source() {
    let mut config = AiwikiConfig::default();
    let source = SourceFolder::new("docs");

    config.add_source(source).unwrap();

    assert_eq!(config.sources.len(), 1);
    assert_eq!(config.sources[0].path, "docs");
}

#[test]
fn test_config_add_source_rejects_duplicates() {
    let mut config = AiwikiConfig::default();
    let source1 = SourceFolder::new("docs");
    let source2 = SourceFolder::new("docs");

    config.add_source(source1).unwrap();
    let result = config.add_source(source2);

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("already registered")
    );
}

#[test]
fn test_config_remove_source() {
    let mut config = AiwikiConfig::default();
    config.add_source(SourceFolder::new("docs")).unwrap();
    config.add_source(SourceFolder::new("src")).unwrap();

    let removed = config.remove_source("docs").unwrap();

    assert_eq!(removed.path, "docs");
    assert_eq!(config.sources.len(), 1);
    assert_eq!(config.sources[0].path, "src");
}

#[test]
fn test_config_remove_source_not_found() {
    let mut config = AiwikiConfig::default();
    config.add_source(SourceFolder::new("docs")).unwrap();

    let result = config.remove_source("nonexistent");

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_config_update_source() {
    let mut config = AiwikiConfig::default();
    config.add_source(SourceFolder::new("docs")).unwrap();

    let mut updated = SourceFolder::new("docs");
    updated.label = Some("Documentation".to_string());
    updated.enabled = false;

    config.update_source("docs", updated).unwrap();

    assert_eq!(config.sources[0].label, Some("Documentation".to_string()));
    assert!(!config.sources[0].enabled);
}

#[test]
fn test_config_get_source() {
    let mut config = AiwikiConfig::default();
    let source = SourceFolder::new("docs");
    config.add_source(source).unwrap();

    let found = config.get_source("docs");
    let not_found = config.get_source("nonexistent");

    assert!(found.is_some());
    assert_eq!(found.unwrap().path, "docs");
    assert!(not_found.is_none());
}

#[test]
fn test_config_list_sources() {
    let mut config = AiwikiConfig::default();
    config.add_source(SourceFolder::new("docs")).unwrap();
    config.add_source(SourceFolder::new("src")).unwrap();

    let sources = config.list_sources();

    assert_eq!(sources.len(), 2);
}

#[test]
fn test_config_enabled_sources() {
    let mut config = AiwikiConfig::default();

    let mut disabled_source = SourceFolder::new("tests");
    disabled_source.enabled = false;

    config.add_source(SourceFolder::new("docs")).unwrap();
    config.add_source(disabled_source).unwrap();

    let enabled: Vec<_> = config.enabled_sources().collect();

    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].path, "docs");
}

// ============================================================================
// SourceFolder Validation Tests
// ============================================================================

#[test]
fn test_source_folder_validate_rejects_absolute_path() {
    use std::path::Path;

    // Create a temp directory as project root
    let temp_dir = std::env::temp_dir();

    let source = SourceFolder::new("/absolute/path");
    let result = source.validate(&temp_dir);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("absolute"));
}

// ============================================================================
// FileState Tests
// ============================================================================

#[test]
fn test_file_state_with_source_field() {
    let state = FileState {
        hash: "abc123".to_string(),
        modified: chrono::Utc::now(),
        size: 1024,
        generated_pages: vec!["page1.md".to_string()],
        source: Some("docs".to_string()),
    };

    assert_eq!(state.source, Some("docs".to_string()));
}

#[test]
fn test_file_state_without_source_field() {
    // Simulating backward compatibility - old state without source field
    let state = FileState {
        hash: "abc123".to_string(),
        modified: chrono::Utc::now(),
        size: 1024,
        generated_pages: vec!["page1.md".to_string()],
        source: None,
    };

    assert_eq!(state.source, None);
}

// ============================================================================
// Pattern Matching Tests (SourceFolder::matches)
// ============================================================================

#[test]
fn test_source_folder_matches_all_files() {
    let source = SourceFolder::new("docs");

    assert!(source.matches("readme.md"));
    assert!(source.matches("guide.md"));
    assert!(source.matches("subfolder/file.txt"));
}

#[test]
fn test_source_folder_matches_rust_files() {
    let source = SourceFolder::from_spec("src/*.rs").unwrap();

    assert!(source.matches("main.rs"));
    assert!(source.matches("lib.rs"));
    assert!(source.matches("module/mod.rs"));
    assert!(!source.matches("readme.md"));
    assert!(!source.matches("main.py"));
}

#[test]
fn test_source_folder_matches_markdown_files() {
    let source = SourceFolder::from_spec("docs/**/*.md").unwrap();

    assert!(source.matches("readme.md"));
    assert!(source.matches("guide.md"));
    assert!(source.matches("subfolder/file.md"));
    assert!(!source.matches("file.txt"));
}

// ============================================================================
// SourceFolder File Source Tests
// ============================================================================

#[test]
fn test_source_folder_from_file_path() {
    let source = SourceFolder::from_file_path("README.md");

    assert_eq!(source.path, "README.md");
    assert_eq!(source.patterns, vec!["README.md"]);
    assert!(source.is_file);
    assert!(!source.recursive);
    assert!(source.enabled);
    assert!(source.label.is_none());
}

#[test]
fn test_source_folder_from_file_path_nested() {
    let source = SourceFolder::from_file_path("docs/CONTRIBUTING.md");

    assert_eq!(source.path, "docs/CONTRIBUTING.md");
    assert_eq!(source.patterns, vec!["docs/CONTRIBUTING.md"]);
    assert!(source.is_file);
    assert!(!source.recursive);
}

#[test]
fn test_source_folder_file_matches() {
    let source = SourceFolder::from_file_path("README.md");

    // For file sources, should match the exact path
    assert!(source.matches("README.md"));
}

#[test]
fn test_source_folder_file_default_serde() {
    // Test that is_file defaults to false when deserializing
    use serde_json;

    let json = r#"{ "path": "docs", "patterns": ["**/*"] }"#;
    let source: SourceFolder = serde_json::from_str(json).unwrap();

    assert!(!source.is_file);
    assert_eq!(source.path, "docs");
    assert_eq!(source.patterns, vec!["**/*"]);
}

// ============================================================================
// Serialization Tests (Backward Compatibility)
// ============================================================================

#[test]
fn test_file_state_serialization_with_source() {
    use serde_json;

    let state = FileState {
        hash: "abc123".to_string(),
        modified: chrono::Utc::now(),
        size: 1024,
        generated_pages: vec!["page1.md".to_string()],
        source: Some("docs".to_string()),
    };

    let json = serde_json::to_string(&state).unwrap();
    assert!(json.contains("abc123"));
    assert!(json.contains("docs"));
}

#[test]
fn test_file_state_deserialization_backward_compat() {
    use serde_json;

    // JSON without source field (old format)
    let json = r#"{
        "hash": "abc123",
        "modified": "2024-01-01T00:00:00Z",
        "size": 1024,
        "generated_pages": ["page1.md"]
    }"#;

    let state: FileState = serde_json::from_str(json).unwrap();

    assert_eq!(state.hash, "abc123");
    assert_eq!(state.source, None);
    assert_eq!(state.generated_pages.len(), 1);
}

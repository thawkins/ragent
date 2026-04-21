//! Integration tests for referenced folder scanning and sync.
//!
//! Tests cover:
//! - Scanning a referenced folder and detecting new files
//! - Detecting modified files on re-scan
//! - Detecting deleted files
//! - Glob pattern filtering
//! - Disabled source folders are skipped
//! - Mixed raw/ + referenced folder sync

use ragent_aiwiki::{
    Aiwiki, AiwikiConfig, AiwikiState, SourceFolder,
    sync::{make_ref_key, parse_ref_key, resolve_file_path, scan_source_folder},
};
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

// ============================================================================
// Test: scan_source_folder detects new files
// ============================================================================

#[tokio::test]
async fn test_scan_source_folder_detects_new_files() {
    // Create temp project structure
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create a docs folder with files
    let docs_dir = project_root.join("docs");
    tokio::fs::create_dir_all(&docs_dir).await.unwrap();
    tokio::fs::write(docs_dir.join("readme.md"), "# Readme")
        .await
        .unwrap();
    tokio::fs::write(docs_dir.join("guide.md"), "# Guide")
        .await
        .unwrap();

    // Create a nested folder
    let sub_dir = docs_dir.join("subdir");
    tokio::fs::create_dir_all(&sub_dir).await.unwrap();
    tokio::fs::write(sub_dir.join("nested.md"), "# Nested")
        .await
        .unwrap();

    // Create source folder config
    let source = SourceFolder::new("docs");

    // Scan the folder
    let files = scan_source_folder(project_root, &source, &[] as &[&str])
        .await
        .unwrap();

    // Should find all 3 files
    assert_eq!(files.len(), 3);

    // Verify the files are found
    let file_names: Vec<String> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(file_names.contains(&"readme.md".to_string()));
    assert!(file_names.contains(&"guide.md".to_string()));
    assert!(file_names.contains(&"nested.md".to_string()));
}

// ============================================================================
// Test: scan_source_folder with glob patterns
// ============================================================================

#[tokio::test]
async fn test_scan_source_folder_glob_patterns() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create a source folder with mixed file types
    let src_dir = project_root.join("src");
    tokio::fs::create_dir_all(&src_dir).await.unwrap();
    tokio::fs::write(src_dir.join("main.rs"), "fn main() {}")
        .await
        .unwrap();
    tokio::fs::write(src_dir.join("lib.rs"), "pub fn lib() {}")
        .await
        .unwrap();
    tokio::fs::write(src_dir.join("readme.md"), "# Readme")
        .await
        .unwrap();

    // Create source folder with pattern for .rs files only
    let source = SourceFolder::from_spec("src/*.rs").unwrap();

    // Scan the folder
    let files = scan_source_folder(project_root, &source, &[] as &[&str])
        .await
        .unwrap();

    // Should find only 2 .rs files
    assert_eq!(files.len(), 2);

    let file_names: Vec<String> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(file_names.contains(&"main.rs".to_string()));
    assert!(file_names.contains(&"lib.rs".to_string()));
    assert!(!file_names.contains(&"readme.md".to_string()));
}

// ============================================================================
// Test: scan_source_folder with ignore patterns
// ============================================================================

#[tokio::test]
async fn test_scan_source_folder_respects_ignore_patterns() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create a folder with files
    let docs_dir = project_root.join("docs");
    tokio::fs::create_dir_all(&docs_dir).await.unwrap();
    tokio::fs::write(docs_dir.join("readme.md"), "# Readme")
        .await
        .unwrap();
    tokio::fs::write(docs_dir.join("temp.tmp"), "temp")
        .await
        .unwrap();

    let source = SourceFolder::new("docs");

    // Scan without ignore patterns
    let files = scan_source_folder(project_root, &source, &[] as &[&str])
        .await
        .unwrap();
    assert_eq!(files.len(), 2);

    // Scan with ignore patterns
    let ignore_patterns = vec!["*.tmp"];
    let files = scan_source_folder(project_root, &source, &ignore_patterns)
        .await
        .unwrap();
    assert_eq!(files.len(), 1);

    let file_names: Vec<String> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(file_names.contains(&"readme.md".to_string()));
    assert!(!file_names.contains(&"temp.tmp".to_string()));
}

// ============================================================================
// Test: get_ref_changes detects new, modified, and deleted files
// ============================================================================

#[tokio::test]
async fn test_get_ref_changes_detects_new_files() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create a docs folder
    let docs_dir = project_root.join("docs");
    tokio::fs::create_dir_all(&docs_dir).await.unwrap();
    tokio::fs::write(docs_dir.join("readme.md"), "# Readme")
        .await
        .unwrap();

    // Create empty state
    let state = AiwikiState::default();
    let source = SourceFolder::new("docs");

    // Get changes
    let changes = state
        .get_ref_changes(project_root, &source, &[])
        .await
        .unwrap();

    // Should detect the new file
    assert_eq!(changes.new.len(), 1);
    assert!(changes.new[0].contains("readme.md"));
    assert!(changes.new[0].starts_with("ref:"));
    assert!(changes.modified.is_empty());
    assert!(changes.deleted.is_empty());
}

#[tokio::test]
async fn test_get_ref_changes_detects_modified_files() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create a docs folder
    let docs_dir = project_root.join("docs");
    tokio::fs::create_dir_all(&docs_dir).await.unwrap();
    tokio::fs::write(docs_dir.join("readme.md"), "# Readme")
        .await
        .unwrap();

    // Create state with the file tracked
    let mut state = AiwikiState::default();
    let source = SourceFolder::new("docs");

    // Calculate hash of original content
    let hash = AiwikiState::calculate_hash(docs_dir.join("readme.md"))
        .await
        .unwrap();

    // Insert the file into state
    state.files.insert(
        make_ref_key("docs", "readme.md"),
        ragent_aiwiki::FileState {
            hash,
            modified: chrono::Utc::now(),
            size: 8, // "# Readme" = 8 bytes
            generated_pages: vec![],
            source: Some("docs".to_string()),
        },
    );

    // Modify the file
    tokio::fs::write(docs_dir.join("readme.md"), "# Updated Readme")
        .await
        .unwrap();

    // Get changes
    let changes = state
        .get_ref_changes(project_root, &source, &[])
        .await
        .unwrap();

    // Should detect the modified file
    assert!(changes.new.is_empty());
    assert_eq!(changes.modified.len(), 1);
    assert!(changes.modified[0].contains("readme.md"));
    assert!(changes.deleted.is_empty());
}

#[tokio::test]
async fn test_get_ref_changes_detects_deleted_files() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create a docs folder
    let docs_dir = project_root.join("docs");
    tokio::fs::create_dir_all(&docs_dir).await.unwrap();
    tokio::fs::write(docs_dir.join("readme.md"), "# Readme")
        .await
        .unwrap();

    // Calculate hash
    let hash = AiwikiState::calculate_hash(docs_dir.join("readme.md"))
        .await
        .unwrap();

    // Create state with the file tracked
    let mut state = AiwikiState::default();
    let source = SourceFolder::new("docs");

    state.files.insert(
        make_ref_key("docs", "readme.md"),
        ragent_aiwiki::FileState {
            hash,
            modified: chrono::Utc::now(),
            size: 8,
            generated_pages: vec![],
            source: Some("docs".to_string()),
        },
    );

    // Delete the file
    tokio::fs::remove_file(docs_dir.join("readme.md"))
        .await
        .unwrap();

    // Get changes
    let changes = state
        .get_ref_changes(project_root, &source, &[])
        .await
        .unwrap();

    // Should detect the deleted file
    assert!(changes.new.is_empty());
    assert!(changes.modified.is_empty());
    assert_eq!(changes.deleted.len(), 1);
    assert!(changes.deleted[0].contains("readme.md"));
}

// ============================================================================
// Test: get_all_changes combines raw/ and source folders
// ============================================================================

#[tokio::test]
async fn test_get_all_changes_combines_sources() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create raw/ folder
    let raw_dir = project_root.join("aiwiki/raw");
    tokio::fs::create_dir_all(&raw_dir).await.unwrap();
    tokio::fs::write(raw_dir.join("raw_file.md"), "# Raw File")
        .await
        .unwrap();

    // Create docs folder
    let docs_dir = project_root.join("docs");
    tokio::fs::create_dir_all(&docs_dir).await.unwrap();
    tokio::fs::write(docs_dir.join("docs_file.md"), "# Docs File")
        .await
        .unwrap();

    // Create state
    let state = AiwikiState::default();
    let sources = vec![SourceFolder::new("docs")];

    // Get all changes
    let changes = state
        .get_all_changes(&raw_dir, project_root, &sources, &[])
        .await
        .unwrap();

    // Should detect both files
    assert_eq!(changes.new.len(), 2);

    // Check that one is from raw/ and one from docs/
    let has_raw = changes.new.iter().any(|k| !k.starts_with("ref:"));
    let has_ref = changes.new.iter().any(|k| k.starts_with("ref:"));
    assert!(has_raw, "Should have raw/ file");
    assert!(has_ref, "Should have ref: file");
}

// ============================================================================
// Test: disabled sources are skipped
// ============================================================================

#[tokio::test]
async fn test_get_ref_changes_single_file_source() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create a single file (not in a directory)
    let spec_file = project_root.join("SPEC.md");
    tokio::fs::write(&spec_file, "# SPEC\n\nThis is the spec.")
        .await
        .unwrap();

    // Create state
    let state = AiwikiState::default();
    let source = SourceFolder::from_file_path("SPEC.md");

    // Get changes
    let changes = state
        .get_ref_changes(project_root, &source, &[])
        .await
        .unwrap();

    // Should detect the new file
    assert_eq!(changes.new.len(), 1, "Should detect 1 new file");
    assert!(
        changes.new[0].starts_with("ref:SPEC.md/"),
        "Key should start with ref:SPEC.md/, got: {}",
        changes.new[0]
    );
    assert!(changes.modified.is_empty());
    assert!(changes.deleted.is_empty());
}

// ============================================================================
// Test: disabled sources are skipped
// ============================================================================

#[tokio::test]
async fn test_disabled_sources_are_skipped() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create docs folder
    let docs_dir = project_root.join("docs");
    tokio::fs::create_dir_all(&docs_dir).await.unwrap();
    tokio::fs::write(docs_dir.join("readme.md"), "# Readme")
        .await
        .unwrap();

    // Create state
    let state = AiwikiState::default();
    let mut disabled_source = SourceFolder::new("docs");
    disabled_source.enabled = false;

    // Get changes - should return empty since source is disabled
    let changes = state
        .get_ref_changes(project_root, &disabled_source, &[])
        .await
        .unwrap();

    // Should be empty since source is disabled
    assert!(changes.is_empty());
}
// ============================================================================
// Test: state key utilities
// ============================================================================

#[tokio::test]
async fn test_make_ref_key() {
    assert_eq!(make_ref_key("docs", "readme.md"), "ref:docs/readme.md");
    assert_eq!(make_ref_key("src", "main.rs"), "ref:src/main.rs");
    assert_eq!(
        make_ref_key("crates/aiwiki", "src/lib.rs"),
        "ref:crates/aiwiki/src/lib.rs"
    );
}

#[tokio::test]
async fn test_parse_ref_key() {
    assert_eq!(
        parse_ref_key("ref:docs/readme.md"),
        Some(("docs".to_string(), "readme.md".to_string()))
    );
    assert_eq!(
        parse_ref_key("ref:src/main.rs"),
        Some(("src".to_string(), "main.rs".to_string()))
    );
    assert_eq!(parse_ref_key("readme.md"), None);
    assert_eq!(parse_ref_key("raw/readme.md"), None);
}

#[tokio::test]
async fn test_resolve_file_path() {
    let root = PathBuf::from("/project");
    let raw_dir = PathBuf::from("/project/aiwiki/raw");

    // Raw file
    let path = resolve_file_path(&root, &raw_dir, "readme.md");
    assert_eq!(path, PathBuf::from("/project/aiwiki/raw/readme.md"));

    // Ref file
    let path = resolve_file_path(&root, &raw_dir, "ref:docs/guide.md");
    assert_eq!(path, PathBuf::from("/project/docs/guide.md"));
}

// ============================================================================
// Test: scan_source_folder returns error for non-existent folder
// ============================================================================

#[tokio::test]
async fn test_scan_source_folder_errors_on_missing_folder() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    let source = SourceFolder::new("nonexistent");
    let result = scan_source_folder(project_root, &source, &[] as &[&str]).await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("does not exist"));
}

// ============================================================================
// Test: nested folder scanning
// ============================================================================

#[tokio::test]
async fn test_scan_source_folder_recurses_into_subdirectories() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create deeply nested structure
    let base_dir = project_root.join("src");
    tokio::fs::create_dir_all(&base_dir).await.unwrap();

    // Create files at different levels
    tokio::fs::write(base_dir.join("root.rs"), "")
        .await
        .unwrap();

    let level1 = base_dir.join("level1");
    tokio::fs::create_dir_all(&level1).await.unwrap();
    tokio::fs::write(level1.join("file.rs"), "").await.unwrap();

    let level2 = level1.join("level2");
    tokio::fs::create_dir_all(&level2).await.unwrap();
    tokio::fs::write(level2.join("deep.rs"), "").await.unwrap();

    let source = SourceFolder::from_spec("src/*.rs").unwrap();
    let files = scan_source_folder(project_root, &source, &[] as &[&str])
        .await
        .unwrap();

    // Should find all 3 .rs files at different levels
    assert_eq!(files.len(), 3);
}

// ============================================================================
// Test: scan_source_folder with single file source
// ============================================================================

#[tokio::test]
async fn test_scan_source_folder_single_file() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create a single file (not in a folder)
    let readme_path = project_root.join("README.md");
    tokio::fs::write(&readme_path, "# Readme").await.unwrap();

    // Create a source for this single file
    let source = SourceFolder::from_file_path("README.md");

    // Scan
    let files = scan_source_folder(project_root, &source, &[] as &[&str])
        .await
        .unwrap();

    // Should find only the one file
    assert_eq!(files.len(), 1);
    assert_eq!(files[0], readme_path);
}

#[tokio::test]
async fn test_scan_source_folder_single_file_with_ignore_pattern() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create a single file
    let readme_path = project_root.join("README.md");
    tokio::fs::write(&readme_path, "# Readme").await.unwrap();

    // Create a source for this single file
    let source = SourceFolder::from_file_path("README.md");

    // Scan with ignore pattern that matches the file
    let ignore_patterns = vec!["*.md"];
    let files = scan_source_folder(project_root, &source, &ignore_patterns)
        .await
        .unwrap();

    // Should return empty since the file is ignored
    assert_eq!(files.len(), 0);
}

#[tokio::test]
async fn test_scan_source_folder_single_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create source for non-existent file
    let source = SourceFolder::from_file_path("nonexistent.md");

    // Should return error
    let result = scan_source_folder(project_root, &source, &[] as &[&str]).await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("does not exist"));
}

// ============================================================================
// Test: scan_source_folder with path configured as directory but is a file
// ============================================================================

#[tokio::test]
async fn test_scan_source_folder_directory_source_that_is_actually_file() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create a single file (not in a folder) - named like a directory source
    let spec_path = project_root.join("SPEC.md");
    tokio::fs::write(&spec_path, "# SPEC\n\nThis is the spec.")
        .await
        .unwrap();

    // Create a source configured as a directory (is_file=false) but path is actually a file
    // This simulates the case where user adds "SPEC.md" without marking it as a file
    let source = SourceFolder::new("SPEC.md");
    assert!(!source.is_file); // By default, is_file is false

    // Scan - should handle the file gracefully even though it's configured as a directory
    let files = scan_source_folder(project_root, &source, &[] as &[&str])
        .await
        .unwrap();

    // Should find the file since scan_source_folder now detects actual file type
    assert_eq!(files.len(), 1);
    assert_eq!(files[0], spec_path);
}

#[tokio::test]
async fn test_get_ref_changes_directory_source_that_is_actually_file() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create a single file
    let spec_path = project_root.join("SPEC.md");
    tokio::fs::write(&spec_path, "# SPEC\n\nThis is the spec.")
        .await
        .unwrap();

    // Create a source configured as a directory (is_file=false) but path is actually a file
    let source = SourceFolder::new("SPEC.md");
    assert!(!source.is_file);

    // Create state
    let state = AiwikiState::default();

    // Get changes - should detect the file even though it's configured as a directory
    let changes = state
        .get_ref_changes(project_root, &source, &[])
        .await
        .unwrap();

    // Should detect the new file
    assert_eq!(changes.new.len(), 1, "Should detect 1 new file");
    assert!(
        changes.new[0].starts_with("ref:SPEC.md/"),
        "Key should start with ref:SPEC.md/"
    );
}

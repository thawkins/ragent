//! Tests for `parse_root_tree`, the pure parser behind
//! `GitHubClient::fetch_root_tree` (FR-006).

use ragent_tools_vcs::github::parse_root_tree;
use serde_json::json;

#[test]
fn test_parse_root_tree_mixed_files_and_dirs() {
    let value = json!([
        {"name": "src", "type": "dir", "size": 0},
        {"name": "Cargo.toml", "type": "file", "size": 512},
        {"name": "README.md", "type": "file", "size": 1024},
        {"name": ".gitignore", "type": "file", "size": 32}
    ]);
    let names = parse_root_tree(&value);
    assert_eq!(
        names,
        vec![
            "src".to_string(),
            "Cargo.toml".to_string(),
            "README.md".to_string(),
            ".gitignore".to_string(),
        ]
    );
}

#[test]
fn test_parse_root_tree_preserves_api_ordering() {
    let value = json!([
        {"name": "zzz.txt", "type": "file"},
        {"name": "aaa.txt", "type": "file"},
        {"name": "mmm.txt", "type": "file"}
    ]);
    let names = parse_root_tree(&value);
    // Ordering must match the API response, not be re-sorted.
    assert_eq!(
        names,
        vec![
            "zzz.txt".to_string(),
            "aaa.txt".to_string(),
            "mmm.txt".to_string(),
        ]
    );
}

#[test]
fn test_parse_root_tree_empty_array() {
    let value = json!([]);
    let names = parse_root_tree(&value);
    assert!(names.is_empty());
}

#[test]
fn test_parse_root_tree_non_array_yields_empty() {
    // A 404 body or single-file object (not an array) yields an empty vector
    // rather than panicking.
    let value = json!({"message": "Not Found"});
    let names = parse_root_tree(&value);
    assert!(names.is_empty());
}

#[test]
fn test_parse_root_tree_single_object_not_array() {
    // When a path points at a single file, the API returns an object, not an
    // array. The root-level endpoint always returns an array, but the parser
    // should still be robust to a non-array body.
    let value = json!({"name": "some_file", "type": "file"});
    let names = parse_root_tree(&value);
    assert!(names.is_empty());
}

#[test]
fn test_parse_root_tree_entries_without_name_skipped() {
    let value = json!([
        {"name": "keep_me", "type": "file"},
        {"type": "dir"}, // missing "name"
        {"name": 42, "type": "file"}, // name is not a string
        {"name": "also_keep", "type": "file"}
    ]);
    let names = parse_root_tree(&value);
    assert_eq!(names, vec!["keep_me".to_string(), "also_keep".to_string(),]);
}

#[test]
fn test_parse_root_tree_null_value() {
    let value = serde_json::Value::Null;
    let names = parse_root_tree(&value);
    assert!(names.is_empty());
}

#[test]
fn test_parse_root_tree_extra_fields_ignored() {
    // Entries may carry many fields beyond name/type; only "name" matters.
    let value = json!([
        {
            "name": "docs",
            "type": "dir",
            "path": "docs",
            "sha": "abc123",
            "size": 0,
            "url": "https://api.github.com/repos/o/r/contents/docs",
            "html_url": "https://github.com/o/r/tree/main/docs",
            "git_url": "https://api.github.com/repos/o/r/git/trees/main/docs",
            "download_url": null,
            "_links": {
                "self": "https://api.github.com/repos/o/r/contents/docs?ref=main",
                "git": "https://api.github.com/repos/o/r/git/trees/main/docs",
                "html": "https://github.com/o/r/tree/main/docs"
            }
        }
    ]);
    let names = parse_root_tree(&value);
    assert_eq!(names, vec!["docs".to_string()]);
}

//! External tests for `ragent_core::reference::fuzzy`.

use ragent_core::reference::fuzzy::{collect_project_files, fuzzy_match};
use std::path::PathBuf;

#[test]
fn test_collect_project_files_not_empty() {
    // The project root should have files
    let files = collect_project_files(std::path::Path::new("."), 100);
    assert!(!files.is_empty(), "project should have files");
}

#[test]
fn test_collect_project_files_max_cap() {
    let files = collect_project_files(std::path::Path::new("."), 5);
    assert!(files.len() <= 5, "should respect max cap");
}

#[test]
fn test_fuzzy_match_exact_basename() {
    let candidates = vec![
        PathBuf::from("src/main.rs"),
        PathBuf::from("src/lib.rs"),
        PathBuf::from("tests/main_test.rs"),
    ];
    let matches = fuzzy_match("main.rs", &candidates);
    assert!(!matches.is_empty());
    // Exact basename match should be first
    assert_eq!(matches[0].path, PathBuf::from("src/main.rs"));
    assert_eq!(matches[0].score, 100);
}

#[test]
fn test_fuzzy_match_prefix() {
    let candidates = vec![PathBuf::from("src/config.rs"), PathBuf::from("src/lib.rs")];
    let matches = fuzzy_match("con", &candidates);
    assert!(!matches.is_empty());
    assert_eq!(matches[0].path, PathBuf::from("src/config.rs"));
}

#[test]
fn test_fuzzy_match_case_insensitive() {
    let candidates = vec![PathBuf::from("src/Config.rs"), PathBuf::from("src/lib.rs")];
    let matches = fuzzy_match("config.rs", &candidates);
    assert!(!matches.is_empty());
    assert_eq!(matches[0].path, PathBuf::from("src/Config.rs"));
}

#[test]
fn test_fuzzy_match_no_match() {
    let candidates = vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")];
    let matches = fuzzy_match("zzzznotafile", &candidates);
    assert!(matches.is_empty());
}

#[test]
fn test_fuzzy_match_empty_query() {
    let candidates = vec![PathBuf::from("src/main.rs")];
    let matches = fuzzy_match("", &candidates);
    // Empty query should return all candidates
    assert!(!matches.is_empty());
}

#[test]
fn test_fuzzy_match_path_component() {
    let candidates = vec![
        PathBuf::from("crates/ragent-core/src/lib.rs"),
        PathBuf::from("src/main.rs"),
    ];
    let matches = fuzzy_match("ragent-core", &candidates);
    assert!(!matches.is_empty());
    assert_eq!(
        matches[0].path,
        PathBuf::from("crates/ragent-core/src/lib.rs")
    );
}

#[test]
fn test_fuzzy_match_sorted_by_score() {
    let candidates = vec![
        PathBuf::from("src/lib.rs"),
        PathBuf::from("src/main.rs"),
        PathBuf::from("tests/main_test.rs"),
    ];
    let matches = fuzzy_match("main", &candidates);
    assert!(matches.len() >= 2);
    // Higher score first
    assert!(matches[0].score >= matches[1].score);
}

#[test]
fn test_fuzzy_match_substring() {
    let candidates = vec![
        PathBuf::from("src/reference_parser.rs"),
        PathBuf::from("src/lib.rs"),
    ];
    let matches = fuzzy_match("parser", &candidates);
    assert!(!matches.is_empty());
    assert_eq!(matches[0].path, PathBuf::from("src/reference_parser.rs"));
}

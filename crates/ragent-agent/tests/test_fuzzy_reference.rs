#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-agent/src/reference/fuzzy.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_agent::reference::fuzzy::*;
use std::path::{Path, PathBuf};

fn candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("src/main.rs"),
        PathBuf::from("src/lib.rs"),
        PathBuf::from("src/config/mod.rs"),
        PathBuf::from("Cargo.toml"),
        PathBuf::from("README.md"),
        PathBuf::from("tests/test_main.rs"),
        PathBuf::from("src/reference/mod.rs"),
        PathBuf::from("src/reference/parse.rs"),
    ]
}

#[test]
fn test_exact_basename_match() {
    let results = fuzzy_match("main.rs", &candidates());
    assert!(!results.is_empty());
    assert_eq!(results[0].path, PathBuf::from("src/main.rs"));
    assert_eq!(results[0].score, 100);
}

#[test]
fn test_prefix_match() {
    let results = fuzzy_match("main", &candidates());
    assert!(!results.is_empty());
    // "main.rs" should match with prefix score
    assert!(results[0].score >= 75);
}

#[test]
fn test_substring_match() {
    let results = fuzzy_match("lib", &candidates());
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .any(|m| m.path == *std::path::Path::new("src/lib.rs"))
    );
}

#[test]
fn test_path_match() {
    let results = fuzzy_match("reference", &candidates());
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .any(|m| m.path == *std::path::Path::new("src/reference/mod.rs"))
    );
}

#[test]
fn test_case_insensitive() {
    let results = fuzzy_match("README", &candidates());
    assert!(!results.is_empty());
    assert_eq!(results[0].path, PathBuf::from("README.md"));
}

#[test]
fn test_no_match() {
    let results = fuzzy_match("nonexistent", &candidates());
    assert!(results.is_empty());
}

#[test]
fn test_empty_query() {
    let results = fuzzy_match("", &candidates());
    assert_eq!(results.len(), candidates().len());
}

#[test]
fn test_empty_candidates() {
    let results = fuzzy_match("main", &[]);
    assert!(results.is_empty());
}

#[test]
fn test_collect_project_files() {
    let tmp = std::env::temp_dir().join("ragent_test_fuzzy_collect");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src")).expect("mkdir");
    std::fs::write(tmp.join("src/main.rs"), "fn main() {}").expect("write");
    std::fs::write(tmp.join("Cargo.toml"), "[package]").expect("write");
    std::fs::create_dir_all(tmp.join(".git")).expect("mkdir .git");
    std::fs::write(tmp.join(".git/HEAD"), "ref").expect("write");

    let files = collect_project_files(&tmp, 100);
    assert!(files.iter().any(|p| p == Path::new("src/main.rs")));
    assert!(files.iter().any(|p| p == Path::new("Cargo.toml")));
    // Directories should be included with trailing /
    assert!(files.iter().any(|p| p == Path::new("src/")));
    // .git should be skipped
    assert!(!files.iter().any(|p| p.starts_with(".git")));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_score_ordering() {
    let candidates = vec![
        PathBuf::from("src/lib/main.rs"),    // path match for "main"
        PathBuf::from("src/main_helper.rs"), // prefix match
        PathBuf::from("src/main.rs"),        // exact basename match
    ];
    let results = fuzzy_match("main.rs", &candidates);
    assert_eq!(results[0].path, PathBuf::from("src/main.rs"));
}

#[test]
fn test_collect_project_files_cache_shares_full_list() {
    let tmp = std::env::temp_dir().join("ragent_test_fuzzy_cache_share");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src")).unwrap();
    std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();
    std::fs::write(tmp.join("src/main.rs"), "").unwrap();
    std::fs::write(tmp.join("src/lib.rs"), "").unwrap();

    // First call with a small max should still cache the full tree.
    let small = collect_project_files(&tmp, 1);
    assert_eq!(small.len(), 1);

    // Second call with a larger max should see the full cached tree.
    let full = collect_project_files(&tmp, 100);
    assert!(full.len() > 1);
    let names: Vec<_> = full
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    assert!(names.iter().any(|n| n.contains("src/main.rs")));
    assert!(names.iter().any(|n| n == "Cargo.toml"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_collect_project_files_cache_invalidates_on_mtime_change() {
    let tmp = std::env::temp_dir().join("ragent_test_fuzzy_cache_mtime");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("first.rs"), "").unwrap();

    let first = collect_project_files(&tmp, 100);
    assert!(
        first
            .iter()
            .any(|p| p.to_string_lossy().contains("first.rs"))
    );

    // Adding a file updates the directory mtime on most filesystems, which
    // should invalidate the cached list on the next call.
    std::fs::write(tmp.join("second.rs"), "").unwrap();

    let second = collect_project_files(&tmp, 100);
    assert!(
        second
            .iter()
            .any(|p| p.to_string_lossy().contains("second.rs")),
        "cache should be invalidated after directory mtime change"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

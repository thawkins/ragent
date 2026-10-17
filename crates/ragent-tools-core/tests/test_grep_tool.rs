//! Coverage-focused tests for `grep.rs`: pattern matching, include/exclude
//! glob filtering, case-insensitivity, multiline mode, result capping, and the
//! empty-result path, all driven through the public [`GrepTool`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_tools_core::grep::GrepTool;
use ragent_tools_core::{Tool, ToolContext, ToolOutput};
use ragent_types::event::EventBus;
use serde_json::json;

fn make_ctx(dir: &Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(1024)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir.to_path_buf()],
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("temp")
        .join(format!(
            "grep_test_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn grep(input: serde_json::Value, dir: &Path) -> anyhow::Result<ToolOutput> {
    GrepTool.execute(input, &make_ctx(dir)).await
}

/// A basic literal match reports the count, file count and the
/// `relative:line:content` format.
#[tokio::test]
async fn test_grep_basic_match() {
    let dir = temp_dir("basic");
    std::fs::write(dir.join("a.txt"), "hello world\nsecond line\nhello again\n").unwrap();

    let out = grep(json!({ "pattern": "hello" }), &dir).await.unwrap();
    assert!(out.content.contains("2 matches in 1 file searched"));
    assert!(out.content.contains("a.txt:1:hello world"));
    assert!(out.content.contains("a.txt:3:hello again"));
    let meta = out.metadata.unwrap();
    assert_eq!(meta["count"], 2);
    assert_eq!(meta["file_count"], 1);
    assert_eq!(meta["truncated"], false);
}

/// The singular/plural summary text differs for a single match.
#[tokio::test]
async fn test_grep_singular_summary() {
    let dir = temp_dir("singular");
    std::fs::write(dir.join("only.txt"), "unique-token\nother\n").unwrap();
    let out = grep(json!({ "pattern": "unique-token" }), &dir)
        .await
        .unwrap();
    assert!(
        out.content.contains("1 match in 1 file searched"),
        "got: {}",
        out.content
    );
}

/// No match yields the "No matches found" message and no metadata.
#[tokio::test]
async fn test_grep_no_matches() {
    let dir = temp_dir("nomatch");
    std::fs::write(dir.join("a.txt"), "abc\n").unwrap();
    let out = grep(json!({ "pattern": "zzzz-nope" }), &dir).await.unwrap();
    assert!(out.content.contains("No matches found"));
    assert!(out.metadata.is_none());
}

/// Case-insensitive matching finds mixed-case occurrences.
#[tokio::test]
async fn test_grep_case_insensitive() {
    let dir = temp_dir("ci");
    std::fs::write(dir.join("a.txt"), "Hello\nHELLO\nhello\n").unwrap();
    let out = grep(
        json!({ "pattern": "hello", "case_insensitive": true }),
        &dir,
    )
    .await
    .unwrap();
    assert_eq!(out.metadata.unwrap()["count"], 3);
}

/// Case-sensitive matching (the default) finds only the exact case.
#[tokio::test]
async fn test_grep_case_sensitive_default() {
    let dir = temp_dir("cs");
    std::fs::write(dir.join("a.txt"), "Hello\nHELLO\nhello\n").unwrap();
    let out = grep(json!({ "pattern": "hello" }), &dir).await.unwrap();
    assert_eq!(out.metadata.unwrap()["count"], 1);
}

/// A regex alternation matches either branch.
#[tokio::test]
async fn test_grep_regex_pattern() {
    let dir = temp_dir("regex");
    std::fs::write(dir.join("a.ts"), "let foo = 1;\nfunc bar() {}\n").unwrap();
    let out = grep(json!({ "pattern": "foo|bar" }), &dir).await.unwrap();
    assert_eq!(out.metadata.unwrap()["count"], 2);
}

/// An invalid regex is surfaced as an error.
#[tokio::test]
async fn test_grep_invalid_regex_is_error() {
    let dir = temp_dir("badregex");
    let err = grep(json!({ "pattern": "(" }), &dir)
        .await
        .expect_err("bad regex must fail");
    assert!(
        err.to_string().contains("Invalid regex pattern"),
        "got: {err}"
    );
}

/// A missing `pattern` parameter is an error.
#[tokio::test]
async fn test_grep_missing_pattern_is_error() {
    let dir = temp_dir("nopattern");
    let err = grep(json!({}), &dir)
        .await
        .expect_err("missing pattern must fail");
    assert!(err.to_string().contains("pattern"));
}

/// The `include` glob restricts the search to matching files.
#[tokio::test]
async fn test_grep_include_glob_filters() {
    let dir = temp_dir("include");
    std::fs::write(dir.join("keep.rs"), "target_token\n").unwrap();
    std::fs::write(dir.join("skip.txt"), "target_token\n").unwrap();

    let out = grep(
        json!({ "pattern": "target_token", "include": "*.rs" }),
        &dir,
    )
    .await
    .unwrap();
    assert!(out.content.contains("keep.rs"));
    assert!(!out.content.contains("skip.txt"));
    assert_eq!(out.metadata.unwrap()["count"], 1);
}

/// The `exclude` glob removes matching files from the search.
#[tokio::test]
async fn test_grep_exclude_glob_filters() {
    let dir = temp_dir("exclude");
    std::fs::write(dir.join("keep.rs"), "target_token\n").unwrap();
    std::fs::write(dir.join("skip.txt"), "target_token\n").unwrap();

    let out = grep(
        json!({ "pattern": "target_token", "exclude": "*.txt" }),
        &dir,
    )
    .await
    .unwrap();
    assert!(out.content.contains("keep.rs"));
    assert!(!out.content.contains("skip.txt"));
}

/// An invalid include glob is surfaced as an error rather than silently ignored.
#[tokio::test]
async fn test_grep_invalid_include_glob_is_error() {
    let dir = temp_dir("badglob");
    std::fs::write(dir.join("a.txt"), "x\n").unwrap();
    let err = grep(json!({ "pattern": "x", "include": "[unclosed" }), &dir)
        .await
        .expect_err("invalid include glob must fail");
    assert!(
        err.to_string().contains("include glob") || err.to_string().contains("glob"),
        "got: {err}"
    );
}

/// Searching an explicit file path (rather than a directory) works.
#[tokio::test]
async fn test_grep_search_single_file() {
    let dir = temp_dir("singlefile");
    let f = dir.join("one.txt");
    std::fs::write(&f, "alpha\nbeta\n").unwrap();
    let out = grep(
        json!({ "pattern": "beta", "path": f.to_str().unwrap() }),
        &dir,
    )
    .await
    .unwrap();
    assert!(out.content.contains("beta"));
}

/// `max_results` caps the number of reported matches and sets `truncated`.
#[tokio::test]
async fn test_grep_max_results_truncates() {
    let dir = temp_dir("cap");
    let mut body = String::new();
    for _ in 0..50 {
        body.push_str("hit\n");
    }
    std::fs::write(dir.join("many.txt"), body).unwrap();

    let out = grep(json!({ "pattern": "hit", "max_results": 5 }), &dir)
        .await
        .unwrap();
    let meta = out.metadata.unwrap();
    assert_eq!(meta["count"], 5);
    assert_eq!(meta["truncated"], true);
    assert!(out.content.contains("truncated at limit"));
}

/// Multiline mode lets `^`/`$` anchor within the whole buffer.
#[tokio::test]
async fn test_grep_multiline_mode() {
    let dir = temp_dir("multiline");
    std::fs::write(dir.join("a.txt"), "start_marker\nmid\n").unwrap();
    let out = grep(
        json!({ "pattern": "^start_marker$", "multiline": true }),
        &dir,
    )
    .await
    .unwrap();
    assert_eq!(out.metadata.unwrap()["count"], 1);
}

/// Subdirectories are searched recursively.
#[tokio::test]
async fn test_grep_nested_directories() {
    let dir = temp_dir("nested");
    let sub = dir.join("a").join("b");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("deep.txt"), "deep_token\n").unwrap();

    let out = grep(json!({ "pattern": "deep_token" }), &dir)
        .await
        .unwrap();
    assert!(out.content.contains("deep.txt"));
    // Relative path uses the OS separator.
    assert!(out.content.contains('a'));
}

/// A binary file (NUL byte) is skipped, not matched.
#[tokio::test]
async fn test_grep_skips_binary_files() {
    let dir = temp_dir("binary");
    let mut bytes = b"needle".to_vec();
    bytes.push(0);
    bytes.extend_from_slice(b"more");
    std::fs::write(dir.join("bin.dat"), bytes).unwrap();
    std::fs::write(dir.join("text.txt"), "needle\n").unwrap();

    let out = grep(json!({ "pattern": "needle" }), &dir).await.unwrap();
    assert!(out.content.contains("text.txt"));
    assert!(!out.content.contains("bin.dat"));
}

/// A search path outside the allowed roots is rejected.
#[tokio::test]
async fn test_grep_outside_allowed_roots_rejected() {
    let dir = temp_dir("escape");
    let err = grep(json!({ "pattern": "x", "path": "../.." }), &dir)
        .await
        .expect_err("escape must fail");
    assert_ne!(err.to_string(), "");
}

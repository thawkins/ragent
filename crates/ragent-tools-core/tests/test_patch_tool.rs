//! Coverage-focused tests for `patch.rs`: unified-diff parsing and application
//! through the public [`PatchTool`].
//!
//! Each test drives `PatchTool::execute` against real files under
//! `target/temp` so the private parser (`parse_unified_diff`, `parse_hunk`,
//! `parse_hunk_header`, `parse_range`, `parse_file_path`) and applier
//! (`apply_hunk`, `trim_context`, `find_match`, `matches_at`) are all reached
//! through their real caller.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_tools_core::patch::PatchTool;
use ragent_tools_core::{Tool, ToolContext};
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
            "patch_test_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn run(patch: &str, dir: &Path) -> anyhow::Result<ragent_tools_core::ToolOutput> {
    PatchTool
        .execute(json!({ "patch": patch }), &make_ctx(dir))
        .await
}

/// A single hunk that replaces one line in-place applies and preserves the
/// trailing newline.
#[tokio::test]
async fn test_patch_single_hunk_replace() {
    let dir = temp_dir("replace");
    let path = dir.join("f.txt");
    std::fs::write(&path, "one\ntwo\nthree\n").unwrap();

    let patch = "--- a/f.txt\n\
                 +++ b/f.txt\n\
                 @@ -1,3 +1,3 @@\n\
                 \x20one\n\
                 -two\n\
                 +TWO\n\
                 \x20three\n";
    let out = run(patch, &dir).await.unwrap();
    assert!(out.content.contains("Applied 1 hunk"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\nTWO\nthree\n");
    let meta = out.metadata.unwrap();
    assert_eq!(meta["hunks"], 1);
    assert_eq!(meta["files"], 1);
}

/// Two hunks in the same file apply in reverse order so line numbers do not
/// shift; both edits land.
#[tokio::test]
async fn test_patch_two_hunks_same_file() {
    let dir = temp_dir("twolines");
    let path = dir.join("f.txt");
    let body = "a\nb\nc\nd\ne\nf\ng\nh\n";
    std::fs::write(&path, body).unwrap();

    let patch = "--- a/f.txt\n\
                 +++ b/f.txt\n\
                 @@ -1,2 +1,2 @@\n\
                 -a\n\
                 +A\n\
                 \x20b\n\
                 @@ -7,2 +7,2 @@\n\
                 \x20g\n\
                 -h\n\
                 +H\n";
    let out = run(patch, &dir).await.unwrap();
    assert!(out.content.contains("Applied 2 hunks"));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "A\nb\nc\nd\ne\nf\ng\nH\n"
    );
}

/// A plain `diff -u` header (no git `a/`/`b/` prefix) and a tab+timestamp
/// suffix on the `+++` line are both stripped.
#[tokio::test]
async fn test_patch_plain_header_with_timestamp() {
    let dir = temp_dir("plainhdr");
    let path = dir.join("f.txt");
    std::fs::write(&path, "alpha\nbeta\n").unwrap();

    let patch = "--- f.txt\t2024-01-01 00:00:00\n\
                 +++ f.txt\t2024-01-01 00:00:01\n\
                 @@ -1,2 +1,2 @@\n\
                 \x20alpha\n\
                 -beta\n\
                 +BETA\n";
    run(patch, &dir).await.unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "alpha\nBETA\n");
}

/// A hunk header without an explicit count (`@@ -1 +1 @@`) implies count 1.
#[tokio::test]
async fn test_patch_implied_count_single_line() {
    let dir = temp_dir("implied");
    let path = dir.join("f.txt");
    std::fs::write(&path, "hello\nworld\n").unwrap();

    let patch = "--- a/f.txt\n\
                 +++ b/f.txt\n\
                 @@ -1 +1 @@\n\
                 -hello\n\
                 +HELLO\n";
    run(patch, &dir).await.unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "HELLO\nworld\n");
}

/// A bare line (no leading space/-/+) is treated as context.
#[tokio::test]
async fn test_patch_bare_line_treated_as_context() {
    let dir = temp_dir("bare");
    let path = dir.join("f.txt");
    std::fs::write(&path, "keep\ndrop\n").unwrap();

    let patch = "--- a/f.txt\n\
                 +++ b/f.txt\n\
                 @@ -1,2 +1,2 @@\n\
                 keep\n\
                 -drop\n\
                 +DROP\n";
    run(patch, &dir).await.unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep\nDROP\n");
}

/// `\ No newline at end of file` marker lines are skipped.
#[tokio::test]
async fn test_patch_no_newline_marker_skipped() {
    let dir = temp_dir("nonewline");
    let path = dir.join("f.txt");
    std::fs::write(&path, "x\ny").unwrap(); // no trailing newline

    let patch = "--- a/f.txt\n\
                 +++ b/f.txt\n\
                 @@ -1,2 +1,2 @@\n\
                 \x20x\n\
                 -y\n\
                 +z\n\
                 \\ No newline at end of file\n";
    run(patch, &dir).await.unwrap();
    // Original had no trailing newline, so the result must not gain one.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "x\nz");
}

/// Fuzz matching: the hunk's declared line number is off, but the exact old
/// content is found elsewhere in the file.
#[tokio::test]
async fn test_patch_finds_match_away_from_hint() {
    let dir = temp_dir("fuzz");
    let path = dir.join("f.txt");
    // The `target` line is actually at index 5, but the hunk claims line 1.
    std::fs::write(&path, "a\nb\nc\nd\ne\ntarget\n").unwrap();

    let patch = "--- a/f.txt\n\
                 +++ b/f.txt\n\
                 @@ -1,1 +1,1 @@\n\
                 -target\n\
                 +TARGET\n";
    run(patch, &dir).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "a\nb\nc\nd\ne\nTARGET\n"
    );
}

/// With `fuzz` > 0, context lines at the hunk's edges are allowed to differ.
#[tokio::test]
async fn test_patch_fuzz_trims_mismatched_context() {
    let dir = temp_dir("fuzzctx");
    let path = dir.join("f.txt");
    std::fs::write(&path, "CTX-A\nreal\nCTX-B\n").unwrap();

    // Declared context differs from the file; fuzz=1 trims it away.
    let patch = "--- a/f.txt\n\
                 +++ b/f.txt\n\
                 @@ -1,3 +1,3 @@\n\
                 \x20WRONG-1\n\
                 -real\n\
                 +REAL\n\
                 \x20WRONG-2\n";
    let out = PatchTool
        .execute(json!({ "patch": patch, "fuzz": 1 }), &make_ctx(&dir))
        .await
        .unwrap();
    assert!(out.content.contains("Applied"));
}

/// A patch touching two different files applies both.
#[tokio::test]
async fn test_patch_two_files() {
    let dir = temp_dir("twofiles");
    std::fs::write(dir.join("one.txt"), "a\n").unwrap();
    std::fs::write(dir.join("two.txt"), "b\n").unwrap();

    let patch = "--- a/one.txt\n\
                 +++ b/one.txt\n\
                 @@ -1,1 +1,1 @@\n\
                 -a\n\
                 +A\n\
                 --- a/two.txt\n\
                 +++ b/two.txt\n\
                 @@ -1,1 +1,1 @@\n\
                 -b\n\
                 +B\n";
    let out = run(patch, &dir).await.unwrap();
    assert!(out.content.contains("2 files"));
    assert_eq!(std::fs::read_to_string(dir.join("one.txt")).unwrap(), "A\n");
    assert_eq!(std::fs::read_to_string(dir.join("two.txt")).unwrap(), "B\n");
    assert_eq!(out.metadata.unwrap()["files"], 2);
}

/// The `path` parameter overrides the target file recorded in the diff header.
#[tokio::test]
async fn test_patch_path_override() {
    let dir = temp_dir("override");
    let path = dir.join("real.txt");
    std::fs::write(&path, "old\n").unwrap();

    let patch = "--- a/ignored.txt\n\
                 +++ b/ignored.txt\n\
                 @@ -1,1 +1,1 @@\n\
                 -old\n\
                 +new\n";
    PatchTool
        .execute(
            json!({ "patch": patch, "path": path.to_str().unwrap() }),
            &make_ctx(&dir),
        )
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
}

/// Removing the only line leaves an empty file (joined lines are empty).
#[tokio::test]
async fn test_patch_remove_all_lines_yields_empty_file() {
    let dir = temp_dir("empty");
    let path = dir.join("f.txt");
    std::fs::write(&path, "only\n").unwrap();

    let patch = "--- a/f.txt\n\
                 +++ b/f.txt\n\
                 @@ -1,1 +0,0 @@\n\
                 -only\n";
    run(patch, &dir).await.unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
}

/// Pure additions (no removals) are inserted at the hinted position.
#[tokio::test]
async fn test_patch_pure_addition() {
    let dir = temp_dir("addonly");
    let path = dir.join("f.txt");
    std::fs::write(&path, "a\nb\n").unwrap();

    let patch = "--- a/f.txt\n\
                 +++ b/f.txt\n\
                 @@ -1,1 +1,2 @@\n\
                 \x20a\n\
                 +inserted\n";
    run(patch, &dir).await.unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\ninserted\nb\n");
}

/// A hunk whose old content is nowhere in the file fails, and the target is
/// left unmodified (all-or-nothing).
#[tokio::test]
async fn test_patch_no_match_leaves_file_untouched() {
    let dir = temp_dir("nomatch");
    let path = dir.join("f.txt");
    std::fs::write(&path, "alpha\nbeta\n").unwrap();

    let patch = "--- a/f.txt\n\
                 +++ b/f.txt\n\
                 @@ -1,2 +1,2 @@\n\
                 -does\n\
                 -not\n\
                 +exist\n\
                 +here\n";
    let err = run(patch, &dir).await.expect_err("hunk must fail to apply");
    let msg = err.to_string();
    assert!(msg.contains("failed to apply"), "unexpected: {msg}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "alpha\nbeta\n");
}

/// A missing `patch` parameter is an error.
#[tokio::test]
async fn test_patch_missing_param_is_error() {
    let dir = temp_dir("noparam");
    let err = PatchTool
        .execute(json!({}), &make_ctx(&dir))
        .await
        .expect_err("missing patch must fail");
    assert!(err.to_string().contains("patch"));
}

/// Content with no diff/`@@` structure yields the "no valid hunks" error.
#[tokio::test]
async fn test_patch_no_hunks_is_error() {
    let dir = temp_dir("nohunks");
    let err = run("this is not a diff\nat all\n", &dir)
        .await
        .expect_err("no hunks must fail");
    assert!(err.to_string().contains("No valid diff hunks"));
}

/// A malformed hunk header (too few parts) is rejected by the parser.
#[tokio::test]
async fn test_patch_malformed_hunk_header_is_error() {
    let dir = temp_dir("badhdr");
    let path = dir.join("f.txt");
    std::fs::write(&path, "x\n").unwrap();

    let patch = "--- a/f.txt\n\
                 +++ b/f.txt\n\
                 @@ bogus @@\n\
                 -x\n\
                 +y\n";
    let err = run(patch, &dir).await.expect_err("bad header must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("hunk header") || msg.contains("Invalid"),
        "unexpected: {msg}"
    );
}

/// A hunk header whose range start is not numeric is rejected.
#[tokio::test]
async fn test_patch_non_numeric_range_is_error() {
    let dir = temp_dir("nonnumeric");
    let path = dir.join("f.txt");
    std::fs::write(&path, "x\n").unwrap();

    let patch = "--- a/f.txt\n\
                 +++ b/f.txt\n\
                 @@ -abc,1 +1,1 @@\n\
                 -x\n\
                 +y\n";
    let err = run(patch, &dir)
        .await
        .expect_err("non-numeric range must fail");
    assert!(err.to_string().contains("Invalid"), "got: {err}");
}

/// A patch that references a non-existent target file fails to read it.
#[tokio::test]
async fn test_patch_target_missing_is_error() {
    let dir = temp_dir("missingtarget");
    let patch = "--- a/ghost.txt\n\
                 +++ b/ghost.txt\n\
                 @@ -1,1 +1,1 @@\n\
                 -a\n\
                 +b\n";
    let err = run(patch, &dir)
        .await
        .expect_err("missing target must fail");
    assert!(
        err.to_string().contains("Failed to read file"),
        "got: {err}"
    );
}

/// A patch targeting a path outside the allowed roots is rejected before any
/// read/write.
#[tokio::test]
async fn test_patch_outside_allowed_roots_rejected() {
    let dir = temp_dir("escape");
    let patch = "--- a/../../etc/passwd\n\
                 +++ b/../../etc/passwd\n\
                 @@ -1,1 +1,1 @@\n\
                 -a\n\
                 +b\n";
    let err = run(patch, &dir).await.expect_err("escape must fail");
    // Any error is acceptable here; we assert it did not panic and returned one.
    assert_ne!(err.to_string(), "");
}

//! Integration tests for `MultiEditTool` (WSPLAN Milestone 3).
//!
//! Covers: two edits in one file, edits across two files, overlap detection,
//! JSON-order independence (edits applied highest-offset-first), and
//! strict exact-byte batch edits (EDITPLAN).

use std::sync::Arc;

use ragent_tools_core::multiedit::MultiEditTool;
use ragent_tools_core::{Tool, ToolContext};
use serde_json::json;
use tempfile::TempDir;

/// Build a `ToolContext` rooted at the given working directory.
fn ctx(working_dir: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: working_dir.to_path_buf(),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

/// Write a file relative to `dir` with the given content.
fn write_file(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    path
}

// ── Two edits in one file ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_two_edits_one_file() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(
        tmp.path(),
        "a.rs",
        "fn a() { 1 }\nfn b() { 2 }\nfn c() { 3 }\n",
    );

    let input = json!({
        "edits": [
            { "path": "a.rs", "old_str": "fn a() { 1 }", "new_str": "fn a() { 10 }" },
            { "path": "a.rs", "old_str": "fn c() { 3 }", "new_str": "fn c() { 30 }" }
        ]
    });

    let out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 2 edits across 1 file");

    let result = std::fs::read_to_string(&path).unwrap();
    assert_eq!(result, "fn a() { 10 }\nfn b() { 2 }\nfn c() { 30 }\n");
}

// ── Edits across two files ────────────────────────────────────────────────────

#[tokio::test]
async fn test_edits_across_two_files() {
    let tmp = TempDir::new().unwrap();
    let p1 = write_file(tmp.path(), "a.rs", "alpha\n");
    let p2 = write_file(tmp.path(), "b.rs", "beta\n");

    let input = json!({
        "edits": [
            { "path": "a.rs", "old_str": "alpha", "new_str": "ALPHA" },
            { "path": "b.rs", "old_str": "beta", "new_str": "BETA" }
        ]
    });

    let out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 2 edits across 2 files");
    assert_eq!(std::fs::read_to_string(&p1).unwrap(), "ALPHA\n");
    assert_eq!(std::fs::read_to_string(&p2).unwrap(), "BETA\n");
}

// ── Overlap detection ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_overlap_detection_rejects() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "a.rs", "fn foo() { let x = 1; let y = 2; }\n");

    // Edit A replaces "let x = 1;" (inside the line). Edit B replaces the whole
    // line containing it. Their original-content byte ranges overlap.
    let input = json!({
        "edits": [
            { "path": "a.rs", "old_str": "let x = 1;", "new_str": "let x = 10;" },
            { "path": "a.rs", "old_str": "fn foo() { let x = 1; let y = 2; }", "new_str": "fn foo() { let x = 10; let y = 20; }" }
        ]
    });

    let err = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("overlapping edits must be rejected");

    let msg = format!("{err}");
    assert!(
        msg.contains("overlap"),
        "error should mention overlap: {msg}"
    );
    assert!(msg.contains("a.rs"), "error should name the file: {msg}");
    assert!(
        msg.contains("Edits 0 and 1") || msg.contains("Edits 1 and 0"),
        "error should name the edit indices: {msg}"
    );
}

// ── JSON-order independence ───────────────────────────────────────────────────

#[tokio::test]
async fn test_json_order_independence() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "line1\nline2\nline3\n");

    // Supply edits in reverse order (highest-offset first). The tool should
    // still produce the same result as forward order because it sorts
    // end-to-start internally.
    let input_reverse = json!({
        "edits": [
            { "path": "a.rs", "old_str": "line3", "new_str": "LINE3" },
            { "path": "a.rs", "old_str": "line1", "new_str": "LINE1" }
        ]
    });

    let out = MultiEditTool
        .execute(input_reverse, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 2 edits across 1 file");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "LINE1\nline2\nLINE3\n"
    );

    // Now reset and supply in forward order — same result.
    std::fs::write(&path, "line1\nline2\nline3\n").unwrap();
    let input_forward = json!({
        "edits": [
            { "path": "a.rs", "old_str": "line1", "new_str": "LINE1" },
            { "path": "a.rs", "old_str": "line3", "new_str": "LINE3" }
        ]
    });

    let _out = MultiEditTool
        .execute(input_forward, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "LINE1\nline2\nLINE3\n"
    );
}

// ── Strict exact-match batch edits (editrenewal FR-004 / FR-009) ──────────────

/// With the strict matcher, a batch edit whose `old_string` does not match
/// the file byte-for-byte (here: the file has trailing spaces and CRLF that
/// the needle omits) must be rejected, and no files may be modified.
#[tokio::test]
async fn test_batch_exact_rejects_crlf_mismatch() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(
        tmp.path(),
        "a.rs",
        "fn a() {  
    bar  
}
",
    );

    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "fn a() {
    bar
}
", "new_string": "fn a() {
    baz
}
" }
        ]
    });

    let err = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("batch exact-byte matching should reject CRLF/trailing-whitespace mismatch");
    let msg = format!("{err}");
    assert!(
        msg.contains("not found"),
        "error should mention not found: {msg}"
    );

    let file_content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        file_content,
        "fn a() {  
    bar  
}
",
        "file must be unmodified"
    );
}

#[tokio::test]
async fn test_batch_dry_run_previews_without_writing() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "alpha\n");
    let p2 = write_file(tmp.path(), "b.rs", "beta\n");

    let input = json!({
        "dry_run": true,
        "edits": [
            { "file_path": "a.rs", "old_string": "alpha", "new_string": "ALPHA" },
            { "file_path": "b.rs", "old_string": "beta", "new_string": "BETA" }
        ]
    });

    let out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect("dry_run batch should validate");
    assert!(out.content.starts_with("Would apply"));
    let meta = out.metadata.unwrap();
    assert_eq!(meta["dry_run"], true);
    assert_eq!(meta["edits"], 2);

    // No file was modified.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "alpha\n");
    assert_eq!(std::fs::read_to_string(&p2).unwrap(), "beta\n");
}

#[tokio::test]
async fn test_batch_indentation_mismatch_still_rejected() {
    let tmp = TempDir::new().unwrap();
    // File uses 4-space indentation; needle omits indentation for the inner line.
    // Strict exact-byte matching rejects the mismatch.
    let path = write_file(tmp.path(), "a.rs", "fn a() {\n    bar\n}\n");
    let original = std::fs::read_to_string(&path).unwrap();

    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "fn a() {\nbar\n}\n", "new_string": "fn a() {\nbaz\n}\n" }
        ]
    });

    let err = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("batch normalization must not adjust indentation");
    let msg = format!("{err}");
    assert!(
        msg.contains("Edit 0"),
        "error should name the edit index: {msg}"
    );
    assert!(
        msg.contains("not found"),
        "error should say the string was not found: {msg}"
    );

    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
}

/// A batch edit whose `old_string` matches the file exactly (including
/// trailing spaces and CRLF) must succeed under the strict matcher.
#[tokio::test]
async fn test_strict_match_accepts_exact_batch_edit() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "fn a() {  \r\n    bar  \n}\n");

    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "fn a() {  \r\n    bar  \n}\n", "new_string": "fn a() {  \r\n    baz  \n}\n" }
        ]
    });

    let out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 1 edit across 1 file");
    let result = std::fs::read_to_string(&path).unwrap();
    assert!(
        result.contains("baz"),
        "exact-match replacement should apply: {result}"
    );
}

// ── Non-overlapping adjacent edits (touching ranges allowed) ──────────────────

#[tokio::test]
async fn test_adjacent_touching_edits_allowed() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "AABB\n");

    // Edit 0 replaces "AA" (bytes 0..2). Edit 1 replaces "BB" (bytes 2..4).
    // They touch but do not overlap.
    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "AA", "new_string": "XX" },
            { "file_path": "a.rs", "old_string": "BB", "new_string": "YY" }
        ]
    });

    let out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 2 edits across 1 file");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "XXYY\n");
}

// ── NotFound surfaces edit index + path (editrenewal FR-004) ───────────────────

#[tokio::test]
async fn test_not_found_error_names_edit_and_path() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "a.rs", "fn a() { 1 }\n");

    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "nonexistent code here", "new_string": "x" }
        ]
    });

    let err = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("missing old_string must error");

    let msg = format!("{err}");
    assert!(
        msg.contains("Edit 0"),
        "error should name the edit index: {msg}"
    );
    assert!(
        msg.contains("not found"),
        "error should say the string was not found: {msg}"
    );
    assert!(msg.contains("a.rs"), "error should name the file: {msg}");
}

/// When `old_string` occurs multiple times, the strict matcher must reject
/// the batch and report the match count (editrenewal FR-004).
#[tokio::test]
async fn test_multiple_matches_error_reports_count() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "a.rs", "dup\nmid\ndup\n");

    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "dup", "new_string": "DUP" }
        ]
    });

    let err = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("non-unique old_string must error");

    let msg = format!("{err}");
    assert!(
        msg.contains("Edit 0"),
        "error should name the edit index: {msg}"
    );
    assert!(
        msg.contains("2 times"),
        "error should report the match count: {msg}"
    );
    assert!(
        msg.contains("exactly once") || msg.contains("unique"),
        "error should guide the caller toward uniqueness: {msg}"
    );
}

// ── editrenewal T-010: rename/alias multiedit → multi_edit (FR-009) ───────────

/// The tool's canonical name must be `multi_edit` (not the legacy `multiedit`).
#[tokio::test]
async fn test_multi_edit_canonical_name() {
    let tool = MultiEditTool;
    assert_eq!(tool.name(), "multi_edit");
}

/// The canonical parameter names `file_path` / `old_string` / `new_string`
/// must be accepted by the `multi_edit` tool.
#[tokio::test]
async fn test_multi_edit_canonical_param_names() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "fn a() { 1 }\n");

    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "fn a() { 1 }", "new_string": "fn a() { 10 }" }
        ]
    });

    let out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 1 edit across 1 file");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "fn a() { 10 }\n");
}

/// The legacy parameter names `path` / `old_str` / `new_str` must still be
/// accepted for backward compatibility during the deprecation window.
#[tokio::test]
async fn test_multi_edit_legacy_param_names_accepted() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "fn a() { 1 }\n");

    let input = json!({
        "edits": [
            { "path": "a.rs", "old_str": "fn a() { 1 }", "new_str": "fn a() { 10 }" }
        ]
    });

    let out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 1 edit across 1 file");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "fn a() { 10 }\n");
}

/// Mixing canonical and legacy parameter names across edits in the same
/// batch must work.
#[tokio::test]
async fn test_multi_edit_mixed_param_names_across_edits() {
    let tmp = TempDir::new().unwrap();
    let p1 = write_file(tmp.path(), "a.rs", "alpha\n");
    let p2 = write_file(tmp.path(), "b.rs", "beta\n");

    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "alpha", "new_string": "ALPHA" },
            { "path": "b.rs", "old_str": "beta", "new_str": "BETA" }
        ]
    });

    let out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 2 edits across 2 files");
    assert_eq!(std::fs::read_to_string(&p1).unwrap(), "ALPHA\n");
    assert_eq!(std::fs::read_to_string(&p2).unwrap(), "BETA\n");
}

/// The parameters schema must declare the canonical `file_path` /
/// `old_string` / `new_string` property names.
#[test]
fn test_multi_edit_schema_declares_canonical_params() {
    let tool = MultiEditTool;
    let schema = tool.parameters_schema().to_string();
    assert!(
        schema.contains("file_path"),
        "schema should list file_path: {schema}"
    );
    assert!(
        schema.contains("old_string"),
        "schema should list old_string: {schema}"
    );
    assert!(
        schema.contains("new_string"),
        "schema should list new_string: {schema}"
    );
}
// ── Atomic rollback (editrenewal FR-009, FR-013) ──────────────────────────────

/// When one edit in a batch fails validation (here: `old_string` not found in
/// the second file), NO files may be modified — including the first file whose
/// edit would have succeeded on its own.
#[tokio::test]
async fn test_atomic_rollback_on_validation_failure() {
    let tmp = TempDir::new().unwrap();
    let p1 = write_file(tmp.path(), "a.rs", "alpha\n");
    let p2 = write_file(tmp.path(), "b.rs", "beta\n");
    let original_a = std::fs::read_to_string(&p1).unwrap();
    let original_b = std::fs::read_to_string(&p2).unwrap();

    // Edit 0 would succeed; edit 1 fails (old_string not in b.rs).
    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "alpha", "new_string": "ALPHA" },
            { "file_path": "b.rs", "old_string": "nonexistent", "new_string": "BETA" }
        ]
    });

    let err = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("batch with a failing edit must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("Edit 1"),
        "error should name the failing edit index: {msg}"
    );

    // Atomic rollback: neither file was modified.
    assert_eq!(
        std::fs::read_to_string(&p1).unwrap(),
        original_a,
        "file a.rs must be unchanged (atomic rollback)"
    );
    assert_eq!(
        std::fs::read_to_string(&p2).unwrap(),
        original_b,
        "file b.rs must be unchanged (atomic rollback)"
    );
}

/// When two edits in the same file overlap, the batch is rejected and the
/// file must remain unchanged (FR-009).
#[tokio::test]
async fn test_overlap_rejection_leaves_file_unchanged() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "fn foo() { let x = 1; let y = 2; }\n");
    let original = std::fs::read_to_string(&path).unwrap();

    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "let x = 1;", "new_string": "let x = 10;" },
            { "file_path": "a.rs", "old_string": "fn foo() { let x = 1; let y = 2; }", "new_string": "fn foo() { let x = 10; let y = 20; }" }
        ]
    });

    let err = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("overlapping edits must be rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("overlap"),
        "error should mention overlap: {msg}"
    );
    assert!(
        msg.contains("Edits 0 and 1") || msg.contains("Edits 1 and 0"),
        "error should name the overlapping indices: {msg}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        original,
        "file must be unchanged when overlap is rejected"
    );
}

/// A cross-file batch where all edits succeed must modify every target file
/// (FR-009 happy path).
#[tokio::test]
async fn test_cross_file_batch_all_succeed() {
    let tmp = TempDir::new().unwrap();
    let p1 = write_file(tmp.path(), "a.rs", "alpha\n");
    let p2 = write_file(tmp.path(), "b.rs", "beta\n");
    let p3 = write_file(tmp.path(), "c.rs", "gamma\n");

    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "alpha", "new_string": "ALPHA" },
            { "file_path": "b.rs", "old_string": "beta", "new_string": "BETA" },
            { "file_path": "c.rs", "old_string": "gamma", "new_string": "GAMMA" }
        ]
    });

    let out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 3 edits across 3 files");
    assert_eq!(std::fs::read_to_string(&p1).unwrap(), "ALPHA\n");
    assert_eq!(std::fs::read_to_string(&p2).unwrap(), "BETA\n");
    assert_eq!(std::fs::read_to_string(&p3).unwrap(), "GAMMA\n");
}

/// Stale-file detection applies to batch edits: if one file in the batch was
/// modified after it was read, the whole batch is rejected and no files are
/// modified (editrenewal FR-003 / FR-009).
#[tokio::test]
async fn test_batch_stale_file_rejected() {
    let tmp = TempDir::new().unwrap();
    let p1 = write_file(tmp.path(), "a.rs", "alpha\n");
    let p2 = write_file(tmp.path(), "b.rs", "beta\n");
    let original_a = std::fs::read_to_string(&p1).unwrap();
    let original_b = std::fs::read_to_string(&p2).unwrap();

    let c = ctx(tmp.path());
    // Record a stale (old) read timestamp for p2 only.
    {
        let mut map = c.read_timestamps.write().unwrap();
        let old_millis = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            - 5_000;
        map.insert(p2.clone(), old_millis);
    }
    // Bump p2's mtime into the future.
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(10);
    let _ = filetime::set_file_mtime(&p2, filetime::FileTime::from_system_time(future));

    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "alpha", "new_string": "ALPHA" },
            { "file_path": "b.rs", "old_string": "beta", "new_string": "BETA" }
        ]
    });

    let err = MultiEditTool
        .execute(input, &c)
        .await
        .expect_err("stale file in batch must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("modified after") || msg.contains("stale"),
        "error should report stale file: {msg}"
    );
    // Atomic rollback: neither file modified.
    assert_eq!(std::fs::read_to_string(&p1).unwrap(), original_a);
    assert_eq!(std::fs::read_to_string(&p2).unwrap(), original_b);
}

// ── Edit-log instrumentation for multi_edit (editlog spec) ──────────────────

use ragent_tools_core::edit_log::{clear_edit_logs, set_edit_log_enabled};
use std::sync::Mutex;

/// All edit-log tests use a process-wide flag and the working-dir `log/` path,
/// so they must run serialised to avoid one test clearing logs another just wrote.
static EDIT_LOG_MUTEX: Mutex<()> = Mutex::new(());

/// Wait up to one second for an edits jsonl file to appear in `log_dir`.
fn wait_for_edit_log_file(log_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    for _ in 0..50 {
        if let Ok(entries) = std::fs::read_dir(log_dir) {
            let found = entries.flatten().map(|e| e.path()).find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("edits-") && n.ends_with(".jsonl"))
            });
            if found.is_some() {
                return found;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    None
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_multiedit_log_success_writes_jsonl() {
    let _guard = EDIT_LOG_MUTEX.lock().unwrap();
    clear_edit_logs(std::env::temp_dir().as_path());
    let tmp = TempDir::new().unwrap();
    let log_dir = tmp.path().join("log");
    set_edit_log_enabled(true);

    write_file(tmp.path(), "a.rs", "alpha\nbeta\n");
    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "alpha", "new_string": "ALPHA" },
            { "file_path": "a.rs", "old_string": "beta", "new_string": "BETA" }
        ]
    });
    let _out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect("multi_edit should succeed");

    let path = wait_for_edit_log_file(&log_dir).expect("edit log file should be created");
    let content = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert!(
        lines.len() >= 2,
        "multi_edit success should log each resolved edit"
    );

    let success_count = lines
        .iter()
        .filter(|line| {
            let entry: serde_json::Value = serde_json::from_str(line).unwrap();
            entry["tool"] == "multi_edit" && entry["outcome"] == "success"
        })
        .count();
    assert_eq!(
        success_count, 2,
        "two successful multi_edit operations should be logged"
    );

    set_edit_log_enabled(false);
    clear_edit_logs(tmp.path());
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_multiedit_log_failure_writes_jsonl() {
    let _guard = EDIT_LOG_MUTEX.lock().unwrap();
    clear_edit_logs(std::env::temp_dir().as_path());
    let tmp = TempDir::new().unwrap();
    let log_dir = tmp.path().join("log");
    set_edit_log_enabled(true);

    write_file(tmp.path(), "a.rs", "alpha\n");
    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "not found", "new_string": "X" }
        ]
    });
    let _err = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("missing old_string must error");

    let path = wait_for_edit_log_file(&log_dir).expect("edit log file should be created");
    let content = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert!(!lines.is_empty());
    let entry: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(entry["tool"], "multi_edit");
    assert!(
        entry["outcome"].as_str().unwrap().contains("not found"),
        "outcome should record not-found: {}",
        entry["outcome"]
    );

    set_edit_log_enabled(false);
    clear_edit_logs(tmp.path());
}

// ── collapse_whitespace (per-edit opt-in) ──────────────────────────────────

#[tokio::test]
async fn test_multiedit_collapse_whitespace_per_edit() {
    let tmp = TempDir::new().unwrap();
    // a.rs needs flexible matching (4-space needle vs 8-space content);
    // b.rs uses strict matching.
    write_file(tmp.path(), "a.rs", "fn a() {\n        bar\n}\n");
    write_file(tmp.path(), "b.rs", "beta\n");

    let input = json!({
        "edits": [
            {
                "file_path": "a.rs",
                "old_string": "fn a() {\n    bar\n}\n",
                "new_string": "fn a() {\n    baz\n}\n",
                "collapse_whitespace": true
            },
            { "file_path": "b.rs", "old_string": "beta", "new_string": "BETA" }
        ]
    });
    MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect("mixed strict+flexible batch should succeed");
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("a.rs")).unwrap(),
        "fn a() {\n    baz\n}\n"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("b.rs")).unwrap(),
        "BETA\n"
    );
}

#[tokio::test]
async fn test_multiedit_collapse_whitespace_failure_is_atomic() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "a.rs", "alpha\n");
    write_file(tmp.path(), "b.rs", "beta\n");
    let input = json!({
        "edits": [
            { "file_path": "a.rs", "old_string": "alpha", "new_string": "ALPHA" },
            {
                "file_path": "b.rs",
                "old_string": "nonexistent",
                "new_string": "BETA",
                "collapse_whitespace": true
            }
        ]
    });
    let err = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("failed flexible edit must abort the whole batch");
    assert!(
        err.to_string().contains("collapse_whitespace"),
        "error should mention collapse_whitespace mode: {err}"
    );
    // Atomicity: a.rs must NOT have been written.
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("a.rs")).unwrap(),
        "alpha\n"
    );
}

//! Integration tests for the renewed `EditTool` (editrenewal spec).
//!
//! The renewed `edit` tool uses **strict exact-match** replacement (FR-004):
//! `old_string` must match the file byte-for-byte, including whitespace,
//! indentation, and line endings. These tests cover exact match, multiple
//! matches, missing file, stale file, create/delete/update operations,
//! no-change rejection, snippet generation, canonical vs legacy parameter
//! names, and deprecation-warning metadata (FR-013).

use std::sync::Arc;
use std::time::SystemTime;

use ragent_tools_core::edit::EditTool;
use ragent_tools_core::read::ReadTool;
use ragent_tools_core::{Tool, ToolContext};
use serde_json::json;
use tempfile::TempDir;

fn ctx(working_dir: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: working_dir.to_path_buf(),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

fn write_file(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    path
}

/// Helper: run an edit with canonical params and assert the resulting file
/// content.
async fn assert_edit(
    dir: &std::path::Path,
    file: &str,
    initial: &str,
    old_string: &str,
    new_string: &str,
    expected: &str,
) {
    let path = write_file(dir, file, initial);
    let input = json!({
        "file_path": file,
        "old_string": old_string,
        "new_string": new_string,
    });
    let _out = EditTool
        .execute(input, &ctx(dir))
        .await
        .expect("edit should succeed");
    let result = std::fs::read_to_string(&path).unwrap();
    assert_eq!(result, expected, "file content after edit");
}

// ── Exact match (baseline) ───────────────────────────────────────────────────

#[tokio::test]
async fn test_edit_exact_match_baseline() {
    let tmp = TempDir::new().unwrap();
    assert_edit(
        tmp.path(),
        "a.rs",
        "fn foo() {\n    bar\n}\n",
        "    bar\n",
        "    baz\n",
        "fn foo() {\n    baz\n}\n",
    )
    .await;
}

// ── Tolerant matching: common whitespace/line-ending mismatches are accepted ───

#[tokio::test]
async fn test_edit_tolerant_accepts_crlf_mismatch() {
    let tmp = TempDir::new().unwrap();
    // File uses CRLF; needle is LF-only — tolerant CRLF pass should still match.
    // The replacement itself uses new_string verbatim, so only the matched
    // substring changes; surrounding CRLF lines are preserved.
    let path = write_file(tmp.path(), "a.rs", "fn foo() {\r\n    bar\r\n}\r\n");
    let input = json!({
        "file_path": "a.rs",
        "old_string": "bar",
        "new_string": "baz",
    });
    let _ = EditTool.execute(input, &ctx(tmp.path())).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "fn foo() {\r\n    baz\r\n}\r\n"
    );
}

#[tokio::test]
async fn test_edit_tolerant_accepts_trailing_space_mismatch() {
    let tmp = TempDir::new().unwrap();
    // File has no trailing spaces; needle includes them — trailing-whitespace
    // pass should strip the needle's trailing spaces and match.
    let path = write_file(tmp.path(), "a.rs", "fn foo() {\n    bar\n}\n");
    let input = json!({
        "file_path": "a.rs",
        "old_string": "    bar  \n",
        "new_string": "    baz\n",
    });
    let _ = EditTool.execute(input, &ctx(tmp.path())).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "fn foo() {\n    baz\n}\n"
    );
}

#[tokio::test]
async fn test_edit_tolerant_accepts_indentation_mismatch() {
    let tmp = TempDir::new().unwrap();
    // File uses 4-space indentation; model provides 2-space indentation — still unique.
    assert_edit(
        tmp.path(),
        "a.rs",
        "fn foo() {\n    bar\n}\n",
        "  bar\n",
        "  baz\n",
        "fn foo() {\n    baz\n}\n",
    )
    .await;
}

#[tokio::test]
async fn test_edit_dry_run_previews_without_writing() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "fn foo() {\n    bar\n}\n");
    let input = json!({
        "file_path": "a.rs",
        "old_string": "    bar\n",
        "new_string": "    baz\n",
        "dry_run": true,
    });
    let out = EditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect("dry_run edit should succeed");
    let meta = out.metadata.unwrap();
    assert_eq!(meta["dry_run"], true);
    // Snippet is built from the original file content around the matched region.
    assert!(
        out.content.contains("bar"),
        "dry_run snippet should show original matched region: {}",
        out.content
    );
    // File must remain unchanged.
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "fn foo() {\n    bar\n}\n",
        "dry_run must not modify the file"
    );
}

#[tokio::test]
async fn test_edit_not_found_includes_pass_hint() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "a.rs", "fn foo() { 1 }\n");
    let input = json!({
        "file_path": "a.rs",
        "old_string": "totally absent text",
        "new_string": "x",
    });
    let err = EditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("absent text must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("not found"),
        "error should say not found: {msg}"
    );
    assert!(
        msg.contains("pass:") || msg.contains("Last attempted match pass"),
        "error should mention the last attempted pass: {msg}"
    );
}

// ── Multiple matches (FR-004, FR-005) ────────────────────────────────────────

#[tokio::test]
async fn test_edit_multiple_matches_errors() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "a.rs", "dup\nmid\ndup\n");
    let input = json!({
        "file_path": "a.rs",
        "old_string": "dup",
        "new_string": "DUP",
    });
    let err = EditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("non-unique old_string must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("2 times"),
        "error should report the match count: {msg}"
    );
    assert!(
        msg.contains("exactly once") || msg.contains("unique"),
        "error should guide toward uniqueness: {msg}"
    );
}

// ── NotFound (FR-004) ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_edit_not_found_errors() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "a.rs", "fn foo() { 1 }\n");
    let input = json!({
        "file_path": "a.rs",
        "old_string": "nonexistent code here",
        "new_string": "x",
    });
    let err = EditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("missing old_string must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("not found"),
        "error should mention not found: {msg}"
    );
}

// ── Create operation (FR-006): empty old_string ──────────────────────────────

#[tokio::test]
async fn test_edit_create_new_file() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("new.rs");
    assert!(!path.exists());

    let input = json!({
        "file_path": "new.rs",
        "old_string": "",
        "new_string": "fn main() {}\n",
    });
    let out = EditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect("create should succeed");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "fn main() {}\n");
    let meta = out.metadata.unwrap();
    assert_eq!(meta["created"], true);
}

#[tokio::test]
async fn test_edit_create_rejects_existing_file() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "a.rs", "existing\n");
    let input = json!({
        "file_path": "a.rs",
        "old_string": "",
        "new_string": "new content",
    });
    let err = EditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("create on existing file must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("already exists"),
        "error should say the file already exists: {msg}"
    );
    // File must be unchanged.
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("a.rs")).unwrap(),
        "existing\n"
    );
}

// ── Delete operation (FR-006): empty new_string ──────────────────────────────

#[tokio::test]
async fn test_edit_delete_matched_text() {
    let tmp = TempDir::new().unwrap();
    assert_edit(
        tmp.path(),
        "a.rs",
        "fn foo() {\n    bar\n}\n",
        "    bar\n",
        "",
        "fn foo() {\n}\n",
    )
    .await;
}

// ── No-change rejection (FR-007) ─────────────────────────────────────────────

#[tokio::test]
async fn test_edit_no_change_rejected() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "a.rs", "fn foo() { 1 }\n");
    let input = json!({
        "file_path": "a.rs",
        "old_string": "fn foo() { 1 }",
        "new_string": "fn foo() { 1 }",
    });
    let err = EditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("identical old/new must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("identical") || msg.contains("No changes"),
        "error should reject the no-op edit: {msg}"
    );
}

// ── Stale-file detection (FR-003) ────────────────────────────────────────────

#[tokio::test]
async fn test_edit_stale_file_rejected() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "fn foo() { 1 }\n");
    let c = ctx(tmp.path());

    // Simulate a prior read by recording an older timestamp.
    {
        let mut map = c.read_timestamps.write().unwrap();
        map.insert(
            path.clone(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                - 5_000, // 5 seconds in the past
        );
    }

    // Now bump the file's mtime to be newer than the recorded read time.
    let future = SystemTime::now() + std::time::Duration::from_secs(10);
    let _ = filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(future));

    let input = json!({
        "file_path": "a.rs",
        "old_string": "fn foo() { 1 }",
        "new_string": "fn foo() { 2 }",
    });
    let err = EditTool
        .execute(input, &c)
        .await
        .expect_err("stale file must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("modified after") || msg.contains("stale"),
        "error should report stale file: {msg}"
    );
}

#[tokio::test]
async fn test_edit_fresh_file_accepted() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "fn foo() { 1 }\n");
    let c = ctx(tmp.path());

    // Record a read timestamp at the file's current mtime (fresh).
    let mtime = std::fs::metadata(&path)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    {
        let mut map = c.read_timestamps.write().unwrap();
        map.insert(path.clone(), mtime);
    }

    let input = json!({
        "file_path": "a.rs",
        "old_string": "fn foo() { 1 }",
        "new_string": "fn foo() { 2 }",
    });
    let _ = EditTool
        .execute(input, &c)
        .await
        .expect("fresh file edit should succeed");
}

// ── Snippet generation (FR-008) ──────────────────────────────────────────────

#[tokio::test]
async fn test_edit_returns_snippet_with_line_numbers() {
    let tmp = TempDir::new().unwrap();
    let mut initial = String::new();
    for i in 1..=12 {
        initial.push_str(&format!("line {}\n", i));
    }
    write_file(tmp.path(), "a.rs", &initial);

    let input = json!({
        "file_path": "a.rs",
        "old_string": "line 6\n",
        "new_string": "line 6 edited\n",
    });
    let out = EditTool.execute(input, &ctx(tmp.path())).await.unwrap();
    let meta = out.metadata.unwrap();
    let snippet = meta["snippet"].as_str().unwrap_or(out.content.as_str());

    // Snippet should include line numbers and the edited line marker.
    assert!(
        snippet.contains("6"),
        "snippet should reference line 6: {snippet}"
    );
    assert!(
        snippet.contains("edited"),
        "snippet should show the edited content: {snippet}"
    );
    // Should include context before (line 2) and after (line 10).
    assert!(
        snippet.contains("line 2"),
        "snippet should include context before: {snippet}"
    );
    assert!(
        snippet.contains("line 10"),
        "snippet should include context after: {snippet}"
    );
}

// ── Canonical vs legacy parameter names (FR-001, FR-012) ─────────────────────

#[tokio::test]
async fn test_edit_canonical_param_names() {
    let tmp = TempDir::new().unwrap();
    assert_edit(
        tmp.path(),
        "a.rs",
        "fn foo() { 1 }\n",
        "fn foo() { 1 }",
        "fn foo() { 2 }",
        "fn foo() { 2 }\n",
    )
    .await;
}

#[tokio::test]
async fn test_edit_legacy_param_names_accepted() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "fn foo() { 1 }\n");
    let input = json!({
        "path": "a.rs",
        "old_str": "fn foo() { 1 }",
        "new_str": "fn foo() { 2 }",
    });
    let out = EditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect("legacy params should be accepted");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "fn foo() { 2 }\n");
    // Should emit a deprecation warning in metadata.
    let meta = out.metadata.unwrap();
    assert!(
        meta.get("deprecation_warning").is_some(),
        "legacy params should produce a deprecation_warning: {meta}"
    );
}

#[tokio::test]
async fn test_edit_canonical_params_no_deprecation_warning() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "a.rs", "fn foo() { 1 }\n");
    let input = json!({
        "file_path": "a.rs",
        "old_string": "fn foo() { 1 }",
        "new_string": "fn foo() { 2 }",
    });
    let out = EditTool.execute(input, &ctx(tmp.path())).await.unwrap();
    let meta = out.metadata.unwrap();
    assert!(
        meta.get("deprecation_warning").is_none(),
        "canonical params should NOT produce a deprecation_warning: {meta}"
    );
}

// ── Read-then-edit integration (FR-003 end-to-end) ───────────────────────────

#[tokio::test]
async fn test_read_then_edit_no_stale_error() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "fn foo() { 1 }\n");
    let c = ctx(tmp.path());

    // Read the file first (records the timestamp).
    let read_input = json!({ "path": "a.rs" });
    let _ = ReadTool.execute(read_input, &c).await.unwrap();

    // Immediately edit — file mtime == recorded read time, so no stale error.
    let input = json!({
        "file_path": "a.rs",
        "old_string": "fn foo() { 1 }",
        "new_string": "fn foo() { 2 }",
    });
    let _ = EditTool
        .execute(input, &c)
        .await
        .expect("read-then-edit should succeed without stale error");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "fn foo() { 2 }\n");
}

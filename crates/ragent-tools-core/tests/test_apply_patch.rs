//! Integration tests for the `apply_patch` Codex-style patch tool.
//!
//! Following EDITPLAN Milestone 1 (T4), hunk context matching is **strict
//! exact-byte** (`find_exact_replacement_range`), matching upstream Codex
//! `apply_patch` behaviour. A hunk whose context differs from the target file
//! only in CRLF line endings or trailing whitespace fails cleanly and leaves
//! the target unmodified.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use ragent_tools_core::apply_patch::ApplyPatchTool;
use ragent_tools_core::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

fn test_ctx(working_dir: PathBuf) -> ToolContext {
    ToolContext {
        event_bus: Arc::new(EventBus::new(128)),
        session_id: "test-session".to_string(),
        working_dir,
        read_timestamps: Arc::new(RwLock::new(std::collections::HashMap::new())),
    }
}

#[tokio::test]
async fn test_apply_patch_add_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let tool = ApplyPatchTool;

    let out = tool
        .execute(
            json!({
                "patch": "*** Begin Patch\n*** Add File: hello.txt\n+Hello, world!\n*** End Patch"
            }),
            &test_ctx(root.to_path_buf()),
        )
        .await
        .expect("execute");

    let path = root.join("hello.txt");
    assert!(path.exists());
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "Hello, world!");
    assert!(out.content.contains("Applied"));
}

#[tokio::test]
async fn test_apply_patch_update_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let path = root.join("app.rs");
    std::fs::write(&path, "fn main() {\n    println!(\"hi\");\n}\n").unwrap();

    let tool = ApplyPatchTool;
    let out = tool
        .execute(
            json!({
                "patch": "*** Begin Patch\n*** Update File: app.rs\n@@\n fn main() {\n-    println!(\"hi\");\n+    println!(\"hello, world!\");\n }\n*** End Patch"
            }),
            &test_ctx(root.to_path_buf()),
        )
        .await
        .expect("execute");

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("hello, world!"));
    assert!(!content.contains("\"hi\""));
    assert!(out.content.contains("Applied"));
}

#[tokio::test]
async fn test_apply_patch_delete_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let path = root.join("obsolete.txt");
    std::fs::write(&path, "old").unwrap();

    let tool = ApplyPatchTool;
    tool.execute(
        json!({
            "patch": "*** Begin Patch\n*** Delete File: obsolete.txt\n*** End Patch"
        }),
        &test_ctx(root.to_path_buf()),
    )
    .await
    .expect("execute");

    assert!(!path.exists());
}

#[tokio::test]
async fn test_apply_patch_move_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let src = root.join("old.rs");
    let dst = root.join("new.rs");
    std::fs::write(&src, "fn f() {}\n").unwrap();

    let tool = ApplyPatchTool;
    tool.execute(
        json!({
            "patch": "*** Begin Patch\n*** Update File: old.rs\n*** Move to: new.rs\n*** End Patch"
        }),
        &test_ctx(root.to_path_buf()),
    )
    .await
    .expect("execute");

    assert!(!src.exists());
    assert!(dst.exists());
}

#[tokio::test]
async fn test_apply_patch_multi_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let first = root.join("a.txt");
    let second = root.join("sub/b.txt");
    std::fs::write(&first, "a\n").unwrap();
    std::fs::create_dir_all(second.parent().unwrap()).unwrap();
    std::fs::write(&second, "b\n").unwrap();

    let tool = ApplyPatchTool;
    tool.execute(
        json!({
            "patch": "*** Begin Patch\n*** Update File: a.txt\n@@\n-a\n+A\n*** Update File: sub/b.txt\n@@\n-b\n+B\n*** End Patch"
        }),
        &test_ctx(root.to_path_buf()),
    )
    .await
    .expect("execute");

    assert_eq!(std::fs::read_to_string(&first).unwrap(), "A\n");
    assert_eq!(std::fs::read_to_string(&second).unwrap(), "B\n");
}

#[tokio::test]
async fn test_apply_patch_dry_run_does_not_write() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let path = root.join("keep.rs");
    std::fs::write(&path, "fn keep() {}\n").unwrap();

    let tool = ApplyPatchTool;
    let out = tool
        .execute(
            json!({
                "patch": "*** Begin Patch\n*** Update File: keep.rs\n@@\n-fn keep() {}\n+fn keep() { 1 }\n*** End Patch",
                "dry_run": true
            }),
            &test_ctx(root.to_path_buf()),
        )
        .await
        .expect("execute");

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(!content.contains("{ 1 }"));
    assert!(out.content.contains("Would apply"));
}

#[tokio::test]
async fn test_apply_patch_rejects_absolute_path() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let tool = ApplyPatchTool;

    let err = tool
        .execute(
            json!({
                "patch": "*** Begin Patch\n*** Add File: /etc/passwd\n+secret\n*** End Patch"
            }),
            &test_ctx(root.to_path_buf()),
        )
        .await
        .expect_err("absolute path should fail");

    assert!(format!("{err:?}").contains("relative"));
}

#[tokio::test]
async fn test_apply_patch_invalid_hunk_prefix_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let path = root.join("x.rs");
    std::fs::write(&path, "fn x() {}\n").unwrap();

    let tool = ApplyPatchTool;
    let err = tool
        .execute(
            json!({
                "patch": "*** Begin Patch\n*** Update File: x.rs\n@@\nfn x() {}\n*** End Patch"
            }),
            &test_ctx(root.to_path_buf()),
        )
        .await
        .expect_err("invalid hunk should fail");

    assert!(format!("{err:?}").contains("Invalid hunk line prefix"));
}

#[tokio::test]
async fn test_apply_patch_rejects_crlf_context_mismatch() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let path = root.join("crlf.txt");
    std::fs::write(&path, "alpha\nbeta\ngamma\n").unwrap();

    let tool = ApplyPatchTool;
    let out = tool
        .execute(
            json!({
                "patch": "*** Begin Patch\n*** Update File: crlf.txt\n@@\n-alpha\n+omega\n*** End Patch"
            }),
            &test_ctx(root.to_path_buf()),
        )
        .await
        .expect("initial LF patch should apply");
    assert!(out.content.contains("Applied"));

    // Convert the file to CRLF — the bytes no longer match the LF-only context.
    let lf = std::fs::read_to_string(&path).unwrap();
    let crlf = lf.replace('\n', "\r\n");
    std::fs::write(&path, &crlf).unwrap();

    let err = tool
        .execute(
            json!({
                "patch": "*** Begin Patch\n*** Update File: crlf.txt\n@@\n-omega beta\n+delta\n*** End Patch"
            }),
            &test_ctx(root.to_path_buf()),
        )
        .await
        .expect_err("LF hunk context against CRLF file must fail cleanly");

    let msg = format!("{err}");
    assert!(
        msg.contains("could not be applied"),
        "expected a hunk-application error: {msg}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        crlf,
        "file must be unmodified after the failed patch"
    );
}

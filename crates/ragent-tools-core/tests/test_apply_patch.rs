//! Integration tests for the `apply_patch` Codex-style patch tool.

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
                "patch": "*** Begin Patch\n*** Update File: app.rs\n@@ fn main\n fn main() {\n-    println!(\"hi\");\n+    println!(\"hello, world!\");\n }\n*** End Patch"
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
    let a = root.join("a.rs");
    let b = root.join("b.rs");
    std::fs::write(&a, "fn a() { 1 }\n").unwrap();
    std::fs::write(&b, "fn b() { 2 }\n").unwrap();

    let tool = ApplyPatchTool;
    tool.execute(
        json!({
            "patch": "*** Begin Patch\n*** Update File: a.rs\n@@ fn a\n-fn a() { 1 }\n+fn a() { 10 }\n*** Update File: b.rs\n@@ fn b\n-fn b() { 2 }\n+fn b() { 20 }\n*** End Patch"
        }),
        &test_ctx(root.to_path_buf()),
    )
    .await
    .expect("execute");

    assert!(std::fs::read_to_string(&a).unwrap().contains("10"));
    assert!(std::fs::read_to_string(&b).unwrap().contains("20"));
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
                              "patch": "*** Begin Patch\n*** Update File: keep.rs\n@@ fn keep\n-fn keep() {}\n+fn keep() { 1 }\n*** End Patch",
                              "dry_run": true
                          }),            &test_ctx(root.to_path_buf()),
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
                "patch": "*** Begin Patch\n*** Update File: x.rs\n@@ fn x\nfn x() {}\n*** End Patch"
            }),
            &test_ctx(root.to_path_buf()),
        )
        .await
        .expect_err("invalid hunk should fail");

    assert!(format!("{err:?}").contains("Invalid hunk line prefix"));
}

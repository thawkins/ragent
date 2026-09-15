//! Regression tests for FUNC-061: file-operation correctness — self-copy
//! refusal, move leaving no orphan directories, and append flush.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_tools_core::append_file::AppendFileTool;
use ragent_tools_core::copy_file::CopyFileTool;
use ragent_tools_core::move_file::MoveFileTool;
use ragent_tools_core::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

fn ctx(dir: &Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(64)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir.to_path_buf()],
    }
}

fn tmp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ragent_file_ops_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("creating temp dir");
    dir
}

#[tokio::test]
async fn func061_copy_self_is_rejected() {
    let dir = tmp_dir();
    let ctx = ctx(&dir);
    std::fs::write(dir.join("same.txt"), "content").unwrap();

    let out = CopyFileTool
        .execute(
            json!({"source": "same.txt", "destination": "same.txt"}),
            &ctx,
        )
        .await;

    assert!(out.is_err(), "self-copy must be refused: {out:?}");
    // Source must be intact.
    assert_eq!(
        std::fs::read_to_string(dir.join("same.txt")).unwrap(),
        "content"
    );
}

#[tokio::test]
async fn func061_copy_self_via_alias_is_rejected() {
    let dir = tmp_dir();
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let ctx = ctx(&dir);
    std::fs::write(dir.join("same.txt"), "content").unwrap();

    let out = CopyFileTool
        .execute(
            json!({"source": "same.txt", "destination": "./sub/../same.txt"}),
            &ctx,
        )
        .await;

    assert!(
        out.is_err(),
        "self-copy through an alias must be refused: {out:?}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("same.txt")).unwrap(),
        "content"
    );
}

#[tokio::test]
async fn func061_move_creates_destination_parents() {
    let dir = tmp_dir();
    let ctx = ctx(&dir);
    std::fs::write(dir.join("src.txt"), "payload").unwrap();

    let out = MoveFileTool
        .execute(
            json!({"source": "src.txt", "destination": "nested/deep/dst.txt"}),
            &ctx,
        )
        .await;

    assert!(
        out.is_ok(),
        "move into a new directory should succeed: {out:?}"
    );
    assert!(dir.join("nested/deep/dst.txt").exists());
    assert!(!dir.join("src.txt").exists());
}

#[tokio::test]
async fn func061_move_failure_leaves_no_orphan_dirs() {
    let dir = tmp_dir();
    let ctx = ctx(&dir);

    // Source does not exist, so the rename fails. The destination parent must
    // NOT be created as a side effect.
    let out = MoveFileTool
        .execute(
            json!({"source": "missing.txt", "destination": "orphan/dir/dst.txt"}),
            &ctx,
        )
        .await;

    assert!(out.is_err(), "moving a missing file must fail: {out:?}");
    assert!(
        !dir.join("orphan").exists(),
        "a failed move must not leave orphan directories"
    );
}

#[tokio::test]
async fn func061_append_is_flushed_and_visible() {
    let dir = tmp_dir();
    let ctx = ctx(&dir);

    let out = AppendFileTool
        .execute(json!({"path": "log.txt", "content": "hello\n"}), &ctx)
        .await;
    assert!(out.is_ok(), "append should succeed: {out:?}");

    // Read back immediately — the bytes must be flushed, not pending in a
    // dropped buffer.
    assert_eq!(
        std::fs::read_to_string(dir.join("log.txt")).unwrap(),
        "hello\n"
    );

    // A second append appends rather than truncates.
    AppendFileTool
        .execute(json!({"path": "log.txt", "content": "world\n"}), &ctx)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("log.txt")).unwrap(),
        "hello\nworld\n"
    );
}

//! Regression tests for FUNC-068: every file-mutating tool must honour
//! `ctx.allowed_roots`, not just `write`. A destination under a whitelisted root
//! outside `working_dir` must be accepted by all of them.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_tools_core::append_file::AppendFileTool;
use ragent_tools_core::copy_file::CopyFileTool;
use ragent_tools_core::create::CreateTool;
use ragent_tools_core::mkdir::MakeDirTool;
use ragent_tools_core::move_file::MoveFileTool;
use ragent_tools_core::rm::RmTool;
use ragent_tools_core::write::WriteTool;
use ragent_tools_core::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

fn tmp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ragent_allowed_roots_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("creating temp dir");
    dir
}

/// Context whose working_dir is `dir/cwd` and whose allowed_roots is
/// `[dir/cwd, dir/other]`.
fn ctx(dir: &Path) -> ToolContext {
    let cwd = dir.join("cwd");
    let other = dir.join("other");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    ToolContext {
        session_id: "test".to_string(),
        working_dir: cwd,
        event_bus: Arc::new(EventBus::new(64)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir.join("cwd"), dir.join("other")],
    }
}

#[tokio::test]
async fn func068_create_accepts_allowed_root() {
    let dir = tmp_dir();
    let ctx = ctx(&dir);
    let out = CreateTool
        .execute(
            json!({"path": "../other/created.txt", "content": "hi"}),
            &ctx,
        )
        .await;
    assert!(out.is_ok(), "create should accept an allowed root: {out:?}");
    assert!(dir.join("other/created.txt").exists());
}

#[tokio::test]
async fn func068_write_accepts_allowed_root() {
    let dir = tmp_dir();
    let ctx = ctx(&dir);
    let out = WriteTool
        .execute(
            json!({"path": "../other/written.txt", "content": "hi"}),
            &ctx,
        )
        .await;
    assert!(out.is_ok(), "write should accept an allowed root: {out:?}");
}

#[tokio::test]
async fn func068_append_accepts_allowed_root() {
    let dir = tmp_dir();
    let ctx = ctx(&dir);
    std::fs::write(dir.join("other/appended.txt"), "a").unwrap();
    let out = AppendFileTool
        .execute(
            json!({"path": "../other/appended.txt", "content": "b"}),
            &ctx,
        )
        .await;
    assert!(out.is_ok(), "append should accept an allowed root: {out:?}");
}

#[tokio::test]
async fn func068_mkdir_accepts_allowed_root() {
    let dir = tmp_dir();
    let ctx = ctx(&dir);
    let out = MakeDirTool
        .execute(json!({"path": "../other/newdir"}), &ctx)
        .await;
    assert!(out.is_ok(), "mkdir should accept an allowed root: {out:?}");
    assert!(dir.join("other/newdir").is_dir());
}

#[tokio::test]
async fn func068_copy_accepts_allowed_root_destination() {
    let dir = tmp_dir();
    let ctx = ctx(&dir);
    std::fs::write(dir.join("cwd/src.txt"), "payload").unwrap();
    let out = CopyFileTool
        .execute(
            json!({"source": "src.txt", "destination": "../other/copied.txt"}),
            &ctx,
        )
        .await;
    assert!(
        out.is_ok(),
        "copy should accept an allowed root destination: {out:?}"
    );
    assert!(dir.join("other/copied.txt").exists());
}

#[tokio::test]
async fn func068_move_accepts_allowed_root_destination() {
    let dir = tmp_dir();
    let ctx = ctx(&dir);
    std::fs::write(dir.join("cwd/tomove.txt"), "payload").unwrap();
    let out = MoveFileTool
        .execute(
            json!({"source": "tomove.txt", "destination": "../other/moved.txt"}),
            &ctx,
        )
        .await;
    assert!(
        out.is_ok(),
        "move should accept an allowed root destination: {out:?}"
    );
    assert!(dir.join("other/moved.txt").exists());
}

#[tokio::test]
async fn func068_rm_accepts_allowed_root() {
    let dir = tmp_dir();
    let ctx = ctx(&dir);
    std::fs::write(dir.join("other/delete_me.txt"), "x").unwrap();
    let out = RmTool
        .execute(json!({"path": "../other/delete_me.txt"}), &ctx)
        .await;
    assert!(out.is_ok(), "rm should accept an allowed root: {out:?}");
    assert!(!dir.join("other/delete_me.txt").exists());
}

#[tokio::test]
async fn func068_rm_allows_literal_bracket_filename() {
    // FUNC-068: a plain filename containing `[`, `*`, or `?` is a valid
    // single-file target; the old glob heuristic wrongly rejected it.
    let dir = tmp_dir();
    let ctx = ctx(&dir);
    let weird = dir.join("cwd/[bracket].txt");
    std::fs::write(&weird, "x").unwrap();
    let out = RmTool.execute(json!({"path": "[bracket].txt"}), &ctx).await;
    assert!(
        out.is_ok(),
        "rm should allow a literal bracket filename: {out:?}"
    );
    assert!(!weird.exists());
}

#[tokio::test]
async fn func068_rm_still_rejects_escape_outside_all_roots() {
    // Containment must not regress: a path outside every allowed root is still
    // rejected.
    let dir = tmp_dir();
    let ctx = ctx(&dir);
    let out = RmTool
        .execute(json!({"path": "../../etc/passwd"}), &ctx)
        .await;
    assert!(out.is_err(), "rm must still reject an out-of-root escape");
}

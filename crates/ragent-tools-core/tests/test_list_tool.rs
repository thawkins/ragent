//! Coverage-focused tests for `list.rs`: tree formatting, depth bounding,
//! skipped/hidden entries, file sizes, and error paths, driven through the
//! public [`ListTool`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_tools_core::list::ListTool;
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
            "list_test_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn list(input: serde_json::Value, dir: &Path) -> anyhow::Result<ToolOutput> {
    ListTool.execute(input, &make_ctx(dir)).await
}

/// A default listing shows the header, top-level files, and nested directories
/// to the default depth of 2, with directories listed before files.
#[tokio::test]
async fn test_list_basic_tree() {
    let dir = temp_dir("basic");
    std::fs::create_dir_all(dir.join("subdir")).unwrap();
    std::fs::write(dir.join("subdir").join("inner.txt"), "hello").unwrap();
    std::fs::write(dir.join("a.txt"), "aaa").unwrap();
    std::fs::write(dir.join("b.txt"), "bb").unwrap();

    let out = list(json!({}), &dir).await.unwrap();
    assert!(out.content.contains("subdir/"));
    assert!(out.content.contains("inner.txt"));
    assert!(out.content.contains("a.txt"));
    // Directory sorts before files.
    let dpos = out.content.find("subdir/").unwrap();
    let apos = out.content.find("a.txt").unwrap();
    assert!(dpos < apos, "directory should precede files");
    let meta = out.metadata.unwrap();
    // subdir/ + inner.txt + a.txt + b.txt
    assert_eq!(meta["count"], 4);
}

/// File sizes are rendered with `format_size` (a parenthesised size suffix).
#[tokio::test]
async fn test_list_shows_file_sizes() {
    let dir = temp_dir("sizes");
    std::fs::write(dir.join("f.txt"), "x".repeat(2048)).unwrap();
    let out = list(json!({}), &dir).await.unwrap();
    assert!(out.content.contains("f.txt"), "got: {}", out.content);
    assert!(out.content.contains('('), "expected a size suffix");
}

/// `depth: 1` lists only the immediate children, not nested directories.
#[tokio::test]
async fn test_list_depth_one() {
    let dir = temp_dir("depth1");
    std::fs::create_dir_all(dir.join("subdir")).unwrap();
    std::fs::write(dir.join("subdir").join("inner.txt"), "x").unwrap();
    std::fs::write(dir.join("top.txt"), "x").unwrap();

    let out = list(json!({ "depth": 1 }), &dir).await.unwrap();
    assert!(out.content.contains("subdir/"));
    assert!(out.content.contains("top.txt"));
    assert!(
        !out.content.contains("inner.txt"),
        "depth 1 must not recurse into subdir"
    );
}

/// Hidden entries (leading dot) are excluded.
#[tokio::test]
async fn test_list_hides_dotfiles() {
    let dir = temp_dir("hidden");
    std::fs::write(dir.join("visible.txt"), "x").unwrap();
    std::fs::write(dir.join(".hidden.txt"), "x").unwrap();

    let out = list(json!({}), &dir).await.unwrap();
    assert!(out.content.contains("visible.txt"));
    assert!(!out.content.contains(".hidden.txt"));
}

/// Generated directories are listed but marked "(skipped)" and not descended.
#[tokio::test]
async fn test_list_skips_generated_directories() {
    let dir = temp_dir("skipped");
    std::fs::create_dir_all(dir.join("node_modules").join("pkg")).unwrap();
    std::fs::write(dir.join("node_modules").join("pkg").join("index.js"), "x").unwrap();

    let out = list(json!({}), &dir).await.unwrap();
    assert!(out.content.contains("node_modules/  (skipped)"));
    assert!(!out.content.contains("index.js"));
}

/// A specific `path` argument lists that directory.
#[tokio::test]
async fn test_list_explicit_path() {
    let dir = temp_dir("explicit");
    let sub = dir.join("target_dir");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("only.txt"), "x").unwrap();

    let out = list(json!({ "path": sub.to_str().unwrap() }), &dir)
        .await
        .unwrap();
    assert!(out.content.contains("only.txt"));
    let meta = out.metadata.unwrap();
    assert_eq!(meta["path"], sub.display().to_string());
}

/// Listing a non-directory path is an error.
#[tokio::test]
async fn test_list_non_directory_is_error() {
    let dir = temp_dir("notdir");
    let f = dir.join("file.txt");
    std::fs::write(&f, "x").unwrap();
    let err = list(json!({ "path": f.to_str().unwrap() }), &dir)
        .await
        .expect_err("non-directory must fail");
    assert!(err.to_string().contains("not a directory"), "got: {err}");
}

/// A listing path outside the allowed roots is rejected.
#[tokio::test]
async fn test_list_outside_allowed_roots_rejected() {
    let dir = temp_dir("escape");
    let err = list(json!({ "path": "../.." }), &dir)
        .await
        .expect_err("escape must fail");
    assert_ne!(err.to_string(), "");
}

/// An empty directory still returns the header line with a zero count.
#[tokio::test]
async fn test_list_empty_directory() {
    let dir = temp_dir("empty");
    let out = list(json!({}), &dir).await.unwrap();
    assert_eq!(out.metadata.unwrap()["count"], 0);
}

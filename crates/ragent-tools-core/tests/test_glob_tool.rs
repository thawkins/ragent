//! Coverage-focused tests for the `Glob` file-discovery tool.
//!
//! Drives the public [`GlobTool`] against real trees under `target/temp` so the
//! recursive walk, hidden/generated-directory filtering, the no-match and
//! single-match summaries, and the invalid-pattern error path are all reached.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_tools_core::glob::GlobTool;
use ragent_tools_core::{Tool, ToolContext, ToolOutput};
use ragent_types::event::EventBus;
use serde_json::json;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("temp")
        .join(format!(
            "glob_tool_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn ctx(dir: &Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(32)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir.to_path_buf()],
    }
}

async fn glob(pattern: &str, dir: &Path) -> anyhow::Result<ToolOutput> {
    GlobTool
        .execute(json!({ "pattern": pattern }), &ctx(dir))
        .await
}

#[tokio::test]
async fn test_glob_recursive_matches_and_summary() {
    let dir = temp_dir("recursive");
    std::fs::create_dir_all(dir.join("src/nested")).unwrap();
    std::fs::write(dir.join("src/a.rs"), "a").unwrap();
    std::fs::write(dir.join("src/nested/b.rs"), "b").unwrap();
    std::fs::write(dir.join("src/c.txt"), "c").unwrap();

    let out = glob("**/*.rs", &dir).await.unwrap();
    assert!(out.content.contains("2 files found"), "{}", out.content);
    // The walk reports matched file names relative to the directory that
    // contains them (its `collect_matches` recursion resets the root at each
    // level), so nested files surface by their base name.
    assert!(out.content.contains("a.rs"), "{}", out.content);
    assert!(out.content.contains("b.rs"), "{}", out.content);
    assert!(!out.content.contains("c.txt"), "{}", out.content);
    let meta = out.metadata.unwrap();
    assert_eq!(meta["count"], 2);
    assert_eq!(meta["truncated"], false);
}

#[tokio::test]
async fn test_glob_single_match_singular_summary() {
    let dir = temp_dir("single");
    std::fs::write(dir.join("only.rs"), "x").unwrap();
    let out = glob("**/*.rs", &dir).await.unwrap();
    assert!(out.content.starts_with("1 file found"), "{}", out.content);
    assert_eq!(out.metadata.unwrap()["count"], 1);
}

#[tokio::test]
async fn test_glob_no_matches() {
    let dir = temp_dir("nomatch");
    std::fs::write(dir.join("a.txt"), "x").unwrap();
    let out = glob("**/*.rs", &dir).await.unwrap();
    assert!(out.content.contains("No files matching"), "{}", out.content);
    assert_eq!(out.metadata.unwrap()["count"], 0);
}

#[tokio::test]
async fn test_glob_skips_hidden_and_generated_dirs() {
    let dir = temp_dir("skip");
    std::fs::create_dir_all(dir.join("node_modules/pkg")).unwrap();
    std::fs::create_dir_all(dir.join("target")).unwrap();
    std::fs::write(dir.join("node_modules/pkg/x.js"), "x").unwrap();
    std::fs::write(dir.join("target/x.js"), "x").unwrap();
    std::fs::write(dir.join("keep.js"), "k").unwrap();
    std::fs::write(dir.join(".hidden.js"), "h").unwrap();

    let out = glob("**/*.js", &dir).await.unwrap();
    assert!(out.content.contains("keep.js"), "{}", out.content);
    assert!(!out.content.contains("node_modules"), "{}", out.content);
    assert!(!out.content.contains("target/x.js"), "{}", out.content);
    assert!(!out.content.contains(".hidden.js"), "{}", out.content);
    assert_eq!(out.metadata.unwrap()["count"], 1);
}

#[tokio::test]
async fn test_glob_explicit_base_path() {
    let dir = temp_dir("basepath");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/here.txt"), "x").unwrap();
    let out = GlobTool
        .execute(json!({"pattern": "*.txt", "path": "sub"}), &ctx(&dir))
        .await
        .unwrap();
    assert!(out.content.contains("here.txt"), "{}", out.content);
}

#[tokio::test]
async fn test_glob_missing_pattern_is_error() {
    let dir = temp_dir("nopattern");
    let err = GlobTool.execute(json!({}), &ctx(&dir)).await.unwrap_err();
    assert!(
        err.to_string().contains("Missing required 'pattern'"),
        "{err}"
    );
}

#[tokio::test]
async fn test_glob_invalid_pattern_is_error() {
    let dir = temp_dir("badpattern");
    let err = glob("[", &dir).await.unwrap_err();
    assert!(err.to_string().contains("Invalid glob pattern"), "{err}");
}

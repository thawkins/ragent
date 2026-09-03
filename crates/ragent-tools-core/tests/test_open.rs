#![allow(clippy::assert_is_empty)]
//! Integration tests for the `open` tool.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use ragent_tools_core::open::OpenTool;
use ragent_tools_core::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

fn test_ctx(working_dir: PathBuf) -> ToolContext {
    ToolContext {
        event_bus: Arc::new(EventBus::new(128)),
        session_id: "test-session".to_string(),
        working_dir,
        read_timestamps: Arc::new(RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

#[tokio::test]
async fn test_open_rejects_disallowed_url_scheme() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = OpenTool;
    let err = tool
        .execute(
            json!({"target": "ftp://example.com/file.txt", "action": "url"}),
            &test_ctx(tmp.path().to_path_buf()),
        )
        .await
        .expect_err("ftp scheme should be rejected");
    assert!(err.to_string().contains("not allowed"));
}

#[tokio::test]
async fn test_open_accepts_https_url() {
    let tmp = tempfile::tempdir().unwrap();
    let _tool = OpenTool;
    // We do not actually launch a browser in CI; just ensure validation passes
    // by checking the command builder.
    let (program, args) =
        ragent_tools_core::open::build_command("https://example.com", "url", tmp.path())
            .expect("build_command");
    assert!(!program.is_empty());
    assert!(args.iter().any(|a| a.contains("example.com")));
}

#[tokio::test]
async fn test_open_reveals_parent_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let file = root.join("src").join("main.rs");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "fn main() {}\n").unwrap();

    let (program, args) = ragent_tools_core::open::build_command("src/main.rs", "reveal", root)
        .expect("build_command");

    assert!(!program.is_empty());
    assert!(args.iter().any(|a| a.contains("src")));
}

#[tokio::test]
async fn test_open_resolves_relative_path() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let file = root.join("readme.md");
    std::fs::write(&file, "# readme\n").unwrap();

    let (program, args) =
        ragent_tools_core::open::build_command("readme.md", "open", root).expect("build_command");

    assert!(!program.is_empty());
    assert!(args.iter().any(|a| a.contains("readme.md")));
}

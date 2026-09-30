//! ANTIPAT F-21: the `glob` walk must not follow symlinks.
//!
//! `collect_matches` classifies entries via `entry.file_type()` so a symlinked
//! directory is neither descended into nor matched as a regular file, which
//! prevents symlink loops from causing unbounded recursion (the walk is also
//! depth-bounded).

#![cfg(unix)]

use std::path::Path;
use std::sync::Arc;

use ragent_tools_core::glob::GlobTool;
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

fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("temp")
        .join(format!(
            "glob_symlink_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A symlinked directory must not be descended into, so its files are matched
/// exactly once (via the real directory) and not duplicated through the link.
#[tokio::test]
async fn test_glob_does_not_follow_symlinked_dir() {
    let dir = temp_dir();
    let real = dir.join("real");
    std::fs::create_dir_all(&real).unwrap();
    // Uniquely-named file so we can count occurrences in the output.
    std::fs::write(real.join("onlyreal_9f3c.rs"), "// real").unwrap();

    std::os::unix::fs::symlink(&real, dir.join("link")).unwrap();

    let out = GlobTool
        .execute(
            json!({"pattern": "**/*.rs", "path": dir.to_str().unwrap()}),
            &ctx(&dir),
        )
        .await
        .expect("glob should succeed");

    let occurrences = out.content.matches("onlyreal_9f3c.rs").count();
    assert_eq!(
        occurrences, 1,
        "symlinked dir must not be followed (file should appear once): {}",
        out.content
    );
}

/// A self-referential symlink must not cause unbounded recursion.
#[tokio::test]
async fn test_glob_self_symlink_terminates() {
    let dir = temp_dir();
    std::fs::write(dir.join("a.rs"), "// a").unwrap();
    // `self` -> dir itself, a classic infinite-recursion trap if followed.
    std::os::unix::fs::symlink(&dir, dir.join("self")).unwrap();

    let out = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        GlobTool.execute(
            json!({"pattern": "**/*.rs", "path": dir.to_str().unwrap()}),
            &ctx(&dir),
        ),
    )
    .await
    .expect("glob must terminate on a symlink loop")
    .expect("glob should succeed");

    assert!(
        out.content.contains("a.rs"),
        "real file should be found: {}",
        out.content
    );
}

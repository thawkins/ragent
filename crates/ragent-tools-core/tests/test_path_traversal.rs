//! Regression tests for C-002: path-traversal protection in file tools.
//!
//! Verifies that the hardened `check_path_within_root` rejects `..`, absolute
//! escapes, symlink escapes, and root-prefix confusion, and that every file
//! tool entry point (`read`, `write`, `create`, `append_file`, `rm`,
//! `mkdir`, `copy_file`, `move_file`, `file_info`, `list`, `glob`, `grep`,
//! `diff`) enforces containment before touching the filesystem.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_tools_core::append_file::AppendFileTool;
use ragent_tools_core::create::CreateTool;
use ragent_tools_core::diff::DiffFilesTool;
use ragent_tools_core::file_info::FileInfoTool;
use ragent_tools_core::glob::GlobTool;
use ragent_tools_core::grep::GrepTool;
use ragent_tools_core::list::ListTool;
use ragent_tools_core::mkdir::MakeDirTool;
use ragent_tools_core::move_file::MoveFileTool;
use ragent_tools_core::read::ReadTool;
use ragent_tools_core::rm::RmTool;
use ragent_tools_core::write::WriteTool;
use ragent_tools_core::{Tool, ToolContext, check_path_within_root};
use ragent_types::event::EventBus;
use serde_json::json;

fn ctx(dir: &Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(64)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

fn tmp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ragent_path_traversal_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("creating temp dir");
    dir
}

// ============================================================================
// Unit tests for `check_path_within_root`
// ============================================================================

#[test]
fn check_path_allows_same_path() {
    let dir = tmp_dir();
    let file = dir.join("safe.txt");
    std::fs::write(&file, "hello").unwrap();
    assert!(check_path_within_root(&file, &dir).is_ok());
}

#[test]
fn check_path_rejects_dotdot_escape() {
    let dir = tmp_dir();
    let escaped = dir.join("..").join("etc").join("passwd");
    assert!(check_path_within_root(&escaped, &dir).is_err());
}

#[test]
fn check_path_rejects_absolute_outside_root() {
    let dir = tmp_dir();
    assert!(check_path_within_root(Path::new("/etc/passwd"), &dir).is_err());
}

#[test]
fn check_path_does_not_confuse_prefix() {
    // /foo should not contain /foobar just because the string prefix matches.
    let root = tmp_dir().join("foo");
    std::fs::create_dir_all(&root).unwrap();
    let sibling = root.parent().unwrap().join("foobar").join("secret.txt");
    assert!(check_path_within_root(&sibling, &root).is_err());
}

#[test]
#[cfg(unix)]
fn check_path_rejects_symlink_escape() {
    use std::os::unix::fs::symlink as make_symlink;
    let dir = tmp_dir();
    let outside = std::env::temp_dir().join(format!(
        "ragent_path_traversal_secret_{}",
        std::process::id()
    ));
    std::fs::write(&outside, "secret").unwrap();
    let link = dir.join("escape");
    make_symlink(&outside, &link).unwrap();
    assert!(check_path_within_root(&link, &dir).is_err());
    let _ = std::fs::remove_file(&outside);
}

#[test]
fn check_path_rejects_nonexistent_traversal() {
    let dir = tmp_dir();
    let escaped = dir.join("..").join("does-not-exist").join("file.txt");
    assert!(check_path_within_root(&escaped, &dir).is_err());
}

// ============================================================================
// Tool-level tests
// ============================================================================

#[tokio::test]
async fn read_rejects_traversal() {
    let dir = tmp_dir();
    let out = ReadTool
        .execute(json!({"path": "../ Cargo.toml"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "read should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn write_rejects_traversal() {
    let dir = tmp_dir();
    let out = WriteTool
        .execute(json!({"path": "../escape.txt", "content": "x"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "write should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn create_rejects_traversal() {
    let dir = tmp_dir();
    let out = CreateTool
        .execute(json!({"path": "../escape.txt", "content": "x"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "create should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn append_rejects_traversal() {
    let dir = tmp_dir();
    let out = AppendFileTool
        .execute(json!({"path": "../escape.txt", "content": "x"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "append should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn rm_rejects_traversal() {
    let dir = tmp_dir();
    let out = RmTool
        .execute(json!({"path": "../Cargo.toml"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "rm should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn mkdir_rejects_traversal() {
    let dir = tmp_dir();
    let out = MakeDirTool
        .execute(json!({"path": "../escape_dir"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "mkdir should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn file_info_rejects_traversal() {
    let dir = tmp_dir();
    let out = FileInfoTool
        .execute(json!({"path": "../Cargo.toml"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "file_info should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn list_rejects_traversal() {
    let dir = tmp_dir();
    let out = ListTool.execute(json!({"path": ".."}), &ctx(&dir)).await;
    assert!(out.is_err(), "list should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn glob_rejects_traversal() {
    let dir = tmp_dir();
    let out = GlobTool
        .execute(json!({"pattern": "**/*.rs", "path": ".."}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "glob should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn grep_rejects_traversal() {
    let dir = tmp_dir();
    let out = GrepTool
        .execute(
            json!({"pattern": "fn", "path": "..", "max_results": 1}),
            &ctx(&dir),
        )
        .await;
    assert!(out.is_err(), "grep should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn diff_rejects_traversal() {
    let dir = tmp_dir();
    let out = DiffFilesTool
        .execute(
            json!({"path_a": "../Cargo.toml", "path_b": "../Cargo.toml"}),
            &ctx(&dir),
        )
        .await;
    assert!(out.is_err(), "diff should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn copy_file_rejects_traversal_source() {
    let dir = tmp_dir();
    let out = ragent_tools_core::copy_file::CopyFileTool
        .execute(
            json!({"source": "../Cargo.toml", "destination": "copy.toml"}),
            &ctx(&dir),
        )
        .await;
    assert!(
        out.is_err(),
        "copy_file source should reject ../ escape: {out:?}"
    );
}

#[tokio::test]
async fn copy_file_rejects_traversal_destination() {
    let dir = tmp_dir();
    let out = ragent_tools_core::copy_file::CopyFileTool
        .execute(
            json!({"source": "in.txt", "destination": "../escape.txt"}),
            &ctx(&dir),
        )
        .await;
    assert!(
        out.is_err(),
        "copy_file destination should reject ../ escape: {out:?}"
    );
}

#[tokio::test]
async fn move_file_rejects_traversal_destination() {
    let dir = tmp_dir();
    let out = MoveFileTool
        .execute(
            json!({"source": "in.txt", "destination": "../escape.txt"}),
            &ctx(&dir),
        )
        .await;
    assert!(
        out.is_err(),
        "move_file destination should reject ../ escape: {out:?}"
    );
}

#[tokio::test]
async fn allows_normal_relative_path() {
    let dir = tmp_dir();
    std::fs::write(dir.join("safe.txt"), "hello").unwrap();
    let out = ReadTool
        .execute(json!({"path": "safe.txt"}), &ctx(&dir))
        .await;
    assert!(
        out.is_ok(),
        "normal relative path should be allowed: {out:?}"
    );
}

// ============================================================================
// Alias / bind-mount acceptance tests
// ============================================================================

#[test]
#[cfg(unix)]
fn check_path_accepts_symlink_alias_to_root() {
    let dir = tmp_dir();
    let alias = dir
        .parent()
        .unwrap()
        .join(format!("ragent_path_alias_{}", std::process::id()));
    std::os::unix::fs::symlink(&dir, &alias).unwrap();

    let file = alias.join("safe.txt");
    std::fs::write(&file, "hello").unwrap();
    assert!(
        check_path_within_root(&file, &dir).is_ok(),
        "symlink alias to root should be accepted"
    );

    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_file(&alias);
}

#[test]
#[cfg(unix)]
fn check_path_accepts_nonexistent_under_alias_root() {
    let dir = tmp_dir();
    let alias = dir
        .parent()
        .unwrap()
        .join(format!("ragent_path_alias_new_{}", std::process::id()));
    std::os::unix::fs::symlink(&dir, &alias).unwrap();

    let new_file = alias.join("does-not-exist-yet.txt");
    assert!(
        check_path_within_root(&new_file, &dir).is_ok(),
        "non-existent path under alias root should be accepted"
    );

    let _ = std::fs::remove_file(&alias);
}

#[test]
#[cfg(unix)]
fn check_path_rejects_alias_to_sibling_directory() {
    let dir = tmp_dir();
    let sibling = tmp_dir();
    let alias = dir
        .parent()
        .unwrap()
        .join(format!("ragent_path_sibling_alias_{}", std::process::id()));
    std::os::unix::fs::symlink(&sibling, &alias).unwrap();

    let file = alias.join("secret.txt");
    std::fs::write(&file, "secret").unwrap();
    assert!(
        check_path_within_root(&file, &dir).is_err(),
        "alias to a sibling directory must still be rejected"
    );

    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_file(&alias);
}

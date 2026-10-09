//! Coverage for the `ragent-surface` store-directory resolution and the
//! symlink-containment guard (`store.rs`), which the plugin and connector
//! store front-ends delegate to.
//!
//! `store_dirs_at` is deliberately pure over its inputs (no env reads), so the
//! project/global legs and the override precedence are asserted directly. The
//! symlink guard is exercised against real directories and symlinks under a
//! `tempfile` dir: a symlink whose target escapes the store must be skipped, a
//! symlink staying inside it must be accepted.

use std::fs;
use std::path::Path;

use ragent_surface::store::{store_dirs_at, store_entry_is_dir};

fn workdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("ragent-surface-{tag}-"))
        .tempdir()
        .expect("tempdir")
}

#[test]
fn test_store_dirs_at_project_uses_workdir_leaf() {
    let tmp = workdir("project");
    let dirs = store_dirs_at(tmp.path(), None, None, "plugins");
    assert_eq!(
        dirs.project.as_deref(),
        Some(tmp.path().join(".ragent/plugins").as_path())
    );
    assert!(dirs.global.is_none());
    assert_eq!(dirs.destination(), dirs.project.as_deref());
}

#[test]
fn test_store_dirs_at_override_replaces_project_leg() {
    let tmp = workdir("override");
    let override_dir = tmp.path().join("custom-plugins");
    fs::create_dir_all(&override_dir).expect("mkdir override");
    let dirs = store_dirs_at(tmp.path(), Some(&override_dir), None, "plugins");
    assert_eq!(dirs.project.as_deref(), Some(override_dir.as_path()));
}

#[test]
fn test_store_dirs_at_global_leg_uses_leaf_child() {
    let tmp = workdir("global");
    let global_root = tmp.path().join("config/ragent");
    let dirs = store_dirs_at(tmp.path(), None, Some(&global_root), "connectors");
    assert_eq!(
        dirs.global.as_deref(),
        Some(global_root.join("connectors").as_path())
    );
    // Project leg still takes precedence as the install destination.
    assert_eq!(dirs.destination(), dirs.project.as_deref());
}

#[test]
fn test_store_dirs_at_no_global_root_leaves_global_none() {
    let tmp = workdir("noglobal");
    let dirs = store_dirs_at(tmp.path(), None, None, "connectors");
    assert!(dirs.global.is_none());
}

#[test]
fn test_store_entry_is_dir_accepts_real_directory() {
    let tmp = workdir("realdir");
    let store = tmp.path().join("store");
    let entry = store.join("plugin-a");
    fs::create_dir_all(&entry).expect("mkdir entry");
    assert!(store_entry_is_dir(&store, &entry, "plugin"));
}

#[test]
fn test_store_entry_is_dir_rejects_non_directory_file() {
    let tmp = workdir("file");
    let store = tmp.path().join("store");
    fs::create_dir_all(&store).expect("mkdir store");
    let file = store.join("not-a-dir.txt");
    fs::write(&file, b"x").expect("write file");
    assert!(!store_entry_is_dir(&store, &file, "plugin"));
}

#[test]
fn test_store_entry_is_dir_skips_escaping_symlink() {
    let tmp = workdir("escape");
    let store = tmp.path().join("store");
    let outside = tmp.path().join("outside");
    fs::create_dir_all(&store).expect("mkdir store");
    fs::create_dir_all(&outside).expect("mkdir outside");
    let link = store.join("escaped");
    if make_symlink(&outside, &link).is_err() {
        return; // platform without symlink permission: skip
    }
    assert!(!store_entry_is_dir(&store, &link, "plugin"));
}

#[test]
fn test_store_entry_is_dir_accepts_contained_symlink() {
    let tmp = workdir("contained");
    let store = tmp.path().join("store");
    let inside = store.join("real-plugin");
    fs::create_dir_all(&inside).expect("mkdir inside");
    let link = store.join("link-plugin");
    if make_symlink(&inside, &link).is_err() {
        return; // platform without symlink permission: skip
    }
    assert!(store_entry_is_dir(&store, &link, "plugin"));
}

/// Create a directory symlink, returning the error so the caller can skip on
/// platforms where symlink creation is not permitted.
#[cfg(unix)]
fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

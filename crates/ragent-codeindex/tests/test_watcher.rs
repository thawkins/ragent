#![allow(clippy::assert_is_empty)]
//! External integration tests for the code-index filesystem watcher.

use ragent_codeindex::watcher::{CodeWatcher, WatchEvent, should_ignore};
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn test_should_ignore_git() {
    let root = Path::new("/project");
    assert!(should_ignore(root, Path::new("/project/.git/HEAD")));
    assert!(should_ignore(root, Path::new("/project/.git/objects/abc")));
}

#[test]
fn test_should_ignore_target() {
    let root = Path::new("/project");
    assert!(should_ignore(root, Path::new("/project/target/debug/bin")));
}

#[test]
fn test_should_not_ignore_source() {
    let root = Path::new("/project");
    // For files that don't exist on disk, is_dir() returns false,
    // so this should not be ignored.
    assert!(!should_ignore(root, Path::new("/project/src/main.rs")));
}

#[test]
fn test_should_ignore_node_modules() {
    let root = Path::new("/project");
    assert!(should_ignore(
        root,
        Path::new("/project/node_modules/foo/index.js")
    ));
}

#[test]
fn test_watcher_receives_create_event() {
    let dir = tempfile::tempdir().unwrap();
    let (tx, rx) = mpsc::channel();
    let _watcher = CodeWatcher::new(dir.path(), tx).unwrap();

    // Give the watcher time to start.
    std::thread::sleep(Duration::from_millis(200));

    // Create a file.
    let file_path = dir.path().join("hello.rs");
    fs::write(&file_path, "fn main() {}").unwrap();

    // Wait for events — FS notifications can be slow.
    let mut got_create = false;
    for _ in 0..20 {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(WatchEvent::Created(p)) => {
                if p.to_string_lossy().contains("hello.rs") {
                    got_create = true;
                    break;
                }
            }
            Ok(WatchEvent::Changed(p)) => {
                // Some platforms emit Changed instead of Created.
                if p.to_string_lossy().contains("hello.rs") {
                    got_create = true;
                    break;
                }
            }
            Ok(_) => continue,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        }
    }
    assert!(got_create, "should receive create event for hello.rs");
}

#[test]
fn test_watcher_filters_git_events() {
    let dir = tempfile::tempdir().unwrap();
    let git_dir = dir.path().join(".git");
    fs::create_dir_all(&git_dir).unwrap();

    let (tx, rx) = mpsc::channel();
    let _watcher = CodeWatcher::new(dir.path(), tx).unwrap();

    std::thread::sleep(Duration::from_millis(200));

    // Create a file inside .git — should be filtered.
    fs::write(git_dir.join("test"), "data").unwrap();

    // Should not receive any events.
    match rx.recv_timeout(Duration::from_millis(500)) {
        Err(mpsc::RecvTimeoutError::Timeout) => {} // Expected
        Ok(ev) => panic!("should not receive .git event, got: {ev:?}"),
        Err(e) => panic!("unexpected error: {e}"),
    }
}

#[test]
fn test_watcher_receives_delete_event() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("delete_me.rs");
    fs::write(&file_path, "fn delete() {}").unwrap();

    let (tx, rx) = mpsc::channel();
    let _watcher = CodeWatcher::new(dir.path(), tx).unwrap();

    std::thread::sleep(Duration::from_millis(200));

    // Delete the file.
    fs::remove_file(&file_path).unwrap();

    let mut got_delete = false;
    for _ in 0..20 {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(WatchEvent::Deleted(p)) => {
                if p.to_string_lossy().contains("delete_me.rs") {
                    got_delete = true;
                    break;
                }
            }
            Ok(_) => continue,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        }
    }
    assert!(got_delete, "should receive delete event for delete_me.rs");
}

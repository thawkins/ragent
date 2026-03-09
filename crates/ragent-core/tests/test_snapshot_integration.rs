use ragent_core::snapshot::*;
use std::io::Write;
use std::path::PathBuf;

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ragent_snapshot_test_{}", name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ── Snapshot multiple files ──────────────────────────────────────

#[test]
fn test_snapshot_multiple_files() {
    let dir = test_dir("multiple");

    let file1 = dir.join("a.txt");
    let file2 = dir.join("b.txt");
    let file3 = dir.join("c.txt");

    std::fs::write(&file1, "content A").unwrap();
    std::fs::write(&file2, "content B").unwrap();
    std::fs::write(&file3, "content C").unwrap();

    let snapshot =
        take_snapshot("s1", "m1", &[file1.clone(), file2.clone(), file3.clone()]).unwrap();
    assert_eq!(snapshot.files.len(), 3);
    assert_eq!(snapshot.files.get(&file1).unwrap(), b"content A");
    assert_eq!(snapshot.files.get(&file2).unwrap(), b"content B");
    assert_eq!(snapshot.files.get(&file3).unwrap(), b"content C");

    // Modify all files
    std::fs::write(&file1, "CHANGED").unwrap();
    std::fs::write(&file2, "CHANGED").unwrap();
    std::fs::write(&file3, "CHANGED").unwrap();

    // Restore
    restore_snapshot(&snapshot).unwrap();

    assert_eq!(std::fs::read_to_string(&file1).unwrap(), "content A");
    assert_eq!(std::fs::read_to_string(&file2).unwrap(), "content B");
    assert_eq!(std::fs::read_to_string(&file3).unwrap(), "content C");

    std::fs::remove_dir_all(&dir).ok();
}

// ── Snapshot with nonexistent files ──────────────────────────────

#[test]
fn test_snapshot_skips_nonexistent_files() {
    let dir = test_dir("nonexistent");
    let existing = dir.join("exists.txt");
    let missing = dir.join("missing.txt");

    std::fs::write(&existing, "hello").unwrap();

    let snapshot = take_snapshot("s1", "m1", &[existing.clone(), missing]).unwrap();
    assert_eq!(snapshot.files.len(), 1);
    assert_eq!(snapshot.files.get(&existing).unwrap(), b"hello");

    std::fs::remove_dir_all(&dir).ok();
}

// ── Snapshot empty file list ─────────────────────────────────────

#[test]
fn test_snapshot_empty_file_list() {
    let snapshot = take_snapshot("s1", "m1", &[]).unwrap();
    assert!(snapshot.files.is_empty());
    assert_eq!(snapshot.session_id, "s1");
    assert_eq!(snapshot.message_id, "m1");
    assert!(!snapshot.id.is_empty());
}

// ── Restore creates parent directories ───────────────────────────

#[test]
fn test_restore_creates_parent_dirs() {
    let dir = test_dir("restore_mkdir");
    let nested_path = dir.join("a").join("b").join("c").join("file.txt");

    std::fs::create_dir_all(nested_path.parent().unwrap()).unwrap();
    std::fs::write(&nested_path, "deep content").unwrap();

    let snapshot = take_snapshot("s1", "m1", &[nested_path.clone()]).unwrap();

    // Remove the entire directory tree
    std::fs::remove_dir_all(&dir).unwrap();

    // Restore should recreate the directories
    restore_snapshot(&snapshot).unwrap();

    assert_eq!(
        std::fs::read_to_string(&nested_path).unwrap(),
        "deep content"
    );

    std::fs::remove_dir_all(&dir).ok();
}

// ── Snapshot preserves binary content ────────────────────────────

#[test]
fn test_snapshot_binary_content() {
    let dir = test_dir("binary");
    let file_path = dir.join("binary.bin");

    let binary_data: Vec<u8> = (0u8..=255).collect();
    std::fs::write(&file_path, &binary_data).unwrap();

    let snapshot = take_snapshot("s1", "m1", &[file_path.clone()]).unwrap();
    assert_eq!(snapshot.files.get(&file_path).unwrap(), &binary_data);

    std::fs::write(&file_path, b"overwritten").unwrap();

    restore_snapshot(&snapshot).unwrap();

    let restored = std::fs::read(&file_path).unwrap();
    assert_eq!(restored, binary_data);

    std::fs::remove_dir_all(&dir).ok();
}

// ── Multiple snapshots are independent ───────────────────────────

#[test]
fn test_multiple_snapshots_independent() {
    let dir = test_dir("independent");
    let file_path = dir.join("evolving.txt");

    // State 1
    std::fs::write(&file_path, "version 1").unwrap();
    let snap1 = take_snapshot("s1", "m1", &[file_path.clone()]).unwrap();

    // State 2
    std::fs::write(&file_path, "version 2").unwrap();
    let snap2 = take_snapshot("s1", "m2", &[file_path.clone()]).unwrap();

    // State 3
    std::fs::write(&file_path, "version 3").unwrap();

    // Restore snap1
    restore_snapshot(&snap1).unwrap();
    assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "version 1");

    // Restore snap2
    restore_snapshot(&snap2).unwrap();
    assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "version 2");

    std::fs::remove_dir_all(&dir).ok();
}

// ── Snapshot metadata ────────────────────────────────────────────

#[test]
fn test_snapshot_metadata() {
    let snapshot = take_snapshot("session-abc", "message-xyz", &[]).unwrap();
    assert_eq!(snapshot.session_id, "session-abc");
    assert_eq!(snapshot.message_id, "message-xyz");
    assert!(!snapshot.id.is_empty());
    // created_at should be roughly now
    let now = chrono::Utc::now();
    let diff = (now - snapshot.created_at).num_seconds().abs();
    assert!(diff < 5, "Snapshot created_at should be close to now");
}

// ── Snapshot IDs are unique ──────────────────────────────────────

#[test]
fn test_snapshot_ids_unique() {
    let snap1 = take_snapshot("s1", "m1", &[]).unwrap();
    let snap2 = take_snapshot("s1", "m1", &[]).unwrap();
    assert_ne!(snap1.id, snap2.id, "Snapshot IDs should be unique");
}

// ── Snapshot empty file ──────────────────────────────────────────

#[test]
fn test_snapshot_empty_file() {
    let dir = test_dir("empty_file");
    let file_path = dir.join("empty.txt");
    std::fs::write(&file_path, "").unwrap();

    let snapshot = take_snapshot("s1", "m1", &[file_path.clone()]).unwrap();
    assert_eq!(snapshot.files.get(&file_path).unwrap(), b"");

    std::fs::write(&file_path, "not empty anymore").unwrap();

    restore_snapshot(&snapshot).unwrap();
    assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "");

    std::fs::remove_dir_all(&dir).ok();
}

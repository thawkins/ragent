//! Tests for test_snapshot.rs

use ragent_core::snapshot::*;
use std::io::Write;

#[test]
fn test_snapshot_roundtrip() {
    let dir = std::env::temp_dir().join("ragent_snapshot_test");
    std::fs::create_dir_all(&dir).unwrap();

    let file_path = dir.join("test.txt");
    let mut f = std::fs::File::create(&file_path).unwrap();
    f.write_all(b"hello snapshot").unwrap();

    let snapshot = take_snapshot("session1", "msg1", &[file_path.clone()]).unwrap();
    assert_eq!(snapshot.files.len(), 1);
    assert_eq!(snapshot.files.get(&file_path).unwrap(), b"hello snapshot");

    // Modify file
    std::fs::write(&file_path, "modified").unwrap();

    // Restore
    restore_snapshot(&snapshot).unwrap();
    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hello snapshot");

    // Cleanup
    std::fs::remove_dir_all(&dir).ok();
}

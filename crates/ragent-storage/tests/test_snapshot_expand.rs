//! PERF-070: tests for moving (not cloning) snapshot bytes during expand.

use std::path::PathBuf;

use ragent_storage::snapshot::{
    IncrementalSnapshot, Snapshot, incremental_save, restore_snapshot, take_snapshot,
};

/// Create a temp directory with the named files and given contents.
fn write_files(dir: &std::path::Path, files: &[(&str, &str)]) -> Vec<PathBuf> {
    files
        .iter()
        .map(|(name, content)| {
            let path = dir.join(name);
            std::fs::write(&path, content).expect("write file");
            path
        })
        .collect()
}

#[test]
fn to_full_moves_added_and_applies_diff() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base_files = write_files(dir.path(), &[("a.txt", "one\ntwo\n")]);
    let base = take_snapshot("s1", "m1", &base_files).expect("snapshot");

    // Mutate a.txt and add b.txt, then build the delta.
    std::fs::write(dir.path().join("a.txt"), "one\nTWO\n").expect("overwrite");
    let current = vec![dir.path().join("a.txt"), dir.path().join("b.txt")];
    std::fs::write(dir.path().join("b.txt"), "brand new\n").expect("write b");
    let delta = incremental_save(&base, "m2", &current).expect("delta");

    // `to_full` consumes the delta and base by value (PERF-070).
    let full = delta.to_full(base).expect("expand");

    assert_eq!(
        full.files.get(&dir.path().join("a.txt")).map(Vec::as_slice),
        Some(b"one\nTWO\n".as_slice()),
        "changed file must reflect the applied diff"
    );
    assert_eq!(
        full.files.get(&dir.path().join("b.txt")).map(Vec::as_slice),
        Some(b"brand new\n".as_slice()),
        "added file bytes must be carried into the expanded snapshot"
    );
    assert_eq!(full.session_id, "s1");
    assert_eq!(full.message_id, "m2");
}

#[test]
fn to_full_removes_deleted_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base_files = write_files(dir.path(), &[("a.txt", "a"), ("b.txt", "b")]);
    let base = take_snapshot("s1", "m1", &base_files).expect("snapshot");

    // Delete b.txt from disk, then snapshot only a.txt.
    std::fs::remove_file(dir.path().join("b.txt")).expect("remove");
    let delta = incremental_save(&base, "m2", &[dir.path().join("a.txt")]).expect("delta");

    let full = delta.to_full(base).expect("expand");
    assert!(full.files.contains_key(&dir.path().join("a.txt")));
    assert!(
        !full.files.contains_key(&dir.path().join("b.txt")),
        "deleted file must be absent from the expanded snapshot"
    );
}

#[test]
fn to_full_round_trips_through_restore() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base_files = write_files(dir.path(), &[("keep.txt", "kept\n")]);
    let base = take_snapshot("s1", "m1", &base_files).expect("snapshot");

    std::fs::write(dir.path().join("keep.txt"), "changed\n").expect("overwrite");
    let delta: IncrementalSnapshot = incremental_save(
        &base,
        "m2",
        &[dir.path().join("keep.txt"), dir.path().join("new.txt")],
    )
    .expect("delta");
    std::fs::write(dir.path().join("new.txt"), "new\n").expect("write new");

    let full: Snapshot = delta.to_full(base).expect("expand");
    restore_snapshot(&full).expect("restore");

    assert_eq!(
        std::fs::read_to_string(dir.path().join("keep.txt")).expect("read"),
        "changed\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("new.txt")).expect("read"),
        "new\n"
    );
}

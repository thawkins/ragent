//! Tests-only coverage for the concurrent/batched file-operation API
//! (`ragent_agent::file_ops`).
//!
//! Exercises [`ConcurrentFileReader::read_paths`] (happy path plus the
//! unreadable-file branch), the [`EditStaging`] dry-run short-circuit, the
//! commit happy path, and the checksum-mismatch conflict branch. No
//! production code is modified; all I/O stays inside a temp directory.

use std::path::PathBuf;

use ragent_agent::file_ops::{ConcurrentFileReader, EditStaging, apply_batch_edits};

fn write(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(&path, body).expect("write file");
    path
}

#[tokio::test]
async fn concurrent_read_paths_preserves_order_and_reports_lines() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = write(dir.path(), "a.txt", "one\ntwo\nthree\n");
    let b = write(dir.path(), "b.txt", "single\n");

    let reader = ConcurrentFileReader::new().with_concurrency(2);
    let results = reader
        .read_paths(vec![a.clone(), b.clone()])
        .await
        .expect("read_paths ok");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].path, a);
    assert_eq!(results[0].content.as_deref(), Some("one\ntwo\nthree\n"));
    assert_eq!(results[0].lines, Some(3));
    assert!(results[0].err.is_none());
    assert_eq!(results[1].path, b);
    assert_eq!(results[1].lines, Some(1));
}

#[tokio::test]
async fn concurrent_read_paths_records_missing_file_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("does-not-exist.txt");

    let reader = ConcurrentFileReader::new();
    let results = reader
        .read_paths(vec![missing.clone()])
        .await
        .expect("read_paths should not fail the batch");

    assert_eq!(results.len(), 1);
    assert!(results[0].content.is_none());
    assert!(
        results[0].err.is_some(),
        "missing file must record an error"
    );
    assert!(results[0].lines.is_none());
}

#[tokio::test]
async fn staging_dry_run_reports_applied_without_writing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write(dir.path(), "conf.txt", "original\n");

    let mut staging = EditStaging::new(true);
    staging
        .stage_edit(&path, "rewritten\n".to_string())
        .await
        .expect("stage_edit");
    assert!(staging.validate().is_ok());

    let result = staging.commit_all(4).await.expect("commit_all");
    assert_eq!(result.applied, vec![path.clone()]);
    assert_eq!(result.conflicts.len(), 0);
    assert_eq!(result.errors.len(), 0);
    // Dry run must not have touched the file on disk.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "original\n");
}

#[tokio::test]
async fn commit_all_writes_staged_content() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write(dir.path(), "live.txt", "before\n");

    let mut staging = EditStaging::new(false);
    staging
        .stage_edit(&path, "after\n".to_string())
        .await
        .expect("stage_edit");

    let result = staging.commit_all(2).await.expect("commit_all");
    assert_eq!(result.applied, vec![path.clone()]);
    assert_eq!(result.conflicts.len(), 0);
    assert_eq!(result.errors.len(), 0);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "after\n");
}

#[tokio::test]
async fn commit_all_detects_checksum_mismatch_conflict() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write(dir.path(), "race.txt", "v1\n");

    let mut staging = EditStaging::new(false);
    staging
        .stage_edit(&path, "v2\n".to_string())
        .await
        .expect("stage_edit");

    // Simulate an external write between staging and commit.
    std::fs::write(&path, "v1-external\n").expect("external write");

    let result = staging.commit_all(2).await.expect("commit_all");
    assert_eq!(result.conflicts.len(), 1, "conflict expected");
    assert_eq!(result.conflicts[0].0, path);
    assert!(result.conflicts[0].1.contains("checksum mismatch"));
    assert_eq!(result.applied.len(), 0);
}

#[tokio::test]
async fn apply_batch_edits_round_trips_multiple_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = write(dir.path(), "x.txt", "old-x\n");
    let b = write(dir.path(), "y.txt", "old-y\n");

    let result = apply_batch_edits(
        vec![
            (a.clone(), "new-x\n".to_string()),
            (b.clone(), "new-y\n".to_string()),
        ],
        2,
        false,
    )
    .await
    .expect("apply_batch_edits");

    assert_eq!(result.applied.len(), 2);
    assert_eq!(std::fs::read_to_string(&a).unwrap(), "new-x\n");
    assert_eq!(std::fs::read_to_string(&b).unwrap(), "new-y\n");
}

#[tokio::test]
async fn apply_batch_edits_dry_run_keeps_disk_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = write(dir.path(), "z.txt", "keep\n");

    let result = apply_batch_edits(vec![(a.clone(), "change\n".to_string())], 1, true)
        .await
        .expect("dry run");

    assert_eq!(result.applied.len(), 1);
    assert_eq!(std::fs::read_to_string(&a).unwrap(), "keep\n");
}

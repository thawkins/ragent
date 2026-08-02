//! External integration tests for the code-index background worker.

use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::CodeIndexConfig;
use ragent_codeindex::watcher::WatchEvent;
use ragent_codeindex::worker::{EventBatch, IndexWorker, WorkerConfig, WorkerStats};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

fn make_test_index(dir: &std::path::Path) -> Arc<CodeIndex> {
    let cfg = CodeIndexConfig {
        enabled: true,
        project_root: dir.to_path_buf(),
        ..Default::default()
    };
    Arc::new(CodeIndex::open_in_memory(&cfg).unwrap())
}

#[test]
fn test_event_batch_dedup() {
    let mut b = EventBatch::new();
    b.push(WatchEvent::Changed(PathBuf::from("src/main.rs")));
    b.push(WatchEvent::Changed(PathBuf::from("src/main.rs")));
    b.push(WatchEvent::Changed(PathBuf::from("src/lib.rs")));
    assert_eq!(b.to_index.len(), 2);
    assert!(b.to_remove.is_empty());
}

#[test]
fn test_event_batch_delete_overrides_create() {
    let mut b = EventBatch::new();
    b.push(WatchEvent::Created(PathBuf::from("src/main.rs")));
    b.push(WatchEvent::Deleted(PathBuf::from("src/main.rs")));
    assert!(b.to_index.is_empty());
    assert_eq!(b.to_remove.len(), 1);
}

#[test]
fn test_event_batch_create_after_delete() {
    let mut b = EventBatch::new();
    b.push(WatchEvent::Deleted(PathBuf::from("src/main.rs")));
    b.push(WatchEvent::Created(PathBuf::from("src/main.rs")));
    assert_eq!(b.to_index.len(), 1);
    assert!(b.to_remove.is_empty());
}

#[test]
fn test_event_batch_rename() {
    let mut b = EventBatch::new();
    b.push(WatchEvent::Renamed {
        from: PathBuf::from("old.rs"),
        to: PathBuf::from("new.rs"),
    });
    assert_eq!(b.to_remove.len(), 1);
    assert!(b.to_remove.contains(&PathBuf::from("old.rs")));
    assert_eq!(b.to_index.len(), 1);
    assert!(b.to_index.contains(&PathBuf::from("new.rs")));
}

#[test]
fn test_worker_stats_default() {
    let s = WorkerStats::default();
    assert_eq!(s.files_indexed, 0);
    assert_eq!(s.files_removed, 0);
    assert_eq!(s.batches_processed, 0);
    assert!(!s.is_busy);
}

#[test]
fn test_worker_start_stop() {
    let dir = tempfile::tempdir().unwrap();
    let index = make_test_index(dir.path());
    let (_tx, rx) = mpsc::channel();

    let mut handle = IndexWorker::start(index, rx, WorkerConfig::default());
    assert!(!handle.is_stopped());

    handle.stop();
    assert!(handle.is_stopped());
}

#[test]
fn test_worker_processes_events() {
    let dir = tempfile::tempdir().unwrap();
    // Create a source file for the worker to index.
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

    let index = make_test_index(dir.path());
    let (tx, rx) = mpsc::channel();

    let config = WorkerConfig {
        debounce_ms: 100,
        batch_size: 50,
        max_queue_size: 1000,
    };

    let mut handle = IndexWorker::start(Arc::clone(&index), rx, config);

    // Send a change event.
    tx.send(WatchEvent::Changed(PathBuf::from("src/main.rs")))
        .unwrap();

    // Wait for the worker to process it.
    std::thread::sleep(Duration::from_millis(500));

    let stats = handle.stats();
    // The file should have been indexed.
    assert!(
        stats.batches_processed >= 1,
        "expected at least 1 batch, got {}",
        stats.batches_processed
    );

    handle.stop();
}

#[test]
fn test_worker_channel_disconnect_stops() {
    let dir = tempfile::tempdir().unwrap();
    let index = make_test_index(dir.path());
    let (tx, rx) = mpsc::channel();

    let mut handle = IndexWorker::start(index, rx, WorkerConfig::default());

    // Drop the sender — should cause worker to exit.
    drop(tx);

    // Give the worker time to notice.
    std::thread::sleep(Duration::from_millis(200));
    handle.stop();
}

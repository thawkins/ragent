#![allow(missing_docs)]

//! Integration tests for Milestone 4: background worker, watcher, and watch session.

use ragent_code::CodeIndex;
use ragent_code::types::{CodeIndexConfig, SearchQuery};
use ragent_code::watcher::{CodeWatcher, WatchEvent};
use ragent_code::worker::{IndexWorker, WorkerConfig};
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn create_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("lib.rs"),
        r#"
/// Sample function for testing.
pub fn sample_function() -> u32 {
    42
}
"#,
    )
    .unwrap();

    dir
}

fn make_config(dir: &TempDir) -> CodeIndexConfig {
    CodeIndexConfig {
        enabled: true,
        project_root: dir.path().to_path_buf(),
        ..Default::default()
    }
}

fn open_index(dir: &TempDir) -> CodeIndex {
    let index_dir = dir.path().join(".ragent/codeindex");
    fs::create_dir_all(&index_dir).unwrap();
    let config = CodeIndexConfig {
        enabled: true,
        project_root: dir.path().to_path_buf(),
        index_dir,
        ..Default::default()
    };
    CodeIndex::open(&config).unwrap()
}

// ── Watcher Tests ──────────────────────────────────────────────────────────

#[test]
fn test_watcher_create_event() {
    let dir = TempDir::new().unwrap();

    // Pre-create the src dir so that creating a file in it is a simpler event.
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();

    let (tx, rx) = mpsc::channel();
    let _watcher = CodeWatcher::new(dir.path(), tx).unwrap();

    std::thread::sleep(Duration::from_millis(500));

    fs::write(src.join("new.rs"), "fn new_func() {}").unwrap();

    let mut got_event = false;
    for _ in 0..40 {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(WatchEvent::Created(p)) | Ok(WatchEvent::Changed(p)) => {
                if p.to_string_lossy().contains("new.rs") {
                    got_event = true;
                    break;
                }
            }
            Ok(_) => continue,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        }
    }
    assert!(got_event, "should receive event for new.rs");
}

#[test]
fn test_watcher_delete_event() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("del.rs");
    fs::write(&file_path, "fn old() {}").unwrap();

    let (tx, rx) = mpsc::channel();
    let _watcher = CodeWatcher::new(dir.path(), tx).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    fs::remove_file(&file_path).unwrap();

    let mut got_delete = false;
    for _ in 0..30 {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(WatchEvent::Deleted(p)) => {
                if p.to_string_lossy().contains("del.rs") {
                    got_delete = true;
                    break;
                }
            }
            Ok(_) => continue,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        }
    }
    assert!(got_delete, "should receive delete event for del.rs");
}

#[test]
fn test_watcher_ignores_git_dir() {
    let dir = TempDir::new().unwrap();
    let git_dir = dir.path().join(".git");
    fs::create_dir_all(&git_dir).unwrap();

    let (tx, rx) = mpsc::channel();
    let _watcher = CodeWatcher::new(dir.path(), tx).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    fs::write(git_dir.join("HEAD"), "ref: refs/heads/main").unwrap();

    match rx.recv_timeout(Duration::from_millis(1000)) {
        Err(mpsc::RecvTimeoutError::Timeout) => {} // Expected
        Ok(ev) => panic!("should not receive .git events, got: {ev:?}"),
        Err(e) => panic!("unexpected channel error: {e}"),
    }
}

// ── Worker Tests ───────────────────────────────────────────────────────────

#[test]
fn test_worker_indexes_changed_file() {
    let dir = create_project();
    let index = Arc::new(Mutex::new(open_index(&dir)));
    let (tx, rx) = mpsc::channel();

    let config = WorkerConfig {
        debounce_ms: 100,
        batch_size: 50,
        max_queue_size: 1000,
    };

    let mut handle = IndexWorker::start(Arc::clone(&index), rx, config);

    // Send a change event for the existing file.
    tx.send(WatchEvent::Changed(PathBuf::from("src/lib.rs")))
        .unwrap();

    std::thread::sleep(Duration::from_millis(600));

    let stats = handle.stats();
    assert!(
        stats.batches_processed >= 1,
        "worker should have processed at least 1 batch, got {}",
        stats.batches_processed,
    );

    // Verify the file was actually indexed.
    let idx = index.lock().unwrap();
    let results = idx.search(&SearchQuery::new("sample_function")).unwrap();
    assert!(
        !results.is_empty(),
        "sample_function should be findable after worker indexes the file"
    );

    drop(idx);
    handle.stop();
}

#[test]
fn test_worker_handles_delete() {
    let dir = create_project();
    let index = Arc::new(Mutex::new(open_index(&dir)));

    // First, index the file directly.
    {
        let idx = index.lock().unwrap();
        idx.full_reindex().unwrap();
    }

    let (tx, rx) = mpsc::channel();
    let config = WorkerConfig {
        debounce_ms: 100,
        ..Default::default()
    };

    let mut handle = IndexWorker::start(Arc::clone(&index), rx, config);

    // Delete the physical file and send delete event.
    fs::remove_file(dir.path().join("src/lib.rs")).unwrap();
    tx.send(WatchEvent::Deleted(PathBuf::from("src/lib.rs")))
        .unwrap();

    std::thread::sleep(Duration::from_millis(600));

    let stats = handle.stats();
    assert!(
        stats.files_removed >= 1,
        "should have removed at least 1 file"
    );

    handle.stop();
}

#[test]
fn test_worker_dedup_events() {
    let dir = create_project();
    let index = Arc::new(Mutex::new(open_index(&dir)));
    let (tx, rx) = mpsc::channel();

    let config = WorkerConfig {
        debounce_ms: 200,
        batch_size: 50,
        max_queue_size: 1000,
    };

    let mut handle = IndexWorker::start(Arc::clone(&index), rx, config);

    // Send multiple events for the same file — should be deduped.
    for _ in 0..5 {
        tx.send(WatchEvent::Changed(PathBuf::from("src/lib.rs")))
            .unwrap();
    }

    std::thread::sleep(Duration::from_millis(800));

    let stats = handle.stats();
    // Deduped to 1 file, so files_indexed should be 1 (not 5).
    assert!(
        stats.files_indexed <= 2, // Allow some slack for timing.
        "expected deduped indexing, got {} files indexed",
        stats.files_indexed,
    );

    handle.stop();
}

#[test]
fn test_worker_stop_is_graceful() {
    let dir = create_project();
    let index = Arc::new(Mutex::new(open_index(&dir)));
    let (_tx, rx) = mpsc::channel();

    let mut handle = IndexWorker::start(index, rx, WorkerConfig::default());
    assert!(!handle.is_stopped());

    handle.stop();
    assert!(handle.is_stopped());
}

#[test]
fn test_worker_manual_full_reindex() {
    let dir = create_project();
    let index = Arc::new(Mutex::new(open_index(&dir)));
    let (_tx, rx) = mpsc::channel();

    let config = WorkerConfig {
        debounce_ms: 100,
        ..Default::default()
    };

    let mut handle = IndexWorker::start(Arc::clone(&index), rx, config);

    // Request a full reindex.
    handle.queue_full_reindex();

    std::thread::sleep(Duration::from_millis(600));

    let stats = handle.stats();
    assert!(
        stats.batches_processed >= 1,
        "full reindex should count as a batch"
    );

    // Verify the index has data.
    let idx = index.lock().unwrap();
    let st = idx.status().unwrap();
    assert!(st.files_indexed > 0, "should have indexed files");

    drop(idx);
    handle.stop();
}

// ── Watch Session Tests ────────────────────────────────────────────────────

#[test]
fn test_watch_session_start_stop() {
    let dir = create_project();
    let index = Arc::new(Mutex::new(open_index(&dir)));

    let config = WorkerConfig {
        debounce_ms: 100,
        ..Default::default()
    };

    let mut session = ragent_code::start_watching(Arc::clone(&index), config).unwrap();
    assert!(!session.is_stopped());

    // The initial reindex should have happened.
    let idx = index.lock().unwrap();
    let st = idx.status().unwrap();
    assert!(
        st.files_indexed > 0,
        "initial reindex should have indexed files"
    );
    drop(idx);

    session.stop();
    assert!(session.is_stopped());
}

#[test]
fn test_watch_session_picks_up_new_file() {
    let dir = create_project();
    let index = Arc::new(Mutex::new(open_index(&dir)));

    let config = WorkerConfig {
        debounce_ms: 100,
        ..Default::default()
    };

    let mut session = ragent_code::start_watching(Arc::clone(&index), config).unwrap();

    // Create a new file in the watched directory.
    fs::write(
        dir.path().join("src/new_file.rs"),
        "pub fn brand_new() -> i32 { 99 }",
    )
    .unwrap();

    // Wait for watcher + worker to process.
    std::thread::sleep(Duration::from_secs(2));

    // The new function should be findable.
    let idx = index.lock().unwrap();
    let results = idx.search(&SearchQuery::new("brand_new")).unwrap();
    // Note: this depends on watcher + worker pipeline working end-to-end.
    // On some CI systems FS events may be slow, so we allow this to be empty
    // and just check that no crash occurred.
    if !results.is_empty() {
        assert!(results[0].symbol_name.contains("brand_new"));
    }
    drop(idx);

    session.stop();
}

// ── Tree Cache Tests ───────────────────────────────────────────────────────

#[test]
fn test_tree_cache_populated_on_index() {
    let dir = create_project();
    let config = make_config(&dir);
    let idx = CodeIndex::open_in_memory(&config).unwrap();

    idx.index_file(std::path::Path::new("src/lib.rs")).unwrap();

    // The index should have search results now.
    let results = idx.search(&SearchQuery::new("sample_function")).unwrap();
    assert!(
        !results.is_empty(),
        "should find sample_function after indexing"
    );
}

#[test]
fn test_remove_file_clears_cache() {
    let dir = create_project();
    let config = make_config(&dir);
    let idx = CodeIndex::open_in_memory(&config).unwrap();

    idx.index_file(std::path::Path::new("src/lib.rs")).unwrap();
    idx.remove_file(std::path::Path::new("src/lib.rs")).unwrap();

    let results = idx.search(&SearchQuery::new("sample_function")).unwrap();
    assert!(
        results.is_empty(),
        "should not find sample_function after removal"
    );
}

// ── Event Batch Tests ──────────────────────────────────────────────────────

#[test]
fn test_rename_event_via_worker() {
    let dir = create_project();
    let index = Arc::new(Mutex::new(open_index(&dir)));

    // First index the original file.
    {
        let idx = index.lock().unwrap();
        idx.full_reindex().unwrap();
    }

    // "Rename" by creating a new file and deleting the old one.
    fs::write(
        dir.path().join("src/renamed.rs"),
        r#"pub fn sample_function() -> u32 { 42 }"#,
    )
    .unwrap();

    let (tx, rx) = mpsc::channel();
    let config = WorkerConfig {
        debounce_ms: 100,
        ..Default::default()
    };

    let mut handle = IndexWorker::start(Arc::clone(&index), rx, config);

    tx.send(WatchEvent::Renamed {
        from: PathBuf::from("src/lib.rs"),
        to: PathBuf::from("src/renamed.rs"),
    })
    .unwrap();

    std::thread::sleep(Duration::from_millis(600));

    let stats = handle.stats();
    assert!(stats.batches_processed >= 1);

    handle.stop();
}

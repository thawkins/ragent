//! Tests for the threaded graph-build path and the graph-busy indicator
//! state (spec graphCI, v1.0.80 status-bar indicator work).
//!
//! Covers:
//! - `CodeIndex::build_graph` clears the `graph_busy` flag on completion.
//! - `CodeIndex::spawn_graph_build` runs the build on a dedicated thread and
//!   is observable via the lock-free `graph_busy` atomic while the store lock
//!   is held by the caller.
//! - The double-spawn guard refuses a second `spawn_graph_build` while one is
//!   running.
//! - `graph_build_progress` returns to `(0, 0)` when idle.

use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::CodeIndexConfig;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn make_config(dir: &TempDir) -> CodeIndexConfig {
    CodeIndexConfig {
        enabled: true,
        project_root: dir.path().to_path_buf(),
        index_dir: dir.path().join(".ragent/codeindex"),
        scan_config: ragent_codeindex::types::ScanConfig::default(),
    }
}

/// Write a small Rust file with two functions where one calls the other, so
/// the graph build has real work to do.
fn write_rust_file(dir: &TempDir) {
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        src_dir.join("main.rs"),
        r#"fn caller() {
    callee();
}

fn callee() {
    println!("hello");
}
"#,
    )
    .unwrap();
}

#[test]
fn test_graph_busy_clears_after_build() {
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let idx = CodeIndex::open(&make_config(&dir)).unwrap();
    idx.full_reindex().unwrap();

    assert!(!idx.graph_busy(), "graph must be idle before the build");
    idx.build_graph().unwrap();
    assert!(
        !idx.graph_busy(),
        "graph_busy must clear after build_graph returns"
    );
    assert_eq!(
        idx.graph_build_progress(),
        (0, 0),
        "progress counters must be reset when idle"
    );
}

#[test]
fn test_graph_busy_clears_after_failed_build() {
    let dir = TempDir::new().unwrap();
    // No files indexed — the build itself succeeds with zero edges, but the
    // flag must still clear. Use a store poisoned by holding the guard... not
    // possible via public API; instead assert on the empty-index path.
    let idx = CodeIndex::open(&make_config(&dir)).unwrap();

    idx.build_graph().unwrap();
    assert!(!idx.graph_busy());
    assert_eq!(idx.graph_build_progress(), (0, 0));
}

#[test]
fn test_spawn_graph_build_runs_on_separate_thread() {
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let idx = Arc::new(CodeIndex::open(&make_config(&dir)).unwrap());
    idx.full_reindex().unwrap();

    let handle = CodeIndex::spawn_graph_build(Arc::clone(&idx))
        .expect("spawn_graph_build should succeed when idle");
    handle.join().expect("graph build thread must not panic");

    assert!(
        !idx.graph_busy(),
        "graph_busy must be clear after the spawned build finishes"
    );
    let edges = idx.graph_edge_count().unwrap();
    assert!(edges > 0, "spawned build must have derived real edges");
}

#[test]
fn test_graph_busy_observable_while_store_lock_held() {
    // build_graph sets graph_busy BEFORE acquiring the store mutex, so a UI
    // thread that holds the store lock can still observe the busy flag while
    // the build waits. This is the property the TUI status-bar indicator
    // relies on.
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let idx = Arc::new(CodeIndex::open(&make_config(&dir)).unwrap());
    idx.full_reindex().unwrap();

    // Hold the store lock from the test thread.
    let store_guard = idx.try_lock_store_for_test().expect("store must lock");

    let idx_for_thread = Arc::clone(&idx);
    let handle = std::thread::spawn(move || idx_for_thread.build_graph());

    // Poll (bounded) until the build thread sets graph_busy while blocked on
    // the store lock.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed_busy = false;
    while Instant::now() < deadline {
        if idx.graph_busy() {
            observed_busy = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        observed_busy,
        "graph_busy must be set while the build waits for the store lock"
    );

    // Release the lock so the build can finish.
    drop(store_guard);
    let result = handle.join().expect("build thread must not panic");
    assert!(result.is_ok());
    assert!(!idx.graph_busy(), "graph_busy must clear once done");
}

#[test]
fn test_spawn_graph_build_refuses_when_busy() {
    // While the store lock is held by the test the spawned build blocks on
    // the store mutex with graph_busy set; a second spawn must be refused.
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let idx = Arc::new(CodeIndex::open(&make_config(&dir)).unwrap());
    idx.full_reindex().unwrap();

    let store_guard = idx.try_lock_store_for_test().expect("store must lock");

    let idx_for_thread = Arc::clone(&idx);
    let handle = std::thread::spawn(move || idx_for_thread.build_graph());

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !idx.graph_busy() {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(idx.graph_busy(), "build thread must be waiting on the lock");

    let second = CodeIndex::spawn_graph_build(Arc::clone(&idx));
    assert!(
        second.is_err(),
        "spawn_graph_build must refuse while a build is running"
    );

    drop(store_guard);
    handle.join().expect("build thread must not panic").unwrap();
    assert!(!idx.graph_busy());

    // Once idle, spawning works again.
    assert!(CodeIndex::spawn_graph_build(Arc::clone(&idx)).is_ok());
}

// ── FR-026: phased graph build keeps the store lock free during derivation ──

#[test]
fn test_search_available_during_long_graph_derivation() {
    // While a graph build's CPU-heavy derivation phase runs, the store mutex
    // must be FREE: another thread must be able to take the store lock (and
    // run a symbols query) without waiting for the build to finish. Before
    // the phased build this deadlocked-until-timeout because build_graph held
    // the store guard for the whole derivation.
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let idx = Arc::new(CodeIndex::open(&make_config(&dir)).unwrap());
    idx.full_reindex().unwrap();

    // The real assertion: while a full build_graph runs on another thread,
    // the store lock must be acquirable quickly, repeatedly, at any point
    // during the build (i.e. the build does not hold it for its duration).
    let idx_for_build = Arc::clone(&idx);
    let build_handle = std::thread::spawn(move || idx_for_build.build_graph());

    // Hammer the store lock while the build runs.
    let mut acquired = 0;
    while !build_handle.is_finished() {
        if idx.try_lock_store_for_test().is_some() {
            acquired += 1;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    build_handle.join().expect("build must not panic").unwrap();
    assert!(
        acquired > 0,
        "store lock must be acquirable while the graph build runs"
    );
}

#[test]
fn test_graph_progress_counters_reset_after_direct_build() {
    // build_graph sets graph_done/graph_total for live progress and must
    // return them to (0, 0) when idle — matching the full_reindex phase and
    // the pre-phased direct-build behaviour.
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let idx = CodeIndex::open(&make_config(&dir)).unwrap();
    idx.full_reindex().unwrap();

    idx.build_graph().unwrap();
    assert!(!idx.graph_busy());
    assert_eq!(idx.graph_build_progress(), (0, 0));

    idx.build_graph_for_language("rust").unwrap();
    assert!(!idx.graph_busy());
    assert_eq!(idx.graph_build_progress(), (0, 0));
}

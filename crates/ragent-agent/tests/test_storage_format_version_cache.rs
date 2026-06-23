//! PERF-004: regression test for the cached `format_version` column check.
//!
//! `Storage::get_session` and `Storage::list_sessions` previously ran a
//! `pragma_table_info` query on every call to detect the `format_version`
//! column. PERF-004 caches that result in an `AtomicBool` populated during
//! `migrate()`, so subsequent calls skip the SQLite round-trip.
//!
//! These tests assert the observable behaviour:
//!   1. The cache starts as `true` after `open_in_memory()` (because migrate
//!      creates the column on the fresh schema).
//!   2. `get_session` and `list_sessions` still return correct rows after the
//!      change (no functional regression).

use ragent_agent::storage::Storage;

#[test]
fn format_version_cache_is_true_after_migrate() {
    // open_in_memory runs migrate(), which creates the sessions table with the
    // format_version column from the initial CREATE TABLE. The PERF-004
    // cache must therefore already be populated to `true`.
    let storage = Storage::open_in_memory().expect("in-memory storage");
    assert!(
        storage
            .has_format_version
            .load(std::sync::atomic::Ordering::Relaxed),
        "PERF-004: has_format_version flag should be true after migrate() creates the column"
    );
}

#[test]
fn get_session_works_with_cached_format_version() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-1", "/tmp/project")
        .expect("create session");

    // Second call should hit the cached flag and still return a valid row.
    let row = storage
        .get_session("sess-1")
        .expect("get_session")
        .expect("row");
    assert_eq!(row.id, "sess-1");
    assert_eq!(row.directory, "/tmp/project");
    assert_eq!(row.format_version, 1, "default format_version is 1");
}

#[test]
fn list_sessions_works_with_cached_format_version() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-a", "/tmp/project-a")
        .expect("create a");
    storage
        .create_session("sess-b", "/tmp/project-b")
        .expect("create b");

    let rows = storage.list_sessions().expect("list_sessions");
    assert_eq!(rows.len(), 2, "both sessions should be listed");
    for row in &rows {
        assert_eq!(row.format_version, 1, "default format_version is 1");
    }
}

#[test]
fn repeated_calls_stay_on_fast_path_without_re_querying_pragma() {
    // This is a behavioural smoke test: repeated calls must not error and must
    // return consistent results. The internal pragma-skip is verified by the
    // AtomicBool state; here we just confirm correctness across many calls.
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-x", "/tmp/project-x")
        .expect("create x");

    for _ in 0..50 {
        let row = storage
            .get_session("sess-x")
            .expect("get_session")
            .expect("row");
        assert_eq!(row.id, "sess-x");
    }
    let rows = storage.list_sessions().expect("list");
    assert_eq!(rows.len(), 1);
}

//! PERF-004: regression test for the cached `format_version` column check.
//!
//! `Storage::get_session` and `Storage::list_sessions` previously ran a
//! `pragma_table_info` query on every call to detect the `format_version`
//! column. PERF-004 caches that result in an `AtomicBool` populated during
//! `migrate()`, so subsequent calls skip the `SQLite` round-trip.
//!
//! The `has_format_version` field is now private (it lives on
//! `ragent_storage::Storage`, which is re-exported by `ragent_agent::storage`).
//! These tests therefore assert the *observable* behaviour rather than the
//! internal flag state:
//!   1. `get_session` / `list_sessions` return correct rows after `open_in_memory`
//!      (which runs `migrate` and populates the cache).
//!   2. Repeated calls stay on the cached fast path and remain correct.

use ragent_agent::storage::Storage;

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
    // Behavioural smoke test: repeated calls must not error and must return
    // consistent results. The internal pragma-skip is verified by the
    // canonical `ragent-storage::tests::test_format_version_cache` suite;
    // here we confirm correctness across many calls through the agent
    // re-export.
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

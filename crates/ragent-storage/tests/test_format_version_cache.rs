//! PERF-004 regression tests for the `format_version` column-existence cache.
//!
//! `Storage` now carries an `AtomicBool` (`has_format_version`) that records
//! whether the `sessions.format_version` column exists, so `get_session` /
//! `list_sessions` can skip the `pragma_table_info` round-trip on every call.
//! These tests exercise the cache from the public API: the flag is populated
//! by `migrate()` (run inside `open` / `open_in_memory`) and reused on every
//! subsequent session read.

use ragent_storage::Storage;

#[test]
fn format_version_column_exists_after_open_in_memory() {
    // `open_in_memory` runs `migrate`, which creates the `format_version`
    // column (and sets the PERF-004 cache flag).  A freshly migrated
    // in-memory database therefore always reports the column as present,
    // and `get_session` / `list_sessions` should read `format_version == 1`
    // for a newly created session.
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-perf004", "/tmp/perf004")
        .expect("create session");

    let row = storage
        .get_session("sess-perf004")
        .expect("get session")
        .expect("session exists");
    assert_eq!(row.format_version, 1, "format_version default should be 1");

    let listed = storage.list_sessions().expect("list sessions");
    let matching = listed
        .iter()
        .find(|s| s.id == "sess-perf004")
        .expect("created session appears in list");
    assert_eq!(
        matching.format_version, 1,
        "list_sessions should also read format_version == 1"
    );
}

#[test]
fn list_sessions_after_archive_excludes_archived_row() {
    // Exercises the `has_format_version_cached` fast path through
    // `list_sessions` (which filters `archived_at IS NULL`).  Archiving a
    // session must drop it from the list while keeping it retrievable via
    // `get_session`.
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-active", "/tmp/active")
        .expect("create active session");
    storage
        .create_session("sess-archived", "/tmp/archived")
        .expect("create archived session");
    storage
        .archive_session("sess-archived")
        .expect("archive session");

    let listed = storage.list_sessions().expect("list sessions");
    let ids: Vec<&str> = listed.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"sess-active"), "active session listed");
    assert!(
        !ids.contains(&"sess-archived"),
        "archived session excluded from list"
    );

    // The archived row is still retrievable directly and reports the
    // default format_version (exercising the cached fast path).
    let archived = storage
        .get_session("sess-archived")
        .expect("get archived session")
        .expect("archived session exists");
    assert_eq!(archived.format_version, 1);
}

#[test]
fn repeated_get_session_calls_use_cached_fast_path() {
    // Smoke test: many `get_session` calls in a row must all succeed and
    // return consistent data.  This exercises the `has_format_version_cached`
    // helper on the hot path — if the cache ever returned a stale `false`
    // (column missing) the SELECT would lack the `format_version` column and
    // the call would error.
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-hot", "/tmp/hot")
        .expect("create session");

    for _ in 0..50 {
        let row = storage
            .get_session("sess-hot")
            .expect("get session")
            .expect("session exists");
        assert_eq!(row.id, "sess-hot");
        assert_eq!(row.format_version, 1);
    }
}
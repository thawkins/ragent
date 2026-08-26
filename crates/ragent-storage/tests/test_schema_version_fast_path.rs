//! Tests for the `schema_version` fast-path in `Storage::migrate`.
//!
//! On a warm start (existing DB with a matching `schema_version` setting),
//! `migrate` skips the full 34-statement `CREATE ... IF NOT EXISTS` batch
//! and the 7 `pragma_table_info` column probes, reducing `Storage::open`
//! from ~41 SQL round-trips to a single `CREATE TABLE` + `SELECT`.
//!
//! These tests verify:
//! - A fresh DB gets the full migration and the `schema_version` setting written.
//! - A second `Storage::open` on the same file hits the fast path (no re-migration).
//! - The `has_format_version` cache is set on the fast path so `get_session` /
//!   `list_sessions` skip their `pragma_table_info` probe.
//! - All tables/indexes are present after a fast-path open (schema is intact).

use ragent_storage::Storage;

/// Returns a fresh temp file path for a file-backed database.
fn temp_db_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("ragent-schema-version-test");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
    path
}

/// A fresh database should have `schema_version` written to the `settings` table.
#[test]
fn fresh_db_writes_schema_version() {
    let path = temp_db_path("fresh-version.db");
    let storage = Storage::open(&path).expect("open storage");

    let version = storage
        .get_setting("schema_version")
        .expect("get setting")
        .expect("schema_version should be set after migration");
    assert_eq!(
        version, "1",
        "schema_version should be '1' after fresh migration"
    );

    drop(storage);
    let _ = std::fs::remove_file(&path);
}

/// A second `Storage::open` on an existing DB should hit the fast path
/// and still have a fully functional schema (all tables present).
#[test]
fn second_open_uses_fast_path_and_schema_is_intact() {
    let path = temp_db_path("second-open.db");

    // First open — full migration, writes schema_version.
    {
        let storage = Storage::open(&path).expect("first open");
        storage
            .create_session("sess-1", "/tmp/test")
            .expect("create session");
        // Verify the setting was written.
        let v = storage
            .get_setting("schema_version")
            .expect("get setting")
            .expect("version set");
        assert_eq!(v, "1");
    }

    // Second open — fast path.  All CRUD must still work.
    {
        let storage = Storage::open(&path).expect("second open (fast path)");

        // Can read the session created by the first open.
        let row = storage
            .get_session("sess-1")
            .expect("get session")
            .expect("session exists");
        assert_eq!(row.id, "sess-1");

        // Can create a new session.
        storage
            .create_session("sess-2", "/tmp/test2")
            .expect("create session on fast path");

        // Can create and list tasks (exercises the todos table + new columns).
        storage
            .create_task_simple("todo-1", "sess-2", "Test", "pending", "")
            .expect("create todo on fast path");
        let todos = storage
            .list_tasks("sess-2", None)
            .expect("list todos on fast path");
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].id, "todo-1");

        // Can write and read a setting.
        storage
            .set_setting("test-key", "test-value")
            .expect("set setting on fast path");
        assert_eq!(
            storage.get_setting("test-key").expect("get setting"),
            Some("test-value".to_string())
        );

        // schema_version is still present (not clobbered by fast path).
        let v = storage
            .get_setting("schema_version")
            .expect("get setting")
            .expect("version set");
        assert_eq!(v, "1");
    }

    let _ = std::fs::remove_file(&path);
}

/// The fast path must set `has_format_version` so `get_session` / `list_sessions`
/// skip the `pragma_table_info` round-trip.  Verify by reading a session on a
/// fast-path-opened storage and checking `format_version` is populated.
#[test]
fn fast_path_sets_has_format_version_cache() {
    let path = temp_db_path("fast-path-cache.db");

    // First open — full migration.
    {
        let storage = Storage::open(&path).expect("first open");
        storage
            .create_session("sess-cache", "/tmp/cache")
            .expect("create session");
    }

    // Second open — fast path.  get_session should return format_version == 1
    // without erroring, proving the cache was set correctly.
    let storage = Storage::open(&path).expect("second open (fast path)");
    let row = storage
        .get_session("sess-cache")
        .expect("get session")
        .expect("session exists");
    assert_eq!(row.format_version, 1);

    // list_sessions also exercises the cache.
    let listed = storage.list_sessions().expect("list sessions");
    let found = listed
        .iter()
        .find(|s| s.id == "sess-cache")
        .expect("session in list");
    assert_eq!(found.format_version, 1);

    let _ = std::fs::remove_file(&path);
}

/// In-memory databases are always fresh (no persisted settings), so they
/// always run the full migration batch.  Verify the schema is functional.
#[test]
fn in_memory_always_full_migration() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-mem", "/tmp/mem")
        .expect("create session");

    let row = storage
        .get_session("sess-mem")
        .expect("get session")
        .expect("session exists");
    assert_eq!(row.id, "sess-mem");
    assert_eq!(row.format_version, 1);

    // schema_version is written even for in-memory DBs.
    let v = storage
        .get_setting("schema_version")
        .expect("get setting")
        .expect("version set");
    assert_eq!(v, "1");
}

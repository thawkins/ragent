//! T-05: `list_sessions` must be served by the partial index
//! `idx_sessions_active_updated`, with no temp b-tree sort.
//!
//! Before the index the plan was `SCAN sessions` + `USE TEMP B-TREE FOR ORDER
//! BY`, so every session picker sorted in a temp file. These tests lock in the
//! new plan and the ordering/archived semantics it must preserve.

use ragent_storage::storage::{SCHEMA_VERSION, Storage};
use std::path::Path;

fn storage() -> Storage {
    Storage::open_in_memory().expect("storage")
}

fn plan(storage: &Storage, sql: &str) -> Vec<String> {
    let guard = storage.conn_lock_for_test().expect("lock");
    let mut stmt = guard
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .expect("prepare plan");
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(3))
        .expect("plan rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect plan");
    rows
}

/// The list query must use the partial index and must not sort in a temp
/// b-tree.
#[test]
fn list_sessions_plan_uses_the_partial_index_without_a_temp_sort() {
    let storage = storage();
    let plan = plan(
        &storage,
        "SELECT id, title, project_id, directory, parent_id, version, format_version, \
         created_at, updated_at, archived_at, summary \
         FROM sessions WHERE archived_at IS NULL ORDER BY updated_at DESC",
    );
    let joined = plan.join("\n");
    assert!(
        joined.contains("idx_sessions_active_updated"),
        "list_sessions must be served by idx_sessions_active_updated; plan was:\n{joined}"
    );
    assert!(
        !joined.contains("TEMP B-TREE"),
        "list_sessions must not sort in a temp b-tree; plan was:\n{joined}"
    );
}

/// An archived session in the middle of the ordering must be excluded while the
/// remaining rows stay ordered `updated_at DESC`.
#[test]
fn list_sessions_excludes_archived_rows_and_keeps_desc_order() {
    let storage = storage();
    storage.create_session("old", "/w").expect("old");
    storage.create_session("mid", "/w").expect("mid");
    storage.create_session("new", "/w").expect("new");
    storage.create_session("gone", "/w").expect("gone");

    // Force a deterministic ordering: the timestamps are set directly.
    {
        let guard = storage.conn_lock_for_test().expect("lock");
        for (id, ts) in [
            ("old", "2024-01-01T00:00:00Z"),
            ("mid", "2024-01-02T00:00:00Z"),
            ("new", "2024-01-03T00:00:00Z"),
            ("gone", "2024-01-04T00:00:00Z"),
        ] {
            guard
                .execute(
                    "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
                    [ts, id],
                )
                .expect("backdate");
        }
        guard
            .execute(
                "UPDATE sessions SET archived_at = ?1 WHERE id = 'gone'",
                ["2024-02-01T00:00:00Z"],
            )
            .expect("archive");
    }

    let ids: Vec<String> = storage
        .list_sessions()
        .expect("list")
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(
        ids,
        vec!["new", "mid", "old"],
        "archived row excluded, newest first"
    );
}

/// The partial index lives in the migration batch, so the version that
/// introduces it must be the current one.
#[test]
fn partial_index_is_part_of_the_versioned_migration() {
    let storage = storage();
    let guard = storage.conn_lock_for_test().expect("lock");
    let found: i64 = guard
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type = 'index' AND name = 'idx_sessions_active_updated'",
            [],
            |r| r.get(0),
        )
        .expect("count index");
    assert_eq!(found, 1);
    let version: String = guard
        .query_row(
            "SELECT value FROM settings WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .expect("version");
    assert_eq!(version, SCHEMA_VERSION.to_string());
    drop(guard);

    // A file-backed re-open hits the fast path and keeps the index.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/temp/t05_sessions");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("ragent.db");
    {
        let s = Storage::open(&path).expect("open");
        drop(s);
    }
    let s = Storage::open(&path).expect("reopen");
    let guard = s.conn_lock_for_test().expect("lock");
    let found: i64 = guard
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type = 'index' AND name = 'idx_sessions_active_updated'",
            [],
            |r| r.get(0),
        )
        .expect("count index after reopen");
    assert_eq!(found, 1);
    drop(guard);
    let _ = std::fs::remove_dir_all(&dir);
}

//! T-06: `list_tasks` must not sort in a temp b-tree, and the ordering must be
//! deterministic for tasks created in the same second.
//!
//! `ORDER BY created_at, id` is fully served by `idx_todos_session_created
//! (session_id, created_at, id)`; the `id` tie-break is in the index because
//! otherwise SQLite reports "USE TEMP B-TREE FOR LAST TERM OF ORDER BY".
//!
//! Both variants (with and without a status filter) used to show
//! `USE TEMP B-TREE FOR ORDER BY` because `ORDER BY created_at` is not a prefix
//! of `(session_id, status)`.

use ragent_storage::storage::Storage;

fn storage() -> Storage {
    Storage::open_in_memory().expect("storage")
}

fn plan(storage: &Storage, sql: &str) -> String {
    let guard = storage.conn_lock_for_test().expect("lock");
    let mut stmt = guard
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .expect("prepare plan");
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(3))
        .expect("plan rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect plan");
    rows.join("\n")
}

/// Neither list variant may sort in a temp b-tree.
#[test]
fn neither_list_tasks_variant_sorts_in_a_temp_b_tree() {
    let storage = storage();
    for (label, sql) in [
        (
            "no status filter",
            "SELECT id, session_id, title, status, description, created_at, updated_at, \
             active_form, owner, metadata, blocked_by \
             FROM todos WHERE session_id = 's' ORDER BY created_at, id",
        ),
        (
            "status filter",
            "SELECT id, session_id, title, status, description, created_at, updated_at, \
             active_form, owner, metadata, blocked_by \
             FROM todos WHERE session_id = 's' AND status = 'pending' \
             ORDER BY created_at, id",
        ),
    ] {
        let joined = plan(&storage, sql);
        assert!(
            !joined.contains("TEMP B-TREE"),
            "list_tasks ({label}) must not sort in a temp b-tree; plan was:\n{joined}"
        );
    }
}

/// Two tasks created in the same second must come back in a stable order.
#[test]
fn tasks_created_in_the_same_second_order_stably() {
    let storage = storage();
    storage.create_session("s", "/w").expect("session");
    for id in ["t-c", "t-a", "t-b"] {
        storage
            .create_task_simple(id, "s", "title", "pending", "desc")
            .expect("create task");
    }

    // Collapse the timestamps so only the tie-break can order them.
    {
        let guard = storage.conn_lock_for_test().expect("lock");
        guard
            .execute(
                "UPDATE todos SET created_at = '2024-05-05T00:00:00Z' WHERE session_id = 's'",
                [],
            )
            .expect("collapse timestamps");
    }

    let listed = storage.list_tasks("s", None).expect("list all");
    let filtered = storage
        .list_tasks("s", Some("pending"))
        .expect("list pending");
    let ids_all: Vec<&str> = listed.iter().map(|t| t.id.as_str()).collect();
    let ids_filtered: Vec<&str> = filtered.iter().map(|t| t.id.as_str()).collect();

    assert_eq!(
        ids_all,
        vec!["t-a", "t-b", "t-c"],
        "same-second tasks must be ordered by id, deterministically"
    );
    assert_eq!(ids_filtered, ids_all, "the filtered variant agrees");
}

/// The index must exist after a migration and survive the fast-path re-open.
#[test]
fn todos_ordering_index_survives_the_schema_fast_path() {
    use std::path::Path;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/temp/t06_todos");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("ragent.db");
    drop(Storage::open(&path).expect("open"));
    let storage = Storage::open(&path).expect("reopen");

    let guard = storage.conn_lock_for_test().expect("lock");
    let found: i64 = guard
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type = 'index' AND name = 'idx_todos_session_created'",
            [],
            |r| r.get(0),
        )
        .expect("count index");
    assert_eq!(found, 1);
    drop(guard);
    let _ = std::fs::remove_dir_all(&dir);
}

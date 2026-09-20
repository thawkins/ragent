//! todo2tasks T-002: tests for the additive SQLite migration that adds
//! `active_form`, `owner`, `metadata` (TEXT JSON), and `blocked_by`
//! (TEXT JSON array) columns to the `todos` table.
//!
//! These tests verify:
//! - The new columns exist after `open_in_memory` / `migrate`.
//! - Existing rows (created before the migration conceptually) receive
//!   safe defaults per FR-002.
//! - `list_tasks` reads the new columns and maps them correctly.
//! - The migration is idempotent (running migrate twice does not error).

use ragent_storage::storage::Storage;

/// Acquire the internal connection lock for direct SQL in tests.
macro_rules! storage_conn {
    ($storage:expr) => {
        $storage.conn_lock_for_test().expect("database lock")
    };
}

/// Verify that the four new Task-model columns exist in the `todos`
/// table after `open_in_memory` (which runs `migrate`).
#[test]
fn test_todos_table_has_task_columns_after_migration() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    let conn = storage_conn!(storage);
    for col in &["active_form", "owner", "metadata", "blocked_by"] {
        let count: i64 = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('todos') WHERE name = ?1")
            .unwrap()
            .query_row([col], |r| r.get(0))
            .unwrap();
        assert_eq!(
            count, 1,
            "column `{col}` should exist in todos table after migration"
        );
    }
}

/// Verify that a row created via `create_task_simple` (which does not set the
/// new columns) gets the safe column defaults when read back via
/// `list_tasks` — FR-002.
#[test]
fn test_create_task_simple_gets_safe_defaults_for_new_columns() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-defaults", "/tmp/defaults")
        .expect("create session");
    storage
        .create_task_simple("todo-1", "sess-defaults", "Write tests", "pending", "TDD")
        .expect("create todo");

    let todos = storage
        .list_tasks("sess-defaults", None)
        .expect("get todos");
    assert_eq!(todos.len(), 1);
    let row = &todos[0];
    assert_eq!(row.id, "todo-1");
    assert_eq!(row.title, "Write tests");
    assert_eq!(row.status, "pending");
    // Safe defaults from ADD COLUMN DEFAULT (FR-002):
    assert!(row.active_form.is_none(), "active_form defaults to NULL");
    assert!(row.owner.is_none(), "owner defaults to NULL");
    assert_eq!(row.metadata, "{}", "metadata defaults to '{{}}'");
    assert!(
        row.blocked_by.is_empty(),
        "blocked_by defaults to empty array"
    );
}

/// Verify that `list_tasks` reads the new columns correctly when they are
/// populated via direct SQL (simulating what the task_* tools will do in
/// T-003+).
#[test]
fn test_list_tasks_reads_populated_task_columns() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-populated", "/tmp/populated")
        .expect("create session");
    storage
        .create_task_simple("todo-a", "sess-populated", "Task A", "completed", "Done")
        .expect("create todo A");
    storage
        .create_task_simple("todo-b", "sess-populated", "Task B", "pending", "Blocked")
        .expect("create todo B");

    // Populate the new columns directly (as the task_* tools will in T-003+).
    {
        let conn = storage_conn!(storage);
        conn.execute(
            "UPDATE todos SET active_form = 'Implementing Task A', owner = 'coder', \
             metadata = '{\"priority\":\"high\"}', blocked_by = '[]' \
             WHERE id = 'todo-a'",
            [],
        )
        .expect("update todo-a");
        conn.execute(
            "UPDATE todos SET active_form = 'Waiting on Task A', owner = 'tester', \
             metadata = '{\"priority\":\"low\",\"phase\":2}', blocked_by = '[\"todo-a\"]' \
             WHERE id = 'todo-b'",
            [],
        )
        .expect("update todo-b");
    }

    let todos = storage
        .list_tasks("sess-populated", None)
        .expect("get todos");
    assert_eq!(todos.len(), 2);

    let a = todos.iter().find(|t| t.id == "todo-a").expect("todo-a");
    assert_eq!(a.active_form.as_deref(), Some("Implementing Task A"));
    assert_eq!(a.owner.as_deref(), Some("coder"));
    assert_eq!(a.metadata, r#"{"priority":"high"}"#);
    assert!(a.blocked_by.is_empty());

    let b = todos.iter().find(|t| t.id == "todo-b").expect("todo-b");
    assert_eq!(b.active_form.as_deref(), Some("Waiting on Task A"));
    assert_eq!(b.owner.as_deref(), Some("tester"));
    assert_eq!(b.metadata, r#"{"priority":"low","phase":2}"#);
    assert_eq!(b.blocked_by, vec!["todo-a"]);
}

/// Verify that the status filter path also reads the new columns.
#[test]
fn test_list_tasks_with_status_filter_reads_task_columns() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-filter", "/tmp/filter")
        .expect("create session");
    storage
        .create_task_simple("todo-p", "sess-filter", "Pending task", "pending", "")
        .expect("create pending todo");
    storage
        .create_task_simple("todo-d", "sess-filter", "Done task", "done", "")
        .expect("create done todo");

    // Populate active_form on the pending one.
    {
        let conn = storage_conn!(storage);
        conn.execute(
            "UPDATE todos SET active_form = 'Working on it' WHERE id = 'todo-p'",
            [],
        )
        .expect("update");
    }

    let pending = storage
        .list_tasks("sess-filter", Some("pending"))
        .expect("get pending todos");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "todo-p");
    assert_eq!(pending[0].active_form.as_deref(), Some("Working on it"));
}

/// Verify that a legacy `blocked_by` value that is not valid JSON does
/// not cause a panic — `map_task_row` falls back to an empty Vec.
#[test]
fn test_blocked_by_invalid_json_falls_back_to_empty() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-bad-json", "/tmp/bad-json")
        .expect("create session");
    storage
        .create_task_simple("todo-x", "sess-bad-json", "Bad JSON", "pending", "")
        .expect("create todo");

    // Write invalid JSON into blocked_by directly.
    {
        let conn = storage_conn!(storage);
        conn.execute(
            "UPDATE todos SET blocked_by = 'not valid json' WHERE id = 'todo-x'",
            [],
        )
        .expect("update");
    }

    let todos = storage
        .list_tasks("sess-bad-json", None)
        .expect("get todos");
    assert_eq!(todos.len(), 1);
    // Invalid JSON should fall back to empty Vec, not panic.
    assert!(todos[0].blocked_by.is_empty());
}

/// Verify the migration is idempotent: calling `migrate` again (or
/// opening a second Storage handle on the same DB file) does not error
/// and the columns remain present.
#[test]
fn test_migration_is_idempotent() {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target/temp/test_todo_migration_idempotent");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.db");

    // First open — runs migration, adds columns.
    {
        let storage = Storage::open(&db_path).expect("open storage");
        storage
            .create_session("sess-1", "/tmp/idempotent")
            .expect("create session");
        storage
            .create_task_simple("todo-1", "sess-1", "Test", "pending", "")
            .expect("create todo");
    }

    // Second open — migration should be a no-op, no errors.
    {
        let storage = Storage::open(&db_path).expect("reopen storage");
        let todos = storage.list_tasks("sess-1", None).expect("get todos");
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].id, "todo-1");
        // Columns still present with safe defaults.
        assert!(todos[0].active_form.is_none());
        assert!(todos[0].owner.is_none());
        assert_eq!(todos[0].metadata, "{}");
        assert!(todos[0].blocked_by.is_empty());
    }

    let _ = std::fs::remove_dir_all(&dir);
}

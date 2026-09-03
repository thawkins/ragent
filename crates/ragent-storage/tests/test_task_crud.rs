#![allow(clippy::assert_is_empty)]
//! todo2tasks T-003: tests for the new Task CRUD storage methods
//! (`create_task`, `get_task`, `list_tasks`, `update_task`).
//!
//! These tests verify that all Task-model columns round-trip through
//! the SQLite `todos` table via the new methods, covering FR-001
//! (session scoping), FR-014 (TaskGet output), and FR-015 (TaskList
//! output).

use ragent_storage::storage::{Storage, TaskUpdateParams};

// ── create_task + get_task round-trip ─────────────────────────────

/// Verify that `create_task` inserts all 11 columns and `get_task`
/// reads them back correctly.
#[test]
fn test_create_task_get_task_full_round_trip() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-rt", "/tmp/round-trip")
        .expect("create session");

    let blocked = vec!["task-a".to_string(), "task-b".to_string()];
    storage
        .create_task(
            "task-rt",
            "sess-rt",
            "Implement JWT auth",
            "Add JWT tokens to the auth middleware",
            "pending",
            Some("Implementing JWT auth"),
            Some("coder"),
            r#"{"priority":"high","phase":2}"#,
            &blocked,
        )
        .expect("create task");

    let row = storage
        .get_task("task-rt", "sess-rt")
        .expect("get task")
        .expect("task exists");
    assert_eq!(row.id, "task-rt");
    assert_eq!(row.session_id, "sess-rt");
    assert_eq!(row.title, "Implement JWT auth");
    assert_eq!(row.status, "pending");
    assert_eq!(row.description, "Add JWT tokens to the auth middleware");
    assert_eq!(row.active_form.as_deref(), Some("Implementing JWT auth"));
    assert_eq!(row.owner.as_deref(), Some("coder"));
    assert_eq!(row.metadata, r#"{"priority":"high","phase":2}"#);
    assert_eq!(row.blocked_by, vec!["task-a", "task-b"]);
}

/// Verify that `create_task` with minimal args (None active_form, None
/// owner, empty metadata, empty blocked_by) produces safe defaults.
#[test]
fn test_create_task_minimal_args_defaults() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-min", "/tmp/minimal")
        .expect("create session");

    storage
        .create_task(
            "task-min",
            "sess-min",
            "Simple task",
            "Just do it",
            "pending",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create task");

    let row = storage
        .get_task("task-min", "sess-min")
        .expect("get task")
        .expect("task exists");
    assert!(row.active_form.is_none());
    assert!(row.owner.is_none());
    assert_eq!(row.metadata, "{}");
    assert!(row.blocked_by.is_empty());
}

/// Verify that `get_task` returns None for a non-existent task.
#[test]
fn test_get_task_nonexistent_returns_none() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-ne", "/tmp/nonexistent")
        .expect("create session");

    let row = storage
        .get_task("no-such-task", "sess-ne")
        .expect("get task");
    assert!(row.is_none());
}

/// Verify that `get_task` is session-scoped — a task in session A is
/// not visible from session B (FR-001).
#[test]
fn test_get_task_session_scoped() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-a", "/tmp/a")
        .expect("create session A");
    storage
        .create_session("sess-b", "/tmp/b")
        .expect("create session B");

    storage
        .create_task(
            "task-shared",
            "sess-a",
            "Task in A",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create task in A");

    // Visible from A, not from B.
    assert!(storage.get_task("task-shared", "sess-a").unwrap().is_some());
    assert!(storage.get_task("task-shared", "sess-b").unwrap().is_none());
}

// ── list_tasks ────────────────────────────────────────────────────

/// Verify that `list_tasks` returns all tasks for a session ordered
/// by created_at (FR-015).
#[test]
fn test_list_tasks_ordered_by_created_at() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-list", "/tmp/list")
        .expect("create session");

    storage
        .create_task(
            "t1",
            "sess-list",
            "First",
            "desc 1",
            "pending",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create t1");
    // Small delay to ensure different created_at timestamps.
    std::thread::sleep(std::time::Duration::from_millis(10));
    storage
        .create_task(
            "t2",
            "sess-list",
            "Second",
            "desc 2",
            "in_progress",
            Some("Working"),
            None,
            "{}",
            &[],
        )
        .expect("create t2");
    std::thread::sleep(std::time::Duration::from_millis(10));
    storage
        .create_task(
            "t3",
            "sess-list",
            "Third",
            "desc 3",
            "completed",
            None,
            Some("tester"),
            "{}",
            &[],
        )
        .expect("create t3");

    let all = storage.list_tasks("sess-list", None).expect("list tasks");
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].id, "t1");
    assert_eq!(all[1].id, "t2");
    assert_eq!(all[2].id, "t3");
}

/// Verify that `list_tasks` with a status filter returns only matching
/// tasks (FR-015).
#[test]
fn test_list_tasks_status_filter() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-filter", "/tmp/filter")
        .expect("create session");

    storage
        .create_task(
            "tp",
            "sess-filter",
            "Pending",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create tp");
    storage
        .create_task(
            "ti",
            "sess-filter",
            "In Progress",
            "desc",
            "in_progress",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create ti");
    storage
        .create_task(
            "tc",
            "sess-filter",
            "Completed",
            "desc",
            "completed",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create tc");

    let pending = storage
        .list_tasks("sess-filter", Some("pending"))
        .expect("list pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "tp");

    let completed = storage
        .list_tasks("sess-filter", Some("completed"))
        .expect("list completed");
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].id, "tc");

    let all = storage
        .list_tasks("sess-filter", Some("all"))
        .expect("list all");
    assert_eq!(all.len(), 3);
}

/// Verify that `list_tasks` returns task-model columns (FR-015).
#[test]
fn test_list_tasks_includes_task_columns() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-cols", "/tmp/cols")
        .expect("create session");

    let blocked = vec!["dep-1".to_string()];
    storage
        .create_task(
            "task-cols",
            "sess-cols",
            "Task with cols",
            "desc",
            "in_progress",
            Some("Working on it"),
            Some("agent-1"),
            r#"{"feature":"auth"}"#,
            &blocked,
        )
        .expect("create task");

    let tasks = storage.list_tasks("sess-cols", None).expect("list");
    assert_eq!(tasks.len(), 1);
    let t = &tasks[0];
    assert_eq!(t.active_form.as_deref(), Some("Working on it"));
    assert_eq!(t.owner.as_deref(), Some("agent-1"));
    assert_eq!(t.metadata, r#"{"feature":"auth"}"#);
    assert_eq!(t.blocked_by, vec!["dep-1"]);
}

// ── update_task ───────────────────────────────────────────────────

/// Verify that `update_task` can update the subject (title) and status.
#[test]
fn test_update_task_subject_and_status() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-upd", "/tmp/update")
        .expect("create session");
    storage
        .create_task(
            "task-upd",
            "sess-upd",
            "Old subject",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create task");

    let params = TaskUpdateParams {
        subject: Some("New subject"),
        status: Some("in_progress"),
        ..Default::default()
    };
    let changed = storage
        .update_task("task-upd", "sess-upd", &params)
        .expect("update task");
    assert!(changed);

    let row = storage.get_task("task-upd", "sess-upd").unwrap().unwrap();
    assert_eq!(row.title, "New subject");
    assert_eq!(row.status, "in_progress");
}

/// Verify that `update_task` can set active_form and owner.
#[test]
fn test_update_task_set_active_form_and_owner() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-af", "/tmp/active-form")
        .expect("create session");
    storage
        .create_task(
            "task-af",
            "sess-af",
            "Task",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create task");

    let params = TaskUpdateParams {
        active_form: Some(Some("Writing tests")),
        owner: Some(Some("qa-agent")),
        ..Default::default()
    };
    storage
        .update_task("task-af", "sess-af", &params)
        .expect("update");

    let row = storage.get_task("task-af", "sess-af").unwrap().unwrap();
    assert_eq!(row.active_form.as_deref(), Some("Writing tests"));
    assert_eq!(row.owner.as_deref(), Some("qa-agent"));
}

/// Verify that `update_task` can clear active_form and owner by passing
/// `Some(None)`.
#[test]
fn test_update_task_clear_active_form_and_owner() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-clear", "/tmp/clear")
        .expect("create session");
    storage
        .create_task(
            "task-clear",
            "sess-clear",
            "Task",
            "desc",
            "pending",
            Some("Working"),
            Some("agent"),
            "{}",
            &[],
        )
        .expect("create task");

    let params = TaskUpdateParams {
        active_form: Some(None),
        owner: Some(None),
        ..Default::default()
    };
    storage
        .update_task("task-clear", "sess-clear", &params)
        .expect("update");

    let row = storage
        .get_task("task-clear", "sess-clear")
        .unwrap()
        .unwrap();
    assert!(row.active_form.is_none(), "active_form should be cleared");
    assert!(row.owner.is_none(), "owner should be cleared");
}

/// Verify that `update_task` can replace the metadata JSON blob.
#[test]
fn test_update_task_metadata() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-meta", "/tmp/metadata")
        .expect("create session");
    storage
        .create_task(
            "task-meta",
            "sess-meta",
            "Task",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create task");

    let params = TaskUpdateParams {
        metadata: Some(r#"{"phase":3,"priority":"critical"}"#),
        ..Default::default()
    };
    storage
        .update_task("task-meta", "sess-meta", &params)
        .expect("update");

    let row = storage.get_task("task-meta", "sess-meta").unwrap().unwrap();
    assert_eq!(row.metadata, r#"{"phase":3,"priority":"critical"}"#);
}

/// Verify that `update_task` can replace the blocked_by dependency list.
#[test]
fn test_update_task_blocked_by() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-bb", "/tmp/blocked-by")
        .expect("create session");
    storage
        .create_task(
            "task-bb",
            "sess-bb",
            "Task",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create task");

    let deps = vec!["dep-1".to_string(), "dep-2".to_string()];
    let params = TaskUpdateParams {
        blocked_by: Some(&deps),
        ..Default::default()
    };
    storage
        .update_task("task-bb", "sess-bb", &params)
        .expect("update");

    let row = storage.get_task("task-bb", "sess-bb").unwrap().unwrap();
    assert_eq!(row.blocked_by, vec!["dep-1", "dep-2"]);

    // Replace with a different list.
    let deps2 = vec!["dep-3".to_string()];
    let params2 = TaskUpdateParams {
        blocked_by: Some(&deps2),
        ..Default::default()
    };
    storage
        .update_task("task-bb", "sess-bb", &params2)
        .expect("update");

    let row = storage.get_task("task-bb", "sess-bb").unwrap().unwrap();
    assert_eq!(row.blocked_by, vec!["dep-3"]);
}

/// Verify that `update_task` can clear the blocked_by list by passing
/// an empty slice.
#[test]
fn test_update_task_clear_blocked_by() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-clear-bb", "/tmp/clear-bb")
        .expect("create session");
    let deps = vec!["dep-1".to_string()];
    storage
        .create_task(
            "task-cbb",
            "sess-clear-bb",
            "Task",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &deps,
        )
        .expect("create task");

    let empty: Vec<String> = Vec::new();
    let params = TaskUpdateParams {
        blocked_by: Some(&empty),
        ..Default::default()
    };
    storage
        .update_task("task-cbb", "sess-clear-bb", &params)
        .expect("update");

    let row = storage
        .get_task("task-cbb", "sess-clear-bb")
        .unwrap()
        .unwrap();
    assert!(row.blocked_by.is_empty());
}

/// Verify that `update_task` with all fields None only updates
/// `updated_at` and leaves everything else unchanged.
#[test]
fn test_update_task_no_fields_only_touches_updated_at() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-touch", "/tmp/touch")
        .expect("create session");
    storage
        .create_task(
            "task-touch",
            "sess-touch",
            "Original",
            "Original desc",
            "pending",
            Some("Original active"),
            Some("owner-1"),
            r#"{"k":"v"}"#,
            &["dep-1".to_string()],
        )
        .expect("create task");

    let original = storage
        .get_task("task-touch", "sess-touch")
        .unwrap()
        .unwrap();

    // Small delay so updated_at will differ.
    std::thread::sleep(std::time::Duration::from_millis(10));

    let params = TaskUpdateParams::default();
    storage
        .update_task("task-touch", "sess-touch", &params)
        .expect("update");

    let updated = storage
        .get_task("task-touch", "sess-touch")
        .unwrap()
        .unwrap();

    // All fields unchanged.
    assert_eq!(updated.title, original.title);
    assert_eq!(updated.status, original.status);
    assert_eq!(updated.description, original.description);
    assert_eq!(updated.active_form, original.active_form);
    assert_eq!(updated.owner, original.owner);
    assert_eq!(updated.metadata, original.metadata);
    assert_eq!(updated.blocked_by, original.blocked_by);
    // updated_at should have advanced.
    assert_ne!(updated.updated_at, original.updated_at);
}

/// Verify that `update_task` returns false for a non-existent task.
#[test]
fn test_update_task_nonexistent_returns_false() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-ne2", "/tmp/nonexistent2")
        .expect("create session");

    let params = TaskUpdateParams {
        status: Some("completed"),
        ..Default::default()
    };
    let changed = storage
        .update_task("no-such-task", "sess-ne2", &params)
        .expect("update");
    assert!(!changed);
}

/// Verify that `update_task` can update all fields at once.
#[test]
fn test_update_task_all_fields_at_once() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-all", "/tmp/all-fields")
        .expect("create session");
    storage
        .create_task(
            "task-all",
            "sess-all",
            "Old",
            "Old desc",
            "pending",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create task");

    let deps = vec!["d1".to_string(), "d2".to_string()];
    let params = TaskUpdateParams {
        subject: Some("New subject"),
        status: Some("completed"),
        description: Some("New description"),
        active_form: Some(Some("Finishing up")),
        owner: Some(Some("lead")),
        metadata: Some(r#"{"done":true}"#),
        blocked_by: Some(&deps),
    };
    storage
        .update_task("task-all", "sess-all", &params)
        .expect("update");

    let row = storage.get_task("task-all", "sess-all").unwrap().unwrap();
    assert_eq!(row.title, "New subject");
    assert_eq!(row.status, "completed");
    assert_eq!(row.description, "New description");
    assert_eq!(row.active_form.as_deref(), Some("Finishing up"));
    assert_eq!(row.owner.as_deref(), Some("lead"));
    assert_eq!(row.metadata, r#"{"done":true}"#);
    assert_eq!(row.blocked_by, vec!["d1", "d2"]);
}

// ── Legacy compatibility ──────────────────────────────────────────

/// Verify that a task created via the legacy `create_task_simple` method can
/// be read via the new `get_task` method with safe defaults (FR-002).
#[test]
fn test_legacy_create_task_simple_readable_via_get_task() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-legacy", "/tmp/legacy")
        .expect("create session");
    storage
        .create_task_simple(
            "todo-legacy",
            "sess-legacy",
            "Legacy todo",
            "pending",
            "Old style",
        )
        .expect("create legacy todo");

    let row = storage
        .get_task("todo-legacy", "sess-legacy")
        .unwrap()
        .unwrap();
    assert_eq!(row.id, "todo-legacy");
    assert_eq!(row.title, "Legacy todo");
    assert_eq!(row.status, "pending");
    assert_eq!(row.description, "Old style");
    // Safe defaults from T-002 migration.
    assert!(row.active_form.is_none());
    assert!(row.owner.is_none());
    assert_eq!(row.metadata, "{}");
    assert!(row.blocked_by.is_empty());
}

/// Verify that a task created via `create_task` can be updated via the
/// legacy `update_task_simple` method (cross-compatibility).
#[test]
fn test_task_created_then_updated_via_legacy_update_task_simple() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-cross", "/tmp/cross")
        .expect("create session");
    storage
        .create_task(
            "task-cross",
            "sess-cross",
            "Task",
            "desc",
            "pending",
            Some("Working"),
            Some("agent"),
            r#"{"k":1}"#,
            &["dep".to_string()],
        )
        .expect("create task");

    // Update via legacy method — should only touch title/status/description.
    storage
        .update_task_simple(
            "task-cross",
            "sess-cross",
            Some("Updated title"),
            Some("in_progress"),
            None,
        )
        .expect("legacy update");

    let row = storage
        .get_task("task-cross", "sess-cross")
        .unwrap()
        .unwrap();
    assert_eq!(row.title, "Updated title");
    assert_eq!(row.status, "in_progress");
    // Task-model fields preserved.
    assert_eq!(row.active_form.as_deref(), Some("Working"));
    assert_eq!(row.owner.as_deref(), Some("agent"));
    assert_eq!(row.metadata, r#"{"k":1}"#);
    assert_eq!(row.blocked_by, vec!["dep"]);
}

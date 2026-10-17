//! Isolation coverage for the background-task CRUD and retention cleanup
//! (`create_background_task`, `get_background_task`, `list_background_tasks`,
//! `update_background_task_status`, `append_background_task_output`,
//! `set_background_task_output`, `set_background_task_output_and_status`,
//! `delete_background_task`, `cleanup_background_tasks`).

use ragent_storage::storage::{BackgroundTaskRow, Storage};

fn storage() -> Storage {
    Storage::open_in_memory().expect("storage")
}

/// Create the parent sessions a background-task row's foreign key needs.
fn with_sessions(sessions: &[&str]) -> Storage {
    let storage = storage();
    for sid in sessions {
        storage
            .create_session(sid, &format!("/tmp/{sid}"))
            .expect("session");
    }
    storage
}

fn row(id: &str, session: &str, status: &str) -> BackgroundTaskRow {
    let now = chrono::Utc::now().to_rfc3339();
    BackgroundTaskRow {
        id: id.to_string(),
        session_id: session.to_string(),
        command: format!("echo {id}"),
        status: status.to_string(),
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        progress_json: "{}".to_string(),
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
    }
}

#[test]
fn create_get_and_list_background_tasks() {
    let storage = with_sessions(&["s1", "s2"]);
    storage
        .create_background_task(&row("t1", "s1", "running"))
        .expect("t1");
    storage
        .create_background_task(&row("t2", "s1", "completed"))
        .expect("t2");
    storage
        .create_background_task(&row("t3", "s2", "running"))
        .expect("t3");

    let fetched = storage
        .get_background_task("t1")
        .expect("get")
        .expect("present");
    assert_eq!(fetched.session_id, "s1");
    assert_eq!(fetched.status, "running");
    assert_eq!(fetched.command, "echo t1");

    assert!(
        storage
            .get_background_task("missing")
            .expect("get missing")
            .is_none()
    );

    // Filter by session.
    let s1 = storage
        .list_background_tasks(Some("s1"), None, 50)
        .expect("by session");
    assert_eq!(s1.len(), 2);
    // Filter by status.
    let running = storage
        .list_background_tasks(None, Some("running"), 50)
        .expect("by status");
    assert_eq!(running.len(), 2);
    // Both filters.
    let both = storage
        .list_background_tasks(Some("s2"), Some("running"), 50)
        .expect("both");
    assert_eq!(both.len(), 1);
    assert_eq!(both[0].id, "t3");
    // No filter.
    assert_eq!(
        storage
            .list_background_tasks(None, None, 50)
            .expect("all")
            .len(),
        3
    );
}

#[test]
fn status_and_output_updates() {
    let storage = with_sessions(&["s1"]);
    storage
        .create_background_task(&row("t1", "s1", "running"))
        .expect("create");

    storage
        .update_background_task_status(
            "t1",
            "completed",
            Some(0),
            Some("2024-01-01T00:00:00+00:00"),
        )
        .expect("status");
    let t = storage
        .get_background_task("t1")
        .expect("get")
        .expect("row");
    assert_eq!(t.status, "completed");
    assert_eq!(t.exit_code, Some(0));
    assert_eq!(t.completed_at.as_deref(), Some("2024-01-01T00:00:00+00:00"));

    storage
        .append_background_task_output("t1", "line1\n", "err1\n", "{\"p\":1}")
        .expect("append");
    storage
        .append_background_task_output("t1", "line2\n", "err2\n", "{\"p\":2}")
        .expect("append again");
    let t = storage
        .get_background_task("t1")
        .expect("get")
        .expect("row");
    assert_eq!(t.stdout, "line1\nline2\n", "appends concatenate");
    assert_eq!(t.stderr, "err1\nerr2\n");
    assert_eq!(t.progress_json, "{\"p\":2}");

    // set_* replaces rather than appends.
    storage
        .set_background_task_output("t1", "fresh out", "fresh err", "{}")
        .expect("set output");
    let t = storage
        .get_background_task("t1")
        .expect("get")
        .expect("row");
    assert_eq!(t.stdout, "fresh out");
    assert_eq!(t.stderr, "fresh err");

    // Combined output+status writer.
    storage
        .set_background_task_output_and_status(
            "t1",
            "final out",
            "final err",
            "{\"done\":true}",
            "failed",
            Some(1),
            Some("2024-02-02T00:00:00+00:00"),
        )
        .expect("combined");
    let t = storage
        .get_background_task("t1")
        .expect("get")
        .expect("row");
    assert_eq!(t.status, "failed");
    assert_eq!(t.exit_code, Some(1));
    assert_eq!(t.stdout, "final out");
    assert_eq!(t.progress_json, "{\"done\":true}");
}

#[test]
fn delete_background_task_removes_row() {
    let storage = with_sessions(&["s1"]);
    storage
        .create_background_task(&row("t1", "s1", "completed"))
        .expect("create");
    storage.delete_background_task("t1").expect("delete");
    assert!(storage.get_background_task("t1").expect("get").is_none());
}

#[test]
fn cleanup_removes_only_completed_rows_when_capped() {
    let storage = with_sessions(&["s1"]);
    storage
        .create_background_task(&row("done", "s1", "completed"))
        .expect("done");
    storage
        .create_background_task(&row("run", "s1", "running"))
        .expect("run");

    // completed_only=true: only the completed row is a candidate; with a
    // 0-minute window both rows are "older than now", so the running row
    // survives and the completed one is removed.
    let removed = storage
        .cleanup_background_tasks(Some("s1"), 0, true)
        .expect("cleanup");
    assert_eq!(removed, 1);
    assert!(
        storage
            .get_background_task("done")
            .expect("get done")
            .is_none(),
        "completed row removed"
    );
    assert!(
        storage
            .get_background_task("run")
            .expect("get run")
            .is_some(),
        "running row survives a completed-only cleanup"
    );
}

#[test]
fn cleanup_can_remove_all_and_rejects_out_of_range_window() {
    let storage = with_sessions(&["s1"]);
    storage
        .create_background_task(&row("a", "s1", "running"))
        .expect("a");
    storage
        .create_background_task(&row("b", "s1", "failed"))
        .expect("b");

    // completed_only=false removes every aged row regardless of status.
    let removed = storage
        .cleanup_background_tasks(None, 0, false)
        .expect("cleanup all");
    assert_eq!(removed, 2);

    // An absurd retention window must return an error, not panic (SEC-049).
    let err = storage
        .cleanup_background_tasks(None, i64::MAX, true)
        .expect_err("out of range");
    assert!(
        err.to_string().contains("out of range"),
        "typed error: {err}"
    );
}

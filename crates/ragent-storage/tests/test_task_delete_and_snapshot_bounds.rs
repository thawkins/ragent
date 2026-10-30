//! `delete_task`, `clear_tasks` and `take_snapshot`'s boundary behaviour.
//!
//! The audit recorded these three storage paths as untested, including the
//! credential- and snapshot-bearing parts of `Storage`. Each test below pins a
//! contract that a plausible regression would break:
//!
//! 1. `delete_task` removes only the named task, reports `true`, and a second
//!    delete of the same id reports `false` (the return value is what the
//!    tool layer reports as "deleted").
//! 2. `delete_task` is session-scoped: a delete that names the wrong session
//!    removes nothing and reports `false`, so one session cannot delete
//!    another's task.
//! 3. `clear_tasks` removes every task for its session, returns the removed
//!    count, and leaves other sessions untouched.
//! 4. `clear_tasks` on a session with no tasks removes nothing and returns `0`
//!    (not an error, and not a count of someone else's rows).
//! 5. `take_snapshot` records only files that exist, so a nonexistent path is
//!    skipped rather than producing an entry that `restore_snapshot` would
//!    then fail to write.
//! 6. `take_snapshot` with no readable files still yields a well-formed, empty
//!    snapshot - the documented behaviour, and the one `expect_err` would
//!    misdescribe.
//!
//! Every fixture is an in-memory database or a `tempfile::tempdir()`, so
//! nothing is written into the repository.

use std::path::PathBuf;

use ragent_storage::snapshot::take_snapshot;
use ragent_storage::storage::Storage;

/// A storage with one session and `titles` seeded into it as `task-N`.
fn storage_with_tasks(session_id: &str, titles: &[&str]) -> Storage {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session(session_id, "/tmp/task-scope")
        .expect("create session");
    for (index, title) in titles.iter().enumerate() {
        storage
            .create_task_simple(
                &format!("task-{}", index + 1),
                session_id,
                title,
                "pending",
                "",
            )
            .expect("seed task");
    }
    storage
}

#[test]
fn test_delete_task_removes_only_the_named_task() {
    let storage = storage_with_tasks("sess-a", &["one", "two"]);

    assert!(
        storage
            .delete_task("task-1", "sess-a")
            .expect("delete task-1"),
        "deleting an existing task must report true"
    );

    assert!(
        storage.get_task("task-1", "sess-a").expect("get").is_none(),
        "the deleted task must be gone"
    );
    assert!(
        storage.get_task("task-2", "sess-a").expect("get").is_some(),
        "a delete must not touch its neighbour"
    );
    assert!(
        !storage.delete_task("task-1", "sess-a").expect("re-delete"),
        "deleting an already-deleted task must report false"
    );
}

#[test]
fn test_delete_task_is_session_scoped() {
    let storage = storage_with_tasks("sess-a", &["one"]);

    assert!(
        !storage
            .delete_task("task-1", "sess-other")
            .expect("delete from another session"),
        "a delete naming the wrong session must remove nothing"
    );
    assert!(
        storage.get_task("task-1", "sess-a").expect("get").is_some(),
        "the task must survive a delete issued from another session"
    );
}

#[test]
fn test_clear_tasks_removes_the_session_and_reports_the_count() {
    let storage = storage_with_tasks("sess-a", &["one", "two", "three"]);
    // A second session's task must survive the sweep.
    storage
        .create_session("sess-b", "/tmp/task-scope-b")
        .expect("create session b");
    storage
        .create_task_simple("task-b1", "sess-b", "other work", "pending", "")
        .expect("seed task b1");

    let removed = storage.clear_tasks("sess-a").expect("clear tasks");

    assert_eq!(removed, 3, "clear_tasks must report every removed row");
    assert!(
        storage.list_tasks("sess-a", None).expect("list").is_empty(),
        "no task may remain in the cleared session"
    );
    assert_eq!(
        storage.list_tasks("sess-b", None).expect("list").len(),
        1,
        "another session's tasks must not be cleared"
    );
}

#[test]
fn test_clear_tasks_on_an_empty_session_reports_zero() {
    let storage = storage_with_tasks("sess-a", &[]);

    assert_eq!(
        storage.clear_tasks("sess-a").expect("clear tasks"),
        0,
        "clearing an empty session must report 0, not an error"
    );
}

#[test]
fn test_take_snapshot_skips_nonexistent_paths() {
    let dir = tempfile::tempdir().expect("tempdir");
    let present = dir.path().join("present.txt");
    std::fs::write(&present, "contents").expect("write file");
    let missing = dir.path().join("missing.txt");

    let snapshot = take_snapshot("sess", "msg", &[present.clone(), missing.clone()])
        .expect("a snapshot with one unreadable path must still succeed");

    assert!(
        snapshot.files.contains_key(&present),
        "the existing file must be captured"
    );
    assert!(
        !snapshot.files.contains_key(&missing),
        "a nonexistent path must be skipped, not recorded with empty bytes"
    );
    assert_eq!(snapshot.files.len(), 1);
    assert_eq!(snapshot.session_id, "sess");
    assert_eq!(snapshot.message_id, "msg");
}

#[test]
fn test_take_snapshot_of_only_missing_paths_is_empty_but_well_formed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing: Vec<PathBuf> = vec![dir.path().join("nope-a"), dir.path().join("nope-b")];

    let snapshot =
        take_snapshot("sess", "msg", &missing).expect("an all-missing snapshot must not error");

    assert!(
        snapshot.files.is_empty(),
        "no file may be recorded for paths that do not exist"
    );
    assert!(
        !snapshot.id.is_empty(),
        "the snapshot must still carry a generated id"
    );
}

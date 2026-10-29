//! FR-037: a malformed `tasks.json` is diagnosed, not discarded.
//!
//! The pre-fix `TaskStore` read path had a `TaskList::default()` fallback, so a
//! broken `tasks.json` silently became an empty list and the *next* write
//! persisted that empty list over every task the file held. T-032 replaces that
//! fallback with one shared parse helper whose error names the file, the byte
//! offset and the offending field.
//!
//! Each test therefore asserts on three things: the read **fails**, the failure
//! **names** what an operator needs, and the file on disk is **byte-identical**
//! afterwards (no write happened, so nothing was lost).

use ragent_agent::team::{Task, TaskStatus, TaskStore};

#[path = "team_support/mod.rs"]
mod team_support;
use team_support::setup_team_with_task_store;

/// A team directory with one good task written, plus the store for it.
///
/// Returns `(tmp, team_dir, store)`; the `TempDir` must outlive the test body.
fn team_with_one_task() -> (tempfile::TempDir, std::path::PathBuf, TaskStore) {
    let (tmp, team_dir, store) = setup_team_with_task_store();
    let task = Task::new("task-001", "parse probe");
    // Block on the async mutator: it fires the team's `TaskCreated` hook.
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(store.add_task(task))
        .expect("seed task");
    (tmp, team_dir, store)
}

/// Write `contents` over the team's `tasks.json`, returning the path.
fn corrupt(team_dir: &std::path::Path, contents: &str) -> std::path::PathBuf {
    let path = team_dir.join("tasks.json");
    std::fs::write(&path, contents).expect("write corrupt tasks.json");
    path
}

#[test]
fn a_truncated_file_is_an_error_not_an_empty_list() {
    let (_tmp, team_dir, store) = team_with_one_task();
    // The TESTPLAN's own corruption: remove the file's final closing brace.
    let original = std::fs::read_to_string(team_dir.join("tasks.json")).expect("read");
    let trimmed = original.trim_end();
    assert!(trimmed.ends_with('}'), "fixture must end with a brace");
    let broken = trimmed[..trimmed.len() - 1].to_string();
    assert_ne!(broken, original, "the fixture must actually differ");
    corrupt(&team_dir, &broken);

    let err = store.read().expect_err("a malformed file must not read");
    let text = err.to_string().to_ascii_lowercase();
    assert!(
        text.contains("malformed"),
        "the error must say the file is malformed: {err}"
    );
    assert!(
        text.contains("tasks.json"),
        "the error must name the file: {err}"
    );
}

#[test]
fn the_error_names_the_file_byte_offset_and_offending_field() {
    let (_tmp, team_dir, store) = team_with_one_task();
    // An unknown key inside the first task: `#[serde(deny_unknown_fields)]`
    // rejects it, and serde names the key in its message.
    let bad = r#"{
  "schema_version": 1,
  "team_name": "task-store-team",
  "tasks": [
    {
      "id": "task-001",
      "title": "parse probe",
      "status": "pending",
      "assigned_to": null,
      "created_at": "2024-01-01T00:00:00Z",
      "bogus_field": 1
    }
  ],
  "updated_at": null
}
"#;
    let path = corrupt(&team_dir, bad);

    let err = store.read().expect_err("an unknown field must be refused");
    let text = err.to_string();

    assert!(
        text.contains("bogus_field"),
        "the error must name the offending field: {text}"
    );
    assert!(
        text.contains("byte offset:"),
        "the error must report a byte offset: {text}"
    );
    assert!(
        text.contains("line 11"),
        "the error must report the line the failure is on: {text}"
    );
    assert!(
        text.contains(&path.display().to_string()),
        "the error must name the file path: {text}"
    );
}

#[test]
fn a_wrong_field_type_names_the_field() {
    let (_tmp, team_dir, store) = team_with_one_task();
    // `tasks` must be an array; a string is a type mismatch on a named key.
    let bad = r#"{
  "schema_version": 1,
  "team_name": "task-store-team",
  "tasks": "not an array",
  "updated_at": null
}
"#;
    corrupt(&team_dir, bad);

    let err = store.read().expect_err("a wrong type must be refused");
    let text = err.to_string();
    assert!(
        text.contains("`tasks`"),
        "the error must name the `tasks` field: {text}"
    );
    assert!(
        text.contains("wrong type"),
        "the error must say the type was wrong: {text}"
    );
}

#[test]
fn a_failed_read_does_not_write_the_empty_list_back() {
    let (_tmp, team_dir, store) = team_with_one_task();
    let broken = r#"{ "schema_version": 1, "team_name": "x", "tasks": [ "#;
    let path = corrupt(&team_dir, broken);
    let before = std::fs::read(&path).expect("read broken file");

    // Read several times: no path may persist anything over the broken file.
    for _ in 0..3 {
        assert!(store.read().is_err(), "the read must keep failing");
    }

    let after = std::fs::read(&path).expect("read broken file");
    assert_eq!(
        before, after,
        "a refused read must leave tasks.json byte-identical"
    );
}

#[test]
fn a_mutating_path_refuses_a_malformed_file_instead_of_overwriting_it() {
    let (_tmp, team_dir, store) = team_with_one_task();
    let broken = r#"{ "schema_version": 1, "tasks": [ { "id": "task-001" "#;
    let path = corrupt(&team_dir, broken);
    let before = std::fs::read(&path).expect("read broken file");

    // Every write path re-reads under the lock. Each must refuse rather than
    // parse-fail into an empty list and write that back.
    let err = store
        .write_through(|list| {
            list.tasks.clear();
        })
        .expect_err("a write over a malformed file must be refused");
    assert!(err.to_string().contains("malformed"), "{err}");

    let after = std::fs::read(&path).expect("read broken file");
    assert_eq!(
        before, after,
        "a refused write must leave the broken file byte-identical"
    );
}

#[test]
fn an_empty_file_still_reads_as_a_default_list() {
    // An empty file is a team that has never had a task, not a malformed one:
    // FR-037 forbids the *parse-failure* fallback, not the missing-file default.
    let (_tmp, team_dir, store) = team_with_one_task();
    corrupt(&team_dir, "");

    let list = store.read().expect("an empty file is not malformed");
    assert!(list.tasks.is_empty());
}

#[test]
fn a_whitespace_only_file_still_reads_as_a_default_list() {
    let (_tmp, team_dir, store) = team_with_one_task();
    corrupt(&team_dir, "  \n\t\n ");

    let list = store.read().expect("whitespace is not malformed");
    assert!(list.tasks.is_empty());
}

#[test]
fn a_good_file_still_reads() {
    let (_tmp, team_dir, store) = team_with_one_task();
    let list = store.read().expect("a good file reads");
    assert_eq!(list.tasks.len(), 1);
    let task = &list.tasks[0];
    assert_eq!(task.id, "task-001");
    assert_eq!(task.status, TaskStatus::Pending);
    // The path is unmodified by a successful read.
    assert!(team_dir.join("tasks.json").exists());
}

#[test]
fn restoring_the_file_restores_the_task() {
    // TESTPLAN step 8: the task is listed again once the good file is back.
    let (_tmp, team_dir, store) = team_with_one_task();
    let good = std::fs::read_to_string(team_dir.join("tasks.json")).expect("read");
    corrupt(&team_dir, "garbage");
    assert!(store.read().is_err());

    corrupt(&team_dir, &good);
    let list = store.read().expect("restored file reads");
    assert_eq!(list.tasks.len(), 1, "the task is listed again");
}

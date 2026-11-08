//! Regression tests for Agents-panel lifecycle bugs found in the agents review.
//!
//! - A failed or timed-out run must enter the completed history with its real
//!   status, not as `completed`, and must carry the durable `output_file` the
//!   registry recorded (the completion event does not carry it, so the history
//!   used to claim every report was in-memory only).
//! - A plain kill must keep the row painted as `terminating` until the run
//!   settles; a forced kill must record the run into history as `failed`.
//! - The registry reconciliation must keep a `terminating` row, and retire a
//!   finished row (including `timed_out`) into history.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_tui::App;
use ragent_tui::app::{
    note_subagent_completed_with_registry_for_tests, note_subagent_killed_for_tests,
};
use support::make_app;

const LEAD_SESSION: &str = "lead-session-lifecycle";

fn task(id: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: LEAD_SESSION.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: "explore".to_string(),
        task_prompt: "prompt".to_string(),
        background: true,
        detached: false,
        status,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
        time_bound_secs: None,
    }
}

fn app() -> App {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agents_history_cap = 8;
    app
}

#[test]
fn test_failed_completion_is_recorded_as_failed() {
    let mut app = app();
    app.active_tasks.push(task("t-fail", TaskStatus::Running));

    note_subagent_completed_with_registry_for_tests(&mut app, "t-fail", "error", false, None);

    let row = app.agents_history.front().expect("history row");
    assert_eq!(row.status, TaskStatus::Failed);
}

#[test]
fn test_completion_uses_registry_status_and_output_file() {
    let mut app = app();
    app.active_tasks.push(task("t-to", TaskStatus::Running));
    let mut registry = task("t-to", TaskStatus::TimedOut);
    registry.output_file = Some(PathBuf::from("log/subagents/t-to.md"));

    note_subagent_completed_with_registry_for_tests(
        &mut app,
        "t-to",
        "error",
        false,
        Some(&registry),
    );

    let row = app.agents_history.front().expect("history row");
    assert_eq!(row.status, TaskStatus::TimedOut);
    assert_eq!(
        row.output_file.as_deref(),
        Some(std::path::Path::new("log/subagents/t-to.md"))
    );
    assert!(row.durable_copy_absence().is_none());
}

#[test]
fn test_plain_kill_keeps_row_as_terminating() {
    let mut app = app();
    app.active_tasks.push(task("t-kill", TaskStatus::Running));

    assert!(!note_subagent_killed_for_tests(&mut app, "t-kill", false));

    assert_eq!(app.active_tasks.len(), 1, "a kill request is not final");
    assert_eq!(app.active_tasks[0].status, TaskStatus::Terminating);
    assert!(app.agents_history.is_empty());
}

#[test]
fn test_force_kill_records_failed_history_row() {
    let mut app = app();
    app.active_tasks
        .push(task("t-force", TaskStatus::Terminating));

    assert!(note_subagent_killed_for_tests(&mut app, "t-force", true));

    assert!(app.active_tasks.is_empty());
    let row = app.agents_history.front().expect("history row");
    assert_eq!(row.id, "t-force");
    assert_eq!(row.status, TaskStatus::Failed);
}

#[test]
fn test_reconcile_keeps_terminating_and_retires_timed_out() {
    let mut app = app();
    app.active_tasks
        .push(task("t-term", TaskStatus::Terminating));
    app.active_tasks.push(task("t-ghost", TaskStatus::Running));

    let mut ghost = task("t-ghost", TaskStatus::TimedOut);
    ghost.output_file = Some(PathBuf::from("log/subagents/t-ghost.md"));
    app.apply_registry_reconciliation(vec![task("t-term", TaskStatus::Terminating), ghost]);

    let live: Vec<&str> = app.active_tasks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(live, vec!["t-term"], "a terminating run is still live");
    let row = app
        .agents_history
        .front()
        .expect("ghost retired into history");
    assert_eq!(row.id, "t-ghost");
    assert_eq!(row.status, TaskStatus::TimedOut);
    assert_eq!(
        row.output_file.as_deref(),
        Some(std::path::Path::new("log/subagents/t-ghost.md"))
    );
}

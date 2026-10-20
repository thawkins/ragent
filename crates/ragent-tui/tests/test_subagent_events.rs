//! Tests for the sub-agent event plumbing behind the Agents window
//! (spec `agentui` T-013, FR-047, FR-048, FR-049, FR-072).
//!
//! `App::active_tasks` is the projection the Agents table paints (A2), so these
//! cases drive the real public `App::handle_event` entry point with synthetic
//! `Event` values and assert on the resulting state and log:
//!
//! - a spawn appends one running row, including for a descendant session
//!   (FR-047);
//! - a completion records the report status derived from the finish reason,
//!   removes the row and logs the truncation marker when the provider cut the
//!   reply (FR-048);
//! - a cancel removes the row and a suspend / resume re-statuses it (FR-049);
//! - a replayed spawn updates the painted row instead of appending a second one,
//!   matched by task id or by agent (child session) id (FR-072).
//!
//! No test writes into the repository working tree: every fixture is in-memory
//! and `make_app` points the log root at the ignored build scratch directory
//! (NFR-012).

use ragent_agent::event::Event;
use ragent_agent::task::{ReportStatus, TaskStatus};
use ragent_tui::App;

mod support;

use support::make_app;

/// An app whose primary session is `s1`.
fn app_with_session(session: &str) -> App {
    let mut app = make_app();
    app.session_id = Some(session.to_string());
    app.agent_name = "general".to_string();
    app
}

/// Publish a `SubagentStart` for `task` / `child` under `session`.
async fn start(app: &mut App, session: &str, task: &str, child: &str, agent: &str) {
    app.handle_event(Event::SubagentStart {
        session_id: session.to_string(),
        task_id: task.to_string(),
        child_session_id: child.to_string(),
        agent: agent.to_string(),
        task: "do work".to_string(),
        background: true,
    })
    .await;
}

/// Publish a `SubagentComplete` for `task` with `finish_reason`.
async fn complete(app: &mut App, session: &str, task: &str, child: &str, reason: &str) {
    app.handle_event(Event::SubagentComplete {
        session_id: session.to_string(),
        task_id: task.to_string(),
        child_session_id: child.to_string(),
        summary: "report body".to_string(),
        success: true,
        duration_ms: 42,
        finish_reason: reason.to_string(),
    })
    .await;
}

/// The tracked task ids in paint order.
fn ids(app: &App) -> Vec<String> {
    app.active_tasks.iter().map(|t| t.id.clone()).collect()
}

/// Every log message, joined by newlines.
fn log_text(app: &App) -> String {
    app.log_entries
        .iter()
        .map(|e| e.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The number of log lines mentioning `needle`.
fn log_lines_containing(app: &App, needle: &str) -> usize {
    app.log_entries
        .iter()
        .filter(|e| e.message.contains(needle))
        .count()
}

/// The single tracked task, panicking when the row is missing.
fn only_task(app: &App) -> &ragent_agent::task::TaskEntry {
    assert_eq!(
        app.active_tasks.len(),
        1,
        "expected exactly one painted row, got {:?}",
        ids(app)
    );
    &app.active_tasks[0]
}

// -- FR-047: a spawn appends a running row ----------------------------------

#[tokio::test]
async fn test_start_appends_a_running_row() {
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;

    let task = only_task(&app);
    assert_eq!(task.id, "explore-a1b2c3d4");
    assert_eq!(task.parent_session_id, "s1");
    assert_eq!(task.child_session_id, "child-1");
    assert_eq!(task.agent_name, "explore");
    assert_eq!(task.task_prompt, "do work");
    assert!(task.background, "the spawn's background flag is carried");
    assert_eq!(task.status, TaskStatus::Running);
    assert_eq!(task.report_status, ReportStatus::default());
    assert!(
        task.completed_at.is_none(),
        "a live row carries no completion timestamp"
    );
    assert!(
        log_lines_containing(&app, "task started: explore-a1b2c3d4 (explore)") == 1,
        "the spawn is logged once: {:?}",
        log_text(&app)
    );
    assert!(app.needs_redraw, "the new row must repaint (NFR-005)");
}

#[tokio::test]
async fn test_start_for_a_descendant_session_is_tracked() {
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;
    // A nested sub-agent reports against its parent's child session.
    start(
        &mut app,
        "child-1",
        "reviewer-e5f6a7b8",
        "child-2",
        "reviewer",
    )
    .await;

    assert_eq!(
        ids(&app),
        vec![
            "explore-a1b2c3d4".to_string(),
            "reviewer-e5f6a7b8".to_string()
        ],
        "a descendant session's spawn still paints"
    );
    assert_eq!(
        app.active_tasks[1].parent_session_id, "child-1",
        "the row keeps its real parent so the tree can indent it"
    );
}

#[tokio::test]
async fn test_start_for_a_foreign_session_is_ignored() {
    let mut app = app_with_session("s1");
    start(
        &mut app,
        "someone-else",
        "explore-a1b2c3d4",
        "child-9",
        "explore",
    )
    .await;

    assert!(
        app.active_tasks.is_empty(),
        "another session's sub-agent never paints here"
    );
}

// -- FR-048: completion records the report status and removes the row -------

#[tokio::test]
async fn test_complete_records_report_status_and_removes_the_row() {
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;
    complete(&mut app, "s1", "explore-a1b2c3d4", "child-1", "stop").await;

    assert!(
        app.active_tasks.is_empty(),
        "a completed sub-agent is removed from the painted list"
    );
    let log = log_text(&app);
    assert!(
        log.contains("Task completed (explore-)"),
        "the completion is logged: {log}"
    );
    assert!(
        !log.contains("TRUNCATED"),
        "a natural finish carries no truncation marker: {log}"
    );
}

#[tokio::test]
async fn test_complete_logs_a_truncation_marker_when_the_reply_was_cut() {
    // `ReportStatus::from_finish_reason_label` classifies every provider cut.
    for reason in ["length", "truncation", "content_filter", "truncated"] {
        let mut app = app_with_session("s1");
        start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;
        complete(&mut app, "s1", "explore-a1b2c3d4", "child-1", reason).await;

        assert_eq!(
            ReportStatus::from_finish_reason_label(reason),
            ReportStatus::Truncated,
            "{reason:?} must classify as a cut reply"
        );
        assert!(
            log_text(&app).contains("(TRUNCATED by provider; report is incomplete)"),
            "finish_reason {reason:?} must mark the completion as truncated: {:?}",
            log_text(&app)
        );
    }
}

#[tokio::test]
async fn test_complete_marks_a_salvaged_reply_as_continued() {
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;
    complete(&mut app, "s1", "explore-a1b2c3d4", "child-1", "continued").await;

    assert_eq!(
        ReportStatus::from_finish_reason_label("continued"),
        ReportStatus::Continued
    );
    let log = log_text(&app);
    assert!(
        log.contains("(truncated; continuation retry recovered the tail)"),
        "a salvaged reply reports the retry: {log}"
    );
    assert!(
        !log.contains("TRUNCATED by provider"),
        "a salvaged reply is not reported as unrecoverable: {log}"
    );
}

#[tokio::test]
async fn test_complete_for_an_untracked_task_is_log_only() {
    let mut app = app_with_session("s1");
    complete(&mut app, "s1", "ghost-99887766", "child-x", "stop").await;

    assert!(
        app.active_tasks.is_empty(),
        "an untracked completion cannot resurrect or append a row"
    );
    assert!(
        log_text(&app).contains("Task completed (ghost-99)"),
        "the completion is still logged: {:?}",
        log_text(&app)
    );
}

#[tokio::test]
async fn test_failed_completion_logs_the_error_icon() {
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;
    app.handle_event(Event::SubagentComplete {
        session_id: "s1".to_string(),
        task_id: "explore-a1b2c3d4".to_string(),
        child_session_id: "child-1".to_string(),
        summary: "Error: boom".to_string(),
        success: false,
        duration_ms: 1,
        finish_reason: "error".to_string(),
    })
    .await;

    assert!(app.active_tasks.is_empty());
    assert!(
        log_text(&app).contains("[err] Task completed"),
        "an unsuccessful run paints the failure icon: {:?}",
        log_text(&app)
    );
}

// -- FR-049: cancel removes, suspend / resume re-status ---------------------

#[tokio::test]
async fn test_cancel_removes_the_row() {
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;
    app.handle_event(Event::SubagentCancelled {
        session_id: "s1".to_string(),
        task_id: "explore-a1b2c3d4".to_string(),
    })
    .await;

    assert!(
        app.active_tasks.is_empty(),
        "a cancelled sub-agent is removed from the painted list"
    );
    assert!(
        log_text(&app).contains("Task cancelled (explore-)"),
        "the cancel is logged: {:?}",
        log_text(&app)
    );
    assert!(app.needs_redraw, "the removal must repaint (NFR-005)");
}

#[tokio::test]
async fn test_cancel_for_an_untracked_task_does_not_touch_other_rows() {
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;
    app.handle_event(Event::SubagentCancelled {
        session_id: "s1".to_string(),
        task_id: "ghost-99887766".to_string(),
    })
    .await;

    assert_eq!(
        ids(&app),
        vec!["explore-a1b2c3d4".to_string()],
        "an unrelated cancel leaves the live row alone"
    );
}

#[tokio::test]
async fn test_suspend_and_resume_restatus_the_row() {
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;

    app.handle_event(Event::SubagentSuspended {
        session_id: "s1".to_string(),
        task_id: "explore-a1b2c3d4".to_string(),
        child_session_id: "child-1".to_string(),
    })
    .await;
    assert_eq!(only_task(&app).status, TaskStatus::Suspended);

    app.handle_event(Event::SubagentResumed {
        session_id: "s1".to_string(),
        task_id: "explore-a1b2c3d4".to_string(),
        child_session_id: "child-1".to_string(),
    })
    .await;
    assert_eq!(only_task(&app).status, TaskStatus::Running);
}

#[tokio::test]
async fn test_suspend_and_resume_for_an_untracked_task_are_no_ops() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::SubagentSuspended {
        session_id: "s1".to_string(),
        task_id: "ghost-99887766".to_string(),
        child_session_id: "child-x".to_string(),
    })
    .await;
    app.handle_event(Event::SubagentResumed {
        session_id: "s1".to_string(),
        task_id: "ghost-99887766".to_string(),
        child_session_id: "child-x".to_string(),
    })
    .await;

    assert!(
        app.active_tasks.is_empty(),
        "a replayed lifecycle event cannot append a row"
    );
}

// -- FR-072: a replayed spawn updates the painted row in place --------------

#[tokio::test]
async fn test_duplicate_spawn_by_task_id_updates_in_place() {
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;

    assert_eq!(
        ids(&app),
        vec!["explore-a1b2c3d4".to_string()],
        "a replayed spawn never appends a second row"
    );
    assert_eq!(
        log_lines_containing(&app, "task started:"),
        1,
        "the replayed spawn must not log a second start: {:?}",
        log_text(&app)
    );
}

#[tokio::test]
async fn test_duplicate_spawn_by_agent_id_updates_in_place() {
    // Same agent instance (child session) reported under a new task id: the
    // painted row is refreshed rather than duplicated.
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;
    start(&mut app, "s1", "explore-deadbeef", "child-1", "explore").await;

    assert_eq!(
        ids(&app),
        vec!["explore-deadbeef".to_string()],
        "the row tracks the agent instance, so it is renamed not duplicated"
    );
    assert_eq!(app.active_tasks.len(), 1);
}

#[tokio::test]
async fn test_replayed_spawn_revives_a_suspended_row() {
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;
    app.handle_event(Event::SubagentSuspended {
        session_id: "s1".to_string(),
        task_id: "explore-a1b2c3d4".to_string(),
        child_session_id: "child-1".to_string(),
    })
    .await;
    assert_eq!(only_task(&app).status, TaskStatus::Suspended);

    // The spawn is replayed (a lag burst re-delivery): the row is live again.
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;
    assert_eq!(
        only_task(&app).status,
        TaskStatus::Running,
        "a replayed spawn clears a stale suspension"
    );
    assert_eq!(app.active_tasks.len(), 1);
}

#[tokio::test]
async fn test_replayed_spawn_does_not_leak_a_completion_timestamp() {
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;
    // Simulate a row that a stale terminal marker had touched.
    if let Some(task) = app.active_tasks.first_mut() {
        task.completed_at = Some(chrono::Utc::now());
        task.report_status = ReportStatus::Truncated;
    }

    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;

    let task = only_task(&app);
    assert!(
        task.completed_at.is_none(),
        "a live row must not carry a completion timestamp"
    );
    assert_eq!(
        task.report_status,
        ReportStatus::default(),
        "a live row resets the report status to the spawn default"
    );
}

#[tokio::test]
async fn test_replayed_spawn_refreshes_the_carried_fields() {
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;

    app.handle_event(Event::SubagentStart {
        session_id: "s1".to_string(),
        task_id: "explore-a1b2c3d4".to_string(),
        child_session_id: "child-1".to_string(),
        agent: "rust-coder".to_string(),
        task: "reworked prompt".to_string(),
        background: false,
    })
    .await;

    let task = only_task(&app);
    assert_eq!(task.agent_name, "rust-coder");
    assert_eq!(task.task_prompt, "reworked prompt");
    assert!(!task.background);
    assert_eq!(task.status, TaskStatus::Running);
}

#[tokio::test]
async fn test_distinct_spawns_append_distinct_rows() {
    let mut app = app_with_session("s1");
    start(&mut app, "s1", "explore-a1b2c3d4", "child-1", "explore").await;
    start(&mut app, "s1", "explore-11223344", "child-2", "explore").await;

    assert_eq!(
        ids(&app),
        vec![
            "explore-a1b2c3d4".to_string(),
            "explore-11223344".to_string()
        ],
        "two genuine spawns paint two rows"
    );
}

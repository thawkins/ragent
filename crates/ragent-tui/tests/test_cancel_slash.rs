//! `/cancel` delegation contract (spec `agentssetup` FR-023, FR-024, FR-041).
//!
//! `/cancel <prefix>` used to resolve the prefix against the TUI's local
//! `active_tasks` mirror, remove the matching row, and report "Cancelled
//! task ..." without ever telling the task registry. The row then reappeared
//! on the next reconciliation sweep because the registry still held it as
//! `running` - a UI-local lie.
//!
//! These tests pin the replacement contract:
//!
//! - FR-023: the request goes through `AgentManager::cancel_agent` and the row
//!   is *not* removed before the registry confirms the transition (the mirror
//!   is authoritative only for display, never for cancellation);
//! - FR-024: a prefix matching more than one live task is refused with every
//!   matching id listed;
//! - FR-041: a prefix matching no live task names the live ids instead of only
//!   echoing the prefix; a finished task reports its own state rather than
//!   claiming a cancellation that will never happen.
//!
//! Tests only - no production logic is touched, and every write goes to the
//! harness tempdir (or the isolated `XDG_CONFIG_HOME` inside it).
#![allow(unsafe_code)]
#![cfg(test)]
// The isolation harness holds a process-global mutex across the async test body
// so cwd/env mutations cannot interleave.
#![allow(clippy::await_holding_lock)]

use chrono::Utc;
use ragent_agent::task::{AgentManager, ReportStatus, TaskEntry, TaskStatus};
use ragent_tui::App;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{EnvGuard, all_messages, app_with_session, enter_project, last_message};

type Harness = (
    std::sync::MutexGuard<'static, ()>,
    EnvGuard,
    tempfile::TempDir,
    App,
);

/// An app in an isolated tempdir project with an empty scratch config.
fn isolated_app() -> Harness {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    let app = app_with_session();
    (guard, env, temp, app)
}

/// A sub-agent task entry with an explicit id, agent and status.
fn task_entry(id: &str, agent: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: "s1".to_string(),
        child_session_id: format!("child-{id}"),
        agent_name: agent.to_string(),
        task_prompt: "do the thing".to_string(),
        background: true,
        detached: false,
        status,
        result: None,
        error: None,
        created_at: Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
        time_bound_secs: None,
    }
}

/// A live task registry wired into `app` so `/cancel` has a real
/// `AgentManager` to delegate to.
fn app_with_registry() -> (Harness, std::sync::Arc<AgentManager>) {
    let harness = isolated_app();
    let manager = std::sync::Arc::new(AgentManager::new(
        harness.3.session_processor.event_bus.clone(),
        harness.3.session_processor.clone(),
        4,
        300,
    ));
    // OnceLock::set fails on a second initialisation; the harness builds a
    // fresh processor per app, so this is the first set.
    let _ = harness
        .3
        .session_processor
        .agent_manager
        .set(manager.clone());
    (harness, manager)
}

// ---------------------------------------------------------------------------
// FR-024 - ambiguous prefixes are refused and listed
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_refuses_an_ambiguous_prefix_and_lists_the_matches() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.active_tasks.push(task_entry(
        "explore-aaaa1111",
        "explore",
        TaskStatus::Running,
    ));
    app.active_tasks.push(task_entry(
        "explore-bbbb2222",
        "explore",
        TaskStatus::Running,
    ));

    app.execute_slash_command("/cancel explore-").await;

    assert_eq!(app.status, "Ambiguous task prefix 'explore-'");
    let text = all_messages(&app);
    assert!(
        text.contains("Ambiguous task prefix `explore-`"),
        "text: {text}"
    );
    // Both candidates are named with their short ids so the user can pick one.
    assert!(text.contains("explore-"), "text: {text}");
    assert!(text.contains("2 tasks match"), "text: {text}");
    // An ambiguous prefix must not cancel anything: both rows survive.
    assert_eq!(app.active_tasks.len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_reports_every_matching_id_in_the_ambiguous_refusal() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.active_tasks.push(task_entry(
        "agent-0001-aaaa",
        "explore",
        TaskStatus::Running,
    ));
    app.active_tasks
        .push(task_entry("agent-0002-bbbb", "plan", TaskStatus::Running));
    app.active_tasks
        .push(task_entry("agent-0003-cccc", "build", TaskStatus::Running));

    app.execute_slash_command("/cancel agent-").await;

    let text = all_messages(&app);
    // Candidates are abreviated to their 8-char short id (FR-008), so each is
    // distinguishable, and each carries its agent name and current status.
    for expected in [
        "agent-00 (explore, running)",
        "agent-00 (plan, running)",
        "agent-00 (build, running)",
    ] {
        assert!(text.contains(expected), "missing {expected} in: {text}");
    }
    assert_eq!(app.active_tasks.len(), 3, "no row may be removed");
}

// ---------------------------------------------------------------------------
// FR-041 - an unmatched prefix names the live ids
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_names_the_live_ids_when_no_task_matches() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.active_tasks
        .push(task_entry("explore-live01", "explore", TaskStatus::Running));
    app.active_tasks
        .push(task_entry("plan-live02", "plan", TaskStatus::Suspended));

    app.execute_slash_command("/cancel zzzz").await;

    assert_eq!(app.status, "No task matching 'zzzz'");
    let text = all_messages(&app);
    assert!(text.contains("No live task matches `zzzz`"), "text: {text}");
    // FR-041: the live ids, not only the echoed prefix. The listing uses the
    // 8-char short id (FR-008), so two 13-char ids that share their first
    // eight characters are still told apart by their agent names.
    assert!(text.contains("explore- (explore)"), "text: {text}");
    assert!(text.contains("plan-liv (plan)"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_says_there_are_no_live_tasks_when_the_mirror_is_empty() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/cancel anything").await;

    assert_eq!(app.status, "No task matching 'anything'");
    assert!(
        last_message(&app).contains("Live tasks: no live sub-agent tasks"),
        "text: {}",
        last_message(&app)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_does_not_list_finished_tasks_as_live() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.active_tasks
        .push(task_entry("done-0001", "explore", TaskStatus::Completed));
    app.active_tasks
        .push(task_entry("dead-0002", "plan", TaskStatus::Cancelled));
    app.active_tasks
        .push(task_entry("tmo-0003", "build", TaskStatus::TimedOut));

    app.execute_slash_command("/cancel zzzz").await;

    let text = last_message(&app);
    assert!(text.contains("no live sub-agent tasks"), "text: {text}");
    assert!(!text.contains("done-0001"), "text: {text}");
    assert!(!text.contains("tmo-0003"), "text: {text}");
}

// ---------------------------------------------------------------------------
// FR-023 - delegation to the registry, no optimistic row removal
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_requests_through_the_registry_and_keeps_the_row() {
    let (harness, manager) = app_with_registry();
    let mut app = harness.3;

    // A task the registry actually knows about, with its cancel flag.
    let mut entry = task_entry("explore-req01", "explore", TaskStatus::Running);
    entry.parent_session_id = app.session_id.clone().unwrap_or_default();
    let task_id = entry.id.clone();
    let flag = manager.insert_task_for_test(entry);
    app.active_tasks
        .push(task_entry(&task_id, "explore", TaskStatus::Running));

    app.execute_slash_command("/cancel explore-req").await;

    // The request, not the outcome, is reported.
    assert_eq!(app.status, "Cancellation requested for explore- (explore)");
    let text = all_messages(&app);
    assert!(
        text.contains("Cancellation requested for `explore-`"),
        "text: {text}"
    );
    // FR-023: the row is not removed before the registry confirms it.
    assert_eq!(
        app.active_tasks.len(),
        1,
        "the row must survive until the registry confirms the transition"
    );

    // FR-023: `AgentManager::cancel_agent` was actually asked - it sets the
    // task's cooperative cancel flag.
    let mut cancelled = false;
    for _ in 0..100 {
        if flag.load(std::sync::atomic::Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        cancelled,
        "/cancel must reach AgentManager::cancel_agent (the cancel flag stays unset)"
    );
    // And the entry is still `running`: the loop has not observed the flag.
    assert_eq!(
        manager
            .get_task(&task_id)
            .await
            .map(|e| e.status)
            .expect("registry entry"),
        TaskStatus::Running
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_reports_the_registry_status_of_a_finished_task() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.active_tasks
        .push(task_entry("done-0001", "explore", TaskStatus::Completed));

    app.execute_slash_command("/cancel done-").await;

    assert_eq!(app.status, "Task done-000 is already completed");
    let text = all_messages(&app);
    assert!(text.contains("is already `completed`"), "text: {text}");
    assert!(
        !text.contains("Cancellation requested"),
        "a finished task must not claim a cancellation: {text}"
    );
    // The row is untouched; the reconcile pass owns its retirement.
    assert_eq!(app.active_tasks.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_still_targets_the_active_benchmark_before_tasks() {
    let (_g, _e, _t, mut app) = isolated_app();

    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    app.active_bench_task_id = Some("bench-42".to_string());
    app.active_bench_cancel = Some(std::sync::Arc::clone(&flag));

    app.execute_slash_command("/cancel bench").await;

    assert_eq!(app.status, "[wait] bench: cancellation requested");
    assert!(flag.load(std::sync::atomic::Ordering::Relaxed));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_help_and_the_empty_prefix_guard_are_unchanged() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/cancel help").await;
    assert_eq!(app.status, "cancel: help");
    // The help body is rendered as a table, so assert on the headings rather
    // than on a phrase the column wrapping would split.
    let text = last_message(&app);
    assert!(
        text.contains("Request cancellation of a task"),
        "text: {text}"
    );
    assert!(text.contains("Subcommand"), "text: {text}");

    app.execute_slash_command("/cancel").await;
    assert_eq!(
        app.status,
        "[warn] Please provide a task ID prefix: /cancel <id>"
    );
}

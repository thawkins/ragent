//! Coverage for the two named panel conditions of spec `agentssetup` T-023.
//!
//! - **FR-042** - a chord or button press that produces no window is never
//!   left unexplained. The two panels now always open (an empty one paints its
//!   own empty-state hint, `agentui` FR-026), so the only silent press left is a
//!   close of an already-closed window, which is a no-op by design; the named
//!   refusal texts are still asserted so no entry point regresses to a silent
//!   "nothing happened".
//! - **FR-032** - a finished row whose report has no durable copy
//!   (`output_file: None`, the state a failed `persist_task_output` leaves
//!   behind) states that the report is in-memory only. The history row and the
//!   panel's rendered history section both carry it, and the wording is the
//!   shared `IN_MEMORY_REPORT_NOTE` that `list_agents` / `wait_agents` print,
//!   so the panel cannot describe the condition differently from the tools.
//!
//! The blocking filesystem of TESTPLAN TC-019 is replaced by the registry
//! state a failed persist leaves behind, for the same reason T-015 did: the
//! requirement under test is the *reporting*, not the write.

#[path = "support/mod.rs"]
mod support;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::tool::wait_agents::IN_MEMORY_REPORT_NOTE;
use ragent_tui::App;
use ragent_tui::app::{
    AGENTS_WINDOW_EMPTY_REFUSAL, CompletedTaskRow, note_subagent_cancelled_for_tests,
    note_subagent_completed_for_tests,
};
use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-refusals";

/// A live sub-agent task.
fn task(id: &str) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: LEAD_SESSION.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: "explore".to_string(),
        task_prompt: "prompt".to_string(),
        background: true,
        detached: false,
        status: TaskStatus::Running,
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

/// A finished history row, optionally carrying a durable report path.
fn history_row(id: &str, output_file: Option<&str>) -> CompletedTaskRow {
    let created_at = chrono::Utc::now();
    CompletedTaskRow {
        id: id.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: "explore".to_string(),
        status: TaskStatus::Completed,
        background: true,
        detached: false,
        created_at,
        completed_at: created_at + chrono::Duration::seconds(7),
        report_status: ReportStatus::default(),
        output_file: output_file.map(std::path::PathBuf::from),
    }
}

/// An app with an explicit status baseline and a clean redraw flag.
fn app() -> App {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.status = "ready".to_string();
    app.needs_redraw = false;
    app
}

/// Flatten the rendered frame into one string.
fn screen_text(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

// ---------------------------------------------------------------------------
// FR-042 / FR-026: the panels always open and name their own empty state
// ---------------------------------------------------------------------------

#[test]
fn opening_an_empty_agents_window_opens_and_paints_its_empty_state() {
    let mut app = app();
    app.toggle_agents_window();

    assert!(
        app.show_agents_window,
        "FR-026: with nothing to show the window still opens on its empty-state hint"
    );
    assert!(app.needs_redraw, "an open repaints (NFR-005)");
    assert_eq!(
        app.status, "ready",
        "an empty open is not an error, so it reports nothing"
    );
}

#[test]
fn the_refusal_names_the_condition_and_a_next_action() {
    // NFR-005: every new message names the condition, the offending value and
    // at least one supported next action.
    assert!(
        AGENTS_WINDOW_EMPTY_REFUSAL.contains("nothing to show"),
        "the refusal names the condition: {AGENTS_WINDOW_EMPTY_REFUSAL}"
    );
    assert!(
        AGENTS_WINDOW_EMPTY_REFUSAL.contains("sub-agent")
            && AGENTS_WINDOW_EMPTY_REFUSAL.contains("shell task"),
        "the refusal names both kinds of subject the panel would show: \
         {AGENTS_WINDOW_EMPTY_REFUSAL}"
    );
    assert!(
        AGENTS_WINDOW_EMPTY_REFUSAL.contains("/spawn"),
        "the refusal names a supported next action: {AGENTS_WINDOW_EMPTY_REFUSAL}"
    );
}

#[test]
fn a_repeated_open_still_opens() {
    // Two presses alternate open/close rather than the second being swallowed:
    // "I pressed it and nothing happened" is exactly the confusion the named
    // refusal contract removes.
    let mut app = app();
    app.toggle_agents_window();
    app.toggle_agents_window();
    assert!(!app.show_agents_window, "the second press closes it");

    app.toggle_agents_window();
    assert!(app.show_agents_window, "the third press opens it again");
}

#[test]
fn a_toggle_while_the_window_is_open_says_nothing() {
    // The window is already open and its subject has since vanished (a state
    // only a direct mutation can produce: every removal path closes the window
    // with its last subject). The press is a close request, so the status must
    // not be overwritten with an empty-panel message.
    let mut app = app();
    app.show_agents_window = true;

    app.toggle_agents_window();

    assert!(!app.show_agents_window, "the press closes the window");
    assert_eq!(
        app.status, "ready",
        "a close request must not print an empty-panel message"
    );
}

#[test]
fn an_available_window_opens_without_a_refusal() {
    let mut app = app();
    app.active_tasks.push(task("task-0001"));

    app.toggle_agents_window();

    assert!(app.show_agents_window, "FR-022: the window opens");
    assert_eq!(
        app.status, "ready",
        "a successful open must not print the refusal"
    );
    assert!(app.needs_redraw, "an open repaints");
}

#[test]
fn a_completed_history_row_keeps_the_window_openable() {
    // A finished run is snapshotted into history and its live row removed, so
    // the panel would otherwise refuse to reopen and the FR-032 note in that
    // row would become unreadable exactly when it matters.
    let mut app = app();
    app.agents_history.push_back(history_row("task-0002", None));

    app.toggle_agents_window();

    assert!(
        app.show_agents_window,
        "FR-036: a history row is a subject, so the panel opens"
    );
    assert_ne!(
        app.status, AGENTS_WINDOW_EMPTY_REFUSAL,
        "an open with a history row to paint is not a refusal"
    );
}

// ---------------------------------------------------------------------------
// FR-032: an absent durable copy is named, not omitted
// ---------------------------------------------------------------------------

#[test]
fn a_history_row_without_a_durable_copy_names_the_condition() {
    let row = history_row("task-0003", None);

    assert_eq!(
        row.durable_copy_absence(),
        Some(IN_MEMORY_REPORT_NOTE),
        "FR-032: a row with no output_file must state the report is in-memory \
         only, using the wording the tools share"
    );
    assert!(
        IN_MEMORY_REPORT_NOTE.contains("in-memory only")
            && IN_MEMORY_REPORT_NOTE.contains("durable copy"),
        "the note names both conditions: {IN_MEMORY_REPORT_NOTE}"
    );
}

#[test]
fn a_history_row_with_a_durable_copy_says_nothing() {
    let row = history_row("task-0004", Some("log/subagents/task-0004.md"));

    assert_eq!(
        row.durable_copy_absence(),
        None,
        "FR-032: a persisted report must not also claim to be in memory only"
    );
}

#[test]
fn the_panel_paints_the_absence_note_and_no_false_path() {
    let mut app = app();
    app.agents_history.push_back(history_row("task-0005", None));
    app.show_agents_window = true;

    let text = screen_text(&mut app, 320, 44);

    assert!(
        text.contains("Completed"),
        "the history section paints under its heading: {text}"
    );
    assert!(
        text.contains("[memory]"),
        "FR-032: the history row flags the missing durable copy: {text}"
    );
    assert!(
        text.contains("completed") && text.contains("in-memory"),
        "FR-032: the history row spells the condition out where the panel has \
         room to paint it: {text}"
    );
    assert!(
        !text.contains("task-0005.md"),
        "FR-043: no path may be claimed for a report that was never written: \
         {text}"
    );
}

#[test]
fn the_panel_paints_no_note_for_a_persisted_report() {
    let mut app = app();
    app.agents_history
        .push_back(history_row("task-0006", Some("log/subagents/task-0006.md")));
    app.show_agents_window = true;

    let text = screen_text(&mut app, 320, 44);

    assert!(
        text.contains("Completed"),
        "the history section still paints: {text}"
    );
    assert!(
        !text.contains("[memory]") && !text.contains("in-memory"),
        "FR-032: a persisted report must not carry the absence note, or the \
         note becomes noise on every healthy task: {text}"
    );
}

// ---------------------------------------------------------------------------
// FR-028 / FR-036: the snapshot is taken before the live row goes
// ---------------------------------------------------------------------------

#[test]
fn a_completed_task_is_snapshotted_before_its_live_row_is_removed() {
    let mut app = app();
    app.agents_history_cap = 4;
    let mut finished = task("task-0007");
    finished.output_file = Some(std::path::PathBuf::from("log/subagents/task-0007.md"));
    app.active_tasks.push(finished);

    note_subagent_completed_for_tests(&mut app, "task-0007", "stop");

    assert!(
        app.active_tasks.is_empty(),
        "the live row is removed once it is finished"
    );
    assert_eq!(
        app.agents_history.len(),
        1,
        "FR-028: the finished run is never lost from both sections"
    );
    let row = app.agents_history.front().expect("history row");
    assert_eq!(row.id, "task-0007");
    assert_eq!(row.status, TaskStatus::Completed);
    assert_eq!(
        row.output_file.as_deref(),
        Some(std::path::Path::new("log/subagents/task-0007.md")),
        "the snapshot carries the durable path the row had when it left"
    );
    assert!(
        row.durable_copy_absence().is_none(),
        "a run that persisted its report carries no absence note"
    );
}

#[test]
fn a_cancelled_task_is_snapshotted_and_named_as_in_memory() {
    let mut app = app();
    app.active_tasks.push(task("task-0008"));

    note_subagent_cancelled_for_tests(&mut app, "task-0008");

    let row = app
        .agents_history
        .front()
        .expect("the cancelled run is snapshotted");
    assert_eq!(row.id, "task-0008");
    assert_eq!(row.status, TaskStatus::Running);
    assert!(
        row.durable_copy_absence().is_some(),
        "a cancelled run with no persisted report must say so (FR-032)"
    );
}

#[test]
fn history_keeps_newest_first_and_evicts_beyond_the_cap() {
    let mut app = app();
    app.agents_history_cap = 2;
    for id in ["task-0009", "task-0010", "task-0011"] {
        app.active_tasks.push(task(id));
        note_subagent_completed_for_tests(&mut app, id, "stop");
    }

    let ids: Vec<&str> = app.agents_history.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["task-0011", "task-0010"],
        "FR-036/NFR-004: newest first, oldest evicted past the cap"
    );
}

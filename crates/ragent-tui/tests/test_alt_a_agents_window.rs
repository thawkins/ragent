//! Tests for the `Alt+A` Agents-window open chord (spec `agentssetup` T-019).
//!
//! Covers FR-022: the Agents panel is openable from the keyboard alone, and the
//! chord must reach the panel through an `InputAction` rather than an inline
//! mutation. Every case here drives the real input router
//! ([`App::handle_key_event`]) and asserts the outcome the Agents button's
//! mouse click produces in `test_window_routing.rs` - the same
//! `App::toggle_agents_window` visibility path, including the no-op when the
//! window has no subject, the Teams-window exclusion, and the redraw flag.
//!
//! Also covers NFR-002 (the `a` is never typed into the input buffer) and
//! NFR-009 (bare `a` still behaves exactly as before).

mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::TeamConfig;
use ragent_tui::App;
use ragent_tui::input::{InputAction, handle_key};
use support::make_app;

/// The lead session id used by every case.
const LEAD_SESSION: &str = "lead-session-0001";

/// An `Alt+A` key event.
fn alt_a() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT)
}

/// A bare `a` key event, for the NFR-009 check.
fn plain_a() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)
}

/// A live sub-agent task so the Agents window has a subject.
fn running_task(id: &str) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: LEAD_SESSION.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
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

/// An app with an explicit redraw baseline of "clean".
fn app() -> App {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.needs_redraw = false;
    app
}

/// An app with one running sub-agent task, so the Agents window is available.
fn app_with_task() -> App {
    let mut app = app();
    app.active_tasks.push(running_task("task-1"));
    app
}

/// Render one frame so the renderer records its hit rects, as a real session
/// does before any click can arrive.
fn render(app: &mut App) {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
}

// ---------------------------------------------------------------------------
// FR-022: the chord resolves to an InputAction, not an inline mutation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn alt_a_resolves_to_the_agents_window_action() {
    let mut app = app_with_task();
    let action = handle_key(&mut app, alt_a()).await;
    assert!(
        matches!(action, Some(InputAction::ToggleAgentsWindow)),
        "FR-022: Alt+A must resolve to InputAction::ToggleAgentsWindow so it \
         shares the mouse's toggle; got {action:?}"
    );
    assert!(
        !app.show_agents_window,
        "resolving the action must not itself mutate the window"
    );
}

#[tokio::test]
async fn alt_a_is_bound_regardless_of_window_availability() {
    // The binding is unconditional; availability is the toggle's own guard.
    // An unconditional binding keeps the chord from silently meaning
    // "type an a" when no task exists yet.
    let mut app = app();
    let action = handle_key(&mut app, alt_a()).await;
    assert!(matches!(action, Some(InputAction::ToggleAgentsWindow)));
}

// ---------------------------------------------------------------------------
// FR-022: the keyboard produces the same outcome as the mouse click
// ---------------------------------------------------------------------------

#[tokio::test]
async fn alt_a_toggles_the_window_and_requests_a_redraw() {
    let mut app = app_with_task();

    app.handle_key_event(alt_a()).await;
    assert!(
        app.show_agents_window,
        "FR-022: the first Alt+A opens the window"
    );
    assert!(
        app.needs_redraw,
        "opening the window requests a redraw, as the click does"
    );

    app.needs_redraw = false;
    app.handle_key_event(alt_a()).await;
    assert!(
        !app.show_agents_window,
        "FR-022: the second Alt+A closes it"
    );
    assert!(
        app.needs_redraw,
        "closing the window requests a redraw, as the click does"
    );
}

#[tokio::test]
async fn alt_a_without_a_subject_still_opens_the_window() {
    // The panel is never hidden behind a chord that looks inert: an empty open
    // paints the empty-state hint (FR-026) and reports nothing.
    let mut app = app();
    app.status = "ready".to_string();

    app.handle_key_event(alt_a()).await;
    assert!(
        app.show_agents_window,
        "with nothing to show the window still opens, as the click does"
    );
    assert!(app.needs_redraw, "opening the window requests a redraw");
    assert_eq!(
        app.status, "ready",
        "an empty open is not an error, so it reports nothing"
    );

    app.needs_redraw = false;
    app.handle_key_event(alt_a()).await;
    assert!(!app.show_agents_window, "the second chord closes it again");
}

#[tokio::test]
async fn alt_a_closes_an_open_window_even_when_its_subject_vanished() {
    // The window can empty out from under the operator (FR-063); the chord
    // that opened it must still be able to dismiss it.
    let mut app = app();
    app.show_agents_window = true;
    app.needs_redraw = false;

    app.handle_key_event(alt_a()).await;
    assert!(
        !app.show_agents_window,
        "an open window always closes, whatever its subject is now"
    );
    assert!(
        app.needs_redraw,
        "closing the window requests a redraw, as the click does"
    );
}

#[tokio::test]
async fn alt_a_opening_the_agents_window_closes_the_teams_window() {
    let mut app = app_with_task();
    app.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    render(&mut app);
    app.show_teams_window = true;

    app.handle_key_event(alt_a()).await;
    assert!(app.show_agents_window, "the Agents window opens");
    assert!(
        !app.show_teams_window,
        "the Teams window is closed, exactly as on a mouse click"
    );
}

// ---------------------------------------------------------------------------
// NFR-002 / NFR-009: the chord consumes the key and shadows nothing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn alt_a_does_not_insert_a_into_the_input() {
    let mut app = app_with_task();
    app.input = "PROBE".to_string();
    app.input_cursor = app.input_len_chars();

    app.handle_key_event(alt_a()).await;

    assert_eq!(
        app.input, "PROBE",
        "NFR-002: Alt+A must not leak the 'a' into the input buffer"
    );
    assert_eq!(
        app.input_cursor,
        app.input_len_chars(),
        "the cursor must not move"
    );
}

#[tokio::test]
async fn alt_a_leaves_a_running_turn_untouched() {
    let mut app = app_with_task();
    app.is_processing = true;

    app.handle_key_event(alt_a()).await;

    assert!(
        app.show_agents_window,
        "FR-022: the panel is reachable while a turn executes"
    );
    assert!(
        !app.pending_stop_confirm,
        "Alt+A must not arm the stop-confirmation dialog"
    );
    assert!(app.is_processing, "the running turn is untouched");
}

#[tokio::test]
async fn plain_a_still_types_a_character() {
    let mut app = app_with_task();

    app.handle_key_event(plain_a()).await;

    assert_eq!(
        app.input, "a",
        "NFR-009: without ALT the 'a' must still type a normal character"
    );
    assert!(
        !app.show_agents_window,
        "a bare 'a' must not open the window"
    );
}

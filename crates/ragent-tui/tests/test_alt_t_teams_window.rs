//! Tests for the `Alt+T` Teams-window toggle chord (spec `agentssetup` FR-045).
//!
//! The twin of `test_alt_a_agents_window.rs`: the chord resolves to an
//! `InputAction` and reaches the panel through the same
//! `App::toggle_teams_window` the Teams button's mouse click calls, so a
//! keyboard-only terminal user can open and dismiss the panel with or without
//! team members being present.
//!
//! Also covers NFR-002 (the `t` is never typed into the input buffer) and
//! NFR-009 (bare `t` still behaves exactly as before).

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

/// An `Alt+T` key event.
fn alt_t() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT)
}

/// A bare `t` key event, for the NFR-002 / NFR-009 checks.
fn plain_t() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE)
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

/// Build an `App` with a live team, so the Teams window has a subject.
fn app_with_team() -> App {
    let mut app = app();
    app.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    app
}

/// Render one frame so the button hit rectangles are populated.
fn render(app: &mut App) {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw");
}

// ---------------------------------------------------------------------------
// FR-045: the chord resolves to an action, not an inline mutation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn alt_t_resolves_to_the_teams_toggle_action() {
    let mut app = app();

    let action = handle_key(&mut app, alt_t()).await;
    assert!(
        matches!(action, Some(InputAction::ToggleTeamsWindow)),
        "FR-045: Alt+T must resolve to InputAction::ToggleTeamsWindow so it \
         reaches the same toggle the Teams button calls"
    );
    assert!(
        !app.show_teams_window,
        "resolving the action must not itself mutate the window"
    );
}

#[tokio::test]
async fn alt_t_is_bound_regardless_of_window_availability() {
    // The binding is unconditional; availability is the toggle's own guard.
    // An unconditional binding keeps the chord from silently meaning
    // "type a t" when no team exists yet.
    let mut app = app();
    let action = handle_key(&mut app, alt_t()).await;
    assert!(matches!(action, Some(InputAction::ToggleTeamsWindow)));
}

// ---------------------------------------------------------------------------
// FR-045: the keyboard produces the same outcome as the mouse click
// ---------------------------------------------------------------------------

#[tokio::test]
async fn alt_t_toggles_the_window_and_requests_a_redraw() {
    let mut app = app_with_team();

    app.handle_key_event(alt_t()).await;
    assert!(
        app.show_teams_window,
        "FR-045: the first Alt+T opens the window"
    );
    assert!(
        app.needs_redraw,
        "opening the window requests a redraw, as the click does"
    );

    app.needs_redraw = false;
    app.handle_key_event(alt_t()).await;
    assert!(!app.show_teams_window, "FR-045: the second Alt+T closes it");
    assert!(
        app.needs_redraw,
        "closing the window requests a redraw, as the click does"
    );
}

#[tokio::test]
async fn alt_t_without_a_team_still_opens_the_window() {
    // With no team the panel paints its lead row and the empty-state hint
    // (FR-026) rather than refusing to open.
    let mut app = app();
    app.status = "ready".to_string();

    app.handle_key_event(alt_t()).await;
    assert!(
        app.show_teams_window,
        "with no team the window still opens on its lead row and empty-state hint"
    );
    assert!(app.needs_redraw, "opening the window requests a redraw");
    assert_eq!(
        app.status, "ready",
        "an empty open is not an error, so it reports nothing"
    );

    app.needs_redraw = false;
    app.handle_key_event(alt_t()).await;
    assert!(!app.show_teams_window, "the second chord closes it again");
}

#[tokio::test]
async fn the_teams_refusal_names_the_condition_and_a_next_action() {
    // NFR-005: every new message names the condition and at least one
    // supported next action.
    let text = ragent_tui::app::TEAMS_WINDOW_EMPTY_REFUSAL;
    assert!(
        text.contains("nothing to show"),
        "the refusal names the condition: {text}"
    );
    assert!(
        text.contains("team"),
        "the refusal names the subject the panel would show: {text}"
    );
    assert!(
        text.contains("/team") || text.contains("/swarm"),
        "the refusal names a supported next action: {text}"
    );
}

#[tokio::test]
async fn alt_t_closes_an_open_window_even_when_its_team_vanished() {
    // The team can end while the window is open; the chord that opened it must
    // still be able to dismiss it.
    let mut app = app();
    app.show_teams_window = true;
    app.needs_redraw = false;

    app.handle_key_event(alt_t()).await;
    assert!(
        !app.show_teams_window,
        "an open window always closes, whatever its subject is now"
    );
    assert!(
        app.needs_redraw,
        "closing the window requests a redraw, as the click does"
    );
}

#[tokio::test]
async fn alt_t_opening_the_teams_window_closes_the_agents_window() {
    let mut app = app_with_team();
    app.active_tasks.push(running_task("task-1"));
    render(&mut app);
    app.show_agents_window = true;

    app.handle_key_event(alt_t()).await;
    assert!(app.show_teams_window, "the Teams window opens");
    assert!(
        !app.show_agents_window,
        "the Agents window is closed, exactly as on a mouse click"
    );
}

#[tokio::test]
async fn alt_a_and_alt_t_alternate_without_ever_showing_both() {
    let mut app = app_with_team();
    app.active_tasks.push(running_task("task-1"));
    render(&mut app);

    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT))
        .await;
    assert!(app.show_agents_window && !app.show_teams_window);

    app.handle_key_event(alt_t()).await;
    assert!(
        app.show_teams_window && !app.show_agents_window,
        "the two windows are mutually exclusive whichever chord opens them"
    );
}

// ---------------------------------------------------------------------------
// NFR-002 / NFR-009: the chord consumes the key and shadows nothing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn alt_t_does_not_insert_t_into_the_input() {
    let mut app = app_with_team();
    app.input = "PROBE".to_string();
    app.input_cursor = app.input_len_chars();

    app.handle_key_event(alt_t()).await;

    assert_eq!(
        app.input, "PROBE",
        "NFR-002: Alt+T must not leak the 't' into the input buffer"
    );
    assert_eq!(
        app.input_cursor,
        app.input_len_chars(),
        "the cursor must not move"
    );
}

#[tokio::test]
async fn bare_t_still_types_t() {
    let mut app = app();

    let action = handle_key(&mut app, plain_t()).await;
    assert!(
        action.is_none(),
        "NFR-009: bare 't' is plain text, not the Teams chord"
    );
    assert_eq!(app.input, "t", "NFR-009: bare 't' is still typed");
    assert!(
        !app.show_teams_window,
        "bare 't' must not open the Teams window"
    );
}

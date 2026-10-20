//! Tests for popup-table wheel scrolling (spec `agentui` T-017).
//!
//! Covers the FR-008 / FR-059 wheel path: a wheel event over the Agents or
//! Teams popup body moves the table's scroll offset by `SCROLL_STEP_LINES`
//! (`3`), the offset re-clamps to the maximum the renderer computed (a wheel
//! up past the bottom cannot overshoot, a wheel down below the top cannot go
//! negative), the wheel is consumed so the pane beneath the popup never
//! scrolls, every accepted wheel sets the redraw flag (NFR-005), and the
//! clamped, non-zero offset shifts the painted rows with the scrollbar
//! tracking the position (FR-056, acceptance 19).
//!
//! Every case drives the real mouse router ([`App::handle_mouse_event`])
//! against areas recorded by the real render entry point through a ratatui
//! `TestBackend`, so the assertions exercise the exact path a real wheel event
//! takes (NFR-001). All state is synthetic and in-memory; nothing is written
//! into the repository (NFR-012). Per-frame work is the visible-window slice
//! the renderers already perform, so a scrolled frame costs no more than a
//! static one (NFR-002).

mod support;

use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::{Terminal, layout::Rect};

use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_tui::App;

/// The wheel step the mouse router applies (`SCROLL_STEP_LINES`).
const STEP: u16 = 3;

/// A wheel-up event at `(col, row)`.
const fn wheel_up(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

/// A wheel-down event at `(col, row)`.
const fn wheel_down(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

/// A running sub-agent [`TaskEntry`] parented to the lead session.
fn task(id: &str, parent_session: &str, agent: &str) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: parent_session.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: agent.to_string(),
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
    }
}

/// Render one full frame of `app` and return the drawn buffer.
fn render_frame(app: &mut App, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
    terminal.backend().buffer().clone()
}

/// Render `app` with the Agents popup open and return the drawn buffer.
fn render_agents(app: &mut App, width: u16, height: u16) -> Buffer {
    app.show_agents_window = true;
    render_frame(app, width, height)
}

/// Render `app` with the Teams popup open and return the drawn buffer.
fn render_teams(app: &mut App, width: u16, height: u16) -> Buffer {
    app.show_teams_window = true;
    render_frame(app, width, height)
}

/// The first screen row of `buffer` containing `needle`, if any.
fn row_containing(buffer: &Buffer, terminal: u16, needle: &str) -> Option<u16> {
    (0..terminal).find(|&y| {
        let row: String = (0..buffer.area.width)
            .filter_map(|x| buffer.cell((x, y)).map(|c| c.symbol().to_string()))
            .collect();
        row.contains(needle)
    })
}

/// The centre cell of `area`, the popup body the wheel targets.
fn centre(area: Rect) -> (u16, u16) {
    (area.x + area.width / 2, area.y + area.height / 2)
}

/// All of the symbols the default ratatui vertical scrollbar paints.
fn scrollbar_glyph(c: char) -> bool {
    ["\u{2588}", "\u{2551}", "\u{25B2}", "\u{25BC}"].contains(&c.to_string().as_str())
}

// ---------------------------------------------------------------------------
// FR-008 / FR-059: the wheel moves the Agents table offset
// ---------------------------------------------------------------------------

/// A wheel-up over the Agents popup body moves the offset by one step, a
/// wheel-down moves it back, and the wheel-down saturates at zero (FR-008,
/// FR-059, T-017).
#[test]
fn test_window_scroll_wheel_moves_the_agents_offset() {
    let mut app = support::make_app();
    app.active_agents_area = Rect::new(5, 5, 100, 20);
    app.active_agents_max_scroll = 20;
    let (col, row) = centre(app.active_agents_area);

    app.active_agents_scroll_offset = 0;
    app.handle_mouse_event(wheel_up(col, row));
    assert_eq!(app.active_agents_scroll_offset, STEP);

    app.handle_mouse_event(wheel_down(col, row));
    assert_eq!(app.active_agents_scroll_offset, 0);
}

/// Wheel up clamps to the maximum the renderer published: a wheel past the
/// bottom of a `max_scroll = 5` table stops at 5, and further wheels stay
/// there (FR-059 clamp, T-017).
#[test]
fn test_window_scroll_wheel_clamps_to_the_agents_maximum() {
    let mut app = support::make_app();
    app.active_agents_area = Rect::new(5, 5, 100, 20);
    app.active_agents_max_scroll = 5;
    let (col, row) = centre(app.active_agents_area);

    app.handle_mouse_event(wheel_up(col, row));
    app.handle_mouse_event(wheel_up(col, row));
    assert_eq!(
        app.active_agents_scroll_offset, 5,
        "the offset must clamp to the published maximum (FR-059)"
    );
    app.handle_mouse_event(wheel_up(col, row));
    assert_eq!(app.active_agents_scroll_offset, 5, "the clamp is stable");
}

/// A wheel over the popup body is consumed: the message pane beneath the
/// modal keeps its offset unchanged (T-017 wheel ownership).
#[test]
fn test_window_scroll_agents_wheel_does_not_scroll_the_pane_beneath() {
    let mut app = support::make_app();
    app.message_area = Rect::new(0, 1, 100, 20);
    app.active_agents_area = Rect::new(5, 5, 100, 20);
    app.active_agents_max_scroll = 20;
    let (col, row) = centre(app.active_agents_area);

    app.handle_mouse_event(wheel_up(col, row));
    assert_eq!(app.scroll_offset, 0, "the popup absorbs the wheel");
    assert_eq!(app.active_agents_scroll_offset, STEP);
}

// ---------------------------------------------------------------------------
// FR-008 / FR-059: the wheel moves the Teams table offset
// ---------------------------------------------------------------------------

/// The same wheel contract for the Teams popup body: step up, step back to
/// zero, saturating at the bottom (FR-008, FR-059, T-017).
#[test]
fn test_window_scroll_wheel_moves_the_teams_offset() {
    let mut app = support::make_app();
    app.teams_area = Rect::new(5, 5, 100, 20);
    app.teams_max_scroll = 20;
    let (col, row) = centre(app.teams_area);

    app.handle_mouse_event(wheel_up(col, row));
    assert_eq!(app.teams_scroll_offset, STEP);

    app.handle_mouse_event(wheel_down(col, row));
    assert_eq!(app.teams_scroll_offset, 0);

    app.handle_mouse_event(wheel_down(col, row));
    assert_eq!(app.teams_scroll_offset, 0, "wheel down saturates at zero");
}

/// Wheel up over the Teams table clamps to its published maximum (FR-059).
#[test]
fn test_window_scroll_wheel_clamps_to_the_teams_maximum() {
    let mut app = support::make_app();
    app.teams_area = Rect::new(5, 5, 100, 20);
    app.teams_max_scroll = 4;
    let (col, row) = centre(app.teams_area);

    app.handle_mouse_event(wheel_up(col, row));
    app.handle_mouse_event(wheel_up(col, row));
    assert_eq!(
        app.teams_scroll_offset, 4,
        "the offset must clamp to the published maximum (FR-059)"
    );
}

/// A wheel over the Teams popup body does not scroll the pane underneath.
#[test]
fn test_window_scroll_teams_wheel_does_not_scroll_the_pane_beneath() {
    let mut app = support::make_app();
    app.message_area = Rect::new(0, 1, 100, 20);
    app.teams_area = Rect::new(5, 5, 100, 20);
    app.teams_max_scroll = 20;
    let (col, row) = centre(app.teams_area);

    app.handle_mouse_event(wheel_up(col, row));
    assert_eq!(app.scroll_offset, 0, "the popup absorbs the wheel");
    assert_eq!(app.teams_scroll_offset, STEP);
}

// ---------------------------------------------------------------------------
// NFR-005: redraw signalling
// ---------------------------------------------------------------------------

/// Every accepted wheel sets the redraw flag so the next frame reflects the
/// new offset without waiting for an unrelated event (NFR-005).
#[test]
fn test_window_scroll_wheel_sets_the_redraw_flag() {
    let mut app = support::make_app();
    app.active_agents_area = Rect::new(5, 5, 100, 20);
    app.teams_area = Rect::new(5, 5, 100, 20);
    app.active_agents_max_scroll = 20;
    app.teams_max_scroll = 20;
    let (col, row) = centre(app.active_agents_area);

    app.needs_redraw = false;
    app.handle_mouse_event(wheel_up(col, row));
    assert!(app.needs_redraw, "an agents wheel must request a redraw");

    let (tcol, trow) = centre(app.teams_area);
    app.needs_redraw = false;
    app.handle_mouse_event(wheel_up(tcol, trow));
    assert!(app.needs_redraw, "a teams wheel must request a redraw");
}

// ---------------------------------------------------------------------------
// No popup: the wheel keeps the old behaviour
// ---------------------------------------------------------------------------

/// With both areas at the default empty rect (no popup rendered), a wheel
/// moves neither offset and falls through to the pane routing (T-017).
#[test]
fn test_window_scroll_wheel_without_a_popup_leaves_the_offsets_alone() {
    let mut app = support::make_app();
    app.active_agents_area = Rect::default();
    app.teams_area = Rect::default();

    app.handle_mouse_event(wheel_up(40, 10));
    assert_eq!(app.active_agents_scroll_offset, 0);
    assert_eq!(app.teams_scroll_offset, 0);
}

// ---------------------------------------------------------------------------
// End-to-end: the offset is applied, hit-rects follow, the scrollbar tracks
// ---------------------------------------------------------------------------

/// Driving the wheel against a real rendered frame scrolls the painted rows
/// by the step, clamps at the rendered maximum, and the scrollbar glyph in
/// the table's rightmost column tracks the position (FR-056, FR-059,
/// acceptance 19).
#[test]
fn test_window_scroll_wheel_scrolls_the_rendered_agents_table_end_to_end() {
    let mut app = support::make_app();
    app.session_id = Some("lead-session-0001".to_string());
    app.agent_name = "general".to_string();
    for i in 0..12 {
        let id = format!("task-{i:02}");
        app.active_tasks
            .push(task(&id, "lead-session-0001", "explore"));
    }
    // A short terminal overflows the table (agents popup = 70% height).
    let first = render_agents(&mut app, 160, 14);
    let area = app.active_agents_area;
    let max = app.active_agents_max_scroll;
    assert!(max >= STEP + 2, "12 tasks must overflow a 14-row popup");

    let (col, row) = centre(area);
    app.needs_redraw = false;
    app.handle_mouse_event(wheel_up(col, row));
    assert_eq!(app.active_agents_scroll_offset, STEP);
    assert!(app.needs_redraw);

    let scrolled = render_agents(&mut app, 160, 14);
    // task-02 sits below the fold at offset 0 (visible rows start at task-00
    // under a 14-row popup); one wheel-up brings it into the viewport.
    assert!(
        row_containing(&first, 14, "task-02").is_none(),
        "task-02 must be under the fold before the wheel"
    );
    let painted =
        row_containing(&scrolled, 14, "task-02").expect("the wheel must bring task-02 into view");
    assert!(
        painted >= area.y && painted < area.y + area.height,
        "task-02 must paint inside the table, not elsewhere"
    );

    // Wheel up the rest of the way: the offset clamps at the rendered
    // maximum rather than overshooting (FR-059).
    let wheels = 12;
    for _ in 0..wheels {
        app.handle_mouse_event(wheel_up(col, row));
    }
    assert_eq!(
        app.active_agents_scroll_offset, max,
        "the wheel must clamp at the rendered maximum"
    );

    // The scrollbar paints in the table's rightmost column.
    let bar_x = area.x + area.width - 1;
    let mut has_thumb = false;
    for y in area.y..area.y + area.height {
        if let Some(cell) = first.cell((bar_x, y)) {
            if scrollbar_glyph(cell.symbol().chars().next().unwrap_or(' ')) {
                has_thumb = true;
            }
        }
    }
    assert!(
        has_thumb,
        "an overflowing table must paint its vertical scrollbar (FR-059)"
    );

    // A full render with a short list paints no scrollbar.
    let mut small = support::make_app();
    small.session_id = Some("lead-session-0001".to_string());
    small.agent_name = "general".to_string();
    let small_buffer = render_agents(&mut small, 160, 44);
    let small_area = small.active_agents_area;
    let small_bar_x = small_area.x + small_area.width - 1;
    let mut small_has_thumb = false;
    for y in small_area.y..small_area.y + small_area.height {
        if let Some(cell) = small_buffer.cell((small_bar_x, y)) {
            if scrollbar_glyph(cell.symbol().chars().next().unwrap_or(' ')) {
                small_has_thumb = true;
            }
        }
    }
    assert!(
        !small_has_thumb,
        "a non-overflowing table paints no scrollbar"
    );
}

/// The Teams table scrolls the same way against a rendered frame: the wheel
/// shifts the painted rows and the offset clamps at the rendered maximum
/// (FR-056, FR-059, acceptance 19).
#[test]
fn test_window_scroll_wheel_scrolls_the_rendered_teams_table_end_to_end() {
    let mut app = support::make_app();
    app.session_id = Some("lead-session-0001".to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(ragent_agent::team::TeamConfig::new(
        "wheel-scroll-teams",
        "lead-session-0001",
    ));
    for i in 0..12 {
        let mut member = ragent_agent::team::TeamMember::new(
            format!("member-{i:02}"),
            format!("tm-{i:03}"),
            "general",
        );
        member.status = ragent_agent::team::MemberStatus::Working;
        member.session_id = Some(format!("tm-{i:03}-session"));
        app.team_members.push(member);
    }

    let pre = render_teams(&mut app, 170, 14);
    let area = app.teams_area;
    let rows_area = app.teams_rows_area;
    let max = app.teams_max_scroll;
    assert!(max >= STEP + 2, "12 members must overflow a 14-row popup");

    let (col, row) = centre(area);
    app.handle_mouse_event(wheel_up(col, row));
    assert_eq!(app.teams_scroll_offset, STEP);

    let scrolled = render_teams(&mut app, 170, 14);
    // member-02 sits below the fold at offset 0 (the viewport opens on the
    // lead and the first members); one wheel-up brings it into the viewport.
    // The label paints as `<name>-<short id>` (FR-022).
    assert!(
        row_containing(&pre, 14, "member-02").is_none(),
        "12 members with a header and lead overflow the viewport"
    );
    let painted = row_containing(&scrolled, 14, "member-02")
        .expect("the wheel must bring member-02 into view");
    assert!(
        painted >= rows_area.y && painted < rows_area.y + rows_area.height,
        "member-02 must paint inside the table, not elsewhere"
    );

    for _ in 0..12 {
        app.handle_mouse_event(wheel_up(col, row));
    }
    assert_eq!(
        app.teams_scroll_offset, max,
        "the wheel must clamp at the rendered maximum"
    );
}

/// A wheel keeps every recorded hit-rect aligned with the row it sits on: the
/// renderer recorded the shifted rects, and the offset composition the row
/// click uses indexes the still-painted rows (FR-056, T-017).
#[test]
fn test_window_scroll_wheel_keeps_hit_rects_on_the_painted_rows() {
    let mut app = support::make_app();
    app.session_id = Some("lead-session-0001".to_string());
    app.agent_name = "general".to_string();
    for i in 0..12 {
        let id = format!("task-{i:02}");
        app.active_tasks
            .push(task(&id, "lead-session-0001", "explore"));
    }
    let _ = render_agents(&mut app, 160, 14);
    let area = app.active_agents_area;
    let (col, row) = centre(area);
    app.handle_mouse_event(wheel_up(col, row));

    let buffer = render_agents(&mut app, 160, 14);
    for (i, task_id) in app.agent_row_button_task_ids.iter().enumerate() {
        let rect = app.agent_row_button_areas[i];
        if task_id.is_empty() || rect == Rect::default() {
            continue;
        }
        let painted: String = (rect.x..rect.x + rect.width)
            .filter_map(|x| buffer.cell((x, rect.y)).map(|c| c.symbol().to_string()))
            .collect();
        assert!(
            painted.contains("[stop]"),
            "click target for {task_id} must sit on its painted control: {painted:?}"
        );
    }
}

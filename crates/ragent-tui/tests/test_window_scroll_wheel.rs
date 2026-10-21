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
    // The content area of a 14-row popup paints the header, the primary row and
    // five sub-agent rows, so task-05 is the first row below the fold at offset
    // 0; one wheel-up brings it into the viewport.
    assert!(
        row_containing(&first, 14, "task-05").is_none(),
        "task-05 must be under the fold before the wheel"
    );
    let painted =
        row_containing(&scrolled, 14, "task-05").expect("the wheel must bring task-05 into view");
    assert!(
        painted >= area.y && painted < area.y + area.height,
        "task-04 must paint inside the table, not elsewhere"
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
    // The content area paints the table title, the header, the lead row and
    // three teammate rows, so member-04 sits below the fold at offset 0; one
    // wheel-up brings it into the viewport. The label paints as
    // `<name>-<short id>` (FR-022).
    assert!(
        row_containing(&pre, 14, "member-04").is_none(),
        "12 members with a title, header and lead overflow the viewport"
    );
    let painted = row_containing(&scrolled, 14, "member-04")
        .expect("the wheel must bring member-04 into view");
    assert!(
        painted >= rows_area.y && painted < rows_area.y + rows_area.height,
        "member-04 must paint inside the table, not elsewhere"
    );

    for _ in 0..12 {
        app.handle_mouse_event(wheel_up(col, row));
    }
    assert_eq!(
        app.teams_scroll_offset, max,
        "the wheel must clamp at the rendered maximum"
    );
}

// ---------------------------------------------------------------------------
// Scrollbar-gutter drag (FR-059's mouse half; the row clicks are keyboard-only)
// ---------------------------------------------------------------------------

/// A left-button press, drag movement and release event.
/// (The hit-rect-on-wheel test was retired with the row clicks: the cursor
/// row maps are scroll-invariant, which `test_hit_rect_lifecycle.rs` pins.)
const fn mouse_event(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

/// Clicking the Agents scrollbar gutter starts a thumb drag with
/// jump-to-position, a Drag event keeps moving the offset, and the button
/// release clears the drag state (FR-059).
#[test]
fn test_agents_scrollbar_gutter_drag_jumps_then_drags_then_ends() {
    let mut app = support::make_app();
    app.active_agents_area = Rect::new(5, 5, 100, 20);
    app.active_agents_max_scroll = 20;

    // Click at the gutter's column, near the top of the track: jump-to-
    // position.
    let gutter = app.active_agents_area.right() - 1;
    app.needs_redraw = false;
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Down(crossterm::event::MouseButton::Left),
        gutter,
        8,
    ));
    assert_eq!(
        app.scrollbar_drag,
        Some(ragent_tui::app::ScrollbarDragPane::AgentsWindow),
        "a gutter click must start the Agents-window drag (FR-059)"
    );
    let jumped = app.active_agents_scroll_offset;
    assert!(
        jumped > 0 && jumped < 20,
        "the click must jump the offset to a track-fraction position, got {jumped}"
    );

    // A drag further down the track moves the offset again.
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Drag(crossterm::event::MouseButton::Left),
        gutter,
        15,
    ));
    let dragged = app.active_agents_scroll_offset;
    assert!(
        dragged >= jumped,
        "dragging down the track must not scroll up: {jumped} -> {dragged}"
    );

    // Releasing the button clears the drag state.
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Up(crossterm::event::MouseButton::Left),
        gutter,
        15,
    ));
    assert_eq!(
        app.scrollbar_drag, None,
        "the release must clear the drag state (FR-059)"
    );
}

/// The Teams scrollbar gutter drags the same way (FR-059).
#[test]
fn test_teams_scrollbar_gutter_drag_jumps_then_drags_then_ends() {
    let mut app = support::make_app();
    app.teams_area = Rect::new(5, 5, 110, 20);
    app.teams_max_scroll = 15;

    let gutter = app.teams_area.right() - 1;
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Down(crossterm::event::MouseButton::Left),
        gutter,
        10,
    ));
    assert_eq!(
        app.scrollbar_drag,
        Some(ragent_tui::app::ScrollbarDragPane::TeamsWindow),
        "a gutter click must start the Teams-window drag (FR-059)"
    );
    let jumped = app.teams_scroll_offset;
    assert!(
        jumped > 0 && jumped < 15,
        "the click must jump the offset to a track-fraction position, got {jumped}"
    );

    // The track's last pixel row is the area's bottom edge row; fraction 1.0
    // jumps the offset to the maximum.
    let bottom_row = app.teams_area.bottom().saturating_sub(1);
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Down(crossterm::event::MouseButton::Left),
        gutter,
        bottom_row,
    ));
    let bottom = app.teams_scroll_offset;
    assert_eq!(
        bottom, 15,
        "a click at the track bottom jumps to the maximum (FR-059)"
    );

    app.handle_mouse_event(mouse_event(
        MouseEventKind::Up(crossterm::event::MouseButton::Left),
        gutter,
        bottom_row,
    ));
    assert_eq!(app.scrollbar_drag, None);
}

/// A gutter click on a non-overflowing table never starts a drag (there is no
/// scrollbar to grab).
#[test]
fn test_a_gutter_click_without_overflow_starts_no_drag() {
    let mut app = support::make_app();
    app.active_agents_area = Rect::new(5, 5, 100, 20);
    app.active_agents_max_scroll = 0;

    let gutter = app.active_agents_area.right() - 1;
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Down(crossterm::event::MouseButton::Left),
        gutter,
        8,
    ));
    assert_eq!(app.scrollbar_drag, None, "no scrollbar, no drag (FR-059)");
    assert_eq!(app.active_agents_scroll_offset, 0);
}

// ---------------------------------------------------------------------------
// PgUp / PgDn: the keyboard paging siblings of the wheel (FR-059's keyboard
// half; the cursor pages one viewport at a time)
// ---------------------------------------------------------------------------

/// PgUp on the Agents window pages the cursor up and clamps at the first row;
/// PgDn pages down and clamps at the last row (FR-034, FR-059).
#[tokio::test]
async fn test_agents_page_keys_move_the_cursor_one_page_and_clamp() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = support::make_app();
    app.session_id = Some("lead-session-0001".to_string());
    app.agent_name = "general".to_string();
    for i in 0..12 {
        let id = format!("task-{i:02}");
        app.active_tasks
            .push(task(&id, "lead-session-0001", "explore"));
    }
    let _ = render_agents(&mut app, 160, 14);
    assert!(
        app.active_agents_area.height > 0,
        "the view must be laid out"
    );

    // First PgDn starts from the top of the list.
    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
        .await;
    let first_page = app.agents_cursor_subagent.expect("the cursor must be set");
    assert!(
        first_page >= 1,
        "one page must move the cursor at least one row: {first_page}"
    );
    for _ in 0..8 {
        app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
            .await;
    }
    assert_eq!(
        app.agents_cursor_subagent,
        Some(11),
        "paging down past the end clamps at the last sub-agent (FR-034)"
    );

    for _ in 0..8 {
        app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE))
            .await;
    }
    assert_eq!(
        app.agents_cursor_subagent,
        Some(0),
        "paging up past the top clamps at the first sub-agent (FR-034)"
    );
}

/// The Teams PgUp/PgDn behave the same way (FR-038, FR-059).
#[tokio::test]
async fn test_teams_page_keys_move_the_cursor_one_page_and_clamp() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = support::make_app();
    app.session_id = Some("lead-session-0001".to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(ragent_agent::team::TeamConfig::new(
        "page-keys-teams",
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
    let _ = render_teams(&mut app, 170, 14);

    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
        .await;
    let first_page = app.teams_cursor_teammate.expect("the cursor must be set");
    assert!(
        first_page >= 1,
        "one page must move the cursor: {first_page}"
    );

    for _ in 0..8 {
        app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
            .await;
    }
    assert_eq!(
        app.teams_cursor_teammate,
        Some(11),
        "paging down past the end clamps at the last teammate (FR-038)"
    );

    for _ in 0..8 {
        app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE))
            .await;
    }
    assert_eq!(
        app.teams_cursor_teammate,
        Some(0),
        "paging up past the top clamps at the first teammate (FR-038)"
    );
}

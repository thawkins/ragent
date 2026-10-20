//! Tests for the hit-rect lifecycle (spec `agentui` T-016).
//!
//! Covers the index alignment of every control hit-rect vector with its
//! identifier vector and with the painted rows (NFR-011), the terminal-row
//! `Rect::default()` placeholders and their inert clicks (FR-071), the
//! scroll-offset composition that keeps a click on the painted cell (FR-056),
//! the area resets while a side panel occupies the side column or no side
//! column exists (FR-057), the stale-row no-op without mutating unrelated
//! state (FR-069) and the blank popup cell that consumes the click (FR-070).
//!
//! Every case drives the real public render entry point through a ratatui
//! `TestBackend` and the real mouse router ([`App::handle_mouse_event`]), so
//! the assertions exercise the same path a real click takes (NFR-001). All
//! state is synthetic and in-memory; nothing is written into the repository
//! (NFR-012, NFR-006 - the test code itself uses safe index lookups only).

mod support;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember};
use ragent_tui::App;
use ragent_tui::layout_active_agents::{columns, glyphs};
use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-0001";

/// A team name no team store on disk can own, so the FR-060 render hydration
/// never overrides the synthetic member statuses these cases rely on.
const TEAM_NAME: &str = "hit-rect-lifecycle";

/// A left-button press at `(col, row)`.
const fn click(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

/// A sub-agent [`TaskEntry`] with `agent`'s label and a session id derived
/// from the task id.
fn task(id: &str, parent_session: &str, agent: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: parent_session.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: agent.to_string(),
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
    }
}

/// A background shell task view.
fn bg_task(id: &str, command: &str, status: &str) -> ragent_tui::app::BgTaskView {
    ragent_tui::app::BgTaskView {
        id: id.to_string(),
        session_id: LEAD_SESSION.to_string(),
        command: command.to_string(),
        status: status.to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    }
}

/// A teammate in `status` with a spawned session id.
fn member(agent_id: &str, name: &str, status: MemberStatus) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// Render `app` with the Agents window open and return the drawn buffer.
fn render_agents(app: &mut App, width: u16, height: u16) -> Buffer {
    app.show_agents_window = true;
    render_plain(app, width, height)
}

/// Render `app` with the Teams window open and return the drawn buffer.
fn render_teams(app: &mut App, width: u16, height: u16) -> Buffer {
    app.show_teams_window = true;
    render_plain(app, width, height)
}

/// Render one full frame with the plain chat surface (no popup forced).
fn render_plain(app: &mut App, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
    terminal.backend().buffer().clone()
}

/// Flatten one full screen row into a string.
fn screen_row(buffer: &Buffer, y: u16) -> String {
    let mut out = String::new();
    for x in 0..buffer.area.width {
        match buffer.cell((x, y)) {
            Some(cell) => out.push_str(cell.symbol()),
            None => out.push('?'),
        }
    }
    out
}

/// The first screen row containing `needle`.
fn row_containing(buffer: &Buffer, needle: &str) -> u16 {
    (0..buffer.area.height)
        .find(|&y| screen_row(buffer, y).contains(needle))
        .unwrap_or_else(|| panic!("{needle:?} paints nowhere"))
}

// ---------------------------------------------------------------------------
// NFR-011 / FR-071: alignment and placeholders
// ---------------------------------------------------------------------------

/// Every control hit-rect vector stays index-aligned with its identifier
/// vector across live rows and terminal placeholders (NFR-011), and a terminal
/// row records the inert `Rect::default()` placeholder with an empty
/// identifier (FR-071, acceptance 6).
#[test]
fn test_hit_rect_lifecycle_agents_vectors_stay_index_aligned_with_placeholders() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks = vec![
        task("live-a", LEAD_SESSION, "explore", TaskStatus::Running),
        task("done-b", LEAD_SESSION, "rust-coder", TaskStatus::Completed),
        task("live-c", LEAD_SESSION, "plan", TaskStatus::Suspended),
    ];
    app.bg_tasks = vec![
        bg_task("bg-live", "sleep 30", "running"),
        bg_task("bg-done", "true", "completed"),
    ];

    render_agents(&mut app, 160, 44);

    assert_eq!(
        app.agent_row_button_areas.len(),
        app.agent_row_button_task_ids.len(),
        "the stop hit-rect vector must stay index-aligned with its ids (NFR-011)"
    );
    assert_eq!(
        app.agent_row_button_areas.len(),
        5,
        "one entry per action-capable painted row"
    );
    assert_eq!(
        app.agent_row_kill_areas.len(),
        app.agent_row_kill_task_ids.len()
    );
    assert_eq!(app.agent_row_kill_areas.len(), 5);

    // The identifiers keep paint order; terminal rows are empty placeholders.
    assert_eq!(
        app.agent_row_button_task_ids,
        vec![
            "live-a".to_string(),
            String::new(),
            "live-c".to_string(),
            "bg-live".to_string(),
            String::new(),
        ]
    );
    assert_eq!(app.agent_row_kill_task_ids, app.agent_row_button_task_ids);

    for (i, rect) in app.agent_row_button_areas.iter().enumerate() {
        if app.agent_row_button_task_ids[i].is_empty() {
            assert_eq!(
                *rect,
                Rect::default(),
                "a terminal row's entry is the inert placeholder (FR-071)"
            );
        } else {
            assert_eq!(
                rect.width,
                columns::STOP_W as u16,
                "live entries keep their rect"
            );
        }
    }
}

/// The Teams control vectors use the same alignment and placeholder rule
/// (NFR-011, FR-071).
#[test]
fn test_hit_rect_lifecycle_teams_vectors_stay_index_aligned_with_placeholders() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members = vec![
        member("tm-001", "reviewer", MemberStatus::Working),
        member("tm-002", "tester", MemberStatus::Failed),
        member("tm-003", "archivist", MemberStatus::Idle),
    ];

    render_teams(&mut app, 170, 44);

    assert_eq!(
        app.team_row_button_areas.len(),
        app.team_row_button_agent_ids.len(),
        "the stop hit-rect vector must stay index-aligned with its ids (NFR-011)"
    );
    assert_eq!(
        app.team_row_button_areas.len(),
        3,
        "one entry per painted teammate row"
    );
    assert_eq!(
        app.team_row_kill_areas.len(),
        app.team_row_kill_agent_ids.len(),
    );
    assert_eq!(app.team_row_kill_areas.len(), 3);
    assert_eq!(
        app.team_row_button_agent_ids,
        vec!["tm-001".to_string(), String::new(), "tm-003".to_string()]
    );
    assert_eq!(app.team_row_kill_agent_ids, app.team_row_button_agent_ids);
    assert_eq!(
        app.team_row_button_areas[1],
        Rect::default(),
        "a terminal row's entry is the inert placeholder (FR-071)"
    );
    assert_eq!(
        app.team_row_kill_areas[1],
        Rect::default(),
        "a terminal row's kill entry is the inert placeholder (FR-071)"
    );
}

// ---------------------------------------------------------------------------
// FR-071: placeholder clicks are inert
// ---------------------------------------------------------------------------

/// A clickable rect whose identifier is empty - the exact shape the
/// renderers record for a terminal row - is consumed by the router and never
/// resolves to a task: no dispatch, no output view, no unrelated state
/// mutated (FR-071, FR-069, FR-070). The vectors are driven directly (as the
/// mouse-control coverage does) so the guard is exercised at the real router
/// entry point.
#[tokio::test]
async fn test_hit_rect_lifecycle_agents_empty_identifier_click_is_inert() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks
        .push(task("live-a", LEAD_SESSION, "explore", TaskStatus::Running));
    app.bg_tasks = vec![bg_task("bg-live", "sleep 30", "running")];
    app.agent_row_button_areas = vec![Rect::new(10, 10, 6, 1)];
    app.agent_row_button_task_ids = vec![String::new()];
    app.agent_row_kill_areas = vec![Rect::new(20, 10, 4, 1)];
    app.agent_row_kill_task_ids = vec![String::new()];

    app.status = "sentinel".to_string();
    app.handle_mouse_event(click(13, 10)); // [stop] placeholder
    app.handle_mouse_event(click(22, 10)); // [x] placeholder

    assert_eq!(
        app.status, "sentinel",
        "the empty-identifier click mutates nothing (FR-071)"
    );
    assert!(
        app.output_view.is_none(),
        "the empty-identifier click opens nothing"
    );
    assert_eq!(app.selected_agent_index, None);
    assert_eq!(
        app.active_tasks.len(),
        1,
        "no suspend/resume was dispatched"
    );
    assert_eq!(app.bg_tasks.len(), 1, "no cancellation was dispatched");
}

/// The Teams router applies the same guard: a clickable empty-identifier
/// entry never focuses, never dispatches and leaves the member list alone
/// (FR-071, FR-069).
#[tokio::test]
async fn test_hit_rect_lifecycle_teams_empty_identifier_click_is_inert() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members = vec![member("tm-001", "reviewer", MemberStatus::Working)];
    app.team_row_button_areas = vec![Rect::new(10, 10, 6, 1)];
    app.team_row_button_agent_ids = vec![String::new()];
    app.team_row_kill_areas = vec![Rect::new(20, 10, 4, 1)];
    app.team_row_kill_agent_ids = vec![String::new()];

    app.status = "sentinel".to_string();
    app.handle_mouse_event(click(13, 10));
    app.handle_mouse_event(click(22, 10));

    assert_eq!(
        app.status, "sentinel",
        "the empty-identifier click mutates nothing (FR-071)"
    );
    assert!(
        app.focused_teammate.is_none(),
        "the empty-identifier click focuses nothing"
    );
    assert!(app.output_view.is_none());
    assert_eq!(app.team_members.len(), 1, "the member list is untouched");
}

// ---------------------------------------------------------------------------
// FR-056: hit-rects track scrolling
// ---------------------------------------------------------------------------

/// A non-zero scroll offset shifts every recorded rect by the offset plus the
/// table origin, so each entry sits on the row painted after the scroll
/// (FR-056), and a control scrolled out of view falls back to the inert
/// placeholder.
///
/// The terminal is deliberately short (like
/// `layout_agents_window_scrolls_past_the_visible_window`) so 12 tasks
/// overflow the table and the offset actually clamps to a positive maximum.
#[test]
fn test_hit_rect_lifecycle_scroll_shifts_the_recorded_rects() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    for i in 0..12 {
        app.active_tasks.push(task(
            &format!("task-{i:02}"),
            LEAD_SESSION,
            "explore",
            TaskStatus::Running,
        ));
    }
    // Row space: header = 0, primary = 1, task-i = 2 + i.
    let row_space: Vec<u16> = (0..12).map(|i| 2 + i).collect();
    app.active_agents_scroll_offset = 2;
    let buffer = render_agents(&mut app, 160, 14);
    let area = app.active_agents_area;
    assert!(
        app.active_agents_max_scroll > 2,
        "12 tasks must overflow the short-terminal table"
    );

    for (i, space) in row_space.iter().enumerate() {
        let expected_row = i32::from(*space) - 2;
        let rect = app.agent_row_button_areas[i];
        if (0..area.height as i32).contains(&expected_row) {
            assert_eq!(
                rect.y,
                area.y + expected_row as u16,
                "the shifted rect must sit on the row painted after the scroll (FR-056)"
            );
            assert_eq!(rect.x, area.x + columns::STOP_X as u16);
            // And the painted control is on that very cell.
            let row = screen_row(&buffer, rect.y);
            assert!(
                row.contains(glyphs::STOP),
                "the shifted cell paints [stop]: {row:?}"
            );
        } else {
            assert_eq!(
                rect,
                Rect::default(),
                "a control scrolled out of view becomes the inert placeholder"
            );
        }
    }
}

/// A row click resolves through the painted-row map under scroll, opening the
/// output view of the task the user actually sees on that row (FR-056, A3).
#[test]
fn test_hit_rect_lifecycle_scrolled_row_click_resolves_to_the_painted_task() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    for i in 0..12 {
        app.active_tasks.push(task(
            &format!("task-{i:02}"),
            LEAD_SESSION,
            "explore",
            TaskStatus::Running,
        ));
    }
    app.active_agents_scroll_offset = 2;
    let buffer = render_agents(&mut app, 160, 14);
    let area = app.active_agents_area;

    // In the scrolled table only row spaces >= 2 paint, so the visible tasks
    // are 00..02; task-02 sits on row space 4, painted at area.y + 2.
    let painted = row_containing(&buffer, "task-02");
    assert_eq!(
        painted,
        area.y + 2,
        "task-02 must paint at its shifted row (FR-056)"
    );
    app.needs_redraw = false;
    app.handle_mouse_event(click(area.x + 5, painted));

    let view = app
        .output_view
        .as_ref()
        .expect("the scrolled row opens its view");
    match &view.target {
        ragent_tui::app::OutputViewTarget::Session { session_id, .. } => {
            assert_eq!(
                session_id, "task-02-session",
                "the click resolves to the painted task (FR-056)"
            );
        }
        _ => panic!("a sub-agent click opens a Session output view"),
    }
}

// ---------------------------------------------------------------------------
// FR-057: cleared areas when a side panel hides the surface
// ---------------------------------------------------------------------------

/// While a side panel occupies the side column (each of the exclusive panels),
/// and while no side column exists, both panel areas reset to the empty
/// rectangle so no hidden surface can receive a click (FR-057).
#[test]
fn test_hit_rect_lifecycle_side_panels_clear_both_areas() {
    type Flag = fn(&mut App);
    let cases: [(&str, Flag); 6] = [
        ("tasks", |app| app.show_tasks_panel = true),
        ("memory", |app| app.show_memory = true),
        ("telemetry", |app| app.show_telemetry = true),
        ("context", |app| app.show_context_panel = true),
        ("profile", |app| app.show_profile = true),
        ("no-side-column", |_| {}),
    ];
    for (case, set_flag) in &cases {
        let mut app = make_app();
        // Poison the areas so only a reset clears them.
        app.active_agents_area = Rect::new(1, 1, 40, 10);
        app.teams_area = Rect::new(1, 1, 40, 10);
        set_flag(&mut app);
        let _ = render_plain(&mut app, 140, 40);

        assert_eq!(
            app.active_agents_area,
            Rect::default(),
            "case {case}: the hidden active-agents surface must reset to the empty rect (FR-057)"
        );
        assert_eq!(
            app.teams_area,
            Rect::default(),
            "case {case}: the hidden teams surface must reset to the empty rect (FR-057)"
        );
    }
}

/// The log panel (the one side panel FR-057 carves out) still clears both
/// areas while it occupies the side column.
#[test]
fn test_hit_rect_lifecycle_log_panel_clears_both_areas() {
    let mut app = make_app();
    app.active_agents_area = Rect::new(1, 1, 40, 10);
    app.teams_area = Rect::new(1, 1, 40, 10);
    app.show_log = true;
    let _ = render_plain(&mut app, 140, 40);

    assert_eq!(
        app.active_agents_area,
        Rect::default(),
        "the log-panel branch resets the active-agents area"
    );
    assert_eq!(
        app.teams_area,
        Rect::default(),
        "the log-panel branch resets the teams area"
    );
}

// ---------------------------------------------------------------------------
// FR-069: stale control clicks mutate nothing
// ---------------------------------------------------------------------------

/// A recorded control rect whose task no longer exists is ignored without
/// panic and without mutating unrelated state (FR-069, NFR-006).
#[tokio::test]
async fn test_hit_rect_lifecycle_stale_control_click_is_ignored() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks
        .push(task("gone-a", LEAD_SESSION, "explore", TaskStatus::Running));
    let _ = render_agents(&mut app, 160, 44);

    // The task list changes between render and click (FR-069).
    app.active_tasks.clear();
    app.status = "sentinel".to_string();
    let rect = app.agent_row_button_areas[0];
    app.handle_mouse_event(click(rect.x + rect.width / 2, rect.y));

    assert_eq!(
        app.status, "sentinel",
        "a stale control click mutates nothing (FR-069)"
    );
    assert!(app.output_view.is_none());
}

// ---------------------------------------------------------------------------
// FR-070: blank popup cells consume the click
// ---------------------------------------------------------------------------

/// A left-click on a blank cell below the last painted row inside the Agents
/// popup table area is consumed: nothing on the surface beneath reacts
/// (FR-070).
#[test]
fn test_hit_rect_lifecycle_agents_blank_cell_click_is_consumed() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks
        .push(task("solo-a", LEAD_SESSION, "explore", TaskStatus::Running));
    let buffer = render_agents(&mut app, 160, 44);

    let area = app.active_agents_area;
    let last = row_containing(&buffer, "solo-a");
    // Two rows below the last painted row is still inside the table area
    // because the popup is 70% of a 44-row terminal.
    let blank_y = (last + 2).min(area.y + area.height - 1);
    assert!(
        blank_y < area.y + area.height,
        "the blank cell must sit inside the table"
    );

    app.status = "sentinel".to_string();
    app.handle_mouse_event(click(area.x + 10, blank_y));

    assert!(
        app.output_view.is_none(),
        "the blank cell opens nothing (FR-070)"
    );
    assert_eq!(
        app.selected_agent_index, None,
        "the blank cell selects nothing"
    );
    assert_eq!(
        app.status, "sentinel",
        "the blank cell mutates nothing on the surface beneath"
    );
}

/// A left-click on a blank cell below the Teams table's content is consumed
/// too (FR-070).
#[test]
fn test_hit_rect_lifecycle_teams_blank_cell_click_is_consumed() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members = vec![member("tm-001", "reviewer", MemberStatus::Working)];
    let buffer = render_teams(&mut app, 170, 44);

    let area = app.teams_area;
    // The id column paints only in the table (not in the teammate strip), so
    // this row probe cannot land on the strip above the status bar.
    let last = row_containing(&buffer, "tm-001");
    let blank_y = (last + 3).min(area.y + area.height - 1);
    app.status = "sentinel".to_string();
    app.handle_mouse_event(click(area.x + 10, blank_y));

    assert!(
        app.focused_teammate.is_none(),
        "the blank cell focuses nothing (FR-070)"
    );
    assert_eq!(
        app.status, "sentinel",
        "the blank cell mutates nothing on the surface beneath"
    );
}

// ---------------------------------------------------------------------------
// Glyph sanity for the control columns used by the tests above
// ---------------------------------------------------------------------------

/// The `[stop]` and `[x]` glyph constants stay pinned in the renderer module
/// so the tests above cannot silently drift from the painted controls
/// (NFR-003).
#[test]
fn test_hit_rect_lifecycle_control_glyphs_are_the_documented_strings() {
    assert_eq!(glyphs::STOP, "[stop]");
    assert_eq!(glyphs::KILL, "[x]");
}

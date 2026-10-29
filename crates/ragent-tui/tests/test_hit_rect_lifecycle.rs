//! Tests for the row-map lifecycle behind the Agents / Teams popup windows
//! (spec `agentui` T-016, keyboard-first rework).
//!
//! The hit-rect state vectors were retired: the windows are keyboard-driven
//! through a block cursor, so the renderer now publishes the painted *row
//! maps* (`agents_row_task_ids` + `agents_subagent_task_ids`,
//! `teams_row_agent_ids` + `teams_teammate_agent_ids`) that the cursor helpers
//! resolve against. This file pins the new invariants in their place:
//!
//! - alignment: the row map stays index-aligned with the painted rows and the
//!   cursor ordinal list is exactly the cursorable slice of it (NFR-011's
//!   alignment contract, re-expressed on the published vectors);
//! - scroll-independence: the maps stay correct across scroll offsets - one
//!   map per painted row no matter where the viewport sits (FR-056's paint
//!   mapping contract, re-expressed);
//! - the cursor rectangle is painted on the row the cursor names: after a
//!   scroll the painted cursor row is the one whose buffer background is the
//!   cursor colour (FR-056, FR-034);
//! - the area resets while a side panel occupies the side column or no side
//!   column exists (FR-057, kept from the old suite);
//! - the control glyphs stay the documented strings (NFR-003, kept).
//!
//! Every case drives the real public render entry point through a ratatui
//! `TestBackend`, so the assertions exercise the same path a real frame takes
//! (NFR-001). All state is synthetic; nothing is written into the repository
//! (NFR-012).

mod support;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember};
use ragent_tui::App;
use ragent_tui::layout_active_agents::{columns, glyphs};
use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-0001";

/// A team name no team store on disk can own, so the FR-060 render hydration
/// never overrides the synthetic member statuses these cases rely on.
const TEAM_NAME: &str = "row-map-lifecycle";

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
        time_bound_secs: None,
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
// NFR-011: the published row maps stay index-aligned with the painted rows
// ---------------------------------------------------------------------------

/// The Agents row map carries one key per painted row (empty on the header,
/// primary, background heading and background rows, the task id on a sub-agent
/// row), and the cursor ordinal list is exactly the sub-agent slice (NFR-011).
#[test]
fn test_agents_row_maps_stay_index_aligned_with_the_painted_rows() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks = vec![
        task("live-a", LEAD_SESSION, "explore", TaskStatus::Running),
        task("done-b", LEAD_SESSION, "rust-coder", TaskStatus::Completed),
        task("live-c", LEAD_SESSION, "plan", TaskStatus::Suspended),
    ];
    app.bg_tasks = vec![
        ragent_tui::app::BgTaskView {
            id: "bg-live".to_string(),
            session_id: LEAD_SESSION.to_string(),
            command: "sleep 30".to_string(),
            status: "running".to_string(),
            created_at: chrono::Utc::now(),
            completed_at: None,
        },
        ragent_tui::app::BgTaskView {
            id: "bg-done".to_string(),
            session_id: LEAD_SESSION.to_string(),
            command: "true".to_string(),
            status: "completed".to_string(),
            created_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        },
    ];

    let buffer = render_agents(&mut app, 160, 44);
    let area = app.active_agents_area;

    // Painted rows: 0 header, 1 primary, 2 live-a, 3 done-b, 4 live-c, then
    // the background heading and two background rows.
    assert_eq!(
        app.agents_row_task_ids,
        vec![
            String::new(),
            "primary".to_string(),
            "live-a".to_string(),
            "done-b".to_string(),
            "live-c".to_string(),
            String::new(),
            String::new(),
            String::new(),
        ],
        "one row key per painted row, aligned with the paint order (NFR-011)"
    );
    assert_eq!(
        app.agents_subagent_task_ids,
        vec![
            "live-a".to_string(),
            "done-b".to_string(),
            "live-c".to_string(),
        ],
        "the cursor ordinal list is the sub-agent slice in paint order (NFR-011)"
    );

    // The keys name the rows that actually paint those ids (a terminal row
    // still paints and still carries its key - the cursor resolves the status
    // through the task list, not through the paint, FR-071's re-expression).
    let painted_row = row_containing(&buffer, "live-a");
    assert!(
        painted_row >= area.y && painted_row < area.y + area.height,
        "the painted rows must include live-a"
    );
    assert_eq!(
        app.agents_row_task_ids.len(),
        8,
        "the map is republished on every render with the same row count"
    );
}

/// The Teams row map uses the same alignment and cursor-slice rule (NFR-011).
#[test]
fn test_teams_row_maps_stay_index_aligned_with_the_painted_rows() {
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

    // Painted rows: header, lead (its own sentinel), three teammates (a
    // terminal member still paints and still carries its key, FR-071's
    // re-expression).
    assert_eq!(
        app.teams_row_agent_ids,
        vec![
            String::new(),
            "lead".to_string(),
            "tm-001".to_string(),
            "tm-002".to_string(),
            "tm-003".to_string(),
        ],
        "one row key per painted row, aligned with the paint order (NFR-011)"
    );
    assert_eq!(
        app.teams_teammate_agent_ids,
        vec![
            "tm-001".to_string(),
            "tm-002".to_string(),
            "tm-003".to_string(),
        ],
        "the cursor ordinal list is the teammate slice in paint order (NFR-011)"
    );
}

// ---------------------------------------------------------------------------
// FR-056: the row maps stay correct across scroll offsets
// ---------------------------------------------------------------------------

/// The Agents row maps are unchanged by scrolling: the same row count, same
/// ids, one map per painted line - the viewport only picks the slice to paint
/// (FR-056).
#[test]
fn test_agents_row_maps_are_unchanged_by_scrolling() {
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

    app.active_agents_scroll_offset = 0;
    render_agents(&mut app, 160, 14);
    let scrolled_at_zero = app.agents_row_task_ids.clone();
    assert!(app.active_agents_max_scroll > 0, "12 tasks must overflow");

    // Render twice with different offsets; the published maps must agree.
    app.active_agents_scroll_offset = 2;
    render_agents(&mut app, 160, 14);
    let scrolled_at_two = app.agents_row_task_ids.clone();

    app.active_agents_scroll_offset = 5;
    render_agents(&mut app, 160, 14);

    assert_eq!(
        app.agents_row_task_ids, scrolled_at_two,
        "the row map must not depend on the scroll offset (FR-056)"
    );
    assert_eq!(
        app.agents_row_task_ids, scrolled_at_zero,
        "the row map must not depend on the scroll offset (FR-056)"
    );
    assert_eq!(
        app.agents_subagent_task_ids,
        (0..12).map(|i| format!("task-{i:02}")).collect::<Vec<_>>(),
        "the cursor list covers every sub-agent row in paint order (FR-056)"
    );
}

/// The Teams row maps are unchanged by scrolling too (FR-056).
#[test]
fn test_teams_row_maps_are_unchanged_by_scrolling() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    for i in 0..12 {
        app.team_members.push(member(
            &format!("tm-{i:03}"),
            &format!("member-{i:02}"),
            MemberStatus::Working,
        ));
    }

    app.teams_scroll_offset = 0;
    render_teams(&mut app, 170, 14);
    let at_zero = app.teams_row_agent_ids.clone();
    assert!(app.teams_max_scroll > 0, "12 members must overflow");

    app.teams_scroll_offset = 2;
    render_teams(&mut app, 170, 14);
    let at_two = app.teams_row_agent_ids.clone();

    app.teams_scroll_offset = 5;
    render_teams(&mut app, 170, 14);

    assert_eq!(
        app.teams_row_agent_ids, at_two,
        "the row map must not depend on the scroll offset (FR-056)"
    );
    assert_eq!(
        app.teams_row_agent_ids, at_zero,
        "the row map must not depend on the scroll offset (FR-056)"
    );
}

// ---------------------------------------------------------------------------
// FR-056 / FR-034: the painted cursor follows the cursor row across scroll
// ---------------------------------------------------------------------------

/// The cursor's painted rectangle sits on the row the cursor names, even after
/// the viewport scrolls: the painted row's background is the block-cursor
/// colour (FR-056, FR-034).
#[test]
fn test_the_agents_cursor_painted_row_follows_the_scroll() {
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

    // Put the cursor on sub-agent ordinal 3 (task-03) and scroll so it sits
    // mid-viewport rather than on the first visible row.
    app.agents_cursor_subagent = Some(3);
    app.active_agents_scroll_offset = 2;
    let buffer = render_agents(&mut app, 160, 14);
    let area = app.active_agents_area;

    // task-03 paints at row space 2 + 3 = 5; with the offset at 2 it paints
    // area.y + 3 inside the table.
    let painted = row_containing(&buffer, "task-03");
    assert_eq!(
        painted,
        area.y + 3,
        "task-03 must paint at its scrolled row (FR-056)"
    );
    // And that row carries the cursor background across its full width.
    for col in area.x..area.x + area.width - 1 {
        let cell = buffer
            .cell((col, painted))
            .cloned()
            .unwrap_or_else(|| panic!("cell off the buffer"));
        assert_eq!(
            cell.bg,
            Color::Rgb(50, 50, 90),
            "the cursor row must paint the block-cursor background (col {col}, FR-034)"
        );
    }
    // A neighbouring row must NOT carry it (the cursor is one row).
    let other = painted + 1;
    let cell = buffer
        .cell((area.x + 5, other))
        .cloned()
        .unwrap_or_else(|| panic!("cell off the buffer"));
    assert_ne!(
        cell.bg,
        Color::Rgb(50, 50, 90),
        "the row below the cursor must not wear the cursor background"
    );
}

/// The Teams cursor paints the same block background on the teammate row the
/// cursor names (FR-056, FR-038).
#[test]
fn test_the_teams_cursor_painted_row_follows_the_scroll() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    for i in 0..12 {
        app.team_members.push(member(
            &format!("tm-{i:03}"),
            &format!("member-{i:02}"),
            MemberStatus::Working,
        ));
    }

    // Cursor ordinal 2 = tm-002 (member-02), with a viewport offset that
    // shifts it below its top-of-table slot.
    app.teams_cursor_teammate = Some(2);
    app.teams_scroll_offset = 1;
    let buffer = render_teams(&mut app, 170, 44);
    let rows = app.teams_rows_area;

    let painted = row_containing(&buffer, "tm-002");
    assert!(
        painted >= rows.y && painted < rows.y + rows.height,
        "the cursor row must paint inside the table (FR-056)"
    );
    for col in rows.x..rows.x + rows.width - 1 {
        let cell = buffer
            .cell((col, painted))
            .cloned()
            .unwrap_or_else(|| panic!("cell off the buffer"));
        assert_eq!(
            cell.bg,
            Color::Rgb(50, 50, 90),
            "the cursor row must paint the block-cursor background (col {col}, FR-038)"
        );
    }
}

/// The sync helper keeps the cursor row visible: moving the cursor down past
/// the visible edge adjusts the scroll offset so the row is painted (FR-034's
/// scroll-sync half) - observable through the row maps and the offset itself.
#[test]
fn test_moving_the_cursor_past_the_visible_edge_scrolls_the_row_into_view() {
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
    // Publish the row maps and viewport first.
    render_agents(&mut app, 160, 14);

    // Move to the last sub-agent: cursor 11 must scroll the offset up so the
    // row paints instead of hiding below the fold.
    for _ in 0..30 {
        app.move_agents_cursor(1);
    }
    assert_eq!(app.agents_cursor_subagent, Some(11));
    let painted_offset = app.active_agents_scroll_offset;
    let visible = app.active_agents_area.height.max(1);
    assert!(
        usize::from(painted_offset) + usize::from(visible) >= 13,
        "the viewport must reach the last row after cursor 11: offset {painted_offset}, visible {visible}"
    );

    // The cursor row's task id is still in the row map (a paint-order
    // invariant the Space/Del/Enter resolution depends on).
    assert_eq!(
        app.agents_subagent_task_ids.get(11).map(String::as_str),
        Some("task-11"),
        "the cursor list must still name the last row (FR-034)"
    );
}

// ---------------------------------------------------------------------------
// FR-057: cleared areas when a side panel hides the surface
// ---------------------------------------------------------------------------

/// While a side panel occupies the side column (each of the exclusive panels),
/// and while no side column exists, both panel areas reset to the empty
/// rectangle so no hidden surface can receive a wheel event or a drag (FR-057).
#[test]
fn test_side_panels_clear_both_areas() {
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
fn test_log_panel_clears_both_areas() {
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
// NFR-003: the control glyphs stay the documented strings
// ---------------------------------------------------------------------------

/// The `[stop]` and `[x]` glyph constants stay pinned in the renderer module
/// so the row assertions cannot silently drift from the painted controls
/// (NFR-003).
#[test]
fn test_control_glyphs_are_the_documented_strings() {
    assert_eq!(glyphs::STOP, "[stop]");
    assert_eq!(glyphs::KILL, "[x]");
    assert_eq!(columns::STOP_X, 70, "the agents cursor column is pinned");
    assert_eq!(columns::KILL_X, 77, "the agents kill column is pinned");
}

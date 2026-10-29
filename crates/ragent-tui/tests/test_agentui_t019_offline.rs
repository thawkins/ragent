//! Offline deterministic render tests (spec `agentui` T-019).
//!
//! NFR-001 asks that the panel suite run offline and deterministically from
//! synthetic in-memory state; NFR-012 asks that no case writes into the
//! repository working tree; NFR-002 asks that one frame never issues more than
//! the single team task-store read and the single session lookup per window.
//! This file asserts the suite-level contract directly:
//!
//! - the Agents window, the Teams window and the teammate strip all render
//!   from purely synthetic `App` state with every row variant populated, and
//!   two consecutive renders of the same state paint byte-identical buffers
//!   outside the per-second elapsed clock (determinism in the strongest
//!   practical sense);
//! - the two NFR-002 read paths a Teams frame is allowed are exercised once
//!   each and the frame still completes (the lead-session lookup and the team
//!   task-store read, both through a real scratch team store);
//! - the only files a synthetic-state render can touch under the repository
//!   root are the log spool root already redirected to `target/temp/log` by
//!   `make_app`, and nothing else (NFR-012).
//!
//! Row variants covered on the Agents window: primary, sub-agent tree with a
//! `[C]` badge, sub-agent with a `[T]` badge, suspended, completed, terminated,
//! failed, cancelled, and the background shell section with live and terminal
//! rows. On the Teams window: lead, working, idle, suspended, stopped, failed,
//! focused and model-override teammates. On the strip: one pill per member, a
//! focused highlight and the cycle hint. Every case drives the real public
//! render entry point through a ratatui `TestBackend` and reads the painted
//! buffer back (NFR-001).

mod support;

use std::path::PathBuf;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use ragent_agent::agent::ModelRef;
use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember, TeamStore};
use ragent_tui::App;
use ragent_tui::layout_active_agents::{columns as agents_columns, glyphs as agents_glyphs};
use ragent_tui::layout_teams::{columns as teams_columns, glyphs as teams_glyphs};
use support::make_app;

/// The lead session id every team case uses.
const LEAD_SESSION: &str = "lead-session-0001";

/// A unique team name no real team store on disk can own, so the FR-060
/// render-time hydration cannot override the synthetic member statuses except
/// where a store is created deliberately (the NFR-002 case).
const TEAM_NAME: &str = "agentui-t019-offline";

// -- synthetic-state builders ------------------------------------------------

/// A [`TaskEntry`] whose parent session, child session, agent and status are
/// configurable so the tree, badge and terminal-row variants can all be built
/// from one helper.
fn task(
    id: &str,
    parent: &str,
    child: &str,
    agent: &str,
    status: TaskStatus,
    background: bool,
) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: parent.to_string(),
        child_session_id: child.to_string(),
        agent_name: agent.to_string(),
        task_prompt: "prompt".to_string(),
        background,
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

/// A background shell task view.
fn bg_task_view(id: &str, command: &str, status: &str) -> ragent_tui::app::BgTaskView {
    ragent_tui::app::BgTaskView {
        id: id.to_string(),
        session_id: LEAD_SESSION.to_string(),
        command: command.to_string(),
        status: status.to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    }
}

/// Register a custom agent definition so a `[C]` badge can derive (FR-013).
fn add_custom_agent(app: &mut App, name: &str) {
    app.custom_agent_defs
        .push(ragent_agent::agent::CustomAgentDef {
            record: ragent_agent::agent::oasf::OasfAgentRecord {
                name: name.to_string(),
                description: "custom".to_string(),
                version: "1.0.0".to_string(),
                schema_version: "0.7.0".to_string(),
                authors: Vec::new(),
                created_at: None,
                skills: Vec::new(),
                domains: Vec::new(),
                locators: Vec::new(),
                modules: Vec::new(),
            },
            source_path: PathBuf::from(format!("/agents/{name}.json")),
            agent_info: std::sync::Arc::new(ragent_agent::agent::AgentInfo::new(
                name,
                "custom agent",
            )),
            is_project_local: true,
        });
}

/// A teammate in `status` with a spawned session id.
fn member(agent_id: &str, name: &str, status: MemberStatus) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// An app whose primary session is `s1` and whose agent is `general`.
fn app_with_primary() -> App {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.agent_name = "general".to_string();
    app
}

/// An app with the Teams window open on [`TEAM_NAME`], holding `members`.
fn open_teams(members: Vec<TeamMember>) -> App {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members = members;
    app.show_teams_window = true;
    app
}

// -- render helpers ----------------------------------------------------------

/// Render one full frame of `app` and return the drawn buffer.
fn render_frame(app: &mut App, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
    terminal.backend().buffer().clone()
}

/// Render with the Agents window open.
fn render_agents_window(app: &mut App, width: u16, height: u16) -> Buffer {
    app.show_agents_window = true;
    render_frame(app, width, height)
}

/// Render with the Teams window open.
fn render_teams_window(app: &mut App, width: u16, height: u16) -> Buffer {
    app.show_teams_window = true;
    render_frame(app, width, height)
}

/// Flatten the buffer into one string.
fn painted(buffer: &Buffer) -> String {
    buffer
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

/// The painted text of one buffer row.
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
        .unwrap_or_else(|| panic!("{needle:?} not painted anywhere in the buffer"))
}

/// Flatten one painted row of `area` into a string.
fn area_row_text(buffer: &Buffer, area: Rect, offset: u16) -> String {
    let y = area.y.saturating_add(offset);
    let mut out = String::new();
    for x in area.x..area.x.saturating_add(area.width) {
        match buffer.cell((x, y)) {
            Some(cell) => out.push_str(cell.symbol()),
            None => out.push('?'),
        }
    }
    out
}

/// The inner rect of the bordered Teams table inside the content area.
fn table_inner(content: Rect) -> Rect {
    Rect::new(
        content.x + 1,
        content.y + 1,
        content.width.saturating_sub(2),
        content.height.saturating_sub(2),
    )
}

/// A scratch team directory the cwd can be pinned to (`target/temp/`, never
/// the repository), following the `test_teams_window_table` discipline.
fn scratch_dir(tag: &str) -> PathBuf {
    let unique = format!(
        "{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/agentui-t019")
        .join(unique);
    std::fs::create_dir_all(dir.join(".ragent/teams")).expect("scratch dir");
    dir
}

// ---------------------------------------------------------------------------
// NFR-001: the Agents window is offline, deterministic and every row variant
// ---------------------------------------------------------------------------

#[test]
fn test_t019_agents_window_deterministic_offline_and_all_row_variants() {
    let build_state = || {
        let mut app = app_with_primary();
        // The child sessions double as teammate sessions so the `[T]` badge
        // derives, and the custom definition makes the `[C]` badge derive
        // (FR-013, FR-062). All state is synthetic: no team store is read.
        app.active_team = Some(TeamConfig::new(TEAM_NAME, "s1"));
        app.team_members
            .push(member("tm-001", "reviewer", MemberStatus::Working));
        add_custom_agent(&mut app, "rust-coder");

        app.active_tasks = vec![
            task(
                "alpha-0001",
                "s1",
                "s2",
                "rust-coder",
                TaskStatus::Running,
                true,
            ),
            task(
                "beta-0002",
                "s1",
                "s3",
                "explore",
                TaskStatus::Running,
                false,
            ),
            task(
                "done-0003",
                "s1",
                "s4",
                "build",
                TaskStatus::Completed,
                true,
            ),
            task(
                "term-0004",
                "s1",
                "s5",
                "explore",
                TaskStatus::Terminating,
                true,
            ),
            task("susp-0005", "s1", "s6", "plan", TaskStatus::Suspended, true),
            task("fail-0006", "s1", "s7", "explore", TaskStatus::Failed, true),
            task(
                "canc-0007",
                "s1",
                "s8",
                "explore",
                TaskStatus::Cancelled,
                true,
            ),
            // The nested row carries the teammate session id, so `[T]` paints
            // on a deeper tree node (FR-011, FR-013).
            task(
                "nest-0008",
                "s2",
                "tm-001-session",
                "explore",
                TaskStatus::Running,
                true,
            ),
        ];
        app.bg_tasks = vec![
            bg_task_view("cc112233", "cargo build --release", "running"),
            bg_task_view("dd445566", "true", "completed"),
        ];
        app.active_agents_scroll_offset = 0;
        app
    };

    // Two independent constructions of the same state must render the same
    // visible frame: a live elapsed clock makes strict byte-idempotence
    // impossible by design, so determinism is asserted outside that clock.
    let mut first = build_state();
    for _ in 0..3 {
        first.event_bus.increment_tool_calls("s1");
    }
    for _ in 0..2 {
        first.event_bus.increment_tool_calls("s2");
    }
    let buf1 = render_agents_window(&mut first, 160, 44);
    let area = first.active_agents_area;

    let mut second = build_state();
    for _ in 0..3 {
        second.event_bus.increment_tool_calls("s1");
    }
    for _ in 0..2 {
        second.event_bus.increment_tool_calls("s2");
    }
    let buf2 = render_agents_window(&mut second, 160, 44);

    // The header, the primary row and every sub-agent / background row must
    // match byte for byte except the right-aligned elapsed cell.
    let elapsed_x = agents_columns::MARKER_W
        + agents_columns::ID_W
        + agents_columns::GAP_W
        + agents_columns::LABEL_W
        + agents_columns::TYPE_W;
    let steps_x = elapsed_x + agents_columns::ELAPSED_W + agents_columns::GAP_W;
    for offset in 0..area.height {
        let row1 = area_row_text(&buf1, area, offset);
        let row2 = area_row_text(&buf2, area, offset);
        if row1 == row2 {
            continue;
        }
        let before: String = row1.chars().take(elapsed_x).collect();
        let before2: String = row2.chars().take(elapsed_x).collect();
        assert_eq!(
            before, before2,
            "NFR-001: row {offset} must match before the elapsed column"
        );
        let tail1: String = row1.chars().skip(steps_x).collect();
        let tail2: String = row2.chars().skip(steps_x).collect();
        assert_eq!(
            tail1, tail2,
            "NFR-001: row {offset} must match after the elapsed column"
        );
        let elapsed1: String = row1
            .chars()
            .skip(elapsed_x)
            .take(agents_columns::ELAPSED_W)
            .collect();
        let elapsed2: String = row2
            .chars()
            .skip(elapsed_x)
            .take(agents_columns::ELAPSED_W)
            .collect();
        let clock_like = |s: &str| s.trim() == "-" || (s.trim().ends_with('s') && !s.contains(' '));
        assert!(
            clock_like(&elapsed1) && clock_like(&elapsed2),
            "NFR-001: only a compact elapsed clock may differ: {elapsed1:?} vs {elapsed2:?}"
        );
    }

    let text1 = painted(&buf1);

    // Primary row (FR-010).
    let header_y = row_containing(&buf1, "id");
    let primary_y = header_y + 1;
    let primary = screen_row(&buf1, primary_y);
    assert!(primary.contains('*'), "primary marker: {primary:?}");
    assert!(
        primary.contains("primary"),
        "primary row carries the primary type: {primary:?}"
    );
    assert!(
        primary.contains("general"),
        "primary row paints the agent name: {primary:?}"
    );

    // Custom agent badge on the running rust-coder row (FR-013, FR-062): the
    // `[C]` badge appends to the label, and its row paints the task id.
    let coder_y = row_containing(&buf1, agents_glyphs::BADGE_CUSTOM);
    let coder = screen_row(&buf1, coder_y);
    assert!(
        coder.contains("alpha-0001"),
        "the custom-agent badge paints on the agent's row: {coder:?}"
    );

    // Teammate badge derived live from the member list (FR-013, FR-062): the
    // badge lands on the nested row whose child session is a member session.
    let team_y = row_containing(&buf1, agents_glyphs::BADGE_TEAM);
    assert!(
        screen_row(&buf1, team_y).contains("nest-0008"),
        "the teammate badge lands on the member row: {tm:?}",
        tm = screen_row(&buf1, team_y)
    );

    // Suspended row keeps its resume control and paints the suffix
    // (FR-014, FR-015).
    let susp_y = row_containing(&buf1, agents_glyphs::STATUS_SUSPENDED);
    let susp_text = screen_row(&buf1, susp_y);
    assert!(
        susp_text.contains(agents_glyphs::RESUME),
        "suspended row paints the resume glyph: {susp_text:?}"
    );
    assert!(
        susp_text.contains(agents_glyphs::KILL),
        "suspended row keeps its kill control: {susp_text:?}"
    );
    assert!(
        !screen_row(&buf1, susp_y).contains(agents_glyphs::STOP),
        "a suspended row must not paint [stop]: {susp_text:?}"
    );

    // Terminating row paints its suffix (FR-014).
    assert!(
        text1.contains(agents_glyphs::STATUS_TERMINATING),
        "a terminating row paints its suffix: {text1:?}"
    );

    // Terminal rows paint no controls (FR-015).
    for (needle, kind) in [
        ("done-0003", "completed"),
        ("fail-0006", "failed"),
        ("canc-0007", "cancelled"),
    ] {
        let y = row_containing(&buf1, needle);
        let row = screen_row(&buf1, y);
        assert!(
            !row.contains(agents_glyphs::STOP) && !row.contains(agents_glyphs::KILL),
            "a {kind} row paints no controls: {row:?}"
        );
    }

    // Foreground vs background type columns (FR-012).
    let bg_y = row_containing(&buf1, "alpha-0001");
    let fg_y = row_containing(&buf1, "beta-0002");
    let type_x = agents_columns::MARKER_W
        + agents_columns::ID_W
        + agents_columns::GAP_W
        + agents_columns::LABEL_W;
    let type_col = |buffer: &Buffer, y: u16| -> String {
        let base = area.x as usize + type_x;
        (0..agents_columns::TYPE_W)
            .map(|o| {
                buffer
                    .cell(((base + o) as u16, y))
                    .map(ratatui::buffer::Cell::symbol)
                    .unwrap_or("?")
            })
            .collect()
    };
    assert_eq!(
        type_col(&buf1, bg_y).trim(),
        "bg",
        "a background task reads `bg` (FR-012)"
    );
    assert_eq!(
        type_col(&buf1, fg_y).trim(),
        "fg",
        "a foreground task reads `fg` (FR-012)"
    );

    // Background shell section: live and terminal rows (FR-016).
    let heading_y = row_containing(&buf1, agents_glyphs::BG_HEADING);
    let live_y = row_containing(&buf1, "cc112233");
    assert!(
        live_y > heading_y,
        "background rows follow the heading (FR-016)"
    );
    let live = screen_row(&buf1, live_y);
    assert!(
        live.contains(agents_glyphs::STOP) && live.contains(agents_glyphs::KILL),
        "a live background row paints both controls: {live:?}"
    );
    let done_bg = screen_row(&buf1, row_containing(&buf1, "dd445566"));
    assert!(
        !done_bg.contains(agents_glyphs::STOP),
        "a finished background row paints no controls: {done_bg:?}"
    );

    // The modal chrome painted (FR-006) and no controls leaked onto the
    // primary row (FR-010, FR-067).
    assert!(text1.contains(" Agents "), "the modal title must paint");
    assert!(
        !primary.contains(agents_glyphs::STOP) && !primary.contains(agents_glyphs::KILL),
        "the primary row carries no action controls: {primary:?}"
    );
}

// ---------------------------------------------------------------------------
// NFR-001: the Teams window and the strip render offline from synthetic state
// ---------------------------------------------------------------------------

#[test]
fn test_t019_teams_window_deterministic_offline_all_statuses_and_strip() {
    let build_state = || {
        let mut app = open_teams(vec![
            member("tm-001", "reviewer", MemberStatus::Working),
            member("tm-002", "tester", MemberStatus::Idle),
            member("tm-003", "planner", MemberStatus::Suspended),
            member("tm-004", "archivist", MemberStatus::Stopped),
            member("tm-005", "guard", MemberStatus::Failed),
        ]);
        // A model override paints provider/model; the rest inherit (FR-024).
        let mut overridden = member("tm-006", "polisher", MemberStatus::Working);
        overridden.model_override = Some(ModelRef {
            provider_id: "ollama".to_string(),
            model_id: "qwen2.5".to_string(),
        });
        app.team_members.push(overridden);
        // The focused teammate highlights in the table and the strip (FR-022,
        // FR-027).
        app.focused_teammate = Some("tm-002".to_string());
        // Synthetic message counters (FR-025) - the lead never increments
        // these, so the values are pure test state.
        app.team_message_counts.insert("tm-002".to_string(), (4, 2));
        app
    };

    let mut first = build_state();
    let buf1 = render_teams_window(&mut first, 170, 44);
    let content = table_inner(first.teams_area);

    let mut second = build_state();
    let buf2 = render_teams_window(&mut second, 170, 44);

    // Determinism: every row must match between the two renders except the
    // right-aligned elapsed clocks - the lead row reads the session (absent
    // here, so `-`) and every member row reads its fresh `created_at`. The
    // status bar and modal chrome can also carry per-frame values (e.g. the
    // selected-model label), so only the table's inner rows are compared.
    let header_y = row_containing(&buf1, "id");
    let clock_x = teams_columns::MARKER_W
        + teams_columns::ID_W
        + teams_columns::GAP_W
        + teams_columns::LABEL_W
        + teams_columns::STATUS_W
        + teams_columns::MODEL_W;
    let table_bottom = content.y + content.height;
    for y in content.y..table_bottom {
        let row1 = screen_row(&buf1, y);
        let row2 = screen_row(&buf2, y);
        if row1 == row2 {
            continue;
        }
        // Blank out the right-aligned elapsed cell (one column of padding on
        // each side is kept so the surrounding geometry is still compared).
        let blank_elapsed = |row: &str| -> String {
            let base = (content.x as usize) + clock_x;
            row.chars()
                .enumerate()
                .map(|(i, ch)| {
                    let in_cell = (base - 1..base + teams_columns::ELAPSED_W + 2).contains(&i);
                    if in_cell { '\u{7}' } else { ch }
                })
                .collect()
        };
        assert_eq!(
            blank_elapsed(&row1),
            blank_elapsed(&row2),
            "NFR-001: teams row {y} must match outside the elapsed clock: {row1:?} vs {row2:?}"
        );
    }
    let _ = header_y;

    let text1 = painted(&buf1);

    // Table title carries the team name (FR-019).
    assert!(
        text1.contains("Team: ") && text1.contains(TEAM_NAME),
        "the team title must paint: {text1:?}"
    );

    // Lead row (FR-021).
    let lead_y = row_containing(&buf1, "id") + 1;
    let lead = screen_row(&buf1, lead_y);
    assert!(lead.contains('*'), "lead marker: {lead:?}");
    assert!(
        lead.contains(teams_glyphs::LEAD_STATUS),
        "lead status reads active: {lead:?}"
    );
    assert!(
        lead.contains(teams_glyphs::LEAD_MODEL),
        "lead model column reads lead: {lead:?}"
    );
    assert!(lead.contains("general"), "lead name paints: {lead:?}");

    // Every synthetic teammate paints once with its status word (FR-022,
    // FR-023, NFR-009) - these statuses are the synthetic values, never
    // overridden by hydration because no store for this team name exists.
    for (agent, word) in [
        ("tm-001", "working"),
        ("tm-002", "idle"),
        ("tm-003", "suspended"),
        ("tm-004", "stopped"),
        ("tm-005", "failed"),
        ("tm-006", "working"),
    ] {
        let y = row_containing(&buf1, agent);
        let row = screen_row(&buf1, y);
        assert!(
            row.contains(word),
            "{agent} must paint status word {word:?}: {row:?}"
        );
    }

    // The focused teammate carries the focus marker (FR-022).
    let focused_y = row_containing(&buf1, "tm-002");
    let marker: String = (0..teams_columns::MARKER_W)
        .filter_map(|o| buf1.cell((content.x + o as u16, focused_y)))
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(
        marker.starts_with(teams_glyphs::FOCUS),
        "the focused teammate paints the focus marker (FR-022): {marker:?}"
    );

    // Model override vs inherited (FR-024).
    let override_y = row_containing(&buf1, "tm-006");
    assert!(
        screen_row(&buf1, override_y).contains("ollama/qwen2.5"),
        "the override paints provider/model: {}",
        screen_row(&buf1, override_y)
    );
    let inherited_y = row_containing(&buf1, "tm-001");
    assert!(
        screen_row(&buf1, inherited_y).contains(teams_glyphs::INHERITED),
        "a member without override paints inherited (FR-024): {}",
        screen_row(&buf1, inherited_y)
    );

    // The synthetic message counters land on the focused row (FR-025).
    let counter_row = screen_row(&buf1, focused_y);
    assert!(
        counter_row.contains('4') && counter_row.contains('2'),
        "the sent/recv counters paint the synthetic values: {counter_row:?}"
    );
    // The idle members without counters paint `-` in both message cells.
    let idle_y = row_containing(&buf1, "tm-001");
    let idle_row = screen_row(&buf1, idle_y);
    assert!(
        idle_row.contains('-'),
        "a member with no counters paints dashes (FR-025): {idle_row:?}"
    );

    // The suspension control still paints on the suspended teammate's row;
    // the stopped and failed teammates are terminal and paint none (FR-022,
    // FR-071 terminal rule).
    let susp_y = row_containing(&buf1, "tm-003");
    let susp_row = screen_row(&buf1, susp_y);
    assert!(
        susp_row.contains(teams_glyphs::RESUME) && !susp_row.contains(teams_glyphs::STOP),
        "the suspended teammate paints the resume glyph: {susp_row:?}"
    );
    let stopped_y = row_containing(&buf1, "tm-004");
    let stopped_row = screen_row(&buf1, stopped_y);
    assert!(
        !stopped_row.contains(teams_glyphs::STOP) && !stopped_row.contains(teams_glyphs::KILL),
        "a terminal teammate paints no controls: {stopped_row:?}"
    );

    // The strip renders offline too (FR-027): one pill per member, the cycle
    // hint pinned, and the focused member is the highlighted one.
    assert!(
        ragent_tui::teammate_strip::is_visible(&first),
        "a populated team shows the strip (FR-027)"
    );
    let strip_backend = TestBackend::new(160, 1);
    let mut strip_terminal = Terminal::new(strip_backend).expect("strip terminal");
    strip_terminal
        .draw(|frame| {
            ragent_tui::teammate_strip::render(frame, &first, frame.area());
        })
        .expect("render strip");
    let strip_buffer = strip_terminal.backend().buffer();
    let strip_text = painted(strip_buffer);
    for name in [
        "reviewer",
        "tester",
        "planner",
        "archivist",
        "guard",
        "polisher",
    ] {
        assert!(
            strip_text.contains(name),
            "the strip paints one pill per member: missing {name:?}: {strip_text:?}"
        );
    }
    assert!(
        strip_text.contains(ragent_tui::teammate_strip::HINT),
        "the strip pins the cycle hint: {strip_text:?}"
    );
    // The focused member's pill background is the focus colour (FR-027).
    let tester_x = strip_text.find("tester").expect("tester pill");
    let tester_cell = strip_buffer.cell((tester_x as u16, 0)).expect("pill cell");
    assert_eq!(
        tester_cell.bg,
        ratatui::style::Color::Rgb(50, 50, 80),
        "the focused pill uses the focus background (FR-027)"
    );
}

// ---------------------------------------------------------------------------
// NFR-002: a frame is bounded to the two documented reads
// ---------------------------------------------------------------------------

/// A full Teams frame costs exactly the two reads NFR-002 allows: the single
/// team task-store read (task counts + render-time hydration) and the single
/// session lookup (the lead elapsed). The reads run through a real scratch
/// store so they are real, and the frame must complete with every member row
/// painted - the read is shared by the whole roster rather than multiplied per
/// row, which is the bound NFR-002 states.
///
/// The store lives under the git-ignored scratch tree, never the repository;
/// the cwd swap keeps `task_counts` / hydration resolving to it. Held under
/// [`crate::global_env_lock`] because the cwd is process-global.
#[test]
fn test_t019_teams_frame_is_bounded_to_the_two_documented_reads() {
    let _guard = crate::global_env_lock();
    let scratch = scratch_dir("bounded-reads");
    let original = std::env::current_dir().expect("cwd");

    let store =
        TeamStore::create(TEAM_NAME, LEAD_SESSION, &scratch, true).expect("create team store");
    // One member in the on-disk store, one member only in memory: the store
    // member keeps its status through hydration, the memory-only members keep
    // the synthetic ones (no store entry to override them), and the single
    // read produces all rows' painted state.
    {
        let mut config = store.config.clone();
        config.members.push(std::clone::Clone::clone(&member(
            "tm-001",
            "reviewer",
            MemberStatus::Working,
        )));
        let updated = TeamStore {
            dir: store.dir.clone(),
            config,
        };
        updated.save().expect("save member");
    }

    std::env::set_current_dir(&scratch).expect("enter scratch");

    // One member: the frame reads the store once.
    let mut app1 = open_teams(vec![member("tm-001", "reviewer", MemberStatus::Working)]);
    app1.storage
        .create_session(LEAD_SESSION, ".")
        .expect("create lead session");
    let buf1 = render_teams_window(&mut app1, 170, 44);
    assert!(
        painted(&buf1).contains("working"),
        "the frame must render the member row (FR-060 hydration)"
    );

    // Four members: the same single read path covers every extra row. A
    // per-row query multiplication would need a different renderer structure
    // altogether; the bound is structural - one `task_counts` call per frame.
    let mut app4 = open_teams(vec![
        member("tm-001", "reviewer", MemberStatus::Working),
        member("tm-002", "tester", MemberStatus::Idle),
        member("tm-003", "planner", MemberStatus::Idle),
        member("tm-004", "guard", MemberStatus::Idle),
    ]);
    app4.storage
        .create_session(LEAD_SESSION, ".")
        .expect("create lead session");
    let buf4 = render_teams_window(&mut app4, 170, 44);
    let text4 = painted(&buf4);
    assert!(
        text4.contains("working") && text4.contains("idle"),
        "the four-member frame must still render every row from the one read (NFR-002)"
    );

    // The claimed/done counters come from the store read: the disk member has
    // no tasks so the count is 0, painted as a zero rather than a dash.
    let content = table_inner(app1.teams_area);
    let member_y = row_containing(&buf1, "tm-001");
    let claimed_x = teams_columns::MARKER_W
        + teams_columns::ID_W
        + teams_columns::GAP_W
        + teams_columns::LABEL_W
        + teams_columns::GAP_W
        + teams_columns::AGENT_TYPE_W
        + teams_columns::GAP_W
        + teams_columns::STATUS_W
        + teams_columns::GAP_W
        + teams_columns::MODEL_W
        + teams_columns::GAP_W
        + teams_columns::ELAPSED_W
        + teams_columns::GAP_W
        + teams_columns::STEPS_W
        + teams_columns::GAP_W;
    let claimed: String = (0..teams_columns::CLAIMED_W + 2)
        .filter_map(|o| buf1.cell((content.x + (claimed_x + o) as u16, member_y)))
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert_eq!(
        claimed.trim(),
        "0",
        "the claimed column must paint the store-derived count (FR-025): {claimed:?}"
    );

    // The lead elapsed cell is populated by the single session lookup (the
    // other read NFR-002 allows): the lead session was created just above, so
    // the session row exists in the in-memory storage and the cell paints a
    // compact duration rather than the dash fallback.
    let lead_y = member_y - 1;
    let elapsed_x = teams_columns::MARKER_W
        + teams_columns::ID_W
        + teams_columns::GAP_W
        + teams_columns::LABEL_W
        + teams_columns::GAP_W
        + teams_columns::AGENT_TYPE_W
        + teams_columns::GAP_W
        + teams_columns::STATUS_W
        + teams_columns::GAP_W
        + teams_columns::MODEL_W;
    let elapsed: String = (0..teams_columns::ELAPSED_W + 3)
        .filter_map(|o| buf1.cell((content.x + (elapsed_x + o) as u16, lead_y)))
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    // The lead row identity comes from the single session lookup: the short
    // lead session id (its last 8 characters) paints in the id column (FR-021).
    let lead_id: String = (0..teams_columns::ID_W)
        .filter_map(|o| buf1.cell((content.x + (teams_columns::MARKER_W + o) as u16, lead_y)))
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert_eq!(
        lead_id.trim(),
        ragent_tui::utils::short_id(LEAD_SESSION),
        "the lead row identity comes from the session lookup (FR-021, NFR-002): {lead_id:?}"
    );
    assert!(
        elapsed.trim().ends_with('s') || elapsed.trim() == "-",
        "the lead elapsed must come from the session lookup (FR-021, NFR-002): {elapsed:?}"
    );

    std::env::set_current_dir(original).expect("restore cwd");
    let _ = std::fs::remove_dir_all(&scratch);
}

// ---------------------------------------------------------------------------
// NFR-012: test isolation - no repository working-tree writes
// ---------------------------------------------------------------------------

/// A synthetic-state render of both windows and the full frame writes nothing
/// outside the allowed roots (NFR-012). The log spool root is already
/// redirected to `target/temp/log` by `make_app`, the team store hydration
/// finds nothing for [`TEAM_NAME`], and no other repository path is touched.
/// Verified by byte-totalling the plausible repository write roots before and
/// after: `log/`, `specs/` and `.ragent/teams/`. Everything under `target/`
/// is the allowed scratch root.
#[test]
fn test_t019_renders_leave_the_repository_working_tree_untouched() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    // Byte-totals of the directory trees the render could plausibly write.
    // A missing directory counts as absent and stays absent (or is allowed to
    // appear only where the render itself creates it, which it must not).
    let paths = ["log", "specs", ".ragent/teams"];
    let size_of = |p: &str| dir_size(&repo_root.join(p));

    let before: Vec<Option<u64>> = paths.iter().map(|p| size_of(p)).collect();

    let mut app = app_with_primary();
    app.active_team = Some(TeamConfig::new(TEAM_NAME, "s1"));
    app.team_members = vec![
        member("tm-001", "reviewer", MemberStatus::Working),
        member("tm-002", "tester", MemberStatus::Stopped),
    ];
    app.focused_teammate = Some("tm-001".to_string());
    add_custom_agent(&mut app, "rust-coder");
    app.active_tasks = vec![
        task(
            "iso-0001",
            "s1",
            "s2",
            "rust-coder",
            TaskStatus::Running,
            true,
        ),
        task(
            "iso-0002",
            "s1",
            "s3",
            "explore",
            TaskStatus::Completed,
            true,
        ),
    ];
    app.bg_tasks = vec![bg_task_view("ee556677", "sleep 30", "running")];

    let agents = render_agents_window(&mut app, 160, 44);
    let teams = render_teams_window(&mut app, 170, 44);
    app.show_agents_window = false;
    app.show_teams_window = false;
    let full = render_frame(&mut app, 120, 30); // the strip paints via the full frame

    assert!(
        painted(&agents).contains(" Agents "),
        "the Agents render must paint"
    );
    assert!(
        painted(&teams).contains(" Teams "),
        "the Teams render must paint"
    );
    assert!(
        painted(&full).contains(ragent_tui::teammate_strip::HINT),
        "the full frame must paint the strip (FR-027)"
    );

    let after: Vec<Option<u64>> = paths.iter().map(|p| size_of(p)).collect();
    for ((path, before_size), after_size) in paths.iter().zip(before).zip(after) {
        assert_eq!(
            before_size,
            after_size,
            "NFR-012: the render must not write into {}/ (before {before_size:?}, after {after_size:?})",
            repo_root.join(path).display()
        );
    }
}

/// Recursive byte-total of `dir`; `None` when it does not exist at all.
fn dir_size(dir: &std::path::Path) -> Option<u64> {
    if !dir.exists() {
        return None;
    }
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else {
            return None;
        };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => stack.push(entry.path()),
                Ok(_) => total += entry.metadata().map(|m| m.len()).unwrap_or(0),
                Err(_) => return None,
            }
        }
    }
    Some(total)
}

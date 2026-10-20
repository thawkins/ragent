//! Tests for the Agents table body (spec `agentui` T-003).
//!
//! Covers the column header (FR-009), the primary row (FR-010), the
//! ancestor-indented sub-agent tree (FR-011), sub-agent row content (FR-012),
//! the `[C]` / `[T]` badges and their derivation (FR-013, FR-062), the status
//! presentation (FR-014), the pinned `[stop]` / `[x]` control columns (FR-015),
//! the background shell section (FR-016), the single glyph source (NFR-003) and
//! character-safe truncation (NFR-004).
//!
//! Every case drives the real public render entry point through a ratatui
//! `TestBackend` and reads the painted buffer back, so no renderer logic is
//! re-implemented here (NFR-001).

mod support;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use ragent_agent::agent::oasf::OasfAgentRecord;
use ragent_agent::agent::{AgentInfo, CustomAgentDef};
use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{MemberStatus, TeamMember};
use ragent_tui::App;
use ragent_tui::layout_active_agents::{columns, glyphs};
use support::make_app;

use std::path::PathBuf;
use std::sync::Arc;

/// Render `app` with the Agents window open and return the drawn buffer.
fn render(app: &mut App, width: u16, height: u16) -> Buffer {
    app.show_agents_window = true;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
    terminal.backend().buffer().clone()
}

/// Build a [`TaskEntry`] for the Agents table.
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
    }
}

/// Build a background shell task view.
fn bg_task(id: &str, command: &str, status: &str) -> ragent_tui::app::BgTaskView {
    ragent_tui::app::BgTaskView {
        id: id.to_string(),
        session_id: "s1".to_string(),
        command: command.to_string(),
        status: status.to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    }
}

/// An app whose primary session is `s1` and whose agent is `general`.
fn app_with_primary() -> App {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.agent_name = "general".to_string();
    app
}

/// Flatten one painted row of `area` into a string.
fn row_text(buffer: &Buffer, area: Rect, offset: u16) -> String {
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

/// The painted style of the cell at column `col` (`0`-based, relative to the
/// table area) of row `offset` inside `area`.
fn cell_at(buffer: &Buffer, area: Rect, offset: u16, col: u16) -> ratatui::buffer::Cell {
    buffer
        .cell((area.x.saturating_add(col), area.y.saturating_add(offset)))
        .cloned()
        .unwrap_or_else(|| panic!("cell ({col}, {offset}) outside the buffer"))
}

/// The painted foreground colour at column `col` of row `offset`.
fn fg_at(buffer: &Buffer, area: Rect, offset: u16, col: u16) -> Color {
    cell_at(buffer, area, offset, col).fg
}

/// The index of `needle` inside the first `area.width` columns of row `offset`,
/// if the row paints it.
fn find_in_row(buffer: &Buffer, area: Rect, offset: u16, needle: &str) -> Option<usize> {
    row_text(buffer, area, offset).find(needle)
}

/// The index of the first painted row (inside the table area) that contains
/// `needle`.
fn find_row(buffer: &Buffer, area: Rect, needle: &str) -> Option<u16> {
    (0..area.height).find(|offset| find_in_row(buffer, area, *offset, needle).is_some())
}

/// Register a custom agent definition named `name` on `app`.
fn add_custom_agent(app: &mut App, name: &str) {
    app.custom_agent_defs.push(CustomAgentDef {
        record: OasfAgentRecord {
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
        agent_info: Arc::new(AgentInfo::new(name, "custom agent")),
        is_project_local: true,
    });
}

// -- FR-009: header ---------------------------------------------------------

/// The first line of the table is a dimmed header naming every column in the
/// documented left-to-right order (FR-009).
#[test]
fn test_agents_header_names_every_column_dimmed_and_in_order() {
    let mut app = app_with_primary();
    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;
    let header = row_text(&buffer, area, 0);

    let columns_in_order = [
        "id",
        "name",
        "type",
        "elapsed",
        "steps",
        glyphs::STOP,
        glyphs::KILL,
    ];
    let mut last = 0usize;
    for column in columns_in_order {
        let at = header
            .find(column)
            .unwrap_or_else(|| panic!("{column:?} missing from the header row: {header:?}"));
        assert!(
            at >= last,
            "{column:?} is out of order in the header row: {header:?}"
        );
        last = at;
    }

    // The header is dimmed hint-coloured, not normal row text.
    let hint = ragent_tui::theme::colors::HINT;
    for column in columns_in_order {
        let at = header.find(column).expect("column present") as u16;
        let cell = cell_at(&buffer, area, 0, at);
        assert_eq!(cell.fg, hint, "{column:?} must use the hint colour");
        assert!(
            cell.modifier.contains(Modifier::DIM),
            "{column:?} must be dimmed: {cell:?}"
        );
    }
}

// -- FR-010: primary row ----------------------------------------------------

/// Row 1 is the primary agent: a green `*`, the literal `lead`, the agent name
/// in bold green, `primary`, a `-` elapsed value and the live tool-call count,
/// with no action controls (FR-010).
#[test]
fn test_primary_row_carries_marker_identity_and_live_step_count() {
    let mut app = app_with_primary();
    app.event_bus.increment_tool_calls("s1");
    app.event_bus.increment_tool_calls("s1");
    app.event_bus.increment_tool_calls("s1");
    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;
    let row = row_text(&buffer, area, 1);

    assert!(
        row.starts_with('*'),
        "the primary marker is a green `*`: {row:?}"
    );
    assert_eq!(
        cell_at(&buffer, area, 1, 0).fg,
        Color::Green,
        "the primary marker must be green"
    );
    assert_eq!(
        row.find("lead"),
        Some(columns::MARKER_W),
        "the id column reads the literal `lead`: {row:?}"
    );

    let name_col = (columns::MARKER_W + columns::ID_W + columns::GAP_W) as u16;
    let name_cell = cell_at(&buffer, area, 1, name_col);
    assert_eq!(name_cell.fg, Color::Green, "the agent name is green");
    assert!(
        name_cell.modifier.contains(Modifier::BOLD),
        "the agent name is bold: {name_cell:?}"
    );
    assert!(
        row[name_col as usize..].starts_with("general"),
        "the configured agent name paints in the name column: {row:?}"
    );

    let type_x = columns::MARKER_W + columns::ID_W + columns::GAP_W + columns::LABEL_W;
    let elapsed_x = type_x + columns::TYPE_W;
    let steps_x = elapsed_x + columns::ELAPSED_W + columns::GAP_W;
    assert_eq!(
        row.get(type_x..elapsed_x).map(str::trim),
        Some("primary"),
        "the type column reads the literal `primary`: {row:?}"
    );
    assert_eq!(
        row.get(elapsed_x..elapsed_x + columns::ELAPSED_W)
            .map(str::trim),
        Some("-"),
        "the elapsed column reads `-`: {row:?}"
    );
    assert_eq!(
        row.get(steps_x..steps_x + columns::STEPS_W).map(str::trim),
        Some("3"),
        "the steps column paints the primary session's live tool-call count: {row:?}"
    );
    assert!(
        !row.contains(glyphs::STOP) && !row.contains(glyphs::KILL),
        "the primary row carries no action controls: {row:?}"
    );
}

// -- FR-011: tree indentation ----------------------------------------------

/// Each sub-agent row is indented by one two-column segment per ancestor and
/// prefixed with the last-child or non-last-child branch glyph (FR-011).
#[test]
fn test_subagent_rows_indent_two_columns_per_ancestor() {
    let mut app = app_with_primary();
    // Two siblings under the primary session, then a grandchild under the
    // first sibling and a great-grandchild under that.
    app.active_tasks.push(task(
        "alpha-0001",
        "s1",
        "s2",
        "explore",
        TaskStatus::Running,
        true,
    ));
    app.active_tasks.push(task(
        "beta-0002",
        "s1",
        "s3",
        "build",
        TaskStatus::Running,
        true,
    ));
    app.active_tasks.push(task(
        "gamma-0003",
        "s2",
        "s4",
        "explore",
        TaskStatus::Running,
        true,
    ));
    app.active_tasks.push(task(
        "delta-0004",
        "s4",
        "s5",
        "explore",
        TaskStatus::Running,
        true,
    ));

    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;
    let label_x = (columns::MARKER_W + columns::ID_W + columns::GAP_W) as u16;

    // Depth 0 siblings: the non-last one keeps the branch open, the last one
    // closes it.
    let alpha = find_row(&buffer, area, "alpha-0001").expect("first sibling paints");
    let beta = find_row(&buffer, area, "beta-0002").expect("second sibling paints");
    let alpha_row = row_text(&buffer, area, alpha);
    let beta_row = row_text(&buffer, area, beta);
    assert!(
        alpha_row[label_x as usize..].starts_with(glyphs::BRANCH_MID),
        "a non-last child paints the mid branch glyph: {alpha_row:?}"
    );
    assert!(
        beta_row[label_x as usize..].starts_with(glyphs::BRANCH_LAST),
        "the last child paints the closing branch glyph: {beta_row:?}"
    );

    // Depth 1 under a non-last parent: one trunk segment then the branch glyph.
    let gamma = find_row(&buffer, area, "gamma-0003").expect("grandchild paints");
    let gamma_row = row_text(&buffer, area, gamma);
    let gamma_label = &gamma_row[label_x as usize..];
    assert!(
        gamma_label.starts_with(glyphs::TRUNK),
        "the grandchild keeps the parent trunk segment: {gamma_row:?}"
    );
    assert!(
        gamma_label[glyphs::TRUNK.len()..].starts_with(glyphs::BRANCH_LAST),
        "the grandchild draws its own branch glyph after the trunk: {gamma_row:?}"
    );

    // Depth 2: the non-last grandparent keeps its trunk, the last-child parent
    // contributes a blank segment, then the child's own branch glyph.
    let delta = find_row(&buffer, area, "delta-0004").expect("great-grandchild paints");
    let delta_row = row_text(&buffer, area, delta);
    let delta_label = &delta_row[label_x as usize..];
    assert!(
        delta_label.starts_with(glyphs::TRUNK),
        "the non-last grandparent keeps its trunk segment: {delta_row:?}"
    );
    assert!(
        delta_label[glyphs::TRUNK.len()..].starts_with(glyphs::TRUNK_BLANK),
        "a last-child ancestor contributes a blank two-column segment: {delta_row:?}"
    );
    assert!(
        delta_label[2 * glyphs::TRUNK.len()..].starts_with(glyphs::BRANCH_LAST),
        "the great-grandchild draws its own branch glyph: {delta_row:?}"
    );

    // One two-column segment per ancestor: depth 2 starts its glyphs 4 columns
    // in, depth 1 starts 2 columns in.
    assert_eq!(
        ancestor_segment_width(&alpha_row, label_x),
        0,
        "a depth-0 row carries no ancestor segment: {alpha_row:?}"
    );
    assert_eq!(
        ancestor_segment_width(&gamma_row, label_x),
        glyphs::TRUNK.len(),
        "a depth-1 row carries exactly one ancestor segment: {gamma_row:?}"
    );
    assert_eq!(
        ancestor_segment_width(&delta_row, label_x),
        2 * glyphs::TRUNK.len(),
        "a depth-2 row carries exactly two ancestor segments: {delta_row:?}"
    );
}

/// The number of two-column ancestor segments painted before the branch glyph.
fn ancestor_segment_width(row: &str, label_x: u16) -> usize {
    let label = &row[label_x as usize..];
    let mut offset = 0usize;
    let mut rest = label;
    while rest.starts_with(glyphs::TRUNK) || rest.starts_with(glyphs::TRUNK_BLANK) {
        let step = if rest.starts_with(glyphs::TRUNK) {
            glyphs::TRUNK.len()
        } else {
            glyphs::TRUNK_BLANK.len()
        };
        rest = &rest[step..];
        offset += step;
    }
    offset
}

// -- FR-012: sub-agent row content ----------------------------------------

/// A sub-agent row paints a status dot, the last 8 characters of the task id,
/// the agent name (at most 32 columns), `bg` / `fg`, a compact elapsed time and
/// the child session's live tool-call count (FR-012).
#[test]
fn test_subagent_row_paints_dot_short_id_label_type_elapsed_and_steps() {
    let mut app = app_with_primary();
    app.active_tasks.push(task(
        "zzzzzzzz-explore-abcd1234",
        "s1",
        "s2",
        "explore",
        TaskStatus::Running,
        true,
    ));
    app.active_tasks.push(task(
        "zzzzzzzz-planner-efgh5678",
        "s1",
        "s3",
        "planner",
        TaskStatus::Running,
        false,
    ));
    app.event_bus.increment_tool_calls("s2");
    app.event_bus.increment_tool_calls("s2");
    app.event_bus.increment_tool_calls("s2");
    app.event_bus.increment_tool_calls("s2");

    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;

    let id_x = columns::MARKER_W as u16;
    let label_x = (columns::MARKER_W + columns::ID_W + columns::GAP_W) as u16;
    let type_x = label_x + columns::LABEL_W as u16;
    let elapsed_x = type_x + columns::TYPE_W as u16;
    let steps_x = elapsed_x + (columns::ELAPSED_W + columns::GAP_W) as u16;

    let bg_row = find_row(&buffer, area, "abcd1234").expect("the bg row paints the short id");
    let bg_text = row_text(&buffer, area, bg_row);
    assert!(
        bg_text.starts_with(glyphs::ROW_DOT),
        "a sub-agent row starts with the status dot: {bg_text:?}"
    );
    assert_eq!(
        fg_at(&buffer, area, bg_row, 0),
        Color::Yellow,
        "a running row's dot is yellow"
    );
    assert!(
        bg_text[id_x as usize..].starts_with("abcd1234"),
        "the id column paints the last 8 characters of the task id: {bg_text:?}"
    );
    assert!(
        !bg_text[id_x as usize..label_x as usize].contains("zzzzzzzz"),
        "the id column never paints the full task id: {bg_text:?}"
    );

    let steps_cell = row_text(&buffer, area, bg_row);
    let steps_slice: String = steps_cell
        .chars()
        .skip(steps_x as usize)
        .take(columns::STEPS_W)
        .collect();
    assert_eq!(
        steps_slice.trim(),
        "4",
        "the steps column paints the child session's live tool-call count: {bg_text:?}"
    );

    let elapsed_slice: String = steps_cell
        .chars()
        .skip(elapsed_x as usize)
        .take(columns::ELAPSED_W)
        .collect();
    assert!(
        elapsed_slice.trim().ends_with('s'),
        "the elapsed column paints a compact duration: {bg_text:?}"
    );

    // The type column reads `bg` for a background task and `fg` otherwise.
    let type_slice: String = bg_text
        .chars()
        .skip(type_x as usize)
        .take(columns::TYPE_W)
        .collect();
    assert_eq!(type_slice.trim(), "bg", "a background task reads `bg`");
    let fg_row = find_row(&buffer, area, "efgh5678").expect("the fg row paints");
    let fg_text = row_text(&buffer, area, fg_row);
    let fg_type: String = fg_text
        .chars()
        .skip(type_x as usize)
        .take(columns::TYPE_W)
        .collect();
    assert_eq!(fg_type.trim(), "fg", "a foreground task reads `fg`");
}

/// The agent-label column never exceeds 32 display columns, whatever badges and
/// a long identifier are appended, so the type column is never pushed out
/// (FR-012, FR-013).
#[test]
fn test_agent_label_never_exceeds_thirty_two_columns() {
    let long_agent = "a-very-long-custom-agent-name-for-testing";
    let mut app = app_with_primary();
    add_custom_agent(&mut app, long_agent);
    app.active_tasks.push(task(
        "a-very-long-custom-agent-name-for-testing-0123456789abcdef",
        "s1",
        "s2",
        long_agent,
        TaskStatus::Suspended,
        true,
    ));
    let mut member = TeamMember::new("watcher", "tm-001", long_agent);
    member.session_id = Some("s2".to_string());
    member.status = MemberStatus::Working;
    app.team_members.push(member);

    let buffer = render(&mut app, 200, 50);
    let area = app.active_agents_area;
    let label_x = columns::MARKER_W + columns::ID_W + columns::GAP_W;
    let type_x = label_x + columns::LABEL_W;

    // Every non-header row is checked: the label slot is exactly the documented
    // width, so nothing can spill past it.
    let row = (1..area.height)
        .find(|offset| {
            let text = row_text(&buffer, area, *offset);
            text.contains(glyphs::BADGE_CUSTOM) && text.contains(glyphs::BADGE_TEAM)
        })
        .unwrap_or_else(|| panic!("the badly-labelled row must paint both badges"));
    let text = row_text(&buffer, area, row);
    let label: String = text.chars().skip(label_x).take(columns::LABEL_W).collect();
    assert!(
        label.contains(glyphs::BADGE_CUSTOM) && label.contains(glyphs::BADGE_TEAM),
        "both badges stay inside the 32-column label: {label:?}"
    );
    assert!(
        label.contains(glyphs::STATUS_SUSPENDED.trim()),
        "the status suffix also stays inside the label: {label:?}"
    );
    let type_slice: String = text.chars().skip(type_x).take(columns::TYPE_W).collect();
    assert_eq!(
        type_slice.trim(),
        "bg",
        "the type column is not pushed out by an over-long label: {text:?}"
    );
    let stop_at = find_in_row(&buffer, area, row, glyphs::STOP);
    assert_eq!(
        stop_at, None,
        "a suspended row paints the resume glyph, not [stop]: {text:?}"
    );
    assert_eq!(
        find_in_row(&buffer, area, row, glyphs::KILL),
        Some(columns::KILL_X),
        "the control column keeps its pinned position: {text:?}"
    );
}

// -- FR-013 / FR-062: badges ----------------------------------------------

/// The `[C]` badge follows the loaded custom agent definitions and the `[T]`
/// badge follows the current members' sessions (FR-013, FR-062).
#[test]
fn test_badges_follow_the_current_custom_agents_and_members() {
    let mut app = app_with_primary();
    app.active_tasks.push(task(
        "first-0001",
        "s1",
        "s2",
        "explore",
        TaskStatus::Running,
        true,
    ));
    app.active_tasks.push(task(
        "second-0002",
        "s1",
        "s3",
        "explore",
        TaskStatus::Running,
        true,
    ));

    // No custom definitions and no team: no badges paint.
    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;
    let row = find_row(&buffer, area, "first-0001").expect("row paints");
    let plain = row_text(&buffer, area, row);
    assert!(
        !plain.contains(glyphs::BADGE_CUSTOM) && !plain.contains(glyphs::BADGE_TEAM),
        "no badge without a custom agent or a team: {plain:?}"
    );

    // A custom definition named `explore` badges both rows with `[C]`.
    add_custom_agent(&mut app, "explore");
    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;
    let row = find_row(&buffer, area, "first-0001").expect("row paints");
    let custom = row_text(&buffer, area, row);
    assert!(
        custom.contains(glyphs::BADGE_CUSTOM),
        "a row whose agent is a custom definition carries [C]: {custom:?}"
    );
    assert!(
        !custom.contains(glyphs::BADGE_TEAM),
        "no teammate badge without a team member: {custom:?}"
    );

    // A member whose session is the second task's child session badges only
    // that row, proving the set is re-derived from the members (FR-062).
    let mut member = TeamMember::new("reviewer", "tm-001", "explore");
    member.session_id = Some("s3".to_string());
    member.status = MemberStatus::Working;
    app.team_members.push(member);
    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;
    let first = find_row(&buffer, area, "first-0001").expect("first row paints");
    let second = find_row(&buffer, area, "second-0002").expect("second row paints");
    let first_text = row_text(&buffer, area, first);
    let second_text = row_text(&buffer, area, second);
    assert!(
        first_text.contains(glyphs::BADGE_CUSTOM) && !first_text.contains(glyphs::BADGE_TEAM),
        "the non-member row carries only [C]: {first_text:?}"
    );
    assert!(
        second_text.contains(glyphs::BADGE_CUSTOM) && second_text.contains(glyphs::BADGE_TEAM),
        "the member row carries both [C] and [T]: {second_text:?}"
    );

    // Dropping the member removes the badge again.
    app.team_members.clear();
    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;
    let second = find_row(&buffer, area, "second-0002").expect("second row paints");
    assert!(
        !row_text(&buffer, area, second).contains(glyphs::BADGE_TEAM),
        "the [T] badge tracks the live member list"
    );
}

// -- FR-014: status presentation ------------------------------------------

/// Running rows are yellow, suspended rows dark grey with a ` [||]` suffix and
/// terminating rows red with a ` ...` suffix (FR-014).
#[test]
fn test_row_status_colours_and_suffixes() {
    let mut app = app_with_primary();
    app.active_tasks.push(task(
        "run-0001",
        "s1",
        "s2",
        "explore",
        TaskStatus::Running,
        true,
    ));
    app.active_tasks.push(task(
        "susp-0002",
        "s1",
        "s3",
        "explore",
        TaskStatus::Suspended,
        true,
    ));
    app.active_tasks.push(task(
        "term-0003",
        "s1",
        "s4",
        "explore",
        TaskStatus::Terminating,
        true,
    ));

    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;
    let label_x = (columns::MARKER_W + columns::ID_W + columns::GAP_W) as u16;

    let running = find_row(&buffer, area, "run-0001").expect("running row paints");
    assert_eq!(
        fg_at(&buffer, area, running, label_x),
        Color::Yellow,
        "a running row's label is yellow"
    );
    let running_text = row_text(&buffer, area, running);
    assert!(
        !running_text.contains(glyphs::STATUS_SUSPENDED)
            && !running_text.contains(glyphs::STATUS_TERMINATING),
        "a running row carries no status suffix: {running_text:?}"
    );

    let suspended = find_row(&buffer, area, "susp-0002").expect("suspended row paints");
    assert_eq!(
        fg_at(&buffer, area, suspended, label_x),
        Color::DarkGray,
        "a suspended row's label is dark grey"
    );
    let suspended_text = row_text(&buffer, area, suspended);
    assert!(
        suspended_text.contains(glyphs::STATUS_SUSPENDED),
        "a suspended row paints the ` [||]` suffix: {suspended_text:?}"
    );

    let terminating = find_row(&buffer, area, "term-0003").expect("terminating row paints");
    assert_eq!(
        fg_at(&buffer, area, terminating, label_x),
        Color::Red,
        "a terminating row's label is red"
    );
    let terminating_text = row_text(&buffer, area, terminating);
    assert!(
        terminating_text.contains(glyphs::STATUS_TERMINATING),
        "a terminating row paints the ` ...` suffix: {terminating_text:?}"
    );
}

// -- FR-015: control columns ----------------------------------------------

/// A live row paints the six-column `[stop]` control at column 70 and the
/// four-column `[x]` control at column 77 of the table area, for the agent rows
/// and the background shell rows alike (FR-015).
#[test]
fn test_live_rows_pin_stop_and_kill_to_the_documented_columns() {
    let mut app = app_with_primary();
    app.active_tasks.push(task(
        "run-0001",
        "s1",
        "s2",
        "explore",
        TaskStatus::Running,
        true,
    ));
    app.bg_tasks.push(bg_task("bg-0001", "sleep 30", "running"));

    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;

    for needle in ["run-0001", "bg-0001"] {
        let row = find_row(&buffer, area, needle).expect("row paints");
        let text = row_text(&buffer, area, row);
        let stop_at = find_in_row(&buffer, area, row, glyphs::STOP)
            .unwrap_or_else(|| panic!("{needle}: the [stop] control must paint: {text:?}"));
        assert_eq!(
            stop_at,
            columns::STOP_X,
            "{needle}: [stop] must start at column {}: {text:?}",
            columns::STOP_X
        );
        let kill_at = find_in_row(&buffer, area, row, glyphs::KILL)
            .unwrap_or_else(|| panic!("{needle}: the [x] control must paint: {text:?}"));
        assert_eq!(
            kill_at,
            columns::KILL_X,
            "{needle}: [x] must start at column {}: {text:?}",
            columns::KILL_X
        );
    }
}

/// The recorded hit rectangles sit exactly on the painted controls, in the same
/// order as their identifier vector (FR-015, NFR-011).
#[test]
fn test_control_hit_rects_match_the_painted_controls() {
    let mut app = app_with_primary();
    app.active_tasks.push(task(
        "run-0001",
        "s1",
        "s2",
        "explore",
        TaskStatus::Running,
        true,
    ));
    app.active_tasks.push(task(
        "done-0002",
        "s1",
        "s3",
        "explore",
        TaskStatus::Completed,
        true,
    ));
    app.bg_tasks.push(bg_task("bg-0001", "sleep 30", "running"));
    app.bg_tasks.push(bg_task("bg-0002", "true", "completed"));

    let _ = render(&mut app, 160, 44);
    let area = app.active_agents_area;

    // Every action-capable painted row keeps one vector entry; terminal rows
    // record the zero-width `Rect::default()` placeholder with an empty
    // identifier (FR-071, NFR-011).
    assert_eq!(app.agent_row_button_areas.len(), 4);
    assert_eq!(app.agent_row_kill_areas.len(), 4);
    assert_eq!(
        app.agent_row_button_task_ids,
        vec![
            "run-0001".to_string(),
            String::new(),
            "bg-0001".to_string(),
            String::new(),
        ],
        "the identifiers stay index-aligned with the painted controls; a terminal row is a placeholder"
    );
    assert_eq!(
        app.agent_row_kill_task_ids, app.agent_row_button_task_ids,
        "every [x] rect stays index-aligned with its own identifier"
    );
    for (i, rect) in app.agent_row_button_areas.iter().enumerate() {
        if app.agent_row_button_task_ids[i].is_empty() {
            assert_eq!(
                *rect,
                Rect::default(),
                "a terminal row's entry is the inert placeholder (FR-071)"
            );
        } else {
            assert_eq!(
                rect.x,
                area.x + columns::STOP_X as u16,
                "the [stop] hit rect sits on the painted control"
            );
            assert_eq!(rect.width, columns::STOP_W as u16);
        }
    }
    for (i, rect) in app.agent_row_kill_areas.iter().enumerate() {
        if !app.agent_row_kill_task_ids[i].is_empty() {
            assert_eq!(
                rect.x,
                area.x + columns::KILL_X as u16,
                "the [x] hit rect sits on the painted control"
            );
            assert_eq!(rect.width, columns::KILL_W as u16);
        }
    }

    // A completed task paints no controls on its row.
    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;
    let done_row = find_row(&buffer, area, "done-0002").expect("completed row paints");
    let done_text = row_text(&buffer, area, done_row);
    assert!(
        !done_text.contains(glyphs::STOP) && !done_text.contains(glyphs::KILL),
        "a terminal row carries no action controls: {done_text:?}"
    );
}

/// A suspended row replaces the `[stop]` glyph with `>` without moving `[x]`
/// (FR-015).
#[test]
fn test_suspended_row_paints_the_resume_glyph_in_the_stop_column() {
    let mut app = app_with_primary();
    app.active_tasks.push(task(
        "susp-0001",
        "s1",
        "s2",
        "explore",
        TaskStatus::Suspended,
        true,
    ));
    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;
    let row = find_row(&buffer, area, "susp-0001").expect("row paints");
    let text = row_text(&buffer, area, row);

    assert!(
        !text.contains(glyphs::STOP),
        "a suspended row drops the [stop] label: {text:?}"
    );
    let cell = cell_at(&buffer, area, row, columns::STOP_X as u16);
    assert_eq!(
        cell.symbol(),
        glyphs::RESUME,
        "the resume glyph paints at the [stop] column: {text:?}"
    );
    assert_eq!(cell.fg, Color::Green, "the resume glyph is green");
    assert_eq!(
        find_in_row(&buffer, area, row, glyphs::KILL),
        Some(columns::KILL_X),
        "the [x] control stays at its pinned column: {text:?}"
    );
    assert_eq!(
        app.agent_row_button_areas[0].x,
        area.x + columns::STOP_X as u16,
        "the resume control keeps the [stop] hit rect"
    );
}

// -- FR-016: background shell section -------------------------------------

/// A ` Background shell tasks ` heading precedes one row per background task,
/// carrying a status dot, the short identifier, the command, `bg`, the elapsed
/// time and both controls while the task is live (FR-016).
#[test]
fn test_background_shell_section_lists_each_task_with_controls() {
    let mut app = app_with_primary();
    app.active_tasks.push(task(
        "run-0001",
        "s1",
        "s2",
        "explore",
        TaskStatus::Running,
        true,
    ));
    app.bg_tasks
        .push(bg_task("cc112233", "cargo build --release", "running"));
    app.bg_tasks.push(bg_task("dd445566", "ls", "completed"));

    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;
    let heading = find_row(&buffer, area, glyphs::BG_HEADING)
        .unwrap_or_else(|| panic!("the background shell heading must paint"));
    let agent_row = find_row(&buffer, area, "run-0001").expect("the agent row paints");
    assert!(
        heading > agent_row,
        "the background section follows the sub-agent rows: heading {heading}, agent row {agent_row}"
    );

    let running = find_row(&buffer, area, "cc112233").expect("the running bg row paints");
    assert_eq!(
        running,
        heading + 1,
        "the first background row sits directly under the heading"
    );
    let running_text = row_text(&buffer, area, running);
    assert!(
        running_text.starts_with(glyphs::ROW_DOT),
        "a background row starts with the status dot: {running_text:?}"
    );
    assert!(
        running_text.contains("cargo build --release"),
        "the row paints the command: {running_text:?}"
    );
    assert!(
        running_text.contains(glyphs::STOP) && running_text.contains(glyphs::KILL),
        "a live background row paints both controls: {running_text:?}"
    );
    let type_x = columns::MARKER_W + columns::ID_W + columns::GAP_W + columns::LABEL_W;
    let type_slice: String = running_text
        .chars()
        .skip(type_x)
        .take(columns::TYPE_W)
        .collect();
    assert_eq!(
        type_slice.trim(),
        "bg",
        "a background row reads `bg` in the type column: {running_text:?}"
    );

    // The completed row paints no controls.
    let completed = find_row(&buffer, area, "dd445566").expect("the completed bg row paints");
    let completed_text = row_text(&buffer, area, completed);
    assert!(
        !completed_text.contains(glyphs::STOP) && !completed_text.contains(glyphs::KILL),
        "a finished background row paints no controls: {completed_text:?}"
    );
    assert!(
        completed_text.contains("ls"),
        "the finished row still paints its command: {completed_text:?}"
    );
}

/// A long background command is truncated inside its column, on a character
/// boundary, leaving the type and control columns in place (FR-016, NFR-004).
#[test]
fn test_long_background_command_truncates_within_its_column() {
    let mut app = app_with_primary();
    let long_command = "cargo build --release --workspace --all-targets --features everything";
    assert!(long_command.chars().count() > columns::BG_COMMAND_MAX_W);
    app.bg_tasks
        .push(bg_task("ee778899", long_command, "running"));

    let buffer = render(&mut app, 200, 50);
    let area = app.active_agents_area;
    let row = find_row(&buffer, area, "ee778899").expect("the bg row paints");
    let text = row_text(&buffer, area, row);

    assert!(
        !text.contains(long_command),
        "an over-long command is never painted in full: {text:?}"
    );
    let label_x = columns::MARKER_W + columns::ID_W + columns::GAP_W;
    let command: String = text.chars().skip(label_x).take(columns::LABEL_W).collect();
    assert!(
        long_command.starts_with(command.trim_end_matches('.')),
        "the truncation keeps a prefix of the command: {command:?}"
    );
    assert!(
        command.chars().count() <= columns::LABEL_W.min(columns::BG_COMMAND_MAX_W),
        "the painted command stays inside its column budget: {command:?}"
    );
    let type_x = label_x + columns::LABEL_W;
    let type_slice: String = text.chars().skip(type_x).take(columns::TYPE_W).collect();
    assert_eq!(
        type_slice.trim(),
        "bg",
        "the type column survives an over-long command: {text:?}"
    );
    assert_eq!(
        find_in_row(&buffer, area, row, glyphs::STOP),
        Some(columns::STOP_X),
        "the [stop] control keeps its pinned column: {text:?}"
    );
}

// -- NFR-003: single glyph source ----------------------------------------

/// Every glyph the table paints comes from the module's glyph set, so a change
/// cannot leave the header and the rows disagreeing (NFR-003).
#[test]
fn test_painted_glyphs_come_from_the_module_glyph_set() {
    let mut app = app_with_primary();
    app.active_tasks.push(task(
        "run-0001",
        "s1",
        "s2",
        "explore",
        TaskStatus::Running,
        true,
    ));
    app.active_tasks.push(task(
        "susp-0002",
        "s1",
        "s3",
        "explore",
        TaskStatus::Suspended,
        true,
    ));
    app.active_tasks.push(task(
        "term-0003",
        "s1",
        "s4",
        "explore",
        TaskStatus::Terminating,
        true,
    ));
    app.bg_tasks.push(bg_task("bg-0001", "sleep 30", "running"));

    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;
    let painted: String = (0..area.height)
        .map(|offset| row_text(&buffer, area, offset))
        .collect::<Vec<_>>()
        .join("\n");

    for glyph in [
        glyphs::PRIMARY.trim_end(),
        glyphs::ROW_DOT.trim_end(),
        glyphs::BRANCH_LAST.trim_end(),
        glyphs::STOP,
        glyphs::KILL,
        glyphs::RESUME,
        glyphs::STATUS_SUSPENDED.trim(),
        glyphs::STATUS_TERMINATING.trim(),
        glyphs::BG_HEADING.trim(),
    ] {
        assert!(
            painted.contains(glyph),
            "the row set must paint the {glyph:?} glyph from the module glyph set"
        );
    }

    // The header and the rows must share the control glyphs verbatim.
    let header = row_text(&buffer, area, 0);
    assert!(
        header.contains(glyphs::STOP) && header.contains(glyphs::KILL),
        "the header names the control columns with the painted glyphs: {header:?}"
    );
}

// -- NFR-004: character-safe truncation ----------------------------------

/// A multi-byte identifier and command truncate on a character boundary without
/// panicking or splitting a sequence (NFR-004).
#[test]
fn test_multibyte_identifiers_truncate_on_a_character_boundary() {
    let mut app = app_with_primary();
    let wide_agent = "a\u{e9}\u{4e2d}\u{6587}".repeat(12);
    app.active_tasks.push(task(
        "task-\u{4e2d}\u{6587}\u{e9}",
        "s1",
        "s2",
        &wide_agent,
        TaskStatus::Running,
        true,
    ));
    app.bg_tasks.push(bg_task(
        "bg-\u{4e2d}\u{6587}",
        &"\u{4e2d}\u{6587}\u{e9}".repeat(40),
        "running",
    ));

    // The render must not panic and must stay inside the table area.
    let buffer = render(&mut app, 160, 44);
    let area = app.active_agents_area;
    for offset in 0..area.height {
        let text = row_text(&buffer, area, offset);
        assert!(
            !text.contains('\u{fffd}') && !text.contains('?'),
            "row {offset} must not paint a broken character: {text:?}"
        );
    }

    // The label column is still exactly `LABEL_W` wide, so nothing split.
    let label_x = columns::MARKER_W + columns::ID_W + columns::GAP_W;
    let row = find_row(&buffer, area, glyphs::BG_HEADING)
        .map_or(area.height - 1, |heading| heading.saturating_add(1));
    let text = row_text(&buffer, area, row);
    let label: String = text.chars().skip(label_x).take(columns::LABEL_W).collect();
    assert!(
        label.chars().count() <= columns::LABEL_W,
        "the label column stays within its budget: {label:?}"
    );
}

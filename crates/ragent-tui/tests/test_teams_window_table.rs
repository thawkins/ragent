//! Tests for the Teams table body (spec `agentui` T-005).
//!
//! Covers the lead row (FR-021), the teammate rows and their focus marker
//! (FR-022), the status vocabulary (FR-023, NFR-009), the model column with
//! global-model suppression (FR-024), the claimed/done/sent/recv counters
//! (FR-025), the empty state (FR-026) and character-safe truncation (NFR-004).
//!
//! Every case drives the real public render entry point through a ratatui
//! `TestBackend` and reads the painted buffer back, so no renderer logic is
//! re-implemented here (NFR-001, NFR-012).

mod support;

use std::path::PathBuf;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use ragent_agent::agent::ModelRef;
use ragent_agent::team::{MemberStatus, Task, TaskStatus, TeamConfig, TeamMember, TeamStore};
use ragent_tui::App;
use ragent_tui::layout_teams::{columns, glyphs};
use ragent_tui::theme::colors;
use support::make_app;

/// The session id every test uses for the lead row.
const LEAD_SESSION: &str = "lead-session-0001";

/// Start column of each left-aligned column, in table-relative display columns.
///
/// Derived from [`columns`] so a width edit moves these with the renderer
/// instead of silently invalidating the assertions.
mod col {
    use super::columns;

    const fn step(so_far: usize, width: usize) -> usize {
        so_far + width + columns::GAP_W
    }

    /// First column of the id cell.
    pub const ID: usize = columns::MARKER_W;
    /// First column of the label cell.
    pub const LABEL: usize = step(ID, columns::ID_W);
    /// First column of the agent-type cell.
    pub const AGENT_TYPE: usize = step(LABEL, columns::LABEL_W);
    /// First column of the status cell.
    pub const STATUS: usize = step(AGENT_TYPE, columns::AGENT_TYPE_W);
    /// First column of the model cell.
    pub const MODEL: usize = step(STATUS, columns::STATUS_W);
    /// First column of the elapsed cell.
    pub const ELAPSED: usize = step(MODEL, columns::MODEL_W);
    /// First column of the steps cell.
    pub const STEPS: usize = step(ELAPSED, columns::ELAPSED_W);
    /// First column of the claimed cell.
    pub const CLAIMED: usize = step(STEPS, columns::STEPS_W);
    /// First column of the done cell.
    pub const DONE: usize = step(CLAIMED, columns::CLAIMED_W);
    /// First column of the sent cell.
    pub const SENT: usize = step(DONE, columns::DONE_W);
    /// First column of the recv cell.
    pub const RECV: usize = step(SENT, columns::SENT_W);
}

/// An app with the Teams window open on team `alpha`, holding `members`.
fn open_teams(members: Vec<TeamMember>) -> App {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    app.team_members = members;
    app.show_teams_window = true;
    app
}

/// A teammate in `status` with a spawned session id.
fn member(name: &str, agent_id: &str, status: MemberStatus) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// Render `app` at `width` x `height` and return the drawn buffer.
fn render(app: &mut App, width: u16, height: u16) -> Buffer {
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

/// The first screen row containing every needle.
fn row_containing(buffer: &Buffer, needles: &[&str]) -> u16 {
    (0..buffer.area.height)
        .find(|&y| {
            let text = screen_row(buffer, y);
            needles.iter().all(|needle| text.contains(needle))
        })
        .unwrap_or_else(|| {
            let mut dump = String::new();
            for y in 0..buffer.area.height {
                dump.push_str(&format!("{y:>3}| {}\n", screen_row(buffer, y)));
            }
            panic!("no row contains {needles:?}. Screen:\n{dump}")
        })
}

/// Terminal width that leaves the whole pinned table, controls included,
/// inside the subpanel's border.
///
/// The control columns are pinned to absolute offsets (`KILL_X + KILL_W`
/// display columns), so a terminal narrower than that clips the last controls
/// against the border and a read at their documented column returns border
/// glyphs. Derived from [`columns`] and the modal frame's own margin so a
/// width edit moves this with the renderer.
fn full_table_width() -> u16 {
    // 21 columns of modal frame and table border sit outside the table's
    // inner content at the default layout (measured, not assumed). The extra
    // pair leaves a blank gutter between the last control and the border, so a
    // read of `KILL_W` columns never swallows a border glyph.
    const FRAME_MARGIN: u16 = 21;
    const BORDER_GUTTER: u16 = 2;
    let needed = u16::try_from(columns::KILL_X + columns::KILL_W).expect("control columns fit u16");
    needed + BORDER_GUTTER + FRAME_MARGIN
}

/// The paint area of the table rows: the content rect minus its own border.
///
/// The subpanel paints a bordered block inside the modal content area, so the
/// header and every data row start one column and one row inside it.
fn table_inner(content: Rect) -> Rect {
    Rect::new(
        content.x + 1,
        content.y + 1,
        content.width.saturating_sub(2),
        content.height.saturating_sub(2),
    )
}

/// The text painted in one table column of screen row `y`.
fn column_text(buffer: &Buffer, content: Rect, y: u16, start: usize, width: usize) -> String {
    let base = content.x as usize + start;
    let mut out = String::new();
    for offset in 0..width {
        match buffer.cell(((base + offset) as u16, y)) {
            Some(cell) => out.push_str(cell.symbol()),
            None => out.push('?'),
        }
    }
    out
}

/// The style of the first cell of `needle` inside screen row `y`.
fn style_at(buffer: &Buffer, y: u16, needle: &str) -> ratatui::style::Style {
    let text = screen_row(buffer, y);
    let offset = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not found on row {y}: {text:?}"));
    buffer
        .cell((offset as u16, y))
        .map(ratatui::buffer::Cell::style)
        .expect("cell exists")
}

/// A unique scratch directory under `target/temp/` (never inside the repo tree).
fn scratch_dir(tag: &str) -> PathBuf {
    let unique = format!(
        "{}-{}-{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/teams-window-table")
        .join(unique);
    std::fs::create_dir_all(dir.join(".ragent/teams")).expect("scratch dir");
    dir
}

// -- FR-021: the lead row ---------------------------------------------------

/// The lead row is the row after the header and carries a green `*`, the short
/// lead session id, the configured agent name, `active`, `lead`, the session's
/// elapsed time, the live tool-call count and `-` in claimed and done (FR-021).
#[test]
fn test_teams_lead_row_carries_marker_identity_and_placeholders() {
    let mut app = open_teams(vec![member("reviewer", "tm-001", MemberStatus::Working)]);
    app.storage
        .create_session(LEAD_SESSION, ".")
        .expect("create lead session");
    for _ in 0..3 {
        app.event_bus.increment_tool_calls(LEAD_SESSION);
    }

    let buffer = render(&mut app, 170, 44);
    let content = table_inner(app.teams_area);
    let header_y = row_containing(&buffer, &["id", "name", "status", "model"]);
    let lead_y = header_y + 1;
    let lead_row = screen_row(&buffer, lead_y);

    let marker = column_text(&buffer, content, lead_y, 0, columns::MARKER_W);
    assert!(
        marker.starts_with(glyphs::LEAD.trim_end()),
        "the lead row must carry the asterisk marker (FR-021): {lead_row:?}"
    );
    assert_eq!(
        buffer
            .cell((content.x, lead_y))
            .map(ratatui::buffer::Cell::style)
            .expect("marker cell")
            .fg,
        Some(Color::Green),
        "the lead marker must be green (FR-021)"
    );

    let short_lead = ragent_tui::utils::short_id(LEAD_SESSION);
    assert_eq!(
        column_text(&buffer, content, lead_y, col::ID, columns::ID_W).trim(),
        short_lead,
        "the lead id column must show the short lead session id (FR-021)"
    );
    assert_eq!(
        column_text(&buffer, content, lead_y, col::LABEL, columns::LABEL_W).trim(),
        "general",
        "the lead name column must show the configured agent name (FR-021)"
    );
    assert_eq!(
        column_text(&buffer, content, lead_y, col::STATUS, columns::STATUS_W).trim(),
        glyphs::LEAD_STATUS,
        "the lead status column must read active (FR-021)"
    );
    assert_eq!(
        column_text(&buffer, content, lead_y, col::MODEL, columns::MODEL_W).trim(),
        glyphs::LEAD_MODEL,
        "the lead model column must read lead (FR-021)"
    );

    let elapsed = column_text(&buffer, content, lead_y, col::ELAPSED, columns::ELAPSED_W);
    assert!(
        elapsed.trim().ends_with('s'),
        "the lead elapsed must come from the persisted session timestamp (FR-021), got {elapsed:?}"
    );
    assert_eq!(
        column_text(&buffer, content, lead_y, col::STEPS, columns::STEPS_W).trim(),
        "3",
        "the lead steps column must show the live tool-call count (FR-021)"
    );
    assert_eq!(
        column_text(&buffer, content, lead_y, col::CLAIMED, columns::CLAIMED_W).trim(),
        "-",
        "the lead claimed column must read - (FR-021)"
    );
    assert_eq!(
        column_text(&buffer, content, lead_y, col::DONE, columns::DONE_W).trim(),
        "-",
        "the lead done column must read - (FR-021)"
    );
}

/// Without a persisted session the lead elapsed falls back to `-` instead of a
/// bogus duration (FR-021).
#[test]
fn test_teams_lead_row_without_a_stored_session_shows_a_dash() {
    let mut app = open_teams(Vec::new());
    let buffer = render(&mut app, 170, 44);
    let content = table_inner(app.teams_area);
    let lead_y = row_containing(&buffer, &["id", "name", "status", "model"]) + 1;

    assert_eq!(
        column_text(&buffer, content, lead_y, col::ELAPSED, columns::ELAPSED_W).trim(),
        "-",
        "an unknown lead session must render - in the elapsed column (FR-021)"
    );
}

// -- FR-022: teammate rows --------------------------------------------------

/// Each teammate row is prefixed with a branch connector, carries the `>` focus
/// marker only when focused, the short agent id and a `<name>-<short id>` label
/// (FR-022).
#[test]
fn test_teams_teammate_rows_carry_connector_focus_marker_and_label() {
    let mut app = open_teams(vec![
        member("reviewer", "tm-001", MemberStatus::Working),
        member("tester", "tm-002", MemberStatus::Idle),
    ]);
    app.focused_teammate = Some("tm-001".to_string());

    let buffer = render(&mut app, 170, 44);
    let content = table_inner(app.teams_area);
    let lead_y = row_containing(&buffer, &["id", "name", "status", "model"]) + 1;

    let focused_y = lead_y + 1;
    let focused_row = screen_row(&buffer, focused_y);
    let focused_marker = column_text(&buffer, content, focused_y, 0, columns::MARKER_W);
    assert!(
        focused_marker.starts_with(glyphs::FOCUS),
        "the focused row must start with the focus marker (FR-022): {focused_row:?}"
    );
    assert!(
        focused_row.contains(glyphs::BRANCH_MID),
        "a non-last teammate must use the mid branch connector (FR-022): {focused_row:?}"
    );
    assert_eq!(
        column_text(&buffer, content, focused_y, col::ID, columns::ID_W).trim(),
        "tm-001",
        "the teammate id column must show the short agent id (FR-022)"
    );
    assert_eq!(
        column_text(&buffer, content, focused_y, col::LABEL, columns::LABEL_W).trim(),
        "reviewer-tm-001",
        "the label must be <name>-<short id> (FR-022)"
    );

    let last_y = focused_y + 1;
    let last_row = screen_row(&buffer, last_y);
    let last_marker = column_text(&buffer, content, last_y, 0, columns::MARKER_W);
    assert!(
        !last_marker.starts_with(glyphs::FOCUS),
        "an unfocused row must not paint the focus marker (FR-022): {last_row:?}"
    );
    assert!(
        last_row.contains(glyphs::BRANCH_LAST),
        "the last teammate must use the last-child connector (FR-022): {last_row:?}"
    );
    assert_eq!(
        column_text(&buffer, content, last_y, col::LABEL, columns::LABEL_W).trim(),
        "tester-tm-002",
        "the second teammate's label must carry its own id (FR-022)"
    );
}

/// A label longer than its column is clipped to exactly
/// [`columns::LABEL_W`] characters (FR-022, NFR-004).
#[test]
fn test_teams_teammate_label_is_clipped_to_its_column() {
    let long_name = "reviewer-with-a-very-long-name-indeed";
    let mut app = open_teams(vec![member(long_name, "tm-009", MemberStatus::Idle)]);
    let buffer = render(&mut app, 170, 44);
    let content = table_inner(app.teams_area);
    let row_y = row_containing(&buffer, &["tm-009"]);

    let painted = column_text(&buffer, content, row_y, col::LABEL, columns::LABEL_W);
    assert_eq!(
        painted.chars().count(),
        columns::LABEL_W,
        "the label cell must occupy exactly its column (FR-022)"
    );
    let expected: String = format!("{long_name}-tm-009")
        .chars()
        .take(columns::LABEL_W)
        .collect();
    assert_eq!(
        painted.trim_end(),
        expected.trim_end(),
        "the label must be a prefix clip of <name>-<short id> (FR-022, NFR-004)"
    );
}

// -- FR-023 / NFR-009: status vocabulary ------------------------------------

/// Every teammate state paints the canonical status word from the agent crate
/// and a colour unique to that state (FR-023, NFR-009).
#[test]
fn test_teams_status_words_match_the_canonical_mapping_in_distinct_colours() {
    let statuses = [
        MemberStatus::Spawning,
        MemberStatus::Working,
        MemberStatus::Idle,
        MemberStatus::PlanPending,
        MemberStatus::Blocked,
        MemberStatus::Suspended,
        MemberStatus::ShuttingDown,
        MemberStatus::Stopped,
        MemberStatus::Failed,
    ];
    let members: Vec<TeamMember> = statuses
        .iter()
        .enumerate()
        .map(|(i, status)| member(&format!("agent-{i}"), &format!("tm-{i:03}"), *status))
        .collect();
    let mut app = open_teams(members);

    let buffer = render(&mut app, 170, 48);
    let content = table_inner(app.teams_area);
    let mut colours = Vec::new();

    for (i, status) in statuses.iter().enumerate() {
        let agent_id = format!("tm-{i:03}");
        let row_y = row_containing(&buffer, &[&agent_id]);
        let painted = column_text(&buffer, content, row_y, col::STATUS, columns::STATUS_W);
        assert_eq!(
            painted.trim(),
            status.display_label(),
            "{status:?} must paint the canonical status word (NFR-009)"
        );
        let style = buffer
            .cell((content.x + col::STATUS as u16, row_y))
            .map(ratatui::buffer::Cell::style)
            .expect("status cell");
        colours.push(style.fg);
    }

    for (i, colour) in colours.iter().enumerate() {
        for (j, other) in colours.iter().enumerate() {
            if i != j {
                assert_ne!(
                    colour, other,
                    "{:?} and {:?} must not share a colour (FR-023)",
                    statuses[i], statuses[j]
                );
            }
        }
    }
}

/// The canonical words are the documented nine, in their compact UI spelling
/// (FR-023).
#[test]
fn test_teams_status_vocabulary_is_exactly_the_documented_nine_words() {
    let words: Vec<&str> = [
        MemberStatus::Spawning,
        MemberStatus::Working,
        MemberStatus::Idle,
        MemberStatus::PlanPending,
        MemberStatus::Blocked,
        MemberStatus::Suspended,
        MemberStatus::ShuttingDown,
        MemberStatus::Stopped,
        MemberStatus::Failed,
    ]
    .iter()
    .map(MemberStatus::display_label)
    .collect();

    assert_eq!(
        words,
        vec![
            "spawning",
            "working",
            "idle",
            "planning",
            "blocked",
            "suspended",
            "stopping",
            "stopped",
            "failed",
        ],
        "the status vocabulary is fixed by FR-023"
    );
}

// -- FR-024: the model column ----------------------------------------------

/// A genuine override paints `<provider>/<model>` clipped to the model column in
/// magenta; a member with no override paints `(inherited)` in hint colour
/// (FR-024).
#[test]
fn test_teams_model_column_shows_override_or_inherited() {
    let mut with_override = member("reviewer", "tm-001", MemberStatus::Working);
    with_override.model_override = Some(ModelRef {
        provider_id: "anthropic".to_string(),
        model_id: "claude-sonnet-4-20250514".to_string(),
    });
    let mut app = open_teams(vec![
        with_override,
        member("tester", "tm-002", MemberStatus::Idle),
    ]);
    // A different global model, so the override is genuinely its own.
    app.selected_model = Some("openai/gpt-4o".to_string());

    let buffer = render(&mut app, 170, 44);
    let content = table_inner(app.teams_area);

    let override_y = row_containing(&buffer, &["tm-001"]);
    let override_text = column_text(&buffer, content, override_y, col::MODEL, columns::MODEL_W);
    let expected: String = "anthropic/claude-sonnet-4-20250514"
        .chars()
        .take(columns::MODEL_W)
        .collect();
    assert_eq!(
        override_text.trim_end(),
        expected.trim_end(),
        "an override must render <provider>/<model> clipped to {} columns (FR-024)",
        columns::MODEL_W
    );
    assert_eq!(
        buffer
            .cell((content.x + col::MODEL as u16, override_y))
            .map(ratatui::buffer::Cell::style)
            .expect("model cell")
            .fg,
        Some(Color::Magenta),
        "a genuine override must be magenta (FR-024)"
    );

    let inherited_y = row_containing(&buffer, &["tm-002"]);
    assert_eq!(
        column_text(&buffer, content, inherited_y, col::MODEL, columns::MODEL_W).trim(),
        glyphs::INHERITED,
        "a member with no override must render (inherited) (FR-024)"
    );
    assert_eq!(
        buffer
            .cell((content.x + col::MODEL as u16, inherited_y))
            .map(ratatui::buffer::Cell::style)
            .expect("model cell")
            .fg,
        Some(colors::HINT),
        "(inherited) must use the hint colour (FR-024)"
    );
}

/// An override that equals the globally selected provider/model renders as
/// inherited, so the column never repeats the status-bar label (FR-024).
#[test]
fn test_teams_model_column_suppresses_an_override_equal_to_the_global_model() {
    let mut same = member("reviewer", "tm-001", MemberStatus::Working);
    same.model_override = Some(ModelRef {
        provider_id: "anthropic".to_string(),
        model_id: "claude-sonnet-4-20250514".to_string(),
    });
    let mut app = open_teams(vec![same]);
    app.selected_model = Some("anthropic/claude-sonnet-4-20250514".to_string());

    let buffer = render(&mut app, 170, 44);
    let content = table_inner(app.teams_area);
    let row_y = row_containing(&buffer, &["tm-001"]);

    assert_eq!(
        column_text(&buffer, content, row_y, col::MODEL, columns::MODEL_W).trim(),
        glyphs::INHERITED,
        "an override equal to the global model must render as inherited (FR-024)"
    );
    assert_eq!(
        buffer
            .cell((content.x + col::MODEL as u16, row_y))
            .map(ratatui::buffer::Cell::style)
            .expect("model cell")
            .fg,
        Some(colors::HINT),
        "a suppressed override must not be magenta (FR-024)"
    );
}

// -- FR-025: counters -------------------------------------------------------

/// Zero message counts paint `-`; non-zero counts paint their value in the
/// category colour (FR-025).
#[test]
fn test_teams_message_counters_render_dash_for_zero_and_colour_for_non_zero() {
    let mut app = open_teams(vec![
        member("reviewer", "tm-001", MemberStatus::Working),
        member("tester", "tm-002", MemberStatus::Idle),
    ]);
    app.team_message_counts.insert("tm-001".to_string(), (0, 0));
    app.team_message_counts.insert("tm-002".to_string(), (4, 2));

    let buffer = render(&mut app, 170, 44);
    let content = table_inner(app.teams_area);

    let zero_y = row_containing(&buffer, &["tm-001"]);
    assert_eq!(
        column_text(&buffer, content, zero_y, col::SENT, columns::SENT_W).trim(),
        "-",
        "a zero sent count must render - (FR-025)"
    );
    assert_eq!(
        column_text(&buffer, content, zero_y, col::RECV, columns::RECV_W).trim(),
        "-",
        "a zero recv count must render - (FR-025)"
    );

    let counted_y = row_containing(&buffer, &["tm-002"]);
    assert_eq!(
        column_text(&buffer, content, counted_y, col::SENT, columns::SENT_W).trim(),
        "4",
        "a non-zero sent count must render its value (FR-025)"
    );
    assert_eq!(
        column_text(&buffer, content, counted_y, col::RECV, columns::RECV_W).trim(),
        "2",
        "a non-zero recv count must render its value (FR-025)"
    );
    for start in [col::SENT, col::RECV] {
        assert_eq!(
            buffer
                .cell((content.x + start as u16, counted_y))
                .map(ratatui::buffer::Cell::style)
                .expect("count cell")
                .fg,
            Some(Color::Cyan),
            "a non-zero message count must use its category colour (FR-025)"
        );
    }
}

/// Claimed and done come from the team task store: claimed counts the tasks
/// assigned to the teammate, done counts those completed (FR-025).
#[tokio::test]
async fn test_teams_task_counters_come_from_the_team_task_store() {
    let _guard = crate::global_env_lock();
    let scratch = scratch_dir("task-counters");
    let original = std::env::current_dir().expect("cwd");

    let store = TeamStore::create("alpha", LEAD_SESSION, &scratch, true).expect("create team");
    let tasks = store.task_store().expect("task store");

    let mut pending = Task::new("task-001", "Pending work");
    pending.assigned_to = Some("tm-001".to_string());
    tasks.add_task(pending).await.expect("add pending");

    let mut running = Task::new("task-002", "Running work");
    running.status = TaskStatus::InProgress;
    running.assigned_to = Some("tm-001".to_string());
    tasks.add_task(running).await.expect("add running");

    let mut done = Task::new("task-003", "Finished work");
    done.status = TaskStatus::Completed;
    done.assigned_to = Some("tm-001".to_string());
    tasks.add_task(done).await.expect("add done");

    std::env::set_current_dir(&scratch).expect("enter scratch");

    let mut app = open_teams(vec![
        member("reviewer", "tm-001", MemberStatus::Working),
        member("tester", "tm-002", MemberStatus::Idle),
    ]);
    let buffer = render(&mut app, 170, 44);
    let content = table_inner(app.teams_area);

    let counted_y = row_containing(&buffer, &["tm-001"]);
    assert_eq!(
        column_text(
            &buffer,
            content,
            counted_y,
            col::CLAIMED,
            columns::CLAIMED_W
        )
        .trim(),
        "3",
        "claimed must count every task assigned to the teammate (FR-025)"
    );
    assert_eq!(
        column_text(&buffer, content, counted_y, col::DONE, columns::DONE_W).trim(),
        "1",
        "done must count the teammate's completed tasks (FR-025)"
    );
    assert_eq!(
        buffer
            .cell((content.x + col::CLAIMED as u16, counted_y))
            .map(ratatui::buffer::Cell::style)
            .expect("claimed cell")
            .fg,
        Some(Color::Yellow),
        "a non-zero claimed count must be coloured (FR-025)"
    );
    assert_eq!(
        buffer
            .cell((content.x + col::DONE as u16, counted_y))
            .map(ratatui::buffer::Cell::style)
            .expect("done cell")
            .fg,
        Some(Color::Green),
        "a non-zero done count must be green (FR-025)"
    );

    let idle_y = row_containing(&buffer, &["tm-002"]);
    assert_eq!(
        column_text(&buffer, content, idle_y, col::CLAIMED, columns::CLAIMED_W).trim(),
        "0",
        "a teammate with no tasks must render a zero claimed count (FR-025)"
    );

    std::env::set_current_dir(original).expect("restore cwd");
    let _ = std::fs::remove_dir_all(&scratch);
}

// -- FR-026: empty state ----------------------------------------------------

/// With no members the hint line paints directly beneath the lead row (FR-026).
#[test]
fn test_teams_empty_state_paints_the_hint_beneath_the_lead_row() {
    let mut app = open_teams(Vec::new());
    let buffer = render(&mut app, 170, 44);

    let header_y = row_containing(&buffer, &["id", "name", "status", "model"]);
    let lead_y = header_y + 1;
    let hint_y = row_containing(&buffer, &[glyphs::EMPTY_HINT]);

    assert_eq!(
        hint_y,
        lead_y + 1,
        "the empty-state hint must sit directly beneath the lead row (FR-026)"
    );
    assert_eq!(
        style_at(&buffer, hint_y, glyphs::EMPTY_HINT).fg,
        Some(colors::HINT),
        "the hint must use the hint colour (FR-026)"
    );
}

/// The hint disappears as soon as the team has a member (FR-026).
#[test]
fn test_teams_empty_state_hint_is_absent_with_members() {
    let mut app = open_teams(vec![member("reviewer", "tm-001", MemberStatus::Idle)]);
    let buffer = render(&mut app, 170, 44);

    for y in 0..buffer.area.height {
        assert!(
            !screen_row(&buffer, y).contains(glyphs::EMPTY_HINT),
            "the empty-state hint must not paint while members exist (FR-026)"
        );
    }
}

// -- Row controls -----------------------------------------------------------

/// The painted control columns align with the published row maps: a live
/// teammate row paints `[stop]` and `[x]` at their pinned columns (a terminal
/// row paints neither), the row map stays index-aligned with the painted rows,
/// and the cursor ordinal list names exactly the teammate rows in paint order
/// (the keyboard cursor reads these vectors instead of the retired hit-rect
/// vectors; FR-071, NFR-011, FR-038).
#[test]
fn test_teams_row_controls_paint_at_the_pinned_columns_and_track_the_cursor_list() {
    let mut app = open_teams(vec![
        member("reviewer", "tm-001", MemberStatus::Working),
        member("tester", "tm-002", MemberStatus::Stopped),
        member("archivist", "tm-003", MemberStatus::Idle),
    ]);
    let buffer = render(&mut app, full_table_width(), 44);
    let content = table_inner(app.teams_area);
    let lead_y = row_containing(&buffer, &["id", "name", "status", "model"]) + 1;

    // Live rows paint both controls at their pinned columns; the stopped row
    // paints neither.
    let live_y = lead_y + 1;
    let stop = column_text(&buffer, content, live_y, columns::STOP_X, columns::STOP_W);
    assert_eq!(
        stop.trim(),
        glyphs::STOP,
        "the first teammate's row must paint [stop] at its documented column"
    );
    let kill = column_text(&buffer, content, live_y, columns::KILL_X, columns::KILL_W);
    assert_eq!(
        kill.trim(),
        glyphs::KILL,
        "the first teammate's row must paint [x] at its documented column"
    );

    let stopped_y = lead_y + 2;
    let stopped_row = screen_row(&buffer, stopped_y);
    assert!(
        stopped_row.contains("tm-002"),
        "a terminal teammate row still paints: {stopped_row:?}"
    );
    assert!(
        !stopped_row.contains(glyphs::STOP) && !stopped_row.contains(glyphs::KILL),
        "a terminal teammate row paints no controls (FR-071): {stopped_row:?}"
    );

    let third_y = lead_y + 3;
    let third = column_text(&buffer, content, third_y, columns::STOP_X, columns::STOP_W);
    assert_eq!(
        third.trim(),
        glyphs::STOP,
        "the third teammate's controls must stay aligned with its painted row"
    );

    // The row map stays index-aligned with the painted rows: header, lead and
    // the three teammates.
    assert_eq!(
        app.teams_row_agent_ids.len(),
        5,
        "one row key per painted row"
    );
    assert_eq!(app.teams_row_agent_ids[0], "", "the header opens nothing");
    assert_eq!(
        app.teams_row_agent_ids[1], "lead",
        "the lead row keeps its own sentinel key"
    );
    assert_eq!(app.teams_row_agent_ids[2], "tm-001");
    assert_eq!(app.teams_row_agent_ids[3], "tm-002");
    assert_eq!(app.teams_row_agent_ids[4], "tm-003");

    // The cursor ordinal list covers teammate rows only, in paint order - a
    // terminal row still carries its id, so Space/Del on it resolve (and are
    // refused) through the member status, not through the paint.
    assert_eq!(
        app.teams_teammate_agent_ids,
        vec![
            "tm-001".to_string(),
            "tm-002".to_string(),
            "tm-003".to_string(),
        ],
        "the cursor range must name the teammate rows in paint order"
    );
}

/// A suspended teammate paints the resume glyph padded to the stop column so
/// the kill control never moves.
#[test]
fn test_teams_suspended_row_paints_the_resume_glyph_in_place_of_stop() {
    let suspended = member("reviewer", "tm-001", MemberStatus::Suspended);
    let mut app = open_teams(vec![suspended]);
    let buffer = render(&mut app, full_table_width(), 44);
    let content = table_inner(app.teams_area);
    let row_y = row_containing(&buffer, &["tm-001"]);

    let control = column_text(&buffer, content, row_y, columns::STOP_X, columns::STOP_W);
    assert_eq!(
        control.trim(),
        glyphs::RESUME,
        "a suspended teammate must paint the resume glyph (FR-022)"
    );
    let kill = column_text(&buffer, content, row_y, columns::KILL_X, columns::KILL_W);
    assert_eq!(
        kill.trim(),
        glyphs::KILL,
        "the kill control must stay pinned on a suspended row"
    );
}

// -- NFR-004: safe truncation ----------------------------------------------

/// Multi-byte names, ids and labels never split a character and never panic
/// (NFR-004).
#[test]
fn test_teams_rows_truncate_multi_byte_text_on_character_boundaries() {
    let multibyte = "wörker-Ωmega-übermäßig-very-long-name";
    let mut app = open_teams(vec![member(multibyte, "tm-001", MemberStatus::Working)]);
    app.agent_name = "gëneral-Ωmega-überlang".to_string();

    let buffer = render(&mut app, 170, 44);
    let content = table_inner(app.teams_area);
    let row_y = row_containing(&buffer, &["tm-001"]);

    let label = column_text(&buffer, content, row_y, col::LABEL, columns::LABEL_W);
    assert!(
        label.chars().count() <= columns::LABEL_W,
        "the label must stay inside its column (NFR-004)"
    );
    let expected: String = format!("{multibyte}-tm-001")
        .chars()
        .take(columns::LABEL_W)
        .collect();
    assert!(
        expected.starts_with(label.trim_end()),
        "the painted label must be a prefix of the source label without splitting a \
         character: painted {label:?}, expected prefix of {expected:?} (NFR-004)"
    );

    let lead_y = row_containing(&buffer, &["id", "name", "status", "model"]) + 1;
    let lead_name = column_text(&buffer, content, lead_y, col::LABEL, columns::LABEL_W);
    assert!(
        lead_name.chars().count() <= columns::LABEL_W,
        "a multi-byte agent name must be clipped to its column (NFR-004)"
    );
}

// -- NFR-011: column grid ---------------------------------------------------

/// The header and every data row share one column grid, so a value never drifts
/// out of the column its heading names (NFR-011).
#[test]
fn test_teams_header_and_rows_share_the_column_grid() {
    let mut app = open_teams(vec![member("reviewer", "tm-001", MemberStatus::Working)]);
    let buffer = render(&mut app, 170, 44);
    let content = table_inner(app.teams_area);
    let header_y = row_containing(&buffer, &["id", "name", "status", "model"]);
    let lead_y = header_y + 1;

    for (name, start, width) in [
        ("id", col::ID, columns::ID_W),
        ("name", col::LABEL, columns::LABEL_W),
        ("status", col::STATUS, columns::STATUS_W),
        ("model", col::MODEL, columns::MODEL_W),
    ] {
        let header = column_text(&buffer, content, header_y, start, width);
        assert!(
            header.trim_end().starts_with(name),
            "the {name} heading must start its column, got {header:?}"
        );
        let value = column_text(&buffer, content, lead_y, start, width);
        assert_eq!(
            value.chars().count(),
            width,
            "the {name} cell must occupy exactly {width} columns on a data row"
        );
    }

    // The right-aligned numeric columns end where the next column starts.
    assert_eq!(
        col::RECV + columns::RECV_W + columns::CONTROL_GAP_W,
        columns::STOP_X,
        "the counters must end exactly where the control block starts"
    );
}

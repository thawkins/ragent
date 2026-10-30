//! Teams subpanel - the table body of the Teams popup window.
//!
//! Projects the active team into the rows painted inside the Teams modal: the
//! lead row, one row per teammate, and the empty-state hint (spec `agentui`
//! FR-021 through FR-026).
//!
//! Every structural glyph the table paints is declared once in [`glyphs`] and
//! every column position in [`columns`], so a glyph or column change cannot
//! leave the header and the rows disagreeing (NFR-003, NFR-011). Identifiers,
//! labels and model strings are truncated on a character boundary so a
//! multi-byte sequence can never be split (NFR-004). Status words come from the
//! canonical mapping in the agent crate ([`MemberStatus::display_label`]) so the
//! panel cannot disagree with tool output (NFR-009).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};

use ragent_agent::team::{
    MemberStatus, PendingPlan, TaskStatus, TaskStore, TeamMember, TeamStore, pending_plans,
};

use crate::app::App;
use crate::layout_table_rows::{band_line, cursor_line};
use crate::theme::colors;
use crate::utils::{format_elapsed, short_id};

/// Every structural glyph the Teams table paints (NFR-003).
///
/// The renderer takes each of these from here rather than spelling the
/// character inline, so the header, the lead row and the teammate rows can
/// never drift apart.
pub mod glyphs {
    /// Marker painted on the lead row (FR-021).
    pub const LEAD: &str = "* ";
    /// Focus marker painted in front of the focused teammate's connector
    /// (FR-022).
    pub const FOCUS: &str = ">";
    /// Stand-in for [`FOCUS`] on an unfocused teammate row (FR-022).
    pub const FOCUS_NONE: &str = " ";
    /// Branch glyph for the last teammate row (FR-022).
    pub const BRANCH_LAST: &str = "`- ";
    /// Branch glyph for a teammate row with a later sibling (FR-022).
    pub const BRANCH_MID: &str = "|- ";
    /// Suspend control painted on a live teammate row.
    pub const STOP: &str = "[stop]";
    /// Resume control painted in place of [`STOP`] while suspended.
    pub const RESUME: &str = ">";
    /// Kill control painted on a live teammate row.
    pub const KILL: &str = "[x]";
    /// Literal painted in the model column of a teammate with no genuine
    /// override (FR-024).
    pub const INHERITED: &str = "(inherited)";
    /// Literal painted in the status column of the lead row (FR-021).
    pub const LEAD_STATUS: &str = "active";
    /// Literal painted in the model column of the lead row (FR-021).
    pub const LEAD_MODEL: &str = "lead";
    /// Literal painted in a count column that has no value (FR-021, FR-025).
    pub const NONE: &str = "-";
    /// Hint painted beneath the lead row while the team has no members
    /// (FR-026).
    pub const EMPTY_HINT: &str = "(no teammates yet - use team_spawn tool or blueprint)";
    /// Approve control painted in place of [`STOP`] on a `plan_pending` row
    /// (spec `teamssetup` FR-034).
    pub const APPROVE: &str = "[approve]";
    /// Reject control painted in place of [`KILL`] on a `plan_pending` row
    /// (spec `teamssetup` FR-034).
    pub const REJECT: &str = "[reject]";
    /// Literal opening the plan line painted beneath a `plan_pending` row
    /// (spec `teamssetup` FR-034).
    pub const PLAN_AWAITING: &str = "plan awaiting approval:";
    /// Literal painted when a `plan_pending` member's plan cannot be found in
    /// the lead's mailbox (spec `teamssetup` FR-034). The row still reports
    /// that a verdict is awaited rather than showing nothing.
    pub const PLAN_MISSING: &str = "(plan request not found)";
    /// Literal opening the hint row shown while the team has a parked member
    /// (spec `teamssetup` FR-028). The row names the resume key so the operator
    /// can see that a parked state has a way out.
    pub const PARKED_HINT: &str =
        "parked - Del resumes a blocked member, Space resumes a suspended one";
}

/// Column layout of the Teams table, counted in display columns from the left
/// edge of the table area (FR-020 through FR-025).
///
/// The widths are chosen so the action controls land past the counters: the
/// six-column `[stop]` control starts at [`STOP_X`] and the four-column `[x]`
/// control at [`KILL_X`]. The compile-time assertion below fails the build if an
/// edit to any width moves them.
pub mod columns {
    /// Width of the leading focus-marker plus connector segment (FR-022).
    pub const MARKER_W: usize = 4;
    /// Width of the short identifier column (FR-021, FR-022).
    pub const ID_W: usize = 8;
    /// Columns separating two adjacent fixed columns.
    pub const GAP_W: usize = 1;
    /// Width of the `<name>-<short id>` label column (FR-022).
    pub const LABEL_W: usize = 31;
    /// Width of the agent-type column (FR-022a).
    pub const AGENT_TYPE_W: usize = 16;
    /// Width of the status-word column (FR-021, FR-023).
    pub const STATUS_W: usize = 10;
    /// Width of the model column, the point a `<provider>/<model>` string is
    /// truncated to (FR-024).
    pub const MODEL_W: usize = 17;
    /// Width of the right-aligned elapsed-time column (FR-021).
    pub const ELAPSED_W: usize = 7;
    /// Width of the right-aligned step-count column (FR-021).
    pub const STEPS_W: usize = 5;
    /// Width of the right-aligned claimed-task column (FR-025).
    pub const CLAIMED_W: usize = 7;
    /// Width of the right-aligned completed-task column (FR-025).
    pub const DONE_W: usize = 5;
    /// Width of the right-aligned sent-message column (FR-025).
    pub const SENT_W: usize = 5;
    /// Width of the right-aligned received-message column (FR-025).
    pub const RECV_W: usize = 5;
    /// Columns separating the counters from the controls, and the two controls
    /// from each other.
    pub const CONTROL_GAP_W: usize = 2;
    /// First column of the `[stop]` / `[approve]` control.
    pub const STOP_X: usize = 132;
    /// Width of the `[stop]` control column, wide enough for the
    /// `plan_pending` row's `[approve]` literal (spec `teamssetup` FR-034).
    pub const STOP_W: usize = 9;
    /// First column of the `[x]` / `[reject]` control.
    pub const KILL_X: usize = 143;
    /// Width of the `[x]` control column, wide enough for the `plan_pending`
    /// row's `[reject]` literal (spec `teamssetup` FR-034).
    pub const KILL_W: usize = 8;
}

/// The fixed columns must add up to the pinned `[stop]` column.
const _: () = assert!(
    columns::MARKER_W
        + columns::ID_W
        + columns::GAP_W
        + columns::LABEL_W
        + columns::GAP_W
        + columns::AGENT_TYPE_W
        + columns::GAP_W
        + columns::STATUS_W
        + columns::GAP_W
        + columns::MODEL_W
        + columns::GAP_W
        + columns::ELAPSED_W
        + columns::GAP_W
        + columns::STEPS_W
        + columns::GAP_W
        + columns::CLAIMED_W
        + columns::GAP_W
        + columns::DONE_W
        + columns::GAP_W
        + columns::SENT_W
        + columns::GAP_W
        + columns::RECV_W
        + columns::CONTROL_GAP_W
        == columns::STOP_X
);

/// The control columns must follow the counters without overlapping.
const _: () =
    assert!(columns::STOP_X + columns::STOP_W + columns::CONTROL_GAP_W == columns::KILL_X);

/// The muted blue the table's own border paints in.
///
/// The modal frame carries the bright team-blue accent; this calmer tone keeps
/// the second border legible as structure rather than as a second frame
/// shouting for attention.
const INNER_BORDER: Color = Color::Rgb(62, 84, 118);

/// Whether `status` is terminal, in which case the row carries no controls.
fn is_terminal(status: &MemberStatus) -> bool {
    matches!(
        status,
        MemberStatus::ShuttingDown | MemberStatus::Stopped | MemberStatus::Failed
    )
}

/// The colour painted for one teammate state (FR-023).
///
/// Every state has its own colour so a glance at the table distinguishes them.
/// The word itself comes from [`MemberStatus::display_label`] (NFR-009).
fn status_color(status: &MemberStatus) -> Color {
    match status {
        MemberStatus::Spawning => Color::Yellow,
        MemberStatus::Working => Color::Cyan,
        MemberStatus::Idle => Color::Green,
        MemberStatus::PlanPending => Color::Magenta,
        MemberStatus::Blocked => Color::LightRed,
        MemberStatus::Suspended => Color::DarkGray,
        MemberStatus::ShuttingDown => Color::LightYellow,
        MemberStatus::Stopped => colors::HINT,
        MemberStatus::Failed => Color::Red,
    }
}

/// Clip `text` to at most `budget` characters.
///
/// The cut is taken on a character boundary so a multi-byte sequence can never
/// be split, and the result never exceeds the budget (NFR-004).
fn clip(text: &str, budget: usize) -> String {
    text.chars().take(budget).collect()
}

/// Render a message-count cell: `"-"` when zero, otherwise the number (FR-025).
fn count_cell(n: u32) -> String {
    if n == 0 {
        glyphs::NONE.to_string()
    } else {
        n.to_string()
    }
}

/// The dimmed style shared by cells that carry no value (FR-025).
fn hint_style() -> Style {
    Style::default().fg(colors::HINT)
}

/// One padded column cell of a Teams row.
///
/// Carrying the width and alignment with the text keeps the header, the lead row
/// and the teammate rows in the same column grid (NFR-011).
struct Cell {
    text: String,
    style: Style,
    width: usize,
    right: bool,
}

impl Cell {
    /// A left-aligned cell.
    fn left(text: impl Into<String>, width: usize, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
            width,
            right: false,
        }
    }

    /// A right-aligned (numeric) cell.
    fn right(text: impl Into<String>, width: usize, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
            width,
            right: true,
        }
    }

    /// The padded span this cell paints.
    fn span(&self) -> Span<'static> {
        let padded = if self.right {
            format!("{:>width$}", self.text, width = self.width)
        } else {
            format!("{:<width$}", self.text, width = self.width)
        };
        Span::styled(padded, self.style)
    }
}

/// Join a row's leading marker spans and its cells into one painted line.
///
/// A single blank column separates adjacent cells, matching [`columns::GAP_W`]
/// and the header built by [`header_line`].
fn row_line(marker: Vec<Span<'static>>, cells: &[Cell]) -> Line<'static> {
    let mut spans = marker;
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" ".repeat(columns::GAP_W), Style::default()));
        }
        spans.push(cell.span());
    }
    Line::from(spans)
}

/// The dimmed header row naming every column (FR-020).
fn header_line() -> Line<'static> {
    let dim = Style::default()
        .fg(colors::HINT)
        .add_modifier(Modifier::DIM)
        .add_modifier(Modifier::BOLD);
    let cells = vec![
        Cell::left("id", columns::ID_W, dim),
        Cell::left("name", columns::LABEL_W, dim),
        Cell::left("agent type", columns::AGENT_TYPE_W, dim),
        Cell::left("status", columns::STATUS_W, dim),
        Cell::left("model", columns::MODEL_W, dim),
        Cell::right("elapsed", columns::ELAPSED_W, dim),
        Cell::right("steps", columns::STEPS_W, dim),
        Cell::right("claimed", columns::CLAIMED_W, dim),
        Cell::right("done", columns::DONE_W, dim),
        Cell::right("sent", columns::SENT_W, dim),
        Cell::right("recv", columns::RECV_W, dim),
    ];
    let mut line = row_line(
        vec![Span::styled(" ".repeat(columns::MARKER_W), dim)],
        &cells,
    );
    let mut spans = std::mem::take(&mut line.spans);
    spans.push(Span::styled(
        " ".repeat(columns::CONTROL_GAP_W),
        Style::default(),
    ));
    spans.push(Span::styled(
        format!("{:<width$}", glyphs::STOP, width = columns::STOP_W),
        dim,
    ));
    spans.push(Span::styled(
        " ".repeat(columns::CONTROL_GAP_W),
        Style::default(),
    ));
    spans.push(Span::styled(
        format!("{:<width$}", glyphs::KILL, width = columns::KILL_W),
        dim,
    ));
    Line::from(spans)
}

/// Build the trailing `[stop]` / `[x]` control spans for a teammate row.
///
/// A live row paints the suspend control padded to [`columns::STOP_W`] - `>`
/// instead of `[stop]` while suspended, padded so `[x]` never moves - then a
/// gap and the kill control. A `plan_pending` row paints the two controls that
/// resolve its parked state instead, `[approve]` and `[reject]` (spec
/// `teamssetup` FR-034): the verdict is the only action a parked-for-plan
/// teammate can act on, so offering suspend/kill in those columns would name a
/// verb that does not unblock it.
fn control_spans(status: &MemberStatus) -> Vec<Span<'static>> {
    let (left, left_fg) = match status {
        MemberStatus::PlanPending => (glyphs::APPROVE, Color::Green),
        MemberStatus::Suspended => (glyphs::RESUME, Color::Green),
        _ => (glyphs::STOP, Color::Yellow),
    };
    let (right, right_fg) = match status {
        MemberStatus::PlanPending => (glyphs::REJECT, Color::LightRed),
        _ => (glyphs::KILL, Color::Red),
    };
    vec![
        Span::styled(" ".repeat(columns::CONTROL_GAP_W), Style::default()),
        Span::styled(
            format!("{left:<width$}", width = columns::STOP_W),
            Style::default().fg(left_fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ".repeat(columns::CONTROL_GAP_W), Style::default()),
        Span::styled(
            right,
            Style::default().fg(right_fg).add_modifier(Modifier::BOLD),
        ),
    ]
}

/// The task counts painted in one teammate row: `(claimed, done)` (FR-025).
type TaskCounts = HashMap<String, (usize, usize)>;

/// The plan each `plan_pending` teammate is waiting on, keyed by agent id
/// (spec `teamssetup` FR-034).
///
/// Read once per frame from the same store `task_counts` reads (NFR-002), never
/// once per row, and `team::pending_plans` is the single implementation of the
/// roster-plus-mailbox join so the popup cannot disagree with `team_status`.
fn pending_plans_by_agent(app: &App) -> HashMap<String, PendingPlan> {
    let working_dir = std::env::current_dir().unwrap_or_default();
    app.active_team
        .as_ref()
        .and_then(|team| TeamStore::load_by_name(&team.name, &working_dir).ok())
        .map(|store| {
            pending_plans(&store)
                .into_iter()
                .map(|plan| (plan.agent_id.clone(), plan))
                .collect()
        })
        .unwrap_or_default()
}

/// The single-line excerpt of a submitted plan painted beneath the row.
///
/// A plan is free-form multi-line text; the popup row is one line, so the first
/// non-blank line stands in for the whole plan and the budget clips it on a
/// character boundary (NFR-004).
fn plan_excerpt(plan: Option<&str>) -> String {
    plan.and_then(|text| text.lines().map(str::trim).find(|line| !line.is_empty()))
        .unwrap_or(glyphs::PLAN_MISSING)
        .to_string()
}

/// The resume hint painted while the team has a parked member (spec
/// `teamssetup` FR-028), `None` when no member is parked.
///
/// The hint names the members it is about - by name, not by status word - so
/// the operator can match the row to the key without re-reading the table. A
/// `Blocked` member is resumed with `Del`, a `Suspended` one with `Space`; the
/// wording follows the key that actually carries the verb on that status.
fn parked_resume_hint(members: &[TeamMember]) -> Option<String> {
    let blocked: Vec<&str> = members
        .iter()
        .filter(|m| m.status == MemberStatus::Blocked)
        .map(|m| m.name.as_str())
        .collect();
    let suspended: Vec<&str> = members
        .iter()
        .filter(|m| m.status == MemberStatus::Suspended)
        .map(|m| m.name.as_str())
        .collect();
    if blocked.is_empty() && suspended.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    if !blocked.is_empty() {
        parts.push(format!("Del resumes blocked {}", blocked.join(", ")));
    }
    if !suspended.is_empty() {
        parts.push(format!("Space resumes suspended {}", suspended.join(", ")));
    }
    Some(format!("({})", parts.join("; ")))
}

/// Read the per-agent task counts from the team task store.
///
/// The store is read once per frame (NFR-002), never once per row.
fn task_counts(app: &App) -> TaskCounts {
    let working_dir = std::env::current_dir().unwrap_or_default();
    app.active_team
        .as_ref()
        .and_then(|team| TeamStore::load_by_name(&team.name, &working_dir).ok())
        .and_then(|store| TaskStore::open(&store.dir).ok())
        .and_then(|tasks| tasks.read().ok())
        .map(|list| {
            let mut counts: TaskCounts = HashMap::new();
            for task in &list.tasks {
                let Some(agent) = task.assigned_to.as_ref() else {
                    continue;
                };
                let entry = counts.entry(agent.clone()).or_default();
                entry.0 += 1; // claimed
                if task.status == TaskStatus::Completed {
                    entry.1 += 1; // done
                }
            }
            counts
        })
        .unwrap_or_default()
}

/// The elapsed time of the lead session, from its persisted creation timestamp.
///
/// Falls back to `"-"` when the session is not in storage or its timestamp is
/// not RFC 3339 (FR-021).
fn lead_elapsed(app: &App, lead_session: &str) -> String {
    app.storage
        .get_session(lead_session)
        .ok()
        .flatten()
        .and_then(|session| DateTime::parse_from_rfc3339(&session.created_at).ok())
        .map(|dt| format_elapsed(dt.with_timezone(&Utc)))
        .unwrap_or_else(|| glyphs::NONE.to_string())
}

/// The `(text, colour)` pair the model column paints for one teammate (FR-024).
///
/// A genuine override paints `<provider>/<model>` clipped to
/// [`columns::MODEL_W`] in magenta. A member with no override - or whose
/// override is the globally selected provider/model, which the status bar
/// already shows - paints the inherited literal in hint colour.
fn model_column(member: &TeamMember, global_model: Option<&str>) -> (String, Color) {
    let Some(model_ref) = member.model_override.as_ref() else {
        return (glyphs::INHERITED.to_string(), colors::HINT);
    };
    let rendered = format!("{}/{}", model_ref.provider_id, model_ref.model_id);
    if global_model == Some(rendered.as_str()) {
        return (glyphs::INHERITED.to_string(), colors::HINT);
    }
    (clip(&rendered, columns::MODEL_W), Color::Magenta)
}

/// Build the `<name>-<short id>` label of one teammate row (FR-022).
///
/// The label is clipped to [`columns::LABEL_W`] on a character boundary so it
/// can never overflow its column or split a multi-byte sequence (NFR-004).
fn teammate_label(member: &TeamMember, id_short: &str) -> String {
    let label = format!("{}-{}", member.name, id_short);
    clip(&label, columns::LABEL_W)
}

/// Build the four fixed cells that follow the marker and label columns.
///
/// `steps` is the live tool-call count, `claimed`/`done` come from the team task
/// store and `sent`/`recv` from the per-agent message counters (FR-025). Zero
/// message counts paint `"-"`; a non-zero count paints in its category colour.
fn counter_cells(
    elapsed: &str,
    steps: &str,
    claimed: usize,
    done: usize,
    sent: u32,
    recv: u32,
) -> Vec<Cell> {
    let hint = hint_style();
    vec![
        Cell::right(elapsed, columns::ELAPSED_W, hint),
        Cell::right(steps, columns::STEPS_W, hint),
        Cell::right(
            claimed.to_string(),
            columns::CLAIMED_W,
            Style::default().fg(if claimed > 0 {
                Color::Yellow
            } else {
                colors::HINT
            }),
        ),
        Cell::right(
            done.to_string(),
            columns::DONE_W,
            Style::default().fg(if done > 0 { Color::Green } else { colors::HINT }),
        ),
        Cell::right(
            count_cell(sent),
            columns::SENT_W,
            Style::default().fg(if sent > 0 { Color::Cyan } else { colors::HINT }),
        ),
        Cell::right(
            count_cell(recv),
            columns::RECV_W,
            Style::default().fg(if recv > 0 { Color::Cyan } else { colors::HINT }),
        ),
    ]
}

/// Render the Teams subpanel into `area`.
///
/// The first row is the dimmed header (FR-020), the second the lead row
/// (FR-021), then one row per teammate (FR-022 through FR-025) or the empty-state
/// hint when the team has no members (FR-026).
pub fn render_teams_subpanel(frame: &mut Frame, app: &mut App, area: Rect) {
    app.teams_row_agent_ids.clear();
    app.teams_teammate_agent_ids.clear();

    app.refresh_team_member_session_ids();
    let team_name = app
        .active_team
        .as_ref()
        .map(|team| team.name.clone())
        .unwrap_or_else(|| "team".to_string());

    let lead_session = app.session_id.clone().unwrap_or_default();
    let lead_steps = app.event_bus.current_tool_calls(&lead_session);
    let lead_elapsed = lead_elapsed(app, &lead_session);
    let lead_agent_id = app.agent_name.clone();
    let (lead_sent, lead_recv) = app
        .team_message_counts
        .get(&lead_agent_id)
        .or_else(|| app.team_message_counts.get("lead"))
        .copied()
        .unwrap_or((0, 0));

    let counts = task_counts(app);
    // FR-034: one store read for every parked plan this frame, so a
    // `plan_pending` row can name the plan it waits on.
    let parked_plans = pending_plans_by_agent(app);
    let global_model = app.active_model_ref_string();
    let member_count = app.team_members.len();

    let mut lines: Vec<Line> = Vec::new();
    // Click key per painted row, kept index-aligned with `lines` (T-008): the
    // header and the empty-state hint open nothing, the lead row is the
    // `"lead"` sentinel, and a teammate row carries its agent id.
    let mut row_keys: Vec<String> = Vec::new();
    lines.push(header_line());
    row_keys.push(String::new());

    // -- lead row (FR-021) -------------------------------------------------
    let lead = Color::Green;
    lines.push(row_line(
        vec![Span::styled(
            format!("{:<width$}", glyphs::LEAD, width = columns::MARKER_W),
            Style::default().fg(lead),
        )],
        &[
            Cell::left(
                short_id(&lead_session),
                columns::ID_W,
                Style::default().fg(lead).add_modifier(Modifier::BOLD),
            ),
            Cell::left(
                clip(&app.agent_name, columns::LABEL_W),
                columns::LABEL_W,
                Style::default().fg(lead).add_modifier(Modifier::BOLD),
            ),
            Cell::left(
                clip(&app.agent_name, columns::AGENT_TYPE_W),
                columns::AGENT_TYPE_W,
                Style::default().fg(lead),
            ),
            Cell::left(
                glyphs::LEAD_STATUS,
                columns::STATUS_W,
                Style::default().fg(lead),
            ),
            Cell::left(glyphs::LEAD_MODEL, columns::MODEL_W, hint_style()),
            Cell::right(lead_elapsed, columns::ELAPSED_W, hint_style()),
            Cell::right(lead_steps.to_string(), columns::STEPS_W, hint_style()),
            // The lead owns no claimable task slot (FR-021).
            Cell::right(glyphs::NONE, columns::CLAIMED_W, hint_style()),
            Cell::right(glyphs::NONE, columns::DONE_W, hint_style()),
            Cell::right(
                count_cell(lead_sent),
                columns::SENT_W,
                Style::default().fg(if lead_sent > 0 {
                    Color::Cyan
                } else {
                    colors::HINT
                }),
            ),
            Cell::right(
                count_cell(lead_recv),
                columns::RECV_W,
                Style::default().fg(if lead_recv > 0 {
                    Color::Cyan
                } else {
                    colors::HINT
                }),
            ),
        ],
    ));
    row_keys.push("lead".to_string());

    // -- teammate rows (FR-022 through FR-025) -----------------------------
    let teammate_row_start = row_keys.len();
    for (index, member) in app.team_members.iter().enumerate() {
        let member_agent_name = member.agent_id.clone();
        let id_short = short_id(&member.agent_id);
        let label = teammate_label(member, &id_short);
        let is_focused = app.focused_teammate.as_ref() == Some(&member.agent_id);
        let steps = member
            .session_id
            .as_deref()
            .map(|sid| app.event_bus.current_tool_calls(sid))
            .unwrap_or(0);
        let (claimed, done) = counts.get(&member.agent_id).copied().unwrap_or((0, 0));
        let (sent, recv) = app
            .team_message_counts
            .get(&member.agent_id)
            .copied()
            .unwrap_or((0, 0));
        let (model_text, model_color) = model_column(member, global_model.as_deref());

        let focus = if is_focused {
            glyphs::FOCUS
        } else {
            glyphs::FOCUS_NONE
        };
        let connector = if index + 1 == member_count {
            glyphs::BRANCH_LAST
        } else {
            glyphs::BRANCH_MID
        };
        let name_style = if is_focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        // FR-052: a `lead` sender or recipient never increments a counter, so
        // the lead row's own sent/recv cells are always `-`; a member whose
        // agent id equals the team lead's agent name (the "lead" counter key
        // the event handlers skip) is rendered the same way.
        let counters_hidden = member_agent_name == "lead";

        let mut spans = vec![
            Span::styled(
                format!("{focus}{connector}"),
                Style::default().fg(if is_focused {
                    Color::Yellow
                } else {
                    Color::Blue
                }),
            ),
            Cell::left(id_short, columns::ID_W, Style::default().fg(Color::Cyan)).span(),
            Span::styled(" ".repeat(columns::GAP_W), Style::default()),
            Cell::left(label, columns::LABEL_W, name_style).span(),
            Span::styled(" ".repeat(columns::GAP_W), Style::default()),
            Cell::left(
                clip(&member.agent_type, columns::AGENT_TYPE_W),
                columns::AGENT_TYPE_W,
                Style::default().fg(Color::White),
            )
            .span(),
            Span::styled(" ".repeat(columns::GAP_W), Style::default()),
            Cell::left(
                member.status.display_label(),
                columns::STATUS_W,
                Style::default().fg(status_color(&member.status)),
            )
            .span(),
            Span::styled(" ".repeat(columns::GAP_W), Style::default()),
            Cell::left(
                model_text,
                columns::MODEL_W,
                Style::default().fg(model_color),
            )
            .span(),
        ];
        let mut counters = counter_cells(
            &format_elapsed(member.created_at),
            &steps.to_string(),
            claimed,
            done,
            sent,
            recv,
        );
        if counters_hidden {
            // The sent and received cells stay at the "-" literal because the
            // event handlers never increment a `lead` counter key (FR-052).
            counters[4] = Cell::right(glyphs::NONE, columns::SENT_W, hint_style());
            counters[5] = Cell::right(glyphs::NONE, columns::RECV_W, hint_style());
        }
        for cell in &counters {
            spans.push(Span::styled(" ".repeat(columns::GAP_W), Style::default()));
            spans.push(cell.span());
        }

        // The `[stop]`/`[x]` columns stay painted as status indicators: the
        // keyboard Space/Del now act on the row, and the glyph shows what
        // each currently means (suspended rows paint `>` = resume, a
        // `plan_pending` row paints its verdict controls).
        if !is_terminal(&member.status) {
            spans.extend(control_spans(&member.status));
        }
        lines.push(Line::from(spans));
        row_keys.push(member.agent_id.clone());

        // FR-034: a teammate parked in `plan_pending` shows the plan it is
        // waiting on, so the row says what is blocked instead of only how long
        // the member has existed. The continuation row carries no click key:
        // the cursor is an ordinal over teammate rows only, and a plan line is
        // part of the row above it, not a row of its own.
        if member.status == MemberStatus::PlanPending {
            let excerpt = plan_excerpt(
                parked_plans
                    .get(&member.agent_id)
                    .and_then(|plan| plan.plan.as_deref()),
            );
            // The excerpt runs to the table's inner right edge, so it can
            // never paint outside the block (NFR-010).
            let budget = (area.width as usize)
                .saturating_sub(2)
                .saturating_sub(columns::MARKER_W + glyphs::PLAN_AWAITING.chars().count() + 1);
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:<width$}", "", width = columns::MARKER_W),
                    Style::default(),
                ),
                Span::styled(
                    format!("{} ", glyphs::PLAN_AWAITING),
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    clip(&excerpt, budget.max(1)),
                    Style::default().fg(Color::White),
                ),
            ]));
            row_keys.push(String::new());
        }
    }
    // The keyboard cursor is an ordinal over the painted teammate rows only:
    // publish the agent ids in paint order for the cursor's Space/Del/Enter
    // resolution and the scroll sync (FR-038).
    app.teams_teammate_agent_ids = row_keys[teammate_row_start..].to_vec();

    // -- parked-member hint row (spec `teamssetup` FR-028) ------------------
    // While any member is `Blocked` or `Suspended`, the popup says which key
    // gets it out, so a parked team is visibly escapable rather than looking
    // wedged. The row carries no click key: it names keys, it is not a row.
    if let Some(hint) = parked_resume_hint(&app.team_members) {
        let budget = (area.width as usize).saturating_sub(4);
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {}", glyphs::PARKED_HINT),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                clip(&hint, budget.max(1)),
                Style::default().fg(colors::HINT),
            ),
        ]));
        row_keys.push(String::new());
    }

    // -- empty state (FR-026) ---------------------------------------------
    if member_count == 0 {
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", glyphs::EMPTY_HINT),
            Style::default()
                .fg(colors::HINT)
                .add_modifier(Modifier::DIM),
        )]));
        row_keys.push(String::new());
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " Team: {}  (lead + {} teammate{}) ",
            team_name,
            member_count,
            if member_count == 1 { "" } else { "s" }
        ))
        .title_style(
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        // The muted inner border keeps the outer modal frame the visual
        // anchor: the double border reads as chrome and content, not as two
        // competing boxes.
        .border_style(Style::default().fg(INNER_BORDER));

    let inner = block.inner(area);
    let visible = inner.height;
    let max_scroll = (lines.len() as u16).saturating_sub(visible);
    app.teams_max_scroll = max_scroll;
    // `teams_area` keeps the frame's content area (FR-008 geometry); the
    // painted-rows rectangle goes to `teams_rows_area` so a click maps onto
    // the row it was painted on without knowing the table border's offset
    // (FR-056, NFR-011).
    app.teams_area = area;
    app.teams_rows_area = inner;
    let scroll = app.teams_scroll_offset.min(max_scroll);
    app.teams_row_agent_ids = row_keys.clone();

    // Paint the block cursor over the selected teammate row, then keep the
    // viewport on that row: when the user pages past the edge the offset
    // follows the cursor instead of desynchronising (FR-038).
    let mut cursor_painted_row: Option<u16> = None;
    if let Some(cursor) = app.teams_cursor_teammate
        && let Some(agent_id) = app.teams_teammate_agent_ids.get(cursor)
    {
        if let Some(row_position) = row_keys.iter().position(|key| key == agent_id) {
            if row_position >= scroll as usize && row_position < scroll as usize + visible as usize
            {
                cursor_painted_row = Some((row_position as u16).saturating_sub(scroll));
            }
        }
    }

    // Render only the visible slice, mirroring the message-window pattern,
    // banding every odd data row (the header on row 0 stays clean) and
    // continuing the band pattern over the trailing body so the table panel
    // reads as one surface.
    let table_width = inner.width as usize;
    let mut window: Vec<Line> = lines
        .into_iter()
        .skip(scroll as usize)
        .take(visible as usize)
        .enumerate()
        .map(|(index, line)| {
            if Some(index as u16) == cursor_painted_row {
                return cursor_line(line, table_width);
            }
            if index % 2 == 1 {
                band_line(line, table_width)
            } else {
                line
            }
        })
        .collect();
    for index in window.len()..visible as usize {
        if index % 2 == 1 {
            window.push(band_line(Line::default(), table_width));
        } else {
            window.push(Line::default());
        }
    }
    frame.render_widget(Paragraph::new(window).block(block), area);

    if max_scroll > 0 {
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll as usize).position(scroll as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::default().fg(colors::HINT)),
            area,
            &mut scrollbar_state,
        );
    }
}

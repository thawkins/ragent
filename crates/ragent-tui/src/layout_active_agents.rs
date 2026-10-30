//! Active-agents subpanel - the table body of the Agents popup window.
//!
//! Projects the primary agent, the ancestor-indented sub-agent tree and the
//! background shell task section into the rows painted inside the Agents modal
//! (spec `agentui` FR-009 through FR-016, FR-062).
//!
//! Every structural and status glyph the table paints is declared once in
//! [`glyphs`], and every column position in [`columns`], so a glyph or column
//! change cannot leave the header and the rows disagreeing (NFR-003). Row
//! identifiers, names and commands are truncated on a character boundary so a
//! multi-byte sequence can never be split (NFR-004).

use std::collections::{HashMap, HashSet};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};

use ragent_agent::event::EventBus;
use ragent_agent::task::{TaskEntry, TaskStatus};

use crate::app::App;
use crate::layout_table_rows::{band_line, cursor_line};
use crate::theme::colors;
use crate::utils::{format_elapsed, short_id};

/// Every structural and status glyph the Agents table paints (NFR-003).
///
/// The renderer takes each of these from here rather than spelling the
/// character inline, so the header, the agent rows and the background shell
/// section can never drift apart.
pub mod glyphs {
    /// Marker painted on the primary-agent row (FR-010).
    pub const PRIMARY: &str = "* ";
    /// Status-dot marker painted on a sub-agent row (FR-012) and on a
    /// background shell task row (FR-016).
    ///
    /// Kept ASCII on purpose: the table tests slice painted rows by byte
    /// offset, so every table glyph must render one byte per cell.
    pub const ROW_DOT: &str = "o ";
    /// Branch glyph for a task that is the last child of its parent (FR-011).
    pub const BRANCH_LAST: &str = "`- ";
    /// Branch glyph for a task that has a later sibling (FR-011).
    pub const BRANCH_MID: &str = "|- ";
    /// Two-column continuation segment for an ancestor that has a later
    /// sibling (FR-011).
    pub const TRUNK: &str = "| ";
    /// Two-column continuation segment for an ancestor that was a last child
    /// (FR-011).
    pub const TRUNK_BLANK: &str = "  ";
    /// Badge appended to a row whose agent is a custom definition (FR-013).
    pub const BADGE_CUSTOM: &str = "[C]";
    /// Badge appended to a row whose session is an active-team member (FR-013).
    pub const BADGE_TEAM: &str = "[T]";
    /// Suspend control painted on a live row (FR-015).
    pub const STOP: &str = "[stop]";
    /// Resume control painted in place of [`STOP`] while suspended (FR-015).
    pub const RESUME: &str = ">";
    /// Kill control painted on a live row (FR-015).
    pub const KILL: &str = "[x]";
    /// Suffix painted after a suspended row's label (FR-014).
    pub const STATUS_SUSPENDED: &str = " [||]";
    /// Suffix painted after a terminating row's label (FR-014).
    pub const STATUS_TERMINATING: &str = " ...";
    /// Suffix painted after a row whose per-call time bound expired (FR-017).
    ///
    /// A bound expiry is a terminal outcome of its own, not a failure, so the
    /// row must not read as a plain error: the suffix names the cause.
    pub const STATUS_TIMED_OUT: &str = " [timeout]";
    /// Suffix painted after a live row held by one or more in-flight
    /// `wait_agents` callers (FR-012).
    ///
    /// The count is appended after this stem and closed with a `]` (rendering
    /// `[w2]`), because two waiters is a different situation from one and the
    /// number decides whether another wait adds value. Kept ASCII for the same
    /// byte-per-cell reason as [`ROW_DOT`].
    pub const STATUS_WAITERS: &str = " [w";
    /// Heading that opens the background shell section (FR-016).
    pub const BG_HEADING: &str = " Background shell tasks ";
    /// Heading that opens the completed-history section (FR-021).
    pub const HISTORY_HEADING: &str = " Completed (newest first) ";
    /// Marker painted on a history row whose report has no durable copy
    /// (FR-032).
    ///
    /// A one-word flag so the condition is visible in the fixed columns even
    /// when the explanatory note is scrolled out of the row's visible width;
    /// ASCII for the same byte-per-cell reason as [`ROW_DOT`].
    pub const REPORT_IN_MEMORY: &str = "[memory]";
    /// Type-column label for a row that blocks its caller (FR-020).
    pub const CONTRACT_FOREGROUND: &str = "fg";
    /// Type-column label for an awaited background row (FR-020).
    pub const CONTRACT_BACKGROUND: &str = "bg";
    /// Type-column label for a fire-and-forget background row (FR-020).
    ///
    /// `detached` is deliberately narrower than the column so it can never
    /// push the elapsed-time column, and it is spelled out rather than
    /// abbreviated (`dt`) because it is the one contract a reader cannot
    /// infer from the row's behaviour.
    pub const CONTRACT_DETACHED: &str = "detached";
    /// Hint painted beneath the primary row when no sub-agent, background-shell
    /// task or completed-history row exists (spec `agentui` FR-026).
    ///
    /// The panel always opens - the operator asked to see it - so an empty
    /// table says what would fill it and how to start one rather than leaving a
    /// lone primary row that reads as a rendering fault. It mirrors the Teams
    /// window's empty state, and is ASCII for the same byte-per-cell reason as
    /// [`ROW_DOT`].
    pub const EMPTY_HINT: &str =
        "(no sub-agents yet - start one with /spawn or a background command)";
}

/// Column layout of the Agents table (FR-009 through FR-016).
///
/// Painted rows indent behind [`columns::CONTENT_X`] blank columns (FR-073),
/// and the control columns are chosen so the FR-015 controls sit where the
/// spec pins them in that indented row space: the six-column `[stop]` control
/// starts at [`columns::STOP_X`] and the four-column `[x]` control at
/// [`columns::KILL_X`]. The compile-time assertion below fails the build if
/// an edit to any width moves them.
pub mod columns {
    /// Width of the leading status-marker segment (`"o "` / `"* "`).
    pub const MARKER_W: usize = 2;
    /// Width of the task identifier column (FR-012).
    pub const ID_W: usize = 10;
    /// Columns separating two adjacent fixed columns.
    pub const GAP_W: usize = 1;
    /// Width of the agent label column, badges and status suffix included
    /// (FR-012, FR-013).
    ///
    /// One column narrower than the 32-column budget the label itself is
    /// allowed, so the type column can hold the widest contract (`detached`)
    /// plus its separating column without moving the pinned controls.
    pub const LABEL_W: usize = 31;
    /// Width of the type column, which paints the row's execution contract
    /// (`fg` / `bg` / `detached`) (FR-012, FR-020).
    ///
    /// Sized for the widest contract (`detached`, eight columns) plus the
    /// separating column, so no contract can push the elapsed-time column.
    pub const TYPE_W: usize = 9;
    /// Width of the right-aligned elapsed-time column (FR-012, FR-016).
    pub const ELAPSED_W: usize = 8;
    /// Width of the right-aligned step-count column (FR-012).
    pub const STEPS_W: usize = 7;
    /// Blank columns painted before every table line so the content indents
    /// into the panel (FR-073). Hit rectangles live in the same indented row
    /// space, so the control columns below are measured after this margin.
    pub const CONTENT_X: usize = 5;
    /// First column of the `[stop]` control behind the leading margin (FR-015).
    pub const STOP_X: usize = 70;
    /// Width of the `[stop]` control column (FR-015).
    pub const STOP_W: usize = 6;
    /// First column of the `[x]` control behind the leading margin (FR-015).
    pub const KILL_X: usize = 77;
    /// Width of the `[x]` control column (FR-015).
    pub const KILL_W: usize = 4;
    /// Columns a background shell command may occupy before truncation
    /// (FR-016). The narrower label column bounds it further.
    pub const BG_COMMAND_MAX_W: usize = 40;
}

/// The fixed columns must add up to the pinned `[stop]` column (FR-015).
const _: () = assert!(
    columns::MARKER_W
        + columns::ID_W
        + columns::GAP_W
        + columns::LABEL_W
        + columns::TYPE_W
        + columns::ELAPSED_W
        + columns::GAP_W
        + columns::STEPS_W
        + columns::GAP_W
        == columns::STOP_X
);

/// Whether `status` is terminal, in which case the row carries no controls.
fn is_terminal(status: &TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
    )
}

/// An unbanded filler row: a blank line that reserves one row of the table
/// body so the band pattern continues over rows past the live list.
fn filler_line(banded: bool, width: usize) -> Line<'static> {
    if banded {
        band_line(Line::default(), width)
    } else {
        Line::default()
    }
}

/// Prepend the blank leading margin to one painted table line (FR-073).
///
/// The band and filler helpers keep padding to the full table width, so the
/// margin only shifts where the first glyph lands; the line's own style is
/// carried over unchanged. The margin span outlives every row's borrow, so the
/// merged line keeps the input line's lifetime.
fn margin_line(line: Line) -> Line {
    let mut spans = vec![Span::raw(" ".repeat(columns::CONTENT_X))];
    spans.extend(line.spans);
    Line::from(spans).style(line.style)
}

/// The status dot colour, row text colour and label suffix for `status`.
///
/// Running rows are yellow, suspended rows dark grey with a `" [||]"` suffix and
/// terminating rows red with a `" ..."` suffix (FR-014).
fn status_presentation(status: &TaskStatus) -> (Color, Color, &'static str) {
    match status {
        TaskStatus::Running => (Color::Yellow, Color::Yellow, ""),
        TaskStatus::Suspended => (Color::DarkGray, Color::DarkGray, glyphs::STATUS_SUSPENDED),
        TaskStatus::Terminating => (Color::Red, Color::Red, glyphs::STATUS_TERMINATING),
        TaskStatus::Completed => (Color::Cyan, Color::Cyan, ""),
        TaskStatus::Failed => (Color::Red, Color::Red, ""),
        TaskStatus::Cancelled => (Color::DarkGray, Color::DarkGray, ""),
        // A per-call bound expiry is not a failure of the agent's work: it is
        // the caller's budget being reached (FR-017). Painted in the warning
        // colour with a suffix that names the cause, so it is never mistaken
        // for a provider error.
        TaskStatus::TimedOut => (Color::Yellow, Color::Yellow, glyphs::STATUS_TIMED_OUT),
    }
}

/// Truncate `text` to at most `budget` characters, marking the cut with `..`.
///
/// The cut is taken on a character boundary so a multi-byte sequence can never
/// be split, and the result never exceeds the budget (NFR-004).
fn fit_chars(text: &str, budget: usize) -> String {
    if text.chars().count() <= budget {
        return text.to_string();
    }
    match budget {
        0 => String::new(),
        1 | 2 => text.chars().take(budget).collect(),
        _ => {
            let head: String = text.chars().take(budget - 2).collect();
            format!("{head}..")
        }
    }
}

/// Build the ancestor-indented, badge-suffixed agent label for one task row.
///
/// The label is one two-column segment per ancestor followed by the last-child
/// or non-last-child branch glyph (FR-011), then the task identifier, then the
/// `[C]`/`[T]` badges (FR-013) and the status suffix (FR-014). It never exceeds
/// [`columns::LABEL_W`] display columns - one of them is left blank so the type
/// column stays separated - and the identifier is truncated when the reserved
/// suffix leaves no room (NFR-004).
fn agent_label(
    task: &TaskEntry,
    last_stack: &[bool],
    is_last: bool,
    custom_names: &HashSet<String>,
    teammate_ids: &HashSet<String>,
    status_suffix: &str,
) -> String {
    let mut prefix = String::new();
    for &ancestor_was_last in last_stack {
        prefix.push_str(if ancestor_was_last {
            glyphs::TRUNK_BLANK
        } else {
            glyphs::TRUNK
        });
    }
    prefix.push_str(if is_last {
        glyphs::BRANCH_LAST
    } else {
        glyphs::BRANCH_MID
    });

    // Badges reserve their own columns inside the label (FR-013).
    let mut suffix = String::new();
    if custom_names.contains(&task.agent_name) {
        suffix.push(' ');
        suffix.push_str(glyphs::BADGE_CUSTOM);
    }
    if teammate_ids.contains(&task.child_session_id) {
        suffix.push(' ');
        suffix.push_str(glyphs::BADGE_TEAM);
    }
    suffix.push_str(status_suffix);

    // Fit the identifier into what the structural prefix and the reserved
    // suffix leave, so the finished label can never exceed its column and the
    // badges are never the part that gets dropped (NFR-004). One column is held
    // back so a full-width label still leaves the type column separated.
    let used = prefix.chars().count() + suffix.chars().count();
    let budget = columns::LABEL_W.saturating_sub(used).saturating_sub(1);
    let name = fit_chars(&task.id, budget);
    format!("{prefix}{name}{suffix}")
}

/// The execution contract a sub-agent row paints in the type column (FR-020).
///
/// Derived from the row's own registry entry - never from a hard-coded
/// default - because the three contracts are not recoverable from appearance:
/// a detached run sets `background` too, so reading `background` alone would
/// paint it as an ordinary awaited background task. Detached is therefore
/// tested first.
fn execution_contract_label(task: &TaskEntry) -> &'static str {
    if task.detached {
        glyphs::CONTRACT_DETACHED
    } else if task.background {
        glyphs::CONTRACT_BACKGROUND
    } else {
        glyphs::CONTRACT_FOREGROUND
    }
}

/// The waiter marker a live task row appends to its label (FR-012), or `None`
/// when no caller is waiting on it.
///
/// `waiter_count` is the registry's tally of in-flight `wait_agents` callers.
/// It is painted only when greater than zero - a row nobody awaits must not
/// carry a `[w0]` that reads as a defect - and only for a row that has not
/// finished, because a finished task's count is the residue of a wait that
/// ended with the task rather than a queue of callers.
fn waiter_suffix(task: &TaskEntry) -> Option<String> {
    if task.waiter_count == 0 || task.completed_at.is_some() {
        return None;
    }
    Some(format!("{}{}]", glyphs::STATUS_WAITERS, task.waiter_count))
}

/// Build the trailing `[stop]` / `[x]` control spans for a task row.
///
/// A terminal row paints nothing (FR-015). A live row paints the control at
/// [`columns::STOP_X`] padded to [`columns::STOP_W`] - `>` instead of `[stop]`
/// while suspended, padded so `[x]` never moves - then `[x]` at
/// [`columns::KILL_X`].
fn control_spans(live: bool, suspended: bool) -> Vec<Span<'static>> {
    if !live {
        return Vec::new();
    }
    let (glyph, fg) = if suspended {
        (glyphs::RESUME, Color::Green)
    } else {
        (glyphs::STOP, Color::Yellow)
    };
    vec![
        Span::styled(
            format!("{glyph:<width$}", width = columns::STOP_W),
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", Style::default()),
        Span::styled(
            glyphs::KILL,
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
    ]
}

/// How one task row's leading fixed columns are styled.
#[derive(Debug, Clone, Copy)]
struct RowStyle {
    /// Colour of the leading status marker (`*` / `o`).
    marker: Color,
    /// Colour of the agent label and the type column (FR-014).
    row: Color,
    /// Whether the agent label paints in bold (the primary row, FR-010).
    bold_label: bool,
}

/// Build the fixed leading spans shared by every task row.
fn fixed_spans(
    marker: &'static str,
    style: RowStyle,
    id: &str,
    label: &str,
    type_label: &str,
    elapsed: &str,
    steps: &str,
) -> Vec<Span<'static>> {
    let mut label_style = Style::default().fg(style.row);
    if style.bold_label {
        label_style = label_style.add_modifier(Modifier::BOLD);
    }
    // The `primary` / `bg` / `fg` type word is metadata, not content: it paints
    // in the hint colour so the row colour belongs to the label alone.
    let type_style = Style::default().fg(colors::HINT);
    vec![
        Span::styled(marker, Style::default().fg(style.marker)),
        Span::styled(
            format!("{:<width$} ", id, width = columns::ID_W),
            Style::default().fg(colors::HINT),
        ),
        Span::styled(
            format!("{label:<width$}", width = columns::LABEL_W),
            label_style,
        ),
        Span::styled(
            format!("{type_label:<width$}", width = columns::TYPE_W),
            type_style,
        ),
        Span::styled(
            format!("{elapsed:>width$}", width = columns::ELAPSED_W),
            Style::default().fg(colors::HINT),
        ),
        Span::styled(" ", Style::default()),
        Span::styled(
            format!("{steps:>width$}", width = columns::STEPS_W),
            Style::default().fg(colors::HINT),
        ),
        Span::styled(" ", Style::default()),
    ]
}

/// The dimmed header row naming every column (FR-009).
fn header_line() -> Line<'static> {
    let dim = Style::default()
        .fg(colors::HINT)
        .add_modifier(Modifier::DIM)
        .add_modifier(Modifier::BOLD);
    Line::from(vec![
        Span::styled(" ".repeat(columns::MARKER_W), dim),
        Span::styled(format!("{:<width$} ", "id", width = columns::ID_W), dim),
        Span::styled(format!("{:<width$}", "name", width = columns::LABEL_W), dim),
        Span::styled(format!("{:<width$}", "type", width = columns::TYPE_W), dim),
        Span::styled(
            format!("{:>width$}", "elapsed", width = columns::ELAPSED_W),
            dim,
        ),
        Span::styled(" ", dim),
        Span::styled(
            format!("{:>width$}", "steps", width = columns::STEPS_W),
            dim,
        ),
        Span::styled(" ", dim),
        Span::styled(
            format!("{:<width$}", glyphs::STOP, width = columns::STOP_W),
            dim,
        ),
        Span::styled(" ", dim),
        Span::styled(
            format!("{:<width$}", glyphs::KILL, width = columns::KILL_W),
            dim,
        ),
    ])
}

/// Recursively build the sub-agent tree rows.
///
/// `last_stack` records, for every ancestor of the current row, whether that
/// ancestor was its parent's last child; each entry contributes one two-column
/// tree segment (FR-011). `row_task_ids` receives the task id of every
/// sub-agent row pushed to `out`, in paint order, so the keyboard cursor
/// ordinal (`agents_cursor_subagent`) resolves to the same rows the table
/// paints (FR-034).
fn build_task_rows<'a>(
    tasks_map: &HashMap<&'a str, Vec<&'a TaskEntry>>,
    parent_sid: &str,
    last_stack: &[bool],
    event_bus: &EventBus,
    custom_names: &HashSet<String>,
    teammate_ids: &HashSet<String>,
    out: &mut Vec<Line<'a>>,
    row_task_ids: &mut Vec<String>,
) {
    let children = tasks_map.get(parent_sid).cloned().unwrap_or_default();
    for (idx, task) in children.iter().enumerate() {
        let is_last = idx + 1 == children.len();
        let (dot_color, row_color, status_suffix) = status_presentation(&task.status);
        // FR-012: a live row held by one or more in-flight `wait_agents`
        // callers names how many, appended after the status suffix so both
        // facts stay readable and neither can be dropped by truncation.
        let suffix = match waiter_suffix(task) {
            Some(waiters) => format!("{status_suffix}{waiters}"),
            None => status_suffix.to_string(),
        };
        let label = agent_label(
            task,
            last_stack,
            is_last,
            custom_names,
            teammate_ids,
            &suffix,
        );
        let type_label = execution_contract_label(task);
        let steps = event_bus
            .current_tool_calls(&task.child_session_id)
            .to_string();
        let mut spans = fixed_spans(
            glyphs::ROW_DOT,
            RowStyle {
                marker: dot_color,
                row: row_color,
                bold_label: false,
            },
            &short_id(&task.id),
            &label,
            type_label,
            &format_elapsed(task.created_at),
            &steps,
        );
        // The `[stop]`/`[x]` columns stay painted as status indicators: the
        // keyboard Space/Del now act on the row, and the glyph shows what
        // each currently means (suspended rows paint `>` = resume).
        spans.extend(control_spans(
            !is_terminal(&task.status),
            task.status == TaskStatus::Suspended,
        ));

        out.push(Line::from(spans));
        row_task_ids.push(task.id.clone());

        let mut new_stack = last_stack.to_vec();
        new_stack.push(is_last);
        build_task_rows(
            tasks_map,
            &task.child_session_id,
            &new_stack,
            event_bus,
            custom_names,
            teammate_ids,
            out,
            row_task_ids,
        );
    }
}

/// Render the active-agents table into `area`.
///
/// Row 0 is the dimmed header (FR-009), row 1 the primary agent (FR-010), then
/// the sub-agent tree (FR-011 through FR-015) and the background shell section
/// (FR-016). The `[C]` and `[T]` badges are derived from the currently loaded
/// custom agent definitions and team members on every render, so they always
/// reflect the current configuration (FR-062).
pub fn render_active_agents_subpanel(frame: &mut Frame, app: &mut App, area: Rect) {
    app.agents_row_task_ids.clear();
    app.agents_subagent_task_ids.clear();

    let primary_session = app.session_id.clone().unwrap_or_default();
    let primary_name = app.agent_name.clone();
    let mut tasks_map: HashMap<&str, Vec<&TaskEntry>> = HashMap::new();
    for task in &app.active_tasks {
        tasks_map
            .entry(task.parent_session_id.as_str())
            .or_default()
            .push(task);
    }
    let primary_steps = app.event_bus.current_tool_calls(&primary_session);

    let custom_names: HashSet<String> = app
        .custom_agent_defs
        .iter()
        .map(|def| def.agent_info.name.clone())
        .collect();
    let teammate_ids: HashSet<String> = app
        .team_members
        .iter()
        .filter_map(|member| member.session_id.clone())
        .collect();
    let primary_label = {
        let mut label = primary_name.clone();
        if custom_names.contains(&primary_name) {
            label.push(' ');
            label.push_str(glyphs::BADGE_CUSTOM);
        }
        // One column is held back so a full-width name still leaves the type
        // column separated (FR-010, NFR-004).
        fit_chars(&label, columns::LABEL_W.saturating_sub(1))
    };

    let mut lines: Vec<Line> = Vec::new();
    // Click key per painted row, kept index-aligned with `lines` (T-008):
    // header and background rows open nothing, the primary row is the
    // `"primary"` sentinel, and a sub-agent row carries its task id.
    let mut row_keys: Vec<String> = Vec::new();
    lines.push(header_line());
    row_keys.push(String::new());

    // -- primary agent (FR-010) --------------------------------------------
    lines.push(Line::from(fixed_spans(
        glyphs::PRIMARY,
        RowStyle {
            marker: Color::Green,
            row: Color::Green,
            bold_label: true,
        },
        "lead",
        &primary_label,
        "primary",
        "-",
        &primary_steps.to_string(),
    )));
    row_keys.push("primary".to_string());

    // -- empty state (FR-026) ----------------------------------------------
    // Nothing but the primary row would be painted, so the table says what it
    // is waiting for: an otherwise bare modal reads as a rendering fault.
    if !app.agents_window_available() {
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", glyphs::EMPTY_HINT),
            Style::default()
                .fg(colors::HINT)
                .add_modifier(Modifier::DIM),
        )]));
        row_keys.push(String::new());
    }

    // -- sub-agents (FR-011 through FR-015) --------------------------------
    let subagent_row_start = row_keys.len();
    build_task_rows(
        &tasks_map,
        &primary_session,
        &[],
        &app.event_bus,
        &custom_names,
        &teammate_ids,
        &mut lines,
        &mut row_keys,
    );
    // The keyboard cursor is an ordinal over the painted sub-agent rows only:
    // publish the task ids in DFS paint order for the cursor's Space/Del/
    // Enter resolution and the scroll sync (FR-034).
    app.agents_subagent_task_ids = row_keys[subagent_row_start..].to_vec();

    // -- background shell tasks (FR-016) -----------------------------------
    if !app.bg_tasks.is_empty() {
        // The heading carries the window accent so the section reads as a
        // titled group, not as more rows (FR-016).
        lines.push(Line::from(vec![Span::styled(
            glyphs::BG_HEADING,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::DIM),
        )]));
        row_keys.push(String::new());
        let command_budget = columns::LABEL_W.min(columns::BG_COMMAND_MAX_W);
        for task in &app.bg_tasks {
            let (dot_color, row_color) = match task.status.as_str() {
                "running" => (Color::Yellow, Color::Yellow),
                "completed" => (Color::Cyan, Color::Cyan),
                "failed" | "cancelled" => (Color::Red, Color::Red),
                _ => (Color::DarkGray, Color::DarkGray),
            };
            let command = fit_chars(&task.command, command_budget);
            let mut spans = fixed_spans(
                glyphs::ROW_DOT,
                RowStyle {
                    marker: dot_color,
                    row: row_color,
                    bold_label: false,
                },
                &short_id(&task.id),
                &command,
                "bg",
                &format_elapsed(task.created_at),
                "-",
            );
            // The `[stop]`/`[x]` columns stay painted as status indicators:
            // the keyboard Space/Del act on sub-agent rows, but the old
            // background-shell cancel glyph keeps its meaning (FR-016).
            if task.status == "running" {
                spans.extend(control_spans(true, false));
            }
            lines.push(Line::from(spans));
            row_keys.push(String::new());
        }
    }

    // -- completed history (FR-021, FR-036) --------------------------------
    if !app.agents_history.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            glyphs::HISTORY_HEADING,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::DIM),
        )]));
        row_keys.push(String::new());
        // The deque is already newest-first and capped at
        // `agents_history_cap` when it is written (FR-036, NFR-004), so the
        // renderer only has to paint it.
        for row in &app.agents_history {
            let (dot_color, row_color, _) = status_presentation(&row.status);
            let contract = if row.detached {
                glyphs::CONTRACT_DETACHED
            } else if row.background {
                glyphs::CONTRACT_BACKGROUND
            } else {
                glyphs::CONTRACT_FOREGROUND
            };
            let label = format!("{} [{}]", row.agent_name, row.status.as_str());
            let mut spans = fixed_spans(
                glyphs::ROW_DOT,
                RowStyle {
                    marker: dot_color,
                    row: row_color,
                    bold_label: false,
                },
                &short_id(&row.id),
                &fit_chars(&label, columns::LABEL_W.saturating_sub(1)),
                contract,
                &ragent_types::strutil::format_duration_compact(row.duration_secs() as i64),
                "-",
            );
            // FR-032: a finished task whose report was never persisted says so
            // instead of leaving the report column blank, which a reader would
            // take to mean "no report file needed".
            if let Some(note) = row.durable_copy_absence() {
                spans.push(Span::styled(
                    format!("{} ", glyphs::REPORT_IN_MEMORY),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(note, Style::default().fg(Color::Yellow)));
            }
            lines.push(Line::from(spans));
            row_keys.push(String::new());
        }
    }

    let total_lines = lines.len() as u16;
    let visible = area.height;
    let max_scroll = total_lines.saturating_sub(visible);
    app.active_agents_max_scroll = max_scroll;
    let scroll = app.active_agents_scroll_offset.min(max_scroll);
    app.agents_row_task_ids = row_keys.clone();

    // Paint the block cursor over the selected sub-agent row, then keep the
    // viewport on that row: when the user pages past the edge the offset
    // follows the cursor instead of desynchronising (FR-034).
    let mut cursor_painted_row: Option<u16> = None;
    if let Some(cursor) = app.agents_cursor_subagent
        && let Some(task_id) = app.agents_subagent_task_ids.get(cursor)
    {
        if let Some(row_position) = row_keys.iter().position(|key| key == task_id) {
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
    let table_width = area.width as usize;
    let mut window: Vec<Line> = lines
        .into_iter()
        .skip(scroll as usize)
        .take(visible as usize)
        .enumerate()
        .map(|(index, line)| {
            let line = margin_line(line);
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
        window.push(filler_line(index % 2 == 1, table_width));
    }
    frame.render_widget(Paragraph::new(window), area);

    if total_lines > visible {
        let mut sb_state = ScrollbarState::new(max_scroll as usize).position(scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(colors::HINT));
        frame.render_stateful_widget(scrollbar, area, &mut sb_state);
    }
}

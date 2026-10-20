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
    /// Heading that opens the background shell section (FR-016).
    pub const BG_HEADING: &str = " Background shell tasks ";
}

/// Column layout of the Agents table, counted in display columns from the left
/// edge of the table area (FR-009 through FR-016).
///
/// The widths are chosen so the control columns land where FR-015 pins them:
/// the six-column `[stop]` control starts at [`STOP_X`] and the four-column
/// `[x]` control at [`KILL_X`]. The compile-time assertion below fails the
/// build if an edit to any width moves them.
pub mod columns {
    /// Width of the leading status-marker segment (`"o "` / `"* "`).
    pub const MARKER_W: usize = 2;
    /// Width of the task identifier column (FR-012).
    pub const ID_W: usize = 10;
    /// Columns separating two adjacent fixed columns.
    pub const GAP_W: usize = 1;
    /// Width of the agent label column, badges and status suffix included
    /// (FR-012, FR-013).
    pub const LABEL_W: usize = 32;
    /// Width of the `primary` / `bg` / `fg` type column (FR-012).
    pub const TYPE_W: usize = 8;
    /// Width of the right-aligned elapsed-time column (FR-012, FR-016).
    pub const ELAPSED_W: usize = 8;
    /// Width of the right-aligned step-count column (FR-012).
    pub const STEPS_W: usize = 7;
    /// First column of the `[stop]` control (FR-015).
    pub const STOP_X: usize = 70;
    /// Width of the `[stop]` control column (FR-015).
    pub const STOP_W: usize = 6;
    /// First column of the `[x]` control (FR-015).
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

/// Record one painted row's control hit rectangle / identifier pair.
///
/// A terminal row paints no control, so it records the `Rect::default()`
/// placeholder with an empty identifier instead of shrinking the vectors: the
/// placeholder can never match a click (zero width) and never resolves to a
/// task at all (empty identifier), while the vector stays index-aligned with
/// the painted rows (FR-015, FR-071, NFR-011). `row` is the row's painted line
/// index; the renderer stores it in `Rect::y` so hit-test placement shifts
/// directly with the paint instead of relying on button-order arithmetic.
fn record_controls(
    row: u16,
    live: bool,
    id: &str,
    button_areas: &mut Vec<Rect>,
    kill_areas: &mut Vec<Rect>,
    button_task_ids: &mut Vec<String>,
    kill_task_ids: &mut Vec<String>,
) {
    let (button, kill, name) = if live {
        (
            Rect::new(columns::STOP_X as u16, row, columns::STOP_W as u16, 1),
            Rect::new(columns::KILL_X as u16, row, columns::KILL_W as u16, 1),
            id.to_string(),
        )
    } else {
        // Terminal-row placeholder (FR-071): zero width and an empty
        // identifier make the entry unresolvable by construction.
        (Rect::default(), Rect::default(), String::new())
    };
    button_areas.push(button);
    kill_areas.push(kill);
    button_task_ids.push(name.clone());
    kill_task_ids.push(name);
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
            Style::default().fg(style.row),
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
        .add_modifier(Modifier::DIM);
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

/// Recursively build the sub-agent tree rows with their control columns.
///
/// `last_stack` records, for every ancestor of the current row, whether that
/// ancestor was its parent's last child; each entry contributes one two-column
/// tree segment (FR-011). `button_areas` / `kill_areas` receive one entry per
/// painted row, index-aligned with their identifier vectors (NFR-011).
/// `row_task_ids` receives the clicked task id of every row pushed to `out`,
/// in paint order, so a row click resolves to the task painted on that line
/// (FR-034, FR-069).
fn build_task_rows_with_buttons<'a>(
    tasks_map: &HashMap<&'a str, Vec<&'a TaskEntry>>,
    parent_sid: &str,
    last_stack: &[bool],
    event_bus: &EventBus,
    custom_names: &HashSet<String>,
    teammate_ids: &HashSet<String>,
    out: &mut Vec<Line<'a>>,
    row_task_ids: &mut Vec<String>,
    button_areas: &mut Vec<Rect>,
    kill_areas: &mut Vec<Rect>,
    button_task_ids: &mut Vec<String>,
    kill_task_ids: &mut Vec<String>,
) {
    let children = tasks_map.get(parent_sid).cloned().unwrap_or_default();
    for (idx, task) in children.iter().enumerate() {
        let is_last = idx + 1 == children.len();
        let (dot_color, row_color, status_suffix) = status_presentation(&task.status);
        let label = agent_label(
            task,
            last_stack,
            is_last,
            custom_names,
            teammate_ids,
            status_suffix,
        );
        let type_label = if task.background { "bg" } else { "fg" };
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

        let live = !is_terminal(&task.status);
        spans.extend(control_spans(live, task.status == TaskStatus::Suspended));
        record_controls(
            out.len() as u16,
            live,
            &task.id,
            button_areas,
            kill_areas,
            button_task_ids,
            kill_task_ids,
        );
        out.push(Line::from(spans));
        row_task_ids.push(task.id.clone());

        let mut new_stack = last_stack.to_vec();
        new_stack.push(is_last);
        build_task_rows_with_buttons(
            tasks_map,
            &task.child_session_id,
            &new_stack,
            event_bus,
            custom_names,
            teammate_ids,
            out,
            row_task_ids,
            button_areas,
            kill_areas,
            button_task_ids,
            kill_task_ids,
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
    app.agent_row_button_areas.clear();
    app.agent_row_button_task_ids.clear();
    app.agent_row_kill_areas.clear();
    app.agent_row_kill_task_ids.clear();
    app.agents_row_task_ids.clear();

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

    // -- sub-agents (FR-011 through FR-015) --------------------------------
    let mut button_areas: Vec<Rect> = Vec::new();
    let mut kill_areas: Vec<Rect> = Vec::new();
    let mut button_task_ids: Vec<String> = Vec::new();
    let mut kill_task_ids: Vec<String> = Vec::new();
    build_task_rows_with_buttons(
        &tasks_map,
        &primary_session,
        &[],
        &app.event_bus,
        &custom_names,
        &teammate_ids,
        &mut lines,
        &mut row_keys,
        &mut button_areas,
        &mut kill_areas,
        &mut button_task_ids,
        &mut kill_task_ids,
    );

    // -- background shell tasks (FR-016) -----------------------------------
    if !app.bg_tasks.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            glyphs::BG_HEADING,
            Style::default()
                .fg(colors::HINT)
                .add_modifier(Modifier::BOLD),
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
            // Background shell tasks are never suspended, only cancelled.
            let live = !matches!(task.status.as_str(), "completed" | "failed" | "cancelled");
            spans.extend(control_spans(live, false));
            record_controls(
                lines.len() as u16,
                live,
                &task.id,
                &mut button_areas,
                &mut kill_areas,
                &mut button_task_ids,
                &mut kill_task_ids,
            );
            lines.push(Line::from(spans));
            row_keys.push(String::new());
        }
    }

    let total_lines = lines.len() as u16;
    let visible = area.height;
    let max_scroll = total_lines.saturating_sub(visible);
    app.active_agents_max_scroll = max_scroll;
    let scroll = app.active_agents_scroll_offset.min(max_scroll);
    app.agents_row_task_ids = row_keys;

    // Render only the visible slice, mirroring the message-window pattern.
    let window: Vec<Line> = lines
        .into_iter()
        .skip(scroll as usize)
        .take(visible as usize)
        .collect();
    frame.render_widget(Paragraph::new(window), area);

    // Store button areas shifted by scroll and area position, keeping IDs in
    // sync 1:1 (`record_controls` pushes both in lockstep). Each Rect's `y`
    // holds the row's painted line index in scroll space (captured at build
    // time), so no button-order arithmetic is needed. A placeholder or an
    // out-of-view entry keeps its vector position as `Rect::default()`, which
    // can never match a click (FR-056, FR-071, NFR-011).
    app.agent_row_button_areas = shift_areas(&button_areas, area, scroll);
    app.agent_row_button_task_ids = button_task_ids;
    app.agent_row_kill_areas = shift_areas(&kill_areas, area, scroll);
    app.agent_row_kill_task_ids = kill_task_ids;

    if total_lines > visible {
        let mut sb_state = ScrollbarState::new(max_scroll as usize).position(scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(colors::HINT));
        frame.render_stateful_widget(scrollbar, area, &mut sb_state);
    }
}

/// Translate a row-space hit rectangle into screen space for `area`.
///
/// Returns `None` for a zero-width placeholder, a control scrolled out of view
/// and a control clipped by the table bottom, so a click can never resolve to
/// a control the user cannot see (FR-056, NFR-011).
fn shift_area(rect: Rect, area: Rect, scroll: u16) -> Option<Rect> {
    if rect.width == 0 || rect.y < scroll {
        return None;
    }
    let y = area.y + (rect.y - scroll);
    if y >= area.y + area.height {
        return None;
    }
    Some(Rect::new(area.x + rect.x, y, rect.width, 1))
}

/// Shift every control rectangle into screen space, keeping one default
/// placeholder per unplaceable entry so the vector stays index-aligned with
/// the painted rows and the identifier vector (NFR-011).
fn shift_areas(areas: &[Rect], area: Rect, scroll: u16) -> Vec<Rect> {
    areas
        .iter()
        .map(|rect| shift_area(*rect, area, scroll).unwrap_or_default())
        .collect()
}

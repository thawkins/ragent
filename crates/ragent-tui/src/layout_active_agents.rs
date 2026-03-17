//! Active-agents subpanel — shown at the bottom of the log panel.
//!
//! Displays the primary agent and all spawned sub-agents in a tree, with
//! each row showing: short session id, type (primary/background/foreground),
//! elapsed active time, and step count.

use chrono::Utc;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};
use ratatui::Frame;

use ragent_core::task::TaskEntry;

use crate::app::App;

/// Format a UTC timestamp as elapsed duration from now (e.g. "2m34s").
fn format_elapsed(created_at: chrono::DateTime<Utc>) -> String {
    let secs = (Utc::now() - created_at).num_seconds().max(0);
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Shorten a session/task id to the first 8 hex chars.
fn short_id(id: &str) -> &str {
    &id[..8.min(id.len())]
}

/// Recursively build agent row lines for tasks whose parent is `parent_sid`.
/// `depth` controls indentation; sub-agents appear below their spawner.
fn build_task_rows<'a>(
    tasks: &'a [TaskEntry],
    parent_sid: &str,
    depth: usize,
    event_bus: &ragent_core::event::EventBus,
    out: &mut Vec<Line<'a>>,
) {
    let indent = "  ".repeat(depth);
    let prefix = if depth == 0 { "○ " } else { "└ " };
    for task in tasks {
        if task.parent_session_id != parent_sid {
            continue;
        }
        let steps = event_bus.current_step(&task.child_session_id);
        let elapsed = format_elapsed(task.created_at);
        let kind_tag = if task.background { "[bg]" } else { "[fg]" };
        let agent_label = format!("{} {}", task.agent_name, kind_tag);
        let tid = short_id(&task.id);
        let (dot_color, name_color) = if task.background {
            (Color::Yellow, Color::Yellow)
        } else {
            (Color::Cyan, Color::Cyan)
        };

        out.push(Line::from(vec![
            Span::styled(
                format!("{indent}{prefix}"),
                Style::default().fg(dot_color),
            ),
            Span::styled(
                format!("{:<10} ", tid),
                Style::default().fg(name_color),
            ),
            Span::styled(
                format!("{:<15}", agent_label),
                Style::default().fg(name_color),
            ),
            Span::styled(
                format!(" {:>6}  ", elapsed),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("steps:{}", steps),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        // Recurse for sub-agents spawned by this task's session
        build_task_rows(tasks, &task.child_session_id.clone(), depth + 1, event_bus, out);
    }
}

/// Render the active-agents subpanel into `area` (8 rows including border).
pub fn render_active_agents_subpanel(frame: &mut Frame, app: &mut App, area: Rect) {
    // Snapshot data we need so we don't keep borrowing `app`.
    let primary_session = app.session_id.clone().unwrap_or_default();
    let primary_name = app.agent_name.clone();
    let tasks = app.active_tasks.clone();
    let primary_steps = app.event_bus.current_step(&primary_session);

    let mut lines: Vec<Line> = Vec::new();

    // ── header ─────────────────────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled(
            "  id         ",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "agent           ",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            " elapsed  ",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "steps",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
    ]));

    // ── primary agent ───────────────────────────────────────────────────────
    let pid = short_id(&primary_session);
    lines.push(Line::from(vec![
        Span::styled("● ", Style::default().fg(Color::Green)),
        Span::styled(
            format!("{:<10} ", pid),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<15}", primary_name),
            Style::default().fg(Color::Green),
        ),
        Span::styled(
            format!(" {:>6}  ", "-"),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("steps:{}", primary_steps),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    // ── sub-agents (depth 0 = direct children of primary) ──────────────────
    build_task_rows(&tasks, &primary_session, 0, &app.event_bus, &mut lines);

    // ── draw block + scroll ─────────────────────────────────────────────────
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Agents ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let block_inner = block.inner(area);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false }).block(block);

    let total_lines = paragraph.line_count(block_inner.width) as u16;
    let visible = block_inner.height;
    let max_scroll = total_lines.saturating_sub(visible);
    app.active_agents_max_scroll = max_scroll;
    let scroll = app.active_agents_scroll_offset.min(max_scroll);

    frame.render_widget(paragraph.scroll((scroll, 0)), area);

    if total_lines > visible {
        let mut sb_state = ScrollbarState::new(max_scroll as usize).position(scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_stateful_widget(scrollbar, area, &mut sb_state);
    }
}


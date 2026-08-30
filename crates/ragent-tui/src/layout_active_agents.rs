//! Active-agents subpanel — shown at the bottom of the log panel.
//!
//! Displays the primary agent and all spawned sub-agents in a tree, with
//! each row showing: agent name, type (primary/background/foreground),
//! elapsed active time, and step count.

use chrono::Utc;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::theme::colors;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};

use ragent_agent::task::{TaskEntry, TaskStatus};

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

/// Shorten a session/task id to the last 8 chars (the unique suffix).
fn short_id(id: &str) -> String {
    let start = id.len().saturating_sub(8);
    id[start..].to_string()
}

/// Recursively build agent row lines with Play/Stop and Kill button columns.
/// `button_areas` and `kill_areas` collect the column x-offsets for each row
/// so the TUI can do mouse hit-testing later.
fn build_task_rows_with_buttons<'a>(
    tasks_map: &std::collections::HashMap<&'a str, Vec<&'a TaskEntry>>,
    parent_sid: &str,
    depth: usize,
    last_stack: &[bool],
    event_bus: &ragent_agent::event::EventBus,
    custom_names: &std::collections::HashSet<String>,
    teammate_ids: &std::collections::HashSet<String>,
    out: &mut Vec<Line<'a>>,
    button_areas: &mut Vec<Rect>,
    kill_areas: &mut Vec<Rect>,
    button_task_ids: &mut Vec<String>,
    kill_task_ids: &mut Vec<String>,
) {
    let children = tasks_map.get(parent_sid).cloned().unwrap_or_default();
    for (idx, task) in children.iter().enumerate() {
        let is_last = idx + 1 == children.len();
        let mut indent = String::new();
        for &ancestor_was_last in last_stack {
            if ancestor_was_last {
                indent.push_str("  ");
            } else {
                indent.push_str("│ ");
            }
        }
        let prefix = if depth == 0 {
            "└─ "
        } else if is_last {
            "  └─ "
        } else {
            "  ├─ "
        };
        let steps = event_bus.current_tool_calls(&task.child_session_id);
        let elapsed = format_elapsed(task.created_at);
        let type_label = if task.background { "bg" } else { "fg" };
        let is_custom = custom_names.contains(&task.agent_name);
        let is_teammate = teammate_ids.contains(&task.child_session_id);
        // Display the unique task id (agent type + suffix) in the name column.
        // Reserve the 4-char badge width so [C]/[T] badges are not truncated.
        let badge_len = (if is_custom { 4 } else { 0 }) + (if is_teammate { 4 } else { 0 });
        let max_name = 28usize.saturating_sub(badge_len);
        let base_name: String = task.id.chars().take(max_name).collect();
        let mut agent_label = format!("{indent}{prefix}{base_name}");
        if is_custom {
            agent_label.push_str(" [C]");
        }
        if is_teammate {
            agent_label.push_str(" [T]");
        }
        let tid = short_id(&task.id);

        let (dot_color, name_color, status_badge) = match task.status {
            TaskStatus::Running => (Color::Yellow, Color::Yellow, ""),
            TaskStatus::Suspended => (Color::DarkGray, Color::DarkGray, " ⏸"),
            TaskStatus::Terminating => (Color::Red, Color::Red, " …"),
            _ => (Color::Cyan, Color::Cyan, ""),
        };

        let btn_char = if task.status == TaskStatus::Suspended {
            "▷"
        } else {
            "⏹"
        };
        let btn_fg = if task.status == TaskStatus::Suspended {
            Color::Green
        } else {
            Color::Yellow
        };
        let btn_style = Style::default().fg(btn_fg).add_modifier(Modifier::BOLD);
        let kill_style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);

        let mut spans = vec![
            Span::styled("◦ ", Style::default().fg(dot_color)),
            Span::styled(format!("{:<10} ", tid), Style::default().fg(colors::HINT)),
            Span::styled(
                format!("{:<28}", agent_label),
                Style::default().fg(name_color),
            ),
            Span::styled(
                format!("{:<8} ", type_label),
                Style::default().fg(name_color),
            ),
            Span::styled(
                format!("{:>8} ", elapsed),
                Style::default().fg(colors::HINT),
            ),
            Span::styled(format!("{:>7}", steps), Style::default().fg(colors::HINT)),
        ];

        if !status_badge.is_empty() {
            spans.push(Span::styled(
                status_badge,
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Only show buttons for non-terminal tasks.
        let is_terminal = matches!(
            task.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        );
        if !is_terminal {
            spans.push(Span::styled("  ", Style::default()));
            spans.push(Span::styled(btn_char, btn_style));
            spans.push(Span::styled("  ", Style::default()));
            spans.push(Span::styled("✕", kill_style));

            // Compute x positions from cumulative column widths.
            // Pre-button fixed columns: "◦ "(2) + id(11) + name(28)
            //   + type(9) + elapsed(9) + steps(7) = 66.
            // Status badge adds 2-3 display cols; buttons follow.
            // Use generous click areas that work regardless of badge.
            // `out.len()` is the line index this row will occupy (the
            // row is pushed to `out` below the button push). Store it in
            // `Rect::y` so hit-test placement shifts directly by scroll
            // instead of relying on button-order arithmetic.
            let btn_x: u16 = 66;
            let kill_x: u16 = 72;
            let row = out.len() as u16;
            button_areas.push(Rect::new(btn_x, row, 6, 1));
            kill_areas.push(Rect::new(kill_x, row, 4, 1));
            button_task_ids.push(task.id.clone());
            kill_task_ids.push(task.id.clone());
        } else {
            button_areas.push(Rect::default());
            kill_areas.push(Rect::default());
            button_task_ids.push(String::new());
            kill_task_ids.push(String::new());
        }
        out.push(Line::from(spans));

        let mut new_stack = last_stack.to_vec();
        new_stack.push(is_last);
        build_task_rows_with_buttons(
            tasks_map,
            &task.child_session_id,
            depth + 1,
            &new_stack,
            event_bus,
            custom_names,
            teammate_ids,
            out,
            button_areas,
            kill_areas,
            button_task_ids,
            kill_task_ids,
        );
    }
}

/// Render the active-agents subpanel into `area` (8 rows including border).
pub fn render_active_agents_subpanel(frame: &mut Frame, app: &mut App, area: Rect) {
    app.agent_row_button_areas.clear();
    app.agent_row_button_task_ids.clear();
    app.agent_row_kill_areas.clear();
    app.agent_row_kill_task_ids.clear();

    let primary_session = app.session_id.clone().unwrap_or_default();
    let primary_name = app.agent_name.clone();
    let mut tasks_map: std::collections::HashMap<&str, Vec<&TaskEntry>> =
        std::collections::HashMap::new();
    for task in &app.active_tasks {
        tasks_map
            .entry(&task.parent_session_id[..])
            .or_default()
            .push(task);
    }
    let primary_steps = app.event_bus.current_tool_calls(&primary_session);

    let custom_names: std::collections::HashSet<String> = app
        .custom_agent_defs
        .iter()
        .map(|d| d.agent_info.name.clone())
        .collect();
    let teammate_ids: std::collections::HashSet<String> = app
        .team_members
        .iter()
        .filter_map(|m| m.session_id.clone())
        .collect();
    let primary_is_custom = custom_names.contains(&primary_name);

    let mut lines: Vec<Line> = Vec::new();

    // ── header (with button columns) ──────────────────────────────────────
    lines.push(Line::from(vec![Span::styled(
        format!(
            "  {:<10} {:<28}{:<8} {:>8} {:>7}  {:>3} {:>3}",
            "id", "name", "type", "elapsed", "steps", "▷⏹", "✕"
        ),
        Style::default()
            .fg(colors::HINT)
            .add_modifier(Modifier::DIM),
    )]));

    // ── primary agent ─────────────────────────────────────────────────────
    let mut primary_spans = vec![
        Span::styled("● ", Style::default().fg(Color::Green)),
        Span::styled(
            format!("{:<10} ", "lead"),
            Style::default().fg(colors::HINT),
        ),
        Span::styled(
            format!("{:<28}", primary_name),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<8} ", "primary"),
            Style::default().fg(Color::Green),
        ),
        Span::styled(format!("{:>8} ", "-"), Style::default().fg(colors::HINT)),
        Span::styled(
            format!("{:>7}", primary_steps),
            Style::default().fg(colors::HINT),
        ),
    ];
    if primary_is_custom {
        primary_spans.push(Span::styled(
            " [C]",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));
    }
    lines.push(Line::from(primary_spans));

    // ── sub-agents ─────────────────────────────────────────────────────────
    let mut button_areas: Vec<Rect> = Vec::new();
    let mut kill_areas: Vec<Rect> = Vec::new();
    let mut button_task_ids: Vec<String> = Vec::new();
    let mut kill_task_ids: Vec<String> = Vec::new();
    build_task_rows_with_buttons(
        &tasks_map,
        &primary_session,
        0,
        &[],
        &app.event_bus,
        &custom_names,
        &teammate_ids,
        &mut lines,
        &mut button_areas,
        &mut kill_areas,
        &mut button_task_ids,
        &mut kill_task_ids,
    );

    // ── background shell tasks (bg tool) ─────────────────────────────────
    if !app.bg_tasks.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            " Background shell tasks ",
            Style::default()
                .fg(colors::HINT)
                .add_modifier(Modifier::BOLD),
        )]));
        for task in &app.bg_tasks {
            let tid = short_id(&task.id);
            let elapsed = format_elapsed(task.created_at);
            let status_str = task.status.as_str();
            let (dot_color, name_color) = match status_str {
                "running" => (Color::Yellow, Color::Yellow),
                "completed" => (Color::Cyan, Color::Cyan),
                "failed" | "cancelled" => (Color::Red, Color::Red),
                _ => (Color::DarkGray, Color::DarkGray),
            };
            let command_label = {
                let s = task.command.clone();
                if s.chars().count() > 24 {
                    let truncated: String = s.chars().take(21).collect();
                    format!("{}...", truncated)
                } else {
                    s
                }
            };
            let mut spans = vec![
                Span::styled("◦ ", Style::default().fg(dot_color)),
                Span::styled(format!("{:<10} ", tid), Style::default().fg(colors::HINT)),
                Span::styled(
                    format!("{:<28}", command_label),
                    Style::default().fg(name_color),
                ),
                Span::styled(format!("{:<8} ", "bg"), Style::default().fg(name_color)),
                Span::styled(
                    format!("{:>8} ", elapsed),
                    Style::default().fg(colors::HINT),
                ),
                Span::styled(format!("{:>7}", "-"), Style::default().fg(colors::HINT)),
            ];

            let is_terminal = matches!(status_str, "completed" | "failed" | "cancelled");
            if !is_terminal {
                spans.push(Span::styled("  ", Style::default()));
                spans.push(Span::styled(
                    "⏹",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("  ", Style::default()));
                spans.push(Span::styled(
                    "✕",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ));

                let btn_x: u16 = 66;
                let kill_x: u16 = 72;
                // The row is pushed to `lines` after the button push, so
                // `lines.len()` is the row's painted line index. Store it in
                // `Rect::y` so hit-test placement shifts directly by scroll
                // instead of relying on button-order arithmetic.
                let row = lines.len() as u16;
                button_areas.push(Rect::new(btn_x, row, 6, 1));
                kill_areas.push(Rect::new(kill_x, row, 4, 1));
                button_task_ids.push(task.id.clone());
                kill_task_ids.push(task.id.clone());
            } else {
                button_areas.push(Rect::default());
                kill_areas.push(Rect::default());
                button_task_ids.push(String::new());
                kill_task_ids.push(String::new());
            }
            lines.push(Line::from(spans));
        }
    }

    let total_lines = lines.len() as u16;
    let visible = area.height;
    let max_scroll = total_lines.saturating_sub(visible);
    app.active_agents_max_scroll = max_scroll;
    let scroll = app.active_agents_scroll_offset.min(max_scroll);

    // Render only the visible slice, mirroring the message-window pattern.
    let window: Vec<Line> = lines
        .into_iter()
        .skip(scroll as usize)
        .take(visible as usize)
        .collect();
    let paragraph = Paragraph::new(window);
    frame.render_widget(paragraph, area);

    // Store button areas shifted by scroll and area position, keeping IDs in
    // sync. Each Rect's `y` holds the row's painted line index in scroll
    // space (captured at build time), so no button-order arithmetic is
    // needed and banner rows cannot desync hit-rects from painted rows.
    let mut shifted_button_areas: Vec<Rect> = Vec::new();
    let mut shifted_button_task_ids: Vec<String> = Vec::new();
    for (r, task_id) in button_areas.iter().zip(button_task_ids.iter()) {
        if r.width == 0 || r.y < scroll {
            continue;
        }
        let y = area.y + (r.y - scroll);
        if y < area.y + area.height {
            shifted_button_areas.push(Rect::new(area.x + r.x, y, r.width, 1));
            shifted_button_task_ids.push(task_id.clone());
        }
    }
    let mut shifted_kill_areas: Vec<Rect> = Vec::new();
    let mut shifted_kill_task_ids: Vec<String> = Vec::new();
    for (r, task_id) in kill_areas.iter().zip(kill_task_ids.iter()) {
        if r.width == 0 || r.y < scroll {
            continue;
        }
        let y = area.y + (r.y - scroll);
        if y < area.y + area.height {
            shifted_kill_areas.push(Rect::new(area.x + r.x, y, r.width, 1));
            shifted_kill_task_ids.push(task_id.clone());
        }
    }
    app.agent_row_button_areas = shifted_button_areas;
    app.agent_row_button_task_ids = shifted_button_task_ids;
    app.agent_row_kill_areas = shifted_kill_areas;
    app.agent_row_kill_task_ids = shifted_kill_task_ids;

    if total_lines > visible {
        let mut sb_state = ScrollbarState::new(max_scroll as usize).position(scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(colors::HINT));
        frame.render_stateful_widget(scrollbar, area, &mut sb_state);
    }
}

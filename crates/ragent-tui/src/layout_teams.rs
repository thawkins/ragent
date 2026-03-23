//! Teams subpanel — shown at the bottom of the log panel when a team is active.
//!
//! Renders the active team as a compact table with lead + teammates, including
//! status, elapsed time, step count, and current task.

use chrono::{DateTime, Utc};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};

use ragent_core::team::MemberStatus;

use crate::app::App;

/// Shorten an agent ID to the first 8 chars.
fn short_id(id: &str) -> &str {
    &id[..8.min(id.len())]
}

/// Format elapsed duration from a UTC timestamp (e.g. "2m34s").
fn format_elapsed(created_at: DateTime<Utc>) -> String {
    let secs = (Utc::now() - created_at).num_seconds().max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Map a `MemberStatus` to a coloured status label.
fn status_label(status: &MemberStatus) -> (&'static str, Color) {
    match status {
        MemberStatus::Spawning => ("spawning", Color::Yellow),
        MemberStatus::Working => ("working", Color::Cyan),
        MemberStatus::Idle => ("idle", Color::Green),
        MemberStatus::PlanPending => ("planning", Color::Magenta),
        MemberStatus::ShuttingDown => ("stopping", Color::Yellow),
        MemberStatus::Stopped => ("stopped", Color::DarkGray),
    }
}

/// Render the Teams subpanel into `area`.
///
/// Shows the active team name, the lead (current session), and all known
/// teammates with their status and current task.
pub fn render_teams_subpanel(frame: &mut Frame, app: &mut App, area: Rect) {
    app.refresh_team_member_session_ids();
    let team_name = app
        .active_team
        .as_ref()
        .map(|t| t.name.clone())
        .unwrap_or_else(|| "team".to_string());

    let members = app.team_members.clone();
    let lead_session = app.session_id.clone().unwrap_or_default();
    let lead_steps = app.event_bus.current_step(&lead_session);
    let lead_elapsed = app
        .storage
        .get_session(&lead_session)
        .ok()
        .flatten()
        .and_then(|s| DateTime::parse_from_rfc3339(&s.created_at).ok())
        .map(|dt| format_elapsed(dt.with_timezone(&Utc)))
        .unwrap_or_else(|| "-".to_string());

    let mut lines: Vec<Line> = Vec::new();

    // ── header ────────────────────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {:<12} ", "id"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            format!("{:<12} ", "name"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            format!("{:<10} ", "role"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            format!("{:<10} ", "status"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            format!("{:>8} ", "elapsed"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            format!("{:>7} ", "steps"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            format!("{:>9} ", "msgs"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "task",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
    ]));

    // ── lead row ─────────────────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("● ", Style::default().fg(Color::Green)),
        Span::styled(
            format!("{:<12} ", "lead"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<12} ", app.agent_name.as_str()),
            Style::default().fg(Color::Green),
        ),
        Span::styled(format!("{:<10} ", "lead"), Style::default().fg(Color::Green)),
        Span::styled(format!("{:<10} ", "active"), Style::default().fg(Color::Green)),
        Span::styled(
            format!("{:>8} ", lead_elapsed),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{:>7} ", lead_steps),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(format!("{:>9} ", "-/-"), Style::default().fg(Color::DarkGray)),
        Span::styled("—", Style::default().fg(Color::DarkGray)),
    ]));

    // ── teammate rows ─────────────────────────────────────────────────────
    for member in &members {
        let (status_str, status_color) = status_label(&member.status);
        let id_short = short_id(&member.agent_id);
        let task_str = member
            .current_task_id
            .as_deref()
            .map(|t| format!("task:{}", &t[..8.min(t.len())]))
            .unwrap_or_else(|| "—".to_string());
        let mut steps = member
            .session_id
            .as_deref()
            .map(|sid| app.event_bus.current_step(sid))
            .unwrap_or(0);
        if steps == 0
            && let Some(sid) = member.session_id.as_deref()
            && let Ok(msgs) = app.storage.get_messages(sid)
        {
            let assistant_msgs = msgs
                .iter()
                .filter(|m| matches!(m.role, ragent_core::message::Role::Assistant))
                .count() as u64;
            steps = assistant_msgs.max(steps);
        }
        if steps == 0
            && let Some(task_id) = member.current_task_id.as_deref()
        {
            let task_logs = app
                .log_entries
                .iter()
                .filter(|entry| entry.message.contains(task_id))
                .count() as u64;
            steps = task_logs.max(steps);
        }
        let elapsed = member.created_at;
        let (sent, received) = app
            .team_message_counts
            .get(&member.agent_id)
            .copied()
            .unwrap_or((0, 0));

        let mut spans = vec![
            Span::styled("└ ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{:<12} ", id_short),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("{:<12} ", member.name.as_str()),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(format!("{:<10} ", "teammate"), Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:<10} ", status_str), Style::default().fg(status_color)),
            Span::styled(
                format!("{:>8} ", format_elapsed(elapsed)),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(format!("{:>7} ", steps), Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:>9} ", format!("{sent}/{received}")),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(task_str, Style::default().fg(Color::DarkGray)),
        ];

        // [T] badge
        spans.push(Span::styled(
            " [T]",
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ));

        lines.push(Line::from(spans));
    }

    // Show a hint when no teammates yet.
    if members.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (no teammates yet — use team_spawn tool)",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        )]));
    }

    let total_lines = lines.len() as u16;
    let visible_lines = area.height.saturating_sub(2); // 2 for border
    app.teams_max_scroll = total_lines.saturating_sub(visible_lines);
    app.teams_scroll_offset = app.teams_scroll_offset.min(app.teams_max_scroll);

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " 👥 Team: {}  (lead + {} teammate{}) ",
                    team_name,
                    members.len(),
                    if members.len() == 1 { "" } else { "s" }
                ))
                .title_style(
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                )
                .border_style(Style::default().fg(Color::Blue)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.teams_scroll_offset, 0));

    frame.render_widget(paragraph, area);

    if app.teams_max_scroll > 0 {
        let mut scrollbar_state = ScrollbarState::new(app.teams_max_scroll as usize)
            .position(app.teams_scroll_offset as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            area,
            &mut scrollbar_state,
        );
    }
}

//! Teams subpanel — shown at the bottom of the log panel when a team is active.
//!
//! Renders the active team tree: lead at the top, teammates below with their
//! current status, active task ID, and `[T]` badge.

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

/// Map a `MemberStatus` to a coloured status label.
fn status_label(status: &MemberStatus) -> (&'static str, Color) {
    match status {
        MemberStatus::Spawning    => ("spawning ", Color::Yellow),
        MemberStatus::Working     => ("working  ", Color::Cyan),
        MemberStatus::Idle        => ("idle     ", Color::Green),
        MemberStatus::PlanPending => ("planning ", Color::Magenta),
        MemberStatus::ShuttingDown => ("stopping", Color::Yellow),
        MemberStatus::Stopped     => ("stopped  ", Color::DarkGray),
    }
}

/// Render the Teams subpanel into `area`.
///
/// Shows the active team name, the lead (current session), and all known
/// teammates with their status and current task.
pub fn render_teams_subpanel(frame: &mut Frame, app: &mut App, area: Rect) {
    let team_name = app
        .active_team
        .as_ref()
        .map(|t| t.name.clone())
        .unwrap_or_else(|| "team".to_string());

    let members = app.team_members.clone();

    let mut lines: Vec<Line> = Vec::new();

    // ── header ────────────────────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {:<12} ", "agent-id"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            format!("{:<12} ", "name"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "status    task",
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
        Span::styled("primary ", Style::default().fg(Color::Green)),
    ]));

    // ── teammate rows ─────────────────────────────────────────────────────
    for member in &members {
        let (status_str, status_color) = status_label(&member.status);
        let id_short = short_id(&member.agent_id);
        let task_str = member
            .current_task_id
            .as_deref()
            .map(|t| format!(" task:{}", &t[..8.min(t.len())]))
            .unwrap_or_default();

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
            Span::styled(status_str, Style::default().fg(status_color)),
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
                .title(format!(" 👥 Team: {team_name} "))
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

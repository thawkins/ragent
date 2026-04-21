//! Teams subpanel — shown at the bottom of the log panel when a team is active.
//!
//! Renders the active team as a compact table with lead + teammates, including
//! status, elapsed time, step count, and tasks claimed/completed.

use crate::theme::{SPACING_SM, colors};
use chrono::{DateTime, Utc};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};

use ragent_team::team::{MemberStatus, TaskStatus, TaskStore, TeamStore};

use crate::app::App;

/// Shorten an agent ID to the first 8 chars.
fn short_id(id: &str) -> String {
    let start = id.len().saturating_sub(8);
    id[start..].to_string()
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
        MemberStatus::Blocked => ("blocked", colors::HINT),
        MemberStatus::ShuttingDown => ("stopping", Color::Yellow),
        MemberStatus::Stopped => ("stopped", colors::HINT),
        MemberStatus::Failed => ("failed", Color::Red),
    }
}

/// Render the Teams subpanel into `area`.
///
/// Shows the active team name, the lead (current session), and all known
/// teammates with their status, elapsed time, step count, tasks claimed/done.
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

    // Load task counts per agent from the team task store.
    let working_dir = std::env::current_dir().unwrap_or_default();
    let task_counts: std::collections::HashMap<String, (usize, usize)> = app
        .active_team
        .as_ref()
        .and_then(|t| {
            TeamStore::load_by_name(&t.name, &working_dir)
                .ok()
                .and_then(|s| TaskStore::open(&s.dir).ok())
                .and_then(|ts| ts.read().ok())
                .map(|list| {
                    let mut counts: std::collections::HashMap<String, (usize, usize)> =
                        std::collections::HashMap::new();
                    for task in &list.tasks {
                        if let Some(agent) = &task.assigned_to {
                            let entry = counts.entry(agent.clone()).or_default();
                            entry.0 += 1; // claimed
                            if task.status == TaskStatus::Completed {
                                entry.1 += 1; // completed
                            }
                        }
                    }
                    counts
                })
        })
        .unwrap_or_default();

    let mut lines: Vec<Line> = Vec::new();

    // Column widths (tight but readable).
    // id=8, name=35, status=10, model=18, elapsed=7, steps=5, claimed=7, done=6, sent=5, recv=5
    let dim = Style::default()
        .fg(colors::HINT)
        .add_modifier(Modifier::DIM);
    // ── header ────────────────────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(format!("{:<8} ", "id"), dim),
        Span::styled(format!("{:<35} ", "name"), dim),
        Span::styled(format!("{:<10} ", "status"), dim),
        Span::styled(format!("{:<18} ", "model"), dim),
        Span::styled(format!("{:>7} ", "elapsed"), dim),
        Span::styled(format!("{:>5} ", "steps"), dim),
        Span::styled(format!("{:>7} ", "claimed"), dim),
        Span::styled(format!("{:>6} ", "done"), dim),
        Span::styled(format!("{:>5} ", "sent"), dim),
        Span::styled(format!("{:>5} ", "recv"), dim),
    ]));

    // ── lead row ─────────────────────────────────────────────────────────
    let lead_status_color = Color::Green;
    let lead_agent_id = app.agent_name.clone();
    let (lead_sent, lead_recv) = app
        .team_message_counts
        .get(&lead_agent_id)
        .or_else(|| app.team_message_counts.get("lead"))
        .copied()
        .unwrap_or((0, 0));
    lines.push(Line::from(vec![
        Span::styled("● ", Style::default().fg(lead_status_color)),
        Span::styled(
            format!("{:<8} ", short_id(&lead_session)),
            Style::default()
                .fg(lead_status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<35} ", app.agent_name.as_str()),
            Style::default().fg(lead_status_color),
        ),
        Span::styled(
            format!("{:<10} ", "active"),
            Style::default().fg(lead_status_color),
        ),
        Span::styled(
            format!("{:<18} ", "lead"),
            Style::default().fg(colors::HINT),
        ),
        Span::styled(
            format!("{:>7} ", lead_elapsed),
            Style::default().fg(colors::HINT),
        ),
        Span::styled(
            format!("{:>5} ", lead_steps),
            Style::default().fg(colors::HINT),
        ),
        Span::styled(format!("{:>7} ", "-"), Style::default().fg(colors::HINT)),
        Span::styled(format!("{:>6} ", "-"), Style::default().fg(colors::HINT)),
        Span::styled(
            format!(
                "{:>5} ",
                if lead_sent > 0 {
                    lead_sent.to_string()
                } else {
                    "-".to_string()
                }
            ),
            Style::default().fg(if lead_sent > 0 {
                Color::Cyan
            } else {
                colors::HINT
            }),
        ),
        Span::styled(
            format!(
                "{:>5} ",
                if lead_recv > 0 {
                    lead_recv.to_string()
                } else {
                    "-".to_string()
                }
            ),
            Style::default().fg(if lead_recv > 0 {
                Color::Cyan
            } else {
                colors::HINT
            }),
        ),
    ]));
    // ── teammate rows ─────────────────────────────────────────────────────
    for (i, member) in members.iter().enumerate() {
        let (status_str, status_color) = status_label(&member.status);
        let id_short = short_id(&member.agent_id);
        let is_focused = app.focused_teammate.as_ref() == Some(&member.agent_id);

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

        let (claimed, done) = task_counts.get(&member.agent_id).copied().unwrap_or((0, 0));

        let (sent, recv) = app
            .team_message_counts
            .get(&member.agent_id)
            .or_else(|| app.team_message_counts.get(&member.name))
            .copied()
            .unwrap_or((0, 0));

        let connector = if i + 1 == members.len() {
            "└ "
        } else {
            "├ "
        };
        let name_color = if is_focused {
            Color::Yellow
        } else {
            Color::White
        };
        let name_mod = if is_focused {
            Modifier::BOLD | Modifier::UNDERLINED
        } else {
            Modifier::empty()
        };
        let focus_marker = if is_focused { "▸" } else { " " };

        let mut spans = vec![
            Span::styled(focus_marker, Style::default().fg(Color::Yellow)),
            Span::styled(connector, Style::default().fg(Color::Blue)),
            Span::styled(
                format!("{:<8} ", id_short),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("{:<35} ", member.name.as_str()),
                Style::default().fg(name_color).add_modifier(name_mod),
            ),
            Span::styled(
                format!("{:<10} ", status_str),
                Style::default().fg(status_color),
            ),
        ];

        // Model column: show override or "(inherited)"
        let model_label: String = member
            .model_override
            .as_ref()
            .map(|mr| {
                let full = format!("{}/{}", mr.provider_id, mr.model_id);
                full.chars().take(17).collect()
            })
            .unwrap_or_else(|| "(inherited)".to_string());
        let model_color = if member.model_override.is_some() {
            Color::Magenta
        } else {
            colors::HINT
        };
        spans.push(Span::styled(
            format!("{:<18} ", model_label),
            Style::default().fg(model_color),
        ));

        spans.extend_from_slice(&[
            Span::styled(
                format!("{:>7} ", format_elapsed(member.created_at)),
                Style::default().fg(colors::HINT),
            ),
            Span::styled(format!("{:>5} ", steps), Style::default().fg(colors::HINT)),
            Span::styled(
                format!("{:>7} ", claimed),
                Style::default().fg(if claimed > 0 {
                    Color::Yellow
                } else {
                    colors::HINT
                }),
            ),
            Span::styled(
                format!("{:>6} ", done),
                Style::default().fg(if done > 0 { Color::Green } else { colors::HINT }),
            ),
            Span::styled(
                format!(
                    "{:>5} ",
                    if sent > 0 {
                        sent.to_string()
                    } else {
                        "0".to_string()
                    }
                ),
                Style::default().fg(if sent > 0 { Color::Cyan } else { colors::HINT }),
            ),
            Span::styled(
                format!(
                    "{:>5} ",
                    if recv > 0 {
                        recv.to_string()
                    } else {
                        "0".to_string()
                    }
                ),
                Style::default().fg(if recv > 0 { Color::Cyan } else { colors::HINT }),
            ),
        ]);
        // [T] badge — clickable hint
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
            "  (no teammates yet — use team_spawn tool or blueprint)",
            Style::default()
                .fg(colors::HINT)
                .add_modifier(Modifier::DIM),
        )]));
    }
    let total_lines = lines.len() as u16;
    let visible_lines = area.height.saturating_sub(SPACING_SM); // border space
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

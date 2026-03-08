use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use ragent_core::message::{MessagePart, Role};

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar
            Constraint::Min(3),   // messages
            Constraint::Length(3), // input
        ])
        .split(frame.area());

    render_status_bar(frame, app, chunks[0]);
    render_messages(frame, app, chunks[1]);
    render_input(frame, app, chunks[2]);

    if app.permission_pending.is_some() {
        render_permission_dialog(frame, app);
    }
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let session_display = app
        .session_id
        .as_deref()
        .map(|s| &s[..8.min(s.len())])
        .unwrap_or("none");

    let status_text = format!(
        " ● ragent  session: {}  agent: {}  tokens: {}/{}  [{}]",
        session_display,
        app.agent_name,
        app.token_usage.0,
        app.token_usage.1,
        app.status,
    );

    let bar = Paragraph::new(Line::from(Span::styled(
        status_text,
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )))
    .style(Style::default().bg(Color::DarkGray));

    frame.render_widget(bar, area);
}

fn render_messages(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line<'_>> = Vec::new();

    for msg in &app.messages {
        let role_style = match msg.role {
            Role::User => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            Role::Assistant => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        };

        let prefix = match msg.role {
            Role::User => "You: ",
            Role::Assistant => "Assistant: ",
        };

        for part in &msg.parts {
            match part {
                MessagePart::Text { text } => {
                    for (i, line) in text.lines().enumerate() {
                        if i == 0 {
                            lines.push(Line::from(vec![
                                Span::styled(prefix, role_style),
                                Span::raw(line),
                            ]));
                        } else {
                            lines.push(Line::from(Span::raw(format!(
                                "{}{}",
                                " ".repeat(prefix.len()),
                                line
                            ))));
                        }
                    }
                }
                MessagePart::ToolCall {
                    tool, state, ..
                } => {
                    let status_str = format!("{:?}", state.status);
                    lines.push(Line::from(vec![
                        Span::styled("  ┌─ ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            format!("tool: {} [{}]", tool, status_str.to_lowercase()),
                            Style::default().fg(Color::Yellow),
                        ),
                    ]));
                    lines.push(Line::from(Span::styled(
                        "  └────",
                        Style::default().fg(Color::Yellow),
                    )));
                }
                MessagePart::Reasoning { text } => {
                    for line in text.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("  💭 {}", line),
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::ITALIC),
                        )));
                    }
                }
            }
        }

        lines.push(Line::from(""));
    }

    // Apply scroll offset
    let total = lines.len() as u16;
    let visible = area.height.saturating_sub(2);
    let max_scroll = total.saturating_sub(visible);
    let scroll = max_scroll.saturating_sub(app.scroll_offset);

    let messages_block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .title(" Messages ");

    let paragraph = Paragraph::new(lines)
        .block(messages_block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    frame.render_widget(paragraph, area);
}

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let input_text = format!("> {}", app.input);
    let block = Block::default().borders(Borders::ALL).title(" Input ");
    let paragraph = Paragraph::new(input_text).block(block);
    frame.render_widget(paragraph, area);

    // Position cursor
    let cursor_x = area.x + 3 + app.input.len() as u16;
    let cursor_y = area.y + 1;
    frame.set_cursor_position((cursor_x, cursor_y));
}

fn render_permission_dialog(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 30, frame.area());
    frame.render_widget(Clear, area);

    if let Some(ref req) = app.permission_pending {
        let text = vec![
            Line::from(Span::styled(
                "Permission Required",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("Permission: {}", req.permission)),
            Line::from(format!(
                "Details: {}",
                req.patterns.first().map(|s| s.as_str()).unwrap_or("")
            )),
            Line::from(""),
            Line::from(Span::styled(
                "[y]es  [a]lways  [n]o",
                Style::default().fg(Color::Cyan),
            )),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Permission ")
            .style(Style::default().fg(Color::Yellow));

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

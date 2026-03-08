use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

use ragent_core::message::{Message, MessagePart, Role};

/// A widget that renders a single `Message` as a list of styled lines.
pub struct MessageWidget<'a> {
    message: &'a Message,
}

impl<'a> MessageWidget<'a> {
    pub fn new(message: &'a Message) -> Self {
        Self { message }
    }

    fn to_lines(&self) -> Vec<Line<'a>> {
        let mut lines = Vec::new();
        let (prefix, style) = match self.message.role {
            Role::User => (
                "You: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Role::Assistant => (
                "Assistant: ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        };

        for part in &self.message.parts {
            match part {
                MessagePart::Text { text } => {
                    for (i, line) in text.lines().enumerate() {
                        if i == 0 {
                            lines.push(Line::from(vec![
                                Span::styled(prefix, style),
                                Span::raw(line.to_string()),
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
                MessagePart::ToolCall { tool, state, .. } => {
                    let status_str = format!("{:?}", state.status).to_lowercase();
                    lines.push(Line::from(vec![
                        Span::styled("  ┌─ ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            format!("tool: {} [{}]", tool, status_str),
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

        lines
    }
}

impl Widget for MessageWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self.to_lines();
        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
        paragraph.render(area, buf);
    }
}

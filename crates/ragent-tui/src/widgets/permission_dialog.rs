//! Permission approval dialog widget.
//!
//! Renders a centered popup over the TUI asking the user to approve or deny a
//! tool permission request with `[y]es`, `[a]lways`, or `[n]o` options.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use ragent_core::permission::PermissionRequest;

/// A centered popup widget that displays a permission request
/// with [y]es / [a]lways / [n]o options.
pub struct PermissionDialog<'a> {
    request: &'a PermissionRequest,
}

impl<'a> PermissionDialog<'a> {
    /// Create a new [`PermissionDialog`] for the given permission request.
    pub fn new(request: &'a PermissionRequest) -> Self {
        Self { request }
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
}

impl Widget for PermissionDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = Self::centered_rect(60, 30, area);
        Clear.render(popup_area, buf);

        let description = self
            .request
            .patterns
            .first()
            .map(|s| s.as_str())
            .unwrap_or("");

        let text = vec![
            Line::from(Span::styled(
                "Permission Required",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("Permission: {}", self.request.permission)),
            Line::from(format!("Details: {}", description)),
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

        paragraph.render(popup_area, buf);
    }
}

//! Question dialog widget for the ragent TUI.
//!
//! Renders a centered popup over the TUI when the agent asks the user a direct
//! question via the `question` tool. Supports two modes:
//!
//! - **Free-text**: shows an editable text input area with a blinking-style
//!   cursor indicator and `Enter`/`Esc` key hints.
//! - **Multiple-choice**: shows a selectable list of options with `↑`/`↓`
//!   (or `j`/`k`) navigation, `Enter` to select, and `Esc` to dismiss.
//!
//! The dialog renders as a modal overlay on top of the chat area, completely
//! blocking the message-window input field while active.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::app::QuestionRequest;

/// A centered popup widget that displays a question from the agent.
///
/// For free-text questions the dialog shows a single-line editable input
/// area with a simulated cursor. For multiple-choice questions it renders
/// a vertical list of options with the currently-selected item
/// highlighted.
pub struct QuestionDialog<'a> {
    /// The question request to display.
    request: &'a QuestionRequest,
    /// Text the user has typed so far (free-text mode only).
    input: &'a str,
    /// Index of the currently-selected option (multiple-choice mode only).
    selected_index: usize,
}

impl<'a> QuestionDialog<'a> {
    /// Create a new [`QuestionDialog`] for the given question request.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ragent_tui::app::QuestionRequest;
    /// use ragent_tui::widgets::question_dialog::QuestionDialog;
    ///
    /// # fn example(request: &QuestionRequest, input: &str) {
    /// let dialog = QuestionDialog::new(request, input, 0);
    /// # }
    /// ```
    pub fn new(request: &'a QuestionRequest, input: &'a str, selected_index: usize) -> Self {
        Self {
            request,
            input,
            selected_index,
        }
    }

    /// Calculate a centered rectangle using percentage-based layout.
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

impl Widget for QuestionDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let has_options = !self.request.options.is_empty();

        // Calculate a good size based on content.
        // Content rows: header(1) + blank(1) + question(N) + blank(1) +
        //   options(M) + blank(1) + footer(1) = N + M + 5
        // Add 2 for borders = N + M + 7 total rows needed.
        let question_lines = self.request.question.lines().count().max(1);
        let min_rows = (question_lines + self.request.options.len() + 7) as u16;
        // Use at least 35% height, enough for content, and cap at 80%.
        let percent_y = if has_options {
            let needed = ((min_rows as u32 * 100) / area.height as u32).min(80) as u16;
            needed.max(30)
        } else {
            40 // Free-text: fixed 40% height
        };

        let dialog_area = Self::centered_rect(70, percent_y, area);

        // Clear the area behind the dialog so nothing bleeds through.
        Clear.render(dialog_area, buf);

        if has_options {
            // ── Multiple-choice mode ──────────────────────────────────
            let mut text: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    "❓ Agent Question",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    self.request.question.as_str(),
                    Style::default().fg(Color::White),
                )),
                Line::from(""),
            ];

            for (i, option) in self.request.options.iter().enumerate() {
                let is_selected = i == self.selected_index;
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let prefix = if is_selected { "▶ " } else { "  " };
                text.push(Line::from(Span::styled(format!("{prefix}{option}"), style)));
            }

            text.push(Line::from(""));
            text.push(Line::from(Span::styled(
                "↑/↓ or j/k to navigate  ·  Enter to select  ·  Esc to dismiss",
                Style::default().fg(Color::DarkGray),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Double)
                .title(Span::styled(
                    " Question ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Black));

            let paragraph = Paragraph::new(text)
                .block(block)
                .alignment(Alignment::Left)
                .wrap(ratatui::widgets::Wrap { trim: false });

            paragraph.render(dialog_area, buf);
        } else {
            // ── Free-text mode ─────────────────────────────────────────
            let input_display = format!("▶ {}_", self.input);

            let text = vec![
                Line::from(Span::styled(
                    "❓ Agent Question",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    self.request.question.as_str(),
                    Style::default().fg(Color::White),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    input_display,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Enter to submit  ·  Esc to dismiss",
                    Style::default().fg(Color::DarkGray),
                )),
            ];

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Double)
                .title(Span::styled(
                    " Question ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Black));

            let paragraph = Paragraph::new(text)
                .block(block)
                .alignment(Alignment::Left)
                .wrap(ratatui::widgets::Wrap { trim: false });

            paragraph.render(dialog_area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(question: &str, options: Vec<&str>) -> QuestionRequest {
        QuestionRequest {
            id: "test-id".to_string(),
            session_id: "test-session".to_string(),
            question: question.to_string(),
            options: options.into_iter().map(String::from).collect(),
        }
    }

    fn render_to_string(dialog: QuestionDialog<'_>, width: u16, height: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| frame.render_widget(dialog, frame.area()))
            .expect("render dialog");
        let buf = terminal.backend().buffer();
        let mut text = String::new();
        let area = buf.area();
        for y in 0..area.height {
            for x in 0..area.width {
                text.push_str(buf[(x, y)].symbol());
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn test_free_text_question_renders_question_text() {
        let req = make_request("What is your name?", vec![]);
        let input = "John";
        let dialog = QuestionDialog::new(&req, input, 0);
        let text = render_to_string(dialog, 120, 40);
        assert!(text.contains("What is your name?"));
        assert!(text.contains("John"));
        assert!(text.contains("Enter to submit"));
    }

    #[test]
    fn test_free_text_question_renders_cursor() {
        let req = make_request("Type something", vec![]);
        let input = "hello";
        let dialog = QuestionDialog::new(&req, input, 0);
        let text = render_to_string(dialog, 120, 40);
        assert!(text.contains("hello_"));
    }

    #[test]
    fn test_multiple_choice_question_renders_options() {
        let req = make_request("Pick one", vec!["Option A", "Option B", "Option C"]);
        let dialog = QuestionDialog::new(&req, "", 0);
        let text = render_to_string(dialog, 120, 40);
        assert!(text.contains("Pick one"));
        assert!(text.contains("Option A"));
        assert!(text.contains("Option B"));
        assert!(text.contains("Option C"));
    }

    #[test]
    fn test_multiple_choice_question_highlights_selected() {
        let req = make_request("Choose", vec!["First", "Second", "Third"]);
        let dialog = QuestionDialog::new(&req, "", 1);
        let text = render_to_string(dialog, 120, 40);
        // Second option should be selected (▶ prefix)
        assert!(text.contains("▶ Second"));
    }

    #[test]
    fn test_free_text_question_shows_dismiss_hint() {
        let req = make_request("Answer me", vec![]);
        let dialog = QuestionDialog::new(&req, "", 0);
        let text = render_to_string(dialog, 120, 40);
        assert!(text.contains("Esc to dismiss"));
    }

    #[test]
    fn test_multiple_choice_question_shows_navigation_hint() {
        let req = make_request("Pick", vec!["A", "B"]);
        let dialog = QuestionDialog::new(&req, "", 0);
        let text = render_to_string(dialog, 120, 40);
        // The hint should contain "Enter to select" at minimum
        assert!(
            text.contains("Enter to select"),
            "Expected 'Enter to select' hint in dialog output"
        );
    }
}

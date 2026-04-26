//! Internal-LLM chat overlay panel.
//!
//! Renders a dedicated floating window over the main TUI that provides a
//! simple two-pane chat interface (message history + text input) backed by
//! the embedded internal LLM service.  The panel is activated by the
//! `/internal-llm chat` slash command and closed with `Esc`.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::utils::centered_rect;

// ── Colours ─────────────────────────────────────────────────────────────────

const BORDER_COLOR: Color = Color::Cyan;
const TITLE_COLOR: Color = Color::Cyan;
const USER_COLOR: Color = Color::Green;
const ASSISTANT_COLOR: Color = Color::White;
const THINKING_COLOR: Color = Color::DarkGray;
const INPUT_COLOR: Color = Color::Yellow;
const HINT_COLOR: Color = Color::DarkGray;

// ── State ────────────────────────────────────────────────────────────────────

/// A single chat turn stored in the panel history.
#[derive(Debug, Clone)]
pub struct ChatTurn {
    /// `true` = user message, `false` = assistant reply.
    pub is_user: bool,
    /// Text content.
    pub text: String,
}

/// State for the internal-LLM chat overlay panel.
#[derive(Debug, Clone, Default)]
pub struct InternalLlmChatState {
    /// Conversation history shown in the panel.
    pub turns: Vec<ChatTurn>,
    /// Current text in the input box.
    pub input: String,
    /// Cursor position (byte index) within `input`.
    pub cursor: usize,
    /// True while an async request is in-flight.
    pub thinking: bool,
    /// Vertical scroll offset for the message list.
    pub scroll_offset: u16,
}

impl InternalLlmChatState {
    /// Create a fresh, empty panel state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a user message into the history.
    pub fn push_user(&mut self, text: impl Into<String>) {
        self.turns.push(ChatTurn {
            is_user: true,
            text: text.into(),
        });
        self.scroll_to_bottom();
    }

    /// Push an assistant reply into the history.
    pub fn push_assistant(&mut self, text: impl Into<String>) {
        self.turns.push(ChatTurn {
            is_user: false,
            text: text.into(),
        });
        self.thinking = false;
        self.scroll_to_bottom();
    }

    /// Push an error reply into the history (shown as assistant message).
    pub fn push_error(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.turns.push(ChatTurn {
            is_user: false,
            text: format!("⚠ {msg}"),
        });
        self.thinking = false;
        self.scroll_to_bottom();
    }

    /// Take the current input text, clear the field, and return the taken text.
    ///
    /// Returns `None` if the input is empty or whitespace-only.
    pub fn take_input(&mut self) -> Option<String> {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return None;
        }
        self.input.clear();
        self.cursor = 0;
        Some(text)
    }

    /// Insert a character at the cursor.
    pub fn insert_char(&mut self, ch: char) {
        self.input.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    /// Delete the character immediately before the cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = self.input[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.input.remove(prev);
        self.cursor = prev;
    }

    /// Move cursor left by one character.
    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.input[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right by one character.
    pub fn cursor_right(&mut self) {
        if self.cursor < self.input.len() {
            let ch = self.input[self.cursor..].chars().next().unwrap();
            self.cursor += ch.len_utf8();
        }
    }

    /// Scroll the message list up by `lines`.
    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Scroll the message list down by `lines` (capped at `max`).
    pub fn scroll_down(&mut self, lines: u16, max: u16) {
        self.scroll_offset = (self.scroll_offset + lines).min(max);
    }

    /// Jump the scroll offset to the end of the message list.
    fn scroll_to_bottom(&mut self) {
        // We don't know the rendered height here, so use a large sentinel value.
        // The renderer will clamp it to the real content height.
        self.scroll_offset = u16::MAX;
    }
}

// ── Rendering ────────────────────────────────────────────────────────────────

/// Render the internal-LLM chat overlay panel.
///
/// Must only be called when `app.internal_llm_chat_panel` is `Some`.
pub fn render_internal_llm_chat(frame: &mut Frame, app: &mut crate::app::App) {
    let state = match &mut app.internal_llm_chat_panel {
        Some(s) => s,
        None => return,
    };

    // Full-screen overlay: 80 × 80 % of the terminal.
    let area = centered_rect(80, 80, frame.area());
    frame.render_widget(Clear, area);

    // Outer border.
    let border_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Internal-LLM Chat  [Esc to close] ",
            Style::default()
                .fg(TITLE_COLOR)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(BORDER_COLOR));
    let inner = border_block.inner(area);
    frame.render_widget(border_block, area);

    // Split inner into: messages (top) + hint line + input box (bottom).
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // messages
            Constraint::Length(1), // hint
            Constraint::Length(3), // input box
        ])
        .split(inner);

    let messages_area = chunks[0];
    let hint_area = chunks[1];
    let input_area = chunks[2];

    render_messages(frame, state, messages_area);
    render_hint(frame, hint_area);
    render_input(frame, state, input_area);
}

/// Render the conversation history into `area`.
fn render_messages(frame: &mut Frame, state: &mut InternalLlmChatState, area: Rect) {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for turn in &state.turns {
        if turn.is_user {
            // User turn: "You › " prefix in green.
            let prefix = Span::styled(
                "You › ",
                Style::default().fg(USER_COLOR).add_modifier(Modifier::BOLD),
            );
            let first_line_done = turn
                .text
                .split('\n')
                .enumerate()
                .map(|(i, part)| {
                    if i == 0 {
                        Line::from(vec![
                            prefix.clone(),
                            Span::styled(part.to_string(), Style::default().fg(USER_COLOR)),
                        ])
                    } else {
                        Line::from(vec![
                            Span::raw("      "),
                            Span::styled(part.to_string(), Style::default().fg(USER_COLOR)),
                        ])
                    }
                })
                .collect::<Vec<_>>();
            lines.extend(first_line_done);
        } else {
            // Assistant turn: "AI  › " prefix.
            let is_error = turn.text.starts_with('⚠');
            let color = if is_error {
                Color::Red
            } else {
                ASSISTANT_COLOR
            };
            let prefix = Span::styled(
                "AI  › ",
                Style::default()
                    .fg(if is_error { Color::Red } else { Color::Cyan })
                    .add_modifier(Modifier::BOLD),
            );
            let first_line_done = turn
                .text
                .split('\n')
                .enumerate()
                .map(|(i, part)| {
                    if i == 0 {
                        Line::from(vec![
                            prefix.clone(),
                            Span::styled(part.to_string(), Style::default().fg(color)),
                        ])
                    } else {
                        Line::from(vec![
                            Span::raw("      "),
                            Span::styled(part.to_string(), Style::default().fg(color)),
                        ])
                    }
                })
                .collect::<Vec<_>>();
            lines.extend(first_line_done);
        }

        // Blank separator between turns.
        lines.push(Line::from(""));
    }

    if state.thinking {
        lines.push(Line::from(Span::styled(
            "AI  › ⏳ thinking…",
            Style::default()
                .fg(THINKING_COLOR)
                .add_modifier(Modifier::ITALIC),
        )));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "Start chatting with the embedded model. Type your message below and press Enter.",
            Style::default().fg(HINT_COLOR),
        )));
    }

    let total_lines = lines.len() as u16;
    let visible_height = area.height;

    // Clamp scroll offset so we never scroll past the content.
    let max_scroll = total_lines.saturating_sub(visible_height);
    if state.scroll_offset > max_scroll {
        state.scroll_offset = max_scroll;
    }

    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((state.scroll_offset, 0)),
        area,
    );
}

/// Render the keyboard hint line.
fn render_hint(frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Enter: send  │  Esc: close  │  ↑↓: scroll history",
            Style::default().fg(HINT_COLOR),
        ))),
        area,
    );
}

/// Render the text input box with a visible cursor.
fn render_input(frame: &mut Frame, state: &InternalLlmChatState, area: Rect) {
    // Build display text: show the input with a block cursor character.
    let input = &state.input;
    let cursor_char = if state.thinking { ' ' } else { '█' };
    let display = if state.cursor < input.len() {
        let (before, after) = input.split_at(state.cursor);
        format!("{before}{cursor_char}{after}")
    } else {
        format!("{input}{cursor_char}")
    };

    let label = if state.thinking {
        "Waiting…"
    } else {
        "Message"
    };
    let border_color = if state.thinking {
        HINT_COLOR
    } else {
        INPUT_COLOR
    };

    frame.render_widget(
        Paragraph::new(display)
            .style(Style::default().fg(INPUT_COLOR))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(
                        format!(" {label} "),
                        Style::default().fg(border_color),
                    ))
                    .border_style(Style::default().fg(border_color)),
            ),
        area,
    );
}

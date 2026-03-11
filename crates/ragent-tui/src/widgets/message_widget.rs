//! Widget for rendering a single chat message.
//!
//! Formats user and assistant messages with role-colored prefixes, renders
//! tool call status indicators and reasoning blocks with distinct styles.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

use ragent_core::message::{Message, MessagePart, Role, ToolCallStatus};

/// Capitalize the first letter of a tool name for display (e.g., "read" → "Read").
fn capitalize_tool_name(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Strip the working directory prefix from a path to produce a project-relative path.
fn make_relative_path(path: &str, cwd: &str) -> String {
    // Expand ~ in cwd to the home directory for comparison
    let expanded_cwd = if cwd.starts_with('~') {
        if let Some(home) = std::env::var_os("HOME") {
            format!("{}{}", home.to_string_lossy(), &cwd[1..])
        } else {
            cwd.to_string()
        }
    } else {
        cwd.to_string()
    };

    let cwd_prefix = if expanded_cwd.ends_with('/') {
        expanded_cwd
    } else {
        format!("{}/", expanded_cwd)
    };
    if path.starts_with(&cwd_prefix) {
        path[cwd_prefix.len()..].to_string()
    } else {
        path.to_string()
    }
}

/// Extract a brief summary from tool input for display next to the tool name.
fn tool_input_summary(tool: &str, input: &serde_json::Value, cwd: &str) -> String {
    match tool {
        "bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .and_then(|s| s.lines().next())
            .map(|s| format!("$ {}", s))
            .unwrap_or_default(),
        "read" | "write" | "create" | "edit" | "list" | "rm" | "office_read" | "office_write"
        | "office_info" | "pdf_read" | "pdf_write" => input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| make_relative_path(p, cwd))
            .unwrap_or_default(),
        "glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "grep" => {
            let pattern = input
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let path = input
                .get("path")
                .and_then(|v| v.as_str())
                .map(|p| make_relative_path(p, cwd));
            match path {
                Some(p) if !p.is_empty() => format!("\"{}\" in {}", pattern, p),
                _ => format!("\"{}\"", pattern),
            }
        }
        _ => String::new(),
    }
}

/// Generate a result summary line for a completed tool call.
fn tool_result_summary(
    tool: &str,
    output: &Option<serde_json::Value>,
    input: &serde_json::Value,
    cwd: &str,
) -> Option<String> {
    let out = output.as_ref()?;
    let line_count = out.get("line_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    match tool {
        "read" => Some(format!(
            "{} line{} read",
            line_count,
            if line_count == 1 { "" } else { "s" }
        )),
        "write" => {
            let path = input["path"]
                .as_str()
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default();
            Some(format!(
                "{} line{} written to {}",
                line_count,
                if line_count == 1 { "" } else { "s" },
                path
            ))
        }
        "create" => {
            let path = input["path"]
                .as_str()
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default();
            Some(format!(
                "{} line{} created in {}",
                line_count,
                if line_count == 1 { "" } else { "s" },
                path
            ))
        }
        "edit" => Some(format!(
            "{} line{} changed",
            line_count,
            if line_count == 1 { "" } else { "s" }
        )),
        "bash" => Some(format!(
            "{} line{}...",
            line_count,
            if line_count == 1 { "" } else { "s" }
        )),
        "grep" => Some(format!(
            "{} line{} matched",
            line_count,
            if line_count == 1 { "" } else { "s" }
        )),
        "glob" => Some(format!(
            "{} file{} found",
            line_count,
            if line_count == 1 { "" } else { "s" }
        )),
        "list" => Some(format!(
            "{} entr{}",
            line_count,
            if line_count == 1 { "y" } else { "ies" }
        )),
        "office_read" | "pdf_read" => Some(format!(
            "{} line{} read",
            line_count,
            if line_count == 1 { "" } else { "s" }
        )),
        "office_write" | "pdf_write" => {
            let path = input["path"]
                .as_str()
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default();
            Some(format!(
                "{} line{} written to {}",
                line_count,
                if line_count == 1 { "" } else { "s" },
                path
            ))
        }
        "office_info" => Some(format!(
            "{} line{} of metadata",
            line_count,
            if line_count == 1 { "" } else { "s" }
        )),
        "rm" => {
            let deleted = out.get("deleted").and_then(|v| v.as_bool()).unwrap_or(false);
            if deleted {
                Some("deleted".to_string())
            } else {
                Some("failed".to_string())
            }
        }
        _ => None,
    }
}

/// A widget that renders a single `Message` as a list of styled lines.
pub struct MessageWidget<'a> {
    message: &'a Message,
    cwd: &'a str,
}

impl<'a> MessageWidget<'a> {
    /// Create a new [`MessageWidget`] for the given message reference.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to render.
    /// * `cwd` - Current working directory, used to make file paths relative.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ragent_core::message::{Message, MessagePart, Role};
    /// use ragent_tui::widgets::message_widget::MessageWidget;
    ///
    /// let msg = Message::new(
    ///     "session-1",
    ///     Role::User,
    ///     vec![MessagePart::Text { text: "Hello!".into() }],
    /// );
    /// let widget = MessageWidget::new(&msg, "/home/user/project");
    /// ```
    pub fn new(message: &'a Message, cwd: &'a str) -> Self {
        Self { message, cwd }
    }

    fn to_lines(&self) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        for part in &self.message.parts {
            match part {
                MessagePart::Text { text } => {
                    let (dot, dot_style, indent) = match self.message.role {
                        Role::User => (
                            "You: ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                            5,
                        ),
                        Role::Assistant => (
                            "● ",
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                            2,
                        ),
                    };
                    for (i, line) in text.lines().enumerate() {
                        if i == 0 {
                            lines.push(Line::from(vec![
                                Span::styled(dot, dot_style),
                                Span::raw(line.to_string()),
                            ]));
                        } else {
                            lines.push(Line::from(Span::raw(format!(
                                "{}{}",
                                " ".repeat(indent),
                                line
                            ))));
                        }
                    }
                }
                MessagePart::ToolCall { tool, state, .. } => {
                    let (indicator, ind_style, name_style) = match state.status {
                        ToolCallStatus::Completed => (
                            "● ",
                            Style::default().fg(Color::Green),
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        ToolCallStatus::Error => (
                            "✗ ",
                            Style::default().fg(Color::Red),
                            Style::default()
                                .fg(Color::Red)
                                .add_modifier(Modifier::BOLD),
                        ),
                        ToolCallStatus::Running | ToolCallStatus::Pending => (
                            "● ",
                            Style::default().fg(Color::DarkGray),
                            Style::default().fg(Color::DarkGray),
                        ),
                    };

                    let display_name = capitalize_tool_name(tool);
                    let summary = tool_input_summary(tool, &state.input, self.cwd);
                    if summary.is_empty() {
                        lines.push(Line::from(vec![
                            Span::styled(indicator, ind_style),
                            Span::styled(display_name, name_style),
                        ]));
                    } else {
                        lines.push(Line::from(vec![
                            Span::styled(indicator, ind_style),
                            Span::styled(format!("{} ", display_name), name_style),
                            Span::styled(summary, Style::default().fg(Color::DarkGray)),
                        ]));
                    }

                    if state.status == ToolCallStatus::Completed {
                        if let Some(result) =
                            tool_result_summary(tool, &state.output, &state.input, self.cwd)
                        {
                            lines.push(Line::from(Span::styled(
                                format!("  └ {}", result),
                                Style::default().fg(Color::DarkGray),
                            )));
                        }
                    }

                    if state.status == ToolCallStatus::Error {
                        if let Some(ref err) = state.error {
                            lines.push(Line::from(Span::styled(
                                format!("  └ {}", err),
                                Style::default().fg(Color::Red),
                            )));
                        }
                    }
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

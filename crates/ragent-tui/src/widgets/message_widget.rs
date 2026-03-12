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
pub(crate) fn capitalize_tool_name(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Strip the working directory prefix from a path to produce a project-relative path.
pub(crate) fn make_relative_path(path: &str, cwd: &str) -> String {
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
pub(crate) fn tool_input_summary(tool: &str, input: &serde_json::Value, cwd: &str) -> String {
    match tool {
        "bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .and_then(|s| s.lines().next())
            .map(|s| format!("$ {}", s))
            .unwrap_or_default(),
        "read" => {
            input
                .get("path")
                .and_then(|v| v.as_str())
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default()
        }
        "write" | "create" | "edit" | "patch" | "list" | "rm" | "office_read"
        | "office_write" | "office_info" | "pdf_read" | "pdf_write" => input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| make_relative_path(p, cwd))
            .unwrap_or_default(),
        "webfetch" => input
            .get("url")
            .and_then(|v| v.as_str())
            .map(|u| {
                if u.len() > 60 {
                    format!("{}…", &u[..60])
                } else {
                    u.to_string()
                }
            })
            .unwrap_or_default(),
        "websearch" => input
            .get("query")
            .and_then(|v| v.as_str())
            .map(|q| format!("\"{}\"", q))
            .unwrap_or_default(),
        "multiedit" => {
            let count = input
                .get("edits")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            format!(
                "{} edit{}",
                count,
                if count == 1 { "" } else { "s" }
            )
        }
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
        "plan_enter" => {
            let task = input
                .get("task")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let truncated = if task.len() > 60 {
                format!("{}…", &task[..60])
            } else {
                task.to_string()
            };
            format!("→ plan: {}", truncated)
        }
        "plan_exit" => {
            let summary = input
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let truncated = if summary.len() > 60 {
                format!("{}…", &summary[..60])
            } else {
                summary.to_string()
            };
            format!("← plan: {}", truncated)
        }
        "todo_read" => {
            let status = input
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("all");
            format!("📋 filter: {}", status)
        }
        "todo_write" => {
            let action = input
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let id = input
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let title = input
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match action {
                "add" => format!("📋 +{}", if title.len() > 40 { &title[..40] } else { title }),
                "update" => format!("📋 ~{}", id),
                "remove" => format!("📋 -{}", id),
                "clear" => "📋 clear all".to_string(),
                _ => format!("📋 {}", action),
            }
        }
        _ => String::new(),
    }
}

/// Return a line-range label for the read tool (e.g. "lines 5-10").
///
/// Returns `None` when no `start_line` / `end_line` parameters are present.
pub(crate) fn read_line_range(input: &serde_json::Value) -> Option<String> {
    let start = input.get("start_line").and_then(|v| v.as_u64());
    let end = input.get("end_line").and_then(|v| v.as_u64());
    match (start, end) {
        (Some(s), Some(e)) => Some(format!("lines {}-{}", s, e)),
        (Some(s), None) => Some(format!("from line {}", s)),
        _ => None,
    }
}

/// Return inline diff stats `(+added, -removed)` for tools that support it.
///
/// Currently only the `edit`, `multiedit`, and `patch` tools provide the
/// necessary `old_lines` / `new_lines` metadata.
pub(crate) fn tool_inline_diff(
    tool: &str,
    output: &Option<serde_json::Value>,
) -> Option<(usize, usize)> {
    let out = output.as_ref()?;
    match tool {
        "edit" => {
            let old_lines = out.get("old_lines").and_then(|v| v.as_u64())? as usize;
            let new_lines = out.get("new_lines").and_then(|v| v.as_u64())? as usize;
            Some((new_lines, old_lines))
        }
        "multiedit" | "patch" => {
            let added = out.get("lines_added").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let removed = out.get("lines_removed").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if added > 0 || removed > 0 {
                Some((added, removed))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Generate a result summary line for a completed tool call.
pub(crate) fn tool_result_summary(
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
        "edit" => None,
        "multiedit" => {
            let edits = out.get("edits").and_then(|v| v.as_u64()).unwrap_or(0);
            let files = out.get("files").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(format!(
                "{} edit{} across {} file{}",
                edits,
                if edits == 1 { "" } else { "s" },
                files,
                if files == 1 { "" } else { "s" }
            ))
        }
        "patch" => {
            let hunks = out.get("hunks").and_then(|v| v.as_u64()).unwrap_or(0);
            let files = out.get("files").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(format!(
                "{} hunk{} applied across {} file{}",
                hunks,
                if hunks == 1 { "" } else { "s" },
                files,
                if files == 1 { "" } else { "s" }
            ))
        }
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
        "webfetch" => {
            let status = out.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(format!(
                "{} line{} (HTTP {})",
                line_count,
                if line_count == 1 { "" } else { "s" },
                status,
            ))
        }
        "websearch" => {
            let results = out.get("results").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(format!(
                "{} result{} found",
                results,
                if results == 1 { "" } else { "s" },
            ))
        }
        "plan_enter" => {
            let task = out
                .get("task")
                .and_then(|v| v.as_str())
                .unwrap_or("plan");
            Some(format!("delegated → plan: {}", task))
        }
        "plan_exit" => {
            let len = out
                .get("summary_length")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            Some(format!("returned ({} chars)", len))
        }
        "todo_read" => {
            let count = out
                .get("count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            Some(format!("{} item{}", count, if count == 1 { "" } else { "s" }))
        }
        "todo_write" => {
            let action = out
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let count = out
                .get("count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            Some(format!("{} → {} remaining", action, count))
        }
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
    tool_step_map: &'a std::collections::HashMap<String, u32>,
}

impl<'a> MessageWidget<'a> {
    /// Create a new [`MessageWidget`] for the given message reference.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to render.
    /// * `cwd` - Current working directory, used to make file paths relative.
    /// * `tool_step_map` - Mapping from tool call IDs to step numbers.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::HashMap;
    /// use ragent_core::message::{Message, MessagePart, Role};
    /// use ragent_tui::widgets::message_widget::MessageWidget;
    ///
    /// let msg = Message::new(
    ///     "session-1",
    ///     Role::User,
    ///     vec![MessagePart::Text { text: "Hello!".into() }],
    /// );
    /// let map = HashMap::new();
    /// let widget = MessageWidget::new(&msg, "/home/user/project", &map);
    /// ```
    pub fn new(message: &'a Message, cwd: &'a str, tool_step_map: &'a std::collections::HashMap<String, u32>) -> Self {
        Self { message, cwd, tool_step_map }
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
                MessagePart::ToolCall { tool, call_id, state } => {
                    let step_tag = self
                        .tool_step_map
                        .get(call_id)
                        .map(|s| format!("[#{}] ", s))
                        .unwrap_or_default();
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

                    // Build the inline diff stats for edit tool (e.g. "(+25 -5)")
                    let inline_diff = if state.status == ToolCallStatus::Completed {
                        tool_inline_diff(tool, &state.output)
                    } else {
                        None
                    };

                    let mut spans = vec![
                        Span::styled(indicator, ind_style),
                        Span::styled(
                            step_tag,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ];
                    if summary.is_empty() {
                        spans.push(Span::styled(display_name, name_style));
                    } else {
                        spans.push(Span::styled(format!("{} ", display_name), name_style));
                        spans.push(Span::styled(summary, Style::default().fg(Color::DarkGray)));
                    }
                    // Show line range for read tool in bold
                    if tool == "read" {
                        if let Some(range) = read_line_range(&state.input) {
                            spans.push(Span::styled(
                                format!(" {}", range),
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                    }
                    if let Some((added, removed)) = inline_diff {
                        spans.push(Span::styled(" (", Style::default().fg(Color::DarkGray)));
                        spans.push(Span::styled(
                            format!("+{}", added),
                            Style::default().fg(Color::Green),
                        ));
                        spans.push(Span::styled(" ", Style::default().fg(Color::DarkGray)));
                        spans.push(Span::styled(
                            format!("-{}", removed),
                            Style::default().fg(Color::Red),
                        ));
                        spans.push(Span::styled(")", Style::default().fg(Color::DarkGray)));
                    }
                    lines.push(Line::from(spans));

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
                        let err_msg = state
                            .error
                            .as_deref()
                            .unwrap_or("Tool execution failed (no error details available)");
                        lines.push(Line::from(Span::styled(
                            format!("  └ Error: {}", err_msg),
                            Style::default().fg(Color::Red),
                        )));
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

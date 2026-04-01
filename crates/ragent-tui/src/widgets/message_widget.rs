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

/// Helper to build a ternary for pluralization (e.g., "1 item" vs "2 items").
fn pluralize(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{} {}", count, singular)
    } else {
        format!("{} {}", count, plural)
    }
}

/// Truncate a string to a maximum length, appending ellipsis if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{truncated}...")
    } else {
        s.to_string()
    }
}

fn truncate_json_strings(value: &serde_json::Value, max_len: usize) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(truncate_str(s, max_len)),
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|v| truncate_json_strings(v, max_len))
                .collect(),
        ),
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), truncate_json_strings(v, max_len));
            }
            serde_json::Value::Object(out)
        }
        _ => value.clone(),
    }
}

fn summarize_tool_args(input: &serde_json::Value, max_str_len: usize) -> String {
    let Some(obj) = input.as_object() else {
        return String::new();
    };
    let truncated = truncate_json_strings(input, max_str_len);
    let Some(tobj) = truncated.as_object() else {
        return String::new();
    };
    let mut keys: Vec<&String> = obj.keys().collect();
    keys.sort();
    let mut parts = Vec::new();
    for k in keys {
        if let Some(v) = tobj.get(k) {
            let value_text = match v {
                serde_json::Value::String(s) => format!("\"{s}\""),
                _ => serde_json::to_string(v).unwrap_or_else(|_| "?".to_string()),
            };
            parts.push(format!("{k}={value_text}"));
        }
    }
    parts.join(", ")
}

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
        "read" => input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| make_relative_path(p, cwd))
            .unwrap_or_default(),
        "write" | "create" | "edit" | "patch" | "list" | "rm" | "office_read" | "office_write"
        | "office_info" | "pdf_read" | "pdf_write" => input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| make_relative_path(p, cwd))
            .unwrap_or_default(),
        "webfetch" => input
            .get("url")
            .and_then(|v| v.as_str())
            .map(|u| truncate_str(u, 60))
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
            pluralize(count, "edit", "edits")
        }
        "glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "grep" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
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
            let task = input.get("task").and_then(|v| v.as_str()).unwrap_or("");
            let truncated = truncate_str(task, 60);
            format!("→ plan: {}", truncated)
        }
        "plan_exit" => {
            let summary = input.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            let truncated = truncate_str(summary, 60);
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
                      let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("?");
                      let id = input.get("id").and_then(|v| v.as_str()).unwrap_or("");
                      let title = input.get("title").and_then(|v| v.as_str()).unwrap_or("");
                      match action {
                          "add" => format!("📋 +{}", truncate_str(title, 40).as_str()),
                          "update" => format!("📋 ~{}", id),
                          "remove" => format!("📋 -{}", id),
                          "clear" => "📋 clear all".to_string(),
                          _ => format!("📋 {}", action),
                      }
                  }
                  "new_task" => {
                      let agent = input.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
                      let task = input.get("task").and_then(|v| v.as_str()).unwrap_or("");
                      let truncated = truncate_str(task, 50);
                      format!("{} → {}", agent, truncated)
                  }
                  "cancel_task" => {
                      let task_id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
                      format!("cancel task: {}", &task_id[..8.min(task_id.len())])
                  }
                  "list_tasks" => {
                      let status = input
                          .get("status")
                          .and_then(|v| v.as_str())
                          .unwrap_or("all");
                      format!("filter: {}", status)
                  }
                  "wait_tasks" => {
                      let task_ids = input
                          .get("task_ids")
                          .and_then(|v| v.as_array())
                          .map(|a| a.len())
                          .unwrap_or(0);
                      if task_ids > 0 {
                          format!("wait on {} task(s)", task_ids)
                      } else {
                          "wait on all tasks".to_string()
                      }
                  }
                  "lsp_definition" => {
                      let column = input.get("column").and_then(|v| v.as_u64()).unwrap_or(0);
                      let line = input.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                      format!("line {}, col {}", line, column)
                  }
                  "lsp_hover" => {
                      let column = input.get("column").and_then(|v| v.as_u64()).unwrap_or(0);
                      let line = input.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                      format!("line {}, col {}", line, column)
                  }
                   "lsp_references" | "lsp_symbols" | "lsp_diagnostics" => input
                       .get("path")
                       .and_then(|v| v.as_str())
                       .map(|p| make_relative_path(p, cwd))
                       .unwrap_or_default(),
        t if t.starts_with("team_") => summarize_tool_args(input, 40),
        _ => summarize_tool_args(input, 40),
    }
}

/// Return a line-range label for the read tool (e.g. "lines 5-10").
///
/// Reads from the tool's output metadata which contains the actual lines read.
/// Returns `None` when metadata is not available or tool did not complete.
pub(crate) fn read_line_range(metadata: &Option<serde_json::Value>) -> Option<String> {
    let meta = metadata.as_ref()?;
    let start = meta.get("start_line").and_then(|v| v.as_u64());
    let end = meta.get("end_line").and_then(|v| v.as_u64());
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
            let removed = out
                .get("lines_removed")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
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
    let line_count = out
        .get("lines")
        .or_else(|| out.get("line_count"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    match tool {
        "read" => {
            let summarised = out
                .get("summarised")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let total = out.get("total_lines").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if summarised && total > 0 {
                Some(format!(
                    "{} read (summarised, {} total)",
                    pluralize(line_count, "line", "lines"),
                    total
                ))
            } else {
                Some(format!("{} read", pluralize(line_count, "line", "lines")))
            }
        }
        "write" => {
            let path = input["path"]
                .as_str()
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default();
            Some(format!(
                "{} written to {}",
                pluralize(line_count, "line", "lines"),
                path
            ))
        }
        "create" => {
            let path = input["path"]
                .as_str()
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default();
            Some(format!(
                "{} created in {}",
                pluralize(line_count, "line", "lines"),
                path
            ))
        }
        "edit" => None,
        "multiedit" => {
            let edits = out.get("edits").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let files = out.get("files").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!(
                "{} across {}",
                pluralize(edits, "edit", "edits"),
                pluralize(files, "file", "files")
            ))
        }
        "patch" => {
            let hunks = out.get("hunks").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let files = out.get("files").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!(
                "{} applied across {}",
                pluralize(hunks, "hunk", "hunks"),
                pluralize(files, "file", "files")
            ))
        }
        "bash" => Some(format!("{}…", pluralize(line_count, "line", "lines"))),
        "grep" => Some(format!(
            "{} matched",
            pluralize(line_count, "line", "lines")
        )),
        "glob" => Some(format!("{} found", pluralize(line_count, "file", "files"))),
        "list" => {
            let entries = out.get("entries").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let path = out.get("path").and_then(|v| v.as_str())
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default();
            let label = if entries == 1 { "entry" } else { "entries" };
            Some(format!("{} {} in {}", entries, label, path))
        }
        "webfetch" => {
            let status = out.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(format!(
                "{} (HTTP {})",
                pluralize(line_count, "line", "lines"),
                status
            ))
        }
        "websearch" => {
            let results = out.get("results").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!("{} found", pluralize(results, "result", "results")))
        }
        "plan_enter" => {
            let task = out.get("task").and_then(|v| v.as_str()).unwrap_or("plan");
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
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(pluralize(count, "item", "items"))
        }
        "todo_write" => {
            let action = out.get("action").and_then(|v| v.as_str()).unwrap_or("?");
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(format!("{} → {} remaining", action, count))
        }
        "office_read" | "pdf_read" => {
            Some(format!("{} read", pluralize(line_count, "line", "lines")))
        }
        "office_write" | "pdf_write" => {
            let path = input["path"]
                .as_str()
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default();
            Some(format!(
                "{} written to {}",
                pluralize(line_count, "line", "lines"),
                path
            ))
        }
        "office_info" => Some(format!(
            "{} of metadata",
            pluralize(line_count, "line", "lines")
        )),
        "rm" => {
            let deleted = out
                .get("deleted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if deleted {
                Some("deleted".to_string())
            } else {
                Some("failed".to_string())
            }
        }
        "new_task" => {
            let agent = out.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
            let background = out
                .get("background")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let status = out
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let task_id = out
                .get("task_id")
                .and_then(|v| v.as_str())
                .map(|id| format!(" ({})", &id[..8.min(id.len())]))
                .unwrap_or_default();
            if background {
                Some(format!("spawned {} agent{}", agent, task_id))
            } else {
                Some(format!("{} agent {} → {}{}", status, agent, status, task_id))
            }
        }
        "cancel_task" => {
            let cancelled = out
                .get("cancelled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if cancelled {
                Some("task cancelled".to_string())
            } else {
                Some("already completed".to_string())
            }
        }
        "list_tasks" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(pluralize(count, "task", "tasks"))
        }
        "wait_tasks" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let success = out
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if success {
                Some(format!("waited on {} task(s)", count))
            } else {
                Some(format!("timeout waiting for {} task(s)", count))
            }
        }
        "lsp_definition" | "lsp_hover" | "lsp_references" | "lsp_symbols" | "lsp_diagnostics" => {
            Some(format!(
                "{} results",
                pluralize(line_count, "result", "results")
            ))
        }
        _ => None,
    }
}

/// A widget that renders a single `Message` as a list of styled lines.
pub struct MessageWidget<'a> {
    message: &'a Message,
    cwd: &'a str,
    tool_step_map: &'a std::collections::HashMap<String, (String, u32)>,
}

impl<'a> MessageWidget<'a> {
    /// Create a new [`MessageWidget`] for the given message reference.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to render.
    /// * `cwd` - Current working directory, used to make file paths relative.
    /// * `tool_step_map` - Mapping from tool call IDs to `(short_session_id, step_number)`.
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
    pub fn new(
        message: &'a Message,
        cwd: &'a str,
        tool_step_map: &'a std::collections::HashMap<String, (String, u32)>,
    ) -> Self {
        Self {
            message,
            cwd,
            tool_step_map,
        }
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
                MessagePart::ToolCall {
                    tool,
                    call_id,
                    state,
                } => {
                    let step_tag = self
                        .tool_step_map
                        .get(call_id)
                        .map(|(sid, s)| format!("[{sid}:{s}] "))
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
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
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
                        if let Some(range) = read_line_range(&state.output) {
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
                                              if tool == "wait_tasks" {
                                                  // Special handling for wait_tasks: show indented list of tasks
                                                  if let Some(tasks_array) = state
                                                      .output
                                                      .as_ref()
                                                      .and_then(|out| out.get("tasks"))
                                                      .and_then(|t| t.as_array())
                                                  {
                                                      for task in tasks_array {
                                                          let agent = task
                                                              .get("agent")
                                                              .and_then(|a| a.as_str())
                                                              .unwrap_or("unknown");
                                                          let task_id = task
                                                              .get("id")
                                                              .and_then(|id| id.as_str())
                                                              .map(|id| &id[..8.min(id.len())])
                                                              .unwrap_or("unknown");
                                                          let elapsed_ms = task
                                                              .get("elapsed_ms")
                                                              .and_then(|e| e.as_u64())
                                                              .unwrap_or(0);
                                                          let output_lines = task
                                                              .get("output_lines")
                                                              .and_then(|l| l.as_u64())
                                                              .unwrap_or(0) as usize;
                                                          let task_status = task
                                                              .get("status")
                                                              .and_then(|s| s.as_str())
                                                              .unwrap_or("unknown");
                    
                                                          let status_icon = if task_status == "completed" {
                                                              "✓"
                                                          } else {
                                                              "✗"
                                                          };
                    
                                                          let elapsed_sec = elapsed_ms as f64 / 1000.0;
                                                          let task_line = format!(
                                                              "  {} {} ({}): {}s, {} line(s)",
                                                              status_icon,
                                                              agent,
                                                              task_id,
                                                              elapsed_sec,
                                                              output_lines
                                                          );
                    
                                                          lines.push(Line::from(Span::styled(
                                                              task_line,
                                                              if task_status == "completed" {
                                                                  Style::default().fg(Color::Green)
                                                              } else {
                                                                  Style::default().fg(Color::Red)
                                                              },
                                                          )));
                                                      }
                                                  }
                                              } else if let Some(result) =
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
                MessagePart::Image { path, .. } => {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("image");
                    lines.push(Line::from(Span::styled(
                        format!("  📎 [image: {}]", name),
                        Style::default().fg(Color::Yellow),
                    )));
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

#[cfg(test)]
mod tests {
    use super::tool_input_summary;
    use serde_json::json;

    #[test]
    fn test_team_tool_summary_includes_args() {
        let input = json!({
            "team_name": "alpha",
            "teammate_name": "reviewer-1",
            "agent_type": "general"
        });
        let summary = tool_input_summary("team_spawn", &input, "/tmp");
        assert!(summary.contains("team_name=\"alpha\""));
        assert!(summary.contains("teammate_name=\"reviewer-1\""));
        assert!(summary.contains("agent_type=\"general\""));
    }

    #[test]
    fn test_team_tool_summary_truncates_long_strings_with_three_dots() {
        let long = "x".repeat(60);
        let input = json!({
            "team_name": "alpha",
            "prompt": long
        });
        let summary = tool_input_summary("team_spawn", &input, "/tmp");
        assert!(summary.contains("prompt=\""));
        assert!(summary.contains("...\""), "summary should use three dots: {summary}");
    }

    #[test]
    fn test_unknown_tool_summary_includes_args() {
        let input = json!({
            "path": "/tmp/file.txt",
            "limit": 10
        });
        let summary = tool_input_summary("some_new_tool", &input, "/tmp");
        assert!(summary.contains("path=\"/tmp/file.txt\""));
        assert!(summary.contains("limit=10"));
    }

    #[test]
    fn test_unknown_tool_summary_truncates_long_strings() {
        let input = json!({
            "note": "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"
        });
        let summary = tool_input_summary("some_new_tool", &input, "/tmp");
        assert!(summary.contains("note=\""));
        assert!(summary.contains("...\""), "summary should truncate with three dots: {summary}");
    }
}

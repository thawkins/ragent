//! The `list_agents` tool - lists sub-agent tasks for the current session.

use anyhow::Result;
use serde_json::{Value, json};
use std::fmt::Write;

use crate::task::TaskStatus;

use super::{Tool, ToolContext, ToolOutput};

/// Lists all sub-agent tasks (running and completed) for the current session.
///
/// Parameters:
/// - `status` (string, optional): Filter by status (`"running"`, `"completed"`,
///   `"failed"`, `"cancelled"`). If omitted, returns all tasks.
/// - `task_id` (string, optional): Get details for a specific task.
pub struct ListAgentsTool;

#[async_trait::async_trait]
impl Tool for ListAgentsTool {
    fn name(&self) -> &'static str {
        "list_agents"
    }

    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &'static str {
        "List sub-agent tasks for the current session. Shows running and completed \
               background tasks with their status, agent, result summary, and - for \
               finished tasks - the `output_file` path to the FULL untruncated report \
               written under `log/subagents/<task-id>.md` (recover truncated output with \
               the `read` tool against that file). No required parameters. Optional: \
               'status' (string enum running/completed/failed/cancelled) to filter, or \
               'task_id' (string) to retrieve details for a single task. Common gotcha: \
               this tool lists tasks created via new_agent with background: true; it does \
               not list team tasks (use team_task_list for those)."
    }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "description": "Filter tasks by status: running, completed, failed, cancelled",
                    "enum": ["running", "completed", "failed", "cancelled"]
                },
                "task_id": {
                    "type": "string",
                    "description": "Get details for a specific task by ID"
                }
            },
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
        "none"
    }

    /// Lists sub-agent tasks for the current session.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `AgentManager` is not available in the context
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let agent_manager = ctx.agent_manager.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Sub-agent management is not available in this context. \
                      AgentManager has not been initialised."
            )
        })?;

        // Single task detail mode
        if let Some(task_id) = input.get("task_id").and_then(|v| v.as_str()) {
            return match agent_manager.get_task(task_id).await {
                Some(entry) => {
                    let detail = format_task_detail(&entry);
                    Ok(ToolOutput {
                        content: detail,
                        metadata: Some(json!({
                            "task_id": entry.id,
                            "status": serde_json::to_value(&entry.status).unwrap_or(Value::Null),
                        })),
                    })
                }
                None => Ok(ToolOutput {
                    content: format!("Task '{task_id}' not found."),
                    metadata: None,
                }),
            };
        }

        // List mode
        let status_filter = input.get("status").and_then(|v| v.as_str());

        let tasks = agent_manager.list_agents(&ctx.session_id).await;

        let filtered: Vec<_> = if let Some(filter) = status_filter {
            tasks
                .into_iter()
                .filter(|t| t.status.as_str() == filter)
                .collect()
        } else {
            tasks
        };

        if filtered.is_empty() {
            let msg = if let Some(filter) = status_filter {
                format!("No tasks with status '{filter}' found.")
            } else {
                "No sub-agent tasks found for this session.".to_string()
            };
            return Ok(ToolOutput {
                content: msg,
                metadata: Some(json!({ "count": 0 })),
            });
        }

        let running_count = filtered
            .iter()
            .filter(|t| t.status == TaskStatus::Running)
            .count();

        let mut output = String::new();
        let _ = write!(
            output,
            "Sub-agent tasks ({} total, {} running):\n\n",
            filtered.len(),
            running_count
        );
        output.push_str("| ID (short) | Agent | Status | Background | Duration | Summary |\n");
        output.push_str("|------------|-------|--------|------------|----------|---------|");

        for task in &filtered {
            let short_id = if task.id.len() > 8 {
                &task.id[..8]
            } else {
                &task.id
            };

            let status = format!("{} {}", status_emoji(&task.status), task.status);
            let duration = format_duration(task, "(running)");

            let summary = task
                .result
                .as_deref()
                .or(task.error.as_deref())
                .unwrap_or("-");
            let summary_short = ragent_types::truncate_bytes(summary, 100);
            let bg = if task.background { "yes" } else { "no" };
            let report_marker = match task.report_status {
                crate::task::ReportStatus::Continued => " (continued)",
                crate::task::ReportStatus::Truncated => " (TRUNCATED)",
                crate::task::ReportStatus::Complete => "",
            };

            let _ = write!(
                output,
                "\n| {short_id} | {} | {status}{report_marker} | {bg} | {duration} | {summary_short} |",
                task.agent_name
            );

            // Surface the durable on-disk copy of the full untruncated
            // report so the model (and the human) can recover it via the
            // `read` tool when the 100-char summary above is not enough.
            if let Some(ref file) = task.output_file {
                let _ = write!(output, "\n  -> [file] Full report: `{}`", file.display());
            }
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "task_count": filtered.len(),
                "running_count": running_count,
            })),
        })
    }
}

/// Format a task's elapsed duration, appending `suffix` while it is still
/// running (list and detail views use different suffixes).
fn format_duration(task: &crate::task::TaskEntry, suffix: &str) -> String {
    match task.completed_at {
        Some(completed) => format!("{}s", (completed - task.created_at).num_seconds()),
        None => format!(
            "{}s {suffix}",
            (chrono::Utc::now() - task.created_at).num_seconds()
        ),
    }
}

/// Format detailed information about a single task.
fn format_task_detail(task: &crate::task::TaskEntry) -> String {
    let status = format!("{} {}", status_emoji(&task.status), task.status);
    let duration = format_duration(task, "(still running)");

    let mut detail = format!(
        "Task: {}\n\
         Agent: {}\n\
         Status: {status}\n\
         Background: {}\n\
         Created: {}\n\
         Duration: {duration}\n\
         Parent Session: {}\n\
         Child Session: {}",
        task.id,
        task.agent_name,
        task.background,
        task.created_at.format("%Y-%m-%d %H:%M:%S UTC"),
        &task.parent_session_id[..8.min(task.parent_session_id.len())],
        &task.child_session_id[..8.min(task.child_session_id.len())],
    );

    let _ = write!(detail, "\n\nTask Prompt:\n{}", task.task_prompt);

    if let Some(ref result) = task.result {
        let _ = write!(detail, "\n\nResult:\n{result}");
    }

    // Surface when the final reply was cut by the provider; silence when it
    // completed normally so the detail view stays clean.
    if task.report_status != crate::task::ReportStatus::Complete {
        let _ = write!(detail, "\n\nReport Status: {}", task.report_status);
    }

    if let Some(ref file) = task.output_file {
        let _ = write!(
            detail,
            "\n\nOutput File (full untruncated report):\n{}",
            file.display()
        );
    }

    if let Some(ref error) = task.error {
        let _ = write!(detail, "\n\nError:\n{error}");
    }

    detail
}

/// Returns the emoji marker used to visually represent a task status.
fn status_emoji(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Running => "[..]",
        TaskStatus::Completed => "[ok]",
        TaskStatus::Failed => "[x]",
        TaskStatus::Cancelled => "[blocked]",
        TaskStatus::Suspended => "[||]",
        TaskStatus::Terminating => "[skull]",
    }
}

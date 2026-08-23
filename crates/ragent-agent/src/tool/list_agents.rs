//! The `list_agents` tool — lists sub-agent tasks for the current session.

use anyhow::Result;
use serde_json::{Value, json};
use std::fmt::Write;

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
               background tasks with their status, agent, result summary, and — for \
               finished tasks — the `output_file` path to the FULL untruncated report \
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
                .filter(|t| {
                    let status_str = match t.status {
                        crate::task::TaskStatus::Running => "running",
                        crate::task::TaskStatus::Completed => "completed",
                        crate::task::TaskStatus::Failed => "failed",
                        crate::task::TaskStatus::Cancelled => "cancelled",
                        crate::task::TaskStatus::Suspended => "suspended",
                        crate::task::TaskStatus::Terminating => "terminating",
                    };
                    status_str == filter
                })
                .collect()
        } else {
            tasks
        };

        if filtered.is_empty() {
            let msg = if status_filter.is_some() {
                format!("No tasks with status '{}' found.", status_filter.unwrap())
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
            .filter(|t| t.status == crate::task::TaskStatus::Running)
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
            let duration = if let Some(completed) = task.completed_at {
                let dur = completed - task.created_at;
                format!("{}s", dur.num_seconds())
            } else {
                let dur = chrono::Utc::now() - task.created_at;
                format!("{}s (running)", dur.num_seconds())
            };

            let summary = task
                .result
                .as_deref()
                .or(task.error.as_deref())
                .unwrap_or("—");
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
                let _ = write!(output, "\n  ↳ 📄 Full report: `{}`", file.display());
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

/// Format detailed information about a single task.
fn format_task_detail(task: &crate::task::TaskEntry) -> String {
    let mut status_words = task.status.to_string();
    let first = status_words.remove(0).to_uppercase().to_string();
    let status = format!("{} {first}{status_words}", status_emoji(&task.status));
    let duration = if let Some(completed) = task.completed_at {
        let dur = completed - task.created_at;
        format!("{}s", dur.num_seconds())
    } else {
        let dur = chrono::Utc::now() - task.created_at;
        format!("{}s (still running)", dur.num_seconds())
    };

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

    if let Some(prompt) = Some(&task.task_prompt) {
        let _ = write!(detail, "\n\nTask Prompt:\n{prompt}");
    }

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
fn status_emoji(status: &crate::task::TaskStatus) -> &'static str {
    match status {
        crate::task::TaskStatus::Running => "⏳",
        crate::task::TaskStatus::Completed => "✅",
        crate::task::TaskStatus::Failed => "❌",
        crate::task::TaskStatus::Cancelled => "🚫",
        crate::task::TaskStatus::Suspended => "⏸",
        crate::task::TaskStatus::Terminating => "💀",
    }
}

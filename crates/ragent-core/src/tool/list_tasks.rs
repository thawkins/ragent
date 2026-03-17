//! The `list_tasks` tool — lists sub-agent tasks for the current session.

use anyhow::Result;
use serde_json::{json, Value};

use super::{Tool, ToolContext, ToolOutput};

/// Lists all sub-agent tasks (running and completed) for the current session.
///
/// Parameters:
/// - `status` (string, optional): Filter by status (`"running"`, `"completed"`,
///   `"failed"`, `"cancelled"`). If omitted, returns all tasks.
/// - `task_id` (string, optional): Get details for a specific task.
pub struct ListTasksTool;

#[async_trait::async_trait]
impl Tool for ListTasksTool {
    fn name(&self) -> &str {
        "list_tasks"
    }

    fn description(&self) -> &str {
        "List sub-agent tasks for the current session. Shows running and completed \
         background tasks with their status, agent, and result summary."
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
            }
        })
    }

    fn permission_category(&self) -> &str {
        "agent:spawn"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let task_manager = ctx
            .task_manager
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Sub-agent management is not available in this context. \
                     TaskManager has not been initialised."
                )
            })?;

        // Single task detail mode
        if let Some(task_id) = input.get("task_id").and_then(|v| v.as_str()) {
            return match task_manager.get_task(task_id).await {
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

        let tasks = task_manager.list_tasks(&ctx.session_id).await;

        let filtered: Vec<_> = if let Some(filter) = status_filter {
            tasks
                .into_iter()
                .filter(|t| {
                    let status_str = match t.status {
                        crate::task::TaskStatus::Running => "running",
                        crate::task::TaskStatus::Completed => "completed",
                        crate::task::TaskStatus::Failed => "failed",
                        crate::task::TaskStatus::Cancelled => "cancelled",
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
        output.push_str(&format!(
            "Sub-agent tasks ({} total, {} running):\n\n",
            filtered.len(),
            running_count
        ));
        output.push_str("| ID (short) | Agent | Status | Background | Duration | Summary |\n");
        output.push_str("|------------|-------|--------|------------|----------|---------|");

        for task in &filtered {
            let short_id = if task.id.len() > 8 {
                &task.id[..8]
            } else {
                &task.id
            };

            let status = match task.status {
                crate::task::TaskStatus::Running => "⏳ running",
                crate::task::TaskStatus::Completed => "✅ completed",
                crate::task::TaskStatus::Failed => "❌ failed",
                crate::task::TaskStatus::Cancelled => "🚫 cancelled",
            };

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
            let summary_short = if summary.len() > 60 {
                format!("{}…", &summary[..60])
            } else {
                summary.to_string()
            };

            let bg = if task.background { "yes" } else { "no" };

            output.push_str(&format!(
                "\n| {short_id} | {} | {status} | {bg} | {duration} | {summary_short} |",
                task.agent_name
            ));
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "count": filtered.len(),
                "running": running_count,
            })),
        })
    }
}

/// Format detailed information about a single task.
fn format_task_detail(task: &crate::task::TaskEntry) -> String {
    let status = match task.status {
        crate::task::TaskStatus::Running => "⏳ Running",
        crate::task::TaskStatus::Completed => "✅ Completed",
        crate::task::TaskStatus::Failed => "❌ Failed",
        crate::task::TaskStatus::Cancelled => "🚫 Cancelled",
    };

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

    if let Some(ref prompt) = Some(&task.task_prompt) {
        detail.push_str(&format!("\n\nTask Prompt:\n{prompt}"));
    }

    if let Some(ref result) = task.result {
        detail.push_str(&format!("\n\nResult:\n{result}"));
    }

    if let Some(ref error) = task.error {
        detail.push_str(&format!("\n\nError:\n{error}"));
    }

    detail
}

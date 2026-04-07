//! The `wait_tasks` tool — blocks until one or more background sub-agent tasks complete.
//!
//! Subscribes to [`Event::SubagentComplete`] on the session event bus so there is
//! no polling.  Returns the full results of all awaited tasks once they finish,
//! or a timeout error if the deadline is exceeded.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::Result;
use serde_json::{Value, json};

use crate::event::Event;
use crate::task::TaskStatus;

use super::{Tool, ToolContext, ToolOutput};

/// Waits for background sub-agent tasks to complete without polling.
///
/// Parameters:
/// - `task_ids` (array of string, optional): IDs of tasks to wait for.
///   If omitted, waits for **all** currently running background tasks.
/// - `timeout_secs` (number, optional): Maximum seconds to wait. Default: 300.
pub struct WaitTasksTool;

#[async_trait::async_trait]
impl Tool for WaitTasksTool {
    fn name(&self) -> &'static str {
        "wait_tasks"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &'static str {
        "Wait for one or more background sub-agent tasks to complete. \
         Returns full results for all awaited tasks. \
         Use this instead of polling with list_tasks. \
         Optionally specify task_ids; if omitted, waits for ALL running background tasks."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "IDs of background tasks to wait for. Omit to wait for all running tasks."
                },
                "timeout_secs": {
                    "type": "number",
                    "description": "Maximum seconds to wait before returning partial results. Default: 300."
                }
            }
        })
    }

    /// # Errors
    ///
    /// Returns an error if the category string cannot be converted or returned.
    fn permission_category(&self) -> &'static str {
        "agent:spawn"
    }

    /// # Errors
    ///
    /// Returns an error if the `TaskManager` is not initialized, if any requested task ID
    /// does not exist or is not a background task, or if the wait operation times out.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let task_manager = ctx
            .task_manager
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("TaskManager not initialised in this context."))?;

        let timeout_secs = input
            .get("timeout_secs")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(300);

        // Subscribe to the event bus BEFORE reading current state to eliminate
        // the race between "task completes" and "we start listening".
        let mut rx = ctx.event_bus.subscribe();

        // Determine which task IDs to wait for.
        let requested: Vec<String> = input
            .get("task_ids")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                    .collect()
            })
            .unwrap_or_default();

        let all_tasks = task_manager.list_tasks(&ctx.session_id).await;

        let mut waiting_for: HashSet<String> = if requested.is_empty() {
            all_tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Running && t.background)
                .map(|t| t.id.clone())
                .collect()
        } else {
            requested.into_iter().collect()
        };

        if waiting_for.is_empty() {
            return Ok(ToolOutput {
                content: "No running background tasks to wait for.".to_string(),
                metadata: Some(json!({ "count": 0 })),
            });
        }

        // Collect results for tasks that already completed before we subscribed.
        let mut results: HashMap<String, (String, bool)> = HashMap::new(); // id → (text, success)
        for task in &all_tasks {
            if waiting_for.contains(&task.id) && task.status != TaskStatus::Running {
                let text = task
                    .result
                    .as_deref()
                    .or(task.error.as_deref())
                    .unwrap_or("(no output)")
                    .to_string();
                let success = task.status == TaskStatus::Completed;
                results.insert(task.id.clone(), (text, success));
                waiting_for.remove(&task.id);
            }
        }

        // Wait for any remaining tasks via event bus (no polling).
        if !waiting_for.is_empty() {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

            loop {
                if waiting_for.is_empty() {
                    break;
                }

                match tokio::time::timeout_at(deadline, rx.recv()).await {
                    Ok(Ok(Event::SubagentComplete {
                        session_id,
                        task_id,
                        summary,
                        success,
                        ..
                    })) if session_id == ctx.session_id && waiting_for.contains(&task_id) => {
                        waiting_for.remove(&task_id);
                        results.insert(task_id, (summary, success));
                    }
                    Ok(Ok(_)) => {
                        // Unrelated event — keep waiting.
                        continue;
                    }
                    Ok(Err(_)) => {
                        // Broadcast channel closed (shouldn't happen in practice).
                        break;
                    }
                    Err(_) => {
                        // Timeout expired.
                        break;
                    }
                }
            }
        }

        // Format the output.
        let timed_out = !waiting_for.is_empty();
        let mut output = String::new();

        if timed_out {
            output.push_str(&format!(
                "⚠️  Timed out after {timeout_secs}s. \
                 {} task(s) still running: {}\n\n",
                waiting_for.len(),
                waiting_for
                    .iter()
                    .map(|id| id[..8.min(id.len())].to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        output.push_str(&format!("{} task(s) completed:\n\n", results.len()));

        // Fetch full task metadata for the completed entries.
        for (task_id, (text, success)) in &results {
            let agent_name = all_tasks
                .iter()
                .find(|t| &t.id == task_id)
                .map_or("unknown", |t| t.agent_name.as_str());

            let icon = if *success { "✅" } else { "❌" };
            let short_id = &task_id[..8.min(task_id.len())];

            output.push_str(&format!(
                "{icon} **{agent_name}** (task {short_id}):\n{text}\n\n---\n\n"
            ));
        }

        // Build metadata with task details for TUI display
        let mut task_details = Vec::new();
        for (task_id, (_, success)) in &results {
            let task = all_tasks.iter().find(|t| &t.id == task_id);
            if let Some(task) = task {
                let elapsed_ms = if let Some(end) = task.completed_at {
                    (end.signed_duration_since(task.created_at)).num_milliseconds() as u64
                } else {
                    0
                };

                let output_lines = task.result.as_ref().map_or(0, |r| r.lines().count());

                task_details.push(json!({
                    "id": &task.id,
                    "agent": &task.agent_name,
                    "status": if *success { "completed" } else { "failed" },
                    "elapsed_ms": elapsed_ms,
                    "output_lines": output_lines,
                }));
            }
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "completed_count": results.len(),
                "timed_out": timed_out,
                "still_running_count": waiting_for.len(),
                "tasks": task_details,
            })),
        })
    }
}

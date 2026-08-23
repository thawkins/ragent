//! The `wait_agents` tool — blocks until one or more background sub-agent tasks complete.
//!
//! Subscribes to [`Event::SubagentComplete`] on the session event bus and also
//! periodically re-scans the [`AgentManager`] task map.  This belt-and-suspenders
//! approach prevents hangs when the broadcast event is dropped, lagged, or races
//! with the initial snapshot.  Returns the full results of all awaited tasks once
//! they finish, or a timeout error if the deadline is exceeded.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::Result;
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::event::Event;
use crate::task::{AgentManager, TaskStatus};

use super::{Tool, ToolContext, ToolOutput};

/// Scan the agent manager for any tasks in `waiting_for` that are no longer
/// `Running`, copy their result into `results`, and remove them from the wait
/// set.  Used both on entry and as a fallback when event delivery is lagged
/// or missed.
async fn collect_completed_tasks(
    agent_manager: &AgentManager,
    parent_session_id: &str,
    waiting_for: &mut HashSet<String>,
    results: &mut HashMap<String, (String, bool)>,
) {
    let fresh = agent_manager.list_agents(parent_session_id).await;
    for task in fresh {
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
}

/// Waits for background sub-agent tasks to complete without polling.
///
/// Parameters:
/// - `task_ids` (array of string, optional): IDs of tasks to wait for.
///   If omitted, waits for **all** currently running background tasks AND
///   collects the full results of any background tasks that already
///   completed for this session.  (The tool is safe to call after the
///   sub-agents have finished — the wait is then instant.)
/// - `timeout_secs` (number, optional): Maximum seconds to wait. Default: 300.
pub struct WaitAgentsTool;

#[async_trait::async_trait]
impl Tool for WaitAgentsTool {
    fn name(&self) -> &'static str {
        "wait_agents"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &'static str {
        "Wait for one or more background sub-agent tasks to complete. No required \
                 parameters. Optional: 'task_ids' (array of strings) to wait for specific \
                 tasks, or 'timeout_secs' (number, default 300). Returns full results for \
                 all awaited tasks (and any tasks that already completed). Every completed \
                 agent's FULL untruncated report is also written to a durable file at \
                 log/subagents/<task-id>.md — its path appears in this tool's output and \
                 metadata as output_file; if the combined report text is cut by the generic \
                 ~12k context truncation, recover the omitted findings with the `read` tool \
                 against that file (the metadata \"results\" array also mirrors every \
                 agent's complete output). Use this tool instead of polling with \
                 list_agents; omitting task_ids waits for ALL running background tasks. \
                 Common gotcha: this tool is for new_agent sub-agents spawned with \
                 background: true, NOT for team members — for teams, always use team_wait."
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
            },
            "additionalProperties": false
        })
    }
    /// # Errors
    ///
    /// Returns an error if the category string cannot be converted or returned.
    fn permission_category(&self) -> &'static str {
        "agent:control"
    }

    /// # Errors
    ///
    /// Returns an error if the `AgentManager` is not initialized, if any requested task ID
    /// does not exist or is not a background task, or if the wait operation times out.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let agent_manager = ctx
            .agent_manager
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("AgentManager not initialised in this context."))?;

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

        let all_tasks = agent_manager.list_agents(&ctx.session_id).await;

        // Include already-completed background tasks: even though they are no
        // longer "running", the caller asked to wait on them and expects
        // their results in the returned content/metadata (previously fixed
        // paths at lines 120-131 & 169-174 were unreachable for this case
        // because waiting_for only seeded Running tasks).  Without this the
        // tool reports "No running background tasks to wait for." even when
        // completed results exist for this session.
        let mut waiting_for: HashSet<String> = if requested.is_empty() {
            all_tasks
                .iter()
                .filter(|t| {
                    t.background
                        && matches!(
                            t.status,
                            TaskStatus::Running
                                | TaskStatus::Completed
                                | TaskStatus::Failed
                                | TaskStatus::Cancelled
                        )
                })
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

        // M7-T3: Increment waiter count for tasks that are still Running.
        // If a task has already completed, `increment_waiter` returns false
        // and we collect it below via a fresh scan instead of waiting for an
        // event that may already have been dropped from the broadcast buffer.
        let mut incremented: HashSet<String> = HashSet::new();
        for task_id in &waiting_for {
            if agent_manager.increment_waiter(task_id).await {
                incremented.insert(task_id.clone());
            }
        }
        // Catch the race where a task finished after the initial snapshot but
        // before we could increment its waiter count.
        collect_completed_tasks(
            agent_manager,
            &ctx.session_id,
            &mut waiting_for,
            &mut results,
        )
        .await;

        // Wait for any remaining tasks via event bus, with a periodic
        // manager re-scan as a fallback against lost or lagged events.
        if !waiting_for.is_empty() {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
            let mut scan_interval = tokio::time::interval(Duration::from_secs(5));
            scan_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            // `interval.tick()` returns immediately the first time; consume
            // that tick so the loop's first scan happens after the interval.
            scan_interval.tick().await;

            loop {
                if waiting_for.is_empty() {
                    break;
                }

                tokio::select! {
                    event = rx.recv() => {
                        match event {
                            Ok(Event::SubagentComplete {
                                session_id,
                                task_id,
                                success,
                                ..
                            }) if session_id == ctx.session_id
                                && waiting_for.contains(&task_id) =>
                            {
                                waiting_for.remove(&task_id);
                                // The event `summary` is truncated to 2000 chars
                                // for TUI display. Look up the full result from the
                                // task entry so the parent agent gets complete output.
                                let text = agent_manager
                                    .get_task(&task_id)
                                    .await
                                    .and_then(|e| e.result.or(e.error).map(|s| s.to_string()))
                                    .unwrap_or_else(|| "(no output)".to_string());
                                results.insert(task_id, (text, success));
                            }
                            Ok(_) => {
                                // Unrelated event — keep waiting.
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                tracing::warn!(
                                    n,
                                    "wait_agents broadcast receiver lagged; \
                                     scanning manager for completed tasks"
                                );
                                collect_completed_tasks(
                                    agent_manager,
                                    &ctx.session_id,
                                    &mut waiting_for,
                                    &mut results,
                                )
                                .await;
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                collect_completed_tasks(
                                    agent_manager,
                                    &ctx.session_id,
                                    &mut waiting_for,
                                    &mut results,
                                )
                                .await;
                                break;
                            }
                        }
                    }
                    _ = scan_interval.tick() => {
                        collect_completed_tasks(
                            agent_manager,
                            &ctx.session_id,
                            &mut waiting_for,
                            &mut results,
                        )
                        .await;
                    }
                    _ = tokio::time::sleep_until(deadline) => {
                        collect_completed_tasks(
                            agent_manager,
                            &ctx.session_id,
                            &mut waiting_for,
                            &mut results,
                        )
                        .await;
                        break;
                    }
                }
            }
        }

        // Refresh the task snapshot so output metadata (agent_name,
        // output_file, report_status) reflects any completions that happened
        // while we were waiting.
        let all_tasks = agent_manager.list_agents(&ctx.session_id).await;

        // M7-T3: Decrement waiter count for every task we incremented.
        for task_id in &incremented {
            agent_manager.decrement_waiter(task_id).await;
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

        // Build output text and metadata in a single pass.
        let mut task_details = Vec::new();
        let mut results_meta = Vec::new();
        for (task_id, (text, success)) in &results {
            let task = all_tasks.iter().find(|t| &t.id == task_id);
            let agent_name = task.map_or("unknown", |t| t.agent_name.as_str());

            let icon = if *success { "✅" } else { "❌" };
            let short_id = &task_id[..8.min(task_id.len())];

            // Report integrity: sub-agent tasks track whether the final
            // reply was fully generated, salvaged via the continuation
            // retry, or was forcibly cut. Older entries default to
            // `Complete` so the marker stays quiet for them.
            let report_status =
                task.map_or(crate::task::ReportStatus::Complete, |t| t.report_status);
            let report_label = report_status.as_str();

            // Full per-agent output in metadata — survives the generic
            // 12k tool-result truncation applied to `content` by
            // `tool_result_content_for_llm`, so the model can recover the
            // complete report even when the printed content is cut.
            //
            // `output_file` is an even more durable recovery path: the
            // agent's complete report is persisted to disk at
            // `log/subagents/<task-id>.md` and can be re-read with the
            // `read` tool at any time.
            let output_file_str =
                task.and_then(|t| t.output_file.as_ref().map(|p| p.display().to_string()));
            results_meta.push(json!({
                "task_id": task_id,
                "agent": agent_name,
                "success": success,
                "output": text,
                "output_file": output_file_str,
                "report_status": report_label,
            }));

            output.push_str(&format!(
                "{icon} **{agent_name}** (task {short_id}):\n{text}\n"
            ));
            if let Some(ref file_path) = output_file_str {
                output.push_str(&format!(
                    "\n📄 Full report: {file_path} (read this file if the text above was truncated)\n"
                ));
            }
            if report_status != crate::task::ReportStatus::Complete {
                let note = match report_status {
                    crate::task::ReportStatus::Continued => {
                        "✅ Report was truncated by the provider; the missing tail was \
                         regenerated by the automatic continuation retry."
                    }
                    crate::task::ReportStatus::Truncated => {
                        "⚠️  Report is TRUNCATED: the provider cut the reply off before it \
                         finished and the continuation retry could not recover it. Treat \
                         this report as incomplete — re-run the task with a narrower scope \
                         or a model with a larger output window if you need the full text."
                    }
                    crate::task::ReportStatus::Complete => unreachable!(),
                };
                output.push_str(note);
                output.push('\n');
            }
            output.push_str("\n---\n\n");

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
                    "report_status": report_label,
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
                // Full untruncated per-agent results. The `content` string
                // above may be shortened by the generic 12k tool-result
                // truncation in `tool_result_content_for_llm`; these
                // entries keep every agent's complete output accessible.
                "results": results_meta,
            })),
        })
    }
}

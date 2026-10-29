//! The `list_agents` tool - lists sub-agent tasks for the current session.

use anyhow::Result;
use serde_json::{Value, json};
use std::fmt::Write;

use crate::task::TaskStatus;

use super::{Tool, ToolContext, ToolOutput};

/// Machine-readable reason for a `status` filter outside the schema's declared
/// set (FR-010).
pub const REASON_INVALID_STATUS_FILTER: &str = "invalid_status_filter";
/// Machine-readable reason for a listing that matched no task (FR-010).
pub const REASON_NO_TASKS: &str = "no_tasks";

/// Lists all sub-agent tasks (running and completed) for the current session.
///
/// Parameters:
/// - `status` (string, optional): Filter by status (`"running"`, `"suspended"`,
///   `"completed"`, `"failed"`, `"cancelled"`, `"timed_out"`). If omitted,
///   returns all tasks.
/// - `task_id` (string, optional): Get details for a specific task.
///
/// FR-012: a task held by one or more in-flight `wait_agents` callers reports
/// its `waiter_count` in both the rendered output and the metadata. The count
/// is reported only when it is greater than zero, so a task nobody awaits
/// never paints a `0` that reads as a defect.
pub struct ListAgentsTool;

#[async_trait::async_trait]
impl Tool for ListAgentsTool {
    fn name(&self) -> &'static str {
        "list_agents"
    }

    /// Returns the generated description of the tool (FR-016).
    ///
    /// It names every property in [`Self::parameters_schema`];
    /// `test_tool_descriptions` asserts that each one appears here.
    fn description(&self) -> &'static str {
        "List sub-agent tasks for the calling session. Shows live and finished \
               background tasks with their status, agent, result summary, and - for \
               finished tasks - the `output_file` path to the FULL untruncated report \
               written under `log/subagents/<task-id>.md` (recover truncated output with \
               the `read` tool against that file). When a finished task has NO durable \
               copy - the report could not be persisted - the row and the detail view say \
               so explicitly (`[memory]`: the report is in-memory only) instead of \
               omitting the field, so you never `read` a path that does not exist. Every \
               result also reports the \
               background limit state: `limit`, `occupancy` and `headroom` in the \
               metadata (describing the whole session even when a `status` filter is \
               given) plus a one-line summary in the content, so the headroom tells you \
               whether a background `new_agent` call will be accepted before you make it. \
               A task held by one or more in-flight `wait_agents` callers also reports \
               how many waiters it has (`waiter_count`) - in the listing rows, in the \
               detail view and in the metadata - but only when that number is greater \
               than zero, so a task nobody awaits shows no waiter count at all. \
               No required parameters. PARAMETERS: \
               `status` (string enum running/suspended/completed/failed/cancelled/timed_out; \
               `terminating` is reachable but unlisted and is read with `task_id`) filters \
               the listing, and `task_id` (string) retrieves the detail view for a single \
               task. A `suspended` background task still occupies one slot of the limit, so \
               suspending frees no headroom - only cancellation does. A parameter this \
               schema does not declare is refused, not ignored. Common gotcha: \
               this tool lists tasks created via new_agent with background: true; it does \
               not list detached tasks (they are fire-and-forget) or team tasks (use \
               team_task_list for those)."
    }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "description": "Filter tasks by status: running, suspended, completed, \
                                    failed, cancelled or timed_out",
                    "enum": [
                        "running",
                        "suspended",
                        "completed",
                        "failed",
                        "cancelled",
                        "timed_out"
                    ]
                },
                "task_id": {
                    "type": "string",
                    "description": "Get details for a specific task by ID"
                }
            },
            "additionalProperties": false
        })
    }
    /// Listing is a read-only operation over the task registry (FR-005): it is
    /// classified `agent:read`, never `none`, so a policy can grant, deny or
    /// prompt on observation separately from spawn and control.
    fn permission_category(&self) -> &'static str {
        "agent:read"
    }

    /// Lists sub-agent tasks for the current session.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The call carries a property the schema does not declare (FR-004, FR-038)
    /// - The `status` filter is not one of the declared statuses
    ///   (`reason_code: invalid_status_filter`)
    /// - `AgentManager` is not available in the context
    ///   (`reason_code: manager_unavailable`)
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // FR-004 / FR-038: an undeclared property is refused, not ignored.
        let schema = self.parameters_schema();
        super::param_schema::validate_tool_input(&schema, &input)?;

        // List mode
        // FR-010: an unsupported filter is a branchable refusal, so it is named
        // with a reason code instead of silently matching nothing. It is
        // checked before the environment so a bad call reports the call, not a
        // missing manager.
        let status_filter = match input.get("status").and_then(|v| v.as_str()) {
            None => None,
            Some(filter) => {
                let declared = [
                    "running",
                    "suspended",
                    "completed",
                    "failed",
                    "cancelled",
                    "timed_out",
                ];
                if !declared.contains(&filter) {
                    return Err(super::param_schema::reason_error(
                        REASON_INVALID_STATUS_FILTER,
                        format!(
                            "Unsupported `status` filter '{filter}'. Declared filters are: {}. \
                             Omit `status` to list every task.",
                            declared
                                .iter()
                                .map(|name| format!("`{name}`"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    ));
                }
                Some(filter)
            }
        };

        let agent_manager = ctx.agent_manager.as_ref().ok_or_else(|| {
            super::param_schema::reason_error(
                super::new_agent::REASON_MANAGER_UNAVAILABLE,
                "Sub-agent management is not available in this context. \
                 AgentManager has not been initialised.",
            )
        })?;

        // FR-011: the limit, its occupancy and the resulting headroom are
        // reported for the whole session - computed before any status filter is
        // applied, so a filtered listing still describes the real limit state
        // rather than the subset it happens to show (FR-035). The numbers come
        // from the same registry report the spawn refusal reads, so a listing
        // and a refusal cannot disagree (NFR-003: no extra storage read).
        let limit_report = agent_manager.limit_report().await;

        // Single task detail mode
        if let Some(task_id) = input.get("task_id").and_then(|v| v.as_str()) {
            return match agent_manager.get_task(task_id).await {
                Some(entry) => {
                    let detail = format!(
                        "{}\n\n{detail}",
                        limit_report.summary(),
                        detail = format_task_detail(&entry)
                    );
                    // FR-012: a detail view reports the number of active
                    // waiters for a live task, and only when there is at
                    // least one - a task nobody awaits must not render
                    // `Waiters: 0`, which would read as a defect.
                    let waiters = active_waiters(&entry);
                    let mut metadata = json!({
                        "task_id": entry.id,
                        "status": serde_json::to_value(&entry.status).unwrap_or(Value::Null),
                    });
                    if let (Value::Object(map), Some(count)) = (&mut metadata, waiters) {
                        map.insert("waiter_count".to_string(), json!(count));
                    }
                    Ok(ToolOutput {
                        content: detail,
                        metadata: Some(limit_metadata(&limit_report, metadata)),
                    })
                }
                None => Ok(ToolOutput {
                    content: format!(
                        "Task '{task_id}' not found. It may have already completed, or it may not \
                         belong to this session.\n\n{}",
                        limit_report.summary()
                    ),
                    // FR-010: the caller can branch on the reason rather than on
                    // the English text of the content.
                    metadata: Some(super::param_schema::with_reason(
                        Some(limit_metadata(&limit_report, json!({ "task_id": task_id }))),
                        super::new_agent::REASON_TASK_NOT_FOUND,
                    )),
                }),
            };
        }

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
                content: format!("{msg}\n\n{}", limit_report.summary()),
                // FR-010: an empty listing is a branchable outcome.
                metadata: Some(super::param_schema::with_reason(
                    Some(limit_metadata(&limit_report, json!({ "count": 0 }))),
                    REASON_NO_TASKS,
                )),
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
        // FR-011: the limit line names all three numbers in the content, not
        // only in the metadata, so a model reading the table also sees how much
        // room is left before a spawn is refused.
        output.push_str(&limit_report.summary());
        output.push('\n');
        output.push_str("\n| ID (short) | Agent | Status | Background | Duration | Summary |\n");
        output.push_str("|------------|-------|--------|------------|----------|---------|");

        for task in &filtered {
            // FR-008: abbreviate through the one shared helper, never by byte
            // offset - `task.id` is a user-visible string and `&id[..8]` panics
            // the tool when a multi-byte codepoint straddles byte 8.
            let short_id = ragent_types::strutil::short_prefix(&task.id, 8);

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

            // FR-012: a list row names how many active waiters hold the live
            // task, and stays silent when nobody is waiting on it.
            if let Some(waiters) = active_waiters(task) {
                let _ = write!(output, "\n  -> {waiters} active waiter(s)");
            }

            // Surface the durable on-disk copy of the full untruncated
            // report so the model (and the human) can recover it via the
            // `read` tool when the 100-char summary above is not enough.
            if let Some(ref file) = task.output_file {
                let _ = write!(output, "\n  -> [file] Full report: `{}`", file.display());
            } else if let Some(note) =
                super::wait_agents::durable_copy_absence_note(task, "\n  -> [memory] ")
            {
                // FR-018 / FR-043: the absence of a durable copy is stated,
                // not omitted. A caller that read the missing field as "no
                // path applies" would go looking for a file that was never
                // written; the note tells it the only copy is in memory.
                let _ = write!(output, "{note}");
            }
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(limit_metadata(
                &limit_report,
                json!({
                    "task_count": filtered.len(),
                    "running_count": running_count,
                }),
            )),
        })
    }
}

/// Adds the limit report's `limit`, `occupancy` and `headroom` to a
/// `list_agents` result's metadata (FR-011).
///
/// Every exit path of the tool - a populated listing, an empty one, a detail
/// hit and a detail miss - reports the same three numbers, so a caller never
/// has to infer the limit state from which branch it received. The numbers
/// describe the whole session, never the status-filtered subset (FR-035).
#[must_use]
fn limit_metadata(report: &crate::task::BackgroundLimitReport, metadata: Value) -> Value {
    let mut metadata = metadata;
    if let Value::Object(ref mut map) = metadata {
        map.insert("limit".to_string(), json!(report.limit));
        map.insert("occupancy".to_string(), json!(report.occupancy));
        map.insert("headroom".to_string(), json!(report.headroom));
        map.insert(
            "occupying_task_ids".to_string(),
            json!(report.occupying_ids),
        );
    }
    metadata
}

/// The number of active waiters holding a live task, or `None` when that
/// number is zero (FR-012).
///
/// `waiter_count` is the registry's own tally of in-flight `wait_agents`
/// callers ([`crate::task::AgentManager::increment_waiter`]). It is reported
/// only when greater than zero, so a task nobody awaits does not paint a
/// `0` that reads as a defect, and it is reported only for a non-terminal
/// task: a finished task's count is a leftover of a wait that ended with the
/// task, not a queue of callers.
#[must_use]
fn active_waiters(task: &crate::task::TaskEntry) -> Option<u32> {
    if task.waiter_count == 0 || task.completed_at.is_some() {
        return None;
    }
    Some(task.waiter_count)
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

    // FR-012: name how many callers currently hold this live task. Silence
    // when nobody awaits it, so an unawaited task's detail stays uncluttered.
    if let Some(waiters) = active_waiters(task) {
        let _ = write!(detail, "\n\nActive Waiters: {waiters}");
    }

    // FR-034: report the per-call wall-clock bound in force, so a caller can
    // see which bound applies without re-deriving it from config. Silence when
    // no bound was requested, matching the report-status convention below.
    if let Some(secs) = task.time_bound_secs {
        let _ = write!(detail, "\n\nTime Bound: {secs}s (per-call)");
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
    } else if let Some(note) = super::wait_agents::durable_copy_absence_note(task, "") {
        // FR-032: the detail view states the in-memory-only condition rather
        // than printing an `Output File` path that cannot be read - and rather
        // than saying nothing, which a caller would read as "no report".
        let _ = write!(detail, "\n\nOutput File: none - {note}");
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
        TaskStatus::TimedOut => "[timeout]",
    }
}

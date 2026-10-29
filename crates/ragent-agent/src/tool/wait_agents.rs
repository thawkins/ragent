//! The `wait_agents` tool - blocks until one or more background sub-agent tasks complete.
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
use crate::task::{AgentManager, TaskEntry, TaskStatus};

use super::{Tool, ToolContext, ToolOutput};

/// Machine-readable reason for a `task_ids` value that is not a string, or not
/// an array of strings (FR-010).
pub const REASON_INVALID_TASK_IDS: &str = "invalid_task_ids";
/// Machine-readable reason for a wait that returned with tasks still running
/// because the deadline passed (FR-010).
pub const REASON_WAIT_TIMED_OUT: &str = "wait_timed_out";
/// Machine-readable reason for a wait that returned with every awaited task
/// settled before the deadline (FR-010).
pub const REASON_WAIT_SETTLED: &str = "wait_settled";
/// Machine-readable reason for a `task_ids` entry that names a detached
/// (fire-and-forget) task, which this tool can never await (FR-044).
pub const REASON_TASK_DETACHED: &str = "task_detached";

/// The refusal for a `wait_agents` call that names a detached task (FR-044).
///
/// A detached task is fire-and-forget: it is excluded from `list_agents` and
/// from this tool's wait set by design, so a wait on its id can never settle.
/// Before this, such an id fell through to the "no running background tasks"
/// answer, which described the one thing that is NOT wrong - the id is real.
/// The refusal therefore names the task as detached, says why it cannot be
/// awaited, and points at the durable report file, because that is the only
/// way its output can still be recovered.
///
/// `task` carries the id, the agent name and the durable report path (when one
/// was written), so the message can name the exact file rather than only
/// suggest that one might exist.
fn detached_refusal(task: &TaskEntry) -> anyhow::Error {
    let short_id = ragent_types::strutil::short_prefix(&task.id, 8);
    // A detached run writes its report to `log/subagents/<task-id>.md` when the
    // write succeeds, so that path is named whether or not `output_file` has
    // been recorded yet - a still-running task has not attempted the write, and
    // a caller that waits for it to finish will find the file there.
    let expected = format!("log/subagents/{}.md", task.id);
    let report = match task.output_file.as_ref() {
        Some(path) => format!(
            "Its report is written to the durable file at {} - read that file with the `read` \
             tool to recover the output.",
            path.display()
        ),
        None => format!(
            "Its report is written to the durable file at {expected} (if the write failed, \
             {IN_MEMORY_REPORT_NOTE}) - read that file with the `read` tool to recover the \
             output."
        ),
    };
    super::param_schema::reason_error(
        REASON_TASK_DETACHED,
        format!(
            "Task {short_id} (`{}`, agent `{}`) is DETACHED: `wait_agents` can never await it. \
             A detached task is spawned with `new_agent(background: true, detached: true)` - \
             fire-and-forget: it is excluded from `list_agents`, it is not awaited here, and \
             its completion is never injected into this session, so a wait on it would hang \
             until the deadline and then report nothing. {report} To find the tasks this \
             session CAN await, call `list_agents`; to receive a sub-agent's result here, \
             spawn it with `new_agent(background: true, detached: false)`.",
            task.id, task.agent_name
        ),
    )
}

/// Names the JSON kind of `value` for a parameter-refusal message (FR-002).
fn describe_json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// One settled sub-agent task, as reported by [`WaitAgentsTool`].
///
/// `status` is the canonical [`TaskStatus::as_str`] label the registry
/// actually recorded, so a cancelled task is reported as `cancelled` rather
/// than collapsed into `failed` (FR-007, FR-014). `success` is kept as the
/// boolean refinement of the same fact for callers written against the older
/// metadata shape; it is never the only signal.
#[derive(Debug, Clone)]
struct SettledTask {
    /// The sub-agent's full report, or its error text, or `(no output)`.
    text: String,
    /// The canonical status label the registry recorded when the task
    /// settled: one of [`TaskStatus::as_str`]'s values.
    status: &'static str,
}

impl SettledTask {
    /// Build a settled entry from the status the wait has for the task.
    ///
    /// `status` is the registry's own status when the task was found there,
    /// and the `success` carried by a [`Event::SubagentComplete`] otherwise.
    /// A cancelled task never publishes that event (it publishes
    /// [`Event::SubagentCancelled`] instead), so it only ever arrives here
    /// through the registry - which is exactly what keeps `cancelled` from
    /// being flattened into `failed` (FR-014).
    fn new(text: String, status: Option<&TaskStatus>, success: bool) -> Self {
        let fallback = if success {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };
        Self {
            text,
            status: status.unwrap_or(&fallback).as_str(),
        }
    }

    /// The canonical status label to report for this task (FR-007).
    fn status_label(&self) -> &'static str {
        self.status
    }

    /// Whether the task reached a successful terminal state.
    fn succeeded(&self) -> bool {
        self.status == TaskStatus::Completed.as_str()
    }
}

/// The line both sub-agent tools print when a finished task has no durable
/// copy of its report (FR-018, FR-043).
///
/// `persist_task_output` returns `None` when the report could not be written -
/// a read-only or otherwise unwritable `log/subagents/` directory, a full disk,
/// a rename that failed. Before this, `output_file` was simply omitted, which
/// made "nothing was ever written" and "the field is not applicable" look
/// identical: a caller could not tell that the only copy of the report lives in
/// memory and will not survive the process.
///
/// It is one constant, shared by `list_agents` and `wait_agents`, so the two
/// surfaces cannot drift into describing the same condition differently.
pub const IN_MEMORY_REPORT_NOTE: &str = "no durable copy of this report was written - it is \
                                        in-memory only and will NOT survive a restart";

/// The explicit no-durable-copy line for a finished task, or [`None`] when a
/// durable copy exists (FR-018, FR-043).
///
/// Only a finished task can have had its report persisted, so a live one
/// reports nothing: saying "no durable copy" about a running task would
/// describe a write that has not been attempted yet. `prefix` is the caller's
/// own marker + indent, so each tool keeps its layout while the wording stays
/// shared.
#[must_use]
pub(crate) fn durable_copy_absence_note(task: &TaskEntry, prefix: &str) -> Option<String> {
    if task.output_file.is_some() || !is_settled(&task.status) {
        return None;
    }
    Some(format!(
        "{prefix}{} - report is {}",
        task.status.as_str(),
        IN_MEMORY_REPORT_NOTE
    ))
}

/// Whether `status` is a terminal state, i.e. the task can no longer produce
/// work and a wait need not stay open for it.
///
/// `Running`, `Suspended` and `Terminating` are in flight: a suspended task is
/// still awaited (FR-030), and a terminating one has not settled yet. Every
/// other status is terminal, including `TimedOut` (FR-017) - a task that hit
/// its per-call bound must be reported, not dropped.
fn is_settled(status: &TaskStatus) -> bool {
    !matches!(
        status,
        TaskStatus::Running | TaskStatus::Suspended | TaskStatus::Terminating
    )
}

/// Describe the state of the tasks a wait is still holding open when it
/// returns, without calling a parked task "running" (FR-015).
///
/// `waiting_for` has already had every settled task removed from it, so these
/// are the in-flight ids: `running` (a task that is working right now),
/// `suspended` (a task parked by a suspend request - it is not producing work
/// and will not resume on its own) and `terminating` (a task that has been
/// asked to stop and has not reached a terminal state yet). Terminating is
/// folded into `running` because from the waiter's point of view it is still
/// an unsettled task; it is never reported as `suspended`.
fn in_flight_breakdown(in_flight: &[&TaskEntry]) -> (Vec<String>, Vec<String>) {
    let mut running = Vec::new();
    let mut suspended = Vec::new();
    for task in in_flight {
        if task.status == TaskStatus::Suspended {
            suspended.push(task.id.clone());
        } else {
            running.push(task.id.clone());
        }
    }
    (running, suspended)
}

/// Compose the ids of an id list as comma-separated 8-character prefixes.
fn short_ids(ids: &[String]) -> String {
    ids.iter()
        .map(|id| ragent_types::strutil::short_prefix(id, 8))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The partial-result headline for a wait that returned with tasks still in
/// flight, naming each unsettled task's real state (FR-015).
///
/// A parked task is reported as `suspended`, never as "still running": the
/// model must be able to tell that nothing will happen to it until somebody
/// resumes or cancels it.
fn partial_result_headline(
    timeout_secs: u64,
    in_flight: &[&TaskEntry],
    settled_count: usize,
) -> String {
    let (running, suspended) = in_flight_breakdown(in_flight);
    if suspended.is_empty() {
        return format!(
            "[!]  Timed out after {timeout_secs}s. {} task(s) still running: {}\n\n",
            running.len(),
            short_ids(&running)
        );
    }
    let mut parts = Vec::new();
    if !running.is_empty() {
        parts.push(format!(
            "{} task(s) still running: {}",
            running.len(),
            short_ids(&running)
        ));
    }
    parts.push(format!(
        "{} task(s) suspended (parked - nothing progresses until they are resumed or \
         cancelled): {}",
        suspended.len(),
        short_ids(&suspended)
    ));
    format!(
        "[!]  Returned after {timeout_secs}s with {} task(s) unsettled. {}. {settled_count} \
         task(s) settled before the deadline.\n\n",
        in_flight.len(),
        parts.join("; ")
    )
}

/// Scan the agent manager for tasks in `waiting_for` that have settled, copy
/// their result and recorded status into `results`, and remove them from the
/// wait set.  Used both on entry and as a fallback when event delivery is
/// lagged or missed.
async fn collect_completed_tasks(
    agent_manager: &AgentManager,
    parent_session_id: &str,
    waiting_for: &mut HashSet<String>,
    results: &mut HashMap<String, SettledTask>,
) {
    let fresh = agent_manager.list_agents(parent_session_id).await;
    for task in fresh {
        // `is_settled` - not `!= Running` - so a parked (`suspended`) task is
        // never mistaken for a settled one and reported as an empty completion
        // (FR-015, FR-030).
        if waiting_for.contains(&task.id) && is_settled(&task.status) {
            // Skip if we already collected a result for this task.
            if results.contains_key(&task.id) {
                continue;
            }
            let text = task
                .result
                .as_deref()
                .or(task.error.as_deref())
                .unwrap_or("(no output)")
                .to_string();
            results.insert(
                task.id.clone(),
                SettledTask::new(text, Some(&task.status), false),
            );
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
///   sub-agents have finished - the wait is then instant.)
/// - `timeout_secs` (number, optional): Maximum seconds to wait. Default: 300.
///
/// A `suspended` task is still awaited: only a terminal status settles a wait
/// (FR-030), so suspending a task neither frees its limit slot nor unblocks the
/// wait that is waiting on it - only a resume that finishes, a completion or a
/// cancellation does.
///
/// Because a parked task is awaited but produces nothing, a wait that returns
/// while it is parked reports it under `suspended` / `suspended_count` (and in
/// `unsettled`, with its own `status`), never under `still_running` /
/// `still_running_count` and never as "still running" in the content, so the
/// caller can tell that the task will not progress without a resume (FR-015).
pub struct WaitAgentsTool;

#[async_trait::async_trait]
impl Tool for WaitAgentsTool {
    fn name(&self) -> &'static str {
        "wait_agents"
    }

    /// Returns the generated description of the tool (FR-016).
    ///
    /// It names every property in [`Self::parameters_schema`];
    /// `test_tool_descriptions` asserts that each one appears here.
    fn description(&self) -> &'static str {
        "Wait for one or more background sub-agent tasks to complete. No required \
                 parameters. PARAMETERS: `task_ids` (array of strings) names the tasks to \
                 wait for, and `timeout_secs` (number, default 300) bounds the wait before \
                 partial results are returned. Returns full results for all awaited tasks \
                 (and any tasks that already completed). Every settled task carries its \
                 canonical `status` (`completed`, `failed`, `cancelled`, `timed_out`) in \
                 this tool's output and metadata, so a cancelled task is never reported \
                 as a failure. A task that is still unsettled when the wait returns is \
                 reported with its own `status` too: a parked (suspended) task is \
                 reported as `suspended` under `suspended` / `suspended_count` and is \
                 never described as still running, because nothing progresses for it \
                 until it is resumed or cancelled. A completed agent's FULL \
                 untruncated report is also written to a durable file at \
                 log/subagents/<task-id>.md - its path appears in this tool's output and \
                 metadata as output_file; if the combined report text is cut by the generic \
                 ~12k context truncation, recover the omitted findings with the `read` tool \
                 against that file (the metadata \"results\" array also mirrors every \
                 agent's complete output). When a report could NOT be persisted the absence \
                 is reported, never omitted: the results entry carries \
                 `report_persisted: false` and `report_location: \"in_memory_only\"`, its \
                 `output_file` is null, and the content prints a `[memory]` line saying the \
                 report is in-memory only - so never `read` a path for such a task. \
                 Use this tool instead of polling with \
                 list_agents; omitting task_ids waits for ALL running background tasks. \
                 A parameter this schema does not declare is refused, not ignored. \
                 Common gotcha: this tool is for new_agent sub-agents spawned with \
                 background: true, NOT for team members - for teams, always use \
                 team_wait. A DETACHED task (`new_agent` with `background: true, \
                 detached: true`) can never be awaited: naming one in `task_ids` is \
                 refused with an error that names the task as detached and points to its \
                 durable report file (`log/subagents/<task-id>.md`), rather than reporting \
                 the id as unknown."
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
    /// Returns an error if the call carries a property the schema does not
    /// declare (FR-004, FR-038), if `task_ids` is not an array of strings, or
    /// if `timeout_secs` is not a non-negative integer - these are checked
    /// before the environment. Also errors if the `AgentManager` is not
    /// initialized, if a requested task id names a DETACHED task (FR-044 - the
    /// refusal names the task as detached and points to its durable report
    /// file), or if the wait operation times out.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // FR-004 / FR-038: an undeclared property is refused, not ignored, and
        // the call shape is checked before the environment so a bad call
        // reports the call, not a missing manager.
        super::param_schema::validate_tool_input(&self.parameters_schema(), &input)?;

        let timeout_secs = input
            .get("timeout_secs")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(300);

        // FR-002: a declared parameter is honoured or refused - an element of
        // `task_ids` that is not a string used to be dropped silently, which
        // waited on a different set of tasks than the caller asked for.
        // FR-010: a refusal a caller could branch on carries a reason code.
        let requested = match input.get("task_ids") {
            None => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .enumerate()
                .map(|(index, item)| match item.as_str() {
                    Some(id) => Ok(id.to_string()),
                    None => Err(super::param_schema::reason_error(
                        REASON_INVALID_TASK_IDS,
                        format!(
                            "Invalid `task_ids[{index}]`: expected a string task id, got {}. \
                             Declared parameters are: `task_ids`, `timeout_secs`.",
                            describe_json_kind(item)
                        ),
                    )),
                })
                .collect::<Result<Vec<String>>>()?,
            Some(other) => {
                return Err(super::param_schema::reason_error(
                    REASON_INVALID_TASK_IDS,
                    format!(
                        "Invalid `task_ids`: expected an array of task id strings, got {}. \
                         Omit `task_ids` to wait for every background task in this session.",
                        describe_json_kind(other)
                    ),
                ));
            }
        };

        let agent_manager = ctx.agent_manager.as_ref().ok_or_else(|| {
            super::param_schema::reason_error(
                super::new_agent::REASON_MANAGER_UNAVAILABLE,
                "AgentManager not initialised in this context.",
            )
        })?;

        // FR-044: a detached (fire-and-forget) task can never be awaited, so
        // asking for one is refused by contract rather than treated as an
        // unknown id. `list_agents` deliberately excludes detached entries, so
        // the lookup is by id against the whole registry; `task_ids` is a
        // `HashSet`-bound set, so the check runs before any waiter is counted
        // or any event subscription is made - a refused call leaves no trace.
        for task_id in &requested {
            if let Some(task) = agent_manager.get_task(task_id).await
                && task.detached
            {
                return Err(detached_refusal(&task));
            }
        }

        // Subscribe to the event bus BEFORE reading current state to eliminate
        // the race between "task completes" and "we start listening".
        let mut rx = ctx.event_bus.subscribe();

        let all_tasks = agent_manager.list_agents(&ctx.session_id).await;

        // Include already-completed background tasks: even though they are no
        // longer "running", the caller asked to wait on them and expects
        // their results in the returned content/metadata (previously fixed
        // paths at lines 120-131 & 169-174 were unreachable for this case
        // because waiting_for only seeded Running tasks).  Without this the
        // tool reports "No running background tasks to wait for." even when
        // completed results exist for this session.
        //
        // `is_settled` rather than a hardcoded status list: every terminal
        // status is collectable, so a task that hit its per-call time bound
        // (`timed_out`, FR-017) is reported like one that was cancelled or
        // failed instead of silently vanishing from the count.
        let mut waiting_for: HashSet<String> = if requested.is_empty() {
            all_tasks
                .iter()
                .filter(|t| t.background && is_settled(&t.status))
                .map(|t| t.id.clone())
                .collect()
        } else {
            requested.into_iter().collect()
        };

        if waiting_for.is_empty() {
            return Ok(ToolOutput {
                content: "No running background tasks to wait for. Spawn one with \
                          `new_agent(background: true)`, or list the tasks this session can \
                          see with `list_agents`."
                    .to_string(),
                // FR-010: the outcome is named, so a caller can branch on it
                // without reading the prose.
                metadata: Some(super::param_schema::with_reason(
                    Some(json!({ "count": 0 })),
                    REASON_WAIT_SETTLED,
                )),
            });
        }

        // Collect results for tasks that had already settled before we
        // subscribed. `is_settled` - not `!= Running` - so a parked
        // (`suspended`) task is not collected as an empty completion; it stays
        // in flight and is reported under `suspended` below (FR-015).
        let mut results: HashMap<String, SettledTask> = HashMap::new();
        for task in &all_tasks {
            if waiting_for.contains(&task.id) && is_settled(&task.status) {
                let text = task
                    .result
                    .as_deref()
                    .or(task.error.as_deref())
                    .unwrap_or("(no output)")
                    .to_string();
                results.insert(
                    task.id.clone(),
                    SettledTask::new(text, Some(&task.status), false),
                );
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
                                let entry = agent_manager.get_task(&task_id).await;
                                let text = entry
                                    .as_ref()
                                    .and_then(|e| e.result.clone().or_else(|| e.error.clone()))
                                    .map_or_else(|| "(no output)".to_string(), |s| s.to_string());
                                // Prefer the status the registry recorded over the
                                // event's boolean: the registry is what distinguishes
                                // `cancelled` from `failed` (FR-014).
                                let status = entry.as_ref().map(|e| &e.status);
                                results.insert(
                                    task_id,
                                    SettledTask::new(text, status, success),
                                );
                            }
                            Ok(_) => {
                                // Unrelated event - keep waiting.
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

        // FR-015: the tasks still in flight when the wait returns are named by
        // their real state. `still_running_count` / `still_running` keep only
        // the tasks that are actually working; a parked task is reported under
        // `suspended_count` / `suspended` with the status `suspended`, so a
        // caller is never told that a parked task is running.
        let in_flight: Vec<&TaskEntry> = all_tasks
            .iter()
            .filter(|t| waiting_for.contains(&t.id))
            .collect();
        let (still_running, suspended) = in_flight_breakdown(&in_flight);

        let mut output = String::new();

        if timed_out {
            output.push_str(&partial_result_headline(
                timeout_secs,
                &in_flight,
                results.len(),
            ));
        }

        output.push_str(&format!("{} task(s) completed:\n\n", results.len()));

        // Build output text and metadata in a single pass.
        let mut task_details = Vec::new();
        let mut results_meta = Vec::new();
        for (task_id, settled) in &results {
            let task = all_tasks.iter().find(|t| &t.id == task_id);
            let agent_name = task.map_or("unknown", |t| t.agent_name.as_str());
            let text = &settled.text;
            let success = settled.succeeded();
            // FR-007 / FR-014: the canonical `TaskStatus::as_str` value the
            // registry recorded - never a re-derived label, so `cancelled` and
            // `failed` stay distinguishable.
            let status_label = settled.status_label();

            let icon = if success { "[ok]" } else { "[x]" };
            let short_id: String = ragent_types::strutil::short_prefix(task_id, 8);

            // Report integrity: sub-agent tasks track whether the final
            // reply was fully generated, salvaged via the continuation
            // retry, or was forcibly cut. Older entries default to
            // `Complete` so the marker stays quiet for them.
            let report_status =
                task.map_or(crate::task::ReportStatus::Complete, |t| t.report_status);
            let report_label = report_status.as_str();

            // Full per-agent output in metadata - survives the generic
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
            // FR-018 / FR-043: the absence of a durable copy is a reported
            // fact, not a missing field. `report_persisted: false` says the
            // write did not land, so a caller reading `output_file: null` can
            // no longer mistake it for "no path applies" and go on to `read` a
            // file that does not exist.
            let report_persisted = output_file_str.is_some();
            results_meta.push(json!({
                "task_id": task_id,
                "agent": agent_name,
                "success": success,
                "status": status_label,
                "output": text,
                "output_file": output_file_str,
                "report_persisted": report_persisted,
                "report_location": if report_persisted {
                    "durable"
                } else {
                    "in_memory_only"
                },
                "report_status": report_label,
            }));

            output.push_str(&format!(
                "{icon} **{agent_name}** (task {short_id}, {status_label}):\n{text}\n"
            ));
            if let Some(ref file_path) = output_file_str {
                output.push_str(&format!(
                    "\n[file] Full report: {file_path} (read this file if the text above was truncated)\n"
                ));
            } else {
                // FR-018: name the absence in the content too, so the model
                // reading only the prose still learns that the report it just
                // received is the only copy there will ever be.
                output.push_str(&format!(
                    "\n[memory] No durable copy: {IN_MEMORY_REPORT_NOTE}.\n"
                ));
            }
            if report_status != crate::task::ReportStatus::Complete {
                let note = match report_status {
                    crate::task::ReportStatus::Continued => {
                        "[ok] Report was truncated by the provider; the missing tail was \
                         regenerated by the automatic continuation retry."
                    }
                    crate::task::ReportStatus::Truncated => {
                        "[!]  Report is TRUNCATED: the provider cut the reply off before it \
                         finished and the continuation retry could not recover it. Treat \
                         this report as incomplete - re-run the task with a narrower scope \
                         or a model with a larger output window if you need the full text."
                    }
                    crate::task::ReportStatus::Complete => "",
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
                    "status": status_label,
                    "elapsed_ms": elapsed_ms,
                    "output_lines": output_lines,
                    "report_status": report_label,
                    // FR-018 / FR-043: the per-task roll-up states the same
                    // fact as the results entry, so a caller reading only
                    // `tasks` cannot conclude that a report was persisted.
                    "report_persisted": task.output_file.is_some(),
                }));
            }
        }

        // Per-task entries for the tasks that did NOT settle. A parked task is
        // reported with its real status so the caller can see that it is
        // suspended rather than running (FR-015).
        let unsettled_meta: Vec<Value> = in_flight
            .iter()
            .map(|t| {
                json!({
                    "task_id": &t.id,
                    "agent": &t.agent_name,
                    "status": t.status.as_str(),
                })
            })
            .collect();

        Ok(ToolOutput {
            content: output,
            // FR-010: a timed-out wait is a branchable outcome, so it is named
            // with a stable reason code rather than only by the `timed_out`
            // boolean and the prose above.
            metadata: Some(super::param_schema::with_reason(
                Some(json!({
                    "completed_count": results.len(),
                    "timed_out": timed_out,
                    // FR-015: only the tasks that are actually working.
                    "still_running_count": still_running.len(),
                    "still_running": still_running,
                    // FR-015: parked tasks are counted and named separately -
                    // they are unsettled, but nothing progresses until they are
                    // resumed or cancelled.
                    "suspended_count": suspended.len(),
                    "suspended": suspended,
                    "unsettled": unsettled_meta,
                    "tasks": task_details,
                    // Full untruncated per-agent results. The `content` string
                    // above may be shortened by the generic 12k tool-result
                    // truncation in `tool_result_content_for_llm`; these
                    // entries keep every agent's complete output accessible.
                    "results": results_meta,
                })),
                if timed_out {
                    REASON_WAIT_TIMED_OUT
                } else {
                    REASON_WAIT_SETTLED
                },
            )),
        })
    }
}

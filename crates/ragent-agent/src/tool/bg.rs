//! `bg` shell task manager tool.
//!
//! Exposes a single tool that can spawn shell commands in the background and
//! list/status/output/tail/cancel/wait/cleanup them. The heavy lifting is done
//! by [`BackgroundTaskService`] in `crate::background`.
//!
//! The tool owns the "shell task" vocabulary (FR-019): its rows live in the
//! background-task store, a different id space from the sub-agent task registry,
//! so every message it produces says "shell task" and never "background task",
//! which is the sub-agent execution contract's name.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::background::BackgroundTaskService;
use crate::tool::{Tool, ToolContext, ToolOutput};

/// Machine-readable reason for a `bg` action the tool does not implement
/// (FR-010, FR-019 - `bg` owns shell-task vocabulary only).
pub const REASON_UNKNOWN_ACTION: &str = "unknown_action";
/// Machine-readable reason for a declared property supplied to an action that
/// does not read it (FR-002, FR-010).
pub const REASON_PARAMETER_NOT_ACCEPTED: &str = "parameter_not_accepted";
/// Machine-readable reason for a `bg` call where no background service is wired
/// in (FR-010).
pub const REASON_SERVICE_UNAVAILABLE: &str = "service_unavailable";
/// Machine-readable reason for a `session_id` that names a session other than
/// the calling one (FR-010, FR-019).
pub const REASON_WRONG_SESSION: &str = "wrong_session";

/// Tool name used by the LLM.
pub const BG_TOOL_NAME: &str = "bg";

/// Shell task manager (`bg`).
pub struct BgTool;

impl BgTool {
    /// Create a new `bg` tool instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for BgTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for BgTool {
    fn name(&self) -> &'static str {
        BG_TOOL_NAME
    }

    fn description(&self) -> &'static str {
        "Manage shell tasks: spawn, list, status, output, tail, cancel, wait, cleanup. A shell \
         task is a shell command this tool runs in the background - it is not a sub-agent, so \
         use `new_agent` for a sub-agent and `agent_complete` to end a run. \
         REQUIRED parameter: 'action' (string enum). Conditional required parameters: for \
         action='spawn' provide 'command'; for status, output, tail, cancel, wait provide 'task_id'. \
         Optional parameters include 'working_dir' (spawn), 'status' and 'limit' (list), \
         'lines' (tail), 'timeout' (wait), 'older_than_minutes' and 'completed_only' (cleanup), \
         and 'session_id' (list/cleanup; it must name the calling session and behaves exactly \
         as omitting it). Common gotcha: unknown action values are rejected."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["spawn", "list", "status", "output", "tail", "cancel", "wait", "cleanup"],
                    "description": "Action to perform"
                },
                "command": {
                    "type": "string",
                    "description": "Shell command to spawn (required for action=spawn)"
                },
                "task_id": {
                    "type": "string",
                    "description": "Task id (required for status, output, tail, cancel, wait)"
                },
                "status": {
                    "type": "string",
                    "description": "Filter by status for list (running/completed/failed/cancelled)"
                },
                "lines": {
                    "type": "integer",
                    "description": "Number of lines to return for tail (default: 20)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds for wait (default: 60)"
                },
                "older_than_minutes": {
                    "type": "integer",
                    "description": "Cleanup tasks older than this many minutes (default: 60)"
                },
                "completed_only": {
                    "type": "boolean",
                    "description": "Only cleanup completed/failed/cancelled tasks (default: true)"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory for spawn (default: session working directory)"
                },
                "session_id": {
                    "type": "string",
                    "description": "Session id for list/cleanup (default: current session). It must name the calling session; a foreign id is refused"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of tasks to return for list (default: 50)"
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "bash:execute"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // FR-004 / FR-038: an undeclared property is refused, not ignored, and
        // the call shape is checked before the environment so a bad call
        // reports the call, not a missing service.
        super::param_schema::validate_tool_input(&self.parameters_schema(), &input)?;

        let action = input["action"]
            .as_str()
            .context("Missing required 'action' parameter")?;

        reject_parameters_for_action(action, &input)?;

        let service = ctx.bg_service.as_ref().ok_or_else(|| {
            super::param_schema::reason_error(
                REASON_SERVICE_UNAVAILABLE,
                "The shell-task service is not available in this context. `bg` manages shell \
                 tasks, so it needs a running session with the background task service \
                 initialised; sub-agents are spawned with `new_agent` instead.",
            )
        })?;

        match action {
            "spawn" => handle_spawn(input, ctx, service).await,
            "list" => handle_list(input, ctx, service).await,
            "status" => handle_status(input, service).await,
            "output" => handle_output(input, service).await,
            "tail" => handle_tail(input, service).await,
            "cancel" => handle_cancel(input, service).await,
            "wait" => handle_wait(input, service).await,
            "cleanup" => handle_cleanup(input, ctx, service).await,
            _ => {
                return Err(super::param_schema::reason_error(
                    REASON_UNKNOWN_ACTION,
                    format!(
                        "Unknown bg action: {action}. Supported actions are: {}.",
                        ACTION_PARAMETERS
                            .iter()
                            .map(|(name, _)| format!("`{name}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        }
    }
}

/// The parameters each `bg` action actually reads.
///
/// `bg` declares twelve properties in one schema but each action consumes a
/// subset. A declared property supplied to an action that does not read it is
/// refused here rather than accepted and ignored (FR-002): `lines` on `spawn`,
/// or `command` on `list`, used to pass silently while doing nothing.
///
/// `action` itself is always read and is not listed.
const ACTION_PARAMETERS: &[(&str, &[&str])] = &[
    ("spawn", &["command", "working_dir"]),
    ("list", &["status", "limit", "session_id"]),
    ("status", &["task_id"]),
    ("output", &["task_id"]),
    ("tail", &["task_id", "lines"]),
    ("cancel", &["task_id"]),
    ("wait", &["task_id", "timeout"]),
    (
        "cleanup",
        &["older_than_minutes", "completed_only", "session_id"],
    ),
];

/// Refuses a property that the selected `bg` action does not read.
///
/// # Errors
///
/// Returns an error naming every property that `action` does not consume and
/// listing the ones it does. An unknown `action` is left to the dispatch
/// `match` so the "Unknown bg action" message is produced in one place.
fn reject_parameters_for_action(action: &str, input: &Value) -> Result<()> {
    let Some((_, accepted)) = ACTION_PARAMETERS.iter().find(|(name, _)| *name == action) else {
        return Ok(());
    };

    let mut unused: Vec<&str> = input
        .as_object()
        .map(|map| {
            map.keys()
                .map(String::as_str)
                .filter(|key| *key != "action" && !accepted.contains(key))
                .collect()
        })
        .unwrap_or_default();
    if unused.is_empty() {
        return Ok(());
    }
    unused.sort_unstable();

    Err(super::param_schema::reason_error(
        REASON_PARAMETER_NOT_ACCEPTED,
        format!(
            "Parameter(s) not accepted by bg action '{action}': {}. Action '{action}' accepts: \
             `action`, {}.",
            unused
                .iter()
                .map(|key| format!("`{key}`"))
                .collect::<Vec<_>>()
                .join(", "),
            accepted
                .iter()
                .map(|key| format!("`{key}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ))
}

/// Resolves the session a `list`/`cleanup` call acts on.
///
/// # Errors
///
/// Returns a [`REASON_WRONG_SESSION`] refusal when `session_id` names a session
/// other than the calling one (FR-019): `bg` reads and deletes only the caller's
/// own shell tasks, so a foreign id could otherwise list another session's
/// commands or clean them up.
fn resolve_scoped_session<'a>(input: &'a Value, ctx: &'a ToolContext) -> Result<&'a str> {
    match input["session_id"].as_str() {
        // FR-037: the caller's own id behaves exactly as omitting it.
        Some(id) if id == ctx.session_id => Ok(ctx.session_id.as_str()),
        Some(id) => {
            // FR-019: refuse the override; the caller's own id is always accepted.
            Err(super::param_schema::reason_error(
                REASON_WRONG_SESSION,
                format!(
                    "session_id '{id}' is not the calling session '{}'. The `bg` tool manages \
                     shell tasks for the session that called it only - omit `session_id` (or \
                     pass '{}') to operate on this session's shell tasks.",
                    ctx.session_id, ctx.session_id
                ),
            ))
        }
        None => Ok(ctx.session_id.as_str()),
    }
}

async fn handle_spawn(
    input: Value,
    ctx: &ToolContext,
    service: &BackgroundTaskService,
) -> Result<ToolOutput> {
    let command = input["command"]
        .as_str()
        .context("Missing required 'command' parameter for spawn")?;
    let working_dir = input["working_dir"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| ctx.working_dir.clone());

    let task_id = service
        .spawn(&ctx.session_id, command, &working_dir)
        .await?;

    Ok(ToolOutput {
        content: format!("Spawned shell task {task_id}: {command}"),
        metadata: Some(json!({
            "task_id": task_id,
            "action": "spawn",
            "command": command,
        })),
    })
}

async fn handle_list(
    input: Value,
    ctx: &ToolContext,
    service: &BackgroundTaskService,
) -> Result<ToolOutput> {
    let session_id = resolve_scoped_session(&input, ctx)?;
    let status = input["status"].as_str();
    let limit = input["limit"].as_u64().map(|n| n as usize).unwrap_or(50);

    let rows = service.list(Some(session_id), status, limit).await?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "session_id": r.session_id,
                "command": r.command,
                "status": r.status,
                "exit_code": r.exit_code,
                "created_at": r.created_at,
                "updated_at": r.updated_at,
                "completed_at": r.completed_at,
            })
        })
        .collect();

    Ok(ToolOutput {
        content: format!("Found {} shell task(s)", items.len()),
        metadata: Some(json!({ "tasks": items, "action": "list" })),
    })
}

async fn handle_status(input: Value, service: &BackgroundTaskService) -> Result<ToolOutput> {
    let task_id = input["task_id"]
        .as_str()
        .context("Missing required 'task_id' parameter for status")?;
    let row = service.status(task_id).await?;
    Ok(ToolOutput {
        content: format!(
            "Task {}: status={}, exit_code={:?}",
            row.id, row.status, row.exit_code
        ),
        metadata: Some(json!({
            "task_id": row.id,
            "action": "status",
            "status": row.status,
            "exit_code": row.exit_code,
            "progress": row.progress_json,
            "updated_at": row.updated_at,
            "completed_at": row.completed_at,
        })),
    })
}

async fn handle_output(input: Value, service: &BackgroundTaskService) -> Result<ToolOutput> {
    let task_id = input["task_id"]
        .as_str()
        .context("Missing required 'task_id' parameter for output")?;
    let (_row, stdout, stderr) = service.output(task_id).await?;
    Ok(ToolOutput {
        content: format!("STDOUT:\n{stdout}\n\nSTDERR:\n{stderr}"),
        metadata: Some(json!({
            "task_id": task_id,
            "action": "output",
        })),
    })
}

async fn handle_tail(input: Value, service: &BackgroundTaskService) -> Result<ToolOutput> {
    let task_id = input["task_id"]
        .as_str()
        .context("Missing required 'task_id' parameter for tail")?;
    let lines = input["lines"].as_u64().map(|n| n as usize).unwrap_or(20);
    let tail = service.tail(task_id, lines).await?;
    Ok(ToolOutput {
        content: tail,
        metadata: Some(json!({
            "task_id": task_id,
            "action": "tail",
            "lines": lines,
        })),
    })
}

async fn handle_cancel(input: Value, service: &BackgroundTaskService) -> Result<ToolOutput> {
    let task_id = input["task_id"]
        .as_str()
        .context("Missing required 'task_id' parameter for cancel")?;
    service.cancel(task_id).await?;
    Ok(ToolOutput {
        content: format!("Cancelled shell task {task_id}"),
        metadata: Some(json!({
            "task_id": task_id,
            "action": "cancel",
        })),
    })
}

async fn handle_wait(input: Value, service: &BackgroundTaskService) -> Result<ToolOutput> {
    let task_id = input["task_id"]
        .as_str()
        .context("Missing required 'task_id' parameter for wait")?;
    let timeout = input["timeout"].as_u64().unwrap_or(60);
    let row = service.wait(task_id, timeout).await?;
    Ok(ToolOutput {
        content: format!(
            "Task {} finished with status={} exit_code={:?}",
            row.id, row.status, row.exit_code
        ),
        metadata: Some(json!({
            "task_id": row.id,
            "action": "wait",
            "status": row.status,
            "exit_code": row.exit_code,
        })),
    })
}

async fn handle_cleanup(
    input: Value,
    ctx: &ToolContext,
    service: &BackgroundTaskService,
) -> Result<ToolOutput> {
    let session_id = resolve_scoped_session(&input, ctx)?;
    let older_than = input["older_than_minutes"].as_i64().unwrap_or(60);
    let completed_only = input["completed_only"].as_bool().unwrap_or(true);
    let count = service
        .cleanup(Some(session_id), older_than, completed_only)
        .await?;
    Ok(ToolOutput {
        content: format!("Cleaned up {count} shell task(s)"),
        metadata: Some(json!({
            "action": "cleanup",
            "deleted": count,
        })),
    })
}

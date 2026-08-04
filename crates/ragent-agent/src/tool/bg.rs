//! `bg` background task manager tool.
//!
//! Exposes a single tool that can spawn shell commands in the background and
//! list/status/output/tail/cancel/wait/cleanup them. The heavy lifting is done
//! by [`BackgroundTaskService`] in `crate::background`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::background::BackgroundTaskService;
use crate::tool::{Tool, ToolContext, ToolOutput};

/// Tool name used by the LLM.
pub const BG_TOOL_NAME: &str = "bg";

/// Background task manager.
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
        "Manage background shell tasks: spawn, list, status, output, tail, cancel, wait, cleanup. \
         Use this for long-running commands that should continue while the agent does other work. \
         REQUIRED parameter: 'action' (string enum). Conditional required parameters: for \
         action='spawn' provide 'command'; for status, output, tail, cancel, wait provide 'task_id'. \
         Optional filters include 'status' (list), 'lines' (tail), 'timeout' (wait), and \
         'older_than_minutes' (cleanup). Common gotcha: unknown action values are rejected."
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
                    "description": "Override session id for list/cleanup (default: current session)"
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
        let service = ctx
            .bg_service
            .as_ref()
            .context("Background task service is not available")?;

        let action = input["action"]
            .as_str()
            .context("Missing required 'action' parameter")?;

        match action {
            "spawn" => handle_spawn(input, ctx, service).await,
            "list" => handle_list(input, ctx, service).await,
            "status" => handle_status(input, service).await,
            "output" => handle_output(input, service).await,
            "tail" => handle_tail(input, service).await,
            "cancel" => handle_cancel(input, service).await,
            "wait" => handle_wait(input, service).await,
            "cleanup" => handle_cleanup(input, ctx, service).await,
            _ => anyhow::bail!("Unknown bg action: {action}"),
        }
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
        content: format!("Spawned background task {task_id}: {command}"),
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
    let session_id = input["session_id"]
        .as_str()
        .or(Some(ctx.session_id.as_str()));
    let status = input["status"].as_str();
    let limit = input["limit"].as_u64().map(|n| n as usize).unwrap_or(50);

    let rows = service.list(session_id, status, limit).await?;
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
        content: format!("Found {} background task(s)", items.len()),
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
        content: format!("Cancelled background task {task_id}"),
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
    let session_id = input["session_id"]
        .as_str()
        .or(Some(ctx.session_id.as_str()));
    let older_than = input["older_than_minutes"].as_i64().unwrap_or(60);
    let completed_only = input["completed_only"].as_bool().unwrap_or(true);
    let count = service
        .cleanup(session_id, older_than, completed_only)
        .await?;
    Ok(ToolOutput {
        content: format!("Cleaned up {count} background task(s)"),
        metadata: Some(json!({
            "action": "cleanup",
            "deleted": count,
        })),
    })
}

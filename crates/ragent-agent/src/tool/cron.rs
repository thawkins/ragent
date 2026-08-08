//! Cron scheduler tools — LLM-callable equivalents of the `/cron` slash commands.
//!
//! These tools give the model direct access to the cron scheduler so it can
//! create, list, remove, enable, and disable scheduled agent runs without
//! going through the TUI slash-command surface.
//!
//! # Tools
//!
//! - `cron_add`     — create a new scheduled event
//! - `cron_remove`  — delete an event by id
//! - `cron_list`    — list all events
//! - `cron_enable`  — enable an event
//! - `cron_disable` — disable an event
//!
//! All tools read/write through [`ragent_storage::Storage`] via the
//! [`ToolContext::storage`] handle, mirroring the storage methods used by the
//! `/cron` slash-command handler in the TUI.

use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::{Value, json};
use std::sync::Arc;

use super::{Tool, ToolContext, ToolOutput};

// ── cron_add ─────────────────────────────────────────────────────────────

/// Create a new scheduled cron event.
pub struct CronAddTool;

#[async_trait::async_trait]
impl Tool for CronAddTool {
    fn name(&self) -> &'static str {
        "cron_add"
    }

    fn description(&self) -> &'static str {
        "Create a new scheduled agent run (cron event). REQUIRED parameters: 'id' \
         (unique event name), 'agent' (agent type to run, e.g. 'general' or 'coder'), \
         'schedule' (schedule expression), and 'prompt' (the prompt the agent executes). \
         Schedule grammar supports three forms: 'at <timestamp>' (one-shot), \
         'from <timestamp> every <duration>' (repeating from a start time), and \
         'every <duration>' (repeating from now). Timestamps accept ISO-8601 \
         ('2025-01-15T09:00:00Z') or natural-language ('5pm', '5:30pm', '17:00', \
         '5am tomorrow'). Durations use <int><unit> where unit is m, h, d, w, or mo. \
         New events are enabled by default. Common gotcha: the 'id' must be unique; \
         inserting a duplicate id fails."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Unique event identifier (cronname). Used as the primary key."
                },
                "agent": {
                    "type": "string",
                    "description": "Agent type to run when the event fires, e.g. 'general', 'coder'."
                },
                "schedule": {
                    "type": "string",
                    "description": "Schedule expression. Forms: 'at <timestamp>', 'from <timestamp> every <duration>', 'every <duration>'."
                },
                "prompt": {
                    "type": "string",
                    "description": "The prompt the agent executes when the event fires."
                }
            },
            "required": ["id", "agent", "schedule", "prompt"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "cron:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let id = input["id"]
            .as_str()
            .context("Missing required 'id' parameter")?
            .to_string();
        let agent = input["agent"]
            .as_str()
            .context("Missing required 'agent' parameter")?
            .to_string();
        let schedule_expr = input["schedule"]
            .as_str()
            .context("Missing required 'schedule' parameter")?
            .to_string();
        let prompt = input["prompt"]
            .as_str()
            .context("Missing required 'prompt' parameter")?
            .to_string();

        let storage = ctx
            .storage
            .as_ref()
            .context("cron_add requires storage (SQLite) but none is available")?;
        let storage = Arc::clone(storage);
        let now = Utc::now();

        let parsed = ragent_types::parse_schedule(&schedule_expr, now)
            .map_err(|e| anyhow::anyhow!("Failed to parse schedule '{schedule_expr}': {e}"))?;

        let event = ragent_types::CronEvent::new(
            id,
            agent,
            prompt,
            parsed.schedule,
            schedule_expr,
            parsed.next_due,
        );

        let id_clone = event.id.clone();
        let agent_clone = event.agent_type.clone();
        let schedule_raw = event.schedule_raw.clone();
        let next_due = event.next_due.to_rfc3339();
        let prompt_preview = if event.prompt.len() > 80 {
            format!("{}…", &event.prompt[..80])
        } else {
            event.prompt.clone()
        };

        tokio::task::spawn_blocking(move || storage.insert_cron_event(&event))
            .await
            .context("cron_add storage task join error")??;

        let content = format!(
            "✅ Scheduled event created.\n\n\
             | Field | Value |\n|---|---|\n\
             | ID | `{id_clone}` |\n\
             | Agent | `{agent_clone}` |\n\
             | Schedule | `{schedule_raw}` |\n\
             | Next due | {next_due} |\n\
             | Prompt | \"{prompt_preview}\" |"
        );

        let metadata = json!({
            "id": id_clone,
            "agent": agent_clone,
            "schedule": schedule_raw,
            "next_due": next_due,
        });

        Ok(ToolOutput {
            content,
            metadata: Some(metadata),
        })
    }
}

// ── cron_remove ──────────────────────────────────────────────────────────

/// Delete a scheduled cron event by id.
pub struct CronRemoveTool;

#[async_trait::async_trait]
impl Tool for CronRemoveTool {
    fn name(&self) -> &'static str {
        "cron_remove"
    }

    fn description(&self) -> &'static str {
        "Delete a scheduled cron event by its id. REQUIRED parameter: 'id' (string). \
         Returns success if the event was removed, or a not-found message if no event \
         with that id exists. Common gotcha: deletion is permanent and cannot be undone."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The event id to delete."
                }
            },
            "required": ["id"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "cron:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let id = input["id"]
            .as_str()
            .context("Missing required 'id' parameter")?
            .to_string();

        let storage = ctx
            .storage
            .as_ref()
            .context("cron_remove requires storage (SQLite) but none is available")?;
        let storage = Arc::clone(storage);

        let removed = tokio::task::spawn_blocking(move || storage.delete_cron_event(&id))
            .await
            .context("cron_remove storage task join error")??;

        if removed {
            Ok(ToolOutput {
                content: format!("✅ Event `{}` removed.", input["id"].as_str().unwrap_or("")),
                metadata: Some(json!({ "id": input["id"], "removed": true })),
            })
        } else {
            Ok(ToolOutput {
                content: format!(
                    "⚠ Event `{}` not found.",
                    input["id"].as_str().unwrap_or("")
                ),
                metadata: Some(json!({ "id": input["id"], "removed": false })),
            })
        }
    }
}

// ── cron_list ────────────────────────────────────────────────────────────

/// List all scheduled cron events.
pub struct CronListTool;

#[async_trait::async_trait]
impl Tool for CronListTool {
    fn name(&self) -> &'static str {
        "cron_list"
    }

    fn description(&self) -> &'static str {
        "List all scheduled cron events. No required parameters. Returns a table of \
         all events with their id, agent, schedule, enabled status, next-due timestamp, \
         and prompt preview. Use this to discover existing scheduled runs before \
         modifying them."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "cron:read"
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let storage = ctx
            .storage
            .as_ref()
            .context("cron_list requires storage (SQLite) but none is available")?;
        let storage = Arc::clone(storage);

        let rows = tokio::task::spawn_blocking(move || storage.list_cron_events())
            .await
            .context("cron_list storage task join error")??;

        if rows.is_empty() {
            return Ok(ToolOutput {
                content: "ℹ️  No scheduled events.".to_string(),
                metadata: Some(json!({ "count": 0 })),
            });
        }

        let count = rows.len();
        let mut lines = vec![
            format!("Found {count} scheduled event(s):\n"),
            "| ID | Agent | Schedule | Enabled | Next Due | Prompt |".to_string(),
            "|---|---|---|---|---|---|".to_string(),
        ];

        for row in &rows {
            let prompt_preview = if row.prompt.len() > 40 {
                format!("{}…", &row.prompt[..40])
            } else {
                row.prompt.clone()
            };
            let enabled_str = if row.enabled { "✓" } else { "✗" };
            lines.push(format!(
                "| `{}` | `{}` | `{}` | {} | {} | \"{}\" |",
                row.id, row.agent_type, row.schedule_raw, enabled_str, row.next_due, prompt_preview,
            ));
        }

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(json!({ "count": count })),
        })
    }
}

// ── cron_enable / cron_disable ───────────────────────────────────────────

/// Enable a scheduled cron event.
pub struct CronEnableTool;

#[async_trait::async_trait]
impl Tool for CronEnableTool {
    fn name(&self) -> &'static str {
        "cron_enable"
    }

    fn description(&self) -> &'static str {
        "Enable a scheduled cron event so the scheduler will fire it. REQUIRED \
         parameter: 'id' (string). Returns success if the event was enabled, or a \
         not-found message if no event with that id exists."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The event id to enable."
                }
            },
            "required": ["id"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "cron:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let id = input["id"]
            .as_str()
            .context("Missing required 'id' parameter")?
            .to_string();

        let storage = ctx
            .storage
            .as_ref()
            .context("cron_enable requires storage (SQLite) but none is available")?;
        let storage = Arc::clone(storage);

        let updated =
            tokio::task::spawn_blocking(move || storage.set_cron_event_enabled(&id, true))
                .await
                .context("cron_enable storage task join error")??;

        if updated {
            Ok(ToolOutput {
                content: format!("✅ Event `{}` enabled.", input["id"].as_str().unwrap_or("")),
                metadata: Some(json!({ "id": input["id"], "enabled": true })),
            })
        } else {
            Ok(ToolOutput {
                content: format!(
                    "⚠ Event `{}` not found.",
                    input["id"].as_str().unwrap_or("")
                ),
                metadata: Some(json!({ "id": input["id"], "enabled": false })),
            })
        }
    }
}

/// Disable a scheduled cron event.
pub struct CronDisableTool;

#[async_trait::async_trait]
impl Tool for CronDisableTool {
    fn name(&self) -> &'static str {
        "cron_disable"
    }

    fn description(&self) -> &'static str {
        "Disable a scheduled cron event so the scheduler skips it. REQUIRED \
         parameter: 'id' (string). Returns success if the event was disabled, or a \
         not-found message if no event with that id exists."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The event id to disable."
                }
            },
            "required": ["id"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "cron:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let id = input["id"]
            .as_str()
            .context("Missing required 'id' parameter")?
            .to_string();

        let storage = ctx
            .storage
            .as_ref()
            .context("cron_disable requires storage (SQLite) but none is available")?;
        let storage = Arc::clone(storage);

        let updated =
            tokio::task::spawn_blocking(move || storage.set_cron_event_enabled(&id, false))
                .await
                .context("cron_disable storage task join error")??;

        if updated {
            Ok(ToolOutput {
                content: format!(
                    "⏸️  Event `{}` disabled.",
                    input["id"].as_str().unwrap_or("")
                ),
                metadata: Some(json!({ "id": input["id"], "enabled": false })),
            })
        } else {
            Ok(ToolOutput {
                content: format!(
                    "⚠ Event `{}` not found.",
                    input["id"].as_str().unwrap_or("")
                ),
                metadata: Some(json!({ "id": input["id"], "enabled": true })),
            })
        }
    }
}

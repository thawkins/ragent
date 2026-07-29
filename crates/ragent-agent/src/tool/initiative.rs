//! `initiative` — Durable, cross-session goals with milestone tracking.
//!
//! Implements JCODEPLAN M8 T-070: long-lived initiatives that persist in
//! SQLite (via `ragent-storage`) and survive compaction, session restarts,
//! and machine reboots. Unlike session-scoped `todo_read`/`todo_write` items,
//! initiatives are scoped to the *project* (working directory) so any future
//! session in the same directory can read and update them.
//!
//! # Actions
//!
//! - `create`     — register a new initiative with optional milestones
//! - `read`       — fetch a single initiative with full milestone list
//! - `update`     — adjust title / description / progress / status
//! - `checkpoint` — record progress: complete a milestone and/or bump overall
//!                  progress with an optional note about what was done
//! - `list`       — list initiatives (`active` by default, `all` to include
//!                  closed)
//! - `close`      — mark `completed` or `abandoned`
//!
//! # System-prompt surfacing
//!
//! [`build_initiatives_prompt_section`] renders active initiatives as a
//! system-prompt block injected on every turn so the agent remains aware of
//! long-term goals (see `session/loop_steps.rs`).

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::sync::Arc;

use super::{Tool, ToolContext, ToolOutput};
use crate::storage::{InitiativeMilestone, InitiativeRow, Storage};

/// Valid lifecycle statuses for an initiative.
const VALID_STATUSES: &[&str] = &["active", "paused", "completed", "abandoned"];

/// Tool for managing durable, cross-session initiatives (goals with milestones).
pub struct InitiativeTool;

#[async_trait::async_trait]
impl Tool for InitiativeTool {
    fn name(&self) -> &'static str {
        "initiative"
    }

    fn description(&self) -> &'static str {
        "Manage durable initiatives — long-lived project goals with milestones \
         that persist across sessions and compaction. Use for multi-week efforts \
         that should not be forgotten. Actions: create, read, update, checkpoint \
         (record progress / complete a milestone), list, close."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "read", "update", "checkpoint", "list", "close"],
                    "description": "Operation to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Initiative id (required for read/update/checkpoint/close). For 'create', supply a short slug (e.g. 'api-v2'); auto-generated when omitted"
                },
                "title": {
                    "type": "string",
                    "description": "Short goal title (required for create; optional for update)"
                },
                "description": {
                    "type": "string",
                    "description": "Detailed description / success criteria (optional for create/update)"
                },
                "milestones": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Milestone titles for create (e.g. [\"design\", \"implement\", \"ship\"])"
                },
                "milestone": {
                    "type": "string",
                    "description": "Milestone id to mark complete (checkpoint action)"
                },
                "progress": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 100,
                    "description": "Overall progress 0-100 (update/checkpoint)"
                },
                "note": {
                    "type": "string",
                    "description": "Free-text note recorded with a checkpoint (what was accomplished)"
                },
                "status": {
                    "type": "string",
                    "enum": ["active", "paused", "completed", "abandoned", "all"],
                    "description": "New status for update/close; filter for list (default: active)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum initiatives returned by list (default: 50)"
                }
            },
            "required": ["action"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "storage:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let action = input["action"]
            .as_str()
            .context("Missing required 'action' parameter")?
            .to_string();
        let storage_ref = ctx
            .storage
            .as_ref()
            .context("initiative requires storage (SQLite) but none is available")?;
        let storage = Arc::clone(storage_ref);
        // Project key = working directory string, so initiatives are shared
        // across all sessions in the same project but isolated across projects.
        let project = ctx.working_dir.to_string_lossy().to_string();
        let session_id = ctx.session_id.clone();

        // rusqlite is synchronous; offload to a blocking thread so the async
        // executor stays free.
        tokio::task::spawn_blocking(move || {
            dispatch(&storage, &project, &session_id, &action, &input)
        })
        .await
        .context("initiative storage task join error")?
    }
}

/// Dispatch the action against storage. Runs inside `spawn_blocking`.
fn dispatch(
    storage: &Storage,
    project: &str,
    session_id: &str,
    action: &str,
    input: &Value,
) -> Result<ToolOutput> {
    match action {
        "create" => action_create(storage, project, session_id, input),
        "read" => action_read(storage, project, input),
        "update" => action_update(storage, project, input),
        "checkpoint" => action_checkpoint(storage, project, input),
        "list" => action_list(storage, project, input),
        "close" => action_close(storage, project, input),
        other => Err(anyhow::anyhow!(
            "Unknown initiative action '{other}'. Valid actions: create, read, update, checkpoint, list, close"
        )),
    }
}

/// `create` — register a new initiative.
fn action_create(
    storage: &Storage,
    project: &str,
    session_id: &str,
    input: &Value,
) -> Result<ToolOutput> {
    let title = input["title"].as_str().context("create requires 'title'")?;
    let description = input["description"].as_str().unwrap_or("");
    let id = match input["id"].as_str() {
        Some(s) if !s.is_empty() => {
            validate_slug(s)?;
            s.to_string()
        }
        _ => {
            let suffix: String = uuid::Uuid::new_v4()
                .simple()
                .to_string()
                .chars()
                .take(8)
                .collect();
            format!("initiative-{suffix}")
        }
    };

    if storage.get_initiative(&id, project)?.is_some() {
        anyhow::bail!(
            "Initiative '{id}' already exists for this project. Use action=update or choose a different id."
        );
    }

    let milestones: Vec<InitiativeMilestone> = match input["milestones"].as_array() {
        Some(arr) => arr
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let title = v
                    .as_str()
                    .with_context(|| format!("milestones[{i}] must be a string"))?;
                Ok(InitiativeMilestone {
                    id: format!("ms-{}", i + 1),
                    title: title.to_string(),
                    done: false,
                    completed_at: None,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        None => Vec::new(),
    };

    storage.create_initiative(&id, title, description, &milestones, project, session_id)?;

    let content = format!(
        "Created initiative `{id}` — **{title}** ({} milestone(s)).\n\
         It is now persisted for this project and will appear in future sessions.",
        milestones.len()
    );
    Ok(ToolOutput {
        content,
        metadata: Some(json!({
            "action": "create",
            "id": id,
            "title": title,
            "milestone_count": milestones.len(),
        })),
    })
}

/// `read` — fetch one initiative.
fn action_read(storage: &Storage, project: &str, input: &Value) -> Result<ToolOutput> {
    let id = require_id(input, "read")?;
    let row = storage
        .get_initiative(&id, project)?
        .ok_or_else(|| anyhow::anyhow!("Initiative '{id}' not found for this project"))?;
    let content = render_full(&row);
    Ok(ToolOutput {
        content,
        metadata: Some(json!({
            "action": "read",
            "id": row.id,
            "status": row.status,
            "progress": row.progress,
        })),
    })
}

/// `update` — adjust mutable fields.
fn action_update(storage: &Storage, project: &str, input: &Value) -> Result<ToolOutput> {
    let id = require_id(input, "update")?;
    storage
        .get_initiative(&id, project)?
        .ok_or_else(|| anyhow::anyhow!("Initiative '{id}' not found for this project"))?;

    let title = input["title"].as_str();
    let description = input["description"].as_str();
    let progress = input["progress"]
        .as_u64()
        .map(|p| u8::try_from(p.min(100)).unwrap_or(100));
    let status = input["status"]
        .as_str()
        .map(|s| {
            if VALID_STATUSES.contains(&s) {
                Ok(s)
            } else {
                Err(anyhow::anyhow!(
                    "Invalid status '{s}'. Valid: {}",
                    VALID_STATUSES.join(", ")
                ))
            }
        })
        .transpose()?;

    if title.is_none() && description.is_none() && progress.is_none() && status.is_none() {
        anyhow::bail!("update requires at least one of: title, description, progress, status");
    }

    storage.update_initiative(
        &id,
        project,
        title,
        description,
        None,
        progress,
        status,
        None,
    )?;

    let row = storage
        .get_initiative(&id, project)?
        .ok_or_else(|| anyhow::anyhow!("Initiative '{id}' vanished after update"))?;

    Ok(ToolOutput {
        content: format!("Updated initiative `{id}`.\n\n{}", render_full(&row)),
        metadata: Some(json!({
            "action": "update",
            "id": id,
            "status": row.status,
            "progress": row.progress,
        })),
    })
}

/// `checkpoint` — record progress: complete a milestone and/or bump overall progress.
///
/// A `note` (what was accomplished) is appended to the description as a
/// timestamped `Checkpoint:` line so it never collides with the human-written
/// description text.
fn action_checkpoint(storage: &Storage, project: &str, input: &Value) -> Result<ToolOutput> {
    let id = require_id(input, "checkpoint")?;
    let mut row = storage
        .get_initiative(&id, project)?
        .ok_or_else(|| anyhow::anyhow!("Initiative '{id}' not found for this project"))?;

    if row.status != "active" {
        anyhow::bail!(
            "Initiative '{id}' is {} — checkpoints are only allowed on active initiatives",
            row.status
        );
    }

    let mut milestones = row.milestones();
    let mut completed_note = String::new();
    if let Some(ms_id) = input["milestone"].as_str() {
        let Some(ms) = milestones.iter_mut().find(|m| m.id == ms_id) else {
            let valid: Vec<String> = milestones.iter().map(|m| m.id.clone()).collect();
            anyhow::bail!(
                "Milestone '{ms_id}' not found on initiative '{id}'. Valid milestones: {}",
                if valid.is_empty() {
                    "(none)".to_string()
                } else {
                    valid.join(", ")
                }
            );
        };
        if ms.done {
            completed_note = format!(" Milestone '{ms_id}' was already complete.");
        } else {
            ms.done = true;
            ms.completed_at = Some(chrono::Utc::now().to_rfc3339());
            completed_note = format!(" Milestone '{ms_id}' marked complete.");
        }
    }

    let progress = input["progress"]
        .as_u64()
        .map(|p| u8::try_from(p.min(100)).unwrap_or(100));
    let note = input["note"].as_str();

    // Append the note as a timestamped checkpoint line in the description so
    // there is a durable audit trail of what happened at each checkpoint.
    let new_description = note.map(|n| {
        let ts = chrono::Utc::now().format("%Y-%m-%d %H:%M");
        if row.description.is_empty() {
            format!("Checkpoint {ts}: {n}")
        } else {
            format!("{}\nCheckpoint {ts}: {}", row.description, n)
        }
    });

    storage.update_initiative(
        &id,
        project,
        None,
        new_description.as_deref(),
        if input["milestone"].as_str().is_some() {
            Some(&milestones)
        } else {
            None
        },
        progress,
        None,
        None,
    )?;

    row = storage
        .get_initiative(&id, project)?
        .ok_or_else(|| anyhow::anyhow!("Initiative '{id}' vanished after checkpoint"))?;

    Ok(ToolOutput {
        content: format!(
            "Checkpoint recorded on `{id}` ({}%).{}\n\n{}",
            row.progress,
            completed_note,
            render_full(&row)
        ),
        metadata: Some(json!({
            "action": "checkpoint",
            "id": id,
            "progress": row.progress,
            "note": note,
        })),
    })
}

/// `list` — list initiatives (status-filtered, `active` by default).
///
/// `status="all"` clears the filter entirely (returns every initiative for
/// the project, regardless of status).
fn action_list(storage: &Storage, project: &str, input: &Value) -> Result<ToolOutput> {
    let show_all = input["status"].as_str() == Some("all");
    let filter = input["status"].as_str().filter(|s| *s != "all");
    if let Some(s) = filter
        && !VALID_STATUSES.contains(&s)
    {
        anyhow::bail!(
            "Invalid status filter '{s}'. Valid: {}, all",
            VALID_STATUSES.join(", ")
        );
    }
    // Default to active-only (so closed goals don't clutter the listing),
    // unless the caller passed "all" explicitly.
    let effective: Option<&str> = if show_all {
        None
    } else {
        filter.or(Some("active"))
    };
    let limit = usize::try_from(input["limit"].as_u64().unwrap_or(50)).unwrap_or(50);

    let rows = storage.list_initiatives(project, effective)?;
    if rows.is_empty() {
        let empty_label = if show_all {
            String::new()
        } else {
            format!("{} ", effective.unwrap_or("active"))
        };
        return Ok(ToolOutput {
            content: format!(
                "No {}initiatives for this project.\n\
                 Create one with: initiative action=\"create\" title=\"…\"",
                empty_label
            ),
            metadata: Some(json!({
                "action": "list",
                "status_filter": effective,
                "count": 0,
            })),
        });
    }

    let header_label = if show_all {
        "total"
    } else {
        effective.unwrap_or("active")
    };
    let mut lines = vec![format!(
        "Found {} {} initiative(s) for this project:\n",
        rows.len(),
        header_label
    )];
    lines.push(format!(
        "| {:<24} | {:<10} | {:>5} | {:<40} |",
        "ID", "Status", "Prog%", "Title"
    ));
    lines.push(format!("|{:-<26}|{:-<12}|{:-<7}|{:-<42}|", "", "", "", ""));
    for row in rows.iter().take(limit) {
        let ms = row.milestones();
        let done = ms.iter().filter(|m| m.done).count();
        let title =
            ragent_types::truncate_chars(&format!("{} [{}/{} ms]", row.title, done, ms.len()), 38);
        lines.push(format!(
            "| {:<24} | {:<10} | {:>4}% | {:<40} |",
            ragent_types::truncate_chars(&row.id, 22),
            row.status,
            row.progress,
            title
        ));
    }
    if rows.len() > limit {
        lines.push(format!("… and {} more (limit={limit})", rows.len() - limit));
    }

    Ok(ToolOutput {
        content: lines.join("\n"),
        metadata: Some(json!({
            "action": "list",
            "status_filter": effective,
            "count": rows.len(),
        })),
    })
}

/// `close` — mark completed or abandoned.
fn action_close(storage: &Storage, project: &str, input: &Value) -> Result<ToolOutput> {
    let id = require_id(input, "close")?;
    let status = input["status"].as_str().unwrap_or("completed");
    if status != "completed" && status != "abandoned" {
        anyhow::bail!("close requires status \"completed\" or \"abandoned\" (got '{status}')");
    }
    let progress = input["progress"]
        .as_u64()
        .map(|p| u8::try_from(p.min(100)).unwrap_or(100));
    // Default: jump to 100 % on completed when the caller didn't say otherwise.
    let progress = match (progress, status) {
        (Some(p), _) => Some(p),
        (None, "completed") => Some(100u8),
        _ => None,
    };

    storage
        .get_initiative(&id, project)?
        .ok_or_else(|| anyhow::anyhow!("Initiative '{id}' not found for this project"))?;

    storage.update_initiative(&id, project, None, None, None, progress, Some(status), None)?;

    Ok(ToolOutput {
        content: format!("Initiative `{id}` closed with status **{status}**."),
        metadata: Some(json!({
            "action": "close",
            "id": id,
            "status": status,
        })),
    })
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Require the `id` parameter for a given action.
fn require_id(input: &Value, action: &str) -> Result<String> {
    input["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .with_context(|| format!("{action} requires the 'id' parameter"))
}

/// Validate a user-supplied initiative slug (safe characters, bounded length).
fn validate_slug(slug: &str) -> Result<()> {
    if slug.len() > 64 {
        anyhow::bail!("Initiative id must be ≤ 64 characters (got {})", slug.len());
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!("Initiative id '{slug}' must contain only ASCII alphanumerics, '-' or '_'");
    }
    Ok(())
}

/// Render a full initiative for `read`/`update`/`checkpoint` output.
fn render_full(row: &InitiativeRow) -> String {
    let ms = row.milestones();
    let mut out = format!(
        "### `{}` — {}\n\n\
         **Status:** {}  **Progress:** {}%\n",
        row.id, row.title, row.status, row.progress
    );
    if !row.description.is_empty() {
        out.push_str(&format!("\n{}\n", row.description));
    }
    if ms.is_empty() {
        out.push_str("\n*(no milestones)*\n");
    } else {
        out.push_str("\n**Milestones:**\n");
        for m in &ms {
            let mark = if m.done { "[x]" } else { "[ ]" };
            out.push_str(&format!("- {} `{}` {}", mark, m.id, m.title));
            if let Some(ref ts) = m.completed_at {
                out.push_str(&format!("  ✓ {ts}"));
            }
            out.push('\n');
        }
    }
    out.push_str(&format!(
        "\n*created {} · updated {}{}*\n",
        row.created_at,
        row.updated_at,
        row.closed_at
            .as_ref()
            .map(|c| format!(" · closed {c}"))
            .unwrap_or_default()
    ));
    out
}

// ── System-prompt section ───────────────────────────────────────────

/// Build the `## Active Initiatives` system-prompt section.
///
/// Returns an empty string when there are no active initiatives (so the
/// caller can skip the section entirely). The block lists each active
/// initiative with progress and remaining milestones, keeping the agent
/// aware of long-term goals across turns and sessions.
///
/// Runs synchronously; call via `spawn_blocking` from async contexts.
#[must_use]
pub fn build_initiatives_prompt_section(
    storage: &Storage,
    working_dir: &std::path::Path,
) -> String {
    let project = working_dir.to_string_lossy().to_string();
    let rows = match storage.list_initiatives(&project, Some("active")) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to load initiatives for system prompt");
            return String::new();
        }
    };
    if rows.is_empty() {
        return String::new();
    }

    let mut section = String::from(
        "## Active Initiatives\n\n\
         The following durable goals were created in earlier sessions and are \
         still active for this project. Use the `initiative` tool to record \
         progress (`checkpoint`), adjust scope (`update`), or `close` them when done.\n\n",
    );
    for row in &rows {
        let ms = row.milestones();
        let pending: Vec<&str> = ms
            .iter()
            .filter(|m| !m.done)
            .map(|m| m.title.as_str())
            .collect();
        section.push_str(&format!(
            "- **`{}`** ({:>3}%) — {}",
            row.id, row.progress, row.title
        ));
        if !pending.is_empty() {
            let preview: Vec<&str> = pending.iter().take(3).copied().collect();
            let more = if pending.len() > 3 {
                format!(" … (+{} more)", pending.len() - 3)
            } else {
                String::new()
            };
            section.push_str(&format!(" · remaining: {}{}", preview.join(", "), more));
        }
        section.push('\n');
    }
    section.push('\n');
    section
}

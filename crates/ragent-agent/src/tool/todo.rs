//! Tools for reading and writing session TODO items.

use anyhow::{Result, bail};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Valid TODO status values (plus "all" for filtering).
const VALID_STATUSES: &[&str] = &["pending", "in_progress", "done", "blocked", "all"];
/// Status values valid for writing (excludes "all").
const WRITE_STATUSES: &[&str] = &["pending", "in_progress", "done", "blocked"];

/// Reads TODO items for the current session, optionally filtered by status.
pub struct TodoReadTool;

#[async_trait::async_trait]
impl Tool for TodoReadTool {
    fn name(&self) -> &'static str {
        "todo_read"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &'static str {
        "List TODO items for the current session, optionally filtered by status"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "description": "Filter by status: pending, in_progress, done, blocked, or all (default: all)",
                    "enum": ["pending", "in_progress", "done", "blocked", "all"]
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "todo"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let storage = ctx.storage.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Storage is not available. Cannot read TODO items without a storage backend."
            )
        })?;

        let status_filter = input["status"].as_str().unwrap_or("all");

        if !VALID_STATUSES.contains(&status_filter) {
            bail!(
                "Invalid status filter '{}'. Must be one of: {}",
                status_filter,
                VALID_STATUSES.join(", ")
            );
        }

        let filter = if status_filter == "all" {
            None
        } else {
            Some(status_filter)
        };

        let todos = storage
            .get_todos(&ctx.session_id, filter)
            .map_err(|e| anyhow::anyhow!("Failed to read todos: {e}"))?;

        let output = format_todo_list(&todos, status_filter);

        let metadata = json!({
            "count": todos.len(),
            "status_filter": status_filter,
        });

        Ok(ToolOutput {
            content: output,
            metadata: Some(metadata),
        })
    }
}

/// Adds, updates, removes, or clears TODO items for the current session.
pub struct TodoWriteTool;

#[async_trait::async_trait]
impl Tool for TodoWriteTool {
    /// # Errors
    ///
    /// Returns an error if the name string cannot be converted or returned.
    fn name(&self) -> &'static str {
        "todo_write"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &'static str {
        "Add, update, remove, or clear TODO items for the current session"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "The action to perform",
                "enum": ["add", "update", "remove", "clear", "complete", "completed", "done"]
                },
                "id": {
                    "type": "string",
                    "description": "TODO item ID (required for update/remove/complete/completed/done)"
                },
                "title": {
                    "type": "string",
                    "description": "Title for the TODO item (required for add, optional for update)"
                },
                "description": {
                    "type": "string",
                    "description": "Optional description"
                },
                "status": {
                    "type": "string",
                    "description": "Status: pending, in_progress, done, or blocked (default: pending for add)",
                    "enum": ["pending", "in_progress", "done", "blocked"]
                }
            },
            "required": ["action"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "todo"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let storage = ctx.storage.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Storage is not available. Cannot manage TODO items without a storage backend."
            )
        })?;

        let action = input["action"].as_str().ok_or_else(|| {
            anyhow::anyhow!(
                "Missing required 'action' parameter. Must be one of: add, update, remove, clear, complete"
            )
        })?;

        let (summary, action_label, affected_id) = match action {
            "add" => {
                let title = input["title"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing required 'title' for add action. Provide a title for the new TODO item."))?;
                if title.trim().is_empty() {
                    bail!("Title must not be empty");
                }
                let status = input["status"].as_str().unwrap_or("pending");
                if !WRITE_STATUSES.contains(&status) {
                    bail!(
                        "Invalid status '{}'. Must be one of: {}",
                        status,
                        WRITE_STATUSES.join(", ")
                    );
                }
                let description = input["description"].as_str().unwrap_or("");
                let id = input["id"]
                    .as_str()
                    .map_or_else(generate_todo_id, std::string::ToString::to_string);

                storage
                    .create_todo(&id, &ctx.session_id, title, status, description)
                    .map_err(|e| anyhow::anyhow!("Failed to add todo: {e}"))?;

                (
                    format!("Added todo '{id}' with status '{status}'"),
                    "add",
                    Some(id.to_string()),
                )
            }
            "update" => {
                let id = input["id"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing required 'id' for update action. Specify which TODO item to update."))?;
                let title = input["title"].as_str();
                let status = input["status"].as_str();
                let description = input["description"].as_str();

                if let Some(s) = status
                    && !WRITE_STATUSES.contains(&s)
                {
                    bail!(
                        "Invalid status '{}'. Must be one of: {}",
                        s,
                        WRITE_STATUSES.join(", ")
                    );
                }

                if title.is_none() && status.is_none() && description.is_none() {
                    bail!(
                        "At least one of title, status, or description must be provided for update"
                    );
                }

                let updated = storage
                    .update_todo(id, &ctx.session_id, title, status, description)
                    .map_err(|e| anyhow::anyhow!("Failed to update todo: {e}"))?;

                if !updated {
                    bail!("Todo '{id}' not found in this session");
                }

                (
                    format!("Updated todo '{id}'"),
                    "update",
                    Some(id.to_string()),
                )
            }
            "complete" | "completed" | "done" => {
                // Mark a specific todo as done by id.
                let id = input["id"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing required 'id' for complete action. Specify which TODO item to mark as done."))?;

                let updated = storage
                    .update_todo(id, &ctx.session_id, None, Some("done"), None)
                    .map_err(|e| anyhow::anyhow!("Failed to complete todo: {e}"))?;

                if !updated {
                    bail!("Todo '{id}' not found in this session");
                }

                (
                    format!("Marked todo '{id}' as done"),
                    "complete",
                    Some(id.to_string()),
                )
            }
            "remove" => {
                let id = input["id"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!(
                        "Missing required 'id' for remove action. Specify which TODO item to delete."
                    ))?;

                // Look up the todo before deletion so we can include its title in the summary.
                let existing = storage
                    .get_todos(&ctx.session_id, None)
                    .map_err(|e| anyhow::anyhow!("Failed to read todos: {e}"))?;
                let existing_title = existing
                    .iter()
                    .find(|t| t.id == id)
                    .map(|t| t.title.as_str())
                    .unwrap_or("");

                let removed = storage
                    .delete_todo(id, &ctx.session_id)
                    .map_err(|e| anyhow::anyhow!("Failed to remove todo: {e}"))?;

                if !removed {
                    bail!("Todo '{id}' not found in this session");
                }

                let summary = if existing_title.is_empty() {
                    format!("Removed todo '{id}'")
                } else {
                    format!("Removed todo '{id}' — \"{existing_title}\"")
                };
                (summary, "remove", None)
            }
            "clear" => {
                let count = storage
                    .clear_todos(&ctx.session_id)
                    .map_err(|e| anyhow::anyhow!("Failed to clear todos: {e}"))?;

                (
                    format!(
                        "Cleared {} todo item{}",
                        count,
                        if count == 1 { "" } else { "s" }
                    ),
                    "clear",
                    None,
                )
            }
            _ => bail!(
                "Invalid action '{action}'. Must be one of: add, update, remove, clear, complete, completed, done"
            ),
        };

        // Read back the current list after the write
        let todos = storage
            .get_todos(&ctx.session_id, None)
            .map_err(|e| anyhow::anyhow!("Failed to read todos: {e}"))?;

        // Enrich summary with the todo's title when we have an affected id.
        let summary = if let Some(ref id) = affected_id {
            if let Some(todo) = todos.iter().find(|t| &t.id == id) {
                format!("{summary} — \"{title}\"", title = todo.title)
            } else {
                summary
            }
        } else {
            summary
        };

        let mut output = format!("{summary}\n\n");
        output.push_str(&format_todo_list(&todos, "all"));

        // Determine the title of the affected todo for metadata enrichment.
        let title_for_metadata = if let Some(ref id) = affected_id {
            todos.iter().find(|t| &t.id == id).map(|t| t.title.clone())
        } else {
            None
        };

        let mut metadata = json!({
            "action": action_label,
            "count": todos.len(),
        });
        if let Some(title) = title_for_metadata {
            metadata
                .as_object_mut()
                .unwrap()
                .insert("title".to_string(), json!(title));
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(metadata),
        })
    }
}

/// Formats a list of TODO items as markdown.
fn format_todo_list(todos: &[crate::storage::TodoRow], status_filter: &str) -> String {
    let mut output = String::new();
    if todos.is_empty() {
        output.push_str("No TODO items found");
        if status_filter != "all" {
            output.push_str(&format!(" with status '{status_filter}'"));
        }
        output.push('.');
    } else {
        output.push_str(&format!("## TODOs ({} items)\n\n", todos.len()));
        for todo in todos {
            let status_icon = match todo.status.as_str() {
                "pending" => "⏳",
                "in_progress" => "🔄",
                "done" => "✅",
                "blocked" => "🚫",
                _ => "❓",
            };
            output.push_str(&format!(
                "- {} **{}** — {} `[{}]`\n",
                status_icon, todo.id, todo.title, todo.status
            ));
            if !todo.description.is_empty() {
                output.push_str(&format!("  {}\n", todo.description));
            }
        }
    }
    output
}

/// Generates a unique ID for a TODO item.
fn generate_todo_id() -> String {
    format!("todo-{}", uuid::Uuid::new_v4().simple())
}

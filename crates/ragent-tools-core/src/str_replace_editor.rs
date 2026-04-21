//! Claude-compatible multi-command file editor tool.
//!
//! Provides [`StrReplaceEditorTool`], which implements the same interface as
//! Anthropic's `str_replace_based_edit_tool`.  Models trained on Claude's tool
//! set emit calls to this tool naturally; providing it prevents "Unknown tool"
//! errors.
//!
//! ## Supported commands
//!
//! | `command`    | Description                                        |
//! |--------------|----------------------------------------------------|
//! | `view`       | Read file contents (with optional line range)      |
//! | `create`     | Create a new file with the given text              |
//! | `str_replace`| Replace an exact substring in a file               |
//! | `insert`     | Insert text after a specific line number           |
//! | `delete`     | Delete a range of lines                            |

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};
use super::{create, edit, read, write};

/// A Claude-compatible multi-command file editor.
pub struct StrReplaceEditorTool;

#[async_trait::async_trait]
impl Tool for StrReplaceEditorTool {
    fn name(&self) -> &'static str {
        "str_replace_editor"
    }

    fn description(&self) -> &'static str {
        "Multi-command file editor compatible with Anthropic's str_replace_based_edit_tool. \
         Supports commands: 'view' (read file), 'create' (new file), 'str_replace' \
         (exact text replacement), 'insert' (insert after line N), 'delete' (remove lines). \
         Use 'path' for the file and 'command' to select the operation. \
         IMPORTANT: The 'str_replace' command REQUIRES both 'old_str' and 'new_str' parameters. \
         The 'old_str' parameter must contain the exact text to find and replace. \
         Do NOT call str_replace without providing old_str — the call will fail."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Operation to perform",
                    "enum": ["view", "create", "str_replace", "insert", "delete"]
                },
                "path": {
                    "type": "string",
                    "description": "File path to operate on"
                },
                "file_text": {
                    "type": "string",
                    "description": "Full file content for 'create' command"
                },
                "old_str": {
                    "type": "string",
                    "description": "REQUIRED for 'str_replace' command. The exact text to find in the file. Must match exactly one location."
                },
                "new_str": {
                    "type": "string",
                    "description": "REQUIRED for 'str_replace' command. The replacement text. Also used as fallback for 'insert' command."
                },
                "insert_line": {
                    "type": "integer",
                    "description": "Line number after which to insert text (1-based) for 'insert'"
                },
                "new_str_insert": {
                    "type": "string",
                    "description": "Text to insert for 'insert' command"
                },
                "start_line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "First line to view/delete (1-based). Must not exceed the file's total line count."
                },
                "end_line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Last line to view/delete (1-based, inclusive). Must not exceed the file's total line count."
                }
            },
            "required": ["command", "path"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let command = input["command"]
            .as_str()
            .context("Missing required 'command' parameter")?;

        match command {
            "view" => handle_view(input, ctx).await,
            "create" => handle_create(input, ctx).await,
            "str_replace" => handle_str_replace(input, ctx).await,
            "insert" => handle_insert(input, ctx).await,
            "delete" => handle_delete(input, ctx).await,
            other => anyhow::bail!(
                "Unknown str_replace_editor command: '{other}'. \
                 Valid commands: view, create, str_replace, insert, delete"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

async fn handle_view(input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
    read::ReadTool.execute(input, ctx).await
}

async fn handle_create(mut input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
    // `file_text` → `content` for WriteTool / CreateTool
    if input.get("content").is_none() {
        if let Some(v) = input.get("file_text").cloned() {
            input["content"] = v;
        }
    }
    // If the file doesn't exist, use CreateTool; otherwise WriteTool
    let path_str = input["path"].as_str().context("Missing 'path'")?;
    let path = resolve_path(&ctx.working_dir, path_str);
    if path.exists() {
        write::WriteTool.execute(input, ctx).await
    } else {
        create::CreateTool.execute(input, ctx).await
    }
}

async fn handle_str_replace(input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
    // Validate old_str is present before delegating — models often omit it
    if input.get("old_str").and_then(|v| v.as_str()).is_none() {
        anyhow::bail!(
            "Missing required 'old_str' parameter for 'str_replace' command. \
             You must provide 'old_str' with the exact text to find in the file, \
             and 'new_str' with the replacement text. Both are required."
        );
    }
    // Delegate to EditTool (same parameter names)
    edit::EditTool.execute(input, ctx).await
}

async fn handle_insert(input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let path_str = input["path"].as_str().context("Missing 'path'")?;
    let insert_line = input["insert_line"]
        .as_u64()
        .context("Missing required 'insert_line' parameter for 'insert' command")?
        as usize;
    let new_text = input["new_str_insert"]
        .as_str()
        .or_else(|| input["new_str"].as_str())
        .context("Missing 'new_str_insert' (or 'new_str') for 'insert' command")?;

    let path = resolve_path(&ctx.working_dir, path_str);
    super::check_path_within_root(&path, &ctx.working_dir)?;

    let original = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("Cannot read file: {}", path.display()))?;

    let mut lines: Vec<&str> = original.lines().collect();
    let insert_at = insert_line.min(lines.len());

    // Build the new lines to insert (may contain embedded newlines)
    let new_lines: Vec<&str> = new_text.lines().collect();
    for (i, line) in new_lines.into_iter().enumerate() {
        lines.insert(insert_at + i, line);
    }

    let new_content = lines.join("\n");
    // Preserve trailing newline
    let new_content = if original.ends_with('\n') {
        format!("{new_content}\n")
    } else {
        new_content
    };

    tokio::fs::write(&path, &new_content)
        .await
        .with_context(|| format!("Failed to write file: {}", path.display()))?;

    let inserted_count = new_text.lines().count();
    Ok(ToolOutput {
        content: String::new(), // Empty on success; errors are returned as Err
        metadata: Some(json!({
            "command": "insert",
            "path": path.display().to_string(),
            "insert_after_line": insert_line,
            "old_lines": 0,
            "new_lines": inserted_count,
        })),
    })
}

async fn handle_delete(input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let path_str = input["path"].as_str().context("Missing 'path'")?;
    let start = input["start_line"]
        .as_u64()
        .context("Missing 'start_line' for 'delete' command")? as usize;
    let end = input["end_line"]
        .as_u64()
        .context("Missing 'end_line' for 'delete' command")? as usize;

    if start == 0 || end < start {
        anyhow::bail!("'start_line' must be ≥ 1 and ≤ 'end_line'");
    }

    let path = resolve_path(&ctx.working_dir, path_str);
    super::check_path_within_root(&path, &ctx.working_dir)?;

    let original = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("Cannot read file: {}", path.display()))?;

    let lines: Vec<&str> = original.lines().collect();
    let total = lines.len();

    if start > total {
        anyhow::bail!("start_line ({start}) exceeds file length ({total})");
    }

    let end_clamped = end.min(total);
    let removed = end_clamped - start + 1;

    let mut new_lines: Vec<&str> = Vec::with_capacity(total.saturating_sub(removed));
    new_lines.extend_from_slice(&lines[..start - 1]);
    if end_clamped < total {
        new_lines.extend_from_slice(&lines[end_clamped..]);
    }

    let new_content = new_lines.join("\n");
    let new_content = if original.ends_with('\n') {
        format!("{new_content}\n")
    } else {
        new_content
    };

    tokio::fs::write(&path, &new_content)
        .await
        .with_context(|| format!("Failed to write file: {}", path.display()))?;

    Ok(ToolOutput {
        content: String::new(), // Empty on success; errors are returned as Err
        metadata: Some(json!({
            "command": "delete",
            "path": path.display().to_string(),
            "start_line": start,
            "end_line": end_clamped,
            "old_lines": removed,
            "new_lines": 0,
            "lines_removed": removed,
        })),
    })
}

fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}

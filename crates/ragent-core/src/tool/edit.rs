//! Surgical text replacement tool for file editing.
//!
//! Provides [`EditTool`], which replaces exactly one occurrence of a search
//! string with a replacement string in a file, ensuring precise edits.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

/// Replaces an exact, unique occurrence of `old_str` with `new_str` in a file.
///
/// The search string must match exactly once; zero or multiple matches are
/// treated as errors to prevent ambiguous edits.
pub struct EditTool;

#[async_trait::async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Replace an exact occurrence of old_str with new_str in a file. \
         The old_str must match exactly one location in the file."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "old_str": {
                    "type": "string",
                    "description": "Exact string to find and replace"
                },
                "new_str": {
                    "type": "string",
                    "description": "Replacement string"
                }
            },
            "required": ["path", "old_str", "new_str"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"].as_str().context("Missing required 'path' parameter")?;
        let old_str = input["old_str"]
            .as_str()
            .context("Missing required 'old_str' parameter")?;
        let new_str = input["new_str"]
            .as_str()
            .context("Missing required 'new_str' parameter")?;

        let path = resolve_path(&ctx.working_dir, path_str);

        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Cannot read file '{}': file may not exist or is not accessible", path.display()))?;

        let count = content.matches(old_str).count();
        if count == 0 {
            bail!(
                "old_str not found in {}. Make sure it matches exactly.",
                path.display()
            );
        }
        if count > 1 {
            bail!(
                "old_str found {} times in {}. It must match exactly once. Add more context to make it unique.",
                count,
                path.display()
            );
        }

        let new_content = content.replacen(old_str, new_str, 1);
        tokio::fs::write(&path, &new_content)
            .await
            .with_context(|| format!("Failed to write file: {}", path.display()))?;

        // Show a small diff summary
        let old_lines = old_str.lines().count();
        let new_lines = new_str.lines().count();
        let lines_changed = old_lines.max(new_lines);

        Ok(ToolOutput {
            content: format!(
                "Edited {}: replaced {} line{} with {} line{}",
                path.display(),
                old_lines,
                if old_lines == 1 { "" } else { "s" },
                new_lines,
                if new_lines == 1 { "" } else { "s" },
            ),
            metadata: Some(json!({
                "path": path.display().to_string(),
                "old_lines": old_lines,
                "new_lines": new_lines,
                "lines": lines_changed,
            })),
        })
    }
}

fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}

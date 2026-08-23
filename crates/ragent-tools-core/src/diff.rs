//! File diff tool.
//!
//! Provides [`DiffFilesTool`], which computes a unified diff between two files
//! (or two inline text strings) using the [`similar`] crate — the same library
//! used by Git, ripgrep, and many other Rust tools.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use similar::{ChangeTag, TextDiff};

use super::{Tool, ToolContext, ToolOutput};
use crate::path_util::resolve_path;

/// Show a unified diff between two files or text strings.
pub struct DiffFilesTool;

#[async_trait::async_trait]
impl Tool for DiffFilesTool {
    fn name(&self) -> &'static str {
        "diff_files"
    }

    fn description(&self) -> &'static str {
        "Show a unified diff between two files or two inline text strings. \
         Provide either `path_a` + `path_b` (file paths) or `text_a` + `text_b` \
         (inline strings); do not mix files and text for the same side. \
         Optional: `context_lines` (integer, default 3) controls how many \
         unchanged lines surround each change. Returns unified-diff style \
         output with `---` / `+++` headers and `@@` hunk markers."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path_a": { "type": "string", "description": "Path to the first file (left / old)" },
                "path_b": { "type": "string", "description": "Path to the second file (right / new)" },
                "text_a": { "type": "string", "description": "First text string (left / old), alternative to path_a" },
                "text_b": { "type": "string", "description": "Second text string (right / new), alternative to path_b" },
                "context_lines": {
                    "type": "integer",
                    "description": "Number of context lines around changes (default: 3)"
                }
            },
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let context_lines = input["context_lines"].as_u64().unwrap_or(3) as usize;

        // Resolve left side
        let (label_a, text_a) = if let Some(p) = input["path_a"].as_str() {
            let path = resolve_path(&ctx.working_dir, p);
            super::check_path_within_root_cached(&path, &ctx.working_dir, &ctx.canonical_cache)?;
            let content = tokio::fs::read_to_string(&path)
                .await
                .with_context(|| format!("Cannot read file: {}", path.display()))?;
            (p.to_string(), content)
        } else if let Some(t) = input["text_a"].as_str() {
            ("a".to_string(), t.to_string())
        } else {
            anyhow::bail!("Provide 'path_a' or 'text_a'");
        };

        // Resolve right side
        let (label_b, text_b) = if let Some(p) = input["path_b"].as_str() {
            let path = resolve_path(&ctx.working_dir, p);
            super::check_path_within_root_cached(&path, &ctx.working_dir, &ctx.canonical_cache)?;
            let content = tokio::fs::read_to_string(&path)
                .await
                .with_context(|| format!("Cannot read file: {}", path.display()))?;
            (p.to_string(), content)
        } else if let Some(t) = input["text_b"].as_str() {
            ("b".to_string(), t.to_string())
        } else {
            anyhow::bail!("Provide 'path_b' or 'text_b'");
        };

        let diff = TextDiff::from_lines(&text_a, &text_b);

        // Build unified diff output
        let mut output = format!("--- {label_a}\n+++ {label_b}\n");
        let mut changes = 0usize;

        for group in diff.grouped_ops(context_lines) {
            // Compute hunk header
            let old_start = group.first().map_or(0, |o| o.old_range().start);
            let new_start = group.first().map_or(0, |o| o.new_range().start);
            let old_len: usize = group.iter().map(|o| o.old_range().len()).sum();
            let new_len: usize = group.iter().map(|o| o.new_range().len()).sum();
            output.push_str(&format!(
                "@@ -{},{} +{},{} @@\n",
                old_start + 1,
                old_len,
                new_start + 1,
                new_len
            ));

            for op in &group {
                for change in diff.iter_changes(op) {
                    let prefix = match change.tag() {
                        ChangeTag::Delete => {
                            changes += 1;
                            '-'
                        }
                        ChangeTag::Insert => {
                            changes += 1;
                            '+'
                        }
                        ChangeTag::Equal => ' ',
                    };
                    output.push(prefix);
                    output.push_str(change.value());
                    if !change.value().ends_with('\n') {
                        output.push('\n');
                    }
                }
            }
        }

        if changes == 0 {
            return Ok(ToolOutput {
                content: "Files are identical (no differences)".to_string(),
                metadata: Some(json!({ "changes": 0 })),
            });
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({ "changes": changes })),
        })
    }
}

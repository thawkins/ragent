//! Directory listing tool with tree-style output.
//!
//! Provides [`ListTool`], which displays directory contents in a tree format
//! with configurable recursion depth, file sizes, and sorted entries.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

/// Lists directory contents in a tree-like format with file sizes.
///
/// Directories are listed before files, hidden entries are excluded, and
/// common generated directories are skipped. Depth defaults to 2 levels.
pub struct ListTool;

#[async_trait::async_trait]
impl Tool for ListTool {
    fn name(&self) -> &str {
        "list"
    }

    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &str {
        "List directory contents with tree-like output. Supports depth control."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path to list (default: working directory)"
                },
                "depth": {
                    "type": "integer",
                    "description": "Maximum depth to recurse (default: 2)"
                }
            }
        })
    }

    fn permission_category(&self) -> &str {
        "file:read"
    }

    /// Lists directory contents in a tree format.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The specified path is not a directory or does not exist
    /// - The directory cannot be read due to permission issues
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let dir = input["path"]
            .as_str()
            .map(|p| resolve_path(&ctx.working_dir, p))
            .unwrap_or_else(|| ctx.working_dir.clone());

        let max_depth = input["depth"].as_u64().unwrap_or(2) as usize;

        if !dir.is_dir() {
            anyhow::bail!(
                "Path '{}' is not a directory or does not exist",
                dir.display()
            );
        }

        let mut lines = Vec::new();
        lines.push(format!("{}/", dir.display()));
        list_recursive(&dir, "", 0, max_depth, &mut lines)?;

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: None,
        })
    }
}

/// Recursively lists directory contents with tree formatting.
///
/// # Errors
///
/// Returns an error if a directory cannot be read due to permission issues.
fn list_recursive(
    dir: &Path,
    prefix: &str,
    depth: usize,
    max_depth: usize,
    lines: &mut Vec<String>,
) -> Result<()> {
    if depth >= max_depth {
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("Cannot read directory: {}", dir.display()))?
        .filter_map(|e| e.ok())
        .collect();

    // Sort entries: directories first, then by name
    entries.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    // Filter hidden files
    entries.retain(|e| e.file_name().to_str().is_none_or(|n| !n.starts_with('.')));

    let count = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let path = entry.path();

        if path.is_dir() {
            // Skip large/generated directories
            if matches!(
                name_str.as_ref(),
                "node_modules" | "target" | ".git" | "__pycache__" | "dist" | "build"
            ) {
                lines.push(format!("{}{}{}/  (skipped)", prefix, connector, name_str));
                continue;
            }
            lines.push(format!("{}{}{}/", prefix, connector, name_str));
            let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
            list_recursive(&path, &new_prefix, depth + 1, max_depth, lines)?;
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            lines.push(format!(
                "{}{}{}  ({})",
                prefix,
                connector,
                name_str,
                format_size(size)
            ));
        }
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Resolves a path string to an absolute `PathBuf` relative to the working directory.
fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}

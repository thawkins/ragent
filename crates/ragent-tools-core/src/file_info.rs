//! File metadata / info tool.
//!
//! Provides [`FileInfoTool`], which returns metadata for a file or directory:
//! size (bytes), last-modified timestamp (UTC ISO-8601), file type
//! (file / directory / symlink), and Unix permissions.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::time::UNIX_EPOCH;

use super::{Tool, ToolContext, ToolOutput};
use crate::path_util::resolve_path;

/// Return metadata for a file or directory.
pub struct FileInfoTool;

#[async_trait::async_trait]
impl Tool for FileInfoTool {
    fn name(&self) -> &'static str {
        "file_info"
    }

    fn description(&self) -> &'static str {
        "Return metadata for a file or directory. Required parameter: `path` \
         (string). Reports size in bytes, last-modified time (UTC), file type \
         (file/directory/symlink), whether the path exists, and filesystem \
         permissions. If the path does not exist, the result clearly says so. \
         Use this before expensive operations to confirm a target's state."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "REQUIRED. Path to the file or directory" }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;

        let path = resolve_path(&ctx.working_dir, path_str);
        super::check_path_within_root_cached(&path, &ctx.working_dir, &ctx.canonical_cache)?;

        if !path.exists() {
            return Ok(ToolOutput {
                content: format!("Path does not exist: {}", path.display()),
                metadata: Some(json!({ "exists": false, "path": path.display().to_string() })),
            });
        }

        let meta = tokio::fs::symlink_metadata(&path)
            .await
            .with_context(|| format!("Failed to read metadata for: {}", path.display()))?;

        let file_type = if meta.is_symlink() {
            "symlink"
        } else if meta.is_dir() {
            "directory"
        } else {
            "file"
        };

        let size = meta.len();

        let mtime_secs = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());

        // Format as a simple ISO-8601-like UTC timestamp
        let mtime_str = format_unix_secs(mtime_secs);

        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::PermissionsExt as _;
            format!("{:o}", meta.permissions().mode() & 0o7777)
        };
        #[cfg(not(unix))]
        let permissions = if meta.permissions().readonly() {
            "readonly"
        } else {
            "read-write"
        }
        .to_string();

        let content = format!(
            "Path:        {}\nType:        {}\nSize:        {} bytes\nModified:    {}\nPermissions: {}",
            path.display(),
            file_type,
            size,
            mtime_str,
            permissions
        );

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "path":        path.display().to_string(),
                "exists":      true,
                "type":        file_type,
                "size":        size,
                "modified":    mtime_str,
                "permissions": permissions,
            })),
        })
    }
}

/// Format a Unix timestamp (seconds since epoch) as `YYYY-MM-DD HH:MM:SS UTC`.
fn format_unix_secs(secs: u64) -> String {
    DateTime::<Utc>::from_timestamp(secs as i64, 0)
        .map_or("1970-01-01 00:00:00 UTC".to_string(), |dt| {
            dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
        })
}

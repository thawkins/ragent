//! File metadata / info tool.
//!
//! Provides [`FileInfoTool`], which returns metadata for a file or directory:
//! size (bytes), last-modified timestamp (UTC ISO-8601), file type
//! (file / directory / symlink), and Unix permissions.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::{Tool, ToolContext, ToolOutput};

/// Return metadata for a file or directory.
pub struct FileInfoTool;

#[async_trait::async_trait]
impl Tool for FileInfoTool {
    fn name(&self) -> &'static str {
        "file_info"
    }

    fn description(&self) -> &'static str {
        "Return metadata for a file or directory: size in bytes, last-modified \
         time (UTC), file type (file/directory/symlink), and whether it exists."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file or directory" }
            },
            "required": ["path"]
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
        super::check_path_within_root(&path, &ctx.working_dir)?;

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
            .map(|d| d.as_secs())
            .unwrap_or(0);

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

fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}

/// Format a Unix timestamp (seconds since epoch) as `YYYY-MM-DD HH:MM:SS UTC`.
fn format_unix_secs(secs: u64) -> String {
    // Simple manual formatter — avoids pulling in a date library.
    let s = secs;
    let sec = s % 60;
    let min = (s / 60) % 60;
    let hr = (s / 3600) % 24;
    let days = s / 86400;

    // Days since 1970-01-01 → calendar date
    let (y, m, d) = days_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02} {hr:02}:{min:02}:{sec:02} UTC")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let leap = is_leap(year);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0usize;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, (month + 1) as u64, days + 1)
}

fn is_leap(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

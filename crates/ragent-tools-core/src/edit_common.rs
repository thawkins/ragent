//! Shared helpers for edit-style tools.
//!
//! [`check_stale_file`] and [`record_edit_timestamp`] are used by both the
//! single-file `edit` tool and the batch `multi_edit` tool. Keeping them in
//! one place avoids copy-paste drift and guarantees both tools enforce the
//! same stale-file baseline semantics.

use anyhow::{Result, bail};
use std::path::Path;
use std::time::SystemTime;

use super::ToolContext;

/// Reject the edit if `path` was modified after the session last read it.
///
/// Returns `Ok(())` when no read timestamp is recorded (no baseline is
/// available) or when the on-disk mtime is within 1 ms of the recorded
/// baseline. A 1 ms tolerance avoids spurious rejections caused by filesystem
/// mtime granularity when a read and edit happen in the same tick.
pub(crate) fn check_stale_file(path: &Path, ctx: &ToolContext) -> Result<()> {
    let recorded = ctx
        .read_timestamps
        .read()
        .ok()
        .and_then(|map| map.get(path).copied());

    let Some(recorded_millis) = recorded else {
        return Ok(());
    };

    let current_millis = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|mtime| {
            mtime
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64)
        });

    let Some(current_millis) = current_millis else {
        return Ok(());
    };

    if current_millis > recorded_millis.saturating_add(1) {
        bail!(
            "File '{}' was modified after it was last read by this session \
             (read mtime {}ms, current mtime {}ms). Re-read the file before \
             editing to avoid clobbering external changes.",
            path.display(),
            recorded_millis,
            current_millis
        );
    }

    Ok(())
}

/// Record the on-disk mtime of `path` as the session's read baseline.
///
/// Call this after a successful write so a follow-up edit in the same session
/// does not trip the stale-file check on a file we just modified.
pub(crate) fn record_edit_timestamp(path: &Path, ctx: &ToolContext) {
    if let Ok(meta) = std::fs::metadata(path)
        && let Ok(mtime) = meta.modified()
    {
        let millis = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        if let Ok(mut map) = ctx.read_timestamps.write() {
            map.insert(path.to_path_buf(), millis);
        }
    }
}

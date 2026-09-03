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
// reason: only consumed inside this crate (edit.rs / multiedit.rs) and by the
// #[path]-re-included copy in tests/test_edit.rs, where the re-included module
// is itself private - `pub` here never escapes the crate.
#[allow(unreachable_pub)]
// reason: used by the edit and multiedit tools in the lib; flagged dead only
// when edit_common.rs is re-included into the test crate via #[path], where
// the tool-execution path is not exercised.
#[allow(dead_code)]
pub fn check_stale_file(path: &Path, ctx: &ToolContext) -> Result<()> {
    let recorded = ctx
        .read_timestamps
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(path)
        .copied();

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
        // reason: no on-disk mtime available - nothing to compare against
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
pub fn record_edit_timestamp(path: &Path, ctx: &ToolContext) {
    if let Ok(meta) = std::fs::metadata(path)
        && let Ok(mtime) = meta.modified()
    {
        let millis = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let mut map = ctx
            .read_timestamps
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.insert(path.to_path_buf(), millis);
    }
}

use super::{CommitResult, EditStaging};
use anyhow::Result;
use std::path::PathBuf;

/// High-level API: apply a batch of (path, new_content) pairs using the EditStaging flow.
/// This is the entry point used by tools and skills to perform concurrent edits.
///
/// # Errors
///
/// Returns an error if:
/// - File I/O errors occur when reading original files during staging
/// - Errors from `staging.stage_edit()` (file read failures)
/// - Errors from `staging.commit_all()` (write failures, conflict detection, join errors)
pub async fn apply_batch_edits<I>(
    pairs: I,
    concurrency: usize,
    dry_run: bool,
) -> Result<CommitResult>
where
    I: IntoIterator<Item = (PathBuf, String)>,
{
    let mut staging = EditStaging::new(dry_run);
    for (p, content) in pairs {
        staging.stage_edit(p, content).await?;
    }
    let res = staging.commit_all(concurrency).await?;
    Ok(res)
}

use anyhow::Result;
use std::path::PathBuf;
use super::{EditStaging, CommitResult};

/// High-level API: apply a batch of (path, new_content) pairs using the EditStaging flow.
/// This is the entry point used by tools and skills to perform concurrent edits.
pub async fn apply_batch_edits<I>(pairs: I, concurrency: usize, dry_run: bool) -> Result<CommitResult>
where
    I: IntoIterator<Item = (PathBuf, String)>,
{
    let mut staging = EditStaging::new(dry_run);
    for (p, content) in pairs.into_iter() {
        staging.stage_edit(p, content).await?;
    }
    let res = staging.commit_all(concurrency).await?;
    Ok(res)
}

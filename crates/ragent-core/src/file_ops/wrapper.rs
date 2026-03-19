use crate::file_ops::{CommitResult, apply_batch_edits};
use anyhow::Result;
use std::path::PathBuf;

/// Convenience wrapper used by higher-level skills (like /simplify) to apply a
/// collection of edits concurrently. Keeps the skill logic simple and delegates
/// staging/commit semantics to the core file_ops module.
///
/// # Errors
///
/// Returns an error if:
/// - File I/O errors occur during staging or commit
/// - Any errors propagated from `apply_batch_edits`
pub async fn apply_edits_from_pairs<I>(
    pairs: I,
    concurrency: usize,
    dry_run: bool,
) -> Result<CommitResult>
where
    I: IntoIterator<Item = (PathBuf, String)>,
{
    apply_batch_edits(pairs, concurrency, dry_run).await
}

//! Concurrent and batched file operations (M1 prototype API)
//!
//! This module contains the API surface and lightweight prototypes prepared as
//! the Milestone M1 deliverable for FASTFILE F17. The implementations are
//! intentionally small, focusing on design and wiring; the full feature set will
//! be implemented in M2.

use anyhow::{Result, Context};
use std::path::{PathBuf, Path};
use tokio::sync::Semaphore;
use std::sync::Arc;
use sha2::{Sha256, Digest};
use tokio::io::AsyncWriteExt;
use tokio::fs::OpenOptions;
use uuid::Uuid;
use std::fs as stdfs;

/// Result of reading a single file.
pub struct FileReadResult {
    pub path: PathBuf,
    pub content: Option<String>,
    pub err: Option<anyhow::Error>,
    pub lines: Option<usize>,
}

/// Concurrent file reader configuration and entry points.
#[derive(Clone)]
pub struct ConcurrentFileReader {
    concurrency: usize,
}

impl ConcurrentFileReader {
    /// Create a new reader with default concurrency (number of CPU threads).
    pub fn new() -> Self {
        let n = num_cpus::get();
        Self { concurrency: n }
    }

    /// Set concurrency limit.
    pub fn with_concurrency(mut self, n: usize) -> Self {
        self.concurrency = n.max(1);
        self
    }

    /// Read multiple paths concurrently and return a vector of results in the
    /// same iteration order as the input.
    pub async fn read_paths<I>(&self, paths: I) -> Result<Vec<FileReadResult>>
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let sem = Arc::new(Semaphore::new(self.concurrency));
        let mut handles: Vec<tokio::task::JoinHandle<anyhow::Result<FileReadResult>>> = Vec::new();

        for p in paths.into_iter() {
            let sem = sem.clone();
            let permit = sem.acquire_owned().await.unwrap();
            let path = p.clone();
            let h = tokio::spawn(async move {
                // permit dropped at the end of scope
                let _permit = permit;
                match tokio::fs::read_to_string(&path).await {
                    Ok(s) => Ok(FileReadResult { path: path.clone(), content: Some(s.clone()), err: None, lines: Some(s.lines().count()) }),
                    Err(e) => Ok(FileReadResult { path: path.clone(), content: None, err: Some(anyhow::Error::new(e)), lines: None }),
                }
            });
            handles.push(h);
        }

        let mut results = Vec::new();
        for h in handles {
            match h.await {
                Ok(Ok(r)) => results.push(r),
                Ok(Err(e)) => return Err(e).context("task failed"),
                Err(e) => return Err(anyhow::Error::new(e)).context("join error"),
            }
        }

        Ok(results)
    }
}

/// A single staged edit record (prototype).
pub struct StagedEdit {
    pub path: PathBuf,
    pub original_checksum: String,
    pub proposed_content: String,
}

pub struct CommitResult {
    pub applied: Vec<PathBuf>,
    pub conflicts: Vec<(PathBuf, String)>,
    pub errors: Vec<(PathBuf, anyhow::Error)>,
}

/// Alias for the result produced by each commit task.
type CommitTaskResult = anyhow::Result<(PathBuf, Option<(PathBuf, String)>, Option<(PathBuf, anyhow::Error)>)>;

/// In-memory staging area for edits.
pub struct EditStaging {
    edits: Vec<StagedEdit>,
    dry_run: bool,
}

impl EditStaging {
    /// Create a new staging area.
    pub fn new(dry_run: bool) -> Self {
        Self { edits: Vec::new(), dry_run }
    }

    /// Stage an edit for the given path. This reads the current content and
    /// stores a checksum and proposed content.
    pub async fn stage_edit<P: AsRef<Path>>(&mut self, path: P, new_content: String) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        let orig = tokio::fs::read_to_string(&path).await.context("reading original file")?;
        let checksum = format!("sha256:{:x}", Sha256::digest(orig.as_bytes()));
        self.edits.push(StagedEdit { path, original_checksum: checksum, proposed_content: new_content });
        Ok(())
    }

    /// Basic validation (no-op prototype).
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }

    /// Commit all staged edits concurrently (prototype: simple write per file).
    pub async fn commit_all(&self, concurrency_limit: usize) -> Result<CommitResult> {
        if self.dry_run {
            tracing::info!("dry_run: not writing files");
            return Ok(CommitResult { applied: self.edits.iter().map(|e| e.path.clone()).collect(), conflicts: Vec::new(), errors: Vec::new() });
        }

        let sem = Arc::new(Semaphore::new(concurrency_limit));
        let mut handles: Vec<tokio::task::JoinHandle<CommitTaskResult>> = Vec::new();

        for e in &self.edits {
            let sem = sem.clone();
            let permit = sem.acquire_owned().await.unwrap();
            let path = e.path.clone();
            let content = e.proposed_content.clone();
            let original_checksum = e.original_checksum.clone();
            let h = tokio::spawn(async move {
                let _permit = permit;
                // simple atomic write: write to temp then rename
                let tmp = path.with_extension(format!("ragent_tmp_{}", Uuid::new_v4()));
                // write backup in case we need rollback
                let backup = path.with_extension("ragent_backup");

                // Conflict detection: re-read current file and compare checksum
                if path.exists() {
                    match tokio::fs::read_to_string(&path).await {
                        Ok(cur) => {
                            let cur_sum = format!("sha256:{:x}", Sha256::digest(cur.as_bytes()));
                            if cur_sum != original_checksum {
                                return Ok((path.clone(), Some((path.clone(), "checksum mismatch: file changed since staging".to_string())), None));
                            }
                            // write backup
                            tokio::fs::write(&backup, cur.as_bytes()).await?;
                        }
                        Err(e) => {
                            return Ok((path.clone(), None, Some((path.clone(), anyhow::Error::new(e)))));
                        }
                    }
                }

                if let Err(e) = tokio::fs::write(&tmp, content.clone()).await {
                    let p1 = path.clone();
                    let p2 = p1.clone();
                    return Ok((p1.clone(), None, Some((p2.clone(), anyhow::Error::new(e)))));
                }
                if let Err(e) = tokio::fs::rename(&tmp, &path).await {
                    let p1 = path.clone();
                    let p2 = p1.clone();
                    return Ok((p1.clone(), None, Some((p2.clone(), anyhow::Error::new(e)))));
                }

                // successful write
                Ok((path.clone(), None, None))
            });
            handles.push(h);
        }

        let mut applied = Vec::new();
        let mut conflicts = Vec::new();
        let mut errors = Vec::new();
        for h in handles {
            match h.await {
                Ok(Ok((p, conflict, err))) => {
                    if let Some(c) = conflict {
                        conflicts.push(c);
                    } else if let Some(e) = err {
                        errors.push(e);
                    } else {
                        applied.push(p);
                    }
                }
                Ok(Err(e)) => return Err(e).context("commit task failed"),
                Err(e) => return Err(anyhow::Error::new(e)).context("join error"),
            }
        }

        // If there were conflicts or errors, attempt rollback for files that were applied.
        if !conflicts.is_empty() || !errors.is_empty() {
            tracing::warn!("conflicts_or_errors_detected: attempting rollback for applied files");
            let mut rollback_errors: Vec<(PathBuf, anyhow::Error)> = Vec::new();
            for p in &applied {
                let backup = p.with_extension("ragent_backup");
                if tokio::fs::metadata(&backup).await.is_ok() {
                    if let Err(e) = tokio::fs::rename(&backup, &p).await {
                        rollback_errors.push((p.clone(), anyhow::Error::new(e)));
                    }
                } else {
                    // No backup exists; best-effort remove the file we wrote.
                    if let Err(e) = tokio::fs::remove_file(&p).await {
                        rollback_errors.push((p.clone(), anyhow::Error::new(e)));
                    }
                }
            }

            // Append rollback errors to errors list
            for r in rollback_errors.into_iter() {
                errors.push(r);
            }

            // After rollback, clear applied to indicate nothing was successfully applied
            applied.clear();
        }

        Ok(CommitResult { applied, conflicts, errors })
    }
}

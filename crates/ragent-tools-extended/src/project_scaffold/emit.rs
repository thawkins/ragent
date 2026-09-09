//! No-silent-overwrite file emission (FR-016).
//!
//! Thin I/O layer that turns the pure plans (T-003 layout, T-005 workspace)
//! into real files under the scaffold root:
//!
//! - existing files are **never** overwritten: they are reported as skipped
//!   and left byte-for-byte untouched (FR-016);
//! - parent directories are created as needed; the FR-004 workspace
//!   directories are ensured idempotently;
//! - every emitted path is recorded in an [`EmitReport`] that the FR-011
//!   summary milestone renders.
//!
//! I/O errors fail fast with the offending path attached
//! ([`ScaffoldError::EmissionFailed`]); earlier creations remain for the
//! partial-completion summary.

use std::fs;
use std::path::Path;

use super::flags::ScaffoldError;
use super::layout::PlannedFile;
use super::workspace::{WorkspaceArtifact, workspace_dirs};

/// Outcome of emitting a single planned file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// The file did not exist and was written.
    Created,
    /// The file already existed; content left untouched (FR-016).
    SkippedExisting,
}

/// Aggregated emission report (feeds the FR-011 summary).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmitReport {
    /// Paths written by this emission, in emission order.
    pub created: Vec<String>,
    /// Paths skipped because they already existed (FR-016).
    pub skipped_existing: Vec<String>,
}

impl EmitReport {
    /// Record one file outcome.
    pub fn record(&mut self, status: FileStatus, path: &str) {
        match status {
            FileStatus::Created => self.created.push(path.to_owned()),
            FileStatus::SkippedExisting => self.skipped_existing.push(path.to_owned()),
        }
    }

    /// Absorb another report, preserving per-list order.
    pub fn merge(&mut self, other: Self) {
        self.created.extend(other.created);
        self.skipped_existing.extend(other.skipped_existing);
    }
}

/// Anything the emitter can write: a planned layout file or a workspace
/// artifact.
pub trait PlannedArtifact {
    /// Path relative to the scaffold root (forward slashes, no leading `/`).
    fn artifact_path(&self) -> &str;
    /// Full file content (UTF-8).
    fn artifact_content(&self) -> &str;
}

impl PlannedArtifact for PlannedFile {
    fn artifact_path(&self) -> &str {
        &self.path
    }

    fn artifact_content(&self) -> &str {
        &self.content
    }
}

impl PlannedArtifact for WorkspaceArtifact {
    fn artifact_path(&self) -> &str {
        self.path
    }

    fn artifact_content(&self) -> &str {
        &self.content
    }
}

/// Create the FR-004 workspace directories under `root`, idempotently.
///
/// # Errors
///
/// [`ScaffoldError::EmissionFailed`] when a directory cannot be created.
pub fn ensure_workspace_dirs(root: &Path) -> Result<(), ScaffoldError> {
    for dir in workspace_dirs() {
        fs::create_dir_all(root.join(dir)).map_err(|err| ScaffoldError::EmissionFailed {
            path: (*dir).to_owned(),
            source: err.to_string(),
        })?;
    }
    Ok(())
}

/// Emit one file under `root` with the FR-016 policy.
///
/// Existing paths (any entry type) are left untouched and reported as
/// [`FileStatus::SkippedExisting`]; missing files are written after creating
/// their parent directories.
///
/// # Errors
///
/// [`ScaffoldError::EmissionFailed`] when parent-directory creation or the
/// write itself fails.
pub fn emit_file(root: &Path, path: &str, content: &str) -> Result<FileStatus, ScaffoldError> {
    let target = root.join(path);
    if target.exists() {
        return Ok(FileStatus::SkippedExisting);
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| ScaffoldError::EmissionFailed {
            path: path.to_owned(),
            source: err.to_string(),
        })?;
    }
    fs::write(&target, content).map_err(|err| ScaffoldError::EmissionFailed {
        path: path.to_owned(),
        source: err.to_string(),
    })?;
    Ok(FileStatus::Created)
}

/// Emit a batch of planned artifacts, aggregating the report.
///
/// # Errors
///
/// [`ScaffoldError::EmissionFailed`] on the first failing file; earlier
/// creations remain (the FR-011 summary reports partial completion).
pub fn emit_files<T: PlannedArtifact>(
    root: &Path,
    files: &[T],
) -> Result<EmitReport, ScaffoldError> {
    let mut report = EmitReport::default();
    for file in files {
        let status = emit_file(root, file.artifact_path(), file.artifact_content())?;
        report.record(status, file.artifact_path());
    }
    Ok(report)
}

//! Scaffold summary report (FR-011).
//!
//! Pure formatting — no I/O. Aggregates what happened during a scaffold run
//! into the text the `/new` foreground flow prints when scaffolding completes
//! fully or partially:
//!
//! - the created layout and generated files (from the T-007 emitter report),
//! - the chosen language / type / stack,
//! - the local git outcome (from the T-008 gitinit report), and
//! - the remote status (none / remote URL / failed).

use std::path::PathBuf;

use super::emit::EmitReport;
use super::flags::{AppType, Language};
use super::gitinit::{GitFailure, GitInitReport};

/// Remote-init status in the summary (FR-011: none / remote URL / failed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteStatus {
    /// No hosting flag supplied: nothing configured, nothing pushed (FR-008).
    None,
    /// Hosting repository created and pushed to this URL (T-009/T-010).
    Created {
        /// Remote URL (the `origin` the push targeted).
        url: String,
    },
    /// A remote step failed (FR-010): local scaffold intact, `step` names the
    /// failed operation and `message` carries remediation context.
    Failed {
        /// Remote step that failed (e.g. `repo create`, `push`).
        step: String,
        /// Failure diagnostics / remediation hint.
        message: String,
    },
}

/// Aggregated, printable scaffold summary (FR-011).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldSummary {
    /// Project slug.
    pub project_name: String,
    /// Scaffold root directory.
    pub target: PathBuf,
    /// Chosen language.
    pub language: Language,
    /// Chosen app type.
    pub app_type: AppType,
    /// Chosen stack overlay, when supplied (FR-007).
    pub stack: Option<String>,
    /// Emission report: created + skipped-existing files (FR-016).
    pub emit: EmitReport,
    /// Local git outcome, when the git step ran (`Err` = contained failure).
    pub git: Option<Result<GitInitReport, GitFailure>>,
    /// Remote status (FR-011).
    pub remote: RemoteStatus,
    /// Set when the scaffold stopped before completing (partial completion).
    pub stopped_early: Option<String>,
}

impl ScaffoldSummary {
    /// Build a summary from the T-007/T-008 reports (plumbing constructor).
    ///
    /// `remote` starts as [`RemoteStatus::None`] and `stopped_early` as
    /// `None`; the orchestrator overwrites them when a hosting flag was
    /// supplied or a step failed.
    pub fn from_reports(
        project_name: impl Into<String>,
        target: PathBuf,
        language: Language,
        app_type: AppType,
        stack: Option<String>,
        emit: EmitReport,
        git: Option<Result<GitInitReport, GitFailure>>,
    ) -> Self {
        Self {
            project_name: project_name.into(),
            target,
            language,
            app_type,
            stack,
            emit,
            git,
            remote: RemoteStatus::None,
            stopped_early: None,
        }
    }

    /// Render the FR-011 summary block (one line per fact, trailing newline).
    pub fn render(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        match &self.stack {
            Some(stack) => lines.push(format!(
                "Scaffolded {} ({}, {}, stack: {})",
                self.project_name, self.language, self.app_type, stack
            )),
            None => lines.push(format!(
                "Scaffolded {} ({}, {})",
                self.project_name, self.language, self.app_type
            )),
        }
        lines.push(format!("Target: {}", self.target.display()));
        lines.push(format!("Created {} file(s):", self.emit.created.len()));
        lines.extend(self.emit.created.iter().map(|path| format!("  + {path}")));
        if !self.emit.skipped_existing.is_empty() {
            lines.push(format!(
                "Skipped existing (left untouched): {}",
                self.emit.skipped_existing.join(", ")
            ));
        }
        match &self.git {
            Some(Ok(report)) => {
                let repo = if report.repo_created {
                    "initialised new repository"
                } else {
                    "reused existing repository"
                };
                let commit = if report.committed {
                    "initial commit created"
                } else {
                    "nothing to commit (clean tree)"
                };
                lines.push(format!("Git: {repo}, {commit}"));
            }
            Some(Err(failure)) => lines.push(format!("Git: {failure}")),
            None => lines.push("Git: not run".to_owned()),
        }
        match &self.remote {
            RemoteStatus::None => lines.push("Remote: none".to_owned()),
            RemoteStatus::Created { url } => lines.push(format!("Remote: {url}")),
            RemoteStatus::Failed { step, message } => {
                lines.push(format!("Remote: failed at {step}: {message}"));
            }
        }
        if let Some(reason) = &self.stopped_early {
            lines.push(format!("Stopped early: {reason}"));
        }
        let mut out = lines.join("\n");
        out.push('\n');
        out
    }
}

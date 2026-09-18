//! Scaffold-reuse stage of a `/spec govcreate` run (spec `govdoc` T-005,
//! FR-010, FR-011).
//!
//! A govcreate run must produce the same new project as `/new`, with no second
//! scaffold path (FR-010). This module therefore *delegates* every scaffold
//! concern to the shared engine in [`crate::project_scaffold`] rather than
//! reimplementing it:
//!
//! - the target folder is created when it does not exist,
//! - the FR-011 empty-directory guard ([`enforce_empty_directory_guard`]) runs
//!   before any other action and reports the blocking entries,
//! - [`plan_and_emit`] writes the language layout, workspace artifacts, and
//!   docs,
//! - [`init_and_commit`] performs the local git half, and the hosting helpers
//!   ([`init_github_remote`] / [`init_gitlab_remote`]) the optional remote half.
//!
//! The stage is I/O only: no LLM turn, no acquisition, no spec authoring. It
//! stops after the scaffold, so the govcreate orchestration (T-012) can insert
//! the acquisition -> extract -> author stages between this stage and the
//! terminal report without changing this code.

use std::path::{Path, PathBuf};

use chrono::SecondsFormat;

use crate::project_scaffold::{
    HostingTarget, RemoteStatus, ScaffoldError, ScaffoldRequest, ScaffoldSummary,
    enforce_empty_directory_guard, init_and_commit, init_github_remote, init_gitlab_remote,
    plan_and_emit, recipe_for,
};

/// Why a [`run_govcreate_scaffold`] call did not complete.
///
/// The guard and emission variants wrap the shared engine's [`ScaffoldError`];
/// the remaining variants cover the target-folder creation and internal-recipe
/// lookup a govcreate run performs around the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovCreateScaffoldError {
    /// The target folder could not be created (FR-011 pre-step).
    TargetFolder {
        /// The folder that could not be created.
        path: PathBuf,
        /// Underlying OS error message.
        detail: String,
    },
    /// The FR-011 empty-directory guard refused a non-empty target.
    Guard(ScaffoldError),
    /// The engine failed while emitting the scaffold (FR-010).
    Emit(ScaffoldError),
    /// No scaffold recipe exists for the selected language (internal error).
    NoRecipe {
        /// The canonical language value that had no recipe.
        language: String,
    },
}

impl std::fmt::Display for GovCreateScaffoldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TargetFolder { path, detail } => {
                write!(
                    f,
                    "cannot create target folder {}: {detail}",
                    path.display()
                )
            }
            Self::Guard(err) | Self::Emit(err) => write!(f, "{err}"),
            Self::NoRecipe { language } => {
                write!(f, "internal: no scaffold recipe for language '{language}'")
            }
        }
    }
}

impl std::error::Error for GovCreateScaffoldError {}

/// Outcome of a successful scaffold stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldStageOutcome {
    /// Scaffolded project name (the target folder's file name, or a fallback).
    pub slug: String,
    /// The FR-011 summary with the git and remote outcomes attached.
    pub summary: ScaffoldSummary,
    /// Stack warning note from the engine (empty when the stack was known or
    /// absent; a `[warn]` line for an unknown stack).
    pub stack_note: String,
}

/// Resolve the slug used to name the scaffolded project: the target folder's
/// last path component, falling back to `new-project` for a path with no final
/// component.
fn scaffold_slug(target: &Path) -> String {
    target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "new-project".to_owned())
}

/// Run the scaffold stage for a validated request (FR-010, FR-011).
///
/// Creates the target folder when missing, runs the FR-011 empty-directory
/// guard before any other action, then delegates to the shared engine for
/// emission, local git init, and (when a hosting flag was supplied) the GitHub
/// or GitLab remote-init half.
pub fn run_govcreate_scaffold(
    request: &ScaffoldRequest,
    target: &Path,
) -> Result<ScaffoldStageOutcome, GovCreateScaffoldError> {
    std::fs::create_dir_all(target).map_err(|err| GovCreateScaffoldError::TargetFolder {
        path: target.to_path_buf(),
        detail: err.to_string(),
    })?;

    // FR-011: refuse a non-empty target before any other action. The guard is
    // evaluated against the real target directory, exactly as `/new` does.
    enforce_empty_directory_guard(target).map_err(GovCreateScaffoldError::Guard)?;

    let slug = scaffold_slug(target);
    let recipe =
        recipe_for(request.language()).ok_or_else(|| GovCreateScaffoldError::NoRecipe {
            language: request.language().as_str().to_owned(),
        })?;

    let generated_at_utc = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let (mut summary, stack_note) =
        plan_and_emit(target, request, &slug, recipe, &generated_at_utc)
            .map_err(GovCreateScaffoldError::Emit)?;

    // FR-008 local half: git init + initial commit, then the hosting remote
    // half when a flag was supplied. The engine contains failures in the
    // summary rather than rolling the scaffold back.
    summary.git = Some(init_and_commit(target, "Initial scaffold"));
    summary.remote = init_hosting(request, target, &slug);

    Ok(ScaffoldStageOutcome {
        slug,
        summary,
        stack_note,
    })
}

/// Perform the optional hosting remote-init half (FR-010: the same helpers as
/// `/new`).
///
/// No hosting flag yields [`RemoteStatus::None`]; a hosting flag runs the
/// matching engine helper, mapping a failure to [`RemoteStatus::Failed`] with
/// the failing step named - the local scaffold is never rolled back.
fn init_hosting(request: &ScaffoldRequest, target: &Path, slug: &str) -> RemoteStatus {
    match request.hosting() {
        None => RemoteStatus::None,
        Some(HostingTarget::GitHub) => match init_github_remote(target, slug, true) {
            Ok(report) => RemoteStatus::Created { url: report.url },
            Err(failure) => RemoteStatus::Failed {
                step: failure.step.as_str().to_owned(),
                message: failure.message,
            },
        },
        Some(HostingTarget::GitLab) => match init_gitlab_remote(target, slug, true) {
            Ok(report) => RemoteStatus::Created { url: report.url },
            Err(failure) => RemoteStatus::Failed {
                step: failure.step.as_str().to_owned(),
                message: failure.message,
            },
        },
    }
}

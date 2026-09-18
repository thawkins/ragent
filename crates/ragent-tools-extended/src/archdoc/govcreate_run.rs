//! Govcreate orchestration runner for `/spec govcreate` (spec `govdoc` T-012;
//! FR-006, FR-011, FR-012, FR-013, FR-014, FR-019).
//!
//! The runner chains the stages in the order the spec fixes - **guard,
//! scaffold, acquire, extract, author, write, report** (FR-012) - and owns
//! the failure-containment and cancellation rules:
//!
//! - **FR-006**: an unreadable/empty content reference aborts the run *before*
//!   any scaffold write, with the specific cause from
//!   [`classify_content_ref`]/[`validate_local_path`] (T-003), or as
//!   `NoUsableContent` after an acquisition that gathered zero usable sources.
//! - **FR-011**: the shared empty-directory guard refuses a non-empty target
//!   before anything else; the report names the blocking entries and no
//!   extraction, generation, or writing follows.
//! - **FR-013**: when a stage fails *after* the scaffold completed, the
//!   report preserves the scaffolded project, names the completed stages and
//!   the failed stage with its cause, and states no spec was generated.
//! - **FR-014**: a scaffold failure aborts the run before acquisition and
//!   authoring, reports the scaffold error, and writes no partial spec.
//! - **FR-019**: a cancelled run stops at the next stage boundary, reports
//!   the completed stages, and never writes spec files after the cancel
//!   point.
//!
//! # Purity (NFR-002)
//!
//! Every effect except the scaffold itself is injected through
//! [`GovCreateStages`]; the scaffold is the T-005
//! [`run_govcreate_scaffold`] call kept concrete because the T-005 suite pins
//! its real filesystem behaviour. With a mocked [`GovCreateStages`], the
//! runner is deterministic and unit-testable.

use std::path::PathBuf;

use crate::archdoc::content_ref::{
    ContentRef, ContentRefError, classify_content_ref, validate_local_path,
};
use crate::archdoc::extract::ArchitectureStructure;
use crate::archdoc::url_source::GatheredCorpus;
use crate::project_scaffold::ScaffoldRequest;

use super::runner::{GovCreateScaffoldError, ScaffoldStageOutcome, run_govcreate_scaffold};

// ---------------------------------------------------------------------------
// Run record + outcome
// ---------------------------------------------------------------------------

/// One validated `/spec govcreate` invocation, handed to the runner.
#[derive(Debug, Clone)]
pub struct GovCreateRun {
    /// Spec identifier (already validated by the T-002 parser).
    pub spec_id: String,
    /// Raw content reference as the user typed it (T-003 re-validates).
    pub content_ref: String,
    /// Target folder (resolved against the TUI working dir by the caller).
    pub target_folder: PathBuf,
    /// Validated `/new` scaffold request (FR-010 flags).
    pub scaffold: ScaffoldRequest,
    /// `--force` (FR-017): overwrite an existing spec directory.
    pub force: bool,
    /// Invoking tree root used by the T-003 local-path safety check.
    pub invoking_root: PathBuf,
}

/// One stage the runner completed, in order (FR-013).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedStage {
    /// Stage identifier (`"guard"`, `"scaffold"`, `"acquire"`, `"extract"`,
    /// `"author"`, `"write"`).
    pub id: &'static str,
    /// One-line human-readable note (acquired-source counts, fallback path,
    /// etc.); empty when not applicable.
    pub note: String,
}

/// Terminal state of a govcreate run (FR-012/FR-013/FR-014/FR-019).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovCreateOutcome {
    /// Full success: scaffold applied and spec files written.
    Completed {
        /// The `<target>/specs/<specid>/` directory that received the files.
        spec_dir: PathBuf,
        /// Populated when acquisition exhausted a budget (FR-016 reporting).
        budget_note: Option<String>,
    },
    /// FR-006: the content reference was unreadable, unreadable-valid, or
    /// produced zero usable text. No scaffold write, no spec.
    NoUsableContent {
        /// The specific cause (T-003 error Display, or an acquisition
        /// budget/format exclusion note).
        cause: String,
    },
    /// FR-011: the target folder was not empty; nothing was written.
    NonEmptyTarget {
        /// Blocking entry names the guard reported.
        blocking: Vec<String>,
    },
    /// FR-014: the scaffold stage failed; no acquisition, no authoring, no
    /// partial spec.
    ScaffoldFailed {
        /// The scaffold error.
        cause: String,
    },
    /// FR-013: a stage after the scaffold failed; the scaffold persists and
    /// the report states no spec was generated.
    StageFailed {
        /// The failing stage id.
        stage: &'static str,
        /// The stage's error message.
        cause: String,
    },
    /// FR-019: the run was cancelled at a stage boundary.
    Cancelled {
        /// The stage that was about to run when cancellation landed.
        stopped_before: &'static str,
    },
}

/// Final report: sequence + terminal outcome (FR-013).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovCreateRunReport {
    /// Completed stages in the order they ran.
    pub completed: Vec<CompletedStage>,
    /// Terminal outcome.
    pub outcome: GovCreateOutcome,
}

/// One staged-progress event emitted by the runner (FR-015).
///
/// Events land in strict stage order - `Started` for a stage always precedes
/// that stage's `StageCompleted`, and any `Terminal` event is the last one.
/// The TUI streams these into a single in-place-updated message rather than
/// stacking one message per stage (T-013); the CLI parity path renders the
/// report alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovCreateProgress {
    /// A stage is about to run.
    Started {
        /// Stage identifier (same set as [`CompletedStage::id`]).
        id: &'static str,
    },
    /// A stage finished successfully; carries the FR-013 completed entry.
    StageCompleted(CompletedStage),
    /// The run finished with the terminal outcome (success, failure, or
    /// FR-019 cancellation).
    Terminal(GovCreateOutcome),
}

impl GovCreateProgress {
    /// Render the event as one NFR-005 (ASCII `[ .. ]`/`[ ok ]`) progress
    /// line for the streamed panel.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Started { id } => format!("[ .. ] {id}"),
            Self::StageCompleted(stage) => {
                if stage.note.is_empty() {
                    format!("[ ok ] {}", stage.id)
                } else {
                    format!("[ ok ] {} - {}", stage.id, stage.note)
                }
            }
            Self::Terminal(outcome) => match outcome {
                GovCreateOutcome::Completed { .. } => "[ ok ] complete".to_owned(),
                GovCreateOutcome::Cancelled { stopped_before } => {
                    format!("[note] cancelled before the {stopped_before} stage")
                }
                GovCreateOutcome::NoUsableContent { .. } => {
                    "[fail] content reference unusable".to_owned()
                }
                GovCreateOutcome::NonEmptyTarget { .. } => {
                    "[fail] target folder is not empty".to_owned()
                }
                GovCreateOutcome::ScaffoldFailed { .. } => "[fail] scaffold".to_owned(),
                GovCreateOutcome::StageFailed { stage, .. } => format!("[fail] {stage}"),
            },
        }
    }
}

/// Human label for each stage id, used by the FR-015 streaming display.
#[must_use]
pub fn stage_label(id: &'static str) -> &'static str {
    match id {
        "guard" => "validating content reference",
        "scaffold" => "scaffolding target project",
        "acquire" => "acquiring referenced content",
        "extract" => "extracting architecture structure",
        "author" => "authoring spec files",
        "write" => "writing spec files",
        other => other,
    }
}

impl GovCreateRunReport {
    /// `true` when the run wrote the spec (the success path).
    #[must_use]
    pub fn succeeded(&self) -> bool {
        matches!(self.outcome, GovCreateOutcome::Completed { .. })
    }

    /// Render the report as a compact NFR-005 message body (used by both the
    /// TUI dispatch arm and the CLI parity path).
    #[must_use]
    pub fn render(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for stage in &self.completed {
            if stage.note.is_empty() {
                lines.push(format!("[ ok ] {}", stage.id));
            } else {
                lines.push(format!("[ ok ] {} - {}", stage.id, stage.note));
            }
        }
        match &self.outcome {
            GovCreateOutcome::Completed {
                spec_dir,
                budget_note,
            } => {
                lines.push(format!("[ ok ] spec written to {}", spec_dir.display()));
                if let Some(note) = budget_note {
                    lines.push(format!("[note] acquisition budget reached: {note}"));
                }
            }
            GovCreateOutcome::NoUsableContent { cause } => {
                lines.push(format!("[err] {cause}"));
                lines.push("[note] no scaffold was applied; no spec was generated".to_owned());
            }
            GovCreateOutcome::NonEmptyTarget { blocking } => {
                lines.push("[err] target folder is not empty".to_owned());
                for entry in blocking {
                    lines.push(format!("      blocking entry: {entry}"));
                }
                lines.push("[note] nothing was extracted, generated, or written".to_owned());
            }
            GovCreateOutcome::ScaffoldFailed { cause } => {
                lines.push(format!("[err] scaffold failed: {cause}"));
                lines.push("[note] no acquisition, no authoring, no spec was written".to_owned());
            }
            GovCreateOutcome::StageFailed { stage, cause } => {
                lines.push(format!("[fail] {stage}: {cause}"));
                lines.push(
                    "[note] the scaffolded project remains; no spec was generated".to_owned(),
                );
            }
            GovCreateOutcome::Cancelled { stopped_before } => {
                lines.push(format!(
                    "[note] cancelled before the {stopped_before} stage"
                ));
                lines.push("[note] completed stages are listed above; no spec was written after cancellation".to_owned());
            }
        }
        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Stages trait (effects the TUI/CLI injects)
// ---------------------------------------------------------------------------

/// The three spec artifacts the authoring stage returns (FR-008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredSpec {
    /// `SPEC.md` body (FR-018 frontmatter + EARS requirements).
    pub spec_md: String,
    /// `PLAN.md` body (task table).
    pub plan_md: String,
    /// `TESTPLAN.md` body (manual cases).
    pub testplan_md: String,
}

/// Error channel from an injected stage back to the runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovCreateRunError {
    /// Acquisition failed beyond retry (URL unreachable and crawl gave up,
    /// local root unreadable mid-walk). The empty-corpus refusal is *not*
    /// signalled this way - an empty `GatheredCorpus` is a successful
    /// acquisition the runner converts to `NoUsableContent`.
    Acquire(String),
    /// Extraction failed (model error with no usable fallback).
    Extract(String),
    /// Authoring failed (model error producing the three spec files).
    Author(String),
    /// The spec-write stage failed (including the FR-017 refusal).
    Write(String),
}

impl std::fmt::Display for GovCreateRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Acquire(m) => write!(f, "acquire: {m}"),
            Self::Extract(m) => write!(f, "extract: {m}"),
            Self::Author(m) => write!(f, "author: {m}"),
            Self::Write(m) => write!(f, "write: {m}"),
        }
    }
}

impl std::error::Error for GovCreateRunError {}

/// The tool effects the runner depends on. Production code (the TUI dispatch
/// arm) supplies an implementation that calls `acquire_url` / `acquire_local`,
/// drives the LLM extraction and authoring turns, and calls
/// `SpecCommand::write_govcreate_spec`; tests supply deterministic fakes.
///
/// [`Send`]/[`Sync`] bounds travel with the trait so the TUI can dispatch a
/// run onto a tokio worker without wrapping every stage future; the runner's
/// own future is consequently `Send` (clippy::future_not_send is a CI gate).
/// When that is not wanted (the CLI parity path runs on a current-thread
/// runtime and can hold a non-`Send` implementation), the crate provides the
/// private blanket wrapper [`LocalStages`].
#[allow(async_fn_in_trait)]
pub trait GovCreateStages: Send + Sync {
    /// Acquire the content reference into a normalised corpus.
    ///
    /// Called after the scaffold stage - the runner guarantees the FR-011
    /// guard passed first (FR-012).
    async fn acquire(&self, reference: &ContentRef) -> Result<GatheredCorpus, GovCreateRunError>;

    /// Extract the architecture structure from a non-empty corpus (FR-007).
    async fn extract(
        &self,
        corpus: &GatheredCorpus,
    ) -> Result<ArchitectureStructure, GovCreateRunError>;

    /// Author the three spec artifacts from the extracted structure (FR-008).
    async fn author(
        &self,
        structure: &ArchitectureStructure,
        run: &GovCreateRun,
    ) -> Result<AuthoredSpec, GovCreateRunError>;

    /// Persist the three artifacts to `<target>/specs/<spec_id>/` (FR-009,
    /// FR-017).
    async fn write_spec(
        &self,
        run: &GovCreateRun,
        authored: &AuthoredSpec,
    ) -> Result<(), GovCreateRunError>;
}

// ---------------------------------------------------------------------------
// Cancellation (FR-019)
// ---------------------------------------------------------------------------

/// Cooperative stage-boundary cancellation token (FR-019).
///
/// The TUI's Escape/keyboard path sets the flag from a worker thread; the
/// runner reads it before starting each stage. When set, the runner stops
/// *at the boundary* - mid-stage work is never interrupted and no spec is
/// written after the cancel point.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl CancellationToken {
    /// A token that is never cancelled - the default for CLI/non-interactive
    /// runs.
    pub fn none() -> Self {
        Self::default()
    }

    /// Set the cancellation flag.
    pub fn cancel(&self) {
        self.flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Read the cancellation flag.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(std::sync::atomic::Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// The runner
// ---------------------------------------------------------------------------

/// Drive one validated govcreate invocation: guard -> scaffold -> acquire ->
/// extract -> author -> write -> report (FR-012).
///
/// Enforces the pre-scaffold validation (FR-006), the empty-target refusal
/// (FR-011), failure containment (FR-013), the scaffold-failure rule
/// (FR-014), and cooperative cancellation (FR-019). Never panics: every
/// error is captured in the report.
pub async fn run_govcreate<S: GovCreateStages>(
    run: &GovCreateRun,
    stages: &S,
    cancel: &CancellationToken,
) -> GovCreateRunReport {
    let mut sink = NullProgress;
    run_govcreate_with_progress(run, stages, cancel, &mut sink).await
}

/// FR-015 progress sink: anything the runner can push a [`GovCreateProgress`]
/// event into. Implemented for every `FnMut(GovCreateProgress)` closure and
/// for the internal [`NullProgress`] no-op used by the bare
/// [`run_govcreate`] driver.
pub trait ProgressSink {
    /// Receive one staged event. Called synchronously from the runner, so
    /// implementations must be cheap and non-blocking (channel send, mutex
    /// push) to keep the FR-012 stage-boundary semantics deterministic.
    fn emit(&mut self, event: GovCreateProgress);
}

impl<F: FnMut(GovCreateProgress)> ProgressSink for F {
    fn emit(&mut self, event: GovCreateProgress) {
        (self)(event);
    }
}

/// No-op progress sink used by [`run_govcreate`] so the shared implementation
/// always runs through the FR-015 emission path.
struct NullProgress;
impl ProgressSink for NullProgress {
    fn emit(&mut self, _event: GovCreateProgress) {}
}

/// Progress-emitting variant of [`run_govcreate`] (FR-015).
///
/// The `progress` sink receives a [`GovCreateProgress::Started`] event before
/// each stage begins, a [`GovCreateProgress::StageCompleted`] event after each
/// stage succeeds, and exactly one [`GovCreateProgress::Terminal`] event at
/// the end (success, failure, or FR-019 cancellation). The sink is invoked
/// synchronously from the runner, so a cheap non-blocking send (channel,
/// mutex push) keeps the stage boundary semantics deterministic.
pub async fn run_govcreate_with_progress<S: GovCreateStages, P: ProgressSink>(
    run: &GovCreateRun,
    stages: &S,
    cancel: &CancellationToken,
    progress: &mut P,
) -> GovCreateRunReport {
    let mut completed: Vec<CompletedStage> = Vec::new();

    macro_rules! push_done {
        ($stage:expr) => {
            let stage = $stage;
            completed.push(stage.clone());
            progress.emit(GovCreateProgress::StageCompleted(stage));
        };
    }

    // ---------------------------------------------------------------- guard
    //
    // FR-012: the content-reference classification and the T-003 local-path
    // safety check are the pre-scaffold validation. FR-013 report note: the
    // "guard" stage carries both this check and the empty-directory check
    // (which owns the scaffold step) so every listed stage is genuinely done.
    if cancel.is_cancelled() {
        return cancelled(completed, "guard", progress);
    }
    progress.emit(GovCreateProgress::Started { id: "guard" });
    let reference = match classify_and_validate(run) {
        Ok(r) => r,
        Err(outcome) => {
            progress.emit(GovCreateProgress::Terminal(outcome.clone()));
            return GovCreateRunReport { completed, outcome };
        }
    };
    push_done!(CompletedStage {
        id: "guard",
        note: "content reference validated".to_owned(),
    });

    // ------------------------------------------------------------- scaffold
    //
    // FR-012: acquisition must not begin before the target-directory guard
    // has passed - the T-005 helper runs the shared guard first, so a refusal
    // here means zero writes and zero acquisition. FR-011 maps the guard
    // variant to a named-blockers report; any other scaffold error maps to
    // FR-014 containment.
    if cancel.is_cancelled() {
        return cancelled(completed, "scaffold", progress);
    }
    progress.emit(GovCreateProgress::Started { id: "scaffold" });
    let outcome = match run_govcreate_scaffold(&run.scaffold, &run.target_folder) {
        Ok(o) => o,
        Err(GovCreateScaffoldError::Guard(err)) => {
            let outcome = GovCreateOutcome::NonEmptyTarget {
                blocking: vec![err.to_string()],
            };
            progress.emit(GovCreateProgress::Terminal(outcome.clone()));
            return GovCreateRunReport { completed, outcome };
        }
        Err(other) => {
            let outcome = GovCreateOutcome::ScaffoldFailed {
                cause: other.to_string(),
            };
            progress.emit(GovCreateProgress::Terminal(outcome.clone()));
            return GovCreateRunReport { completed, outcome };
        }
    };
    push_done!(scaffold_note(&outcome));

    // -------------------------------------------------------------- acquire
    if cancel.is_cancelled() {
        return cancelled(completed, "acquire", progress);
    }
    progress.emit(GovCreateProgress::Started { id: "acquire" });
    let corpus = match stages.acquire(&reference).await {
        Ok(c) => c,
        Err(GovCreateRunError::Acquire(cause)) => {
            return stage_failed(completed, "acquire", cause, progress);
        }
        Err(other) => {
            return stage_failed(completed, "acquire", other.to_string(), progress);
        }
    };
    push_done!(CompletedStage {
        id: "acquire",
        note: acquire_note(&corpus),
    });

    // FR-006 post-acquisition half: a successful acquisition that gathered no
    // usable text still terminates the run before extraction/spec authoring.
    if corpus.is_empty() {
        let outcome = GovCreateOutcome::NoUsableContent {
            cause: "empty corpus: no usable text gathered".to_owned(),
        };
        progress.emit(GovCreateProgress::Terminal(outcome.clone()));
        return GovCreateRunReport { completed, outcome };
    }

    // -------------------------------------------------------------- extract
    if cancel.is_cancelled() {
        return cancelled(completed, "extract", progress);
    }
    progress.emit(GovCreateProgress::Started { id: "extract" });
    let structure = match stages.extract(&corpus).await {
        Ok(s) => s,
        Err(GovCreateRunError::Extract(cause)) => {
            return stage_failed(completed, "extract", cause, progress);
        }
        Err(other) => {
            return stage_failed(completed, "extract", other.to_string(), progress);
        }
    };
    push_done!(CompletedStage {
        id: "extract",
        note: extract_note(&structure),
    });

    // --------------------------------------------------------------- author
    if cancel.is_cancelled() {
        return cancelled(completed, "author", progress);
    }
    progress.emit(GovCreateProgress::Started { id: "author" });
    let authored = match stages.author(&structure, run).await {
        Ok(a) => a,
        Err(GovCreateRunError::Author(cause)) => {
            return stage_failed(completed, "author", cause, progress);
        }
        Err(other) => {
            return stage_failed(completed, "author", other.to_string(), progress);
        }
    };
    push_done!(CompletedStage {
        id: "author",
        note: "generated SPEC.md, PLAN.md, TESTPLAN.md".to_owned(),
    });

    // ---------------------------------------------------------------- write
    if cancel.is_cancelled() {
        return cancelled(completed, "write", progress);
    }
    progress.emit(GovCreateProgress::Started { id: "write" });
    match stages.write_spec(run, &authored).await {
        Ok(()) => {}
        Err(GovCreateRunError::Write(cause)) => {
            return stage_failed(completed, "write", cause, progress);
        }
        Err(other) => {
            return stage_failed(completed, "write", other.to_string(), progress);
        }
    }
    push_done!(CompletedStage {
        id: "write",
        note: format!("spec written to specs/{}", run.spec_id),
    });

    let outcome = GovCreateOutcome::Completed {
        spec_dir: run.target_folder.join("specs").join(&run.spec_id),
        budget_note: corpus.budget_reached.map(|b| b.to_string()),
    };
    progress.emit(GovCreateProgress::Terminal(outcome.clone()));
    GovCreateRunReport { completed, outcome }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Classify and validate the content reference (T-003) - the FR-006
/// pre-scaffold abort path.
fn classify_and_validate(run: &GovCreateRun) -> Result<ContentRef, GovCreateOutcome> {
    match classify_content_ref(&run.content_ref) {
        Ok(ContentRef::Url(url)) => Ok(ContentRef::Url(url)),
        Ok(ContentRef::Local(path)) => validate_local_path(&path, &run.invoking_root)
            .map(ContentRef::Local)
            .map_err(no_usable_content),
        Err(err) => Err(no_usable_content(err)),
    }
}

/// Map a [`ContentRefError`] to the FR-006 report shape.
fn no_usable_content(err: ContentRefError) -> GovCreateOutcome {
    GovCreateOutcome::NoUsableContent {
        cause: err.to_string(),
    }
}

/// Record the scaffold stage with the slug the engine picked.
fn scaffold_note(outcome: &ScaffoldStageOutcome) -> CompletedStage {
    CompletedStage {
        id: "scaffold",
        note: format!("scaffolded '{}'", outcome.slug),
    }
}

/// One-line acquisition summary for the report.
fn acquire_note(corpus: &GatheredCorpus) -> String {
    let mut note = format!(
        "acquired {} source{} ({} chars)",
        corpus.sources.len(),
        if corpus.sources.len() == 1 { "" } else { "s" },
        corpus.stats.total_chars
    );
    if let Some(reason) = corpus.budget_reached {
        note.push_str(&format!("; budget reached: {reason}"));
    }
    if !corpus.excluded.is_empty() {
        note.push_str(&format!("; {} excluded", corpus.excluded.len()));
    }
    note
}

/// One-line extraction summary for the report.
fn extract_note(structure: &ArchitectureStructure) -> String {
    let path = if structure.from_fallback {
        "deterministic fallback"
    } else {
        "LLM extraction"
    };
    format!(
        "extracted {} component{} via {path}",
        structure.components.len(),
        if structure.components.len() == 1 {
            ""
        } else {
            "s"
        },
    )
}

/// FR-013 helper: report a post-scaffold stage failure.
fn stage_failed(
    completed: Vec<CompletedStage>,
    stage: &'static str,
    cause: String,
    progress: &mut dyn ProgressSink,
) -> GovCreateRunReport {
    let outcome = GovCreateOutcome::StageFailed { stage, cause };
    progress.emit(GovCreateProgress::Terminal(outcome.clone()));
    GovCreateRunReport { completed, outcome }
}

/// FR-019 helper: report a cancelled run at the named stage boundary.
fn cancelled(
    completed: Vec<CompletedStage>,
    stage: &'static str,
    progress: &mut dyn ProgressSink,
) -> GovCreateRunReport {
    let outcome = GovCreateOutcome::Cancelled {
        stopped_before: stage,
    };
    progress.emit(GovCreateProgress::Terminal(outcome.clone()));
    GovCreateRunReport { completed, outcome }
}

/// `true` when the spec already exists and `--force` was not supplied.
///
/// The runner itself does not use this (it delegates the decision to
/// `write_spec`), but the dispatch surface and the CLI parity path need the
/// pure predicate to refuse an existing spec *before* touching anything else.
#[must_use]
pub fn is_forced_overwrite(exists: bool, force: bool) -> bool {
    crate::archdoc::content_ref::is_forced_overwrite(exists, force)
}

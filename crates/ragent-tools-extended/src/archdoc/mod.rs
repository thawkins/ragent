//! Architecture-document content handling for `/spec govcreate` (spec `govdoc`).
//!
//! Module family layout:
//!
//! - [`content_ref`] - content-reference classification, local-path safety and
//!   readability validation, and the FR-017 overwrite predicate (T-003).
//! - [`runner`] - scaffold-reuse stage: create the target folder, run the
//!   FR-011 empty-directory guard, then delegate to the shared
//!   `project_scaffold` engine for emission, git init, and hosting (T-005,
//!   FR-010).
//! - [`govcreate_run`] - the orchestration runner: pre-scaffold validation,
//!   FR-012 stage ordering, FR-013/FR-014 failure containment, and FR-019
//!   stage-boundary cancellation over injected [`GovCreateStages`] (T-012).
//! - [`url_source`] - bounded URL acquisition through the shared masterfetch
//!   crawl engine, producing a [`GatheredCorpus`] (T-006, FR-004, FR-016).
//! - [`local_source`] - bounded local file/folder acquisition with
//!   supported-format filtering, producing the same [`GatheredCorpus`] shape
//!   (T-007, FR-005).
//! - [`extract`] - architecture-structure extraction: the LLM prompt builder,
//!   the [`ArchitectureStructure`] JSON contract, the response parser, and the
//!   deterministic fallback structure (T-008, FR-007).
//!
//! The authoring stage (T-009) lives in `ragent-specs`
//! (`SpecCommand::build_govcreate_prompt`,
//! `SpecCommand::build_architecture_structure_summary`, and
//! `SpecCommand::write_govcreate_spec`) and consumes
//! [`ArchitectureStructure`] plus the FR-018 invocation frontmatter.

pub mod author_split;
pub mod content_ref;
pub mod extract;
pub mod govcreate_run;
pub mod local_source;
pub mod runner;
pub mod url_source;

pub use author_split::split_authored_sections;
pub use content_ref::{
    ContentRef, ContentRefError, classify_content_ref, is_forced_overwrite, validate_local_path,
};
pub use extract::{
    ArchitectureStructure, Component, DataStore, ExternalDependency, Interface,
    MAX_PROMPT_CORPUS_CHARS, Relationship, build_arch_extraction_prompt, fallback_structure,
    parse_architecture_response,
};
pub use govcreate_run::{
    AuthoredSpec, CancellationToken, CompletedStage, GovCreateOutcome, GovCreateProgress,
    GovCreateRun, GovCreateRunError, GovCreateRunReport, GovCreateStages, ProgressSink,
    run_govcreate, run_govcreate_with_progress, stage_label,
};
pub use local_source::{
    DEFAULT_LOCAL_MAX_TOTAL_CHARS, DEFAULT_MAX_FILES, LocalAcquisitionBudget,
    LocalAcquisitionStats, LocalBudgetReason, LocalExclusionReason, LocalWalkEntry, acquire_local,
    is_supported_document, local_budget_exhausted,
};
pub use runner::{GovCreateScaffoldError, ScaffoldStageOutcome, run_govcreate_scaffold};
pub use url_source::{
    AcquisitionBudget, AcquisitionStats, BudgetReason, ExcludedSource, ExclusionReason,
    GatheredCorpus, GatheredSource, UrlAcquisitionError, acquire_url, acquire_url_with_fetcher,
    budget_exhausted,
};

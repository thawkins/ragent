//! Research system for ragent.
//!
//! This crate provides the data structures and lifecycle helpers for the
//! `/research` slash command and the `ragent research` CLI. It enforces the
//! requirements defined in `specs/researchsystem/SPEC.md`.
//!
//! ## Modules
//!
//! - [`research_name`] — the URL-safe `ResearchName` newtype with full FR-002
//!   validation (lowercase ASCII letters/digits/hyphens, starting with a
//!   letter, 3-64 chars) and FR-017 path-traversal rejection.
//! - [`status`] — the `ResearchStatus` enum (draft, in-progress, complete,
//!   archived) covering FR-013.
//! - [`source`] — the `Source` enum (Web/Local/Spec/Other) backing the
//!   References Index block in every `RESEARCH.md`.
//! - [`item`] — the `ResearchItem` struct that ties name, title, status,
//!   timestamps, and sources together for FR-005. Includes YAML frontmatter
//!   rendering and parsing.
//! - [`web_gatherer`] — the `WebGatherer` that orchestrates web discovery
//!   and capture for FR-006 and FR-007.
//! - [`local_gatherer`] — the `LocalGatherer` that orchestrates local
//!   cross-referencing and FR-019 `--sources-dir` scanning for FR-006,
//!   FR-008, and FR-009.
//! - [`plan_dep`] — the parser for `research: <name>` dependency lines in
//!   `specs/<id>/PLAN.md` for FR-015.
//!
//! ## Future modules
//!
//! Additional modules will be added as later milestones land:
//!
//! - `manager` — `ResearchManager` with create/list/show/delete/archive
//! - `session` — gathering orchestration engine
//! - `index` — `research/INDEX.md` derived cache
//!
//! ## Implemented modules (Milestone 1+)
//!
//! - [`io`] — atomic file I/O, supporting-file paths, References Index and
//!   `research/INDEX.md` rendering (T-006, T-012, T-013).

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod adaptive;
pub mod analysis;
pub mod cli;
pub mod diagram;
pub mod document;
pub mod engine;
pub mod io;
pub mod item;
pub mod local_gatherer;
pub mod manager;
pub mod plan_dep;
pub mod planner;
pub mod research_name;
pub mod run_config;
pub mod session;
pub mod source;
pub mod source_registry;
pub mod state;
pub mod status;
pub mod verify;
pub mod web_date;
pub mod web_gatherer;

pub use adaptive::{AdaptiveStopper, StopDecision};
pub use analysis::{
    AnalysisEngine, AnalysisOutcome, AnalysisResult, HeuristicSummarizer, LlmAnalysisEngine,
    NoopAnalysisEngine, SourceBody, SourceSummarizer, build_source_bodies, chunk_source_bodies,
    merge_chunk_results, summarize_source_bodies, total_body_chars,
};
pub use cli::{
    FsLocalTool, ResearchCliCommand, render_list_output, render_search_output,
    render_session_event_json, render_show_output,
};
pub use diagram::render_findings_diagram;
pub use document::{
    AssembledDocument, CrossReference, MAX_SOURCE_BODY_BYTES, REQUIRED_SECTIONS, ResearchDocument,
    assemble_document, fence_source_body, mark_complete, mark_in_progress, render_skeleton,
    render_supporting_file, truncate_body_to_bytes,
};
pub use engine::{
    Critic, CriticResult, EngineConfig, IterationResult, IterativeEngine, SimpleCritic,
};
pub use io::{IndexEntry, ResearchIo, ResearchIoError};
pub use item::{
    DERIVED_TITLE_MAX_CHARS, ResearchItem, ResearchItemError, derive_title, derive_title_full,
};
pub use local_gatherer::{
    DEFAULT_GLOBS, DEFAULT_LOCAL_CONCURRENCY, DEFAULT_MAX_LOCAL_SOURCES, GrepMatch,
    LocalGatherConfig, LocalGatherError, LocalGatherer, LocalTool, MAX_LOCAL_EXCERPT_LINES,
    build_local_excerpt, build_relevance_note, collect_matched_terms, derive_terms,
    local_body_path,
};
pub use manager::{
    IndexTimestamp, ResearchError, ResearchManager, SearchHit, SearchIndex, SearchIndexEntry,
    render_document_for, suggest_closest_from, union_with_existing,
};
pub use plan_dep::{
    ResearchDependency, ResearchDependencyError, parse_research_dependencies,
    parse_spec_frontmatter_research, research_dependency_names,
};
pub use planner::{HeuristicPlanner, LlmPlanner, Planner};
pub use research_name::{MAX_LEN, MIN_LEN, ResearchName, ResearchNameError, is_path_traversal};
pub use run_config::{Depth, OutputFormat};
pub use session::{
    NoopObserver, ResearchSession, RunOutcome, SessionConfig, SessionEvent, SessionObserver,
    SessionPhase, SynthesizeOutcome,
};
pub use source::{LocalSourceKind, Source};
pub use source_registry::{BuiltinSourceRegistry, ResearchSourceKind, SourceRegistry};
pub use state::{
    EvidenceGap, ResearchPlan, ResearchState, StateCounts, SubQuestion, SubQuestionStatus,
};
pub use status::ResearchStatus;
pub use verify::{KeywordVerifier, VerificationResult, Verifier};
pub use web_date::extract_published_at;
pub use web_gatherer::{
    DEFAULT_FETCH_CONCURRENCY, DEFAULT_FETCH_TIMEOUT, DEFAULT_MAX_WEB_RESULTS,
    DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD, DEFAULT_SEARCH_MAX_RETRIES,
    DEFAULT_SEARCH_RETRY_BASE_DELAY_MS, GatherEvent, GatherResult, HeuristicQueryDecomposer,
    LlmQueryDecomposer, QueryDecomposer, WebFetchTool, WebFetchedPage, WebGatherError, WebGatherer,
    WebSearchHit, WebSearchTool, WebSourceKind, classify_web_source,
};

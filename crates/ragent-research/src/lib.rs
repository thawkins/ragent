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
pub mod brief;
pub mod cite_checker;
pub mod clarify;
pub mod cli;
pub mod cluster;
pub mod comparison;
pub mod contradiction;
pub mod corpus_critic;
pub mod diagram;
pub mod digest;
pub mod document;
pub mod engine;
pub mod entities;
pub mod evaluation;
pub mod gather_log;
pub mod io;
pub mod item;
pub mod local_gatherer;
pub mod locus;
pub mod manager;
pub mod open_access;
pub mod page_summarizer;
pub mod patcher;
pub mod plan_dep;
pub mod planner;
mod polarity;
pub mod provider_stats;
pub mod readability;
pub mod reconcile;
pub mod research_name;
pub mod run_config;
pub mod run_manifest;
pub mod run_request;
pub mod scoreboard;
pub mod search_budget;
pub mod session;
pub mod source;
pub mod source_registry;
pub mod source_vault;
pub mod state;
pub mod status;
pub mod supervisor;
pub mod synthesis;
pub mod tier_router;
pub mod verify;
pub mod web_date;
pub mod web_gatherer;

pub use adaptive::{AdaptiveStopper, StopDecision};
pub use analysis::{
    AnalysisEngine, AnalysisOutcome, AnalysisResult, HeuristicSummarizer, LlmAnalysisEngine,
    NoopAnalysisEngine, SourceBody, SourceSummarizer, build_source_bodies, chunk_source_bodies,
    merge_chunk_results, summarize_source_bodies, total_body_chars,
};
pub use brief::generate_research_brief;
pub use cite_checker::{CitationCheckResult, check_citations};
pub use clarify::needs_clarification;
pub use cli::{
    ResearchCliCommand, render_list_output, render_list_output_json, render_search_output,
    render_search_output_json, render_session_event_json, render_show_output,
    render_show_output_json, session_event_json,
};
pub use cluster::{
    CONCEPT_EXTRACTION_PROMPT_TEMPLATE, ClusterPayload, DEFAULT_CONTEXT_WINDOW_TOKENS,
    WebSourceMeta, build_cluster_payload, build_cluster_payload_sync,
    build_concept_extraction_prompt, build_concepts_payload_from_bodies,
    concepts_section_for_research, estimate_max_payload_bytes, format_concepts_md,
    format_concepts_md_with_sources, load_web_source_metadata, resolve_context_window_tokens,
    write_concepts_md,
};
pub use contradiction::{
    ContradictionClaim, ContradictionConfig, ContradictionEdge, ContradictionGraph,
    PolarityDimension, build_contradiction_graph, build_contradiction_graph_with,
};
pub use corpus_critic::{
    CorpusCriticReport, GapFetchResult, build_corpus_critic, derive_gap_queries,
};
pub use diagram::render_findings_diagram;
pub use document::{
    AssembledDocument, CrossReference, MAX_SOURCE_BODY_BYTES, REQUIRED_SECTIONS, ResearchDocument,
    assemble_document, fence_source_body, mark_complete, mark_in_progress, render_corpa_skeleton,
    render_skeleton, render_supporting_file, truncate_body_to_bytes,
};
pub use engine::{
    Critic, CriticResult, EngineConfig, IterationResult, IterativeEngine, SimpleCritic,
};
pub use entities::{
    CompetitiveEntity, EntityExtractionResult, extract_comparison_criteria,
    extract_entities_for_competitive_analysis, infer_competitive_set,
};
pub use evaluation::{EvaluationScorecard, evaluate_report, render_scorecard};
pub use io::{IndexEntry, ResearchIo, ResearchIoError};
pub use item::{
    DERIVED_TITLE_MAX_CHARS, ResearchItem, ResearchItemError, derive_title, derive_title_files,
    derive_title_full,
};
pub use local_gatherer::{
    DEFAULT_GLOBS, DEFAULT_LOCAL_CONCURRENCY, DEFAULT_MAX_LOCAL_SOURCES, GrepMatch,
    LocalGatherConfig, LocalGatherError, LocalGatherer, LocalTool, MAX_LOCAL_EXCERPT_LINES,
    build_local_excerpt, build_relevance_note, collect_matched_terms, derive_terms,
    local_body_path,
};
pub use locus::{DepthInvestigation, DepthLevel, Locus, LocusSet, analyze_loci, investigate_depth};
pub use manager::{
    IndexTimestamp, ResearchError, ResearchManager, SearchHit, SearchIndex, SearchIndexEntry,
    render_document_for, suggest_closest_from, union_with_existing,
};
pub use open_access::{
    DEFAULT_OA_MIN_FULL_TEXT_CHARS, OpenAccessClient, OpenAccessError, RecoveredOpenAccess,
    RecoverySource, ReqwestOpenAccessClient, query_europepmc, query_unpaywall, recover_open_access,
};
pub use page_summarizer::{LlmPageSummarizer, PageSummarizer, PageSummary};
pub use patcher::{PatchResult, SurgicalPatch, build_surgical_patches};
pub use plan_dep::{
    ResearchDependency, ResearchDependencyError, parse_research_dependencies,
    parse_spec_frontmatter_research, research_dependency_names,
};
pub use planner::{HeuristicPlanner, LlmPlanner, Planner};
pub use provider_stats::ProviderCallStats;
pub use readability::{
    PolishChange, PolishResult, ReadabilityAudit, audit_readability, polish_analysis,
};
pub use reconcile::{
    CrossLocusReconcile, ReconcilePair, SourceTensions, TensionKind, TensionRecord,
    build_cross_locus_reconcile, build_source_tensions,
};
pub use research_name::{MAX_LEN, MIN_LEN, ResearchName, ResearchNameError, is_path_traversal};
pub use run_config::{Depth, OutputFormat, ResearchMode, Tier};
pub use run_manifest::{
    ResumeOutcome, RunManifest, RunManifestError, RunStep, StepEntry, StepStatus,
};
pub use run_request::{InvocationParseError, ResearchRunRequest, build_session_config};
pub use scoreboard::{GradeBand, METER_CELLS, render_meter_bar};
pub use search_budget::{SearchBudget, SharedQueryCache};
pub use session::{
    AnalysisConfig, AnalysisEvent, DEFAULT_WEB_PHASE_TIMEOUT_SECS, InputConfig, LocalConfig,
    NoopObserver, OutputConfig, ResearchSession, ResilienceConfig, RunEngineConfig, RunOutcome,
    SessionConfig, SessionEvent, SessionObserver, SessionPhase, SupervisorConfig, SynthesisEvent,
    SynthesizeOutcome, WebConfig,
};
pub use source::{LocalSourceKind, Source};
pub use source_registry::{BuiltinSourceRegistry, ResearchSourceKind, SourceRegistry};
pub use source_vault::{NewVaultSource, SourceVault, SourceVaultError, VaultSource};
pub use state::{
    EvidenceGap, ResearchPlan, ResearchState, StateCounts, SubQuestion, SubQuestionStatus,
};
pub use status::ResearchStatus;
pub use supervisor::{
    DEFAULT_MAX_CONCURRENT_RESEARCH_UNITS, IterativeResearcherNode, ResearcherAssignment,
    ResearcherNode, ResearcherStatus, SupervisorNode, SupervisorState,
};
pub use synthesis::{CriticReport, SynthesisAudit, build_synthesis_audit};
pub use tier_router::{TierRouter, TierRouterObserver, TierRouterToSessionObserver};
pub use verify::{KeywordVerifier, VerificationResult, Verifier};
pub use web_date::extract_published_at;
pub use web_gatherer::{
    DEFAULT_FETCH_CONCURRENCY, DEFAULT_FETCH_TIMEOUT, DEFAULT_MAX_WEB_RESULTS,
    DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD, DEFAULT_SEARCH_MAX_RETRIES,
    DEFAULT_SEARCH_RETRY_BASE_DELAY_MS, GatherEvent, GatherResult, HeuristicQueryDecomposer,
    LlmQueryDecomposer, QueryDecomposer, WebFetchTool, WebFetchedPage, WebGatherError, WebGatherer,
    WebSearchHit, WebSearchTool, WebSourceKind, classify_web_source,
};

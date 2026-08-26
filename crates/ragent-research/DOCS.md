# ragent-research

Structured research system for ragent — orchestrates web + local evidence
gathering, synthesis, critique, and `RESEARCH.md` document assembly. Provides
40 modules covering gathering, analysis, session orchestration, document
assembly, and CLI.

## Workspace Dependencies

- ragent-types
- ragent-config
- ragent-llm
- ragent-storage
- ragent-tools-extended

## External Dependencies

- async-trait, futures, tokio
- chrono, serde, serde_json, thiserror, anyhow
- url, uuid, blake3, rusqlite
- reqwest (OA recovery)
- similar, regex

Dev-dependencies: tempfile, criterion, proptest, ragent-specs.

## Public API (crate root re-exports)

### Core data types & validation

- **ResearchName** (struct) — URL-safe research name newtype; `MIN_LEN=3`, `MAX_LEN=64`, `is_path_traversal`.
- **ResearchNameError** (enum) — Name validation errors.
- **ResearchStatus** (enum) — Draft, InProgress, Complete, Archived.
- **Source** (enum) / **LocalSourceKind** (enum) — Evidence source types.
- **ResearchItem** (struct) / **ResearchItemError** (enum) — Central research item with YAML frontmatter.
- **derive_title** / **derive_title_files** / **derive_title_full** (fns) — Title derivation.
- **ResearchState** / **ResearchPlan** / **SubQuestion** / **SubQuestionStatus** / **EvidenceGap** / **StateCounts** (structs) — In-memory iterative state.

### I/O & persistence

- **ResearchIo** (struct) / **ResearchIoError** (enum) / **IndexEntry** (struct) — Atomic file I/O.
- **ResearchManager** (struct) / **ResearchError** (enum) / **SearchHit** / **SearchIndex** / **SearchIndexEntry** / **IndexTimestamp** — High-level facade.
- **SourceVault** (struct) / **VaultSource** / **NewVaultSource** / **SourceVaultError** — Per-run persistent source store.
- **GatherLog** (struct) — Per-gather JSONL instrumentation log.

### Gathering

- **WebGatherer** (struct) / **GatherResult** / **WebSearchHit** / **WebFetchedPage** / **WebGatherError** / **GatherEvent** — Web gathering.
- **WebSearchTool** / **WebFetchTool** (traits) — Tool abstractions for gatherer injection.
- **WebSourceKind** (enum) / **classify_web_source** (fn) — Source classification.
- **QueryDecomposer** (trait) / **HeuristicQueryDecomposer** / **LlmQueryDecomposer** — Topic decomposition.
- **LocalGatherer** (struct) / **LocalTool** (trait) / **LocalGatherConfig** / **LocalGatherError** / **GrepMatch** — Local filesystem gathering.
- **OpenAccessClient** (trait) / **ReqwestOpenAccessClient** / **RecoveredOpenAccess** / **RecoverySource** / **OpenAccessError** — OA full-text recovery.

### Session orchestration

- **ResearchSession** (struct) / **SessionConfig** / **SessionEvent** / **SessionObserver** (trait) / **NoopObserver** / **SessionPhase** — Gathering orchestration.
- **AnalysisEvent** / **SynthesisEvent** / **SynthesizeOutcome** / **RunOutcome** — Event/result types.
- **ResearchRunRequest** (struct) / **build_session_config** (fn) — Front-end-agnostic run request.
- **IterativeEngine** (struct) / **EngineConfig** / **IterationResult** / **Critic** (trait) / **CriticResult** / **SimpleCritic** — Iterative research loop.
- **Planner** (trait) / **HeuristicPlanner** / **LlmPlanner** — Topic planning.
- **AdaptiveStopper** / **StopDecision** — Adaptive stopping.
- **TierRouter** / **TierRouterObserver** / **TierRouterToSessionObserver** — Tier routing.
- **OutputFormat** / **Tier** / **Depth** (enums) — Run config.
- **RunManifest** / **RunStep** / **StepStatus** / **StepEntry** / **RunManifestError** / **ResumeOutcome** — Resumable run manifest.

### Analysis pipeline (deterministic, LLM-free QA)

- **AnalysisEngine** (trait) / **LlmAnalysisEngine** / **NoopAnalysisEngine** / **AnalysisResult** / **AnalysisOutcome** — Source analysis.
- **ContradictionGraph** / **ContradictionEdge** / **ContradictionClaim** / **build_contradiction_graph** — Contradiction detection.
- **Locus** / **LocusSet** / **DepthLevel** / **DepthInvestigation** / **analyze_loci** / **investigate_depth** — Loci analysis.
- **CrossLocusReconcile** / **ReconcilePair** / **SourceTensions** / **TensionKind** / **TensionRecord** / **build_cross_locus_reconcile** / **build_source_tensions** — Reconciliation.
- **EvidenceDigest** / **DigestClaim** / **DraftCandidate** / **TripleDraft** — Evidence digest.
- **CorpusCriticReport** / **GapFetchResult** / **build_corpus_critic** / **derive_gap_queries** — Corpus quality.
- **SynthesisAudit** / **CriticReport** / **build_synthesis_audit** — 4-critic audit.
- **SurgicalPatch** / **PatchResult** / **build_surgical_patches** — Patch revision.
- **ReadabilityAudit** / **PolishResult** / **PolishChange** / **audit_readability** / **polish_analysis** — Readability.
- **CitationCheckResult** / **check_citations** — Citation verification.
- **Verifier** (trait) / **KeywordVerifier** / **VerificationResult** — Claim verification.
- **extract_published_at** (fn) — Publication date extraction.

### Document assembly & output

- **ResearchDocument** / **AssembledDocument** / **CrossReference** / **assemble_document** / **mark_complete** / **mark_in_progress** / **render_skeleton** / **render_supporting_file** — RESEARCH.md assembly.
- **FindingEdge** / **EdgeStrength** / **render_findings_diagram** — Mermaid diagram.
- **Chapter** / **ChapterPlan** / **partition_topic** — Dissertation chapter partitioning.

### Source registry & dependencies

- **SourceRegistry** (trait) / **BuiltinSourceRegistry** / **ResearchSourceKind** — Pluggable source registry.
- **ResearchDependency** / **ResearchDependencyError** / **parse_research_dependencies** / **parse_spec_frontmatter_research** / **research_dependency_names** — Plan dependencies.

### CLI

- **ResearchCliCommand** (enum) — Parsed `ragent research` subcommand.
- **FsLocalTool** (struct) — Filesystem-backed `LocalTool` for CLI.
- **render_list_output** / **render_show_output** / **render_search_output** / **render_session_event_json** / **session_event_json** / **explain_name_error** (fns) — CLI rendering.
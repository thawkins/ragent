---
status: draft
audit:
  - { time: 1788477188, from: "none", to: "draft", actor: "system" }
---
# SPEC: Open Deep Research feature-gap integration into ragent

## Context

LangChain's [Open Deep Research (ODR)](https://github.com/langchain-ai/open_deep_research) is an open-source deep-research agent that uses a multi-agent supervisor/researcher architecture to produce comprehensive reports. It scored competitively on the [Deep Research Bench leaderboard](https://huggingface.co/spaces/Ayanami0730/DeepResearch-Leaderboard). This specification identifies the feature gaps between ODR and ragent's existing `/research` tooling and defines the requirements to close those gaps inside ragent. The user is especially interested in competitive analysis capabilities.

## Goals

1. Add a clarification phase so ragent can resolve ambiguous research scope before expensive web searches begin.
2. Generate an explicit research brief from the user's prompt so downstream agents have a concrete, detailed mission.
3. Introduce a supervisor/researcher multi-agent research mode that can delegate independent sub-topics in parallel.
4. Add a dedicated competitive-analysis research mode that produces structured comparison tables and per-entity profiles.
5. Support per-page webpage summarization using a lightweight model so the final synthesis works with concise, salient source material.
6. Enable separate model selection for research, summarization, compression, and final-report writing.
7. Add a self-evaluation harness that scores produced reports on quality, relevance, groundedness, completeness, and structure.

## Non-goals

1. Port ODR's Python code verbatim or depend on LangGraph/LangChain.
2. Add a hosted LangGraph Studio-style UI.
3. Add new paid search APIs that require separate subscriptions beyond what ragent already supports.
4. Replace ragent's existing tiered research pipeline; this spec extends it.

## Requirements

### Ubiquitous requirements

**FR-001** While the `/research` command is available, the system SHALL accept a `--mode <mode>` option that selects the research execution strategy, including at least `tiered` (existing pipeline), `supervisor` (supervisor/researcher multi-agent), and `competitive` (dedicated competitive analysis).

**FR-002** While the `/research` command is available, the system SHALL accept a `--summarization-model` option that selects a lightweight model for summarizing fetched webpages independently from the synthesis model.

**FR-003** While a research run is in progress, the system SHALL persist every summarized source to the existing research vault so downstream agents and evaluation tools can reuse them.

**FR-004** While the final report is generated, the system SHALL cite sources using the existing ragent citation format and reference the original URLs, even when a summarized version was used for synthesis.

### Event-driven requirements

**FR-005** When the user invokes `/research` with an ambiguous scope, the system SHALL ask a single clarifying question before performing any web searches.

**FR-006** When the user invokes `/research --mode competitive`, the system SHALL decompose the topic into comparable entities, delegate one parallel researcher per entity, and synthesize a report containing per-entity profiles and a comparison table.

**FR-007** When a supervisor researcher delegates a sub-topic, the system SHALL run each sub-topic in parallel up to a configurable `max_concurrent_research_units` limit.

**FR-008** When the final report is written, the system SHALL optionally run a self-evaluation step and append a quality scorecard to the report frontmatter.

### State-driven requirements

**FR-009** If the user provides a `--mode supervisor` or `--mode competitive` option, the system SHALL use the supervisor/researcher graph instead of the tiered pipeline.

**FR-010** If the `--summarization-model` option is omitted, the system SHALL default to the configured default model and apply a short-output limit suitable for page summaries.

**FR-011** If the `--mode competitive` option is used and the topic does not explicitly name entities to compare, the system SHALL infer the competitive set from the topic before delegation.

### Optional requirements

**FR-012** The user MAY configure `research.supervisor.max_concurrent_research_units` in `ragent.json` to limit parallel researchers (default: 5).

**FR-013** The user MAY configure `research.models.research_model`, `research.models.compression_model`, and `research.models.final_report_model` in `ragent.json` to use distinct models for each phase.

**FR-014** The user MAY invoke `/research --mode competitive --format comparison-table` to request a shorter artifact that contains only the comparison table and entity profiles.

**FR-015** The user MAY enable self-evaluation with `--evaluate` or `research.evaluate.enabled: true` in `ragent.json`.

### Unwanted requirements

**FR-016** The system SHALL NOT ship a competitive-analysis report that lacks explicit comparison criteria or that synthesizes per-entity findings without a cross-entity table.

**FR-017** The system SHALL NOT perform web searches before asking a clarifying question when the user scope is ambiguous and clarification is enabled.

**FR-018** The system SHALL NOT reuse summarized source text as if it were the original source without recording the original URL and summary timestamp in the vault.

**FR-019** The system SHALL NOT silently ignore evaluation failures; any self-evaluation error SHALL be logged and surfaced in the report scorecard.

## Definitions

- **ODR**: Open Deep Research, the LangChain reference implementation.
- **supervisor/researcher graph**: an execution mode where a lead agent plans and delegates sub-research tasks to worker agents that each produce compressed findings.
- **competitive analysis mode**: a specialization of the supervisor/researcher graph where sub-researchers are assigned to individual entities in a comparison set.
- **research brief**: a detailed, first-person research question derived from the user's prompt that guides all downstream agents.
- **source vault**: the existing persistent source store used by ragent research runs.

## Acceptance criteria

- `/research --mode competitive "Compare Fireworks AI, Together.ai, and Groq for LLM inference"` produces a `RESEARCH.md` with per-entity profiles and a Markdown comparison table.
- `/research --mode supervisor "Explain Rust async runtimes"` uses parallel researchers, each returning compressed notes, and writes a single synthesized report.
- `/research "Research the inference market"` with an ambiguous scope asks one clarifying question before searching.
- Enabling self-evaluation appends a scorecard with scores for quality, relevance, groundedness, completeness, and structure.
- Summarized sources are persisted to the vault with original URLs and timestamps intact.

## Notes

- The existing `ragent-research` crate already contains `ResearchRunRequest`, `Tier`, `OutputFormat`, `WebGatherer`, source vaults, and the tier router. The new modes should extend these modules rather than replace them.
- The existing `mf_fetch` / `mf_search` infrastructure can fetch and search the web. Per-page summarization should invoke the configured lightweight LLM after fetching, if no lightweight LLM model is provided then it should use the currently selected model.
- The existing `ragent-llm` provider registry can resolve model names such as `openai:gpt-4.1-mini`.

---
status: implemented
audit:
  - { time: 1782809320, from: "none", to: "draft", actor: "system" }
---
# Research System Extensions (researchext)

## Introduction

The ragent `/research` command currently produces a single-pass research report by
running a web-gathering phase, an optional local/spec cross-reference phase, and a
single synthesis pass.  The research report `research/researchext/RESEARCH.md`
identifies a set of realistic, evidence-backed mechanisms that would improve the
quality, depth, transparency, and extensibility of the research output without
relying on closed, vendor-defined agent loops.

This specification defines a series of incremental extensions to `/research` that:

1. ground generation in explicit, quality-controlled retrieval;
2. add iterative planning, critique, and adaptive stopping;
3. decompose work across role-specific sub-agents and execute it in parallel;
4. verify claims against retrieved sources and emit structured citations;
5. persist research state across turns and support human-in-the-loop refinement;
6. expose a pluggable source registry so new sources can be added iteratively.

The extensions are designed to be implemented and released incrementally.  Each
requirement is written in EARS notation and is numbered, testable, and linked to
the implementation plan.

## Scope

### In scope

- The `ragent-research` crate, the `/research` slash command family, the
  `ragent research` CLI, and the `POST /research` HTTP endpoint.
- The `ResearchSession` orchestration engine and `ResearchDocument` output model.
- Research state persistence, progress events, and TUI/CLI rendering.
- Integration with existing ragent subsystems: event bus, memory tools, MCP client,
  team/task runtime, and code index.

### Out of scope

- Adding new LLM providers (covered by existing provider work).
- Rewriting the TUI event loop (small, focused changes are in scope; a full
  rewrite is not).
- Browser automation, CAPTCHA solving, or authenticated scraping.

## Requirements

### Ubiquitous

FR-001.  The `/research` command SHALL decompose the user topic into a set of
focused sub-questions before issuing any external search queries.

FR-002.  Every claim in the generated report SHALL be traceable to at least one
retrieved source passage via a stable citation identifier.

FR-003.  The research pipeline SHALL emit structured progress events for every
phase transition, sub-question completion, and source-fetch outcome so that the TUI
and CLI can render a live progress view.

### Event-driven

FR-004.  WHEN the user sends a follow-up message during an active research
session, `/research` SHALL update the current research plan with the new
requirement and issue any additional searches needed to satisfy it.

FR-005.  WHEN the synthesizer detects a missing link (e.g. an unresolved entity
or contradiction across sources), `/research` SHALL automatically spawn a targeted
follow-up retrieval for that bridge evidence and append the result to the current
session state.

FR-006.  WHEN a source fetch fails or returns no hits, `/research` SHALL log the
failure in session state and emit a `SourceFailed` event so the user can inspect
gaps after the run.

### State-driven

FR-007.  WHILE a research session is in progress, the session state SHALL track the
current plan, sub-question statuses, gathered sources, evaluation score, iteration
count, and pending evidence gaps.

FR-008.  IF the evaluation score does not improve between two consecutive
iterations, the adaptive stopper SHALL terminate retrieval early unless the user
has explicitly requested a deeper run.

FR-009.  IF the session state file exists for an incomplete research item, the
`ragent research continue <name>` command SHALL resume from that state instead of
starting over.

### Optional

FR-010.  The user MAY provide an `--iterations <N>` flag to override the default
maximum number of research iterations.

FR-011.  The user MAY provide a `--depth <shallow|standard|deep>` flag to choose a
predefined configuration of max iterations, source budget, and verification level.

FR-012.  The user MAY request a specific output artifact via `--format
<report|executive-summary|comparison-table|source-bibliography>`.

### Unwanted

FR-013.  The research system SHALL NOT emit a final report until at least one
source verification pass has been run when verification is enabled.

FR-014.  The research system SHALL NOT block the tokio runtime with synchronous
I/O during any phase that can be executed concurrently with other phases or
sub-questions.

FR-015.  The research system SHALL NOT hard-code a closed set of search sources;
instead, it SHALL discover available research sources through the MCP client and
skill registry at runtime.

## Design Principles

1. **Incremental release.** Each extension can be merged independently behind
   feature flags or new subcommand flags.
2. **Existing subsystems first.** Reuse the event bus, memory tools, MCP client,
   team/task runtime, and code index rather than inventing parallel mechanisms.
3. **Observable by default.** Every plan change, sub-agent decision, source
   fetch, and verification step is traced and surfaced as a structured event.
4. **Quality over speed.** The default path maximizes report reliability; speed
   improvements (parallelism, caching) must not remove verification or citation
   requirements.

## Acceptance Criteria

- A user can run `/research create --depth deep my-topic "some question"` and
  receive a multi-section report with citations and a sources appendix.
- A user can interrupt an in-progress session with a follow-up request and the
  planner will incorporate it without losing prior findings.
- A user can register a new MCP search tool and `/research` will include it in
  source discovery for future runs without a core code change.
- The benchmark added by this work shows measurable improvement in citation
  recall and precision compared to the current single-pass baseline.

## Related Work

- `research/researchext/RESEARCH.md` — background findings and source references.
- `docs/research.md` — current user-facing research documentation.
- `crates/ragent-research` — existing implementation.
- `APERFPLAN.md` — performance remediation that supports parallelism and
  non-blocking I/O requirements.

# Implementation Plan: Research System Extensions (researchext)

This plan turns the `researchext` specification into a concrete, phased set of
tasks.  Tasks are ordered so that early tasks build the data structures and
observability that later work depends on.  Each task maps to one or more
requirements and includes an effort estimate (S/M/L) and priority.

## Assumptions

- The existing `ResearchSession`, `ResearchDocument`, and gatherer architecture
  remains the execution base; new code is added alongside it.
- Work on blocking I/O and redundant loads in `ragent-agent` / `ragent-team`
  continues under `APERFPLAN.md`; this plan reuses the same patterns inside
  `ragent-research`.
- MCP client and skill registry APIs are already present in the workspace.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Define `ResearchState` and plan data model | FR-007 | M | Critical | completed | — |
| T-002 | Add structured session events for planner/critic/verifier/writer | FR-003 | M | Critical | completed | T-001 |
| T-003 | Implement topic-to-sub-question planner | FR-001 | M | High | completed | T-001, T-002 |
| T-004 | Add iterative research loop with state transitions | FR-004, FR-007 | L | Critical | completed | T-001, T-003 |
| T-005 | Implement missing-link detection and bridge queries | FR-005 | M | High | completed | T-003, T-004 |
| T-006 | Add source fetch failure handling and `SourceFailed` events | FR-006 | S | High | completed | T-002, T-004 |
| T-007 | Implement adaptive stopping policy | FR-008 | M | High | completed | T-004 |
| T-008 | Add `--iterations` and `--depth` CLI/TUI flags | FR-010, FR-011 | S | Medium | completed | T-004, T-007 |
| T-009 | Add `--format` output artifact selection | FR-012 | S | Medium | completed | T-004 |
| T-010 | Implement claim-to-source verification pass | FR-002, FR-013 | L | High | completed | T-004, T-006 |
| T-011 | Produce structured citations and sources appendix | FR-002 | M | High | completed | T-010 |
| T-012 | Implement `ragent research continue <name>` resume command | FR-009 | M | Medium | completed | T-001, T-004 |
| T-013 | Persist research state across turns via memory/state file | FR-007, FR-009 | M | High | completed | T-001, T-012 |
| T-014 | Add follow-up message handling during active session | FR-004 | M | High | completed | T-004, T-013 |
| T-015 | Integrate MCP/skill source registry for pluggable sources | FR-015 | L | High | completed | T-004 |
| T-016 | Execute sub-question tasks asynchronously and in parallel | FR-014, FR-004 | L | Critical | completed | T-004, T-015 |
| T-017 | Remove blocking I/O and redundant loads from research hot path | FR-014 | M | Critical | completed | T-016 |
| T-018 | Build research benchmark with golden queries and metrics | FR-002, FR-008 | L | Medium | completed | T-010, T-011 |
| T-019 | Update user documentation (`docs/research.md`) | FR-003, FR-010, FR-011, FR-012 | M | Low | completed | T-008, T-009, T-011 |
| T-020 | Update `SPEC.md` and `QUICKSTART.md` with research extensions | FR-003, FR-012 | S | Low | completed | T-019 |
## Task Details

### T-001 — Define `ResearchState` and plan data model

Create a serializable `ResearchState` struct in `crates/ragent-research/src/state.rs`
that stores:

- `topic` and derived `sub_questions` with status (`Pending`, `InProgress`,
  `Complete`, `Blocked`).
- Gathered `sources` keyed by stable source IDs.
- `evaluation_score` history per iteration.
- `iteration` counter and `max_iterations`.
- `pending_gaps` list of missing evidence.
- `plan_todos` list of planner actions.

Add serde support so the state can be saved to `research/<name>/STATE.json`.

### T-002 — Add structured session events for planner/critic/verifier/writer

Extend `SessionEvent` in `crates/ragent-research/src/session.rs` with events such
as:

- `PlanUpdated { sub_questions, gaps }`
- `SubQuestionComplete { id, sources_found }`
- `SourceFailed { url, error }`
- `CriticPass { iteration, score, new_gaps }`
- `VerifierPass { unsupported_claims }`
- `ArtifactGenerated { format }`

Ensure every new event can be serialized to the JSON-line protocol consumed by
`ragent-research:` prefixed output.

### T-003 — Implement topic-to-sub-question planner

Add a `ResearchPlanner` that takes the user topic and optional follow-up
requirements and produces an initial set of sub-questions.  The planner is an LLM
call with a structured output schema (JSON) and a chain-of-thought prompt.  Store
the result in `ResearchState`.

### T-004 — Add iterative research loop with state transitions

Refactor `ResearchSession::run` to execute a loop:

1. Plan (T-003).
2. For each pending sub-question, retrieve sources.
3. Synthesize partial findings.
4. Critique coverage and produce evaluation score.
5. If gaps remain and iterations allow, refine plan and repeat.
6. Verify and assemble final artifact.

Make the loop resumable from `ResearchState`.

### T-005 — Implement missing-link detection and bridge queries

In the critique step, detect unresolved entities or contradictions across
partial findings.  When detected, emit `PlanUpdated` with new bridge sub-questions
and run targeted searches before the next synthesis pass.

### T-006 — Add source fetch failure handling and `SourceFailed` events

Capture every failed web fetch, empty search result, or MCP source error.  Store
the failure in `ResearchState.sources` with a `Failed` variant and emit a
`SourceFailed` event.  Failed sources must still appear in the final sources
appendix with an error note.

### T-007 — Implement adaptive stopping policy

Implement an `AdaptiveStopper` that compares the evaluation score delta between
iterations.  Stop early when the delta falls below a configured threshold,
contradictions are resolved, and all sub-questions reach `Complete`.  Cap with
`max_iterations`.  Provide a `--depth` preset that configures the threshold and
budget.

### T-008 — Add `--iterations` and `--depth` CLI/TUI flags

Extend `ResearchCliCommand::Create` parsing and the `/research create` slash
command to accept `--iterations <N>` and `--depth <shallow|standard|deep>`.
Forward the values into `SessionConfig` and `ResearchState`.

### T-009 — Add `--format` output artifact selection

Add `--format <report|executive-summary|comparison-table|source-bibliography>` to
select which artifact(s) are produced.  The default remains the full report.
Implement dedicated writer prompts for each format.

### T-010 — Implement claim-to-source verification pass

Add a `Verifier` pass that compares each claim in the draft report against the
retrieved source passages.  Use either a deterministic string-overlap check or an
LLM-as-judge call.  Flag unsupported claims; either remove them or trigger a
follow-up bridge query.  Block final assembly when verification is enabled and
unsupported claims remain unaddressed (FR-013).

### T-011 — Produce structured citations and sources appendix

Update the final writer to emit citations as stable `[#id]` references and a
sources appendix with URL/path, retrieved snippet, and confidence tag.  Map each
citation back to its `Source` entry in `ResearchState`.

### T-012 — Implement `ragent research continue <name>` resume command

Add the `Continue { name }` variant to `ResearchCliCommand`, the equivalent slash
command `/research continue <name>`, and the matching HTTP endpoint.  Load the
existing `STATE.json`, validate that the item is not already `Complete`, and
resume the loop from the last persisted phase.

### T-013 — Persist research state across turns via memory/state file

On every phase transition, save `ResearchState` to
`research/<name>/STATE.json`.  Use `tokio::task::spawn_blocking` for the write so
the async runtime is not blocked.  Also write a memory entry with the research
name and current status so it is visible to the memory subsystem.

### T-014 — Add follow-up message handling during active session

Allow a user message sent while a research session is active to be treated as a
follow-up requirement rather than a new query.  Update `ResearchState.topic` and
append the requirement to a `follow_ups` list.  Trigger a new planner pass from the
updated state and continue the loop.

### T-015 — Integrate MCP/skill source registry for pluggable sources

Query the MCP client and skill registry at startup for tools tagged as research
sources.  Build a `SourceRegistry` of adapters.  When planning, select the
appropriate adapters per sub-question.  Document how a user can add a new MCP
server or skill to extend research sources without changing `ragent-research`
core code.

### T-016 — Execute sub-question tasks asynchronously and in parallel

Convert the per-sub-question retrieval and synthesis steps into async futures
executed with bounded concurrency (e.g. `futures::stream::FuturesUnordered` with a
`tokio::sync::Semaphore`).  Respect per-source rate limits and global concurrency
budgets.

### T-017 — Remove blocking I/O and redundant loads from research hot path

Audit `ragent-research` for `std::fs`, direct SQLite calls, and repeated
`Config::load()` / `ToolRegistry::definitions()` calls inside async functions.
Move blocking work to `spawn_blocking`, cache immutable configuration and tool
metadata, and reduce cloning of large source bodies.

### T-018 — Build research benchmark with golden queries and metrics

Create `crates/ragent-research/benches/research_quality.rs` or a dedicated
benchmark harness with a small set of golden queries and reference reports.
Compute:

- citation recall
- citation precision
- claim coverage
- hallucination/unsupported-claim rate

Run the benchmark against the baseline single-pass implementation and the new
iterative implementation.

### T-019 — Update user documentation (`docs/research.md`)

Document the new slash commands (`/research continue`, follow-up messages),
flags (`--depth`, `--iterations`, `--format`), adaptive stopping behavior,
citation format, and how to extend sources via MCP/skills.

### T-020 — Update `SPEC.md` and `QUICKSTART.md` with research extensions

Add a new section to the root `SPEC.md` describing the research system
extensions, their CLI/TUI surface, and the HTTP endpoints.  Update
`QUICKSTART.md` with a concise example of a deep research run.

## Milestones

### Milestone 1 — Observable state and planning

T-001, T-002, T-003

Outcome: `ResearchState` exists, events are emitted, and the planner produces
sub-questions.  No iterative loop yet.

### Milestone 2 — Iterative retrieval and adaptive stopping

T-004, T-005, T-006, T-007, T-008

Outcome: `/research create --depth deep` runs an iterative loop, detects missing
links, stops adaptively, and exposes failures.

### Milestone 3 — Verification, citations, and resumption

T-009, T-010, T-011, T-012, T-013, T-014

Outcome: Reports include verified citations, users can resume and refine
sessions, and multiple output formats are available.

### Milestone 4 — Extensibility and performance

T-015, T-016, T-017, T-018

Outcome: New sources are pluggable, sub-questions run in parallel with
non-blocking I/O, and a benchmark measures quality improvements.

### Milestone 5 — Documentation

T-019, T-020

Outcome: User docs and spec/QUICKSTART updates are merged.

## Effort Key

| Effort | Approximate duration |
|---|---|
| S | ≤ 1 day |
| M | 1–3 days |
| L | 3–7 days |

## Priority Key

| Priority | Meaning |
|---|---|
| Critical | Blocks other tasks or required for the feature to be coherent |
| High | Core user-visible capability |
| Medium | Important quality-of-life or testing improvement |
| Low | Documentation or polish |
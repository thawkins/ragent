---
status: draft
audit:
  - { time: 1789558898, from: "none", to: "draft", actor: "system" }
---
# Specification — Concept and finding output limits (`researchmax`)

## Introduction

The `/research create` pipeline produces a `RESEARCH.md` report with two
LLM-authored list sections: a **Concepts** section (cross-source theme map,
rendered under `## Concepts` in the report layout) and a **Findings** section
(numbered analytical findings, rendered under `## Findings`).

Both sections are currently unbounded apart from a soft prompt instruction:

- `CONCEPT_EXTRACTION_PROMPT_TEMPLATE` in `crates/ragent-research/src/cluster.rs`
  asks the model for "up to 20 core concepts" but enforces nothing; a model may
  return more or fewer, and the post-processor
  (`format_concepts_md_with_sources`, `concepts_section_for_research`) never
  truncates.
- The findings list (`AnalysisResult.findings`, `crates/ragent-research/src/analysis.rs`)
  is whatever the model emits; `merge_chunk_results` concatenates and renumbers
  but never caps or reorders.

This produces long, uneven reports: a verbose run can emit dozens of findings
and concepts of mixed relevance, and there is no way for a user to bound the
output. Findings and concepts are also ordered by whatever order the model
happened to emit them, not by how well-supported they are by relevant sources.

This specification adds two run-level limits — a concept limit (default 5) and a
finding limit (default 20) — settable with `--max-concepts` and `--max-findings`,
and requires both lists to be ordered by **reverse relevance** before the cap is
applied, so truncation always discards the least-relevant entries.

## Goals

1. Cap the number of concepts and findings that appear in the `/research create`
   report output, with defaults of 5 concepts and 20 findings.
2. Expose `--max-concepts N` and `--max-findings N` on every research entry
   point (root CLI, hand parser, TUI, HTTP `POST /research`).
3. Order concepts and findings by **reverse relevance** (most relevant first)
   before truncation, so the cap keeps the strongest entries.
4. Make the limits configurable through `ragent.json` `research.*` with per-run
   flag precedence, matching the existing `max_synthesis_sources` pattern.
5. Keep the default report output stable for callers that do not set the flags
   beyond the newly-capped list lengths; never panic or emit an empty section
   purely because a limit was reached.

## Scope

In scope:

- `crates/ragent-research/src/analysis.rs` (`AnalysisResult` ordering + cap).
- `crates/ragent-research/src/session.rs` (`AnalysisConfig`, synthesis and
  concepts-engine cap application).
- `crates/ragent-research/src/cluster.rs` (concept-section ordering and
  truncation; prompt instruction parameterisation).
- `crates/ragent-research/src/run_request.rs`, `cli.rs`, `run_config.rs`
  (flag plumbing and `SessionConfig` build).
- `crates/ragent-config/src/config.rs` (`research.max_concepts`,
  `research.max_findings` defaults).
- `src/cli.rs` (root clap arguments).
- `crates/ragent-server/src/routes/research.rs` (HTTP parity + invocation
  summary).
- `crates/ragent-tui` (completion/help parity).
- Docs (`docs/howtos/research.md`, `docs/howtos/slashcommands/research.md`) and
  CHANGELOG entries.

Out of scope:

- Changing how relevance ranks are computed for sources
  (`compute_relevance_label`, `Source::relevance_rank`) — reused as-is.
- Changing the `--max-synthesis-sources` cap, which bounds *input* sources and
  is orthogonal to these *output* list caps.
- Capping any other report section (implications, open questions,
  cross-references).

## Definitions

- **Reverse relevance order** — entries sorted most-relevant-first. A finding's
  relevance is derived from the source relevance of the `[#N]` citations it
  contains (highest cited `Source::relevance_rank` wins; ties broken by cited
  count, then by original model order). A concept's relevance is derived the
  same way from the `[#N]`/`web-NN` citations in its section body.
- **Effective limit** — the per-run flag value when supplied, otherwise the
  `ragent.json` `research.*` value, otherwise the built-in default.
- **Concept section** — one `### N. Name` block inside `ResearchDocument.concepts`.

## Requirements

### Ubiquitous requirements

**FR-001** — The system shall apply a concept limit and a finding limit to the
concepts and findings rendered in the `/research create` report output.

**FR-002** — The system shall default the concept limit to 5 entries and the
finding limit to 20 entries.

**FR-003** — The system shall order the retained findings by reverse relevance
(most relevant first) before applying the finding limit.

**FR-004** — The system shall order the retained concepts by reverse relevance
(most relevant first) before applying the concept limit.

**FR-005** — The system shall reuse the existing source relevance vocabulary
(`Source::relevance_rank`, ranks 8..1) to derive an entry's relevance; entries
with no recognized `[#N]` citation shall receive the default medium rank (5) so
they are ordered fairly rather than dropped first.

**FR-006** — The system shall accept a `--max-concepts N` argument that sets the
concept limit for a single research run.

**FR-007** — The system shall accept a `--max-findings N` argument that sets the
finding limit for a single research run.

**FR-008** — The system shall accept `research.max_concepts` and
`research.max_findings` keys in `ragent.json`, defaulting to 5 and 20
respectively.

**FR-009** — The system shall preserve the existing `## Concepts` and
`## Findings` section headings and structure; only the number and order of
entries shall change.

### Event-driven requirements

**FR-010** — When a `/research create` run completes synthesis, the system shall
truncate `AnalysisResult.findings` to the effective finding limit after
reordering, and shall renumber the surviving findings contiguously from 1.

**FR-011** — When a `/research create` run completes concept extraction, the
system shall truncate the concept section to the effective concept limit after
reordering and renumber the surviving `### N.` headings contiguously from 1.

**FR-012** — When a caller supplies `--max-concepts` or `--max-findings`, the
system shall forward the value through `ResearchRunRequest` into
`SessionConfig` so both the CLI/TUI path and the HTTP path observe the same
limit.

**FR-013** — When the research invocation is summarized (server
`invocation_summary`, replay strings), the system shall emit `--max-concepts N`
and `--max-findings N` so a recorded invocation round-trips through the hand
parser.

### State-driven requirements

**FR-014** — While the effective limit is unset for a run, the system shall use
the configured default (5 concepts, 20 findings), so behaviour is deterministic
across front ends.

**FR-015** — While the number of available concepts or findings is at or below
the effective limit, the system shall render all of them unchanged in count (only
their order may change).

### Optional requirements

**FR-016** — Where a run sets `--max-concepts 0` or `--max-findings 0`, the
system shall treat the limit as "unbounded" and apply no truncation, matching the
`0 disables the cap` convention already used by `--search-max-retries`.

**FR-017** — Where `research.max_concepts` / `research.max_findings` are
configured in `ragent.json`, the system shall use them in preference to the
built-in defaults for runs that do not pass the corresponding flag.

**FR-018** — Where the concept-extraction engine is not wired (no LLM concepts
engine) or the model returns no concept sections, the system shall omit the
`## Concepts` section entirely, as it does today.

### Unwanted-behaviour requirements

**FR-019** — If a supplied `--max-concepts` or `--max-findings` value is not a
non-negative integer, the system shall reject the argument with a clear error
rather than silently ignoring it or applying the default.

**FR-020** — If applying the finding limit would leave zero findings but the
model and fallback both produced findings, the system shall retain at least the
single most-relevant finding rather than emitting an empty `## Findings`
section.

**FR-021** — If a finding or concept contains malformed or non-numeric `[#N]`
citations, the system shall classify it with the default medium rank (5) and
shall not panic, error, or drop the entry on that basis alone.

**FR-022** — If the effective limit is larger than the number of available
entries, the system shall not pad, duplicate, or invent entries to reach the
limit.

### Non-functional requirements

**NFR-001** — The ordering and truncation passes shall be `O(n log n)` in the
number of entries and shall not read source bodies from disk (relevance is
derived from already-loaded citation markers and source metadata).

**NFR-002** — The change shall not alter report output for a run whose counts
are already within the limits, apart from the permitted reordering, and shall
keep `RESEARCH.md` byte-stable for the no-findings and no-concepts cases.

**NFR-003** — The new arguments and config keys shall be documented in
`docs/howtos/research.md` and the `/research help` output, and covered by a
CHANGELOG entry.

**NFR-004** — Code shall pass `cargo fmt --check`, `cargo clippy` (no new
warnings), and the existing `ragent-research`, `ragent-config`, `ragent-server`,
and `ragent-tui` test suites.

## Acceptance Criteria

1. A `/research create` run with no flags renders at most 5 concepts and at most
   20 findings, ordered most-relevant-first.
2. `--max-concepts 2` and `--max-findings 3` produce exactly 2 concepts and 3
   findings (when that many are available), and the retained entries are the
   highest-relevance ones.
3. `--max-concepts 0` and `--max-findings 0` produce no truncation.
4. `research.max_concepts` / `research.max_findings` in `ragent.json` change the
   default for runs without the corresponding flag.
5. `POST /research` accepts `max_concepts` / `max_findings` and the invocation
   summary round-trips them.
6. `/research help` lists both flags and the docs describe the defaults.

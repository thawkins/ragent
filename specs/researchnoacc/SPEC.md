---
status: draft
audit:
  - { time: 1789500365, from: "none", to: "draft", actor: "system" }
---
# Specification — Research scholarly-engine exclusion (`researchnoacc`)

## Introduction

When a user runs `/research create ... --no-papers` (the research "no papers"
option), the system intends to gather **web** sources only and skip academic
literature. Today it does not do that at the search layer.

The `--no-papers` option is threaded through the CLI, TUI, and server into
`WebGatherer.disable_scholarly`, but its only effect is a **post-search, per-hit
filter** in `crates/ragent-research/src/web_gatherer.rs`:

- the OpenAlex search call is still issued, consuming search budget and a
  (potentially metered) backend call;
- OpenAlex hits are still returned, deduplicated, and counted before being
  discarded;
- because deduplication is first-seen-wins, an academic URL consumed from the
  dedup set can shadow an otherwise-acceptable general-web URL for the same
  destination.

The `mf_search` tool exposes only a positive `engine` selector (restrict to
exactly one backend); it has **no** way to exclude a set of engines. The
`ragent-research` crate cannot steer engine choice either: its `WebSearchTool`
trait passes only `query` and `max_results`.

This specification adds a first-class **engine exclusion** capability to the
`mf_search` API and routes the research "no papers" option through it, so
OpenAlex and any other academically-classified provider is never queried when
the user asks for no papers. It also corrects the flag-name mismatch discovered
during analysis (the root CLI exposes `--no-scholarly` while the research parser
and every doc say `--no-papers`).

## Goals

1. Add an `exclude_engines` parameter to `mf_search` that removes a named set of
   backends from the orchestrator before any network call is made.
2. Introduce a single authoritative definition of which engines are
   "academic/scholarly" (OpenAlex today), reused by research classification and
   the research engine-exclusion mapping.
3. Make `--no-papers` (research) translate into an `mf_search` engine exclusion
   so OpenAlex is not queried, not returned, and not processed.
4. Make the engine exclusion configurable in `ragent.json` for both research
   runs and direct `mf_search` callers.
5. Keep `mf_search` fully functional and keyless when exclusions remove engines.
6. Fix the `--no-papers` / `--no-scholarly` flag spelling so the documented flag
   works on every entry point (CLI, TUI, server).

## Scope

In scope:

- `crates/ragent-tools-extended/src/masterfetch/search/` (engine classification,
  orchestrator exclusion).
- `crates/ragent-tools-extended/src/masterfetch/tools/search_tool.rs`
  (`mf_search` schema and execution).
- `crates/ragent-research` (web gatherer, config, request plumbing).
- `src/cli.rs`, `crates/ragent-research/src/cli.rs` (flag spelling).
- `crates/ragent-server/src/routes/research.rs` (HTTP parity).
- `crates/ragent-tui` (completion/help parity).
- Docs and CHANGELOG entries.

Out of scope:

- Changing the default engine set (OpenAlex + Wikipedia remain keyless defaults).
- Removing the existing hit-level scholarly filter (kept as defence in depth for
  callers that do not exclude the engine, e.g. a third party invoking
  `mf_search` directly).
- Open-access full-text recovery (`open_access_recovery`), which is an
  independent scholarly knob.

## Requirements

### Ubiquitous requirements

**FR-001** — The `mf_search` tool shall accept an optional `exclude_engines`
array parameter whose elements are engine names drawn from the same vocabulary
as the existing `engine` parameter (`openalex`, `wikipedia`, `langsearch`,
`tavily`, `perplexity`, `exa`, `serper`).

**FR-002** — The `SearchOrchestrator` shall provide a method that returns a new
orchestrator containing every registered engine **except** those named in the
exclusion list, leaving the receiver unchanged.

**FR-003** — The system shall define the set of academically-classified engine
names in exactly one place and reuse it for (a) research scholarly-hit
classification and (b) the research-to-`mf_search` exclusion mapping.

**FR-004** — The `WebSearchTool` trait in `crates/ragent-research/src/web_gatherer.rs`
shall accept an exclusion parameter so the gatherer can request that named
engines are not queried, and the concrete implementation that backs the trait
shall translate that parameter into `mf_search`'s `exclude_engines`.

**FR-005** — The `--no-papers` research option shall be spelled identically and
accepted on every entry point: the root `ragent` CLI, the `ragent research`
hand parser, the TUI `/research create` command, and `POST /research`.

### Event-driven requirements

**FR-006** — When a research run executes with `--no-papers` (or
`research.exclude_academic_engines` enabled in `ragent.json`), the system shall
issue search requests that exclude the academically-classified engines, so that
no OpenAlex query is dispatched.

**FR-007** — When `mf_search` executes and `exclude_engines` is supplied, the
tool shall invoke the orchestrator's exclusion method before running the search,
so excluded engines are never queried.

**FR-008** — When `mf_search` is invoked with `exclude_engines` naming an engine
that is not among the configured engines, the tool shall ignore that name and
proceed with the engines that remain, rather than failing the call.

**FR-009** — When `POST /research` receives a request carrying the scholarly
exclusion intent, the server shall serialise the corresponding flag into the run
invocation so the spawned research run excludes the academic engines.

### State-driven requirements

**FR-010** — While the research scholarly exclusion is active, the system shall
still fetch and process results from non-academic engines normally; exclusion
shall not reduce or disable general web gathering.

**FR-011** — While `exclude_engines` is absent or empty, `mf_search` shall
behave exactly as before, querying all configured engines in parallel.

### Optional requirements

**FR-012** — The system may expose the scholarly exclusion as a persistent
configuration value (`research.exclude_academic_engines`) in `ragent.json`, with
the per-run `--no-papers` flag taking precedence over the configured value.

**FR-013** — The system may accept the `exclude_engines` parameter in the
`websearch` compatibility tool as well, mapping it onto `mf_search`.

### Unwanted requirements

**FR-014** — If every configured engine would be removed by `exclude_engines`,
the system shall not query any engine and shall return an explicit result
stating that all engines were excluded, rather than panicking, hanging, or
reporting a silent zero-result success.

**FR-015** — The system shall not exclude an engine as a side effect of
`--no-papers` when that engine is not academically classified (for example,
Wikipedia or any configured web engine must continue to run).

### Non-functional requirements

**NFR-001** — Engine exclusion shall be applied before any network request is
issued for the excluded engines, so no search budget or metered call is spent on
an excluded backend.

**NFR-002** — `cargo fmt --check`, `cargo clippy`, and `cargo test` shall pass
for every crate touched by this change.

**NFR-003** — The default `mf_search` tool invocation (no `exclude_engines`)
shall produce byte-identical ranking, dedup, and metadata output to the current
implementation.

**NFR-004** — The excluded-engine behaviour shall be observable in logs and in
the tool metadata so operators can confirm which engines were skipped and why.

## Engine classification

The only academically-classified engine today is **OpenAlex** (scholarly works
catalog). The specification treats "academic" as a small, explicitly enumerated
set so that future scholarly backends (for example arXiv or Semantic Scholar) can
be added in one place:

| Engine name | Classification |
| ----------- | -------------- |
| `openalex`  | academic       |
| `wikipedia` | encyclopedia   |
| `langsearch`, `tavily`, `perplexity`, `exa`, `serper` | web |

## Configuration schema

`ragent.json` gains an optional boolean under the `research` block:

```jsonc
{
  "research": {
    // When true, mf_search is called with exclude_engines = ["openalex"] and
    // OpenAlex hits are filtered. Per-run --no-papers overrides this value.
    "exclude_academic_engines": false
  }
}
```

Precedence (highest first):

1. Per-run `--no-papers` flag (CLI/TUI) or the server request field.
2. `research.exclude_academic_engines` in `ragent.json`.
3. Default: `false` (academic engines enabled).

## Interfaces

### `mf_search` tool parameter (new)

```jsonc
{
  "exclude_engines": {
    "type": "array",
    "items": { "type": "string", "enum": ["openalex","wikipedia","langsearch","tavily","perplexity","exa","serper"] },
    "description": "Engine names to exclude from the search. Applied before any request; excluded engines are never queried."
  }
}
```

### `SearchOrchestrator` (new method)

```text
exclude_engines(&self, names: &[&str]) -> SearchOrchestrator
```

Returns a new orchestrator whose `engines` are those whose `name()` is not in
`names`. Mirrors the existing `select_engine`, including a fresh empty cache.

### `WebSearchTool` (research) (new parameter)

The trait's `search` method gains a parameter carrying the engine names to
exclude; the production adapter translates it to the `mf_search`
`exclude_engines` argument. The default (empty) must reproduce today's
behaviour.

## Acceptance criteria

1. `ragent research create x "topic" --no-papers` runs with no OpenAlex query
   dispatched (verified by log/metadata inspection), while Wikipedia and any
   configured web engines still return results.
2. `mf_search` invoked with `exclude_engines: ["openalex"]` returns no
   `openalex`-sourced rows and lists `openalex` in the skipped/excluded
   reporting.
3. `mf_search` invoked with no `exclude_engines` behaves exactly as before.
4. `mf_search` invoked with `exclude_engines` naming all configured engines
   returns an explicit "all engines excluded" result with no panic.
5. `POST /research` can disable scholarly engines without hand-editing the
   invocation string.
6. `--no-papers` is accepted by the root CLI, the research hand parser, and the
   TUI, and is listed in help/completion.

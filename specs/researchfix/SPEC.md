---
status: draft
audit:
  - { time: 1788368416, from: "none", to: "draft", actor: "system" }
---
# Research Web-Gather Phase Deadline Fix Specification

## Overview

This specification defines the required behaviour of the web-gathering phase of the
`/research create` pipeline. The phase discovers, fetches, and captures web sources. A
phase deadline (default 60 seconds) bounds the phase so a slow or unresponsive network
cannot stall a research run.

The current implementation partially satisfies this: a deadline is attached to the
`WebGatherer` via `with_phase_deadline`, and partial results are returned on truncation.
However, users observe runs that do not move to the next stage at the deadline, and the
TUI gives no visibility of the remaining time. Investigation identified three concrete
defects: (1) `GatherEvent::PhaseTimedOut` is emitted from multiple non-mutually-exclusive
sites, producing duplicated diagnostics and, in the mid-search truncation case, an event
claiming zero captured sources; (2) the iterative (multi-iteration / deep) engine never
attaches a phase deadline, so those runs are unbounded; (3) no structured phase-start
event exists, so the TUI cannot render a live countdown in the status-bar wait message.

This spec pins the correct end-state: a reliably enforced 60-second default deadline,
partial-captured-sources-as-corpus semantics, a single deadline notification, and a live
countdown in the top-right wait status message of the status bar.

## Requirements

### Functional Requirements

**FR-001 (Ubiquitous)**
The system shall bound the web-gathering phase of every `/research create` run to a
configurable deadline, defaulting to 60 seconds (`DEFAULT_WEB_PHASE_TIMEOUT_SECS`).

**FR-002 (Event-driven)**
When the web-phase deadline elapses, the system shall stop the web-gathering phase and
proceed to the next pipeline stage (ingestion, analysis, synthesis) using the sources
captured so far.

**FR-003 (State-driven)**
If the web-gathering phase is truncated by the deadline, the system shall ingest every
source fully captured before the deadline as the input corpus for the remaining pipeline
stages, and the generated `RESEARCH.md` shall cite only those captured sources.

**FR-004 (Event-driven)**
When the web-gathering phase is truncated by the deadline, the system shall emit exactly
one phase-timeout notification (`PhaseTimedOut`) per gather phase, carrying the effective
deadline in seconds and the number of sources captured before truncation.

**FR-005 (Unwanted)**
The system shall not discard partially captured sources, return an empty corpus, or abort
the research run when the web-phase deadline elapses.

**FR-006 (State-driven)**
While a research run uses the iterative engine (iterations greater than 1, or deep mode),
the system shall apply the same web-phase deadline to the web-gathering phase of each
iteration.

**FR-007 (Optional)**
The user may set the per-run web-phase deadline via `--web-time <secs>` (alias of
`--web-phase-timeout-secs`) on `ragent research create` and via the equivalent `/research
create` front-ends. Where a value of `0` is supplied, the system shall disable the
deadline and allow the web phase to run to natural completion.

**FR-008 (Unwanted)**
The system shall not start new web searches or new document fetches after the deadline has
elapsed; any overshoot beyond the deadline shall be bounded by the completion of at most
the fetches already in flight at truncation.

**FR-009 (Event-driven)**
When the web-gathering phase of a run begins, the system shall emit a phase-start
notification carrying the effective deadline in seconds, forwarded to the session event
stream so that UI layers can render a live countdown.

**FR-010 (Ubiquitous)**
While the web-gathering phase of a research run is active, the TUI shall display a live
countdown of the remaining time, appended to the wait status message in the top-right
segment of the status bar, updating at least once per second.

**FR-011 (State-driven)**
When the web-gathering phase ends (deadline hit, completed, or failed), the TUI shall
remove the countdown from the status-bar wait message and restore the normal wait or ready
state.

**FR-012 (Event-driven)**
When a phase-timeout notification is received, the TUI shall display a distinct notice in
the research progress message stating that the web-phase deadline was reached and how many
sources were captured, rendered at most once per run.

**FR-013 (Ubiquitous)**
The countdown display shall compute remaining time from a stored wall-clock deadline
(`Instant`) at render time, not from accumulated tick counts, so it remains accurate across
lagged event bursts and skipped redraws.

### Non-Functional Requirements

**NFR-001 (Ubiquitous)**
The TUI event loop shall wake at one-second cadence only while a web-phase countdown is
active; the countdown shall not introduce idle busy-redraws when no research run is
active.

**NFR-002 (Ubiquitous)**
A research run whose web phase is truncated by the deadline shall still complete and write
`RESEARCH.md`.

**NFR-003 (Ubiquitous)**
The default deadline value shall have a single source of truth
(`DEFAULT_WEB_PHASE_TIMEOUT_SECS`) shared by the CLI, TUI, and HTTP `/research` front-ends.

## Glossary

- **Web-gathering phase:** The pipeline stage that decomposes the research question into
  queries, searches the configured engines, and fetches/captures source documents.
- **Phase deadline:** The wall-clock `Instant` at which the web-gathering phase must stop
  initiating new work and return its partial results.
- **Partial corpus:** The set of sources fully captured before deadline truncation.
- **Phase-timeout notification:** The `GatherEvent::PhaseTimedOut` event and its
  `SessionEvent::RunStep { step: "web_deadline" }` forwarding.
- **Wait status message:** The status text rendered in the top-right segment of the TUI
  status bar (session status section, e.g. `[wait] research: <name> ...`).

## Assumptions and Constraints

- The existing deadline plumbing (`WebConfig.web_phase_timeout_secs`,
  `WebGatherer::with_phase_deadline`, `--web-time` CLI flag) is retained; this spec pins
  its correctness and observability rather than replacing it.
- Worst-case deadline overshoot is bounded by one in-flight fetch (bounded by the existing
  per-fetch timeout); sub-second precision is not required.
- HTTP `/research` SSE consumers receive the same session events; no new HTTP surface is
  introduced by this spec beyond the phase-start notification forwarded as a session
  event.
- Documentation (`docs/howtos/research.md`, root `SPEC.md` research section,
  `CHANGELOG.md`) shall be updated with the countdown and dedup behaviour as part of the
  change.
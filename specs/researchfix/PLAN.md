---
status: draft
---

# Research Web-Gather Phase Deadline Fix Implementation Plan

## Overview

This plan implements the web-gather phase deadline fixes: a reliably enforced 60-second
default deadline with single-notification semantics, partial-captured-sources-as-corpus
behaviour (including the iterative engine), a phase-start notification carrying the
effective deadline, and a live countdown in the TUI status-bar wait message.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Make `PhaseTimedOut` emission single-shot: remove duplicate emit sites in `web_gatherer.rs` (decomposer, search-loop, fetch-race) and emit only from the terminal site with the final captured count | FR-004, FR-005 | S | Critical | completed | — |
| T-002 | Attach the web-phase deadline in the iterative engine path so every iteration's gather is bounded by the configured timeout | FR-006 | M | Critical | completed | — |
| T-003 | Emit a phase-start notification (`GatherEvent` or `RunStep`) carrying the effective deadline seconds when the web-gather phase begins, and forward it through `session.rs` and `engine.rs` forwarders (exhaustive match) | FR-009 | S | High | completed | — |
| T-004 | Verify partial-corpus ingestion end-to-end for a truncated web phase: captured sources ingested, cited in `RESEARCH.md`, run completes | FR-002, FR-003, FR-005, NFR-002 | M | Critical | completed | T-001 |
| T-005 | Ensure the no-new-work-after-deadline guarantee: deadline gates decomposer, each search-result wait, and each fetch wait; document the in-flight-fetch overshoot bound | FR-008 | S | High | completed | — |
| T-006 | Add `web_phase_deadline: Option<Instant>` (and active-phase flag) to TUI research progress state, populated from the phase-start and phase-timeout events | FR-010, FR-013 | S | High | completed | T-003 |
| T-007 | Render the live countdown in the status-bar top-right wait segment (`build_line1_right`) from the stored `Instant` deadline, formatted `M:SS`; remove on phase end | FR-010, FR-011, FR-013 | M | High | completed | T-006 |
| T-008 | Add a one-second wake condition for active web-phase countdowns in `compute_next_deadline`, mirroring the permission-queue clause; no wake when inactive | FR-010, NFR-001 | S | High | completed | T-006 |
| T-009 | Render the distinct single-shot deadline notice in the research progress message (deadline reached, N sources captured) | FR-012 | S | Medium | completed | T-001, T-006 |
| T-010 | Preserve and verify CLI flag semantics: `--web-time` alias, default 60, `0` disables; keep single source of truth for the default across CLI/TUI/HTTP | FR-007, NFR-003 | S | Medium | completed | — |
| T-011 | Add/extend tests: single emission, iterative-path deadline, phase-start event, countdown rendering, and default-value assertions | FR-004, FR-006, FR-009, FR-010, FR-012 | M | High | completed | T-001, T-002, T-003, T-006 |
| T-012 | Update documentation: `docs/howtos/research.md` (deadline, countdown, dedup), root `SPEC.md` research section, `CHANGELOG.md` | FR-004, FR-007, FR-010 | S | Low | completed | T-007, T-009 |
## Notes

- T-001 through T-005 address the "not moving to the next stage" symptom: deduplicated
  notification plus verified partial-corpus continuation.
- T-003, T-006, T-007, T-008 deliver the status-bar countdown; the countdown must render
  from a stored `Instant` (FR-013) because `App.status` is only rewritten on events.
- T-010 is mostly verification: the flag plumbing exists and is covered by
  `test_web_time_deadline.rs`; the task guards against regressions and keeps
  `DEFAULT_WEB_PHASE_TIMEOUT_SECS` as the single default source.
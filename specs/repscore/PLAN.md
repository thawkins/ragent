# Implementation Plan: Reputation-Scored Web Source Relevance

## Overview

This plan implements the `repscore` specification. The work is divided into
research scaffolding, core reputation logic, configuration wiring, gatherer
integration, and verification.

## Tasks

| ID    | Title | Requirement | Effort | Priority | Dependencies |
| ----- | ----- | ----------- | ------ | -------- | -------------- |
| T-001 | Create `reputation.rs` module with tier/score types and bundled table | FR-001, FR-002, FR-003 | M | High | — |
| T-002 | Implement domain/suffix lookup with most-specific match wins | FR-002, FR-004, FR-005 | M | High | T-001 |
| T-003 | Implement score multiplier and `adjust_relevance_with_reputation` | FR-006, FR-007, FR-008 | M | High | T-002 |
| T-004 | Add override-file loading and merge logic (project-local + user-global) | FR-009 | L | Medium | T-001 |
| T-005 | Add override-file JSON schema validation and fallback on error | FR-010 | M | Medium | T-004 |
| T-006 | Add configuration surface in `ragent-config` for `research.reputation` | FR-009, FR-011 | M | High | — |
| T-007 | Wire reputation enabled/disabled flag through `WebGatherer` | FR-011 | S | Medium | T-006 |
| T-008 | Integrate reputation lookup into `WebGatherer` source capture | FR-001, FR-008, FR-013 | M | High | T-003, T-007 |
| T-009 | Update `Source::Web` to carry `reputation_score` and adjusted label | FR-001, FR-008 | S | High | T-008 |
| T-010 | Implement blocked-tier drop behaviour with `--use-low-relevance` override | FR-013 | S | High | T-008 |
| T-011 | Add protected-domain guard for override demotions | FR-016 | S | Low | T-004 |
| T-012 | Add optional `--show-reputation-breakdown` CLI flag | FR-014 | S | Low | T-009 |
| T-013 | Write unit tests for lookup, scoring, override merging, and edge cases | NFR-003 | M | High | T-002, T-003, T-004, T-005 |
| T-014 | Write integration test verifying labels include reputation tier | FR-008, NFR-002 | M | High | T-009 |
| T-015 | Write integration test verifying blocked sources are dropped | FR-013 | M | High | T-010 |
| T-016 | Run `cargo test -p ragent-research`, `cargo clippy`, and `cargo fmt` | NFR-003 | S | High | T-013, T-014, T-015 |
| T-017 | Update `RESEARCH.md` documentation and CHANGELOG entry | — | S | Medium | T-016 |

## Notes

- **T-001** keeps the bundled table conservative and technology-focused to match
the project's default user base.
- **T-004** uses existing `ragent-config` patterns for path resolution and
merging.
- **T-008** deliberately does not change the pre-filter thresholds; it only
changes the label and the adjusted score passed downstream.
- **T-014** can reuse the existing `WebGatherer` test harness with a fake fetch
that returns controlled URLs.
- **T-016** is the gate before the spec can transition to `implemented`.

## Milestones

1. **Milestone 1 — Core reputation engine** (T-001..T-003)
   - Pure functions exist and are unit-tested.
2. **Milestone 2 — Configuration and overrides** (T-004..T-007)
   - Users can enable/disable and supply custom tables.
3. **Milestone 3 — Gatherer integration** (T-008..T-012)
   - Real research runs produce adjusted labels.
4. **Milestone 4 — Verification and docs** (T-013..T-017)
   - Tests pass, clippy is clean, docs updated.

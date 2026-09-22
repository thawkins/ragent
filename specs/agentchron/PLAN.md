---
status: draft
spec_id: agentchron
title: Agent Cron System — Implementation Plan
created: 2025-01-01
owner: ragent-core
---

# Agent Cron System — Implementation Plan

## Overview

This plan implements the `agentchron` specification. Work is organised into
small, independently verifiable tasks that trace directly to the requirements in
`SPEC.md`. The order respects dependency chains: types → schedule parser →
data model → persistence → scheduler → slash command → logging → tests.

The key refinement in this revision is the **schedule grammar**: three forms
(`at <ts>`, `from <ts> every <d>`, `every <d>`) with a duration parser supporting
minutes, hours, days, weeks, and months.

## Architecture Decisions

1. **New crate module, not a new crate.** The cron scheduler is a background
   runtime concern that fits inside `ragent-agent` alongside the existing
   background-task and session machinery. No new workspace crate is needed.
2. **Reuse `Storage` + SQLite.** A new `cron_events` table is added to the
   existing `migrate()` batch in `ragent-storage`, mirroring how `todos`,
   `initiatives`, and `memories` tables were added.
3. **Reuse the edit-log JSONL pattern.** Event execution logging follows the
   exact pattern in `ragent-tools-core/src/edit_log.rs`: a `log/` directory,
   timestamped `cron-<ts>.jsonl` files, best-effort append, and a
   "pick most recent file" helper.
4. **Reuse the `new_task` spawn path.** Firing an event calls the same
   background spawn logic used by `new_task`, so permissions and agent
   resolution are consistent.
5. **Month = 30 days.** Calendar-month arithmetic would require a date library
   and introduces ambiguity (28–31 day months). The spec approximates a month
   as 2,592,000 seconds (30 × 24 × 3600), keeping `next_due` computation a simple
   integer add.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Define `CronEvent`, `CronForm`, `CronSchedule` types in `ragent-types` | FR-001, FR-002 | S | Critical | completed | — |
| T-002 | Implement duration parser (`<int><unit>` → secs) with unit aliases + validation | FR-014, FR-018, FR-019 | M | Critical | completed | T-001 |
| T-003 | Implement schedule parser for 3 forms (`at`, `from…every`, `every`) | FR-008, FR-009 | M | Critical | completed | T-002 |
| T-004 | Implement `next_due` computation (now+duration, start+duration, past-start advancement) | FR-004, FR-005, FR-008, FR-009 | S | Critical | completed | T-003 |
| T-005 | Add `cron_events` table + migration to `Storage` | FR-001, FR-002 | S | Critical | completed | T-001 |
| T-006 | Implement CRUD storage methods (insert/list/remove/get/touch-next-due) | FR-001, FR-002 | M | Critical | completed | T-005 |
| T-007 | Implement cron execution JSONL logger in `ragent-tools-core` | FR-003, FR-006 | M | High | completed | — |
| T-008 | Implement `read_cron_log` to parse `cron-*.jsonl` files | FR-003 | S | High | completed | T-007 |
| T-009 | Implement background scheduler loop (30 s tick) | FR-010, FR-017 | M | Critical | completed | T-006, T-004 |
| T-010 | Wire scheduler to spawn agent runs via `new_task` path + advance `next_due` | FR-004, FR-005 | M | Critical | completed | T-009 |
| T-011 | Enforce disabled-skip + unknown-agent-guard in scheduler | FR-007, FR-011, FR-016 | S | Critical | completed | T-010 |
| T-012 | Enforce no-double-fire guard for running repeating events | FR-012 | S | Medium | completed | T-010 |
| T-013 | Add `/cron` slash-command handler (add/remove/list/log/help) | FR-007 (surface), FR-008, FR-009 | L | Critical | completed | T-006, T-008, T-003 |
| T-014 | Register `/cron` in `SLASH_COMMANDS` + autocomplete menu | FR-007 (surface) | S | High | completed | T-013 |
| T-015 | Human-readable schedule description helper for `/cron list` | FR-015 | S | Low | completed | T-003 |
| T-016 | Write unit tests: duration parser (valid, zero, negative, bad unit) | FR-014, FR-018, FR-019 | S | Critical | completed | T-002 |
| T-017 | Write unit tests: schedule parser for all 3 forms | FR-008, FR-009 | S | Critical | completed | T-003 |
| T-018 | Write unit tests: storage round-trip (all fields persist) | FR-001, FR-002 | M | Critical | completed | T-006 |
| T-019 | Write integration tests: scheduler fires + logs + advances | FR-004, FR-005, FR-006 | L | Critical | completed | T-010, T-007 |
| T-020 | Write integration tests: disabled-skip, unknown-agent-error, double-fire-skip | FR-007, FR-011, FR-012, FR-016 | L | Critical | completed | T-011, T-012 |
| T-021 | Write test: `every <d>` with no start → next_due = now + d | FR-008 | S | Critical | completed | T-004 |
| T-022 | Write test: `from <past> every <d>` → next_due advanced to future | FR-009 | S | High | completed | T-004 |
| T-023 | Update `SPEC.md`, `QUICKSTART.md`, `CHANGELOG.md` | — | S | Medium | completed | T-013 |
## Dependency Graph

```
T-001 (types)
  └── T-002 (duration parser) ── T-003 (schedule parser) ── T-004 (next_due)
                                             │                    │
                                             ├── T-015 (desc)      │
                                             │                    │
T-005 (table) ── T-006 (CRUD) ───────────────┤                    │
                                             │                    │
T-007 (logger) ── T-008 (read log) ──────────┤                    │
                                             │                    │
                                             └── T-009 (scheduler) ── T-010 (spawn+advance) ── T-011 (guards) ── T-012 (double-fire)
                                                                    │
                                                                    ├── T-013 (/cron command) ── T-014 (register) ── T-023 (docs)
                                                                    │
T-016 (duration tests) ── T-017 (schedule tests) ── T-018 (storage tests)
T-021 (now+duration test) ── T-022 (past-start test)
T-019 (fire+log tests) ── T-020 (guard tests)
```

## Verification Plan

| Milestone                       | Verification                                                                  |
| ------------------------------- | ---------------------------------------------------------------------------- |
| Duration parser                 | `cargo test -p ragent-types cron_duration` — valid units, zero/negative rejected, bad unit rejected. |
| Schedule parser                 | `cargo test -p ragent-types cron_schedule` — all 3 forms parse correctly.    |
| `next_due` computation          | `every 30m` (no start) → now + 30m; `from <past> every 1h` → next future multiple. |
| Data model + storage            | `cargo test -p ragent-storage cron` — round-trip persists across reopen.     |
| Execution logging               | Unit test asserts a JSONL line exists in `log/cron-*.jsonl`.                  |
| Scheduler fires + logs + advances | Integration test: past-due repeating event → run spawned + log written + next_due advanced. |
| One-shot fires once             | Integration test: past-due one-shot → run spawned + event marked fired.      |
| Disabled / unknown / double-fire  | Integration tests assert `skipped`, `error`, and `skipped` outcomes.         |
| Slash command                   | Manual: `/cron add`, `/cron list`, `/cron log`, `/cron help` in the TUI.     |
| No TUI blocking                 | Assert scheduler tick runs on a background task (no main-thread stall).      |

## Effort & Priority Summary

- **Critical**: T-001, T-002, T-003, T-004, T-005, T-006, T-009, T-010, T-011, T-013, T-016, T-017, T-018, T-019, T-020, T-021
- **High**: T-007, T-008, T-014, T-022
- **Medium**: T-012, T-023
- **Low**: T-015

- **Effort S (small)**: T-001, T-004, T-005, T-008, T-011, T-012, T-014, T-015, T-016, T-017, T-021, T-022, T-023
- **Effort M (medium)**: T-002, T-003, T-006, T-007, T-009, T-010, T-018
- **Effort L (large)**: T-013, T-019, T-020
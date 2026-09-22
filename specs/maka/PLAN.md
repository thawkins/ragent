---
status: draft
---

# Implementation Plan: Activity Logging with Rollback and Resume

## Overview

This plan implements the activity logging subsystem specified in
`SPEC.md`. It is organised as a sequence of tasks that build the append-only
event log, the event types, the projection/replay engine, the rollback and
resume operations, and the supporting tooling.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Define event schema and event types | FR-001, FR-002, NFR-003 | S | Critical | completed | — |
| T-002 | Implement append-only event log store | FR-001, FR-002, FR-017, NFR-001 | L | Critical | completed | T-001 |
| T-003 | Add sequence number and immutable event ID | FR-002 | S | Critical | completed | T-001, T-002 |
| T-004 | Record model-message events | FR-001 | S | High | completed | T-002 |
| T-005 | Record tool-call and tool-result events | FR-004 | M | High | completed | T-002 |
| T-006 | Record permission-decision events | FR-005 | M | High | completed | T-002 |
| T-007 | Record termination events on interruption | FR-003 | M | Critical | completed | T-002 |
| T-008 | Implement checkpoint events | FR-008 | M | Medium | completed | T-002 |
| T-009 | Implement event-log retention (keep after completion) | FR-015 | S | Medium | completed | T-002 |
| T-010 | Implement optional retention limit / archival | FR-016, NFR-003 | M | Low | completed | T-009 |
| T-011 | Build projection replay engine | FR-012, FR-013, NFR-002 | L | Critical | completed | T-002, T-003 |
| T-012 | Implement derived-state projection (active context) | FR-012, FR-013 | M | High | completed | T-011 |
| T-013 | Implement rollback to checkpoint / sequence | FR-007, FR-012 | M | Critical | completed | T-011 |
| T-014 | Implement resume of interrupted run | FR-006, FR-013 | L | Critical | completed | T-011, T-007 |
| T-015 | Block concurrent ops during rebuild | FR-014 | M | High | completed | T-011 |
| T-016 | Reject mutation of committed events | FR-010 | M | High | completed | T-002 |
| T-017 | Validate event log consistency on resume | FR-011 | M | Critical | completed | T-011, T-014 |
| T-018 | Implement optional context pruning | FR-009 | M | Medium | completed | T-012 |
| T-019 | Implement run branching from checkpoint | FR-018 | M | Low | completed | T-008, T-011 |
| T-020 | Implement JSON Lines export of run log | NFR-004 | S | Medium | completed | T-002 |
| T-021 | Benchmark append latency and replay speed | NFR-001, NFR-002 | M | Medium | completed | T-002, T-011 |
| T-022 | Document operator controls (checkpoint, resume, retention) | FR-008, FR-016 | S | Medium | completed | T-008, T-010 |
## Sequencing Notes

- **T-001 → T-002** form the foundation; nothing else can proceed without the
  event schema and the append-only store.
- **T-011 (replay engine)** is the critical-path dependency for rollback
  (T-013) and resume (T-014); both are blocked until replay is functional.
- **T-014 (resume)** additionally depends on T-007 (termination events),
  because resume must operate on runs marked interrupted.
- **T-017 (consistency validation)** must land with resume, since an
  inconsistent log must abort resume per FR-011.
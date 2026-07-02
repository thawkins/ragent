# Implementation Plan: Edit Tool Renewal

## Overview

This plan implements the edit-tool renewal specified in `SPEC.md`. The work is concentrated in the `ragent-tools-core` crate, with follow-on updates to prompts, tests, and documentation. The strategy is to build the new strict matcher and the renewed `edit` tool first, then split the existing atomic batch logic into `multi_edit`, finally update all callers, prompts, and tests.

---

## Milestones

### Milestone 1: Strict Single-File Edit Tool
**Deliverable:** A Claude Code-compatible `edit` tool that enforces exact match, read-before-write, absolute paths, and structured result snippets.

- Add a strict exact-match replacement helper to `ragent-tools-core`.
- Implement the renewed `edit` tool with `file_path`, `old_string`, `new_string` parameters.
- Integrate read-timestamp validation and stale-file rejection.
- Generate line-numbered result snippets.
- Write unit and integration tests for FR-001 through FR-008.

### Milestone 2: Atomic Batch Edit Tool
**Deliverable:** A `multi_edit` tool that applies multiple exact-match edits atomically across files.

- Reuse the strict matcher from Milestone 1 for each edit in the batch.
- Preserve the existing overlap-check and highest-offset-first application order.
- Rename/alias the existing `multiedit` tool to `multi_edit`.
- Update batch validation to reject relative paths, stale files, and non-unique matches.
- Write integration tests for cross-file batches, overlap rejection, and atomic rollback.

### Milestone 3: Legacy Alias and Deprecation
**Deliverable:** Backward-compatible alias for the old `edit`/`multiedit` names with deprecation warnings.

- Register the legacy tool names as aliases that forward to the new implementations.
- Emit a deprecation warning in the tool output metadata when the alias is used.
- Add tests confirming the alias behaviour and warning text.

### Milestone 4: Prompts, Docs, and Agent Instructions
**Deliverable:** Updated tool descriptions, built-in prompts, user docs, and agent instructions.

- Update tool descriptions in `ragent-tools-core`.
- Update agent system prompt instructions that mention `edit`/`multiedit`.
- Write migration documentation under `docs/`.
- Update `QUICKSTART.md` or `SPEC.md` references if they mention the old tool names.

### Milestone 5: Verification and Release Readiness
**Deliverable:** All tests passing and the spec marked as implemented.

- Run `cargo test -p ragent-tools-core` and the full workspace test suite.
- Update `CHANGELOG.md` with the breaking/deprecation notes.
- Transition spec status to `implemented`.

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Create strict exact-match replacement helper | FR-004 | S | Critical | completed | — |
| T-002 | Add file read-timestamp tracking to `ToolContext` or session | FR-003 | S | Critical | completed | — |
| T-003 | Implement renewed `edit` tool with `file_path`/`old_string`/`new_string` | FR-001, FR-002, FR-006, FR-007 | M | Critical | completed | T-001, T-002 |
| T-004 | Add read-before-write and stale-file validation to `edit` | FR-003 | S | Critical | completed | T-002, T-003 |
| T-005 | Add absolute-path enforcement with clear error message | FR-002 | S | High | completed | T-003 |
| T-006 | Implement create/update/delete operation handling | FR-006 | S | High | completed | T-003 |
| T-007 | Generate line-numbered post-edit snippet in tool output | FR-008 | M | High | completed | T-003 |
| T-008 | Write unit and integration tests for renewed `edit` | FR-013 | M | Critical | completed | T-003–T-007 |
| T-009 | Refactor batch edit logic to use strict matcher | FR-009, FR-004 | M | High | completed | T-001 |
| T-010 | Rename/alias `multiedit` to `multi_edit` and update schema | FR-009 | S | High | completed | — |
| T-011 | Enforce absolute paths and read-timestamps for every batch edit | FR-009, FR-002, FR-003 | S | High | completed | T-002, T-010 |
| T-012 | Preserve overlap detection and atomic rollback in `multi_edit` | FR-009 | S | High | completed | T-009, T-010 |
| T-013 | Write integration tests for `multi_edit` | FR-013 | M | Critical | completed | T-009–T-012 |
| T-014 | Register legacy `edit`/`multiedit` aliases with deprecation warnings | FR-012 | S | Medium | completed | T-003, T-010 |
| T-015 | Test legacy alias behaviour and warning output | FR-012, FR-013 | S | Medium | completed | T-014 |
| T-016 | Update built-in agent instructions for new tool names | FR-014 | S | High | completed | T-003, T-010 |
| T-017 | Update user documentation and migration guide | FR-014 | M | Medium | completed | T-003, T-010 |
| T-018 | Update `CHANGELOG.md` with breaking/deprecation notes | FR-014 | S | Medium | completed | T-003, T-010, T-014 |
| T-019 | Run full workspace tests and fix regressions | FR-013 | M | Critical | completed | T-008, T-013, T-015 |
| T-020 | Mark spec status as implemented | FR-014 | S | Low | completed | T-018, T-019 |
## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Breaking existing ragent users** — Removing strict matcher tolerance may break many current agent prompts. | High | High | Keep legacy `edit`/`multiedit` aliases active for a deprecation window; update agent instructions; document migration path. |
| **Read-timestamp plumbing** — `ToolContext` currently does not carry read timestamps; plumbing through session may be invasive. | Medium | High | Store read timestamps in a shared session state accessible via `ToolContext`; fallback to file read if no timestamp exists with a warning. |
| **Result snippet format divergence** — Claude Code's snippet format is not fully specified and may be hard to replicate exactly. | Medium | Medium | Implement a `cat -n`-style snippet and accept minor formatting differences; add tests for line ranges and boundary clamping. |
| **Atomic batch deadlock** — File lock acquisition order changes when moving to `multi_edit`. | Low | High | Keep the existing sorted-path lock acquisition; add regression tests for concurrent batch edits. |

---

## Traceability Summary

- FR-001, FR-002 → T-003, T-005, T-014
- FR-003 → T-002, T-004, T-011
- FR-004 → T-001, T-009
- FR-005 → T-001, T-003
- FR-006 → T-003, T-006
- FR-007 → T-003
- FR-008 → T-007
- FR-009 → T-009–T-013
- FR-010 → existing permission/path checks (no new tasks)
- FR-011 → T-016, T-017
- FR-012 → T-014, T-015
- FR-013 → T-008, T-013, T-015, T-019
- FR-014 → T-016, T-017, T-018, T-020
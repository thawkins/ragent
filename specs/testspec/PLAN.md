# Implementation Plan: ragent Spec Management System

## Overview

This plan implements the Spec Management System defined in SPEC.md. The approach is incremental: build the core data structures and file I/O first, add validation and status tracking, then wire up TUI slash commands and agent integration. Each milestone delivers a working increment that can be demonstrated and tested independently.

The implementation spans two new crates:
- `ragent-specs` — core logic: spec discovery, validation, status transitions, task tracking
- Integration points in `ragent-tui` and `ragent-agent` — slash commands and programmatic API

---

## Milestones

### Milestone 1: Core Data Structures and File I/O ✅ COMPLETE
**Deliverable:** A standalone `ragent-specs` crate that can read and write spec directories.

- Define `SpecId`, `SpecStatus`, `Requirement`, `Task`, `Spec`, and `Plan` structs
- Implement directory creation, SPEC.md/PLAN.md generation from templates
- Implement spec discovery (walk `specs/` and parse directories)
- Implement file read/write with atomic updates
- Unit tests for all core types (27 tests passing)

### Milestone 2: EARS Validation Engine
**Deliverable:** A validation engine that can check any SPEC.md for EARS compliance.

- Implement regex-based EARS template detection for all five templates
- Implement structural validation (required sections, numbered requirements, valid status values)
- Implement PLAN.md completeness checks (existence, task IDs linked to requirements)
- CLI command `/spec validate` or `ragent spec validate`
- Unit tests for valid and invalid specs

### Milestone 3: Status Tracking and Transitions
**Deliverable:** A state machine for spec lifecycle with persistence.

- Implement status transition graph and validation
- Implement audit trail recording in spec frontmatter
- Implement spec listing with status filtering
- Implement spec search (full-text across SPEC.md and PLAN.md)
- SQLite index for fast listing/search (optional, fallback to filesystem)

### Milestone 4: TUI and CLI Integration
**Deliverable:** All `/spec` slash commands and `ragent spec` subcommands working.

- Register `/spec create`, `/spec list`, `/spec search`, `/spec validate`, `/spec status`, `/spec task` in TUI
- Implement equivalent CLI subcommands under `ragent spec`
- Render spec lists, detail views, and validation reports in the TUI
- Add autocomplete for spec IDs and status values

### Milestone 5: Agent Workflow Integration
**Deliverable:** Agents can read specs before implementation and update task status.

- Expose `SpecManager` API to agents
- Implement automatic task status updates when agent actions complete
- Implement requirement coverage reporting (`/spec coverage <spec-id>`)
- Add spec context injection into agent system prompts when a spec is active

### Milestone 6: Review Workflow and Archival
**Deliverable:** Full review workflow with reviewer assignment and archival.

- Implement REVIEW.md parsing and comment recording
- Implement reviewer assignment in spec frontmatter
- Implement spec archival and exclusion from default queries
- End-to-end integration tests

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|----|-------|-------------|--------|----------|--------------|
| T-001 | Define `SpecId` and `SpecStatus` enums | FR-005 | S | Critical | — |
| T-002 | Define `Requirement` struct with EARS template type | FR-003 | S | Critical | T-001 |
| T-003 | Define `Task` struct with requirement linkage | FR-015 | S | Critical | T-001 |
| T-004 | Define `Spec` and `Plan` structs | FR-001, FR-004 | S | Critical | T-002, T-003 |
| T-005 | Implement spec directory creation and template generation | FR-001, FR-002 | M | Critical | T-004 |
| T-006 | Implement spec discovery (walkdir over `specs/`) | FR-007 | S | High | T-004 |
| T-007 | Implement atomic file read/write for spec files | NFR-003 | S | High | T-005 |
| T-008 | Implement EARS ubiquitous template detection | FR-003 | S | Critical | T-002 |
| T-009 | Implement EARS event-driven template detection | FR-003 | S | Critical | T-008 |
| T-010 | Implement EARS state-driven template detection | FR-003 | S | Critical | T-008 |
| T-011 | Implement EARS optional template detection | FR-003 | S | Critical | T-008 |
| T-012 | Implement EARS unwanted behaviour template detection | FR-003 | S | Critical | T-008 |
| T-013 | Implement structural validation (sections, numbering, status) | FR-003, FR-012 | M | High | T-008–T-012 |
| T-014 | Implement PLAN.md completeness check | FR-004 | S | High | T-013 |
| T-015 | Implement spec status transition validation | FR-006 | M | High | T-001 |
| T-016 | Implement audit trail recording | FR-011 | M | Medium | T-015 |
| T-017 | Implement spec listing with sorting and filtering | FR-007 | M | High | T-006, T-015 |
| T-018 | Implement full-text search across specs | FR-008 | L | Medium | T-006 |
| T-019 | Implement SQLite spec index (optional performance layer) | NFR-001, NFR-002 | L | Low | T-017, T-018 |
| T-020 | Implement `/spec create` TUI slash command | FR-002 | M | High | T-005 |
| T-021 | Implement `/spec list` TUI slash command | FR-007 | M | High | T-017 |
| T-022 | Implement `/spec search` TUI slash command | FR-008 | M | Medium | T-018 |
| T-023 | Implement `/spec validate` TUI slash command | FR-012 | M | High | T-013, T-014 |
| T-024 | Implement `/spec status` TUI slash command | FR-005, FR-006 | M | High | T-015 |
| T-025 | Implement `/spec task` TUI slash command | FR-015 | M | High | T-003 |
| T-026 | Implement `ragent spec` CLI subcommands | FR-002, FR-007, FR-012 | M | High | T-020–T-025 |
| T-027 | Expose `SpecManager` API to agents | FR-013 | M | Medium | T-004 |
| T-028 | Implement automatic task status updates from agent actions | FR-009, FR-013 | M | Medium | T-027 |
| T-029 | Implement requirement coverage reporting | FR-009 | S | Medium | T-028 |
| T-030 | Implement spec context injection into agent prompts | FR-013 | M | Low | T-027 |
| T-031 | Implement REVIEW.md parsing and comment recording | FR-010 | M | Medium | T-004 |
| T-032 | Implement reviewer assignment in spec frontmatter | FR-010 | S | Medium | T-031 |
| T-033 | Implement spec archival and filtered queries | FR-014 | M | Low | T-017 |
| T-034 | Write unit tests for core data structures | NFR-003 | M | Critical | T-001–T-007 |
| T-035 | Write integration tests for validation engine | FR-012 | M | High | T-013–T-014 |
| T-036 | Write end-to-end tests for TUI commands | NFR-004 | L | Medium | T-020–T-026 |
| T-037 | Write user documentation (`docs/specs.md`) | NFR-004 | M | Low | T-026 |
| T-038 | Update `ragent.json` schema with `specs.*` configuration keys | NFR-005 | S | Low | T-005 |

---

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Markdown parsing complexity** — Pulldown-cmark may not expose enough structure for reliable EARS validation | Medium | High | Use regex as primary validator; treat markdown parser as supplementary. Maintain a comprehensive test corpus of valid and invalid specs. |
| **Performance at scale** — Projects with 1,000+ specs may exceed the 500ms listing target | Medium | Medium | Implement optional SQLite index (T-019). Benchmark early (Milestone 3). Allow spec ID prefix filtering to limit traversal. |
| **File I/O race conditions** — Concurrent agent and user edits to the same spec | Low | High | Use atomic writes (write to temp, rename). Detect mtime changes on read and warn. Consider file locking for future enhancement. |
| **TUI command bloat** — Too many `/spec` commands may clutter slash command help | Low | Low | Group commands logically (`/spec create`, `/spec list`, etc.). Provide `/spec help` for detailed subcommand help. |
| **Agent integration fragility** — Agents may not reliably update task status | Medium | Medium | Make task updates idempotent. Provide manual `/spec task complete` as fallback. Add validation that warns on stale task states. |
| **Template drift** — EARS templates may evolve, invalidating old specs | Low | Medium | Version the EARS schema in spec frontmatter. Maintain backward compatibility in validators. Document template changes in CHANGELOG. |

---

## Definition of Done

The Spec Management System is considered complete when all the following criteria are met:

1. **All Milestones Delivered** — Milestones 1 through 6 are implemented, merged, and tagged.
2. **All Critical and High Priority Tasks Complete** — Tasks T-001 through T-026, T-034, and T-035 are done.
3. **Test Coverage** — Unit and integration tests achieve >80% code coverage for `ragent-specs`.
4. **Validation Passes** — Running `/spec validate` on the project's own `specs/` directory reports zero structural violations.
5. **Performance Targets Met** — Listing 1,000 specs completes in <500ms; search returns results in <2s.
6. **Documentation Complete** — `docs/specs.md` exists and describes all commands, configuration, and workflow.
7. **No Critical Bugs** — Zero open issues labeled `bug` with `priority: critical` or `priority: high`.
8. **CI Integration** — A CI job runs `/spec validate` and fails the build on violations.
9. **User Acceptance** — At least one non-author user has created a spec, advanced it through the lifecycle, and implemented it without assistance.
10. **Code Review** — All changes reviewed by at least one maintainer and approved.

---

*End of Implementation Plan*

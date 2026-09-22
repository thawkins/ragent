---
spec_id: reqeng
---

# Implementation Plan: Back-fill SDD Capabilities

## Overview

This plan back-fills the missing Spec-Driven Development (SDD) capabilities
from GitHub's `spec-kit/spec-driven.md` into ragent's `/spec` feature. The
work is organized into six milestones, each closing a cluster of related
gaps. All new capabilities are opt-in and backward-compatible with existing
spec directories.

### Current State Summary

The updated gap analysis in the SPEC.md clarifies that several SDD
capabilities are already partially present in ragent, albeit implemented
differently:

- **Plan generation** — `/spec update` and `/spec add` already regenerate
  PLAN.md from an edited SPEC.md. The new `/spec plan` command adds a
  separate entry point that accepts technology context as an argument,
  rather than replacing the existing update workflow.
- **Task lists** — Tasks are currently embedded in the PLAN.md task
  table and executed via `/spec impl`. The new `/spec tasks` command
  extracts them into a standalone `TASKS.md` artifact, matching SDD's
  separation of plan and tasks.
- **Research linking** — `/research` can already be linked to a spec
  via the `--from-research` flag. The new work tightens this integration
  into the `/spec specify` flow and standardises the frontmatter
  `research:` field.

These nuances are reflected in the milestone descriptions and task
scoping below.

---

## Milestones

### Milestone 1: Core Commands & Markers
**Deliverable:** Separate `/spec specify`, `/spec plan`, and `/spec tasks` commands plus `[NEEDS CLARIFICATION]` marker support.

- Add `SpecCommand::Specify` variant and parser
- Add `SpecCommand::Plan` variant with technology-context argument (alongside existing `/spec update` and `/spec add`)
- Add `SpecCommand::Tasks` variant (extracts task table from PLAN.md into standalone TASKS.md)
- Implement `[NEEDS CLARIFICATION]` marker detection in validate.rs
- Add clarification-gate to lifecycle transitions (block `approved`)

### Milestone 2: Templates & Quality Checklists
**Deliverable:** Updated spec and plan templates with embedded quality checklists.

- Extend `SpecTemplate` with optional quality-checklist section
- Extend `PlanTemplate` with optional quality-checklist section
- Add template-generation tests for checklists

### Milestone 3: Constitution & Pre-Implementation Gates
**Deliverable:** `CONSTITUTION.md` support and Phase -1 gate validation.

- Add `Constitution` struct and parser
- Add `CONSTITUTION.md` default template
- Implement Phase -1 gate parsing in PLAN.md
- Add gate validation to `validate_plan`
- Block `in_progress` transition on failed gates

### Milestone 4: Artifacts (Research, Data-Model, Contracts, Quickstart)
**Deliverable:** Generation of SDD artifacts alongside specs.

- Add `data-model.md` generation in `/spec plan`
- Add `contracts/` directory generation in `/spec plan`
- Add `quickstart.md` generation in `/spec tasks`
- Tighten research linking in `/spec specify` frontmatter (existing `--from-research` flag remains)

### Milestone 5: Consistency Validation & Feedback Loop
**Deliverable:** Semantic validation and production feedback.

- Implement ambiguity/contradiction/gap detection in validate.rs
- Add `FEEDBACK.md` support and `/spec feedback` command
- Surface feedback notes during `/spec plan` regeneration

### Milestone 6: Workflow Integration & Polish
**Deliverable:** Branch-per-spec, test-first ordering, config toggles, docs.

- Add optional git branch creation in `/spec specify`
- Enforce test-first file creation order in plan templates
- Add configuration flags for opt-in capabilities
- Update QUICKSTART.md and SPEC.md with new commands
- Update CHANGELOG.md

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `SpecCommand::Specify` variant and parser | FR-001 | S | Critical | completed | — |
| T-002 | Implement `/spec specify` handler generating SPEC.md only (no PLAN.md) | FR-001 | M | Critical | completed | T-001 |
| T-003 | Add `SpecCommand::Plan` variant with technology-context argument | FR-004 | S | Critical | completed | — |
| T-004 | Implement `/spec plan` handler generating/regenerating PLAN.md from SPEC.md + tech context (alongside existing `/spec update` and `/spec add`) | FR-004 | L | Critical | completed | T-003 |
| T-005 | Add `SpecCommand::Tasks` variant and parser | FR-005 | S | High | completed | — |
| T-006 | Implement `/spec tasks` handler extracting task table from PLAN.md into standalone TASKS.md | FR-005 | M | High | completed | T-005, T-004 |
| T-007 | Implement `[NEEDS CLARIFICATION]` marker detection regex in validate.rs | FR-002 | S | Critical | completed | — |
| T-008 | Add clarification-marker reporting to validation Report output | FR-002 | S | Critical | completed | T-007 |
| T-009 | Block `approved` transition when unresolved clarification markers exist | FR-003 | S | Critical | completed | T-007 |
| T-010 | Extend `SpecTemplate::generate` with optional quality-checklist section | FR-006 | S | High | completed | — |
| T-011 | Extend `PlanTemplate::generate` with optional quality-checklist section | FR-006 | S | High | completed | — |
| T-012 | Add template-generation tests for checklist embedding | FR-006, NFR-004 | S | Medium | completed | T-010, T-011 |
| T-013 | Define `Constitution` struct and parser in ragent-specs | FR-007 | M | High | completed | — |
| T-014 | Add `CONSTITUTION.md` default template generation | FR-007 | S | High | completed | T-013 |
| T-015 | Implement Phase -1 gate checkbox parsing in PLAN.md (plan_parser.rs) | FR-008 | M | High | completed | T-013 |
| T-016 | Add Phase -1 gate validation to `validate_plan` | FR-008 | M | High | completed | T-015 |
| T-017 | Block `in_progress` transition when Phase -1 gates are unchecked | FR-008 | S | High | completed | T-016 |
| T-018 | Add optional git branch creation in `/spec specify` | FR-009 | M | Medium | completed | T-002 |
| T-019 | Standardise research artifact linking in SPEC.md frontmatter (build on existing `--from-research` flag) | FR-010 | S | Medium | completed | T-002 |
| T-020 | Add `## Related Research` section generation when research context is provided | FR-010 | S | Medium | completed | T-019 |
| T-021 | Implement `data-model.md` generation in `/spec plan` | FR-011 | M | Medium | completed | T-004 |
| T-022 | Implement `contracts/` directory generation in `/spec plan` | FR-012 | M | Medium | completed | T-004 |
| T-023 | Implement `quickstart.md` generation in `/spec tasks` | FR-013 | M | Medium | completed | T-006 |
| T-024 | Add test-first file creation ordering to plan template | FR-014 | S | Medium | completed | T-011 |
| T-025 | Add advisory warning in implementation runner when tasks violate file creation order | FR-014 | M | Low | completed | T-024 |
| T-026 | Implement ambiguity detection in validate.rs (vague terms, undefined references) | FR-015 | L | High | completed | — |
| T-027 | Implement contradiction detection between requirements | FR-015 | L | Medium | completed | T-026 |
| T-028 | Implement gap detection for requirements lacking acceptance criteria | FR-015 | M | Medium | completed | T-026 |
| T-029 | Add consistency-check warnings to validation Report | FR-015 | S | High | completed | T-026, T-027, T-028 |
| T-030 | Implement constitutional amendment process with dated changelog | FR-016 | M | Low | completed | T-013 |
| T-031 | Add `FEEDBACK.md` file support in spec directory | FR-017 | S | Low | completed | — |
| T-032 | Add `/spec feedback <spec-id> <note>` command to append feedback notes | FR-017 | S | Low | completed | T-031 |
| T-033 | Surface feedback notes during `/spec plan` regeneration | FR-017 | S | Low | completed | T-032, T-004 |
| T-034 | Verify backward compatibility: existing specs validate without new artifacts | FR-018 | S | Critical | completed | T-002, T-004, T-006, T-013, T-016 |
| T-035 | Add configuration flags for opt-in SDD capabilities in ragent.json | FR-019 | M | High | completed | — |
| T-036 | Wire configuration flags to gate new artifact generation and validation | FR-019 | M | High | completed | T-035 |
| T-037 | Document gap status and resolution in this PLAN.md | FR-020 | S | Low | completed | — |
| T-038 | Update QUICKSTART.md with new `/spec` subcommands | FR-020 | S | Medium | completed | T-002, T-004, T-006 |
| T-039 | Update SPEC.md (project root) with SDD capability descriptions | FR-020 | S | Medium | completed | T-002, T-004, T-006 |
| T-040 | Update CHANGELOG.md with all SDD back-fill changes | FR-020 | S | Medium | completed | T-001–T-039 |
| T-041 | Add unit tests for all new parsers, validators, and generators | NFR-004 | L | High | completed | T-001–T-033 |
| T-042 | Add DOCBLOCK documentation to all new public functions | NFR-005 | S | Medium | completed | T-001–T-033 |
| T-043 | Benchmark validation with 50-requirement spec to confirm <500ms | NFR-001 | S | Medium | completed | T-026, T-029 |
## Gap Resolution Tracking

This section tracks how each gap identified in the SPEC.md Gap Analysis
is addressed by the tasks above, including the current resolution status of
each gap so that back-fill progress is trackable (FR-020).

**Status legend:** ✅ Resolved — all tasks complete · ⏳ Partial — some tasks
complete, some pending · ⬜ Not started — no tasks complete yet

**Progress summary (as of T-037):** 0 resolved · 7 partial · 9 not started ·
16 total gaps

| Gap # | SDD Capability | Resolution | Tasks | Status |
|-------|---------------|-----------|-------|--------|
| 1 | Separate `/specify` command | New `/spec specify` command creates SPEC.md only | T-001, T-002 | ⏳ Partial |
| 2 | Separate `/plan` command | New `/spec plan` command with tech-context argument (alongside existing `/spec update`/`/spec add`) | T-003, T-004 | ⏳ Partial |
| 3 | Separate `/tasks` command | New `/spec tasks` command extracts task table into TASKS.md | T-005, T-006 | ⏳ Partial |
| 4 | `[NEEDS CLARIFICATION]` markers | Marker detection in validation + transition gate | T-007, T-008, T-009 | ⏳ Partial |
| 5 | Quality checklists in templates | Optional checklist sections in SpecTemplate and PlanTemplate | T-010, T-011, T-012 | ⏳ Partial |
| 6 | Constitution artifact | `CONSTITUTION.md` with nine articles | T-013, T-014 | ⏳ Partial |
| 7 | Phase -1 pre-implementation gates | Gate parsing and validation blocking `in_progress` | T-015, T-016, T-017 | ⬜ Not started |
| 8 | Branch-per-spec git workflow | Optional branch creation in `/spec specify` | T-018 | ⬜ Not started |
| 9 | Research artifact alongside spec | Standardised frontmatter linking (builds on existing `--from-research` flag) | T-019, T-020 | ⬜ Not started |
| 10 | Data-model artifact | `data-model.md` generation in `/spec plan` | T-021 | ⬜ Not started |
| 11 | Contracts artifact | `contracts/` directory generation in `/spec plan` | T-022 | ⬜ Not started |
| 12 | Quickstart validation scenarios | `quickstart.md` generation in `/spec tasks` | T-023 | ⬜ Not started |
| 13 | Test-first file creation ordering | Ordering in plan template + advisory warning in runner | T-024, T-025 | ⬜ Not started |
| 14 | Ongoing consistency validation | Ambiguity, contradiction, gap detection in validate.rs | T-026, T-027, T-028, T-029 | ⏳ Partial |
| 15 | Constitutional amendment process | Dated changelog and rationale requirements | T-030 | ⬜ Not started |
| 16 | Production feedback loop | `FEEDBACK.md` + `/spec feedback` command + surfacing in `/spec plan` | T-031, T-032, T-033 | ⏳ Partial |

### Completed Task Summary

The following tasks have been completed, contributing to the partial gap
resolutions above:

| Task | Gap(s) | Deliverable |
|------|--------|-------------|
| T-001 | 1 | `SpecCommand::Specify` variant and parser |
| T-003 | 2 | `SpecCommand::Plan` variant with technology-context argument |
| T-005 | 3 | `SpecCommand::Tasks` variant and parser |
| T-007 | 4 | `[NEEDS CLARIFICATION]` marker detection regex |
| T-010 | 5 | Quality-checklist section in `SpecTemplate` |
| T-011 | 5 | Quality-checklist section in `PlanTemplate` |
| T-013 | 6 | `Constitution` struct and parser |
| T-026 | 14 | Ambiguity detection (vague terms, undefined references) |
| T-031 | 16 | `FEEDBACK.md` file support in spec directory |
| T-035 | — | Configuration flags (`SddConfig`) for opt-in SDD capabilities |

### Remaining Work by Gap

Gaps not yet started (⬜) require implementation of all listed tasks. Partial
gaps (⏳) require completing the remaining pending tasks listed in the Tasks
table above. See the Tasks table for dependency ordering.

---

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| New validation checks slow down `/spec validate` on large specs | Medium | Medium | Benchmark with 50-requirement spec (T-043); make semantic checks opt-in |
| Backward-incompatible changes break existing spec directories | Low | High | T-034 explicitly tests backward compatibility; all new artifacts are opt-in (FR-019) |
| Constitution parser conflicts with user-authored CONSTITUTION.md | Low | Medium | Use lenient parsing with fallback defaults; document the expected format |
| Git branch creation fails in non-git workspaces | Medium | Low | Make branching optional with graceful fallback (FR-009) |
| Consistency validation produces false positives | Medium | Medium | Start with warnings (not errors); allow suppression via configuration |
| `/spec plan` overlaps confusingly with existing `/spec update` and `/spec add` | Medium | Medium | Document the distinction: `/spec plan` accepts tech-context argument; `/spec update` regenerates from edited SPEC.md; both coexist |
| `/spec tasks` duplicates task table already in PLAN.md | Low | Low | TASKS.md is a derived artifact — a flattened, ordered list; PLAN.md retains the rich table with dependencies |

---

## Definition of Done

1. All 20 functional requirements (FR-001 through FR-020) are implemented and verified
2. All 5 non-functional requirements (NFR-001 through NFR-005) are satisfied
3. Existing spec directories continue to validate, list, search, and implement without modification
4. All new capabilities are opt-in via configuration flags
5. Unit tests cover all new parsing, validation, and generation logic
6. QUICKSTART.md, SPEC.md, and CHANGELOG.md are updated
7. `cargo test -p ragent-specs` passes with no regressions
8. `cargo clippy -p ragent-specs` produces no new warnings

---

*End of Implementation Plan*
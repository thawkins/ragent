---
status: draft
audit:
  - { time: 1786622116, from: "none", to: "draft", actor: "system" }
---
# Specification: Back-fill Spec-Driven Development (SDD) Capabilities

## Executive Summary

The GitHub `spec-kit` project defines a Spec-Driven Development (SDD) methodology
in `spec-driven.md`. ragent already implements a substantial `/spec` feature set —
EARS-notation requirements, lifecycle states, plan parsing, validation, coverage
reports, JTBD analysis, and an implementation runner. However, several capabilities
described in the SDD methodology are missing or only partially implemented.

This specification defines the requirements to back-fill the missing SDD
capabilities into ragent's spec management system, bringing it into closer
alignment with the SDD methodology while preserving ragent's existing EARS-based
workflow and crate architecture.

## Scope & Objectives

### Scope

**In scope:**

- Gap analysis between SDD `spec-driven.md` and ragent's `/spec` implementation
- New `/spec plan` command for technology-context-driven implementation planning
- New `/spec tasks` command for task-list generation from a plan
- `[NEEDS CLARIFICATION]` marker support in spec creation and validation
- Quality checklists in spec and plan templates (completeness, no speculative features)
- Architectural constitution artifact (`CONSTITUTION.md`) with immutable articles
- Phase -1 pre-implementation gates (Simplicity, Anti-Abstraction, Integration-First)
- Branch-per-spec git workflow integration
- Research, data-model, and contracts artifact generation alongside specs
- Quickstart validation scenarios as a spec artifact
- Test-first file creation ordering enforcement in implementation plans
- Ongoing consistency validation (ambiguity, contradiction, gap detection)
- Constitutional amendment process with versioned principles
- Production feedback loop (metrics/incidents → spec updates)

**Out of scope:**

- Rewriting the existing EARS validation engine
- Replacing the current lifecycle state machine
- Changes to the `ragent-specs` crate's public API for existing commands
- Full code generation from specifications (a future capability)

### Objectives

1. Close every identified gap between SDD methodology and ragent's `/spec` feature
2. Preserve backward compatibility with existing spec directories and commands
3. Maintain ragent's EARS-notation requirement style
4. Keep all new capabilities optional — existing workflows must not break

---

## Gap Analysis

The following table summarizes the gaps between the SDD methodology (as described
in `spec-driven.md`) and ragent's current `/spec` implementation:

| #  | SDD Capability                                                     | ragent Status                                                                                                                   | Gap                                                                        |
| -- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| 1  | Separate`/specify` command creating a spec from a feature prompt | Partial —`/spec create` exists but auto-generates plan; no separate specify stage                                            | Missing separate specify stage with ambiguity marking                      |
| 2  | Separate`/plan` command generating plan from tech context        | Missing — plan generated at create time, Plan can be updated from edited SPEC.md file using /spec update or /spec add function | Present but uses update/change commands in /spec to build the PLAN.md file |
| 3  | Separate`/tasks` command generating task list from plan          | Missing —`/spec impl` executes but doesn't generate tasks.md, Contents of tasks.md is incorportade in PLAN.md                | Present but implemented differently                                        |
| 4  | `[NEEDS CLARIFICATION]` markers                                  | Missing                                                                                                                         | No mechanism to flag ambiguities in specs                                  |
| 5  | Quality checklists in templates                                    | Missing                                                                                                                         | No completeness, testability, or speculative-feature checks                |
| 6  | Constitution artifact (immutable articles)                         | Missing                                                                                                                         | No`CONSTITUTION.md` or architectural principles                          |
| 7  | Phase -1 pre-implementation gates                                  | Missing                                                                                                                         | No simplicity, anti-abstraction, or integration-first gates                |
| 8  | Branch-per-spec git workflow                                       | Missing                                                                                                                         | Specs not tied to git branches                                             |
| 9  | Research artifact alongside spec                                   | Partial —`/research` exists separately, but research can be linked using --from -research flag.                             | Implemented in a seperate but linkjable function                           |
| 10 | Data-model artifact                                                | Missing                                                                                                                         | No`data-model.md` generation                                             |
| 11 | Contracts artifact                                                 | Missing                                                                                                                         | No`contracts/` directory with API specs                                  |
| 12 | Quickstart validation scenarios                                    | Missing —`TESTPLAN.md` exists but not as validation scenarios                                                                | No`quickstart.md` artifact                                               |
| 13 | Test-first file creation ordering                                  | Missing                                                                                                                         | No contracts → tests → source enforcement                                |
| 14 | Ongoing consistency validation                                     | Partial — EARS syntax validation only                                                                                          | No ambiguity/contradiction/gap analysis                                    |
| 15 | Constitutional amendment process                                   | Missing                                                                                                                         | No versioned principles or amendment workflow                              |
| 16 | Production feedback loop                                           | Missing                                                                                                                         | No metrics/incidents → spec update path                                   |

---

## Requirements

### FR-001 — Separate Specify Stage

`The ragent spec system shall provide a "/spec specify <feature>" command that creates a SPEC.md with structured requirements, user stories, and acceptance criteria without simultaneously generating a PLAN.md.`

This separates the specification stage from the planning stage, matching
SDD's `/speckit.specify` workflow. The existing `/spec create` command may
delegate to this new command for the SPEC.md portion.

### FR-002 — Needs-Clarification Markers

`When a specification is created or edited, the ragent spec system shall detect and report [NEEDS CLARIFICATION: <question>] markers in the SPEC.md content.`

The system must not reject specs containing these markers but must surface
them during validation so the author knows which ambiguities remain
unresolved.

### FR-003 — Clarification Completeness Gate

`While a specification contains unresolved [NEEDS CLARIFICATION] markers, the ragent spec system shall prevent the spec from transitioning to the "approved" lifecycle state.`

This enforces SDD's principle that no `[NEEDS CLARIFICATION]` markers
remain before a spec is approved for implementation.

### FR-004 — Separate Plan Command

`When the "/spec plan <spec-id> <tech-context>" command is invoked, the ragent spec system shall generate (or regenerate) the PLAN.md from the existing SPEC.md using the provided technology context as guidance.`

This separates plan generation from spec creation, matching SDD's
`/speckit.plan` command. The technology context informs technology choices
and rationale documented in the plan.

### FR-005 — Separate Tasks Command

`When the "/spec tasks <spec-id>" command is invoked, the ragent spec system shall generate a TASKS.md file containing an ordered task list derived from the PLAN.md.`

This creates a standalone task-list artifact distinct from the PLAN.md
task table, matching SDD's `/speckit.tasks` command.

### FR-006 — Quality Checklists in Templates

`Where a new SPEC.md or PLAN.md is generated from a template, the ragent spec system shall embed quality checklists covering requirement completeness, testability, and absence of speculative features.`

Checklists act as self-review gates for the specification author and
for LLM-driven generation.

### FR-007 — Constitution Artifact

`The ragent spec system shall support a CONSTITUTION.md file in the specs root directory containing immutable architectural principles that govern generated implementations.`

The constitution defines articles (library-first, simplicity,
anti-abstraction, integration-first testing, etc.) that constrain
how plans and tasks are structured.

### FR-008 — Pre-Implementation Gates

`When a spec is transitioned to the "in_progress" state, the ragent spec system shall validate that the PLAN.md contains completed Phase -1 gates for Simplicity, Anti-Abstraction, and Integration-First principles.`

If any gate is unchecked, the transition is blocked with a diagnostic
listing the failed gates. The author may document justified exceptions
in a "Complexity Tracking" section.

### FR-009 — Branch-Per-Spec Workflow

`When a new spec is created, the ragent spec system shall offer to create a git branch named after the spec identifier so that spec work is isolated and version-controlled.`

This is optional — if the workspace is not a git repository or the user
declines, spec creation proceeds without branching.

### FR-010 — Research Artifact Integration

`Where a spec is created with a research context, the ragent spec system shall link the research output in the SPEC.md frontmatter and include a "## Related Research" section referencing the research artifact.`

ragent's existing `/research` command can produce research artifacts;
this requirement integrates them into the spec creation flow.

### FR-011 — Data-Model Artifact

`When the "/spec plan" command is invoked and the spec involves data entities, the ragent spec system shall generate a data-model.md file describing the domain data models.`

This artifact is optional — if no data entities are identified, the file
is not created.

### FR-012 — Contracts Artifact

`Where a spec defines API endpoints or inter-service contracts, the ragent spec system shall generate a contracts/ directory within the spec folder containing individual contract definition files.`

Each contract file documents request/response schemas, events, or
interface signatures.

### FR-013 — Quickstart Validation Scenarios

`When the "/spec tasks" command is invoked, the ragent spec system shall generate a quickstart.md file containing key validation scenarios derived from the spec's acceptance criteria.`

These scenarios serve as smoke-test validation steps distinct from the
full manual TESTPLAN.md.

### FR-014 — Test-First File Creation Order

`The ragent spec system shall document and enforce a file creation order in implementation plans: contracts first, then test files (contract, integration, e2e, unit), then source files.`

Enforcement is advisory during plan generation — the plan template
includes the ordering and the implementation runner warns if tasks
violate it.

### FR-015 — Consistency Validation

`While a specification is being validated, the ragent spec system shall analyze the SPEC.md for ambiguity, contradictions between requirements, and gaps where requirements lack acceptance criteria.`

This extends the existing EARS syntax validation with semantic-level
checks, producing warnings (not errors) for detected issues.

### FR-016 — Constitutional Amendment Process

`If the CONSTITUTION.md is modified, the ragent spec system shall require explicit rationale documentation, maintain backwards-compatibility assessment, and record the amendment in a dated changelog section within the file.`

This ensures principles evolve deliberately rather than silently.

### FR-017 — Production Feedback Loop

`When production metrics or incidents are associated with a spec, the ragent spec system shall allow annotating the spec with feedback notes that inform the next plan regeneration.`

This is an advisory mechanism — the notes are stored in a
`FEEDBACK.md` file within the spec directory and surfaced during
`/spec plan` regeneration.

### FR-018 — Backward Compatibility

`The ragent spec system shall preserve backward compatibility with existing spec directories that do not contain CONSTITUTION.md, TASKS.md, data-model.md, contracts/, or quickstart.md artifacts.`

Existing specs must continue to validate, list, search, and implement
without modification.

### FR-019 — Optional Capability Toggles

`Where a new SDD capability is added, the ragent spec system shall make the capability opt-in via configuration so that existing workflows are not disrupted.`

New artifacts and gates are generated only when the corresponding
configuration flag is enabled.

### FR-020 — Gap Documentation

`The ragent spec system shall document the SDD capability gaps and their resolution status in the spec's PLAN.md so that back-fill progress is trackable.`

This spec's own PLAN.md serves as the tracking artifact.

---

## Non-Functional Requirements

### NFR-001 — Performance

`The ragent spec system shall validate a spec with all new consistency checks in under 500 milliseconds for a spec containing up to 50 requirements.`

### NFR-002 — No Unsafe Code

`The ragent spec system shall contain no unsafe Rust code in any new or modified modules.`

### NFR-003 — Error Handling

`The ragent spec system shall use Result<T, SpecError> for all new file I/O and parsing operations, propagating errors with context using the "?" operator.`

### NFR-004 — Testability

`The ragent spec system shall include unit tests for all new parsing, validation, and generation logic in the ragent-specs crate's tests/ directory.`

### NFR-005 — Documentation

`The ragent spec system shall include DOCBLOCK documentation comments above all new public functions describing purpose, arguments, and return values.`

---

*End of Specification*

---
status: draft
audit:
  - { time: 1780835986, from: "none", to: "draft", actor: "system" }
---
# specadd — Incremental Spec and Plan Update

## Overview

This specification defines a new `/spec add` subcommand that incrementally
updates an existing spec's `SPEC.md` and `PLAN.md` with new requirements. Today,
the only way to extend a spec is to manually edit the files or recreate the
entire spec from scratch. The `add` subcommand automates this: it parses the
existing spec, determines the highest requirement and task numbers, generates
new requirements with correctly sequenced IDs, inserts them into the spec, and
updates the plan with corresponding new tasks while preserving all existing
content, numbering, and dependency relationships.

## Requirements

### Command Parsing

**FR-001** (Ubiquitous) The system shall provide a `SpecCommand::Add` variant
with fields `spec_id: String` and `feature: String`.

**FR-002** (Event-driven) When the user issues `/spec add <spec-id> <feature
description>`, the system shall parse the command into a `SpecCommand::Add`
with the spec ID and the remaining text as the feature description.

**FR-003** (Event-driven) When `/spec add` is invoked without both a spec ID
and a feature description, the system shall return a `SpecCommand::Unknown(
"add".to_string() )` so the TUI displays a usage error.

### Spec Loading and Validation

**FR-004** (Ubiquitous) The system shall load the existing spec identified by
`spec_id` using `SpecManager::read_spec()` before performing any modifications.

**FR-005** (Event-driven) When the specified spec ID does not exist, the
system shall return an error message indicating that the spec was not found,
without creating a new spec directory.

**FR-006** (State-driven) While the loaded spec has a status of `archived`,
the system shall reject the add operation and return an error indicating that
archived specs cannot be modified.

### Requirement Numbering

**FR-007** (Ubiquitous) The system shall scan the existing `SPEC.md` to
determine the highest-numbered functional requirement ID (matching the
pattern `FR-NNN`) and the highest-numbered non-functional requirement ID
(matching the pattern `NFR-NNN`).

**FR-008** (Event-driven) When the feature description contains non-functional
concerns (performance, security, scalability, reliability, etc.), the system
shall generate new requirements with `NFR-NNN` IDs; otherwise, the system shall
generate new requirements with `FR-NNN` IDs.

**FR-009** (Ubiquitous) The system shall number new requirements sequentially
starting from the next available number after the highest existing ID of the
same prefix. For example, if the spec contains FR-001 through FR-012, the
first new functional requirement shall be FR-013.

**FR-010** (Unwanted) The system shall not renumber or rename any existing
requirement IDs when adding new requirements.

### Spec Content Insertion

**FR-011** (Ubiquitous) The system shall insert new requirement blocks into the
`SPEC.md` content, preserving the existing frontmatter, title, overview,
section structure, and all existing requirement text unchanged.

**FR-012** (Event-driven) When the new requirements share an existing section
heading (e.g. both the existing spec and the new feature concern
"Configuration"), the system shall append the new requirement blocks under
that existing section heading.

**FR-013** (Event-driven) When the new requirements introduce a concern not
covered by any existing section heading, the system shall create a new `##`
section heading and place the new requirement blocks under it, positioned
before the "Non-Functional Requirements" section (or at the end of the file
if no NFR section exists).

**FR-014** (Unwanted) The system shall not modify, reorder, or delete any
existing requirement text, section headings, or frontmatter in the `SPEC.md`.

### Plan Task Generation

**FR-015** (Ubiquitous) The system shall scan the existing `PLAN.md` to
determine the highest-numbered task ID (matching the pattern `T-NNN`).

**FR-016** (Ubiquitous) The system shall generate new plan tasks, one per new
requirement (or grouped logically when multiple requirements share a single
implementation unit), numbered sequentially starting from the next available
task number.

**FR-017** (Ubiquitous) Each new task shall include the columns required by
the existing table format: ID, Title, Requirement, Effort, Priority, and
Dependencies.

**FR-018** (Ubiquitous) The system shall populate each new task's
"Requirement" column with the new requirement ID(s) it implements.

**FR-019** (Ubiquitous) The system shall populate each new task's
"Dependencies" column with references to any existing tasks that must be
completed first, as inferred from the requirement content (e.g. a new
requirement that extends an existing type depends on the task that defines
that type).

**FR-020** (Unwanted) The system shall not modify, reorder, or delete any
existing task rows in the `PLAN.md` task table.

### Plan Content Insertion

**FR-021** (Ubiquitous) The system shall insert new task rows into the
`## Tasks` table in `PLAN.md`, appending them after the last existing task
row.

**FR-022** (Event-driven) When the `PLAN.md` contains a `## Task Details`
section with per-task subsections (e.g. `### T-001 — Title`), the system
shall append new task detail subsections for each new task after the last
existing detail subsection, following the same format.

**FR-023** (Unwanted) The system shall not modify, reorder, or delete any
existing task detail subsections, estimated effort table, risk table, or
other content in the `PLAN.md` outside the `## Tasks` table and `## Task
Details` sections.

### Estimated Effort Update

**FR-024** (Optional) Where the `PLAN.md` contains an estimated effort
summary table (e.g. a table breaking down effort by priority), the system
shall update the summary numbers to include the new tasks' effort values.

**FR-025** (Optional) Where the `PLAN.md` contains a risk table, the system
shall append rows for any new risks introduced by the added requirements.

### Atomicity

**FR-026** (Event-driven) When the spec update (SPEC.md write) succeeds but
the plan update (PLAN.md write) fails, the system shall roll back the SPEC.md
to its original content so that the spec and plan remain consistent.

**FR-027** (Event-driven) When the plan update (PLAN.md write) succeeds but
the task detail section insertion fails, the system shall leave the PLAN.md
as-is (task table updated) and log a warning that the detail sections may be
incomplete.

### Validation After Update

**FR-028** (Event-driven) When the spec and plan have been successfully
updated and written to disk, the system shall run the EARS validator
(`validate::validate()`) on the updated spec and display the validation
report.

**FR-029** (State-driven) While the validation report contains errors
(severity `Error`), the system shall display a warning advising the user to
fix the errors before transitioning the spec status.

### LLM-Assisted Generation

**FR-030** (Ubiquitous) The system shall generate new requirements and tasks
by sending a prompt to the LLM agent that includes: the existing `SPEC.md`
content, the existing `PLAN.md` content, and the new feature description.

**FR-031** (Ubiquitous) The system shall instruct the LLM to produce only the
incremental additions (new requirement blocks and new task rows) rather than
rewriting the entire spec and plan.

**FR-032** (Event-driven) When the LLM returns generated content, the system
shall parse the output to extract new requirement blocks and new task rows,
then insert them into the existing documents using the insertion rules
(FR-011–FR-014, FR-021–FR-023).

**FR-033** (Unwanted) The system shall not replace the entire `SPEC.md` or
`PLAN.md` with LLM-generated content — only incremental insertions are
permitted.

### Status Bar and Help

**FR-034** (Ubiquitous) The system shall update the `/spec help` message to
include the `add` subcommand in the command reference table.

**FR-035** (Event-driven) When the `/spec add` command starts processing, the
system shall display a status message indicating that the spec is being
updated (e.g. `"spec: updating specs/<id>/SPEC.md + PLAN.md…"`).

**FR-036** (Event-driven) When the `/spec add` command completes successfully,
the system shall display a summary showing: the number of new requirements
added, the number of new tasks added, and the IDs of both.

### TUI Autocomplete

**FR-037** (Event-driven) When the user types `/spec ` in the input field, the
autocomplete menu shall include `add` as a suggested subcommand with the
description `"Incrementally add requirements to an existing spec and update its plan"`.

### Audit Trail

**FR-038** (Event-driven) When the `/spec add` operation completes
successfully, the system shall append an audit trail entry to the spec's
`audit_trail` recording the update with actor `"spec:add"` and the current
timestamp.

## Non-Functional Requirements

**NFR-001** (Ubiquitous) The add operation shall complete in under 60 seconds,
excluding the time taken by the LLM to generate the incremental content.

**NFR-002** (Ubiquitous) The parser shall correctly identify the highest
requirement and task IDs regardless of whether the IDs use zero-padded
numbers (e.g. `FR-001` vs `FR-1`) or sequential numbering with gaps (e.g.
`FR-001, FR-003, FR-007`).

**NFR-003** (Ubiquitous) The add operation shall preserve the original YAML
frontmatter of `SPEC.md` unchanged, including the `status` field.

**NFR-004** (Ubiquitous) Existing tasks whose status is `completed` shall not
have their status changed by the add operation.

## Out of Scope

- Removing or modifying existing requirements (use manual editing).
- Renumbering all requirements to be contiguous after an add.
- Merging duplicate requirements automatically.
- Updating the `CHANGELOG.md` or `SPEC.pdf` as part of the add operation.
- Multi-user conflict resolution (concurrent edits to the same spec).
- Undo/rollback of an add operation beyond the atomicity guarantees in
  FR-026.
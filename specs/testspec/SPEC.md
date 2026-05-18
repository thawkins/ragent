# Specification: ragent Spec Management System

## Executive Summary

This document specifies a **Spec Management System** for ragent that enables users to create, organise, track, and enforce software specifications alongside code. The system provides a structured workflow for writing specifications in EARS notation, linking them to implementation plans, tracking compliance status, and integrating with ragent's existing tool ecosystem. It ensures that every significant feature or change is preceded by a clear, reviewable specification that lives in version control.

## Scope & Objectives

### Scope

The Spec Management System covers:
- **Spec lifecycle**: creation, editing, review, approval, implementation, and archival
- **Directory conventions**: standardised folder structure for specs
- **Template system**: EARS-notation templates and boilerplate generation
- **Validation**: automated checks for spec completeness and EARS syntax
- **Status tracking**: state machine for spec lifecycle (draft → review → approved → implemented → verified → archived)
- **Plan linkage**: mandatory PLAN.md pairing with every SPEC.md
- **Tool integration**: slash commands and programmatic APIs for spec operations
- **Reporting**: listing, filtering, and summarising spec status across a project

### Out of Scope
- Real-time collaborative editing (concurrent file editing is handled by VCS)
- Visual diagram editing (specs are text/markdown only)
- External spec repository hosting (specs live inside the project repo)

### Objectives

1. Reduce specification ambiguity by enforcing EARS notation
2. Ensure every spec has a corresponding implementation plan
3. Provide visibility into which specs are implemented, in progress, or pending
4. Integrate spec authoring into the developer workflow without friction
5. Enable programmatic access so agents and CI can validate spec compliance

---

## Functional Requirements

### FR-001 — Spec Directory Structure

The ragent Spec Management System shall enforce a standard directory layout for specifications under `specs/`.

`The <ragent Spec Management System> shall <enforce that every spec resides in a subdirectory of specs/ named after the spec identifier, containing SPEC.md and PLAN.md>.`

`When <a spec is created>, the <ragent Spec Management System> shall <generate the directory specs/<spec-id>/ and place both SPEC.md and PLAN.md inside it>.`

### FR-002 — EARS Template Generation

`The <ragent Spec Management System> shall <provide a CLI and TUI command to generate a new spec from an EARS-aware template>.`

`When <the user runs "/spec create <spec-id>" or the equivalent CLI command>, the <ragent Spec Management System> shall <create a new spec directory and populate SPEC.md with the EARS template sections and numbered requirement placeholders>.`

### FR-003 — EARS Syntax Validation

`The <ragent Spec Management System> shall <validate that every requirement in a SPEC.md conforms to one of the five EARS templates>.`

`While <a spec is in Draft or Review status>, the <ragent Spec Management System> shall <allow incomplete EARS requirements but emit warnings during validation>.`

`If <a spec is submitted for approval and contains requirements that do not match any EARS template>, the <ragent Spec Management System> shall <reject the submission and report the offending lines>.`

### FR-004 — Mandatory Plan Pairing

`The <ragent Spec Management System> shall <require that every SPEC.md has a sibling PLAN.md in the same directory>.`

`If <a SPEC.md exists without a corresponding PLAN.md>, the <ragent Spec Management System> shall <treat the spec as incomplete and block approval>.`

`When <a spec is created>, the <ragent Spec Management System> shall <generate a PLAN.md alongside SPEC.md containing the standard implementation plan template>.`

### FR-005 — Spec Status Tracking

`The <ragent Spec Management System> shall <maintain a status field for each spec through its lifecycle>.`

`When <a spec is created>, the <ragent Spec Management System> shall <set its status to Draft>.`

`The <ragent Spec Management System> shall <support the following status values: Draft, In Review, Approved, In Progress, Implemented, Verified, Archived>.`

`While <a spec is in Approved status>, the <ragent Spec Management System> shall <allow its PLAN.md to be executed and its requirements to be implemented>.`

`If <a user attempts to transition a spec to Implemented without all linked tasks being completed>, the <ragent Spec Management System> shall <reject the transition and report the open tasks>.`

### FR-006 — Status Transition Rules

`The <ragent Spec Management System> shall <enforce valid status transitions according to a state machine>.`

`If <an invalid status transition is requested>, the <ragent Spec Management System> shall <reject the request and display the allowed next states>.`

Allowed transitions:
- Draft → In Review
- In Review → Draft (with feedback)
- In Review → Approved
- Approved → In Progress
- In Progress → Implemented
- Implemented → Verified
- Verified → Archived
- Any → Draft (reopen)

### FR-007 — Spec Listing and Filtering

`The <ragent Spec Management System> shall <provide a command to list all specs with their status, title, and last modified date>.`

`When <the user runs "/spec list" or the equivalent CLI command>, the <ragent Spec Management System> shall <display a tabular summary of all specs sorted by status and modification time>.`

`Where <filter arguments are provided>, the <ragent Spec Management System> shall <filter the list by status, spec ID prefix, or date range>.`

### FR-008 — Spec Search

`The <ragent Spec Management System> shall <support full-text search across all SPEC.md, PLAN.md, and REVIEW.md files>.`

`When <the user runs "/spec search <query>" or the equivalent CLI command>, the <ragent Spec Management System> shall <return matching specs with context snippets and relevance ranking>.`

`If <a spec is Archived>, the <ragent Spec Management System> shall <exclude it from search results unless the user explicitly requests archived specs via the include_archived flag>.`

### FR-009 — Requirement Traceability

`The <ragent Spec Management System> shall <track the implementation state of each requirement within a spec>.`

`Where <a PLAN.md task is linked to a requirement ID>, the <ragent Spec Management System> shall <update the requirement's implementation status when the task is completed>.`

`When <a spec is queried for its implementation coverage>, the <ragent Spec Management System> shall <report the percentage of requirements with completed linked tasks>.`

### FR-010 — Spec Review Workflow

`The <ragent Spec Management System> shall <support a review workflow where specs can be assigned reviewers via YAML frontmatter (reviewers: [list]) and review comments recorded>.`

`While <a spec is In Review>, the <ragent Spec Management System> shall <accept review comments stored in a REVIEW.md file co-located with the spec in specs/<spec-id>/REVIEW.md>.`

`When <a reviewer marks a spec as Approved>, the <ragent Spec Management System> shall <transition the spec to Approved status and record the reviewer's identity and timestamp in the audit trail>.`

`The <ragent Spec Management System> shall <parse reviewers: from SPEC.md YAML frontmatter on read and persist them on write>.`

### FR-011 — Spec Versioning

`The <ragent Spec Management System> shall <record the version of each spec and append an audit trail entry on every status change>.`

`When <a spec is modified after approval>, the <ragent Spec Management System> shall <require a version bump and reset status to Draft or In Review>.`

`The <ragent Spec Management System> shall <store version history as Git commits, relying on the repository for full versioning>.`

### FR-012 — Compliance Validation

`The <ragent Spec Management System> shall <provide a validation command that checks all specs for structural compliance>.`

`When <the user runs "/spec validate" or the equivalent CLI command>, the <ragent Spec Management System> shall <scan all specs and report violations such as missing PLAN.md, invalid EARS syntax, unknown status values, or orphaned requirements>.`

### FR-013 — Integration with Agent Workflow

`Where <ragent is operating in agent mode>, the <ragent Spec Management System> shall <allow an agent to read the spec before implementation and update task status in PLAN.md as work progresses via the spec_read, spec_search, spec_list, spec_task_update, and spec_coverage tools>.`

`When <an agent completes a file-write tool (write, edit, multiedit, patch, create, append_to_file)>, the <ragent Spec Management System> shall <auto-complete all in_progress tasks on the active spec when exactly one in_progress task exists>.`

`When <the user runs "/spec activate <spec-id>">, the <ragent Spec Management System> shall <inject the spec's requirements and tasks into the agent's system prompt for context-aware implementation>.`

`When <the user runs "/spec deactivate">, the <ragent Spec Management System> shall <remove spec context from subsequent agent prompts>.`

### FR-014 — Spec Archival

`The <ragent Spec Management System> shall <support archiving completed specs via the Verified → Archived status transition to reduce noise in active listings>.`

`When <a spec is transitioned to Archived status>, the <ragent Spec Management System> shall <exclude it from default list and search results but retain it for historical reference (files remain on disk)>.`

`If <the user explicitly requests archived specs via the SpecFilter.with_archived() or search_specs_with_archived() API>, the <ragent Spec Management System> shall <include them in list and search output>.`

### FR-015 — Plan Task Management

`The <ragent Spec Management System> shall <allow tasks in PLAN.md to be created, updated, and marked complete via commands>.`

`When <the user runs "/spec task add <spec-id> <task-title>" or the equivalent CLI command>, the <ragent Spec Management System> shall <append a new task to the PLAN.md with a unique task ID and link it to the specified requirement if provided>.`

`When <a task is marked complete>, the <ragent Spec Management System> shall <record the completion timestamp and update the spec's implementation coverage>.`

---

## Non-Functional Requirements

### NFR-001 — Performance

`The <ragent Spec Management System> shall <list all specs in a project containing up to 1,000 specs within 500 milliseconds>.`

`The <ragent Spec Management System> shall <perform a full-text search across all specs within 2 seconds for projects with up to 1,000 specs>.`

### NFR-002 — Scalability

`The <ragent Spec Management System> shall <support projects with up to 10,000 specs without degradation in listing or search performance>.`

`Where <a project exceeds 10,000 specs>, the <ragent Spec Management System> shall <recommend archiving old specs and using spec ID prefixes for filtering>.`

### NFR-003 — Reliability

`The <ragent Spec Management System> shall <not corrupt or lose spec data during status transitions or editing operations>.`

`If <a write operation fails>, the <ragent Spec Management System> shall <leave the original files unchanged and report the error>.`

### NFR-004 — Usability

`The <ragent Spec Management System> shall <provide both TUI slash commands ("/spec ...") and CLI subcommands for all operations>.`

`The <ragent Spec Management System> shall <display clear error messages that include the spec ID, file path, and corrective action when validation fails>.`

### NFR-005 — Maintainability

`The <ragent Spec Management System> shall <be implemented as a module within ragent-agent or a dedicated ragent-specs crate with no circular dependencies>.`

`The <ragent Spec Management System> shall <use plain Markdown files for storage so specs remain readable without ragent>.`

### NFR-006 — Security

`The <ragent Spec Management System> shall <respect the project's file permission system and require appropriate permissions for spec creation and modification>.`

`If <the user lacks file:write permission for the specs/ directory>, the <ragent Spec Management System> shall <request permission before creating or modifying spec files>.`

### NFR-007 — Portability

`The <ragent Spec Management System> shall <work identically across all platforms supported by ragent (Linux, macOS, Windows)>.`

---

## Constraints & Assumptions

### Constraints

1. **Storage format**: Specs must be stored as plain Markdown files in the repository; no external database is permitted for spec content.
2. **Version control**: Spec versioning relies entirely on Git; no separate versioning mechanism is required.
3. **EARS notation**: All functional requirements must use one of the five approved EARS templates.
4. **Directory location**: All specs must reside under the `specs/` directory at the project root.
5. **File naming**: Each spec directory must contain files named exactly `SPEC.md` and `PLAN.md` (case-sensitive).

### Assumptions

1. Users have basic familiarity with Markdown and Git workflows.
2. Projects using the Spec Management System already have a `specs/` directory or are willing to create one.
3. Reviewers have read access to the repository and can open pull requests or commit review comments.
4. Agents implementing specs have access to the same filesystem as the spec files.

---

## Interfaces & Dependencies

### Internal Interfaces

| Component | Interface | Purpose |
|-----------|-----------|---------|
| `ragent-tui` | Slash commands `/spec create`, `/spec list`, `/spec search`, `/spec validate`, `/spec status`, `/spec task`, `/spec activate`, `/spec deactivate`, `/spec coverage` | User-facing TUI commands |
| `ragent-agent` | Agent tools: `spec_read`, `spec_list`, `spec_search`, `spec_task_update`, `spec_coverage` | Agent-driven spec operations |
| `ragent-agent` | Session API: `SessionProcessor.active_spec`, `SessionProcessor.spec_manager` | Active spec context injection |
| `ragent-specs` | `SpecManager`, `SpecIo`, `SpecFilter`, `SpecSearchResult` | Core spec lifecycle, discovery, search |
| `ragent-specs` | `validate::validate()`, `Report`, `Issue`, `Severity` | EARS compliance validation |

### External Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| `regex` | ^1.10 | EARS template validation |
| `walkdir` | ^2.5 | Directory traversal for spec discovery |
| `pulldown-cmark` | ^0.12 | Markdown parsing for requirement extraction |
| `chrono` | ^0.4 | Timestamp recording in audit entries |

### Dependencies on Existing ragent Crates

- `ragent-types` — for `SpecId`, `SpecStatus` enums, and event types
- `ragent-config` — for reading `specs.*` configuration keys
- `ragent-storage` — for SQLite-based spec index (optional, for performance)
- `ragent-tui` — for slash command registration and UI rendering
- `ragent-agent` — for agent workflow integration

---

## Glossary

| Term | Definition |
|------|------------|
| **EARS** | Easy Approach to Requirements Syntax — a constrained natural language for writing requirements using five templates: Ubiquitous, Event-driven, State-driven, Optional, and Unwanted behaviour. |
| **SPEC.md** | The primary specification document for a feature, containing EARS-formatted requirements. |
| **PLAN.md** | The implementation plan document paired with a SPEC.md, containing milestones, tasks, and risk analysis. |
| **Spec ID** | A unique, URL-safe identifier for a spec, used as the directory name (e.g., `spec-mgt-v1`). |
| **Spec Status** | The lifecycle state of a spec: Draft, In Review, Approved, In Progress, Implemented, Verified, or Archived. |
| **Requirement ID** | A unique identifier within a spec for a single requirement (e.g., `FR-007`, `NFR-003`). |
| **Task ID** | A unique identifier within a PLAN.md for an implementation task (e.g., `T-003`). |
| **Validation** | The process of checking a spec for structural compliance, EARS syntax correctness, and completeness. |
| **Traceability** | The ability to link requirements in SPEC.md to tasks in PLAN.md and track implementation progress. |
| **Audit Trail** | A chronological record of status transitions and modifications appended to a spec file. |

---

*End of Specification*

---
status: draft
audit:
  - { time: 1786474278, from: "none", to: "draft", actor: "system" }
---
# Specification: `/spec update` Subcommand — Regenerate PLAN.md and TESTPLAN.md from Edited SPEC.md

## Overview

The `/spec` slash command family provides a set of subcommands for managing
specifications within ragent. Users can create specs (`/spec create`), add
incremental requirements (`/spec add`), validate (`/spec validate`), list,
search, manage status, manage tasks, implement, and perform JTBD analysis.

However, when a user manually edits the `SPEC.md` file — adding, removing, or
modifying requirements — there is no way to regenerate the downstream `PLAN.md`
and `TESTPLAN.md` files to reflect those changes. The only workaround is to
manually re-run `/spec create` (which fails because the directory already
exists) or to manually edit `PLAN.md` and `TESTPLAN.md` by hand, which is
error-prone and tedious.

This specification defines a new `/spec update [specid]` subcommand that re-reads
the existing `SPEC.md` file and regenerates `PLAN.md` and `TESTPLAN.md` based on
the current contents of the spec. This allows the user to edit `SPEC.md`
freely and then regenerate the plan and test plan to incorporate the changes.

## Scope & Objectives

### Scope

**In scope:**

- A new `Update` variant in the `SpecCommand` enum in
  `crates/ragent-specs/src/commands.rs`.
- Parsing of `/spec update <spec-id>` from the slash command argument string.
- Validation that the spec directory and `SPEC.md` file exist.
- Guard against updating archived specs.
- Delegation to an LLM agent (default: explore) to regenerate `PLAN.md` and
  `TESTPLAN.md` from the current `SPEC.md` content.
- User-facing status messages, assistant messages, and log entries consistent
  with existing subcommands (`/spec create`, `/spec add`, `/spec jtbd`).
- Inclusion of the `update` subcommand in the `/spec help` reference table.
- Inclusion of `update` in the `is_usage_error()` guard list.
- TUI dispatch handling in `crates/ragent-tui/src/app/slash.rs`.

**Out of scope:**

- Automatic detection of SPEC.md changes via file watching.
- Merging or diffing old `PLAN.md` / `TESTPLAN.md` content with new content
  (the files are fully regenerated).
- Regeneration of `JTBD.md` or `REVIEW.md`.
- Changes to the `SpecManager` or `SpecIo` core data structures.
- HTTP API endpoints for spec update (TUI slash command only).

### Objectives

1. Allow users to edit `SPEC.md` and regenerate `PLAN.md` and `TESTPLAN.md`
   with a single command.
2. Maintain consistency with existing `/spec` subcommand patterns for parsing,
  validation, messaging, and LLM delegation.
3. Preserve the spec's existing status and frontmatter (only `PLAN.md` and
   `TESTPLAN.md` are regenerated; `SPEC.md` is read-only).

---

## Requirements

### FR-001 — Update subcommand parsing (ubiquitous)

`The spec command parser shall recognise "update" as a valid subcommand and parse the spec ID from the remaining argument string.`

The `SpecCommand::parse` method in `crates/ragent-specs/src/commands.rs` shall
match `"update"` in its `match` block and produce an `Update { spec_id }`
variant. The spec ID is the first whitespace-delimited token after `"update"`.
If the spec ID is empty, the parser shall return `Unknown("update")` so the
caller can surface a usage error.

### FR-002 — Update variant in SpecCommand enum (ubiquitous)

`The SpecCommand enum shall include an Update variant carrying the spec identifier.`

A new `Update { spec_id: String }` variant shall be added to the `SpecCommand`
enum in `crates/ragent-specs/src/commands.rs`.

### FR-003 — Usage error detection for update (state-driven)

`While the parsed subcommand is Unknown("update"), the is_usage_error method shall return true so the TUI can display a usage hint.`

The `is_usage_error()` method shall include `"update"` in its guard list
alongside the other subcommand names.

### FR-004 — Help message entry for update (ubiquitous)

`The spec help reference table shall include a row for the update subcommand describing its arguments and purpose.`

The `build_help_message()` constant shall include a new table row:
`| /spec update <spec-id> | required spec-id | Re-read the existing SPEC.md and regenerate PLAN.md and TESTPLAN.md from its current content. |`

### FR-005 — Spec existence validation (event-driven)

`When the user invokes /spec update <spec-id>, the system shall validate that the spec directory exists and contains a readable SPEC.md file before proceeding with regeneration.`

The TUI dispatch handler shall:
1. Validate the spec ID format via `SpecId::new()`.
2. Use `SpecManager::read_spec()` to load the existing spec.
3. If the spec is not found, display an error message listing available specs
   (consistent with `/spec impl` error handling).
4. If the spec's status is `Archived`, refuse the update with an error message.

### FR-006 — Archived spec guard (state-driven)

`While a spec is in the Archived status, the update subcommand shall refuse to regenerate its plan and test plan.`

If `spec.status == SpecStatus::Archived`, the system shall return an error
message: `"spec: '<spec-id>' is archived and cannot be modified"`.

### FR-007 — LLM agent delegation for regeneration (event-driven)

`When the spec passes validation, the system shall delegate to an LLM agent to regenerate PLAN.md and TESTPLAN.md from the current SPEC.md content.`

The TUI dispatch handler shall:
1. Select the `explore` agent (falling back to the current agent if explore is
   not available), consistent with `/spec create` and `/spec add`.
2. Apply the selected model and thinking configuration.
3. Set the agent's permissions to default permissions.
4. Build a prompt using `SpecCommand::build_update_prompt()` that instructs the
   agent to re-read `SPEC.md` and rewrite `PLAN.md` and `TESTPLAN.md`.
5. Push the prompt as a user message and spawn the session processor.

### FR-008 — Update prompt builder (ubiquitous)

`The spec command module shall provide a build_update_prompt function that constructs the LLM prompt for regenerating PLAN.md and TESTPLAN.md from the existing SPEC.md.`

The prompt shall:
- Instruct the agent to read `specs/<spec-id>/SPEC.md`.
- Regenerate `specs/<spec-id>/PLAN.md` with a `## Tasks` section containing a
  markdown table with columns: ID, Title, Requirement, Effort, Priority,
  Dependencies; task IDs as T-001, T-002, etc.; effort values S/M/L; priority
  values Critical/High/Medium/Low.
- Regenerate `specs/<spec-id>/TESTPLAN.md` as a manual test plan with YAML
  frontmatter (`status: draft`), a `## Test Cases` section, and manual test
  case entries (TC-001, TC-002, …) with preconditions, steps, test data, and
  expected results.
- NOT modify the `SPEC.md` file.
- Use the `write` tool to overwrite `PLAN.md` and `TESTPLAN.md`.

### FR-009 — User-facing status message (ubiquitous)

`The spec command module shall provide build_update_status, build_update_message, and build_update_log helper functions consistent with the existing subcommand helpers.`

- `build_update_status(spec_id)` returns `"spec: updating specs/<spec-id>/PLAN.md + specs/<spec-id>/TESTPLAN.md…"`.
- `build_update_message(spec_id)` returns an assistant-facing message explaining
  that the plan and test plan are being regenerated from the current SPEC.md.
- `build_update_log(spec_id)` returns a log entry string for the info log.

### FR-010 — TUI dispatch handler (event-driven)

`When the TUI receives a SpecCommand::Update variant from the slash command dispatcher, it shall execute the validation, agent selection, prompt building, and LLM spawning sequence.`

The handler in `crates/ragent-tui/src/app/slash.rs` shall:
1. Display the update message via `append_assistant_text`.
2. Push the log entry via `push_log_no_agent`.
3. Resolve the working directory and specs root.
4. Validate the spec ID, read the spec, and check for archived status.
5. Select the explore agent (fallback to current agent).
6. Build the prompt via `build_update_prompt`.
7. Push the prompt as a user message.
8. Spawn the session processor.
9. Set the status bar to `build_update_status`.
10. On error, set the status and append an error message.

### FR-011 — Existing task status preservation (optional)

`Where the existing PLAN.md contains tasks with completed or in-progress statuses, the regenerated PLAN.md should preserve those task statuses for tasks whose IDs remain valid.`

The prompt in FR-008 shall instruct the agent to read the existing `PLAN.md`
content and, for tasks whose IDs (T-001, T-002, …) remain in the regenerated
plan, carry over the existing status values. This is a best-effort
instruction — the LLM is responsible for honouring it.

### FR-012 — Unknown subcommand handling for missing spec ID (unwanted)

`If the user invokes /spec update without providing a spec ID, the system shall not spawn an LLM task and shall display a usage error message.`

The parser shall return `Unknown("update")` and the TUI shall display
`"Usage: /spec update — try /spec help"` via the existing `Unknown` handler
branch.

### FR-013 — Invalid spec ID format handling (unwanted)

`If the provided spec ID fails SpecId::new validation, the system shall display an error message and not proceed with regeneration.`

The TUI handler shall set the status to `"spec: invalid spec ID: <id>"` and
append an error message explaining the allowed character set (alphanumeric,
hyphens, underscores only).

---

## Non-Functional Requirements

### NFR-001 — Consistency with existing subcommands

The new subcommand shall follow the same code patterns, helper function naming
conventions, and TUI dispatch structure as `/spec add` and `/spec jtbd`.

### NFR-002 — No new external dependencies

The implementation shall not introduce new crate dependencies. It reuses the
existing `ragent-specs` and `ragent-tui` infrastructure.

### NFR-003 — Testability

The parser, usage error detection, and helper functions shall be unit-testable
without requiring a running LLM, following the pattern in
`crates/ragent-specs/tests/test_slash_spec.rs`.

### NFR-004 — Documentation

The `/spec help` output and any relevant documentation shall mention the new
`update` subcommand.
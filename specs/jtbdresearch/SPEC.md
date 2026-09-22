---
status: draft
audit:
  - { time: 1786415857, from: "none", to: "draft", actor: "system" }
---
# Specification: `/spec jtbd` — JTBD Analysis Slash Command

## Overview

The `/spec` slash-command family (parsed in `ragent-specs/src/commands.rs` and
dispatched in `crates/ragent-tui/src/app/slash.rs`) currently supports
`create`, `list`, `search`, `validate`, `status`, `task`, and `help`. This
specification adds a new sub-command:

```
/spec jtbd <specname>
```

which performs a **Jobs-To-Be-Done (JTBD) analysis** of an existing spec's
`SPEC.md` and writes the result to a new file `JTBD.md` in the same spec
folder (`specs/<specname>/JTBD.md`).

The JTBD analysis extracts, from the spec's overview and numbered requirements,
the underlying "jobs" the feature is hired to do:

- **Job statement** — "When \<situation\>, I want to \<motivation\>, so I can
  \<expected outcome\>."
- **Functional job** — the practical task or goal.
- **Emotional job** — how the user wants to feel or avoid feeling.
- **Social job** — how the user wants to be perceived by others.
- **Related requirements** — the `FR-NNN` / `NFR-NNN` identifiers in
  `SPEC.md` that trace to each job.
- **Success signals** — observable evidence that the job is being done well.

The analysis is performed by the **explore agent** (falling back to the
currently selected agent) via the session processor — the same pattern used by
`/spec create`. The sub-command itself performs argument validation, spec
discovery, and prompt construction deterministically in Rust.

## Background

### Existing `/spec` infrastructure

- `crates/ragent-specs/src/commands.rs` defines the `SpecCommand` enum with a
  `parse(args: &str) -> Self` method and builder helpers such as
  `build_create_prompt`, `build_create_message`, `build_create_status`, and
  `build_help_message`.
- `crates/ragent-tui/src/app/slash.rs` handles the `"spec"` arm (two sites:
  argument-hint generation around line 107 and execution around line 4767).
  Execution resolves the spec root as `<working_dir>/specs`, constructs a
  `SpecManager`, and either performs the work inline (validate/status/list) or
  spawns the explore/coder agent with a constructed prompt (create).
- `crates/ragent-specs/src/manager.rs` and `io.rs` provide `SpecManager`,
  `SpecId` validation, spec discovery, and template helpers.
- `crates/ragent-specs/src/validate.rs` already parses requirements out of a
  `SPEC.md` (`parse_requirements`), producing IDs, titles, and EARS kinds.

### JTBD framework

Jobs-To-Be-Done is a product-analysis framework that reframes features as
"jobs" a user hires the product to accomplish. A typical JTBD output contains:

| Section                 | Content                                                        |
| ----------------------- | -------------------------------------------------------------- |
| Job statement           | Situation / motivation / expected outcome                      |
| Job type                | Functional, emotional, or social                                |
| Performer               | Who is hiring the product (the spec's primary user)            |
| Related requirements    | FR/NFR identifiers from the source spec                         |
| Success signals         | Observable indicators the job is fulfilled                      |
| Out-of-scope jobs       | Jobs explicitly rejected or deferred                            |

Persisting the analysis inside the spec folder keeps it versioned alongside
`SPEC.md` and `PLAN.md` and discoverable by `spec_list` / `spec_search`.

## Requirements

### FR-001 — New `Jtbd` variant on `SpecCommand` (ubiquitous)

The `SpecCommand` enum **shall** include a `Jtbd { spec_id: String, force: bool }`
variant parsed from the argument string `jtbd <specname>` (with optional
`--force` flag), and `SpecCommand::parse` **shall** route that string to the
variant.

> *Ubiquitous requirement — applies every time the slash command is parsed.*

### FR-002 — `/spec jtbd <specname>` triggers JTBD analysis (event-driven)

**When** the user invokes `/spec jtbd <specname>` in the TUI, the system
**shall** validate the spec name, confirm the target `SPEC.md` exists, and
dispatch an LLM task to the explore agent (falling back to the currently
selected agent) with a prompt that instructs it to read `specs/<specname>/SPEC.md`
and write `specs/<specname>/JTBD.md` containing the JTBD analysis sections listed
in the Overview.

### FR-003 — Existing JTBD.md is preserved by default (state-driven)

**While** `specs/<specname>/JTBD.md` already exists **and** the user did not pass
`--force`, the system **shall** refuse to regenerate the file, surface a status
message indicating the file exists, and instruct the user to re-run with
`--force` to overwrite.

### FR-004 — `--force` overwrites existing JTBD.md (state-driven)

**While** `--force` is present on the command line, the system **shall** allow
regeneration and overwrite `specs/<specname>/JTBD.md` atomically.

### FR-005 — Optional `--agent <name>` override (optional)

**Where** the user supplies `--agent <name>` after the spec name, the system
**shall** dispatch the analysis to that named agent instead of the default
explore agent; if the named agent is not found the system **shall** surface an
error and **shall not** spawn any task.

### FR-006 — Job statements follow JTBD grammar (ubiquitous)

The generated `JTBD.md` **shall** express every identified job using the grammar
*"When \<situation\>, I want to \<motivation\>, so I can \<expected outcome\>"*
and **shall** classify each job as functional, emotional, or social.

### FR-007 — Traceability to requirement IDs (ubiquitous)

Every job recorded in `JTBD.md` **shall** list the `FR-NNN` and/or `NFR-NNN`
identifiers from the source `SPEC.md` that the job traces to; jobs with no
traceable requirement **shall** be marked explicitly as *untraced* so gaps in
coverage are visible.

### FR-008 — Unknown or missing spec name is rejected (unwanted)

**If** the user invokes `/spec jtbd` with a `<specname>` that does not exist,
is not a valid `SpecId`, or has no `SPEC.md`, **then** the system **shall**
display an actionable error (naming the missing path) and **shall not** create
any file or spawn any agent task.

### FR-009 — Missing or unreadable SPEC.md is rejected (unwanted)

**If** the target `SPEC.md` exists but cannot be read or is empty, **then**
the system **shall** abort the analysis with an error status and **shall not**
write a `JTBD.md`.

### FR-010 — Slash-command help and hint updated (ubiquitous)

`SpecCommand::build_help_message` **shall** include a row documenting
`/spec jtbd [specname] [--force] [--agent <name>]`, and the argument-hint
table in `crates/ragent-tui/src/app/session_ops.rs` **shall** advertise the
`jtbd` sub-command.

### FR-011 — Status and logging parity with `/spec create` (event-driven)

**When** a JTBD analysis is dispatched, the system **shall** set the TUI status
line to `"spec jtbd: <specname>"`, append a user-visible assistant message
summarising the request, and emit an `Info`-level log entry — matching the
observability behaviour of `/spec create`.

### FR-012 — No network calls outside the agent task (ubiquitous)

The dispatch path (parse, validation, prompt construction) **shall** perform no
network I/O; the only network access occurs inside the spawned agent task,
identical to `/spec create`.

### FR-013 — Idempotent re-runs produce valid markdown (state-driven)

**When** `/spec jtbd <specname>` is re-run with `--force`, the system **shall**
replace `JTBD.md` with a fresh, well-formed markdown document containing the
sections listed in the Overview — partial or malformed prior contents **shall**
not be merged in.

### FR-014 — Cancellation honoured (unwanted)

**If** the user cancels the in-flight JTBD task (Esc / cancel flag), **then**
the agent task **shall** terminate via the shared cancellation flag and the
system **shall not** leave a partially-written `JTBD.md`; any file created
before cancellation **shall** be removed or left in a detectably incomplete
state (e.g. an explicit `<!-- incomplete -->` marker).

## Non-functional Requirements

### NFR-001 — Performance

The dispatch path (FR-001 through FR-005) **shall** complete within 50 ms of
key-press on a warmed cache; spec discovery is a single directory read.

### NFR-002 — Testability

The parsing behaviour in `SpecCommand::parse` (FR-001, FR-003, FR-004, FR-005,
FR-008) **shall** be unit-testable without a running TUI; the dispatch path
**shall** be covered by an integration test in `crates/ragent-tui/tests/`
following the existing `test_slash_spec_validate_all` pattern.

### NFR-003 — Documentation

The `/spec` help text, the user-facing `docs/teams.md`-style how-to (if one is
added for specs), and `QUICKSTART.md` **shall** be updated to mention the new
sub-command.

### NFR-004 — Backwards compatibility

All existing `/spec` sub-commands (`create`, `list`, `search`, `validate`,
`status`, `task`, `help`) **shall** continue to parse and execute identically;
no existing variant signature **shall** change.

## Scope

### In scope

- New `SpecCommand::Jtbd` variant and parser changes in `ragent-specs`.
- TUI dispatch in `crates/ragent-tui/src/app/slash.rs` (mirroring `Create`).
- Prompt builder (`build_jtbd_prompt`), status/message/log builders, and help-text update in `ragent-specs/src/commands.rs`.
- Argument-hint update in `crates/ragent-tui/src/app/session_ops.rs`.
- Unit tests for parsing; integration test for dispatch and error handling.
- `JTBD.md` output written into the target spec folder.

### Out of scope

- JTBD analysis of `PLAN.md` or any file other than `SPEC.md`.
- Automatic regeneration of `JTBD.md` when `SPEC.md` changes (no watcher).
- Server/CLI (`ragent run`) equivalents of the sub-command (a follow-up spec
  may add `ragent spec jtbd <name>`).
- Multi-spec batch JTBD (`/spec jtbd --all`).

## Risks and Assumptions

- **Assumption:** The explore agent is always available in
  `cycleable_agents`; the fallback path (current agent) matches `/spec create`.
- **Assumption:** LLM-authored JTBD output is acceptable quality without a
  dedicated template enforcement pass; validators only check markdown
  well-formedness, not content accuracy.
- **Risk:** A very large `SPEC.md` could exceed the explore agent's context;
  mitigation is to instruct the agent to read the file in sections using the
  existing `read` tool conventions.
- **Risk:** Users may expect `JTBD.md` to be re-generated automatically on
  spec edits; out of scope here and documented as such.

## Related Work

- `/spec create` — the model for how this sub-command dispatches an agent task.
- `/research` — another example of a TUI command that spawns an agent and
  writes artifacts to disk.
- `specs/repscore/SPEC.md` — style template for this document.

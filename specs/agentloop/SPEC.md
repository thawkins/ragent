---
status: draft
audit:
  - { time: 1788748243, from: "none", to: "draft", actor: "system" }
---
# Specification: Agentic Loop Mechanism (`/loop`) — Goal-Driven Plan-Act-Observe with Four Stop Conditions

## Executive Summary

This specification defines ragent's **agentic loop mechanism**: a goal-driven
**plan-act-observe** execution cycle, exposed to users as a **`/loop` slash
command** for selecting an agent and setting a goal, and bounded by **four
stop conditions** — the goal is achieved, the agent hits an unrecoverable
error, a step or cost budget is exhausted, or a human intervenes.

The loop is what turns the language model from a chatbot into a system that
performs work. Its decisive ingredient is **feedback**: each iteration feeds
the observation of the previous action back into the model's context,
allowing it to self-correct. Autonomy is bounded by design — goals, budgets,
and intervention triggers, not the model alone, determine loop behaviour,
cost, and safety.

The mechanism implements the five build stages of an agentic loop:

1. **Define the goal format** — clear, evaluable goals with a success state,
   scope boundaries, and constraints (FR-006, FR-007, FR-021, FR-022).
2. **Choose the minimum tool set** — a restricted per-loop tool surface
   (FR-008, FR-009).
3. **Set stopping conditions** — step limit, cost limit, and human
   checkpoints for destructive actions (FR-013 – FR-015).
4. **Test with rollback** — pre-loop snapshots, post-loop change summary,
   and a one-key rollback (FR-018 – FR-020).
5. **Evaluate the outputs** — verification-command outcome, anti-cheat
   read-only constraints, and diff review (FR-007, FR-019, FR-021).

## Background & Definitions

- **Agentic loop** — the iterative cycle in which an agent receives a goal,
  plans the next action, executes it (tool call), observes the output, and
  decides whether to terminate or loop back.
- **Iteration** — one pass of the loop: budget gates, LLM request, response
  parse, tool execution, observation append.
- **Observation** — a tool result, tool error, denial, or constraint
  violation appended to the model's context.
- **Goal format** — the structured goal: *success state* (what must be true,
  optionally an evaluable verification command), *scope boundaries* (paths in
  play), *constraints* (forbidden paths/actions, e.g. read-only tests).
- **Stop conditions** — the four loop exits: goal achieved
  (`completed`), unrecoverable error (`error`), step/cost budget exhausted
  (`budget_exhausted`), human intervention (`interrupted`).
- **Termination status** — the published outcome label of a loop run: one of
  the four statuses above.
- **Destructive action** — file deletion, config writes, dependency
  installation, destructive git operations, or any tool call marked
  destructive by the tool registry.
- **Checkpoint** — a forced human-approval pause before a destructive action.
- **Rollback** — restoration of the workspace to the pre-loop snapshot.

## Scope & Objectives

### In scope

- The `/loop` slash command: interactive setup dialog (agent picker, goal
  form) and one-shot argument form.
- Structured goal composition (success state, scope, constraints) injected
  into the loop prompt.
- Optional verification command that gates goal achievement.
- Per-loop tool-set restriction with out-of-scope denial observations.
- The four stop conditions with explicit termination statuses and event-bus
  publication, including step limit and cost limit budget gates.
- Destructive-action checkpoints (forced ask prompts) and user interrupt.
- Pre-loop snapshot/git-state capture, post-loop change summary, and
  rollback.
- Loop telemetry (iterations, duration, tool-call counts).
- HTTP API parity for starting and monitoring loops.

### Out of scope

- Provider/model selection changes; compaction behaviour (see
  `specs/compact/SPEC.md`); team/swarm orchestration loops; research-system
  supervision loops; new TUI panels beyond the setup dialog and the existing
  status/step surfaces; background scheduled loops.

## Related Research

- Research finding: agentic loop defined as plan-act-observe cycle with four
  stop conditions ([mindstudio.ai practitioner article]; consistent with
  survey-level definitions of agentic AI). The finding's implication drives
  this spec: teams must specify **goals, budgets, and intervention triggers
  explicitly** because these parameters — not the model alone — determine
  loop behaviour, cost, and safety.

---

## Requirements

### FR-001 — `/loop` command availability (ubiquitous)

The system **shall** register a `/loop` slash command in the TUI so that it
appears in slash-command autocomplete and `/help`, and **shall** accept both
the interactive form (`/loop` with no arguments) and the one-shot form
(`/loop <agent> <goal text...>`).

### FR-002 — Interactive setup dialog (event-driven)

When `/loop` is invoked with no arguments, the system **shall** open a loop
setup dialog with: an agent selector (list of loaded agents navigable with
arrow keys), a goal text field, an optional verification command field, an
optional scope field (path globs), an optional constraints field
(read-only path globs), an optional tool-set field (tool names), a step-limit
field (default 25), a cost-limit field (token budget, default from config),
and a checkpoints toggle (default on) — and **shall** start the loop only
when the user confirms.

### FR-003 — One-shot form defaults (event-driven)

When `/loop <agent> <goal>` is invoked with arguments, the system **shall**
start the loop immediately using the named agent and goal text with the
documented defaults (step limit 25, config cost limit, checkpoints on, no
verification command, no extra restrictions).

### FR-004 — Dialog cancellation (state-driven)

While the setup dialog is open, the system **shall** close it and start
nothing when the user presses `Esc`, and **shall** preserve all entered
values if the user re-opens `/loop` in the same session.

### FR-005 — No loop without a goal (unwanted)

The system **shall not** start a loop with an empty or whitespace-only goal;
when the goal is missing it **shall** show an error message naming the
missing field and return to the dialog (or, for the one-shot form, print
usage help).

### FR-006 — Structured goal composition (ubiquitous)

The system **shall** compose the loop's system/user prompt from the
structured goal — the success state, the verification command (if any), the
scope boundaries, the constraints, and the active step/cost limits — so the
model can verify its own progress and know its boundaries before acting.

### FR-007 — Verification-gated goal achievement (optional)

Where a verification command is configured for the loop, the system **shall**
run it automatically when the model first responds without tool calls:
if the command succeeds the loop terminates with status `completed`; if it
fails and steps remain, the system **shall** append the command's failure
output as an observation and continue the loop; if it fails and no steps
remain, the loop terminates with status `budget_exhausted`. Where no
verification command is configured, a no-tool-call response terminates the
loop with status `completed` directly.

### FR-008 — Per-loop tool-set restriction (optional)

Where a tool set is configured for the loop, the system **shall** restrict
the loop's tool surface to exactly that set plus mandatory safety tools
(memory and think), while all other tools remain visible to the rest of the
application.

### FR-009 — Out-of-scope tool denial (unwanted)

The system **shall not** execute a tool call outside the loop's configured
tool set; the system **shall** instead return a denial observation to the
model stating that the tool is outside the loop's scope.

### FR-010 — Stop condition 1: goal achieved (event-driven)

When the model responds without tool calls and (where configured) the
verification command passes, the system **shall** terminate the loop with
termination status `completed`, publish the loop-termination event, and
record final loop metrics.

### FR-011 — Stop condition 2: unrecoverable error (event-driven)

When a loop stage fails with an unrecoverable error — provider transport
failure, permission hard-deny, tool panic, context overflow — the system
**shall** terminate the loop with termination status `error`, surface the
failure reason to the user, and **shall not** retry the failed stage.

### FR-012 — Recoverable errors as observations (event-driven)

When a tool invocation fails with a recoverable error, the system **shall**
append the error text as an observation and continue; when consecutive
recoverable failures exceed the retry allowance (default 3), the system
**shall** terminate the loop with termination status `error`.

### FR-013 — Stop condition 3a: step limit (state-driven)

While the iteration counter has reached the configured step limit, the system
**shall** terminate the loop with termination status `budget_exhausted`
**before** sending another LLM request.

### FR-014 — Stop condition 3b: cost limit (state-driven)

While the accumulated token usage of the loop has reached the configured cost
limit, the system **shall** terminate the loop with termination status
`budget_exhausted` **before** sending another LLM request.

### FR-015 — Destructive-action checkpoints (optional)

Where checkpoints are enabled for the loop (default), the system **shall**
force a human-approval prompt before any destructive action (file deletion,
config writes, dependency installation, destructive git operations) even when
an allow rule would otherwise auto-approve it, and **shall** treat a prompt
timeout as denial (intervention defaults to the safe choice).

### FR-016 — Human interrupt (optional)

Where the user presses `Esc` while a loop is running, the system **shall**
abort the loop at the next safe point between stages, terminate with status
`interrupted`, and leave the session persisted and resumable.

### FR-017 — No iteration after a stop condition (unwanted)

The system **shall not** send another LLM request or execute another tool
after any stop condition has been met for the loop run.

### FR-018 — Pre-loop state capture (event-driven)

When a loop starts, the system **shall** capture a file snapshot of the
workspace before the first write action and, when the workspace is inside a
git repository, record the current branch and HEAD; when it is not, the
system **shall** warn that rollback is snapshot-only and require the user to
confirm before starting.

### FR-019 — Post-loop change summary (event-driven)

When a loop terminates, the system **shall** publish a change summary —
counts of files modified, created, and deleted plus a diffstat — together
with the termination status, iteration count, and verification outcome (if
any), rendered in the TUI and available on the event bus.

### FR-020 — Rollback on request (optional)

Where the user accepts the rollback offer after a loop terminates, the system
**shall** restore the workspace from the pre-loop snapshot; where the user
declines, the system **shall** keep the changes.

### FR-021 — Read-only constraints (state-driven)

While a constraint marks paths read-only for the loop, the system **shall**
deny writes to those paths and return a denial observation explaining the
constraint, so the agent cannot satisfy a goal by modifying protected files
such as tests.

### FR-022 — Scope boundaries (ubiquitous)

The system **shall** restrict loop file access to the configured scope
boundaries (default: the working directory) and return a denial observation
for out-of-scope file operations.

### FR-023 — No silent observation loss (unwanted)

The system **shall not** drop, truncate, or silently discard any
observation; when an observation cannot be appended to the context the
system **shall** surface an error and treat it as an unrecoverable failure.

### FR-024 — Permission layer always in the path (unwanted)

The system **shall not** execute any tool call without passing the permission
layer — including checkpoint-escalated destructive calls in auto-approve
(autopilot/YOLO) mode: auto-approval may answer prompts automatically but
hard-deny rules and loop checkpoints are still enforced.

### FR-025 — Loop telemetry (ubiquitous)

The system **shall** record, for every loop run, the iteration count, total
duration, and per-iteration tool-call counts via the existing agent-loop
telemetry instruments, so loop depth and cost are observable after the run.

### FR-026 — HTTP loop endpoints (optional)

Where the HTTP server is running, the system **shall** accept
`POST /loop` to start a loop (agent, goal, and the structured fields of
FR-002) and return the session/run identifier, and **shall** expose the
loop's termination status and change summary through the existing session
SSE stream.

---

## Non-Functional Requirements

### NFR-001 — Performance

Budget gates shall be in-memory counter comparisons adding no measurable
per-iteration overhead; the setup dialog shall open in under 100 ms.

### NFR-002 — No new dependencies

The mechanism shall be implemented in existing workspace crates
(`ragent-agent`, `ragent-config`, `ragent-types`, `ragent-tui`,
`ragent-server`, `ragent-storage`) without adding external dependencies.

### NFR-003 — Testability

Every functional requirement shall be verifiable by a deterministic test in
the owning crate's `tests/` directory (scripted fixture providers, small
budgets, scripted permission decisions) or by the manual test plan.

### NFR-004 — Documentation

The loop contract, the `/loop` command, the goal format, and the budget and
checkpoint tunables shall be documented in `QUICKSTART.md` (or
`TUI-QUICKSTART.md`) and crate rustdoc.

## Constraints & Assumptions

- The existing `SessionProcessor` loop is the implementation home; this spec
  formalises its contract and layers the `/loop` UX, verification gate, and
  rollback flow on top.
- Compaction may run inside the loop and is transparent to these
  requirements (it is not a stop condition).
- The permission system remains the sole enforcement point for tool
  execution; the loop adds checkpoint escalation and constraint denials as
  inputs to it, not a parallel authority.
- Snapshot/undo machinery already exists (`ragent-storage` snapshots); the
  loop reuses it for pre-loop capture and rollback.

## Interfaces & Dependencies

- `crates/ragent-tui/src/app/slash.rs` — `/loop` command, setup dialog.
- `crates/ragent-agent/src/session/processor.rs` — `SessionProcessor` loop:
  iteration contract, budget gates, verification gate, stop-condition
  dispatch, termination publication.
- `crates/ragent-config/src/config.rs` — `AgentConfig::max_steps` (existing)
  and loop defaults (cost limit, retry allowance, checkpoint behaviour).
- `crates/ragent-types/src/event.rs` — loop lifecycle events (termination
  status, change summary).
- `crates/ragent-storage/` — pre-loop snapshot capture and rollback.
- `crates/ragent-telemetry/` — `agent_loop_duration` / `agent_loop_iterations`
  instruments (existing).
- `crates/ragent-server/` — `POST /loop` endpoint (FR-026).

## Glossary

- **Goal format** — structured goal: success state, scope boundaries,
  constraints.
- **Verification command** — an evaluable shell command whose exit status
  defines goal achievement.
- **Stop condition** — goal achieved, unrecoverable error, budget exhausted,
  human intervention.
- **Checkpoint** — forced human approval before a destructive action.
- **Change summary** — post-loop diffstat of modified/created/deleted files.
- **Rollback** — restore of the pre-loop snapshot.
- **EARS** — Easy Approach to Requirements Syntax, the notation used here.

---

*End of Specification*
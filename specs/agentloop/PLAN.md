---
spec_id: agentloop
---

# Implementation Plan: Agentic Loop Mechanism (`/loop`)

## Overview

This plan implements the `agentloop` specification (`specs/agentloop/SPEC.md`):
a goal-driven plan-act-observe agentic loop exposed as a **`/loop` slash
command** for selecting an agent and setting a goal, with the four stop
conditions (goal achieved FR-010, unrecoverable error FR-011, budget
exhausted FR-013/FR-014, human intervention FR-015/FR-016), structured goal
composition (FR-006), verification gating (FR-007), per-loop tool-set and
scope restriction (FR-008/FR-009/FR-022), read-only constraints (FR-021),
destructive-action checkpoints (FR-015), pre-loop snapshot + rollback
(FR-018/FR-020), post-loop change summary (FR-019), and telemetry
(FR-025).

All work targets existing crates (`ragent-agent`, `ragent-config`,
`ragent-types`, `ragent-tui`, `ragent-server`, `ragent-storage`). No new
external dependencies (NFR-002).

## Architecture

```
crates/ragent-tui/src/app/
├── slash.rs                  # + /loop dispatch (interactive + one-shot)
└── loop_dialog.rs            # NEW: agent picker + goal form (Esc to cancel,
                             #  values preserved)

crates/ragent-agent/src/
├── session/
│   ├── processor.rs          # loop contract: budget gates, verification gate,
│   │                         # stop dispatch, change summary publication
│   └── loop_state.rs         # NEW: LoopSpec {agent, goal, verify_cmd, scope,
│                             #   constraints, tool_set, max_steps, cost_limit,
│                             #   checkpoints}, StopCondition enum, LoopState
└── error.rs                  # + ErrorKind::{Recoverable, Unrecoverable}

crates/ragent-config/src/
└── config.rs                 # + loop defaults: cost_limit, error_retry_allowance,
                              #   checkpoints default

crates/ragent-types/src/
└── event.rs                  # + Event::LoopTerminated {status, iterations,
                              #   verification}, Event::LoopChangeSummary

crates/ragent-storage/src/
└── snapshots.rs              # + capture_pre_loop_snapshot(), rollback()

crates/ragent-server/src/
└── routes/loop.rs            # NEW: POST /loop (FR-026)

crates/ragent-telemetry/src/  # record_agent_loop wiring (exists)
```

### Data flow — one iteration

```
        ┌───────────────────────────────────────────────────────┐
        │ budget gates (BEFORE any LLM request)                 │
        │  step >= max_steps?  -> budget_exhausted (FR-013)     │
        │  tokens >= cost_limit? -> budget_exhausted (FR-014)   │
        └──────────────┬────────────────────────────────────────┘
                       ▼
   plan    : send goal + context -> LLM response (tool calls | final answer)
                       ▼
   act     : permission layer -> tool-set/scope/constraint denials (FR-009/021/022)
             destructive? && checkpoints on -> forced ask (FR-015)
             execute allowed calls
                       ▼
   observe : append results/errors/denials to context (FR-012, FR-023)
                       ▼
   decide  : no tool calls?  -> verification gate (FR-007) -> completed (FR-010)
             unrecoverable?  -> error, no retry (FR-011)
             consecutive recoverable failures > allowance? -> error (FR-012)
             else loop back
```

Termination publishes `Event::LoopTerminated` + `Event::LoopChangeSummary`
exactly once (FR-019), records `record_agent_loop` (FR-025), and no stage
runs afterwards (FR-017).

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Create `session/loop_state.rs`: `LoopSpec` (agent, goal, verify_cmd, scope globs, read-only constraints, tool set, max_steps, cost_limit, checkpoints), `StopCondition` enum (`GoalAchieved`, `UnrecoverableError`, `BudgetExhausted`, `HumanIntervention`), `LoopState` tracker (step, token tally, consecutive-failure counter, stop flag) | FR-006, FR-013, FR-014 | M | Critical | completed | — |
| T-002 | Add loop defaults to `ragent-config`: `cost_limit` (token budget), `error_retry_allowance` (default 3), checkpoint default (on); document `max_steps` default 25 | FR-013, FR-014, FR-015 | S | High | completed | — |
| T-003 | Insert pre-request budget gates in `SessionProcessor`: check step counter and token tally before each LLM request; on breach stop with `BudgetExhausted` and skip the request | FR-013, FR-014, FR-017 | M | Critical | completed | T-001, T-002 |
| T-004 | Implement stop condition 1: no-tool-call response with no verification command terminates with `GoalAchieved` (status `completed`), publishes `Event::LoopTerminated`, records loop metrics | FR-010, FR-025 | S | Critical | completed | T-001 |
| T-005 | Implement the verification gate: on first no-tool-call response, run the verification command; success -> `completed`, failure with steps remaining -> append failure output as observation and continue, failure without steps -> `budget_exhausted` | FR-007, FR-010 | M | Critical | completed | T-004 |
| T-006 | Add `ErrorKind::{Recoverable, Unrecoverable}` classification: transport failure, permission hard-deny, panic, context overflow = unrecoverable; tool-level failures = recoverable by default | FR-011, FR-012 | M | Critical | completed | T-001 |
| T-007 | Implement stop condition 2: unrecoverable errors terminate immediately (status `error`, no retry, reason surfaced); recoverable failures append error observations and terminate at the retry allowance | FR-011, FR-012 | M | Critical | completed | T-006 |
| T-008 | Add `Event::LoopTerminated {session_id, status, iterations, verification, reason}` to `ragent-types::event`; guard `LoopState` so no stage executes after the stop flag is set | FR-010, FR-017 | S | Critical | completed | T-001 |
| T-009 | Implement per-loop tool-set and scope restriction in the permission layer: calls outside the loop's tool set or scope return denial observations (scope violation, tool out of scope); read-only constraints deny writes to constrained paths | FR-008, FR-009, FR-021, FR-022 | M | Critical | completed | T-001 |
| T-010 | Implement destructive-action checkpoints: mark destructive tool calls (delete, config write, dependency install, destructive git); with checkpoints on, force ask prompts even under allow rules and in auto-approve mode; prompt timeout = denial | FR-015, FR-024 | M | High | completed | T-009 |
| T-011 | Implement human interrupt: `Esc` during a loop aborts at the next inter-stage safe point, stops with `HumanIntervention` (status `interrupted`), leaves the session persisted and resumable | FR-016 | M | High | completed | T-008 |
| T-012 | Pre-loop capture: before the first write action, snapshot the workspace (existing storage snapshots); when inside git, record branch + HEAD, else warn and require confirmation; on termination, compute the change summary (modified/created/deleted counts + diffstat) and publish `Event::LoopChangeSummary` | FR-018, FR-019 | M | High | completed | T-008 |
| T-013 | Rollback flow: after termination, offer rollback; acceptance restores the pre-loop snapshot, decline keeps changes | FR-020 | S | High | completed | T-012 |
| T-014 | Build the loop setup dialog (`loop_dialog.rs`): agent picker (arrow keys), goal/verification/scope/constraints/tool-set fields, step and cost limit fields, checkpoints toggle; `Esc` cancels without starting; re-open preserves entered values; confirm starts the loop | FR-002, FR-004 | L | High | completed | T-001 |
| T-015 | Register `/loop` in `slash.rs`: no-arg opens the dialog; one-shot `/loop <agent> <goal>` starts immediately with documented defaults; empty goal shows an error naming the missing field (dialog re-open or usage help); add to `SLASH_COMMANDS` for autocomplete and `/help` | FR-001, FR-003, FR-005 | M | Critical | completed | T-014, T-008 |
| T-016 | Compose the structured goal prompt: success state, verification command, scope, constraints, active limits injected into the loop system/user prompt before the first iteration | FR-006 | S | High | completed | T-001, T-015 |
| T-017 | Wire loop telemetry: `record_agent_loop(iterations, duration)` once per run plus per-iteration tool-call counts; confirm per-step event-bus counter visibility | FR-025 | S | Medium | completed | T-004 |
| T-018 | HTTP endpoint: `POST /loop` accepting agent, goal, and structured fields; loop status + change summary exposed through the existing session SSE stream | FR-026 | M | Medium | completed | T-008, T-012 |
| T-019 | Tests: stop conditions 1 + 3 — scripted no-tool-call response ends `completed` (no verification); `max_steps: 2` stops `budget_exhausted` before a 3rd request; token budget breach stops before the next request; verification-pass and verification-fail-then-continue paths | FR-007, FR-010, FR-013, FR-014, NFR-003 | M | Critical | completed | T-003, T-005 |
| T-020 | Tests: errors — scripted provider failure stops `error` with no retry; recoverable tool failures stop at the retry allowance with error observations in context; observation-append failure escalates to `error` (never silent) | FR-011, FR-012, FR-023, NFR-003 | M | Critical | completed | T-007 |
| T-021 | Tests: restrictions — out-of-tool-set call denied with scope observation; write to read-only path denied with constraint observation; out-of-scope file op denied | FR-008, FR-009, FR-021, FR-022, NFR-003 | M | High | completed | T-009 |
| T-022 | Tests: checkpoints + interrupt — destructive call forces ask even with allow rule; timeout behaves as denial; `Esc` yields `interrupted` with resumable session; hard-deny still enforced in auto-approve | FR-015, FR-016, FR-024, NFR-003 | M | High | completed | T-010, T-011 |
| T-023 | Tests: capture/summary/rollback — pre-loop snapshot exists after any write; change summary counts match induced file changes; rollback restores the snapshot; decline keeps changes | FR-018, FR-019, FR-020, NFR-003 | M | High | completed | T-012, T-013 |
| T-024 | Tests: dialog + one-shot + telemetry — dialog fields populate/parse; empty goal rejected with error; one-shot starts with defaults; `record_agent_loop` recorded exactly once with the final iteration count | FR-001, FR-002, FR-003, FR-005, FR-025, NFR-003 | M | High | completed | T-014, T-015, T-017 |
| T-025 | TUI: render `LoopTerminated` as a status-aware banner (`completed` / `error` / `budget exhausted` / `interrupted` + iteration count) and `LoopChangeSummary` as a diffstat line with a rollback offer prompt | FR-019, FR-020 | S | Medium | completed | T-008, T-012, T-013 |
| T-026 | Documentation: `/loop` usage, goal format (success state / scope / constraints), stop conditions, budget + checkpoint tunables, rollback flow in `TUI-QUICKSTART.md` and loop module rustdoc | FR-006, NFR-004 | S | Medium | completed | T-015 |
| T-027 | Full verification: `cargo fmt`, `cargo clippy`, targeted `cargo test -p ragent-agent -p ragent-config -p ragent-types -p ragent-tui -p ragent-server -p ragent-storage` with no regressions | NFR-001, NFR-002, NFR-003 | S | Critical | completed | T-001–T-026 |
## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Budget gates placed after the LLM request waste spend on the breaching iteration | Medium | High | FR-013/FR-014 mandate before-request gates; T-019 tests the pre-request ordering. |
| Verification command itself is destructive or slow | Medium | Medium | Run through the permission layer with checkpoints applied (FR-015); command output is truncated as an observation. |
| Scope/constraint denial observations confuse the model into looping | Low | Medium | Denial messages state the rule and suggest in-scope alternatives; retry allowance bounds looping (FR-012). |
| Dialog state lost on accidental `Esc` | Low | Low | FR-004 preserves entered values for re-open in the same session. |
| Snapshot cost on very large workspaces | Medium | Medium | Reuse incremental snapshots (changed-file diffs only); capture before first write, not at loop start. |
| Rollback destroys unrelated user changes made during the loop | Low | High | Rollback restores only the pre-loop snapshot's file set and the summary lists affected files before the user confirms (FR-019/FR-020). |
| Auto-approve (YOLO) bypassing checkpoints | Low | Critical | FR-024 keeps checkpoints and hard-deny rules enforced in every mode; covered by T-022. |

---

## Definition of Done

1. All 26 functional requirements (FR-001 – FR-026) have at least one passing
   test or verified manual check (NFR-003).
2. The four stop conditions are each exercised end-to-end: `completed`,
   `error`, `budget_exhausted`, `interrupted`.
3. `/loop` is registered with autocomplete, opens the setup dialog, and
   accepts the one-shot form; an empty goal is rejected.
4. `Event::LoopTerminated` and `Event::LoopChangeSummary` are published
   exactly once per run and rendered in the TUI; rollback works from the
   pre-loop snapshot.
5. `cargo fmt --check`, `cargo clippy`, and targeted `cargo test` are clean.
6. `TUI-QUICKSTART.md` documents the `/loop` command and goal format.
7. The `agentloop` spec status is moved to `implemented` in its frontmatter.

---

*End of Implementation Plan*
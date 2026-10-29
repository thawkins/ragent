# Implementation Plan: Sub-Agent Tool Family and Agent-Control Surface

**Spec:** [SPEC.md](SPEC.md) · **Test plan:** [TESTPLAN.md](TESTPLAN.md)

## Approach

The work is a **tightening**, not a rewrite. The sub-agent family already has the
right primitives - `AgentManager` owns the registry, `TaskStatus` already
distinguishes the six states, `bg` already owns its own service - and every finding
in SPEC.md is a *seam* between them: a declared parameter that is not honoured, a
status collapsed into a coarser label, a UI action that does not delegate, a
description that has drifted. The plan therefore changes the seams, not the engine.

The change splits into **five layers**, and every layer is independently shippable:

1. **Contract (tool layer, `crates/ragent-agent/src/tool/` and `task/mod.rs`).**
   Make the three execution contracts explicit, enforce the declared schema, add the
   per-call bound, fix the permission taxonomy, share the short-id helper, and make
   refusals into errors with machine-readable reasons (FR-001..FR-010, FR-017,
   FR-019, FR-034, FR-037, FR-038, FR-039, FR-040, FR-044, FR-045).

2. **Status and observation (tool layer + registry).**
   Stop collapsing `cancelled` / `failed` / `suspended` / `terminating`, report the
   limit and its headroom, expose waiter counts, and make the durable-copy absence
   explicit (FR-007, FR-011..FR-015, FR-018, FR-029..FR-033, FR-035, FR-043).

3. **Presentation (`crates/ragent-tui/src/app/subagent_events.rs`, `layout*.rs`).**
   Paint the real execution contract, add the completed-history section, add the
   `SubagentKilled` arm, paint cancellation-requested and terminating states, and
   give the window a keyboard open chord (FR-020..FR-022, FR-026..FR-028,
   FR-031, FR-036, FR-042).

4. **Commands (`crates/ragent-tui/src/app/slash.rs`, `app/spawn.rs`).**
   Make `/cancel` delegate to `AgentManager::cancel_agent` with ambiguity handling,
   and fold the `/spawn` hand-rolled launch slot into the shared async-launch helper
   (FR-023, FR-024, FR-041).

5. **Documentation and guards (prompt, README, rustdoc, tests).**
   Regenerate the descriptions from the contract, teach the whole contract in the
   system prompt, and add the missing tests plus a guard that keeps descriptions and
   schemas in step (FR-006, FR-016, FR-025, NFR-005, H1..H5).

## Ordering constraints

Two orderings are **not** negotiable:

- **T-001 precedes every other tool-layer task.** Removing the `detached` /
  `background` ambiguity first means the rest of the layer reasons about three named
  contracts instead of two booleans. Building the schema enforcement (T-002) before
  the ambiguity is gone would lock the ambiguous shape in.
- **T-018 (`/cancel` delegation) precedes T-021 (completed history).** If history is
  built while `/cancel` still removes rows optimistically, the history will record
  cancellations that never happened; the delegation must land first.

## Sequencing

```
Layer 1  T-001 -> T-002 -> T-003 -> T-004 -> T-005 -> T-006 -> T-007 -> T-008
                                                                   |
Layer 2  T-009 <- T-010 <- T-011 <- T-012 <- T-013 <- T-014 <- T-015 <- T-016 <- T-017
                                                                   |
Layer 3  T-018 -> T-019 -> T-020 -> T-021 -> T-022 -> T-023
                                                                   |
Layer 4  T-024 -> T-025
                                                                   |
Layer 5  T-026 -> T-027
```

Layers 1 and 2 both touch `tool/`, so they run in sequence. Layer 3 needs the status
vocabulary from layer 2 (a panel cannot paint `terminating` before the tool reports
it). Layer 4's `/cancel` work needs T-012's honest cancel status. Layer 5 lands last
so the descriptions and the prompt describe the finished contract.

## Layer 1 - Contract

**Design note (FR-001, FR-003).** Replace the two independent booleans with one
derived enum inside `new_agent`:

```rust
enum Execution { Foreground, Background, Detached }
```

`Foreground` when `background` is absent or `false` and `detached` is absent or
`false`; `Background` when `background: true` and `detached` is absent or `false`;
`Detached` when both are `true`; **refused** when `detached: true` and `background`
is absent or `false`. The refusal is the only behaviour change in the whole plan and
it is deliberate: today that call shape silently runs a blocking foreground task
(SPEC.md A1), which is the opposite of what the caller asked for.

**Design note (FR-004, FR-038).** Schema enforcement goes in **one** helper called by
all four tools plus `bg`, not four copies: the family shares a validation entry point
that reads `parameters_schema()["properties"]` and rejects unknown keys, so the
schema stays the single source of truth and a future property cannot be added to the
schema and forgotten in the validator.

**Design note (FR-009, FR-045).** The team refusal changes from
`Ok(ToolOutput{ metadata: {blocked:true} })` to `Err(anyhow!)`. That is a real
behaviour change for a team session, but no caller branches on the current shape
(the content is prose), and treating "you may not do this" as success is precisely
the anti-pattern. The error text keeps the existing guidance verbatim so the model
loses no information.

**Design note (FR-005).** `list_agents` moves from `"none"` to a read-only agent
category. It is already in `AUTO_APPROVED_AGENT_TOOLS`
(`session/permissions.rs:207-213`), so the category change tightens classification
without changing the auto-approval outcome.

## Layer 2 - Status and observation

**Design note (FR-013, FR-040).** `cancel_agent` gains a "cancel requested, current
status" report: after the flag is set it re-reads the entry and reports
`cancel_requested: true` plus `status: <actual>`. The existing `"cancelled"` literal
disappears; the terminal `cancelled` status still arrives later via
`SubagentCancelled` and is reported by `list_agents` / `wait_agents`.

**Design note (FR-011, FR-029, NFR-003).** The limit report is computed from the same
`DashMap` scan the spawn path already does; no storage read is added. `list_agents`
gains `limit`, `occupancy`, `headroom` in metadata and a one-line content summary.
The spawn refusal reuses the same helper, so the two numbers can never disagree.

**Design note (FR-030).** Counting `suspended` tasks against the limit removes a
working escape hatch: today a user can suspend a task to free a slot. That is
deliberate - a suspended task still holds its wall-clock budget and resumes
spontaneously, so letting it free a slot means a later resume can push occupancy
over the limit silently. Cancellation (`kill_task`) becomes the supported way to
free a slot, and T-024 updates the panel legend and the guidance block to say so.

**Design note (FR-014).** `wait_agents` carries a `status` string per result entry
derived from `TaskStatus`, alongside the existing `success` boolean, so a caller can
distinguish `cancelled` from `failed` while the boolean keeps backward
compatibility for a model that already branches on it.

## Layer 3 - Presentation

**Design note (FR-020).** `note_subagent_started` stops hard-coding
`detached: false`: the `SubagentStart` event carries `background` today and gains a
`detached` flag in the same change, so the painted row and the registry entry are
the same shape. The renderer derives the contract badge from that field.

**Design note (FR-021, NFR-004).** The completed history is a bounded `VecDeque` on
`App` with a configurable cap (default 20), pushed by the same handlers that remove
live rows, so no new event plumbing is required. The history section renders under
the live section, newest first, with a `[file]` marker when `output_file` exists
and an explicit "in-memory only" marker when it does not.

**Design note (FR-022).** The open chord is `Alt+A`, chosen because it is the next
free letter in the established `Alt+C` / `Alt+M` / `Alt+T` / `Alt+Q` family; the
chord goes through the `InputAction` enum like every other panel toggle, so the
mouse path and the keyboard path call the same `toggle_agents_window`.

## Layer 4 - Commands

**Design note (FR-023, FR-024).** `/cancel` becomes: collect live task ids that start
with the prefix; zero matches → name the live ids; more than one → list them and
refuse; exactly one → call `AgentManager::cancel_agent` and report the registry's
answer. The row is **not** removed by the command - the terminal event or the
reconciliation does that, which is what makes the panel trustworthy.

**Design note (D3, `/spawn`).** The hand-rolled `Mutex<Option<Result<..>>>` slot and
the fallback `current_thread` runtime are folded into one shared async-launch helper
the other `poll_*` paths can use, so `/spawn` stops being a special case.

## Layer 5 - Documentation and guards

**Design note (FR-016).** A guard test asserts that every property name in a family
tool's `parameters_schema` appears in that tool's `description()`. That converts the
drift recorded as SPEC.md D1 from a review finding into a build failure.

**Design note (FR-025).** The system-prompt block gains the detached contract, the
limit occupancy wording and the "cancel, don't suspend, to free a slot" rule, using
the same words as the tool descriptions.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Derive one `Execution` enum in `new_agent` and refuse `detached: true` without `background` | FR-001, FR-003 | S | Critical | completed | — |
| T-002 | Add one shared schema-property validator and call it from `new_agent`, `cancel_agent`, `list_agents`, `wait_agents`, `bg` | FR-002, FR-004, FR-038 | M | Critical | completed | T-001 |
| T-003 | Add machine-readable `reason` codes to every branchable sub-agent tool error | FR-010, NFR-005 | S | High | completed | T-002 |
| T-004 | Fix the permission taxonomy (`list_agents` read category, `cancel_agent` to `agent:control`) | FR-005 | S | High | completed | T-001 |
| T-005 | Replace `&task.id[..8]` with `short_prefix` and audit every id abbreviation | FR-008 | S | High | completed | — |
| T-006 | Turn the team-context refusal in `new_agent` into a tool error while keeping the guidance text | FR-009, FR-045 | S | High | completed | T-001 |
| T-007 | Align the `model` parameter documentation across schema, description, prompt and README | FR-006 | S | Medium | completed | T-001 |
| T-008 | Add the optional per-call time bound to `new_agent` and apply it to the spawned task | FR-017, FR-034 | M | Medium | completed | T-001 |
| T-009 | Report the real current status after a cancel request in `cancel_agent` | FR-007, FR-013, FR-040 | S | Critical | completed | T-003 |
| T-010 | Carry a `status` string per result entry in `wait_agents` and stop collapsing cancelled into failed | FR-007, FR-014 | S | Critical | completed | T-009 |
| T-011 | Report a suspended task as `suspended` in `wait_agents` partial results and refusal text | FR-015 | S | High | completed | T-010 |
| T-012 | Report limit, occupancy and headroom in `list_agents` metadata and content | FR-011, FR-029, FR-035, NFR-003 | S | High | completed | T-002 |
| T-013 | Count suspended background tasks against the limit and document the change | FR-030 | S | High | completed | T-012 |
| T-014 | Report `waiter_count` per live task in `list_agents` detail mode | FR-012 | S | Medium | completed | T-012 |
| T-015 | Report durable-copy absence explicitly in `list_agents` and `wait_agents` | FR-018, FR-032, FR-043 | S | High | completed | T-010 |
| T-016 | Refuse a `wait_agents` await of a detached task with a contract-naming error | FR-044 | S | Medium | completed | T-010 |
| T-017 | Restrict `bg` to shell-task vocabulary and refuse a foreign `session_id`, accepting the caller's own | FR-019, FR-037 | S | Medium | completed | T-002 |
| T-018 | Make `/cancel` delegate to `AgentManager::cancel_agent` with ambiguity refusal and live-id listing | FR-023, FR-024, FR-041 | M | Critical | completed | T-009 |
| T-019 | Add `Alt+A` open chord and route it through `InputAction` to the same toggle as the mouse | FR-022 | S | High | completed | — |
| T-020 | Carry `detached` on `SubagentStart` and stop hard-coding it in `note_subagent_started`; paint the contract badge | FR-020 | M | High | completed | T-001 |
| T-021 | Add the bounded completed-history section to the Agents window | FR-021, FR-028, FR-036, NFR-004 | M | High | completed | T-018 |
| T-022 | Add the `SubagentKilled` event arm and paint cancellation-requested and terminating states | FR-026, FR-027, FR-031, FR-033 | M | High | completed | T-020 |
| T-023 | Name the reason on a refused empty-window open and on an absent durable copy in the panel | FR-032, FR-042 | S | Medium | completed | T-021 |
| T-024 | Fold the `/spawn` launch slot and helper runtime into one shared async-launch helper | D3 (SPEC.md) | M | Low | completed | T-018 |
| T-025 | Regenerate the four tool descriptions from the contract and add the description/schema guard test | FR-016 | M | High | completed | T-008, T-017 |
| T-026 | Teach the three contracts, the limit occupancy and the cancel-to-free rule in the system prompt | FR-025 | S | High | completed | T-025 |
| T-027 | Add the five missing tests (detached-without-background, unknown key, cancel status, cancelled-vs-failed, `/cancel` delegation) and update the registry rustdoc | FR-003, FR-004, FR-013, FR-014, FR-023, G1 | M | High | completed | T-009, T-018 |
| T-028 | Bind `Alt+T` and make both window chords close an open window unconditionally, refusing and naming an empty open | FR-042, FR-046 | S | High | completed | T-019, T-023 |
## Risks

1. **FR-003 is a behaviour change, not an additive requirement.** A caller that
   passes `detached: true` with `background` omitted silently degrades to a blocking
   foreground run today. After T-001 that call fails. The mitigation is that T-001
   lands first, the error names both legal combinations, and no in-repo caller uses
   that shape (the `/spawn` command and the server route both set `background`). If
   an external client depends on the degradation, the requirement needs revisiting
   before implementation, not after.

2. **FR-030 removes a working escape hatch.** Counting suspended tasks against the
   limit means "suspend one to make room" stops working. The mitigation is that
   `kill_task` already exists and works, T-024's legend/prompt update names
   cancellation as the supported path, and TC-008 exercises the sequence explicitly.
   A user who relies on suspend-to-free is the only party affected.

3. **The completed-history section grows a new unbounded-looking surface.** A
   `VecDeque` that only ever grows would be a slow leak in a long session. NFR-004
   and FR-036 make the cap mandatory and TC-010 asserts the oldest row is evicted.

4. **Description/schema drift can regress.** T-025's guard test only covers the four
   sub-agent tools; `bg` is included because it shares the family's
   `additionalProperties` pattern. A future tool outside those five can still drift -
   acceptable, because the guard is cheap to extend and naming five is better than
   naming none.

5. **`/cancel` delegation depends on event delivery.** If the `SubagentCancelled`
   event is dropped, T-018's honest reporting means the row lingers until
   reconciliation rather than disappearing optimistically. That is the intended
   trade (a stale row for one reconciliation interval beats a false "cancelled"
   claim), and it is asserted by TC-012.
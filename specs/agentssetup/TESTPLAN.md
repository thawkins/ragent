---
status: draft
---
# Manual Test Plan: Sub-Agent Tool Family and Agent-Control Surface

**Spec:** [SPEC.md](SPEC.md) · **Plan:** [PLAN.md](PLAN.md)

This is a **manual** test plan. Every case is executed by a human in a real terminal
session against a debug build. It contains no automated test code and no reference to
any test harness.

Two kinds of case appear below:

- **Agent-driven cases** type a natural-language prompt into the TUI input and
  observe what the *agent* does with the sub-agent tools. These cases assert the
  tool **contract** (what the tool reports, what lands in the log panel, what the
  model then does with it).
- **Operator-driven cases** are executed directly by the tester with keys and slash
  commands, asserting the **panel and command** behaviour.

Where a case asserts a reported number equals a configured number, the
configuration is pinned in **Prerequisites** so the expected value is deterministic.

Keys are written as `Alt+A`, `Down`, `Space`, `Del`, `Enter`, `Esc`, and literal
characters to type are written in `backticks`. "Status line" means the line
immediately above the input area. "Log panel" means the scrollback message pane.

## Prerequisites

1. **Build the binary.** `cargo build` in the repository root, then run the
   workspace binary. Confirm the version string matches `CHANGELOG.md`'s newest
   entry.
2. **Configure a model.** A working provider is required (a local Ollama model is
   sufficient, because no case depends on answer quality). Confirm by sending `hello`
   and receiving a reply before starting TC-001.
3. **Pin the sub-agent limit.** Edit `~/.config/ragent/ragent.json` (or the profile
   in use) and set:

   ```json
   { "experimental": { "max_background_agents": 2 } }
   ```

   The default is 8; TC-006, TC-007 and TC-008 assert reported numbers equal this
   value, so it must be `2` for those cases. Restart the TUI after editing.
4. **Pin the sub-agent stall timeout** high enough that no case trips it:

   ```json
   { "experimental": { "max_background_agents": 2 },
     "agent": { "stall_timeout_secs": 600 } }
   ```

   If the profile in use has no `agent.stall_timeout_secs` key, leave it absent -
   the important property is only that a task spawned in TC-002 does not time out
   during the observation window (60 s or less in every case).
5. **Prepare a scratch root.** Create `~/scratch/agentssetup-tests/` and run every
   session from inside it, so any `log/subagents/*.md` artefacts land under the
   scratch root rather than in a real project.
6. **Confirm the Agents window has a keyboard chord.** This plan assumes T-019 has
   landed, so `Alt+A` opens the Agents window. If `Alt+A` types an `a` into the
   input instead, record that as a TC-011 failure and use `left-click the Agents
   button beside the message input` for the remaining panel cases.
7. **Know how to make a slow sub-agent.** The reliable prompt is
   `spawn an explore agent to read every markdown file under docs/ and list every
   heading`. On a repository with a large `docs/` tree this takes long enough to
   observe and cancel. For a *guaranteed* slow task on any tree, use
   `spawn an explore agent to run find / -name "*.rs" 2>/dev/null and count the
   lines` - never finish it, always cancel it.

## Test Cases

### TC-001 - `detached` without `background` is refused, not degraded

**Preconditions:** Fresh session in `~/scratch/agentssetup-tests/`. Limit pinned to
`2` (Prerequisites 3). No sub-agents running.

**Steps:**
1. Start the TUI.
2. Type the prompt below and press `Enter`.
3. Wait for the agent's turn to finish.
4. Open the log panel and locate the `new_agent` tool call row.
5. Read the tool result text for that call.

**Test data:**
`Call the new_agent tool exactly once with agent set to "explore", task set to "list the files in src/", and detached set to true but do NOT pass background. Report the tool result verbatim and nothing else.`

**Expected results:**
- The `new_agent` tool call is reported as a **failure**, not a success.
- The error text names both `detached` and `background` and states the two legal
  combinations (`background: true` with `detached: false`, or `background: true`
  with `detached: true`).
- No sub-agent task starts: the Agents button count does not increase, and no
  `SubagentStart` line appears in the log.
- The turn does not block for the duration of an `explore` run - the refusal is
  immediate.
- **Regression guard:** before this change the same call ran a *foreground* task and
  reported success. Any success result here is a failure of FR-003.

*Automated coverage (T-027, FR-003):*
`tests/inline/new_agent_tests.rs::test_new_agent_refuses_detached_without_background`
drives `execute` with `detached: true` and no `background` against a context with
no `AgentManager`, and asserts the refusal names both parameters, states both legal
combinations (`background: true, detached: false` and `background: true,
detached: true`) and names the omit-both foreground shape. Because the context has
no manager, a call that degraded to a foreground task would fail with the manager
message instead - so the assertion also pins that the contract check runs *before*
the environment check, which is what makes the refusal immediate.

### TC-002 - An unknown parameter is rejected by name

**Preconditions:** As TC-001.

**Steps:**
1. Type the prompt below and press `Enter`.
2. Find the `new_agent` tool call in the log panel.
3. Read the tool result.

**Test data:**
`Call the new_agent tool with agent "explore", task "list the files in src/", and backgound set to true (note the missing r). Report the tool result verbatim.`

**Expected results:**
- The call fails with an error naming the offending key exactly as written:
  `backgound`.
- The error does not say "Missing required parameter" - both required parameters were
  supplied.
- No sub-agent starts (the Agents button count is unchanged).
- Repeat the same prompt once with `timeout` misspelled as `timeout_sec` and confirm
  the same rejection shape.

*Automated coverage (T-027, FR-002, FR-004):*
`tests/test_tool_param_schema.rs::test_tc002_new_agent_rejects_the_manual_test_spelling`
asserts both manual-test spellings (`backgound`, `timeout_sec`) are rejected naming
the key exactly as written, and that neither refusal is a missing-parameter error.
The same file covers each family tool plus `bg`
(`test_new_agent_refuses_unknown_property`, `test_cancel_agent_refuses_unknown_property`,
`test_list_agents_refuses_unknown_property`, `test_wait_agents_refuses_unknown_property`,
`test_bg_refuses_unknown_property`), so the shared validator is exercised through
every tool that declares `additionalProperties: false` rather than once in isolation.

### TC-003 - The three execution contracts are distinguishable in `list_agents`

**Preconditions:** As TC-001.

**Steps:**
1. Type the first prompt below and press `Enter`; wait for the turn to finish.
2. Type the second prompt and press `Enter`; wait for the turn to finish.
3. Open the log panel and compare the two `new_agent` results.
4. Ask for a listing (third prompt) and read its table.

**Test data:**
1. `Call new_agent with agent "explore", task "list the files in src/", background true, detached false. Report the task id.`
2. `Call new_agent with agent "explore", task "list the files in tests/", background true, detached true. Report the task id.`
3. `Call list_agents with no parameters and show me the full result.`

**Expected results:**
- Call 1 returns a task id and the text says the result **will be** injected into the
  session when it completes.
- Call 2 returns a task id and the text says the task is **detached** - it will not
  appear in `list_agents`, cannot be awaited, and its result will not be injected.
- The `list_agents` table in call 3 contains **one** row (call 1's task) and does
  **not** contain call 2's task id.
- Both results name the contract in words a reader can tell apart; neither says only
  "background task spawned".

### TC-004 - The four tool descriptions name every parameter they accept

**Preconditions:** As TC-001. Requires T-025 to have landed.

**Steps:**
1. Type the prompt below and press `Enter`.
2. Read the `tool_info` output for the four sub-agent tools.

**Test data:**
`Call tool_info and show me the description and parameters for new_agent, cancel_agent, list_agents and wait_agents, exactly as returned.`

**Expected results:**
- `new_agent`'s description mentions `agent`, `task`, `background`, `detached`,
  `model` and the per-call time bound parameter by name.
- `cancel_agent`'s description mentions `task_id`.
- `list_agents`' description mentions `status` and `task_id`.
- `wait_agents`' description mentions `task_ids` and `timeout_secs`.
- No description mentions a parameter that is absent from that tool's schema.
- `new_agent`'s description states the `model` spellings (`provider/model` and
  `provider:model`) and names one as preferred (FR-006).

*Automated coverage for the description/schema contract (T-025):*
`test_tool_descriptions::every_declared_property_is_named_in_the_description` reads
each of the five guarded tools' own `parameters_schema()` (`new_agent`,
`cancel_agent`, `list_agents`, `wait_agents`, `bg`) and fails if any declared
property name is absent from that tool's `description()`;
`::no_description_advertises_a_property_the_schema_rejects` checks the converse -
every identifier the description presents as a parameter (the `PARAMETERS:` bullets,
or `bg`'s "Optional parameters include ..." list) is declared by the schema, because
`additionalProperties: false` would otherwise turn the advertised call into a
guaranteed refusal; `::the_descriptions_carry_the_parameters_tc_004_requires` pins
this case's parameter list from the requirement side, so a schema rename that kept
both sides self-consistent still fails;
`::the_descriptions_name_the_execution_contracts_they_support` pins "every execution
contract they support" - `new_agent` names foreground, background and detached and
states that `detached` without `background` is refused, while `cancel_agent` states a
cancel is *requested*, and `list_agents` / `wait_agents` say a detached task is
neither listed nor awaitable;
`::new_agent_description_states_the_model_spellings_and_the_preference` pins FR-006 in
both the description and the `model` property's own schema description;
`::bg_description_keeps_the_shell_task_vocabulary` pins FR-019's boundary (it says
"shell task" and never "background task"). The suite is a live drift guard: deleting
the name `` `task_id` `` from `cancel_agent`'s description fails both
`every_declared_property_is_named_in_the_description` (naming the missing property)
and `the_descriptions_carry_the_parameters_tc_004_requires`.

*Automated coverage for G1 (T-027):* G1 recorded that
`create_default_registry`'s rustdoc listed `new_agent`, `cancel_agent` and
`list_agents` but not `wait_agents`, so the one tool a caller needs to await a
background task was absent from the registry's own documentation.
`crates/ragent-agent/tests/test_default_registry_doc_lists_the_family.rs` reads the
doc block out of `src/tool/mod.rs` with `include_str!` (scanning upward from the
`pub fn create_default_registry` signature, so an unrelated edit cannot move the
target) and asserts every sub-agent family tool the registry actually registers is
named, with `wait_agents` asserted separately so a regression fails naming the drift.
`::test_registry_doc_describes_the_family_contract` additionally requires the block
to describe the family as one contract - naming the execution contract, the detached
contract, FR-044 and FR-013 - because G1's deeper defect was that a bare tool list
gave a reader no way to choose between the four tools.

### TC-005 - Permission categories distinguish spawn, control and read

**Preconditions:** As TC-001.

**Steps:**
1. Type the prompt below and press `Enter`.
2. Read the permission category reported for each tool.

**Test data:**
`Call tool_info and tell me the permission category of new_agent, cancel_agent, list_agents and wait_agents.`

**Expected results:**
- `new_agent` → `agent:spawn`.
- `cancel_agent` → `agent:control`.
- `wait_agents` → `agent:control`.
- `list_agents` → a read-only agent category. It must **not** be `none` (FR-005).
- **Regression guard:** the four tools must produce three distinct categories, not
  the previous `agent:spawn`, `agent:spawn`, `none`, `agent:control` mapping.

### TC-006 - `list_agents` reports the limit and headroom

**Preconditions:** Fresh session, limit pinned to `2`, no sub-agents running, no
shell tasks running.

**Steps:**
1. Type the first prompt below and press `Enter`.
2. Read the `list_agents` result in the log panel and note the three numbers.
3. Start one slow sub-agent (see Prerequisites 7) in the background.
4. Ask for the listing again (second prompt).
5. Read the three numbers again.

**Test data:**
1. `Call list_agents with no parameters and report the limit, the occupancy and the headroom it gives you.`
2. `Call list_agents again and report the limit, the occupancy and the headroom.`

**Expected results:**
- After step 2: limit = `2`, occupancy = `0`, headroom = `2`.
- After step 5: limit = `2`, occupancy = `1`, headroom = `1`.
- The three numbers also appear in the tool result **metadata**, not only in the
  human-readable content (FR-011).
- The `status`-filtered variant composes with the report: ask
  `Call list_agents with status "completed" and report the limit, occupancy and headroom.`
  and confirm the three numbers still describe the whole session (limit `2`,
  occupancy `1`, headroom `1`) even though the filtered list is empty (FR-035).

*Automated coverage for the limit report (T-012):*
`test_list_agents_tool::limit_occupancy_and_headroom_are_reported` pins the idle
(`2/0/2`) and occupied (`2/1/1`) triples in **both** the metadata and the content
line and checks the occupying task id rides in the metadata;
`::limit_report_composes_with_a_status_filter` pins FR-035 - a `status:
"completed"` filter whose listing is empty still reports `2/1/1` and keeps its
`no_tasks` reason; `::detail_mode_reports_the_limit_on_hit_and_miss` pins the same
triple on detail mode's two paths. The report itself is pinned by
`test_task_registry_bookkeeping::limit_report_counts_running_and_suspended_background_tasks`
(`running` and `suspended` both occupy, `detached` / foreground / finished never
do, and headroom saturates at zero), and FR-029's headroom-in-the-refusal half by
`::spawn_refusal_names_limit_occupancy_and_occupying_tasks`, which reads the code
back through `reason_in` and asserts the limit, the occupancy and both occupying
task ids appear in the error.

### TC-007 - A limit refusal names the occupancy and the occupying tasks

**Preconditions:** Fresh session, limit pinned to `2`, no sub-agents running.

**Steps:**
1. Start two slow sub-agents one after the other (Prerequisites 7, two different
   prompts so both stay live).
2. Confirm via `/list_agents`-style prompt that occupancy is `2`.
3. Attempt a third background spawn with the prompt below.
4. Read the third call's tool result.
5. Cancel one of the two live tasks from the Agents window: press `Alt+A`, then
   `Down` to select a live row, then `Del`, then `Enter` to confirm.
6. Wait for the row to leave the live section.
7. Attempt the third spawn again.

**Test data:**
3. `Call new_agent with agent "explore", task "list the files in docs/", background true. Report the full tool result verbatim.`

**Expected results:**
- Step 4: the spawn **fails** with an error naming the limit (`2`), the current
  occupancy (`2`) and at least one occupying task id (FR-029).
- The error carries a machine-readable reason code identifying the limit condition
  (FR-010) - readable in the tool result metadata.
- Step 7: the same spawn now **succeeds** and returns a task id.
- No partial state is left behind by the refused call: the Agents button count is `2`
  before step 5 and `2` after step 7 (one cancelled, one new).

*Automated coverage for the headroom half (T-012):*
`test_task_registry_bookkeeping::spawn_refusal_names_limit_occupancy_and_occupying_tasks`
drives the refusal with two occupying tasks (one `running`, one `suspended`) and
asserts the error carries `limit_reached` **and** names the limit, the occupancy,
the headroom and both occupying task ids, so a refusal that reverts to "limit
reached, wait for a running task" fails the build.

*Automated coverage for the reason-code half (T-003):* every code listed for the
family in `SPEC.md` is pinned by a test that reads it back through the shared
extractor (`tool::param_schema::reason_in`) rather than by matching prose, so a
refusal that reverts to an uncoded `anyhow!` message fails the build -
`new_agent_tests::test_new_agent_refusals_carry_reason_codes` (contract, timeout,
manager, team context) and `new_agent_tests::test_registry_refusal_keeps_its_reason_code`
(the registry's `limit_reached` survives the lift, and an uncoded error is left alone);
`test_list_agents_tool::refusals_carry_reason_codes` (status filter, manager, and the
`task_not_found` result metadata); `test_subagent_tool_reasons::test_cancel_agent_names_a_missing_manager`,
`::test_cancel_agent_reports_an_unknown_task` and `::test_wait_agents_reason_codes`;
`test_tool_param_schema::test_bg_refusals_carry_reason_codes`; and
`param_schema_tests::test_reason_code_round_trip` /
`::test_with_reason_attaches_to_metadata` for the extractor itself.

### TC-008 - Suspended tasks still occupy the limit

**Preconditions:** Fresh session, limit pinned to `2`, no sub-agents running.

**Steps:**
1. Start two slow background sub-agents (limit is now full).
2. Press `Alt+A` to open the Agents window.
3. Press `Down` until the cursor is on the first live sub-agent row.
4. Press `Space` to suspend it.
5. Confirm the row's status reads suspended and its action glyph shows the resume
   form.
6. Close the window with `Esc`.
7. Attempt a third background spawn with the prompt below.
8. Read the result.
9. Reopen `Alt+A`, select the suspended row, press `Space` to resume it, close with
   `Esc`.

**Test data:**
7. `Call new_agent with agent "explore", task "list the files in assets/", background true. Report the full tool result verbatim.`

**Expected results:**
- Step 5: the row status reads suspended (parked), not running and not finished.
- Step 8: the spawn **fails** with the limit error, naming occupancy `2` - the
  suspended task is still counted (FR-030).
- The error text or the accompanying guidance names cancellation (not suspension) as
  the supported way to free a slot.
- Step 9: the resumed row returns to running and its elapsed time continues.
- **Regression guard:** if step 8 succeeds, FR-030 has not landed. Record this and
  note that the previous behaviour (suspend frees a slot) was the deliberate change.

*Automated coverage (T-013):* `test_task_suspend_resume::suspended_task_still_occupies_the_limit`
drives the real transition against an `AgentManager` (`suspend_task`, not a seeded
`Suspended` entry) and then reads all three surfaces that must agree (FR-011): the
entry is `Suspended` through `get_task`, `running_background_count()` is still `1`, and
`limit_report()` reports `(limit 1, occupancy 1, headroom 0)` naming the suspended task
in `occupying_ids`. It then asserts the spawn refusal a caller actually sees carries
`reason_code: limit_reached`, names the suspended task among the occupying tasks and
says `suspending does not free a slot` - so "cancel, do not suspend" is stated in the
refusal and not only in the prose (step 8's expected results, FR-029). Finally it
resumes the task and asserts occupancy is unchanged, so the slot moves with the task
status in both directions. `test_ta[REDACTED]::limit_report_counts_running_and_suspended_background_tasks`
covers the seeded-status half (including `detached`, foreground and finished tasks
never occupying) and `::spawn_refusal_names_limit_occupancy_and_occupying_tasks` the
refusal wording.

### TC-009 - Cancelling a running task reports the state it is actually in

**Preconditions:** Fresh session, one slow background sub-agent running.

**Steps:**
1. Start one slow sub-agent (Prerequisites 7) in the background.
2. Note its task id from the tool result.
3. Type the prompt below and press `Enter` immediately - the task must still be
   running.
4. Read the `cancel_agent` tool result.
5. Immediately ask for the listing (second prompt) and read the row for that task.
6. Wait up to 30 seconds and ask for the listing again.

**Test data:**
3. `Call cancel_agent for the task id you just spawned and report the full tool result verbatim. Do not wait for the task to finish first.`
5. `Call list_agents and show me the row for that task id.`
6. `Call list_agents again and show me the row for that task id.`

**Expected results:**
- Step 4: the result says the cancel was **requested** and reports the task's
  **current** status alongside it. It does **not** claim the task is already
  `cancelled` when the registry still shows it running (FR-013).
- The result metadata carries both a `cancel_requested: true` flag and the real
  `status` value.
- Step 5: the row may still read running, terminating or cancelled - any of the three
  is acceptable, because the cancel is cooperative and lands at a step boundary.
- Step 6: the row reads `cancelled` and stays `cancelled`.
- At no point does the tool result claim a terminal status the listing contradicts
  in the same turn.

*Automated coverage (T-009):* `test_cancel_agent_status` drives the tool's `execute`
against a real `AgentManager`. `cancel_reports_the_current_status_not_a_cancellation`
asserts `cancel_requested: true` and that `status` equals the entry's status read back
through the same `get_task` the tool uses, and explicitly asserts it is **not**
`cancelled` (FR-013). `cancel_of_a_finished_task_reports_its_real_status` lets a
spawned task settle (which drops its cancel flag), then asserts the rejection reports
`reason: cancel_failed`, `cancel_requested: false` and the task's terminal status in
`status` plus `already <status>` in the content (FR-040).
`every_status_cancel_agent_emits_is_the_canonical_vocabulary` asserts no status is a
hard-coded literal in the tool body, so every reported value comes from
`TaskStatus::as_str()` (FR-007).

### TC-010 - `/cancel` delegates to the registry and refuses ambiguity

**Preconditions:** Fresh session, two slow background sub-agents whose task ids share
a common prefix (both `explore-...`, which they will by construction).

**Steps:**
1. Start two slow `explore` sub-agents with different prompts.
2. Note both task ids from the two tool results; write down the first four characters
   they share (expected: `expl`).
3. Type `/cancel expl` and press `Enter`.
4. Read the status line and the log entry.
5. Confirm via prompt that **both** tasks are still running.
6. Type `/cancel <full id of the first task>` (paste the whole id) and press `Enter`.
7. Wait up to 30 s and confirm the task reached a terminal status.
8. Type `/cancel zzzzzz` and press `Enter`.

**Test data:**
3. `/cancel expl`
6. `/cancel explore-<full id copied from the tool result>`
8. `/cancel zzzzzz`

**Expected results:**
- Step 4: `/cancel expl` is **refused** as ambiguous: the message lists the matching
  task ids and asks for a longer prefix (FR-024). The status line does not read
  "Cancelled task ...".
- Step 5: both tasks are still running - the ambiguous command cancelled nothing
  (FR-023, FR-041).
- Step 7: after the full-id cancel, the task reaches `cancelled` and its live row
  leaves the Agents window.
- Step 8: `/cancel zzzzzz` reports no match **and lists the live task ids**
  (FR-041). It does not merely echo the prefix.
- **Regression guard:** if step 4 removes a row from the Agents window while both
  tasks are still running, the pre-fix UI-local behaviour is still present.

*Automated coverage for the `/cancel` delegation contract (T-018):*
`cancel_refuses_an_ambiguous_prefix_and_lists_the_matches` (FR-024 - two live tasks
under one prefix are refused, both named, neither cancelled),
`cancel_reports_every_matching_id_in_the_ambiguous_refusal` (FR-024 - three matches
each listed with id, agent name and status), `cancel_names_the_live_ids_when_no_task_matches`
and `cancel_says_there_are_no_live_tasks_when_the_mirror_is_empty` (FR-041 - the
unmatched branch lists the live ids, and says so plainly when there are none),
`cancel_does_not_list_finished_tasks_as_live` (FR-041 - `completed` / `cancelled` /
`timed_out` rows are never offered as cancel targets), `cancel_requests_through_the_registry_and_keeps_the_row`
(FR-023 - a seeded `AgentManager` entry's cancel flag is set, and the row survives
until the registry reports the transition), and `cancel_reports_the_registry_status_of_a_finished_task`
(FR-023 - a `completed` task reports `is already completed` rather than a
cancellation). `test_slash_arms_deep::cancel_targets_benchmark_flag_and_active_task`
keeps the benchmark-first dispatch covered.

### TC-011 - The Agents window opens from the keyboard

**Preconditions:** Fresh session, at least one sub-agent or shell task present so the
window has a subject.

**Steps:**
1. Start one slow sub-agent.
2. Press `Alt+A`.
3. Observe the window.
4. Press `Esc`.
5. Press `Alt+A` again.
6. With no sub-agent and no shell task present (wait for the task to finish), press
   `Alt+A` while the window is closed.

**Test data:** No typed data; keys only (`Alt+A`, `Esc`, `Alt+A`).

**Expected results:**
- Step 3: the Agents window opens and its footer legend ends with `Esc close`
  (FR-022).
- Step 4: `Esc` closes it and nothing else changes.
- Step 5: the same chord opens it again - the chord is a toggle.
- Step 6: with nothing to show, the open is a **no-op** and the status line says why
  (e.g. no sub-agents or shell tasks), rather than opening an empty modal (FR-042).
- The mouse button beside the input still opens the same window (both paths reach the
  same toggle).

**Automated coverage (T-023):** step 6's refusal is pinned offline in
`test_agents_window_refusals.rs`, against the same `App::toggle_agents_window`
the chord and the Agents button both call:

- `::opening_an_empty_agents_window_names_the_reason` pins the no-op *and* the
  reason: the window stays closed, the status line equals
  `AGENTS_WINDOW_EMPTY_REFUSAL`, and `needs_redraw` is untouched (a refusal
  paints nothing).
- `::the_refusal_names_the_condition_and_a_next_action` pins NFR-005 on the new
  message: it names the condition, both kinds of subject (sub-agent, shell
  task) and a supported next action (`/spawn`).
- `::a_repeated_refused_open_still_reports_the_reason` pins that an operator who
  presses the chord twice is told twice - "nothing happened" is the exact
  confusion FR-042 removes.
- `::a_refused_toggle_while_the_window_is_open_says_nothing` pins the scope:
  only a refused *open* is named, so a close request never overwrites the
  status line.
- `::an_available_window_opens_without_a_refusal` pins the negative: a real
  open prints no refusal, so the message cannot become noise on every open.
- `::a_completed_history_row_keeps_the_window_openable` pins the interaction
  with FR-036: a finished run that now lives only in the completed-history
  section is still a subject, so the panel reopens and the FR-032 note in that
  row stays readable. FR-063 is unaffected - the window still closes with its
  last *live* row (`close_agents_window_when_empty` tests
  `agents_window_has_live_subject`).

The step-3/4/5 chord toggle, the footer legend and the mouse path remain manual
(`test_alt_a_agents_window.rs` and `test_window_routing.rs` cover the routing
they exercise). The `Alt+T` / Teams variant of steps 2-6 is TC-023.

### TC-023 - `Alt+T` opens the Teams window, and an open window always closes

**Preconditions:** Fresh session. For the open cases, at least one team member (a
team with a lead and one teammate); for the refusal case, no team at all.

**Steps:**
1. With no team created, press `Alt+T` while the Teams window is closed.
2. Create a team with one teammate, then press `Alt+T`.
3. Observe the window, then press `Alt+T` again.
4. With the Teams window open and the team since cleaned up, press `Alt+T`.
5. Repeat steps 1-4 for the Agents window with `Alt+A`.
6. Confirm `Alt+I` still opens the Tasks side panel (the `Alt+T` slot is free).

**Test data:** No typed data; keys only (`Alt+T`, `Alt+A`, `Alt+I`).

**Expected results:**
- Step 1: the open is a **no-op** and the status line names the reason
  (`TEAMS_WINDOW_EMPTY_REFUSAL`), rather than opening an empty modal (FR-042,
  FR-046).
- Step 2: the Teams window opens; its footer legend ends with `Esc close`.
- Step 3: the same chord closes it - the chord is a toggle.
- Step 4: an **open** window closes unconditionally, even when there is no
  subject left to reopen it (FR-046). A close request is never refused.
- Step 5: the Agents window behaves identically with `Alt+A`
  (`AGENTS_WINDOW_EMPTY_REFUSAL` for the empty open).
- Step 6: `Alt+I` is unaffected, so the Tasks panel and the Teams window are
  separate chords.

**Automated coverage (T-028):**

- `test_alt_t_teams_window.rs` pins the chord: `alt_t_resolves_to_the_teams_toggle_action`
  and `alt_t_is_bound_regardless_of_window_availability` fix the mapping and its
  independence from subject state; `alt_t_toggles_the_window_and_requests_a_redraw`
  pins the toggle; `alt_t_without_a_team_names_the_refusal_and_stays_closed` pins
  the empty open (status line equals `TEAMS_WINDOW_EMPTY_REFUSAL`, `needs_redraw`
  untouched - a refusal paints nothing, NFR-005);
  `the_teams_refusal_names_the_condition_and_a_next_action` pins NFR-005 on the
  wording; `alt_t_closes_an_open_window_even_when_its_team_vanished` pins the
  close-always arm (FR-046).
- Mutual exclusion is pinned by `alt_t_opening_the_teams_window_closes_the_agents_window`
  and `alt_a_and_alt_t_alternate_without_ever_showing_both`, so the two chords are
  one modal slot rather than two windows.
- `test_alt_a_agents_window.rs` pins the mirrored `Alt+A` behaviour:
  `alt_a_without_a_subject_names_the_refusal_and_stays_closed` and
  `alt_a_closes_an_open_window_even_when_its_subject_vanished`.
- `test_agents_window_refusals.rs::a_refused_toggle_while_the_window_is_open_says_nothing`
  pins the negative scope: that state is now reached only by a vanished subject
  while open, and the close arm never writes a refusal.
- `test_window_routing.rs` pins the mouse path through the same toggle:
  `teams_button_click_without_a_team_is_refused_and_named` and
  `teams_button_click_closes_an_open_window_even_when_the_team_is_gone`, so the
  button and the chord cannot diverge on the close-always rule (FR-042, FR-046).
- `Alt+I` remaining bound to the Tasks side panel is covered by
  `test_input_more.rs` (the `('i', "ToggleTasksPanel")` binding row) and
  `test_input_queue_menu_binding.rs`; the `Alt+T` move did not disturb it.

### TC-012 - A killed row disappears from the event, and the panel is honest about it

**Preconditions:** Fresh session, one slow background sub-agent running.

**Steps:**
1. Start one slow sub-agent.
2. Press `Alt+A` to open the Agents window and confirm the row is live.
3. Press `Down` to select the row.
4. Press `Del` to arm the kill confirmation.
5. Press `Enter` to confirm.
6. Watch the row for 3 seconds without taking any action.
7. Close the window with `Esc` and confirm via prompt that the task is not running.

**Test data:**
7. `Call list_agents and tell me the status of the task you spawned.`

**Expected results:**
- Step 5: the row enters a distinct terminating state - it is not painted as a plain
  running row, and the Agents button count no longer includes it as live (FR-031).
- Step 6: the row leaves the window from the kill event, not after a multi-second
  reconciliation wait (FR-026). If the row visibly lingers for more than about two
  seconds, record the observed delay.
- Step 7: the listing reports the task's terminal status (`cancelled` or `failed`
  with a force-kill cause). It does not report it as still running.
- A cancelled task's reported status is distinguishable from a failed one (FR-014):
  compare against TC-014's failure case if both have been run.

### TC-013 - Cancellation and failure are distinguishable in `wait_agents`

**Preconditions:** Fresh session. Two slow sub-agents: one to be cancelled, one to be
made to fail.

**Steps:**
1. Start two slow sub-agents (Prerequisites 7).
2. Cancel the first one with `Alt+A`, `Down` to it, `Del`, `Enter`, `Esc`.
3. Force the second to fail: ask the agent to spawn a sub-agent against a nonexistent
   model, using the prompt below.
4. Wait for both to settle, then ask for a combined wait (third prompt) and read the
   result.
5. Read the tool result metadata for the per-task entries.

**Test data:**
3. `Call new_agent with agent "explore", task "say hello", background true, model "no-such-provider/no-such-model". Report the tool result.`
4. `Call wait_agents with task_ids set to both task ids and timeout_secs set to 120, and show me the full result including metadata.`

**Expected results:**
- The cancelled task's entry reports `status: "cancelled"`.
- The failed task's entry reports `status: "failed"`.
- The two entries are **not** both reported as `failed` (FR-014).
- The human-readable section renders a different marker for each: a cancelled task
  does not carry the error text of a failed one.
- If a task is still parked (suspended) when the wait times out, its entry reports
  `status: "suspended"` and the summary does not call it "still running" (FR-015).

**Automated coverage (FR-044):**
`crates/ragent-agent/tests/test_wait_agents_detached_refusal.rs` pins the detached
refusal exercised here, seeding the registry with a detached entry (the state a
`/spawn` leaves behind) because a detached report needs a live sub-agent run.
`::awaiting_a_detached_task_is_refused_by_contract` asserts the refusal names the
task as detached, names the contract as the spawn shape
(`background: true, detached: true`), names the task id, points at the durable
report file `log/subagents/<task-id>.md` with the `read` call that recovers it,
and contains neither "unknown" nor "not found";
`::the_detached_refusal_carries_a_reason_code` asserts the machine-readable
`task_detached`; `::a_detached_id_refuses_the_call_even_beside_an_awaitable_one`
asserts the whole call is refused and the awaitable task is left undisturbed;
`::a_refused_await_leaves_the_waiter_count_untouched` asserts the refusal counts no
waiter (FR-012); `::a_background_task_is_still_awaitable` and
`::a_detached_task_from_another_session_is_refused_too` fix the scope of the
refusal in both directions; `::the_description_states_the_detached_refusal` pins
the tool description (FR-016).

### TC-014 - A suspended task is reported as parked, not running

**Preconditions:** Fresh session, one slow background sub-agent running.

**Steps:**
1. Start one slow sub-agent.
2. Press `Alt+A`, `Down` to select the row, `Space` to suspend, `Esc` to close.
3. Ask for a wait on that task with a short timeout (prompt below).
4. Read the result.
5. Resume the task with `Alt+A`, `Space`, `Esc` and confirm it finishes or cancel it.

**Test data:**
3. `Call wait_agents with task_ids set to ["<the task id>"] and timeout_secs set to 20. Show me the full result including metadata.`

**Expected results:**
- The wait's partial-result section reports the task as `suspended` (parked), not as
  "still running" (FR-015).
- The metadata's `still_running_count` does not silently include the parked task as a
  running one; either it is reported separately or the reported status text makes the
  distinction clear.
- The result does not claim the task completed.

**Automated coverage:** `crates/ragent-agent/tests/test_wait_agents_suspended.rs`
(`test_wait_agents_reports_suspended_task_as_suspended`,
`test_wait_agents_unsettled_entries_carry_the_suspended_status`,
`test_wait_agents_keeps_a_running_task_out_of_the_suspended_set`,
`test_wait_agents_mixed_running_and_suspended_are_reported_separately`). The registry
is seeded with a `Suspended` background entry (the state `suspend_task` leaves behind)
because driving a real sub-agent to a parked state needs a live provider. The suite
asserts that `metadata.suspended` / `metadata.suspended_count` name the parked task,
that `metadata.still_running` / `metadata.still_running_count` never do, that
`metadata.unsettled[*].status` carries the canonical `suspended` label, that the
content says `suspended` and never "still running", and - in the other direction -
that a genuinely running task keeps the still-running wording and is not folded into
the suspended set. `completed_count` is asserted to be zero, so a parked task is never
reported as having completed.

### TC-015 - The panel paints the real execution contract, including detached

**Preconditions:** Fresh session.

**Steps:**
1. Start one **background** sub-agent via prompt.
2. Start one **detached** sub-agent via `/spawn`: type
   `/spawn explore list the files in src/` and press `Enter`.
3. Press `Alt+A` and read both rows.
4. Start one **foreground** sub-agent (a quick task) and watch the window while it
   runs.
5. Press `Esc`.

**Test data:**
1. `Call new_agent with agent "explore", task "list the files in src/", background true.`
2. `/spawn explore list the files in src/`

**Expected results:**
- Step 3: both rows are painted, and their execution contract is **visibly
  different**: the background row reads as background and the `/spawn` row reads as
  detached (FR-020).
- Neither row is painted with a hard-coded "background" contract that hides the
  detached flag (the pre-fix behaviour painted both identically).
- Step 4: the foreground row is marked as blocking the caller - its presence explains
  why other tool calls are not progressing (FR-033).
- The detached row's eventual completion does **not** produce a "result injected"
  section in the conversation.

*Automated coverage for the second half (T-020):*
`test_agents_window_table::test_type_column_paints_the_execution_contract` drives all
three contracts through the public render entry point and reads the type column back
(`fg` / `bg` / `detached`);
`test_agents_window_table::test_detached_badge_fits_the_type_column` pins the badge
inside its column so the elapsed-time column never moves;
`test_subagent_events::test_spawn_paints_the_contract_carried_by_the_event` and
`test_subagent_events::test_replayed_spawn_refreshes_the_contract` pin the other half -
the row takes both flags from `Event::SubagentStart` instead of hard-coding
`detached: false`;
`test_event_to_sse::test_subagent_start_carries_the_detached_flag` pins the flag's
journey to a remote consumer.

### TC-016 - Completed sub-agents remain inspectable in the panel

**Preconditions:** Fresh session, at least three quick sub-agents runnable.

**Steps:**
1. Run three quick background sub-agents one after another (e.g. `list the files in
   src/`, `list the files in tests/`, `list the files in docs/`), waiting for each to
   finish.
2. Press `Alt+A`.
3. Read the window: identify the live section and the completed section.
4. Read the entry for the most recently finished task.
5. Press `Down` through the completed rows and note their order.
6. Press `Esc`.
7. Confirm the report file exists: run `ls log/subagents/` in the session's working
   directory.

**Test data:** Three `new_agent` prompts with `background true`, one per directory.

**Expected results:**
- Step 3: the window contains a **completed** section separate from the live section
  (FR-021). Before this change the finished rows vanished and the window closed when
  it emptied.
- Step 4: the newest completed entry shows its final status, its duration and a
  `[file]` marker naming its `log/subagents/<task-id>.md` path.
- Step 5: completed entries are ordered newest first (FR-036).
- Step 7: the `ls` output lists one markdown file per completed task id.

### TC-017 - The completed history is bounded

**Preconditions:** A completed-history cap exists and is set low for the test: add
`"agents": { "completed_history": 3 }` to the profile (or use the documented
default of 20 and run enough tasks). Restart the TUI after editing.

**Steps:**
1. Run four quick background sub-agents, waiting for each to finish.
2. Press `Alt+A`.
3. Count the completed rows.
4. Read which completed tasks are present.
5. Press `Esc`.

**Test data:** Four `new_agent` prompts with `background true`, each listing a
different directory.

**Expected results:**
- Step 3: at most `3` completed rows are painted (FR-036, NFR-004).
- Step 4: the three **most recent** tasks are present; the first (oldest) one has
  been evicted.
- The window's memory use does not grow with the number of tasks ever spawned: run
  four more tasks and re-check the count stays at `3`.

### TC-018 - The `bg` tool keeps shell vocabulary and refuses a foreign session

**Preconditions:** Fresh session in `~/scratch/agentssetup-tests/`.

**Steps:**
1. Start a shell task via prompt (below) and note its task id.
2. Wait about 5 seconds and ask for its tail.
3. Ask for a `bg` list with an explicit foreign `session_id`.
4. Ask for a `bg` list with the caller's own `session_id`.
5. Ask for a `bg` cleanup with a foreign `session_id`.

**Test data:**
1. `Call bg with action "spawn" and command "sleep 30; echo done". Report the task id.`
2. `Call bg with action "tail", task_id "<id>", lines 5.`
3. `Call bg with action "list" and session_id "some-other-session-id". Show me the full result.`
4. `Call bg with action "list" and session_id "<this session's id>". Show me the full result.`
5. `Call bg with action "cleanup" and session_id "some-other-session-id". Show me the result.`

**Expected results:**
- Steps 1-2: the task is described as a **shell task**, never as a "background
  agent" or "sub-agent" (FR-019). The tail returns command output.
- Step 3: the foreign-session list is **refused** with an error naming the mismatch,
  consistent with `cancel_agent`'s cross-session refusal (FR-019).
- Step 4: the caller's own `session_id` behaves exactly like omitting it - the list is
  identical to a plain `{"action":"list"}` call (FR-037).
- Step 5: the foreign-session cleanup is **refused** and deletes nothing.
- The shell task does not appear in `list_agents` output (different id space, different
  registry) and its row in the Agents window sits under the shell-task section, not
  the sub-agent tree.

*Automated coverage (T-017):* `test_bg_tool::bg_tool_uses_shell_task_vocabulary`
asserts the tool description and the `spawn`/`list`/`cleanup` messages say "shell
task" and that none of them says "background task" (the sub-agent contract's
name); `::bg_tool_refuses_a_foreign_session_id` pins step 3 and step 5 - a foreign
`session_id` on `list` **and** `cleanup` is refused with `reason_code:
wrong_session`, the text names both the foreign id and the calling session, and
the caller's shell task is still listed afterwards, so the refused cleanup
deleted nothing; `::bg_tool_accepts_the_callers_own_session_id` pins step 4 - the
caller's own id returns a list **identical** to the one a plain
`{"action":"list"}` returns (metadata and content compared), and a `cleanup`
with it still deletes. Steps 1-2's tail output and the Agents-window placement
remain manual.

### TC-019 - Durable-copy absence is reported, not omitted

**Preconditions:** Fresh session in a directory where `log/subagents/` cannot be
created. Create the blockage first: run `touch log/subagents` (a *file* named
`subagents`) inside `~/scratch/agentssetup-tests/` so the directory cannot be made.

**Steps:**
1. Type `ls log/` and confirm there is a file (not a directory) named `subagents`.
2. Run one quick background sub-agent and wait for it to finish.
3. Ask for the listing and read the completed row.
4. Ask for the task's detail and read the `Output File` line.
5. Remove the blockage: `rm log/subagents`, then re-run the detail request.

**Test data:**
3. `Call list_agents and show me the full result including the output file path.`
4. `Call list_agents with task_id "<the task id>" and show me the full detail.`

**Expected results:**
- Step 3: the completed row states explicitly that the report is **in-memory only**
  and that no durable copy exists (FR-018, FR-043). It does not silently omit the
  path.
- Step 4: the detail view states the same condition - it does not print an `Output
  File` path that cannot be read.
- No tool claims a file was written when it was not (FR-043).
- Step 5: `rm log/subagents` removes the blocking file; a new task afterwards reports
  a real `log/subagents/<task-id>.md` path.

**Automated coverage (T-015):** the blocking filesystem (step 1) is replaced by the
registry state a failed `persist_task_output` leaves behind - a finished
`TaskEntry` with `output_file: None` - because the condition under test is the
*reporting*, and a real failure needs a live provider plus an unwritable directory.
`test_durable_copy_absence` drives both tools against that entry through the
`#[doc(hidden)]` seeding hook:

- `::list_mode_states_the_report_is_in_memory_only` pins step 3: the row prints
  `[memory]`, contains the shared `IN_MEMORY_REPORT_NOTE`, says "in-memory only",
  and contains no `Full report:` line (so no path is claimed - FR-043).
- `::detail_mode_states_the_report_is_in_memory_only` pins step 4: the detail view
  prints `Output File: none - ...` with the shared wording and never the
  `Output File (full untruncated report)` heading (FR-032).
- `::list_mode_names_the_status_that_lost_its_report` pins that the absence line
  names the status that finished without a copy (`cancelled`, `timed_out`), so the
  marker is not a generic "no file" that hides the outcome.
- `::wait_agents_reports_the_absence_in_content_and_metadata` pins FR-018/FR-043 on
  the await path: the content prints `[memory] No durable copy:`, and the
  `results` entry carries `report_persisted: false`, `report_location:
  "in_memory_only"` and `output_file` present-and-null (not omitted), with the
  `tasks` roll-up repeating the flag.
- `::a_persisted_report_gets_no_absence_note` and
  `::wait_agents_marks_a_persisted_report_as_durable` pin the negative case, so the
  note cannot become noise on every healthy task and cannot contradict a real path
  (step 5's post-fix shape).
- `::a_live_task_reports_no_absence_note` and
  `::wait_agents_says_nothing_about_an_unsettled_task_report` pin the scope: an
  in-flight (running or suspended) task has not had a write attempted, so neither
  surface claims one failed and no `unsettled` entry carries a persistence verdict.

The `touch log/subagents` precondition and the `rm log/subagents` recovery remain
manual, because a test must not depend on being able to make a path unwritable.

**Panel coverage (T-023):** the same absence condition as the Agents panel shows
it - the history row, which is where a finished task's report state survives once
its live row is gone - is pinned in `test_agents_window_refusals.rs`:

- `::the_panel_paints_the_absence_note_and_no_false_path` renders one frame with
  an `output_file: None` history row and asserts the row paints the `[memory]`
  flag plus the shared note text, and never paints a `.md` path (FR-032, FR-043).
- `::the_panel_paints_no_note_for_a_persisted_report` pins the negative: a row
  with a real `log/subagents/<id>.md` path carries no absence note, so the note
  cannot become noise on every healthy task nor contradict a real path.
- `::a_history_row_without_a_durable_copy_names_the_condition` and
  `::a_history_row_with_a_durable_copy_says_nothing` pin
  `CompletedTaskRow::durable_copy_absence` itself, including that the wording is
  `ragent_agent::tool::wait_agents::IN_MEMORY_REPORT_NOTE` - the one constant
  `list_agents` and `wait_agents` print, so the panel and the tools cannot
  describe the condition two ways.
- `::a_completed_task_is_snapshotted_before_its_live_row_is_removed` and
  `::a_cancelled_task_is_snapshotted_and_named_as_in_memory` pin FR-028's
  ordering: the snapshot (carrying `output_file`) is taken before the live row
  is dropped, which is what makes the panel's note possible at all.
- `::history_keeps_newest_first_and_evicts_beyond_the_cap` pins the FR-036 cap
  that keeps that section bounded (NFR-004).

### TC-020 - Awaited tasks report their waiter count

**Preconditions:** Fresh session, one slow background sub-agent running.

**Steps:**
1. Start one slow sub-agent and note its task id.
2. Ask the main agent to wait on it with a long timeout, using the prompt below.
3. While that wait is in flight (the agent is blocked), press `Alt+A` **if** the
   window can be opened while a turn runs. If it cannot, skip to step 5.
4. Read the row.
5. Press `Esc`, then ask for the detail listing after the wait returns.

**Test data:**
2. `Call wait_agents with task_ids set to ["<the task id>"] and timeout_secs set to 120, then report what you got.`
5. `Call list_agents with task_id "<the task id>" and show me the full detail including the waiter count.`

**Expected results:**
- Step 4: while the wait is in flight, the row reports one active waiter (FR-012).
- Step 5: after the wait returns, the waiter count is back to `0`.
- The waiter count appears only when it is greater than zero; a task nobody is waiting
  on does not display a `0` waiter column (FR-012).

**Automated coverage (T-014):** the count is driven through the real registry
tally (`AgentManager::increment_waiter` / `decrement_waiter`), not by writing the
field behind the implementation.

- `test_list_agents_tool::detail_mode_reports_the_waiter_count_of_an_awaited_live_task`
  - two increments then a suspend: the detail view prints `Active Waiters: 2` and
  the metadata carries `waiter_count: 2` (steps 2 and 5, plus the suspended-task
  case the panel paints).
- `::detail_mode_drops_the_waiter_count_after_the_wait_returns` - one increment,
  then the decrement the returning wait performs: the line is gone (step 5).
- `::detail_mode_omits_the_waiter_count_when_nobody_is_waiting` and
  `::list_mode_marks_only_the_awaited_live_task` - no `0` is ever printed, and a
  listing marks only the awaited row (the "greater than zero" clause).
- `::a_finished_task_never_reports_waiters` - a completed task's leftover tally
  is not painted as a live queue.
- `test_agents_window_table::an_awaited_live_row_paints_its_waiter_count`,
  `::an_unawaited_row_paints_no_waiter_marker`,
  `::a_suspended_awaited_row_still_paints_its_waiters` and
  `::the_waiter_marker_does_not_move_the_pinned_controls` - the Agents panel side
  of step 4, including that the marker never shifts `[stop]` / `[x]`.

### TC-021 - The refusal text is actionable, not a success

**Preconditions:** An active team. Create one: type
`create a team called audit-team with two teammates` and press `Enter`, then confirm
the teammate strip appears under the status bar.

**Steps:**
1. With the team active as lead, use the prompt below.
2. Read the `new_agent` tool result.
3. Attempt the same call from a teammate: send `team_message`-style instruction via
   the lead so a teammate attempts `new_agent`, then read what the teammate reports.

**Test data:**
1. `Call new_agent with agent "explore" and task "list the files in src/". Report the full tool result verbatim, including any error text.`

**Expected results:**
- Step 2: the call is reported as a **failure** whose message names the active team,
  this session's role (lead or teammate) and the supported alternative tools
  (`team_spawn`, `team_task_create`, `team_assign_task`, `team_message` for a lead;
  `team_read_messages`, `team_task_claim`, `team_task_complete`, `team_idle` for a
  teammate) - FR-009, FR-045.
- The result is **not** a successful output whose content is guidance prose. A success
  result here is a failure of FR-045.
- No sub-agent task starts: the Agents button count is unchanged.

### TC-022 - The system prompt teaches the whole contract

**Preconditions:** No team active (this case reads the prompt a *primary* agent
receives). A model configured and reachable.

**Steps:**
1. Render the primary agent's system prompt without spending a turn: type
   `/prompt` and press `Enter`.
2. Scroll the rendered prompt to the `## Sub-Agent Spawning` section and read it.
3. Read the `new_agent`, `cancel_agent`, `list_agents` and `wait_agents`
   descriptions in the same prompt/tool reference and compare the wording.

**Expected results:**
- Step 2: the section names all three execution contracts (`foreground`,
  `background`, `detached`) with the `background` / `detached` shape that selects
  each, and states that `detached: true` without `background: true` names no
  contract and is **refused**.
- The section states the configured background limit as a number (never the
  literal `MAX_BG_TASKS`), says occupancy counts `running`, `suspended` and
  `terminating` background tasks but never a detached or foreground one, and
  states the cancel-to-free rule: to free a slot, cancel - suspending frees no
  headroom.
- The section teaches the await contract: `task_ids` and `timeout_secs`, the
  canonical `completed` / `failed` / `cancelled` / `timed_out` statuses with
  cancelled never reported as failed, `suspended` not settled and `timed_out`
  settled, and a detached task never awaitable.
- The section names the durable report path `log/subagents/<task-id>.md`, the
  `output_file` field it is reported in, and the in-memory-only case when no
  durable copy exists.
- Step 3: the prompt and the four descriptions use the same words for the three
  contracts, the `limit` / `occupancy` / `headroom` triple, the suspend-frees-
  nothing rule and the status vocabulary - FR-025's vocabulary clause.
- Changing the agent profile to a sub-agent and running `/prompt` shows the
  completion protocol instead, with **no** `## Sub-Agent Spawning` block: the
  contract is taught to the agent that can spawn, not the one that cannot.

**Automated coverage:** `test_subagent_prompt_contract` runs these assertions
against the rendered prompt,
`::the_prompt_teaches_the_three_execution_contracts`,
`::the_prompt_teaches_the_limit_and_how_a_slot_is_freed`,
`::the_prompt_teaches_the_await_contract`,
`::the_prompt_teaches_the_durable_report_path`,
`::the_prompt_uses_the_same_vocabulary_as_the_tool_descriptions` (which reads the
*schemas* and *descriptions* rather than a hand-copied word list, so drift in
either direction fails the build) and
`::the_contract_block_is_scoped_to_primary_agents`. The limit-substitution half is
also pinned by rendering the prompt with a non-default
`experimental.max_background_agents`.

## Cleanup

Run these after the last case:

1. Cancel anything still live: open the Agents window with `Alt+A`, select each live
   row, press `Del` then `Enter`, and `Esc` when the live list is empty.
2. Stop any lingering shell task from TC-018:
   `Call bg with action "cleanup", older_than_minutes 0, completed_only false.`
3. Tear down the team from TC-021 if it is still active: `team_cleanup` for
   `audit-team`, or `/team cleanup`.
4. Remove the scratch tree: `rm -rf ~/scratch/agentssetup-tests/`.
5. Restore the pinned configuration: remove the `experimental.max_background_agents`
   override (and any `agents.completed_history` override added for TC-017) from the
   profile so the defaults (`8`, `20`) apply again.
6. Confirm no stray report files were written into a real project: run
   `ls log/subagents/` in the repository root. A test run must leave the working tree
   byte-identical - if files appeared there, record it as a leak and delete only those
   generated files.

## Coverage matrix

| Requirement | Case(s) |
|-------------|---------|
| FR-001 | TC-003 |
| FR-002 | TC-002 |
| FR-003 | TC-001 |
| FR-004 | TC-002 |
| FR-005 | TC-005 |
| FR-006 | TC-004 |
| FR-007 | TC-009, TC-013 |
| FR-008 | TC-003 (ids render without panic), TC-013 |
| FR-009 | TC-021 |
| FR-010 | TC-007, TC-009 |
| FR-011 | TC-006 |
| FR-012 | TC-020 |
| FR-013 | TC-009 |
| FR-014 | TC-013 |
| FR-015 | TC-014 |
| FR-016 | TC-004 |
| FR-017 | TC-001 (no long block), TC-006 |
| FR-018 | TC-019 |
| FR-019 | TC-018 |
| FR-020 | TC-015 |
| FR-021 | TC-016 |
| FR-022 | TC-011 |
| FR-023 | TC-010 |
| FR-024 | TC-010 |
| FR-025 | TC-022 (the prompt teaches the contracts, limit, await and report path), TC-003 |
| FR-026 | TC-012 |
| FR-027 | TC-009 |
| FR-028 | TC-016 |
| FR-029 | TC-007 |
| FR-030 | TC-008 |
| FR-031 | TC-012 |
| FR-032 | TC-019 (panel row: `test_agents_window_refusals.rs`) |
| FR-033 | TC-015 |
| FR-034 | TC-001 (immediate refusal), TC-006 |
| FR-035 | TC-006 |
| FR-036 | TC-017 |
| FR-037 | TC-018 |
| FR-038 | TC-002 |
| FR-039 | TC-018, TC-010 |
| FR-040 | TC-009 |
| FR-041 | TC-010 |
| FR-042 | TC-011 (automated by `test_agents_window_refusals.rs`), TC-023 (both chords, open and close) |
| FR-043 | TC-019 |
| FR-044 | TC-013 (detached task await refusal exercised as part of the combined wait; automated in `test_wait_agents_detached_refusal.rs`) |
| FR-045 | TC-021 |
| FR-046 | TC-023 (automated by `test_alt_t_teams_window.rs`, `test_alt_a_agents_window.rs`, `test_window_routing.rs`, `test_agents_window_refusals.rs`) |
| G1 | TC-004 (automated by `test_default_registry_doc_lists_the_family.rs`) |
| NFR-003 | TC-006 (no storage read, instant report) |
| NFR-004 | TC-017 |
| NFR-005 | TC-007, TC-010, TC-019, TC-021, TC-023 |

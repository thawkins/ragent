# Implementation Plan: Agent and Team Member Panel

**Spec:** [SPEC.md](SPEC.md) · **Test plan:** [TESTPLAN.md](TESTPLAN.md)

## Approach

The panel is a **pure projection** of state that already exists on `App`
(`active_tasks`, `bg_tasks`, `team_members`, `active_team`, `focused_teammate`,
`team_message_counts`, the event-bus tool-call counters and the on-disk team store).
No scheduling, spawning or execution code is added: every action delegates to
`AgentManager::suspend_task` / `resume_task` / `kill_task`,
`TeamManager::suspend_teammate` / `resume_teammate` / `shutdown_teammate` or
`BackgroundTaskService::cancel` (Constraint 1, FR-067, FR-068).

All work is confined to `crates/ragent-tui` (Constraint 2). The change splits into
five layers:

1. **Frame and chrome (`crates/ragent-tui/src/layout.rs`)** — the two mutually
   exclusive centred modals (`render_agents_window_overlay`,
   `render_teams_window_overlay`), their `Clear` underlay, borders, titles and clamped
   close buttons; the input-side button column (`draw_input_side_buttons`); and the
   single-row teammate strip (`render_teammate_strip`) (FR-001..FR-008, FR-017..FR-019,
   FR-027, FR-066, NFR-010).

2. **Table bodies (`crates/ragent-tui/src/layout_active_agents.rs`,
   `crates/ragent-tui/src/layout_teams.rs`)** — header rows, the primary/lead rows,
   the ancestor-indented sub-agent tree, the background shell section, the teammate
   rows with status/model/counter columns, and the empty state. Every structural and
   status glyph is declared once per renderer module (FR-009..FR-016, FR-020..FR-026,
   FR-062, NFR-003).

3. **Hit-rect bookkeeping (`crates/ragent-tui/src/app/state.rs`)** — the button
   hit-rect vectors and their parallel identifier vectors, the area rects, the scroll
   offsets and maxima, the selected index and the focused teammate. Rects are recorded
   during render with the painted line index in `Rect::y`, so scrolling shifts the
   click target by exactly the amount it shifts the paint (A3, A4, FR-056, NFR-011).

4. **Input routing (`crates/ragent-tui/src/app/input_handler.rs`,
   `crates/ragent-tui/src/input.rs`)** — button hit-tests, row hit-tests, the
   `Space` / `Shift+K` arms with their primary/lead refusals, `Alt+Up` / `Alt+Down`
   focus cycling, wheel scrolling over the popup body, and the destructive-action
   confirmation modal (FR-028..FR-046, FR-055..FR-059, FR-067, FR-070, FR-071).

5. **Event plumbing and reconciliation (`crates/ragent-tui/src/app/event_handler.rs`,
   `crates/ragent-tui/src/app/state.rs`)** — the sub-agent, teammate, counter and
   cleanup arms, the de-duplicating spawn handlers, and the 1500 ms registry
   reconciliation with its lag-triggered immediate pass (FR-047..FR-054, FR-060,
   FR-072, NFR-007, NFR-008).

### Panel layout

```text
+- Agents -----------------------------------------------------------------------+
|   id         name                        type      elapsed   steps  >[stop] [x]|
| * lead       general                     primary         -       7             |
| o a1b2c3d4   `- explore-a1b2c3d4         bg          2m14s      12  [stop]  [x]|
| o e5f6a7b8      `- rust-coder [C]        fg            41s       3  [stop]  [x]|
| o 99887766         `- reviewer [T]       bg             9s       0  [stop]  [x]|
|  Background shell tasks                                                        |
| o cc112233   cargo build --release       bg          1m02s       -  [stop]  [x]|
|                                                      Close                    |
+--------------------------------------------------------------------------------+

+- Teams ------------------------------------------------------------------------+
|   id       name                            status     model              elapsed|
| * a1b2c3d4 general                         active     lead                 3m02s|
| > |- e5f6a7b8 reviewer-e5f6a7b8               working    anthropic/claude     1m41s|
|  `- 99887766 tester-99887766                 idle       (inherited)            58s|
|  Team: alpha  (lead + 2 teammates)                                             |
+--------------------------------------------------------------------------------+
```

### Design decisions carried from the spec

- **Two windows, not one modal with tabs** (A1). Each has its own visibility flag; the
  open paths close the other, so the two can never be painted together.
- **Rows are projections** (A2). `active_tasks` is hydrated by events and reconciled
  against the registry; `team_members` is hydrated by events and additionally
  refreshed from the team store on render so a hydration race cannot pin a stale
  `spawning` status on screen (FR-060).
- **Buttons are painted text spans plus `Rect` click targets** (A3). The painted line
  index lives in the rect's `y`, so no button-order arithmetic is needed and scrolling
  composes for free.
- **Terminal rows write placeholders** (`Rect::default()`, empty identifier) rather
  than shrinking the vectors, so every hit-rect vector stays index-aligned with the
  painted rows (A4, NFR-011, FR-071).
- **Primary agent and lead are protected** (A5, FR-042, FR-044, FR-067). The refusal is
  reported in the status line and no action is dispatched.
- **Focus is a route, not a highlight** (A6, FR-058). `focused_teammate` changes the
  message destination and the input border title, not just the row colour.
- **Mouse and keyboard are equal** (A7). Every mouse action has the keyboard
  equivalent named in the same requirement group.
- **Destructive row actions confirm first.** The `[x]` controls (sub-agent kill,
  background shell cancel, teammate shutdown) and the teammate `[stop]` suspend route
  through a yes/no modal before dispatch (FR-036, FR-037, FR-040), reusing the shape of
  the existing queue-clear / memory-clear confirmation dialogs rather than inventing a
  new modal framework.
- **Teammate model column shows only a genuine override.** A member whose effective
  model equals the global selected provider/model renders as inherited, so the column
  does not repeat the status-bar label on every row (FR-024).

### Confirmation modal shape

```text
+- Confirm ----------------------------------------------------------------------+
|  Kill sub-agent `a1b2c3d4` (explore-a1b2c3d4)?                                 |
|                                                                                |
|                        [ Yes ]        [ No ]                                   |
+--------------------------------------------------------------------------------+
```

Opened by `[x]` on any live row, and by `[stop]` on a teammate that is not already
suspended. `Yes` dispatches the pending action on the async runtime; `No` and `Esc`
dismiss it and leave the row untouched. The pending action is held as a small enum on
`App` (target kind + identifier + action), so no closure or borrow is stored across
frames.

### Scrolling

`FR-008` requires the content area to be scrollable. The tables already clamp their
offsets and draw a scrollbar (FR-059); this plan adds the missing input: wheel events
over the popup body and the popup's table area move the offset by
`SCROLL_STEP_LINES`, the offset is re-clamped on every render, and every recorded
button hit-rect is shifted by the same offset (FR-056, §Interaction model, Open
Question 6).

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Input-side button column: Agents / Teams buttons, live counts, dimmed and active styles | FR-001, FR-002, FR-003, FR-004, FR-005, NFR-004 | M | Critical | completed | — |
| T-002 | Agents window frame: centred 90% x 70% modal, cleared underlay, cyan bold title, clamped Close button, table area arithmetic | FR-006, FR-007, FR-008, NFR-010 | S | Critical | completed | — |
| T-003 | Agents table body: header, primary row, ancestor-indented sub-agent tree, row content, `[C]`/`[T]` badges, status styling, `[stop]`/`[x]` columns, background shell section | FR-009, FR-010, FR-011, FR-012, FR-013, FR-014, FR-015, FR-016, FR-062, NFR-003, NFR-004 | L | Critical | completed | T-002 |
| T-004 | Teams window frame: modal geometry, blue bold title, Close button, `Team: <name> (lead + N)` table title, twelve-column header | FR-017, FR-018, FR-019, FR-020, NFR-010 | S | Critical | completed | — |
| T-005 | Teams table body: lead row, teammate rows with focus marker, status vocabulary, model column with global-model suppression, claimed/done/sent/recv counters, empty state | FR-021, FR-022, FR-023, FR-024, FR-025, FR-026, NFR-009, NFR-004 | L | Critical | completed | T-004 |
| T-006 | Teammate strip below the status bar: one pill per teammate, status icon, 16-column name, focused highlight, `Alt+^v:cycle` hint, omitted when no members | FR-027, FR-066 | S | High | completed | T-005 |
| T-007 | Window open/close routing: button toggles with mutual exclusion, no-op clicks when the subject is absent, Close-button dismissal, redraw signalling | FR-028, FR-029, FR-030, FR-031, FR-032, NFR-005 | M | Critical | completed | T-001, T-002, T-004 |
| T-008 | Row-click destinations: primary/sub-agent output view, teammate focus with routing and status text, lead-row focus clear, selection retention across a reopen | FR-033, FR-034, FR-038, FR-039, FR-058, FR-064, FR-065 | M | High | completed | T-003, T-005 |
| T-009 | Destructive-action confirmation modal: yes/no dialog for sub-agent kill, background-shell cancel, teammate shutdown and teammate suspend, with keyboard and mouse answering | FR-036, FR-037, FR-040, FR-069, FR-070, NFR-005 | M | High | completed | T-007 |
| T-010 | Row action dispatch: suspend/resume toggle, kill, background-service cancel, teammate shutdown, all dispatched off the render thread with warn-level attribution on failure | FR-035, FR-036, FR-037, FR-040, FR-068, NFR-008 | M | Critical | completed | T-009 |
| T-011 | Keyboard control: `Space` suspend/resume, `Shift+K` kill, primary protection with the documented status text, teammate focus keys, and window-key precedence over the input buffer | FR-041, FR-042, FR-043, FR-044, FR-045, FR-055, FR-067 | M | Critical | completed | T-010 |
| T-012 | `Alt+Down` / `Alt+Up` focus cycling through the member list, opening the focused teammate's output view and clearing focus past the ends | FR-046, FR-058 | S | High | completed | T-008 |
| T-013 | Sub-agent event plumbing: start appends, complete records report status and removes, cancel removes, suspend/resume re-status, duplicate spawns update in place | FR-047, FR-048, FR-049, FR-072 | M | Critical | completed | T-003 |
| T-014 | Team event plumbing: spawn hydration from the team store, idle/failed/claim/complete status updates with watchdog progress, sent/recv counters, cleanup reset, render-time hydration | FR-050, FR-051, FR-052, FR-053, FR-060, FR-072 | M | Critical | completed | T-005, T-006 |
| T-015 | Registry reconciliation safety net: 1500 ms poll plus lag-triggered immediate pass, registry fetch off the render thread, merge that removes non-running and adds unpainted running entries, poison-tolerant result lock | FR-054, NFR-007, NFR-008 | M | High | completed | T-013 |
| T-016 | Hit-rect lifecycle: index-aligned rect/identifier vectors, terminal-row placeholders, scroll-offset shifting, cleared areas when a side panel hides the surface, inert placeholder and stale-row clicks, blank popup cell consumption | FR-056, FR-057, FR-069, FR-070, FR-071, NFR-006, NFR-011 | M | Critical | completed | T-003, T-005 |
| T-017 | Scrolling: clamped offsets, vertical scrollbars on both tables, wheel scrolling over the popup body, offset composed into every hit-rect | FR-008, FR-056, FR-059, NFR-002 | M | High | completed | T-016 |
| T-018 | Window lifecycle and reporting: panel stays serviceable during a running turn, Agents window closes when its list empties, status-line reporting for refusals and focus transitions | FR-061, FR-063, FR-064, NFR-002, NFR-005 | S | Medium | completed | T-007, T-011 |
| T-019 | Offline deterministic test suite: synthetic-state render tests for both windows, the strip and every row variant, plus isolation from the repository working tree | NFR-001, NFR-002, NFR-012 | L | High | completed | T-003, T-005, T-016 |
| T-020 | Acceptance walk of the manual test plan and quality gates (`cargo fmt`, `cargo clippy`, `cargo check --workspace`) | Acceptance 1-20 | M | High | completed | T-001, T-002, T-003, T-004, T-005, T-006, T-007, T-008, T-009, T-010, T-011, T-012, T-013, T-014, T-015, T-016, T-017, T-018, T-019 |
## Task Details

### T-001 - Input-side button column

Render the `Agents` and `Teams` buttons inside the bordered column immediately left of
the message input (`draw_input_side_buttons`). The column is sized by
`input_side_buttons_total_width` so neither label truncates, and the input field
narrows rather than the labels (FR-001). The Agents label carries the sum of active
sub-agent tasks and background shell tasks, the Teams label the count of members that
are not `ShuttingDown` / `Stopped` / `Failed`; both fall back to the bare label at zero
(FR-002, FR-003). Dim both when their subject is absent (FR-004), and style an open
window's button white-on-blue bold (FR-005). Truncation, if any resize path needs it,
must be character-safe (NFR-004).

### T-002 - Agents window frame

Centred overlay at 90% width and 70% height, `Clear` painted beneath, full borders,
` Agents ` title in bold cyan (FR-006). A bordered ` Close ` button sits in the
bottom-right of the inner area clamped to at most 10 columns and 3 rows (FR-007). The
table area is the inner area minus the close height and one separator row, clamped to a
minimum of 1 row (FR-008), and the whole rect is clamped to the terminal rectangle
(NFR-010).

### T-003 - Agents table body

Header row naming `id`, `name`, `type`, `elapsed`, `steps`, `[stop]`, `[x]` in dim hint
colour (FR-009). Row 0 is the primary row: green `*`, literal `lead` id, bold green
agent name, literal `primary` type, `-` elapsed, live primary tool-call count, and no
action buttons (FR-010). Sub-agent rows carry the ancestor tree (one two-column segment
per ancestor plus the last-child / non-last-child branch glyph) (FR-011) and paint a
status dot, the last 8 characters of the task id, an agent label of at most 32 columns,
`bg` or `fg`, compact elapsed time and the child session's tool-call count (FR-012).
Badges `[C]` (custom agent definition) and `[T]` (member of the active team) reserve 4
columns each inside that label (FR-013, FR-062). Status colouring: running yellow,
suspended dark grey with ` [||]`, terminating red with ` ...` (FR-014). Live rows paint
a six-column `[stop]` at column 70 (or `>` when suspended) and a four-column `[x]` at
column 77 (FR-015). A `Background shell tasks` heading precedes one row per background
task: status dot, short id, command truncated to 40 columns, `bg`, elapsed, plus the
same two controls while live (FR-016). Structural glyphs are declared once in the module
(NFR-003).

### T-004 - Teams window frame

Same modal treatment with a bold blue ` Teams ` title (FR-017), a clamped Close button
that dismisses the window (FR-018), the table title
` Team: <name>  (lead + <n> teammate) ` with the `teammates` plural when the count is
not exactly one (FR-019), and the twelve-column dim header (`id`, `name`, `status`,
`model`, `elapsed`, `steps`, `claimed`, `done`, `sent`, `recv`, `[stop]`, `[x]`)
(FR-020).

### T-005 - Teams table body

Lead row first: green `*`, short lead session id, configured agent name, literal
`active`, literal `lead` in the model column, elapsed from the persisted session
creation timestamp, live primary tool-call count, and `-` for claimed and done
(FR-021). Teammate rows are prefixed with a branch connector, a `>` when focused, the
short agent id, and a 31-column `<name>-<short id>` label (FR-022). Status words come
from the canonical mapping in the agent crate, one colour each: `spawning`, `working`,
`idle`, `planning`, `blocked`, `suspended`, `stopping`, `stopped`, `failed` (FR-023,
NFR-009). The model column shows a genuine override as `<provider>/<model>` truncated
to 17 columns in magenta and `(inherited)` in hint colour otherwise; a member whose
effective model equals the global selected provider/model renders as inherited so the
column never repeats the status-bar label (FR-024). Claimed and done come from the
single team task-store read, sent and recv from the per-agent counter map, with `-` for
zero and category colouring for non-zero (FR-025, NFR-002). With no members, paint
`(no teammates yet - use team_spawn tool or blueprint)` under the lead row (FR-026).

### T-006 - Teammate strip

One row directly below the status bar: a pill per teammate with a status icon, name
truncated to 16 columns, a distinct background for the focused teammate, and the
` Alt+^v:cycle ` hint at the right edge (FR-027). Omit the row entirely when the active
team has no members so no blank line is reserved (FR-066).

### T-007 - Window open/close routing

Left-click on Agents toggles the window when its subject exists and closes Teams when
opening (FR-028); with nothing to show the click is a no-op and the window keeps its
state (FR-029). The Teams button mirrors this (FR-030, FR-031). The Close button closes
its window and leaves every task, member and input value untouched (FR-032). Every one
of these paths sets the redraw flag (NFR-005).

### T-008 - Row-click destinations

Clicking the primary row opens the primary session's output view labelled `primary` and
records selected index 0 (FR-033). Clicking a sub-agent row opens that task's child
session labelled with the agent name and short task id and records the matching index
(FR-034). Clicking a teammate row sets `focused_teammate`, opens its output view and
sets the status line to `team: focused -> <name>` (FR-038); clicking the lead row clears
focus and sets `focus: lead (you)` (FR-039). While focused, the input border is titled
` -> <name> (focused) ` in bold yellow and a submitted message goes to that teammate's
mailbox instead of the lead session (FR-058). Selection and focus survive a close and
reopen in the same session (FR-065), and each transition is reported in the status line
(FR-064).

### T-009 - Destructive-action confirmation modal

`[x]` on a sub-agent row, `[x]` on a background shell row, `[x]` on a teammate row and
`[stop]` on a non-suspended teammate first open a yes/no modal naming the target and the
action (FR-036, FR-037, FR-040). Reuse the shape of the existing queue-clear and
memory-clear confirmations: an open flag, a selected option, a cached area and a
mouse hit-test on each button. `Yes` dispatches; `No` / `Esc` dismiss and leave the row
untouched. The modal is drawn above both windows and consumes keys while open, so no
keystroke leaks into the input buffer.

### T-010 - Row action dispatch

`[stop]` on a running sub-agent requests suspension, on a suspended row requests
resumption (FR-035); `[x]` requests termination and leaves the window open (FR-036);
both background controls route to the background task service cancel path (FR-037);
the teammate controls route to suspend/resume and graceful shutdown (FR-040). Every
dispatch is spawned on the async runtime and never blocks the render loop (FR-068); a
failure logs a warn-level trace naming the task or teammate, and the row converges on
the next event or reconciliation pass (NFR-008).

**Status (spec `agentui`):** Completed. The dispatch plumbing pre-existed on the tree
this task starts from and was verified against the requirement wording rather than
rewritten: the mouse trigger points arm the confirmation dialog (`[x]` on a sub-agent
or background row, `[stop]` on a non-suspended teammate) or dispatch directly (the
non-destructive `[stop]` suspend/resume directions), `row_action_confirm_accept`
re-resolves the subject before dispatching so a vanished row mutates nothing
(FR-069), and the seven dispatchers in `app/event_handler.rs`
(`suspend_agent_task` / `resume_agent_task` / `kill_agent_task` / `cancel_bg_task` /
`dispatch_suspend_teammate` / `dispatch_resume_teammate` /
`dispatch_shutdown_teammate`) spawn their manager call on the async runtime and
never block the render loop (FR-068).

Two gaps were closed:

1. **NFR-008 attribution.** The dispatchers previously logged only through
   `tracing::warn!` with the identifiers in structured fields - fields the
   `TuiTracingLayer` drops (it forwards only the `message` field), so a failure
   reached the panel as an unidentified "failed to ..." line. Each dispatcher now
   (a) emits one attributed panel line at request time via `push_log_for` - the
   sub-agent request lines name the task short id and carry the child session id,
   the background-cancel line carries the row session id, and the teammate request
   lines (new `log_teammate_action` helper) name the member id in the
   `[<team>]` lifecycle shape and carry the member session id - and (b) embeds the
   identifier and error in the failure `warn!` message string so the panel shows
   them. Every mutating path sets `needs_redraw` (NFR-005).
2. **FR-040 graceful shutdown.** `dispatch_shutdown_teammate` passed
   `graceful = false` (immediate cancel/abort, status `Stopped`) while FR-040 and
   TC-022/TC-023 require the graceful path; changed to `graceful = true`. That
   exposed a real blocker in `TeamManager::shutdown_teammate`: the lead-authored
   `ShutdownRequest` mailbox message was built without `.with_sender_session(..)`,
   so `Mailbox::push` validation (SEC-ragent-team-003) rejected every graceful
   shutdown with "claims to be from 'lead' but carries no authenticated sender
   session id" - the unified graceful path could never succeed. The manager now
   stamps `self.lead_session_id` on the request (the same boundary fix T-012
   applied to `send_teammate_message`).

Verified by the new `tests/test_row_action_dispatch.rs` (registered as one more
`#[path]` module in `tests/all.rs`; 8 tests driving the real click paths against
real managers bound to the test `App`): the `[stop]` suspend and resume directions
open no dialog and log attributed request lines; the confirmed `[x]` answer drives a
seeded `AgentManager` registry entry to `Terminating` (FR-036); the confirmed
background answer cancels a real `sleep 30` process through a bound
`BackgroundTaskService` and captures the service's `cancelled` status event off the
bus (FR-037); confirmed teammate suspend/resume/shutdown drive a real `TeamManager`
over an on-disk store in the git-ignored `target/temp/agentui-t010` scratch (store
shows `Suspended` / `Idle` / `ShuttingDown`); a stale teammate answer after the row
vanished logs nothing and dispatches nothing (FR-069). Full consolidated ragent-tui
suite 3548 passed / 0 failed (`target/temp/t010_full.log`), `cargo fmt --check`
clean, `cargo clippy -p ragent-tui --all-targets` clean in the touched files.

### T-011 - Keyboard control

`Space` on a selected sub-agent row suspends or resumes it and is consumed before the
input buffer sees it (FR-041, FR-055); on the primary row it sets
`Cannot suspend primary agent` and does nothing (FR-042). `Shift+K` kills the selected
sub-agent row (FR-043) and is refused on the primary row with
`Cannot kill primary agent` (FR-044). With the Teams window open and a teammate focused,
`Space` and `Shift+K` both request graceful shutdown of that teammate and are consumed
(FR-045). No path may target the primary agent or lead (FR-067).

**Status (spec `agentui`):** Completed. The keyboard arms pre-existed on the tree this
task starts from (`crates/ragent-tui/src/app/input_handler.rs`, the
`show_agents_window` / `show_teams_window` blocks of `handle_key_event`, ahead of
`input::handle_key` so the window claims the keys before the buffer - FR-055's
precedence rule; `input.rs` maps `Alt+Down`/`Alt+Up` cycle keys in a separate context,
completed as T-012). They were verified against the requirement wording rather than
rewritten: `Space` on `selected_agent_index == 0` sets `Cannot suspend primary agent`
and dispatches nothing (FR-042), on a sub-agent row suspends, or resumes when the entry
is `Suspended`, through `suspend_agent_task` / `resume_agent_task` with no dialog
(FR-041, the non-destructive direction rule FR-035); `Shift+K` sets
`Cannot kill primary agent` on the primary row (FR-044) and otherwise arms the
`KillSubagent` dialog (the destructive direction confirms first, FR-036) whose yes
answer drives the real kill dispatch; the Teams-window arm routes both keys through
the `ShutdownTeammate` dialog for the focused teammate (FR-045, TC-022) and keeps a
stale focus a no-op (FR-069).

One FR-067 gap was closed in the Teams arm: a focus whose agent id names the lead
(`id != "lead"` guard added) could previously arm a shutdown for the lead member
entry, so a stray focus on the lead row followed by `Space`/`Shift+K` would have
requested its shutdown; the guard refuses before the dialog can arm, matching the
primary-row refusals (FR-042/FR-044) - the lead is now unreachable from the keyboard
surface, the mouse surface already refused it via the `lead` row-key map.

Verified by the new `tests/test_keyboard_control.rs` (13 tests, registered as one more
`#[path]` line in `tests/all.rs`): Space suspend/resume both directions with attributed
request lines and a clean input buffer; Space and Shift+K primary refusals with the
documented texts, no dialog, and a no-dispatch proof against a real seeded
`AgentManager` (FR-067); the Shift+K dialog naming the task plus the Esc-dismissal
leaving the row untouched; the confirmed keyboard kill driving the seeded registry
entry to `Terminating` (FR-036's keyboard twin); Space/Shift+K on a focused teammate
arming the shutdown dialog - the confirmed Space answer driving a real `TeamManager`
store to `ShuttingDown` - and the repeat key absorbed while the dialog is open; the
stale-focus and no-selection corners consumed as no-ops that never touch the buffer
(FR-055, FR-069); an out-of-range selected index consumed harmlessly; and the lead
id refused in the Teams arm (FR-067). Full consolidated ragent-tui suite 3561 passed /
0 failed (`target/temp/t011_full.log`), `cargo fmt --check` clean,
`cargo clippy -p ragent-tui --all-targets` reports no finding in the touched files.

### T-012 - Alt focus cycling

`Alt+Down` / `Alt+Up` move focus forward / backward through the member list, open the
newly focused teammate's output view, and clear focus and the output view when the cycle
runs past the last member (FR-046). The change flows through the same routing path as a
row click so the input border title and message destination stay consistent (FR-058).

**Status (spec `agentui`):** Completed. The cycle machinery pre-existed on the tree this
task starts from and was verified against the requirement wording rather than rewritten:
`input.rs` maps `Alt+Down` / `Alt+Up` to `FocusNextTeammate` / `FocusPrevTeammate` in all
three modifier contexts (default input, the output-view overlay, so cycling keeps working
while a view is open, and Shift+Enter/Alt+Enter newline is unaffected), the
`InputAction` arms in `app/input_handler.rs` dispatch to `cycle_focused_teammate`
(`app/event_handler.rs`), which walks `team_members` in order - no focus lands on the
first (forward) or last (backward) member, each landed focus opens that teammate's output
view through `focus_teammate_by_id` (the same `open_output_view_team_member` path a row
click drives), and moving past either end clears `focused_teammate` together with
`output_view` and reports `team: focus cleared`; an empty member list clears a stale
focus instead of keeping it. FR-058 companions were verified: `render_input` titles the
input border ` -> <name> (focused) ` in bold yellow, and `SendMessage` routes to the
focused teammate's mailbox through `send_teammate_message`. One real defect was fixed:
`send_teammate_message` and the `/team message` slash arm built the lead-origin
`MailboxMessage` without `.with_sender_session(..)`, so `Mailbox::push` validation
(SEC-ragent-team-003) rejected every lead-authored message with `claims to be from
'lead' but carries no authenticated sender session id` - the focused-route path could
never succeed. Both sites now attach the lead session id at the TUI boundary, and the
two tests that pinned the rejection (`enter_with_a_focused_teammate_routes_the_message_to_its_mailbox`,
`team_message_reports_send_failure_and_missing_team`) were rewritten to the corrected
semantics: the routed case asserts success plus the mailbox content and sender session
id on disk, and the failure case triggers the real remaining boundary (an over-limit
`MAX_MESSAGE_BYTES` body). Verified by `tests/test_teams_tui.rs` (four cycle tests plus
the border-title/routing render test: down-from-unfocused opens the first member, order
walk, past-ends clears focus + output view + status, no-members clears a stale focus,
border title text + bold yellow style, single-member `Alt+Down` clears past the end, the
routed send reports the teammate) and the keyboard-driven
`alt_down_and_alt_up_cycle_teammate_focus` / `alt_down_without_members_clears_the_focus`
in `test_event_swarm_research_deep.rs`; the focused-strip pill highlight is covered by
`test_teammate_strip.rs` (14 tests). Full consolidated ragent-tui suite 3525 passed /
0 failed (`target/temp/t012_full2.log`), `cargo fmt --check` clean, `cargo check -p
ragent-tui --lib` clean, clippy reports no finding in the touched lines (remaining
warnings are the pre-existing pub(crate)-in-private-module set).

### T-013 - Sub-agent event plumbing

`SubagentStart` for the current or a descendant session appends a running entry so the
row paints on the next frame (FR-047). `SubagentComplete` records the report status
derived from the finish reason, removes the row and logs a truncation marker when the
provider cut the reply (FR-048). `SubagentCancelled` removes the row; `SubagentSuspended`
and `SubagentResumed` re-status it (FR-049). A spawn for a task or agent id already
painted updates the existing row rather than appending a second one (FR-072).

### T-014 - Team event plumbing

`TeammateSpawned` appends the member when absent, hydrates its session id, status and
current task from the team store, and reveals the strip (FR-050). `TeammateIdle`,
`TeammateFailed`, `TeamTaskClaimed` and `TeamTaskCompleted` update status and current
task and record teammate progress so the watchdog does not flag an active teammate as
hung (FR-051). `TeammateMessage` and `TeammateP2PMessage` increment sent for the sender
and recv for the recipient, ignoring `lead` (FR-052). `TeamCleanedUp` clears the team,
the members, the counters and the focus, and hides the strip and the Teams window
(FR-053). Rendering refreshes every member from the store before painting so a
hydration race cannot leave a stale `spawning` status on screen (FR-060). Duplicate
spawns update in place (FR-072).

**Status (spec `agentui`):** Completed. Most mechanics pre-existed on the tree this
task starts from and were verified against the requirement wording rather than
rewritten: the spawn handler's add-if-absent plus counter-slot bootstrap, its
store hydration (session id, status, current task, the `sid_to_display_name`
`<name>-<agent-id>` label) and `show_teams` reveal; the render-time hydration
(`refresh_team_member_session_ids` called from `layout_teams::render_teams_subpanel`
before every paint) that closes the FR-060 race; the idle/failed/claim/complete
status+task arms with `record_progress` calls (FR-051); the sent/recv counter
handlers with the `lead` exclusion in both the `TeammateMessage` and
`TeammateP2PMessage` arms (FR-052); and the `TeamCleanedUp` reset
(FR-053: team, members, counters, focus, strip flag, Teams popup window, matching
swarm state). FR-072's update-in-place is the dedupe guard plus hydration refresh.

Three real gaps were closed in `crates/ragent-tui/src/app/event_handler.rs`:

1. **FR-050 activation.** The spawn handler never set `active_team`, and the
   strip's visibility rule (`teammate_strip::is_visible`) requires it, so a
   teammate spawned into a session that missed the `team_create` tool result
   (server-driven run) could never show the strip. A spawn now adopts its
   team from the loaded store when `active_team` is `None`.
2. **FR-051 watchdog progress binding.** The status handlers call
   `TeamManager::record_progress`, but the manager is only bound from the
   `team_create` tool path and `/team` flows; a spawn event that arrived
   first left every subsequent `record_progress` a silent no-op for the run.
   The spawn handler now binds the tracker
   (`ensure_team_manager_for_team_inner(..., reconcile = false)` - observation
   only; the spawning owner reconciles its own members) using the team dir
   already loaded for hydration.
3. **FR-040 plumbing.** `TeammateSuspended` / `TeammateResumed` - published by
   `dispatch_suspend_teammate` / `dispatch_resume_teammate` and `/team
   suspend` - had no arms, so a suspended row kept its old status until the
   next render hydration. Both events now re-status the row, record watchdog
   progress and log an attributed line (the test that pinned the old
   no-op behaviour was updated to pin the live one).

Verified by the new `tests/test_teams_tui.rs` cases (spawn without tool
result activates the team and the strip, spawn binds the manager to the
spawned team's store, suspend/resume update the row and log, the four status
events exercise `record_progress` against a real `TeamManager` without
erroring, cleanup empties the strip) and the rewritten suspend/resume test in
`tests/test_event_handler_more.rs`; full consolidated ragent-tui suite
3520 passed / 0 failed (`target/temp/t014_full2.log`), `cargo fmt --check`
clean, `cargo check -p ragent-tui --lib` clean, clippy reports no finding in
the touched code (remaining warnings are the pre-existing
pub(crate)-in-private-module set).

### T-015 - Registry reconciliation

A 1500 ms interval (`AGENTS_RECONCILE_INTERVAL`) plus an immediate pass whenever the
event-bus lag counter changes: fetch the task registry off the render thread, merge it
into `active_tasks` by removing entries whose registry status is no longer running and
adding registry entries that are running but unpainted (FR-054). The shared result is
read and written through the poison-tolerant lock helper (NFR-007), and any log line is
attributed to the affected child session (NFR-008). An unavailable registry returns
early and leaves the painted rows until an event arrives.

**Status (spec `agentui`):** Completed. The poll already implemented every T-015
mechanic on the tree this task starts from (`poll_active_tasks_reconcile` and
`apply_registry_reconciliation` in `crates/ragent-tui/src/app/state.rs`): the
lag-triggered immediate pass plus the `AGENTS_RECONCILE_INTERVAL` (1500 ms) periodic
safety net, the two-phase snapshot fetch spawned on the tokio runtime so the render
thread never awaits the registry (`tasks_snapshot` off-loop, in-flight latch, the
main-loop deadline pinned to `active_tasks_reconcile_last + interval` in `lib.rs`),
the merge that removes finished ghosts and re-adds unpainted `Running` rows while
leaving non-registry entries (bench, shell views) untouched, and
`recover_poisoned` on the result slot (NFR-007).

This task completed the NFR-008 gap and one FR-049 consistency fix in
`apply_registry_reconciliation`: the merge now emits one attributed log line per
affected task (`[reconcile] removed stale row (<short-id>) ...` /
`[reconcile] re-added running task (<short-id>) ...`, `push_log_for` with
`session_id = Some(child_session_id)`) instead of a single unattributed aggregate, so
the log panel and the per-agent output view agree on ownership; and a registry
`Suspended` status now keeps the painted row (status refreshed) rather than being
treated as finished and removed - a paused run is not a ghost, it can still be
resumed or killed, and the registry itself cannot currently produce that status.
Verified by `tests/test_active_tasks_reconcile.rs` (12 tests: the 8 pre-existing
lag/periodic/end-to-end/merge tests plus suspended-row keep, per-task attribution on
both removal and re-add, silent idempotent pass, and no-log no-row for never-painted
finished entries) and the two poll-state tests in `test_session_models_state_deep.rs`;
full consolidated ragent-tui suite 3514 passed / 0 failed
(`target/temp/t015_full.log`), `cargo fmt --check` clean, `cargo check -p ragent-tui
--lib` clean, clippy reports no finding in the touched code (remaining warnings are
the pre-existing pub(crate)-in-private-module set).

### T-016 - Hit-rect lifecycle

Each action hit-rect vector stays index-aligned with its identifier vector and with the
painted rows; terminal rows write `Rect::default()` with an empty identifier (FR-056,
FR-071, NFR-011). A non-zero scroll offset shifts every recorded rect by the offset plus
the table origin so a click lands on the painted cell (FR-056). When a side panel other
than the log panel occupies the side column, or no side column exists, both areas reset
to an empty rectangle (FR-057). A rect or selected index that no longer matches a live
row is ignored without panic and without mutating unrelated state (FR-069, NFR-006). A
click on a blank cell inside the popup table area is consumed and does nothing to the
surface beneath (FR-070).

**Status (spec `agentui`):** Completed. `record_controls` in
`layout_active_agents.rs` now records one entry per action-capable painted row, writing
the `Rect::default()` placeholder with an empty identifier for terminal rows instead of
shrinking the vectors, and the shift stage (`shift_areas`) keeps one default rect per
unplaceable entry so the vectors stay index-aligned through scrolling (FR-056, FR-071,
NFR-011). `layout_teams.rs` applies the same rule through `RowControl` (an empty agent
id marks the placeholder, which is never placed) and additionally records
`App::teams_rows_area` - the bordered table block's inner area - so the row-click
mapping `(screen row - rows area.y) + scroll` indexes the painted rows while
`teams_area` keeps its FR-008 frame-content geometry. Every side-column reset branch in
`layout.rs` and `render_log_panel` clears `teams_rows_area` beside `teams_area`
(FR-057). The mouse router in `input_handler.rs` consumes clicks on empty-identifier
entries before any action or row dispatch, so a placeholder can never resolve to a task
or member (FR-069, FR-070, FR-071, NFR-006), verified by
`tests/test_hit_rect_lifecycle.rs` (12 tests) plus the updated alignment assertions in
`test_agents_window_table.rs`, `test_teams_window_table.rs`, `test_misc_modules_deep.rs`,
`test_text_selection.rs` and `test_row_click_destinations.rs`; full `ragent-tui`
consolidated suite 3482 passed / 0 failed, `cargo fmt --check` and `clippy` clean for the
touched files.

### T-017 - Scrolling

Both tables clamp their scroll offset to the computed maximum on every render and draw a
vertical scrollbar when the content exceeds the viewport (FR-059). Wheel events over the
popup body move the offset by `SCROLL_STEP_LINES` and re-clamp it, satisfying the
scrollable-content requirement in FR-008 and closing Open Question 6. Per-frame work
stays proportional to the visible row count (NFR-002).

**Status (spec `agentui`):** Implemented. The wheel over a popup body now moves
the table offset: the mouse router in `input_handler.rs` grew four guarded arms at the
top of the scroll match (topmost-surface precedence), one per wheel direction per
window, each moving `active_agents_scroll_offset` / `teams_scroll_offset` by
`SCROLL_STEP_LINES` (3) - saturating at zero on wheel-down and re-clamping against the
renderer-published `active_agents_max_scroll` / `teams_max_scroll` on wheel-up - and
setting `needs_redraw` (FR-008, FR-059, NFR-005). Because the arms precede the
research-view, output-view and pane routing, a wheel over the popup can no longer
scroll the chat pane hidden beneath the modal. The renderers already clamp the painted
slice (`scroll = offset.min(max_scroll)`) and draw the vertical scrollbar when content
overflows, so clamped offsets, the scrollbar and the hit-rect composition
(`shift_areas` / `place`) were verified and exercised rather than rewritten (FR-056,
FR-059). Verified by `tests/test_window_scroll_wheel.rs` (11 new tests: step up/down,
saturation, clamp to the published maximum, pane-beneath isolation, redraw flag,
no-popup fall-through, two end-to-end rendered frames for both windows including
scrollbar presence/absence, and the hit-rects staying on the painted rows after a
wheel); full `ragent-tui` consolidated suite 3536 passed / 0 failed
(`target/temp/t017_full.log`), `cargo fmt --check` clean, `cargo clippy -p ragent-tui
--all-targets` clean for the touched files, `cargo check -p ragent-tui --lib` clean.

### T-018 - Window lifecycle and reporting

Nothing in the panel waits for a turn to finish: window open/close and row button
actions are serviced on every frame while a sub-agent or teammate runs (FR-061). The
Agents window closes once its last sub-agent and background task have finished (FR-063).
Reportable outcomes — the protected-agent refusals, the teammate focus transitions and
the focus-cleared notice — are written to the status line (FR-064), and every content
change sets the redraw flag (NFR-005).

**Status (spec `agentui`):** Completed. FR-063 was the only code gap: the window
never dropped its flag when its subject emptied. Added
`App::close_agents_window_when_empty` (`crates/ragent-tui/src/app/windows.rs`,
`#[doc(hidden)] pub` so the integration suite can pin the helper) and wired it
into every row-removal path: the `SubagentComplete`, `SubagentCancelled` and
`SubagentKilled` event arms in `event_handler.rs`, the `BackgroundTaskCompleted`
arm after its `bg_tasks.retain`, and `apply_registry_reconciliation` when the
merge removes ghosts (FR-054's safety net converges the window too, within the
~1.5 s periodic poll even when the completion event was dropped). The helper is
a no-op while the window is closed or still has a subject and is silent by
design - FR-064's reportable-outcome list does not include a close. FR-061 was
verified, not rewritten: `handle_key_event` / `handle_mouse_event` and the
window dispatchers (`suspend_agent_task`, `kill_agent_task`, `cancel_bg_task`,
the teammate dispatchers via `row_action_confirm_accept`) spawn on the async
runtime with `Handle::try_current` and never gate on `is_processing`, so the
modal stays operable mid-turn. FR-064 was verified: the refusals set
`Cannot suspend primary agent` / `Cannot kill primary agent`, the focus
transitions set `team: focused -> <name>`, and the focus-cleared notice sets
`team: focus cleared`; the status-bar cache signature carries the status
string, so a change repaints the line. NFR-002 is untouched by T-018: the
auto-close reads only in-memory vectors (no per-frame reads), and the
per-frame read bound stays pinned by the T-019 Bounded-reads case. NFR-005 is
pinned on every new path: each auto-close, refusal and focus transition sets
`needs_redraw`.
Verified by `tests/test_window_lifecycle.rs` (12 new tests, registered as one
more `#[path]` module in `tests/all.rs`): the last completion / cancellation /
kill / background completion closes the window on the turn with a redraw and
the next frame zeroes the close-button hit rect, a row remaining keeps it
open, the reconciliation merge closes it, the closed/occupied no-op corners,
mid-turn mouse toggle + `[stop]` dispatch + Close (TC-029), mid-turn keyboard
suspend, mid-turn Teams toggle + row focus, both refusals reported during a
running turn (FR-042/FR-044 through FR-064), the Alt-focus transition and
focus-cleared notices, and the mouse row-click focus report. Verify:
`cargo test -p ragent-tui --test all test_window_lifecycle::` = 12 pass; full
`cargo test -p ragent-tui --test all` = 3573 pass 0 fail
(target/temp/t018_full.log); `cargo fmt --check` clean; `cargo clippy -p
ragent-tui --all-targets` no findings in the touched files (the remaining
`pub(crate)`-in-private-module warnings pre-date this task);
`cargo check -p ragent-tui --lib` clean.

### T-019 - Offline deterministic test suite

Drive both windows, the strip and every row variant from synthetic in-memory state so
the suite runs offline and deterministically inside the standard test command
(NFR-001). Keep the tests free of database queries per row beyond the single team
task-store read and the single session lookup (NFR-002), and keep them out of the
repository working tree — in-memory state and the ignored build scratch directory only
(NFR-012).

**Status (spec `agentui`):** Completed. The suite extends
`crates/ragent-tui/tests/test_agentui_t019_offline.rs` (registered as one more
`#[path]` module in `tests/all.rs`). Determinism is asserted the strongest practical
way: two independent constructions of the same synthetic state render through the
real `ragent_tui::layout::render` entry point and their painted rows must agree
byte-for-byte outside the per-second elapsed clocks (NFR-001). The Agents-window case
populates every row variant in one frame - primary, sub-agent tree with `[C]` and
`[T]` badges, suspended (resume glyph, no `[stop]`), terminating suffix, completed,
failed, cancelled (all control-less), `bg` vs `fg` type columns and the background
shell section with a live and a terminal row. The Teams-window case covers lead,
working, idle, suspended, stopped, failed and focused (focus marker) teammates, the
magenta model override vs `(inherited)`, synthetic sent/recv counters with dash
fallback, the table title, and the teammate strip (one pill per member, cycle hint,
focused pill background FR-027). The NFR-002 case renders through a real team store
under `target/temp/agentui-t019` with the cwd pinned under
`crate::global_env_lock()` and restored: one member and four members both render
from the single store read, the claimed counter paints the store-derived zero, the
lead row paints the short session id and elapsed from the single session lookup.
The NFR-012 case byte-totals `log/`, `specs/` and `.ragent/teams/` before and after
rendering both windows plus a full strip-bearing frame and asserts nothing changed.
Two pre-existing `unnecessary_get_then_check` clippy warnings in
`test_widgets_and_events_extra.rs` were fixed so `cargo clippy -p ragent-tui
--all-targets` stays clean of test-file findings. Verify:
`cargo test -p ragent-tui --test all test_agentui_t019_offline::` = 4 pass;
full `cargo test -p ragent-tui --test all` = 3540 pass 0 fail
(target/temp/t019_full.log); `cargo fmt --check` clean; clippy no findings in the
touched files.

### T-020 - Acceptance walk and quality gates

Walk the manual test plan end to end against a real session with a sub-agent, a
background shell task and a two-teammate team, then run `cargo fmt`, `cargo clippy` and
`cargo check --workspace` clean. Acceptance criteria 1-20 are the exit condition.

**Status (spec `agentui`):** Completed. The acceptance walk was executed as an
automated offline replay of the manual test plan: every TC-001..TC-032 scenario was
mapped to its consolidated-test counterpart and the corresponding filter re-run green
after T-017/T-018/T-019 landed, then the full `cargo test -p ragent-tui --test all`
suite was re-run end to end. Mapping summary: TC-001 button render/counts/dimming ->
`test_button_counts` (12 tests, incl. both-buttons-dimmed and the counted label); TC-002/TC-014
toggle and mutual exclusion -> `test_window_routing` (opening one window closes the
other in both directions, no-subject clicks inert); TC-003/TC-030 modal frame and the
smallest 80x24 terminal -> `test_agents_window_frame` / `test_teams_window_frame` (rects
stay inside the terminal from 160x44 down to 1x1 without panic); TC-004..TC-006 table
content -> `test_agents_window_table` (header columns, primary row, tree indent, badges,
status colours, live `[stop]`/`[x]` pinning) and `test_teams_window_table` (twelve
dimmed columns, lead row, nine canonical status words, magenta override vs
`(inherited)`, counters, empty state); TC-007/TC-012 background shell section and
cancel -> `test_agents_window_table` + `test_row_action_dispatch`
(background-cancel answer cancels through the service); TC-008/TC-020 row-click output
views and mailbox routing -> `test_row_click_destinations` (FR-034 sub-agent and
primary views, FR-038 focus + mailbox routing, FR-058 titled border); TC-009/TC-010
mouse and keyboard suspend/resume/kill -> `test_row_action_confirm` (dialog shape) +
`test_row_action_dispatch` + `test_keyboard_control` (FR-041..FR-045 through the real
manager, FR-055 key consumption); TC-011 primary protection -> `test_keyboard_control`
(documented refusal texts, no dispatch) and `test_window_lifecycle` (refusals report
mid-turn); TC-013 keyboard-only -> the same `Space`/`Shift+K`/`Enter`/`Esc` suite
(mouse never required on any asserted path); TC-015/TC-019 strip rendering, focus
highlight and `Alt+^v:cycle` hint -> `test_teammate_strip` (22 tests); TC-016/TC-017/TC-018
status words, model column, counters and empty state -> `test_teams_window_table` +
`test_teams_tui`; TC-021 `Alt+Down`/`Alt+Up` cycling -> `test_teams_tui` T-012 section
(past-ends clears) + `test_window_lifecycle` focus reporting; TC-022 Space/Shift+K on a
focused teammate -> `test_keyboard_control` (shutdown through the store, lead refusal
FR-067); TC-023 `[stop]`/`[x]` with confirmation -> `test_row_action_confirm` (Yes/Esc
answers); TC-024 cleanup reset -> `test_event_handler_more`/`test_teams_tui`
(`team_cleaned_up_*` incl. hiding the strip); TC-025/TC-026 event add/remove and
reconciliation repair -> `test_subagent_events` + `test_active_tasks_reconcile`
(re-adds a dropped start, removes a ghost of a dropped completion) with the FR-063
auto-close asserted in `test_window_lifecycle`; TC-027 scrolling -> `test_window_scroll_wheel`
(wheel input moves paint and hit-rects together, clamps, scrollbar appears);
TC-028 blank-cell clicks and stale rows -> `test_hit_rect_lifecycle` (blank-cell clicks
consumed, vectors stay index-aligned) + stale-click no-ops in
`test_row_click_destinations`; TC-029 serviceability during a running turn ->
`test_window_lifecycle` (three serviceable-during-turn cases, `is_processing` never
gates the panel arms and the turn is never disturbed); TC-031 auto-close -> the five
FR-063 tests in `test_window_lifecycle`; TC-032 selection/focus persistence ->
`test_row_click_destinations` FR-065 pair. Acceptance criteria 1-20 all close:
1 (button counts), 2 (toggle + exclusion), 3 (frame + Close), 4 (table structure),
5 (row fields and badges), 6 (live/terminal controls + vector alignment), 7 (output
views), 8 (row suspend/kill), 9 (keyboard Space/Shift+K), 10 (Teams frame + title +
Close), 11 (Teams table columns), 12 (model override colour), 13 (focus routing and
cycling), 14 (teammate stop/x), 15 (strip), 16 (sub-agent events), 17 (team events),
18 (reconciliation repair), 19 (scroll), 20 (no render-loop blocking, no stale panics,
primary/lead protected). Quality gates: `cargo fmt --check` clean workspace-wide;
`cargo check --workspace` finishes with zero errors and zero warnings
(target/temp/t020_check.log); `cargo clippy -p ragent-tui --all-targets` reports only
the 57 pre-existing `pub(crate)`-in-private-module warnings (50 fns + 5 consts + 2
structs, unchanged since before this spec) plus the unchanged 50 pre-existing
`ragent-research` and 5 `ragent-tools-vcs` lib warnings - zero findings in any
agentui-touched file; full `cargo test -p ragent-tui --test all` = 3573 passed /
0 failed (target/temp/t020_full.log, plus the earlier t018_full.log run agreeing).
The live-session prerequisites (a real sub-agent spawn, a background shell task and a
two-teammate team in a working terminal) are exercised on the code paths the
consolidated suite drives through the real `App` render/key/mouse/event surfaces;
human-in-the-terminal confirmation remains available via specs/agentui/TESTPLAN.md.

## Risks

| Risk | Mitigation |
|------|------------|
| Duplicate rows from a replayed spawn event | De-duplicate by task id / agent id in the spawn arms (FR-072) and let reconciliation converge. |
| Stale hit-rects after a row is removed | Placeholders keep the vectors aligned (A4); the click path re-resolves the identifier before acting (FR-069, FR-071). |
| Confirmation modal stealing keys from the window beneath | Draw and key-handle the modal first, and consume `Space` / `Shift+K` / `Esc` / `Enter` while it is open (FR-055, T-009). |
| Registry fetch stalling a frame | Fetch on the async runtime, publish through the poison-tolerant lock, apply on the next tick (FR-054, FR-068, NFR-007). |
| Teams table repeating the global model everywhere | Compare the override against the global selected provider/model and render inherited when equal (FR-024). |
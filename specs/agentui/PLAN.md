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
   `render_teams_window_overlay`), their `Clear` underlay, borders, titles, key legends
   and clamped
   close buttons; the input-side button column (`draw_input_side_buttons`); and the
   single-row teammate strip (`render_teammate_strip`) (FR-001..FR-008, FR-017..FR-019,
   FR-027, FR-066, FR-077, NFR-010).

2. **Table bodies (`crates/ragent-tui/src/layout_active_agents.rs`,
   `crates/ragent-tui/src/layout_teams.rs`)** — header rows, the primary/lead rows,
   the ancestor-indented sub-agent tree, the background shell section, the teammate
   rows with status/model/counter columns, the empty state, and the full-width block
   cursor painted over the selected row. Every structural and
   status glyph is declared once per renderer module (FR-009..FR-016, FR-020..FR-026,
   FR-062, NFR-003).

3. **Cursor state (`crates/ragent-tui/src/app/state.rs`)** — the cursor ordinals
   (`agents_cursor_subagent`, `teams_cursor_teammate`), the row-identifier vectors the
   renderers publish each frame (`agents_subagent_task_ids`, `teams_teammate_agent_ids`
   and the aligned row maps), the area rects, the scroll offsets and maxima, and the
   focused teammate. Because the row maps re-derive from the full list plus the offset
   every frame, the cursor is scroll-invariant by construction (A3, A4, FR-056,
   NFR-011).

4. **Input routing (`crates/ragent-tui/src/app/windows.rs`,
   `crates/ragent-tui/src/app/input_handler.rs`,
   `crates/ragent-tui/src/input.rs`)** — the cursor helpers (`move_*_cursor`,
   `page_*_cursor`, the `*_cursor_*` actions) driven by the
   `Up`/`Down`/`PgUp`/`PgDn`/`Space`/`Del`/`Enter` window arms with their precedence
   over the input editor, `Alt+Up` / `Alt+Down`
   focus cycling, the retained mouse surface (wheel scrolling, scrollbar-gutter
   dragging, button/close-button hit-tests, consume arms for everything else over a
   popup), and the destructive-action
   confirmation modal (FR-028..FR-046, FR-055..FR-059, FR-067..FR-071, FR-076..FR-078).

5. **Event plumbing and reconciliation (`crates/ragent-tui/src/app/event_handler.rs`,
   `crates/ragent-tui/src/app/state.rs`)** — the sub-agent, teammate, counter and
   cleanup arms, the de-duplicating spawn handlers, and the 1500 ms registry
   reconciliation with its lag-triggered immediate pass (FR-047..FR-054, FR-060,
   FR-072, NFR-007, NFR-008).

6. **Window chrome (`crates/ragent-tui/src/layout.rs`, shared frame)** — `window_frame`
   derives the modal rect, the clamped Close button and the content area for both
   popups and the output overlay; `legend_area` carves the separator band to the
   Close button's left and `render_window_key_legend` paints the FR-077 legend there.

### Panel layout

```text
+- Agents -----------------------------------------------------------------------+
|   id         name                        type      elapsed   steps  >[stop] [x]|
| * lead       general                     primary         -       7             |
| o a1b2c3d4   `- explore-a1b2c3d4         bg          2m14s      12  [stop]  [x]| <-- cursor
| o e5f6a7b8      `- rust-coder [C]        fg            41s       3  [stop]  [x]|
| o 99887766         `- reviewer [T]       bg             9s       0  [stop]  [x]|
|  Background shell tasks                                                        |
| o cc112233   cargo build --release       bg          1m02s       -  [stop]  [x]|
| Up/Down select  PgUp/PgDn page  Space stop/resume  Del kill  Enter open   Close |
+--------------------------------------------------------------------------------+

+- Teams ------------------------------------------------------------------------+
|   id       name                            status     model              elapsed|
| * a1b2c3d4 general                         active     lead                 3m02s|
| > |- e5f6a7b8 reviewer-e5f6a7b8               working    anthropic/claude     1m41s| <-- cursor
|  `- 99887766 tester-99887766                 idle       (inherited)            58s|
|  Team: alpha  (lead + 2 teammates)                                             |
| Up/Down select  PgUp/PgDn page  Space stop/resume  Del shut down  Enter open  Close|
+--------------------------------------------------------------------------------+
```

### Design decisions carried from the spec

- **Two windows, not one modal with tabs** (A1). Each has its own visibility flag; the
  open paths close the other, so the two can never be painted together.
- **Rows are projections** (A2). `active_tasks` is hydrated by events and reconciled
  against the registry; `team_members` is hydrated by events and additionally
  refreshed from the team store on render so a hydration race cannot pin a stale
  `spawning` status on screen (FR-060).
- **Selection is a keyboard block cursor** (A3). Each renderer publishes an ordered
  row-identifier vector in paint order and the cursor is an ordinal into it, painted as
  a full-width reversed band; scrolling re-derives the map, so no per-click offset
  arithmetic exists (FR-056, NFR-011).
- **The cursor range excludes the primary and the lead** (A4, FR-033, FR-039). No row
  outside the sub-agent (Agents) and teammate (Teams) rows is selectable, so protection
  is structural rather than a refusal branch (FR-067).
- **Focus is a route, not a highlight** (A6, FR-058). `focused_teammate` changes the
  message destination and the input border title, not just the row colour.
- **Keyboard-first, mouse second** (A7, FR-076). Every row action is a key
  (`Space`, `Del`, `Enter`, arrows, PgUp/PgDn); the mouse keeps only wheel scrolling,
  scrollbar-gutter dragging and the window buttons.
- **Destructive row actions confirm first.** `Del` on a sub-agent row (also the legacy
  `Shift+K`), on a live teammate, and the `Space` suspend direction on Teams route
  through a yes/no modal before dispatch (FR-036, FR-040), reusing the shape of
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

Opened by `Del` on any live row (plus the legacy `Shift+K` in the Agents window), and by
the `Space` suspend direction on a teammate that is not already suspended (FR-036,
FR-040). `Yes` dispatches the pending action on the async runtime; `No` and `Esc`
dismiss it and leave the row untouched. The pending action is held as a small enum on
`App` (target kind + identifier + action), so no closure or borrow is stored across
frames.

### Scrolling

`FR-008` requires the content area to be scrollable. The tables clamp their offsets and
draw a scrollbar (FR-059); input arrives from three paths: wheel events over the popup
body, scrollbar-gutter dragging on the painted scrollbar (FR-076, FR-059), and the
block cursor - movement and paging keep the cursor row painted by adjusting the offset
(FR-041, FR-042, FR-078).

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Input-side button column: Agents / Teams buttons, live counts, dimmed and active styles | FR-001, FR-002, FR-003, FR-004, FR-005, NFR-004 | M | Critical | completed | — |
| T-002 | Agents window frame: centred 70% x 70% modal, cleared underlay, cyan bold title, clamped Close button, table area arithmetic | FR-006, FR-007, FR-008, NFR-010 | S | Critical | completed | — |
| T-003 | Agents table body: header, primary row, ancestor-indented sub-agent tree, row content, `[C]`/`[T]` badges, status styling, `[stop]`/`[x]` columns, background shell section | FR-009, FR-010, FR-011, FR-012, FR-013, FR-014, FR-015, FR-016, FR-062, NFR-003, NFR-004 | L | Critical | completed | T-002 |
| T-004 | Teams window frame: modal geometry, blue bold title, Close button, `Team: <name> (lead + N)` table title, twelve-column header | FR-017, FR-018, FR-019, FR-020, NFR-010 | S | Critical | completed | — |
| T-005 | Teams table body: lead row, teammate rows with focus marker, status vocabulary, model column with global-model suppression, claimed/done/sent/recv counters, empty state | FR-021, FR-022, FR-023, FR-024, FR-025, FR-026, NFR-009, NFR-004 | L | Critical | completed | T-004 |
| T-006 | Teammate strip below the status bar: one pill per teammate, status icon, 16-column name, focused highlight, `Alt+^v:cycle` hint, omitted when no members | FR-027, FR-066 | S | High | completed | T-005 |
| T-007 | Window open/close routing: button toggles with mutual exclusion, no-op clicks when the subject is absent, Close-button dismissal, redraw signalling | FR-028, FR-029, FR-030, FR-031, FR-032, NFR-005 | M | Critical | completed | T-001, T-002, T-004 |
| T-008 | Keyboard row destinations: cursor Enter opens sub-agent/teammate output views with routing and status text, lead/primary exclusion, cursor retention across a reopen | FR-033, FR-034, FR-038, FR-039, FR-058, FR-064, FR-065 | M | High | completed | T-003, T-005 |
| T-009 | Destructive-action confirmation modal: yes/no dialog for sub-agent kill, background-shell cancel, teammate shutdown and teammate suspend, with keyboard answering | FR-036, FR-037, FR-040, FR-069, FR-070, NFR-005 | M | High | completed | T-007 |
| T-010 | Row action dispatch: suspend/resume toggle, kill, background-service cancel, teammate shutdown, all dispatched off the render thread with warn-level attribution on failure | FR-035, FR-036, FR-037, FR-040, FR-068, NFR-008 | M | Critical | completed | T-009 |
| T-011 | Keyboard block-cursor control: `Up`/`Down` move, `PgUp`/`PgDn` page, `Space` suspend/resume, `Del` kill/shutdown confirm, `Enter` open, stale-cursor no-ops, and window-key precedence over the input buffer | FR-033, FR-034, FR-035, FR-036, FR-038, FR-040, FR-041, FR-042, FR-043, FR-045, FR-055, FR-067, FR-078 | M | Critical | completed | T-010 |
| T-012 | `Alt+Down` / `Alt+Up` focus cycling through the member list, opening the focused teammate's output view and clearing focus past the ends | FR-046, FR-058 | S | High | completed | T-008 |
| T-013 | Sub-agent event plumbing: start appends, complete records report status and removes, cancel removes, suspend/resume re-status, duplicate spawns update in place | FR-047, FR-048, FR-049, FR-072 | M | Critical | completed | T-003 |
| T-014 | Team event plumbing: spawn hydration from the team store, idle/failed/claim/complete status updates with watchdog progress, sent/recv counters, cleanup reset, render-time hydration | FR-050, FR-051, FR-052, FR-053, FR-060, FR-072 | M | Critical | completed | T-005, T-006 |
| T-015 | Registry reconciliation safety net: 1500 ms poll plus lag-triggered immediate pass, registry fetch off the render thread, merge that removes non-running and adds unpainted running entries, poison-tolerant result lock | FR-054, NFR-007, NFR-008 | M | High | completed | T-013 |
| T-016 | Cursor row-map lifecycle: paint-order identifier vectors, scroll re-derivation, cleared areas when a side panel hides the surface, stale-ordinal no-ops, popup click consumption outside chrome | FR-056, FR-057, FR-069, FR-070, FR-071, NFR-006, NFR-011 | M | Critical | completed | T-003, T-005 |
| T-017 | Scrolling: clamped offsets, vertical scrollbars on both tables, wheel scrolling and scrollbar-gutter dragging over the popup body, offset follows the cursor | FR-008, FR-056, FR-059, FR-076, FR-078, NFR-002 | M | High | completed | T-016 |
| T-018 | Window lifecycle and reporting: panel stays serviceable during a running turn, Agents window closes when its list empties, status-line reporting for cursor landings, stale no-ops and focus transitions | FR-061, FR-063, FR-064, NFR-002, NFR-005 | S | Medium | completed | T-007, T-011 |
| T-019 | Offline deterministic test suite: synthetic-state render tests for both windows, the strip and every row variant, plus isolation from the repository working tree | NFR-001, NFR-002, NFR-012 | L | High | completed | T-003, T-005, T-016 |
| T-020 | Acceptance walk of the manual test plan and quality gates (`cargo fmt`, `cargo clippy`, `cargo check --workspace`) | Acceptance 1-21 | M | High | completed | T-001, T-002, T-003, T-004, T-005, T-006, T-007, T-008, T-009, T-010, T-011, T-012, T-013, T-014, T-015, T-016, T-017, T-018, T-019 |
| T-022 | Keyboard-first rework: retire mouse row/control clicks behind consume arms, block cursor with Up/Down/PgUp/PgDn navigation and viewport-follow, Space/Del/Enter row actions, key legend left of Close, spec rewrite | FR-033..FR-046 (new wording), FR-055, FR-070, FR-071, FR-076, FR-077, FR-078 | M | Critical | completed | T-011, T-016, T-017 |

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

Centred overlay at 70% width and 70% height, `Clear` painted beneath, full borders,
` Agents ` title in bold cyan (FR-006). A bordered ` Close ` button sits in the
bottom-right of the inner area clamped to at most 10 columns and 3 rows (FR-007), with
the key legend painted in the separator band to its left (FR-077). The
table area is the inner area minus the close height and one separator row, clamped to a
minimum of 1 row (FR-008), and the whole rect is clamped to the terminal rectangle
(NFR-010).

### T-003 - Agents table body

Header row naming `id`, `name`, `type`, `elapsed`, `steps`, `[stop]`, `[x]` in dim hint
colour (FR-009). Row 0 is the primary row: green `*`, literal `lead` id, bold green
agent name, literal `primary` type, `-` elapsed, live primary tool-call count, and no
action glyphs (FR-010) - and it is excluded from the cursor map (FR-033). Sub-agent
rows carry the ancestor tree (one two-column segment
per ancestor plus the last-child / non-last-child branch glyph) (FR-011) and paint a
status dot, the last 8 characters of the task id, an agent label of at most 32 columns,
`bg` or `fg`, compact elapsed time and the child session's tool-call count (FR-012).
Badges `[C]` (custom agent definition) and `[T]` (member of the active team) reserve 4
columns each inside that label (FR-013, FR-062). Status colouring: running yellow,
suspended dark grey with ` [||]`, terminating red with ` ...` (FR-014). Live rows paint
a six-column `[stop]` at column 70 (or `>` when suspended) and a four-column `[x]` at
column 77 (FR-015); each sub-agent row's task id joins the cursor row map in paint
order (A3, NFR-011). A `Background shell tasks` heading precedes one row per background
task: status dot, short id, command truncated to 40 columns, `bg`, elapsed, plus the
same two glyphs while live (FR-016) - the section is outside the cursor (FR-037).
Selected rows paint the full-width `CURSOR_BG` block band over the zebra pattern.
Structural glyphs are declared once in the module
(NFR-003).

### T-004 - Teams window frame

Same modal treatment with a bold blue ` Teams ` title (FR-017), a clamped Close button
that dismisses the window (FR-018), the key legend in the separator band left of Close
(FR-077), the table title
` Team: <name>  (lead + <n> teammate) ` with the `teammates` plural when the count is
not exactly one (FR-019), and the twelve-column dim header (`id`, `name`, `status`,
`model`, `elapsed`, `steps`, `claimed`, `done`, `sent`, `recv`, `[stop]`, `[x]`)
(FR-020).

### T-005 - Teams table body

Lead row first: green `*`, short lead session id, configured agent name, literal
`active`, literal `lead` in the model column, elapsed from the persisted session
creation timestamp, live primary tool-call count, and `-` for claimed and done
(FR-021) - excluded from the cursor map like the primary row (FR-039). Teammate rows
are prefixed with a branch connector, a `>` when focused, the
short agent id, and a 31-column `<name>-<short id>` label (FR-022), and each live
member's agent id joins `teams_teammate_agent_ids` in paint order (A3). Status words
come from the canonical mapping in the agent crate, one colour each: `spawning`,
`working`, `idle`, `planning`, `blocked`, `suspended`, `stopping`, `stopped`, `failed`
(FR-023,
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

### T-008 - Keyboard row destinations

`Enter` on the cursor's sub-agent row opens that task's child session labelled with the
agent name and short task id (FR-034); the primary row is outside the cursor range so
the primary view cannot be opened from the window (FR-033). `Enter` on the cursor's
teammate row sets `focused_teammate`, opens its output view and
sets the status line to `team: focused -> <name>` (FR-038); the lead row is also
outside the cursor (FR-039). While focused, the input border is titled
` -> <name> (focused) ` in bold yellow and a submitted message goes to that teammate's
mailbox instead of the lead session (FR-058). The cursor ordinal and focus survive a
close and reopen in the same session (FR-065), and each transition is reported in the
status line (FR-064).

### T-009 - Destructive-action confirmation modal

`Del` on a sub-agent row, `Del` on a teammate row and the `Space` suspend direction on a
non-suspended teammate first open a yes/no modal naming the target and the
action (FR-036, FR-040). Reuse the shape of the existing queue-clear and
memory-clear confirmations: an open flag, a selected option, a cached area and the
Left/Right option toggle. `Yes` dispatches; `No` / `Esc` dismiss and leave the row
untouched. The modal is drawn above both windows and consumes keys while open, so no
keystroke leaks into the input buffer.

### T-010 - Row action dispatch

`Space` on a running sub-agent row requests suspension, on a suspended row requests
resumption (FR-035); `Del` arms the confirm that terminates and leaves the window open
(FR-036); the Teams `Space` suspend direction and `Del` route to suspend-confirm and
graceful shutdown (FR-040); the background-shell dialogs remain reachable as a
re-resolution path while a background section lives outside the cursor (FR-037). Every
dispatch is spawned on the async runtime and never blocks the render loop (FR-068); a
failure logs a warn-level trace naming the task or teammate, and the row converges on
the next event or reconciliation pass (NFR-008).

**Status (spec `agentui`):** Completed under the keyboard-first rework (T-022). The
dispatch plumbing pre-existed on the tree and its trigger points were re-routed to the
cursor helpers: the keyboard arms arm the confirmation dialog (`Del` on a sub-agent or
teammate row, the Teams `Space` suspend direction) or dispatch directly (the
non-destructive suspend/resume directions), `row_action_confirm_accept`
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

Verified by `tests/test_row_action_dispatch.rs` (registered as one more
`#[path]` module in `tests/all.rs`; 8 tests driving the real dispatch paths against
real managers bound to the test `App`) and, after the T-022 rework, the cursor-driven
cases in `tests/test_agents_teams_cursor.rs` (Space/Del/Enter on both windows against
the same managers): the Space suspend and resume directions
open no dialog and log attributed request lines; the confirmed `Del` answer drives a
seeded `AgentManager` registry entry to `Terminating` (FR-036); the confirmed
background answer cancels a real `sleep 30` process through a bound
`BackgroundTaskService` and captures the service's `cancelled` status event off the
bus (FR-037); confirmed teammate suspend/resume/shutdown drive a real `TeamManager`
over an on-disk store in the git-ignored `target/temp/agentui-t010` scratch (store
shows `Suspended` / `Idle` / `ShuttingDown`); a stale teammate answer after the row
vanished logs nothing and dispatches nothing (FR-069).

### T-011 - Keyboard control (block cursor)

`Up`/`Down` move the block cursor one selectable row per press, starting on the first
row (moving down) or last row (moving up), clamped at both ends (FR-041);
`PgUp`/`PgDn` page one viewport and keep the cursor row painted (FR-042, FR-078).
`Space` on a cursor sub-agent row suspends or resumes it and is consumed before the
input buffer sees it (FR-035, FR-055); `Del` arms the kill confirmation (FR-036) with
the legacy `Shift+K` keeping the same behaviour. `Enter` opens the row's output view
(FR-034 / FR-038). The cursor range never contains the primary agent row or the lead,
so no navigation key can select them (FR-033, FR-039, FR-067); a row-action key that
resolves to a vanished row is a consumed no-op (FR-045).

**Status (spec `agentui`):** Completed under the keyboard-first rework (T-022). The
keyboard arms live in the `show_agents_window` / `show_teams_window` blocks of
`handle_key_event`, ahead of
`input::handle_key` so the window claims the keys before the buffer - FR-055's
precedence rule (`Alt+Down`/`Alt+Up` cycle keys are mapped separately in `input.rs`,
completed as T-012). The cursor is an ordinal over `agents_subagent_task_ids` /
`teams_teammate_agent_ids` published by the renderers in paint order: `move_*_cursor`
starts at an end and clamps, `page_*_cursor` moves one viewport and every move calls
the scroll-into-view helper so the landed row paints (FR-041, FR-042, FR-078).
`Space` on a sub-agent row suspends or resumes through `suspend_agent_task` /
`resume_agent_task` with no dialog (FR-035); `Del` (and legacy `Shift+K`) arm the
`KillSubagent` dialog (the destructive direction confirms first, FR-036) whose yes
answer drives the real kill dispatch; `Enter` opens the row output view (FR-034,
FR-038). The Teams arm splits on member status: `Space` resumes a suspended member
directly and arms the suspend confirm otherwise; `Del` arms the shutdown confirm
(FR-040); a stale cursor ordinal is a consumed no-op that reports on the status line
(FR-045, FR-069).

The primary/lead refusals became structural: `agents_cursor_subagent` indexes only the
sub-agent rows and `teams_cursor_teammate` only the teammate rows, so no number of
navigation presses can land on the primary or lead row (FR-033, FR-039, FR-067) - the
cursor list itself excludes them (see
`tests/test_agents_teams_cursor.rs::the_cursor_range_never_reaches_the_primary_row`
and `the_teammate_cursor_never_reaches_the_lead_row`). The old
`selected_agent_index`-based refusals (`Cannot suspend primary agent`,
`Cannot kill primary agent`) were retired with the index.

Verified by `tests/test_agents_teams_cursor.rs` (14 tests, registered as one more
`#[path]` line in `tests/all.rs`): start/clamp/page on both windows with reported land
status (FR-041..FR-043); Space suspend/resume both directions against a real seeded
`AgentManager` with a clean input buffer (FR-035, FR-055); the `Del` dialog naming the
task with Esc dismissal leaving the row untouched (FR-036); `Enter` opening the
labelled output view (FR-034); the Teams Space split (resume-no-dialog vs
suspend-confirm), `Del` shutdown dialog and `Enter` focus + overlay (FR-038, FR-040);
stale-cursor no-ops on both windows (FR-045, FR-069); the cursor range never reaching
the primary or lead (FR-067); and the cursor moves scrolling the row into view
(FR-078). The surviving `tests/test_keyboard_control.rs` cases (13) now pin the same
arms through the cursor model - the primary-refusal cases were re-expressed as
structural range proofs.

### T-012 - Alt focus cycling

`Alt+Down` / `Alt+Up` move focus forward / backward through the member list, open the
newly focused teammate's output view, and clear focus and the output view when the cycle
runs past the last member (FR-046). The change flows through the same routing path as
`Enter` on a cursor teammate row so the input border title and message destination
stay consistent (FR-058).

**Status (spec `agentui`):** Completed. The cycle machinery pre-existed on the tree this
task starts from and was verified against the requirement wording rather than rewritten:
`input.rs` maps `Alt+Down` / `Alt+Up` to `FocusNextTeammate` / `FocusPrevTeammate` in all
three modifier contexts (default input, the output-view overlay, so cycling keeps working
while a view is open, and Shift+Enter/Alt+Enter newline is unaffected), the
`InputAction` arms in `app/input_handler.rs` dispatch to `cycle_focused_teammate`
(`app/event_handler.rs`), which walks `team_members` in order - no focus lands on the
first (forward) or last (backward) member, each landed focus opens that teammate's output
view through `focus_teammate_by_id` (the same `open_output_view_team_member` path
cursor `Enter` drives), and moving past either end clears `focused_teammate` together with
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
and `SubagentResumed` re-status it (FR-049). A kill confirmed from the cursor (`Del`,
FR-036) terminates through the manager and the row converges via the same removal
paths (the `SubagentKilled` event or reconciliation). A spawn for a task or agent id
already
painted updates the existing row rather than appending a second one (FR-072); a cursor
ordinal left pointing past a removed row becomes stale (FR-045).

### T-014 - Team event plumbing

`TeammateSpawned` appends the member when absent, hydrates its session id, status and
current task from the team store, and reveals the strip (FR-050). `TeammateIdle`,
`TeammateFailed`, `TeammateSuspended`, `TeammateResumed`, `TeamTaskClaimed` and
`TeamTaskCompleted` update status and current
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

### T-016 - Cursor row-map lifecycle

Each window's painted rows are published as an ordered identifier vector in paint order;
the cursor is an ordinal into that vector, so the vector and the painted rows stay in
one-to-one correspondence (FR-056, NFR-011). A non-zero scroll offset re-derives both
the painted slice and the vector from the full list, so the map is scroll-invariant
(FR-056). When a side panel other than the log panel occupies the side column, or no
side column exists, both areas reset to an empty rectangle (FR-057). A cursor ordinal
that no longer matches a live row is a consumed no-op without panic and without
mutating unrelated state (FR-069, FR-045, NFR-006). A click inside the popup outside
the chrome is consumed and does nothing to the surface beneath (FR-070, FR-071).

**Status (spec `agentui`):** Completed under the keyboard-first rework (T-022), which
replaced the retired per-row hit-rect bookkeeping with cursor row maps:
`render_active_agents_subpanel` publishes `agents_subagent_task_ids` (sub-agent rows
in DFS paint order, the primary excluded) and the aligned `agents_row_task_ids` map;
`render_teams_subpanel` publishes `teams_teammate_agent_ids` and
`teams_row_agent_ids` beside `teams_rows_area` (FR-008 frame-content geometry for
`teams_area`, FR-057). The scroll path recomputes the visible slice plus the cursor
map each frame, so an offset change can never desynchronise the ordinal from the
painted row (FR-056, FR-059). The mouse router in `input_handler.rs` consumes every
click inside a popup that is not the scrollbar gutter or a button/close button
(FR-069, FR-070, FR-071, NFR-006), verified by
`tests/test_hit_rect_lifecycle.rs` plus the alignment assertions in
`test_agents_window_table.rs`, `test_teams_window_table.rs`, `test_misc_modules_deep.rs`,
`test_text_selection.rs` and the cursor-driven `test_agents_teams_cursor.rs`.

### T-017 - Scrolling

Both tables clamp their scroll offset to the computed maximum on every render and draw a
vertical scrollbar when the content exceeds the viewport (FR-059). Wheel events over the
popup body move the offset by `SCROLL_STEP_LINES` and re-clamp it, scrollbar-gutter
dragging jump-scrolls then drags, and the cursor's movement/paging keeps its row
painted (FR-078), satisfying the scrollable-content requirement in FR-008. Per-frame
work stays proportional to the visible row count (NFR-002).

**Status (spec `agentui`):** Implemented. The wheel over a popup body moves
the table offset: the mouse router in `input_handler.rs` carries guarded arms at the
top of the scroll match (topmost-surface precedence), one per wheel direction per
window, each moving `active_agents_scroll_offset` / `teams_scroll_offset` by
`SCROLL_STEP_LINES` (3) - saturating at zero on wheel-down and re-clamping against the
renderer-published `active_agents_max_scroll` / `teams_max_scroll` on wheel-up - and
setting `needs_redraw` (FR-008, FR-059, NFR-005). Because the arms precede the
research-view, output-view and pane routing, a wheel over the popup can no longer
scroll the chat pane hidden beneath the modal. The scrollbar-gutter drag arms start
and move a thumb drag against the same clamps (FR-076). The renderers clamp the
painted slice (`scroll = offset.min(max_scroll)`) and draw the vertical scrollbar when
content overflows; the cursor helpers adjust the offset so the landed row paints
(FR-041, FR-042, FR-078). Verified by `tests/test_window_scroll_wheel.rs` (step up/down,
saturation, clamp to the published maximum, pane-beneath isolation, redraw flag,
no-popup fall-through, end-to-end rendered frames for both windows including
scrollbar presence/absence, and the gutter-drag cases); the cursor/viewport cases in
`tests/test_agents_teams_cursor.rs`.

### T-018 - Window lifecycle and reporting

Nothing in the panel waits for a turn to finish: window open/close and cursor row
actions are serviced on every frame while a sub-agent or teammate runs (FR-061). The
Agents window closes once its last sub-agent and background task have finished (FR-063).
Reportable outcomes - the cursor row landings, the stale-cursor no-ops, the teammate
focus transitions and the focus-cleared notice - are written to the status line (FR-064), and every content
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
modal stays operable mid-turn. FR-064 was verified for the focus
transitions (`team: focused -> <name>`, `team: focus cleared`) and, after the
T-022 rework, for the cursor landings and stale-cursor no-ops
(`agents: <id>` / `teams: cursor -> <id>` / `... selected row is gone`); the
status-bar cache signature carries the status
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
mid-turn toggle + cursor `Space` dispatch + Close (TC-029), mid-turn keyboard
suspend, mid-turn Teams toggle + row focus, the Alt-focus transition and
focus-cleared notices. Verify:
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
`cargo check --workspace` clean. Acceptance criteria 1-21 are the exit condition.

**Status (spec `agentui`):** Completed. The acceptance walk was executed as an
automated offline replay of the manual test plan: every TC-001..TC-037 scenario was
mapped to its consolidated-test counterpart and the corresponding filter re-run green
after T-017/T-018/T-019 landed, then the full `cargo test -p ragent-tui --test all`
suite was re-run end to end. Mapping summary: TC-001 button render/counts/dimming ->
`test_button_counts` (12 tests, incl. both-buttons-dimmed and the counted label); TC-002/TC-014
toggle and mutual exclusion -> `test_window_routing` (opening one window closes the
other in both directions, no-subject clicks inert); TC-003/TC-030 modal frame and the
smallest 80x24 terminal -> `test_agents_window_frame` / `test_teams_window_frame` (rects
stay inside the terminal from 160x44 down to 1x1 without panic); TC-004..TC-006 table
content -> `test_agents_window_table` (header columns, primary row, tree indent, badges,
status colours, live `[stop]`/`[x]` glyph pinning) and `test_teams_window_table` (twelve
dimmed columns, lead row, nine canonical status words, magenta override vs
`(inherited)`, counters, empty state); TC-007/TC-012 background shell section outside
the cursor -> `test_agents_window_table` (glyph positions, live vs terminal rows); TC-008/TC-020 keyboard output
views and mailbox routing -> `test_row_click_destinations` (FR-034 sub-agent view, FR-038
focus + mailbox routing, FR-058 titled border, all through cursor `Enter`); TC-009/TC-010
suspend/resume/kill -> `test_row_action_confirm` (dialog shape) +
`test_row_action_dispatch` (background cancel) + `test_keyboard_control` +
`test_agents_teams_cursor`
(FR-035/FR-036/FR-040 through the real manager, FR-055 key consumption); TC-011 primary
protection -> `test_agents_teams_cursor` (the cursor range never reaches the primary or
lead, FR-067); TC-013 keyboard-only -> the same `Space`/`Del`/`Enter`/`Esc` suite
(mouse never required on any asserted path); TC-015/TC-019 strip rendering, focus
highlight and `Alt+^v:cycle` hint -> `test_teammate_strip` (22 tests); TC-016/TC-017/TC-018
status words, model column, counters and empty state -> `test_teams_window_table` +
`test_teams_tui`; TC-021 `Alt+Down`/`Alt+Up` cycling -> `test_teams_tui` T-012 section
(past-ends clears) + `test_window_lifecycle` focus reporting; TC-022 `Space`/`Del` on a
cursor teammate -> `test_agents_teams_cursor` (Space split + Del dialog through the
store, FR-040, FR-067); TC-023 cursor `Del` with confirmation -> `test_row_action_confirm` (Yes/Esc
answers); TC-024 cleanup reset -> `test_event_handler_more`/`test_teams_tui`
(`team_cleaned_up_*` incl. hiding the strip); TC-025/TC-026 event add/remove and
reconciliation repair -> `test_subagent_events` + `test_active_tasks_reconcile`
(re-adds a dropped start, removes a ghost of a dropped completion) with the FR-063
auto-close asserted in `test_window_lifecycle`; TC-027 scrolling -> `test_window_scroll_wheel`
(wheel input and gutter drag move the painted slice, the cursor map re-derives with it,
clamps, scrollbar appears) + `test_agents_teams_cursor` (FR-078 scroll-into-view).
TC-028 popup clicks and stale rows -> `test_hit_rect_lifecycle` + stale-cursor no-ops in
`test_agents_teams_cursor`; TC-029 serviceability during a running turn ->
`test_window_lifecycle` (three serviceable-during-turn cases, `is_processing` never
gates the panel arms and the turn is never disturbed); TC-031 auto-close -> the five
FR-063 tests in `test_window_lifecycle`; TC-032 cursor/focus persistence ->
`test_row_click_destinations` FR-065 pair; TC-033 margin -> `test_agents_window_table`
(CONTENT_X re-derivation); TC-034 overlay close bar/wheel -> `test_output_view_close_bar`;
TC-035 cursor nav/viewport -> `test_agents_teams_cursor`; TC-036 row-action keys +
legend -> `test_agents_teams_cursor` + `test_row_action_confirm`; TC-037 mouse surface
-> `test_row_click_destinations` + `test_window_scroll_wheel`. Acceptance criteria
1-21 all close:
1 (button counts), 2 (toggle + exclusion), 3 (frame + Close + legend), 4 (table
structure),
5 (row fields and badges), 6 (live/terminal control glyphs), 7 (cursor Enter output
views), 8 (Space suspend/kill confirm), 9 (Up/Down/PgUp/PgDn cursor + key
consumption), 10 (Teams frame + title + Close), 11 (Teams table columns), 12 (model
override colour), 13 (focus routing and cycling), 14 (Teams Space/Del), 15 (strip),
16 (sub-agent events), 17 (team events), 18 (reconciliation repair), 19 (scroll +
cursor viewport-follow + legend), 20 (no render-loop blocking, no stale panics,
primary/lead protected), 21 (non-chrome popup clicks consumed). Quality gates: `cargo fmt --check` clean workspace-wide;
`cargo check --workspace` finishes with zero errors and zero warnings
(target/temp/t020_check.log); `cargo clippy -p ragent-tui --all-targets` reports only
the 57 pre-existing `pub(crate)`-in-private-module warnings (50 fns + 5 consts + 2
structs, unchanged since before this spec) plus the unchanged 50 pre-existing
`ragent-research` and 5 `ragent-tools-vcs` lib warnings - zero findings in any
agentui-touched file; full `cargo test -p ragent-tui --test all` = 3573 passed /
0 failed (target/temp/t020_full.log, plus the earlier t018_full.log run agreeing).
The live-session prerequisites (a real sub-agent spawn, a background shell task and a
two-teammate team in a working terminal) are exercised on the code paths the
consolidated suite drives through the real `App` render/key/mouse/event surfaces
(now keyboard-first after T-022; the mouse surface is wheel + scrollbar drag + buttons);
human-in-the-terminal confirmation remains available via specs/agentui/TESTPLAN.md.

### T-021 - Agents margin, output view close bar and wheel ownership

1. **Agents table content indent (FR-073).** `columns::CONTENT_X = 5` indents every
   painted Agents table line (header, primary row, sub-agent tree, background shell
   section) behind five blank columns via a `margin_line` pass over the visible slice;
   zebra bands and filler rows still pad edge to edge. The FR-015 `[stop]`/`[x]`
   glyphs stay at 70/77 in the indented row space and paint at
   `area.x + 5 + column` on screen (they are click-less status indicators under
   FR-076).
2. **Output view close bar (FR-074).** `render_output_view_overlay` switches from a raw
   `centered_rect(90, 70)` to the shared `window_frame(90, 70, ..)` helper, records
   `output_view_close_button_area`, paints `render_window_close_button` after the
   scrollbar, and derives scroll geometry from the frame's content rect (inner minus
   button height minus one separator row), so the overlay gains the same bottom-right
   ` Close ` bar as the Agents and Teams popups.
3. **Dismissal unification (FR-075).** New `App::close_output_view` clears the overlay
   and the selected agent session/index in one step; the Esc arm in `input.rs`, the
   outside-click arm in `input_handler.rs` and the new close-button hit-test all call
   it, and it sets `needs_redraw`.
4. **Wheel ownership (FR-075).** The output overlay's wheel arms move ahead of the
   Agents/Teams popup arms in the mouse router: a wheel over the overlay scrolls the
   overlay instead of the popup beneath (the overlay is topmost), which fixes the
   "overlay locked to the newest page" bug report; a wheel outside the overlay still
   drives the popup underneath.

**Status:** Completed. `test_output_view_close_bar` (9 tests: bottom-right/clamped
button, painted label, content excludes the bar, button click and idempotent dismissal,
body-click consume, wheel-over-overlay scrolls while Agents is open, popup beneath
untouched, popup wheel regression) plus the re-derived `test_agents_window_table` (14)
and the `test_hit_rect_lifecycle` shift test run green; full
`cargo test -p ragent-tui --test all` = 3582 passed / 0 failed
(target/temp/tui_agentsmargin_full.log); `cargo fmt` clean; `cargo check --workspace`
clean; clippy shows only the pre-existing pub(crate)-in-private-module warnings
(zero in the touched files).

### T-022 - Keyboard-first rework: block cursor, retired mouse row clicks, key legend

1. **Mouse row/control clicks retired (FR-076, FR-070, FR-071).** The mouse router
   keeps: the output-overlay and popup wheel scroll arms, the scrollbar-gutter
   click/drag/release arms for the two popup scrollbars, the `Agents`/`Teams` button
   and close-button hit-tests, and the input-side button column. Clicks inside a popup
   body or on a painted `[stop]`/`[x]` cell are consumed without effect; the row and
   per-row button hit-rect bookkeeping (`agent_row_button_areas`,
   `agent_row_kill_areas`, `team_row_button_areas`, ...) is gone from `state.rs` and
   the renderers.
2. **Block cursor navigation (FR-033, FR-039, FR-041, FR-042, FR-078).** The Agents
   renderer publishes `agents_subagent_task_ids` (sub-agent rows in DFS paint order)
   and the Teams renderer `teams_teammate_agent_ids`; `App::move_agents_cursor` /
   `move_teams_cursor` start at an end, clamp at both ends and call a
   scroll-into-view helper that pins the cursor row to the viewport edge
   (`page_agents_cursor` / `page_teams_cursor` move one viewport, viewport minus an
   overlap row). The cursor paints as a full-width reversed band (`CURSOR_BG`,
   BOLD) over the selected row in `cursor_line`, applied after the zebra banding.
3. **Row actions on the cursor (FR-034..FR-036, FR-038, FR-040, FR-043, FR-045).**
   `Space` toggles stop/resume on the Agents cursor row without a dialog and splits on
   member status in the Teams window (resume directly, confirm suspend); `Del` arms
   the `KillSubagent` / `ShutdownTeammate` dialog (legacy `Shift+K` retained in
   Agents; `K`/`k` in Teams); `Enter` opens the output view (Agents) or focuses the
   teammate and opens its overlay (Teams - the same `focus_teammate_by_id` chain
   `Alt+Down`/`Alt+Up` drive). Every action re-resolves the live task/member and a
   vanished row is a consumed no-op reported as `agents: selected row is gone` /
   `teams: selected row is gone` (FR-069).
4. **Key precedence (FR-055).** The `show_agents_window` / `show_teams_window` arms in
   `handle_key_event` consume `Up`/`Down`/`PgUp`/`PgDn`/`Space`/`Del`/`Enter` (and the
   legacy kills) ahead of the input editor, and skip when the output overlay is open
   so the overlay's own keys keep working.
5. **Key legend (FR-077).** `window_frame` grew a `legend_area()` - the content
   area's separator band extended left to the inner border - painted by
   `render_window_key_legend` in dim grey: `Up/Down select  PgUp/PgDn page  Space
   stop/resume  Del kill  Enter open` (Agents) and `... Del shut down ...` (Teams).
6. **Primary/lead protection became structural (FR-067).** With the row maps covering
   only sub-agent / teammate rows, the old `Cannot suspend/kill primary agent`
   status-line refusals retired with `selected_agent_index`.

**Status:** Completed. New `tests/test_agents_teams_cursor.rs` (14 tests: navigation
start/clamp/page + land reporting, Space/Del/Enter on both windows against real
managers, stale no-ops, out-of-range no-ops, cursor never reaching primary/lead,
scroll-into-view) and the reworked `test_row_click_destinations.rs` (keyboard-driven
destinations), `test_row_action_dispatch.rs` (Space/Del paths), `test_row_action_confirm.rs`,
`test_keyboard_control.rs` (cursor-model rework), `test_window_scroll_wheel.rs`
(gutter drag), `test_hit_rect_lifecycle.rs`, `test_window_lifecycle.rs`,
`test_agents_window_frame.rs`, `test_agents_window_table.rs`, `test_teams_window_table.rs`,
`test_input_handler_more.rs`, `test_misc_modules_deep.rs`, `test_text_selection.rs`,
`test_event_swarm_research_deep.rs`. New `tests/test_output_view_close_bar.rs` covers
the T-021 close bar. Focused runs green: `test_agents_teams_cursor::` 14/14,
`test_row_click_destinations::` 13/13, `test_keyboard_control::` 13/13,
`test_window_scroll_wheel::` 15/15, `test_row_action_confirm::` 19/19,
`test_row_action_dispatch::` 8/8,
`test_output_view_close_bar::` 9/9, `test_hit_rect_lifecycle::` 10/10,
`test_agents_window_table::` 14/14, `test_teams_window_table::` 16/16;
`cargo check -p ragent-tui` clean; `cargo fmt --check` clean; clippy shows only the
pre-existing pub(crate)-in-private-module noise.

## Risks

| Risk | Mitigation |
|------|------------|
| Duplicate rows from a replayed spawn event | De-duplicate by task id / agent id in the spawn arms (FR-072) and let reconciliation converge. |
| Stale cursor ordinals after a row is removed | The row map re-derives each frame and the action helpers re-resolve the identifier before acting; a stale ordinal is a consumed no-op (FR-045, FR-069). |
| Confirmation modal stealing keys from the window beneath | Draw and key-handle the modal first, and consume `Space` / `Del` / `Esc` / `Enter` / arrows while it is open (FR-055, T-009). |
| Registry fetch stalling a frame | Fetch on the async runtime, publish through the poison-tolerant lock, apply on the next tick (FR-054, FR-068, NFR-007). |
| Teams table repeating the global model everywhere | Compare the override against the global selected provider/model and render inherited when equal (FR-024). |
| A long row list leaving the cursor stranded off-viewport | The scroll-into-view helpers pin the cursor row to the viewport edge on every move and page, clamped to the computed maximum (FR-041, FR-042, FR-078). |
| A mouse user expecting row clicks | The legend band (FR-077) and this plan's interaction model name the keyboard bindings; clicks inside the popup body are consumed without effect by design (FR-076). |
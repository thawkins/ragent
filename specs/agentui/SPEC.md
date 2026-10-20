---
status: draft
audit:
  - { time: 1792429906, from: "none", to: "draft", actor: "system" }
  - { time: 1792495400, from: "draft", to: "draft", actor: "cursor-rework (T-022): keyboard-first FR rewrites, mouse retirement, legend, spec text refreshed against the implemented block-cursor model" }
---
# Specification: Agent and Team Member Panel - Live Sub-Agent and Teammate Control Surface

## Executive Summary

This specification defines the **Agent and Team Member panel** of the ragent Terminal
User Interface (TUI): the two popup windows opened by the `Agents` and `Teams` buttons
that sit beside the message input, the single-row teammate strip under the status bar,
the row layout of each window, the keyboard-first control surface attached to those
rows, and the event plumbing that keeps the rows live while sub-agents and teammates
run.

The panel is the user's only in-terminal view of *where the work is happening*. It
lists the primary agent, every spawned sub-agent task and background shell task in the
`Agents` window, and the lead plus every teammate of the active team in the `Teams`
window. Each row carries an identity, a live status, an elapsed time, a tool-call step
count, and - while the row is still live - two action control columns that indicate
suspend/resume or kill. A keyboard block cursor selects a row: `Space` toggles
stop/resume, `Del` arms the destructive confirm, and `Enter` opens the row's output
view. Opening a teammate's row also routes the message input to that teammate's
mailbox. Mouse row clicks are retired: the only mouse input the windows accept is
wheel scrolling, scrollbar-gutter dragging and the window buttons.

The panel performs **no scheduling, spawning, or task execution of its own**. Every
action it offers is a thin delegation to an existing entry point
(`AgentManager::suspend_task` / `resume_task` / `kill_task`,
`TeamManager::suspend_teammate` / `resume_teammate` / `shutdown_teammate`,
`BackgroundTaskService::cancel`), and every row it draws is a projection of state that
already exists in `App` (`active_tasks`, `bg_tasks`, `team_members`, `active_team`,
`event_bus` tool-call counters, and the on-disk team store).

### Why this exists

An autonomous agent run is not one process. The primary agent spawns sub-agents, the
team runtime spawns teammates, and the background shell service runs detached commands.
Without a dedicated surface the user sees a chat transcript scrolling past but cannot
answer the three questions that matter while a run is expensive and long: *what is
running right now*, *how long has each unit been running and how much has it done*, and
*how do I stop just that one*. The panel answers all three without leaving the chat
screen, and without the destructive option of stopping the whole session.

### Worked examples

The frames below are illustrative. They are drawn in ASCII and elide the right-hand
columns; the renderer paints the line-drawing connectors described in FR-011 and FR-022,
and the full column set of the Teams table (FR-020).

```text
+- Agents -----------------------------------------------------------------------+
|   id         name                        type      elapsed   steps  >[stop] [x]|
| * lead       general                     primary         -       7             |
| o a1b2c3d4   `- explore-a1b2c3d4         bg          2m14s      12  [stop]  [x]|  <-- cursor
| o e5f6a7b8      `- rust-coder [C]        fg            41s       3  [stop]  [x]|
| o 99887766         `- reviewer [T]       bg             9s       0  [stop]  [x]|
|  Background shell tasks                                                        |
| o cc112233   cargo build --release       bg          1m02s       -  [stop]  [x]|
| Up/Down select  PgUp/PgDn page  Space stop/resume  Del kill  Enter open  Close |
+--------------------------------------------------------------------------------+
```

The `*` marker identifies the primary agent row, which is outside the cursor range and
never carries the action glyphs. Each sub-agent row is indented by its ancestor tree,
carries a `[C]` badge when its agent definition is a custom agent and a `[T]` badge
when its session is one of the active team's teammates, and hides both glyphs once it
reaches a terminal status. The bottom band is the FR-077 legend; the cursor row paints
as a full-width reversed band (`CURSOR_BG`), indicated here with `<-- cursor`.

```text
+- Teams -------------------------------------------------------------------------+
|   id       name                            status     model              elapsed
| * a1b2c3d4 general                         active     lead                 3m02s
| > |- e5f6a7b8 reviewer-e5f6a7b8               working    anthropic/claude     1m41s  <-- cursor
|  `- 99887766 tester-99887766                 idle       (inherited)            58s
|  Team: alpha  (lead + 2 teammates)
| Up/Down select  PgUp/PgDn page  Space stop/resume  Del shut down  Enter open  Close
+---------------------------------------------------------------------------------+
```

The `>` marker shows the focused teammate: that teammate's pill is highlighted in the
strip under the status bar, its output view is open, and a message submitted from the
input is delivered to its mailbox instead of the lead session. The bottom band is the
FR-077 legend with the Teams wording (`Del shut down`).

## Scope & Objectives

### Scope

**In scope:** the Agents and Teams buttons; both popup windows and their geometry,
titles, headers, close buttons and content areas; the primary, sub-agent, background
shell, lead and teammate row layouts; row badges and status colouring; the `[stop]`,
`>`, and `[x]` control glyphs; the keyboard block cursor and its bindings
(`Up`/`Down`, `PgUp`/`PgDn`, `Space`, `Del`, `Enter`); the key legend band; the
retained mouse surfaces (wheel scrolling, scrollbar-gutter dragging, the window and
close buttons); the teammate strip; teammate focus and its
input routing; `Alt+Down` / `Alt+Up` cycling; the event handlers that keep rows live;
the registry reconciliation safety net; scrolling and scrollbars; and the tests for all
of the above.

**Out of scope:** spawning sub-agents or teammates (spec `spawnagent` and the team
runtime); team lifecycle slash commands (spec `team`); plan approval dialogs; the output
view overlay itself; the log panel; task-store persistence; the LLM delegation policy;
and any change to the agent or team crate APIs.

### Objectives

1. Answer "what is running, how long, how much has it done, and how do I stop just that
   one" without leaving the chat screen or stopping the whole session.
2. Give the user one in-terminal place to see every sub-agent task, background shell task
   and teammate, with live status, elapsed time and step count.
3. Make the destructive row actions available only on live rows, with the primary agent
   and team lead protected.
4. Route input and output to a chosen teammate so a team can be driven from one terminal
   without a second session.
5. Keep the panel live from the event stream, with a reconciliation safety net so a
   dropped event cannot leave a phantom row.

## Constraints & Assumptions

### Constraints

1. Every action the panel offers delegates to an existing entry point; no new
   scheduling, spawning or execution code is introduced.
2. All work is confined to `crates/ragent-tui`; the agent, team and storage crates are
   consumed exactly as they are.
3. The panel is a projection of existing `App` state; it keeps no second copy of task or
   member state.
4. Panel text is ASCII apart from the fixed glyph set the panel already uses for
   structure and status (tree connectors `|`, `|-`, `` `- ``; markers `*`, `>`; status
   icons), which are listed in one place rather than scattered through the renderers.

### Assumptions

Read these before implementing; they resolve every ambiguity this spec could otherwise
carry into the plan stage.

- **A1 - Two windows, not one.** `Agents` and `Teams` are separate popups with separate
  state flags and separate renderers. They are *mutually exclusive*: opening either
  closes the other. They are not two tabs of one modal.
- **A2 - Rows are projections.** A row is derived from `App` state on every frame; the
  panel keeps no authoritative copy of task or member state. `active_tasks` and
  `team_members` are hydrated by events, and `active_tasks` is additionally reconciled
  against the task registry (`AGENTS_RECONCILE_INTERVAL` = 1500 ms, plus an immediate
  pass when the event-bus lag counter changes).
- **A3 - Selection is a keyboard block cursor.** The selectable rows of each window
  are published during render as ordered identifier vectors (`agents_subagent_task_ids`,
  `teams_teammate_agent_ids`) and the cursor is an ordinal into that vector painted as
  a full-width reversed band. Scrolling re-derives the painted rows, so the cursor map
  is scroll-invariant by construction - no per-click offset arithmetic exists.
- **A4 - The cursor range covers sub-agent and teammate rows only.** The primary agent
  row and the team lead row are never part of the cursor list, so no row-action key or
  mouse path can select, suspend or kill them (protection is structural, not a refusal
  branch).
- **A5 - The primary agent and the lead are protected.** As a defence in depth beyond
  A4's structural exclusion, a suspend/resume/kill request that would name the
  primary agent or the lead is refused (FR-067), and an action key pressed with no
  valid cursor row is a consumed no-op.
- **A6 - Focus is a route, not a highlight.** `focused_teammate` changes two behaviours
  beyond the highlight: `Enter` on a non-empty input sends to that teammate's mailbox,
  and the input border title changes to `-> <name> (focused)`.
- **A7 - Keyboard-first, mouse second.** Every row action has a keyboard binding
  (`Space`, `Del`, `Enter`, arrows, PgUp/PgDn); the mouse retains only the surfaces
  keyboards cannot do naturally - wheel scrolling, scrollbar-gutter dragging, the two
  input-side window buttons and the window/output close buttons. No mouse row click or
  control click exists.
- **A8 - Ambiguity in "all its functionality".** Read as: the two popup windows, the
  teammate strip, the two buttons beside the input, the row content, the keyboard
  cursor control surface and the retained mouse surfaces, teammate focus and its input
  routing, the event-driven updates
  that keep rows live, the reconciliation safety net, and scrolling. Recorded as Open
  Question 1.

## Definitions

| Term                             | Meaning                                                                                                                                                                                                  |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Agents window**          | The centred popup listing the primary agent, sub-agent tasks, and background shell tasks (A1).                                                                                                           |
| **Teams window**           | The centred popup listing the active team's lead and teammates (A1).                                                                                                                                     |
| **Active-agents subpanel** | The table renderer shared by the Agents window body.                                                                                                                                                     |
| **Sub-agent task**         | A `TaskEntry` in `App::active_tasks`, one per `SubagentStart`.                                                                                                                                     |
| **Background shell task**  | A `BgTaskView` in `App::bg_tasks`, one per `bg` tool spawn.                                                                                                                                        |
| **Teammate**               | A `TeamMember` of the active `TeamConfig`, rendered in the Teams window and the strip.                                                                                                               |
| **Lead**                   | The running session that created the team; always the first row of the Teams window.                                                                                                                     |
| **Row action controls**  | The `[stop]` / `>` suspend-resume glyph and the `[x]` kill glyph painted at the right edge of a live row. They are status indicators; the actions are driven by the keyboard cursor (FR-035, FR-036, FR-040, FR-076). |
| **Block cursor**         | The full-width reversed-colour band over the keyboard-selected row; an ordinal into the window's ordered row-identifier vector (A3).                                                                       |
| **Cursor row map**       | The ordered identifier vector (`agents_subagent_task_ids`, `teams_teammate_agent_ids`) the renderer publishes each frame; keyboard actions resolve the cursor ordinal against it (A3, NFR-011).             |
| **Focused teammate**       | The teammate named by `App::focused_teammate`; receives routed input (A6).                                                                                                                             |
| **Terminal status**        | For a task: `Completed`, `Failed`, or `Cancelled`. For a member: `ShuttingDown`, `Stopped`, or `Failed`.                                                                                     |
| **Strip**                  | The single-row teammate pill bar rendered directly below the status bar.                                                                                                                                 |
| **Glyph set**              | The fixed structural and status characters the panel paints (tree connectors, the `*` primary/lead marker, the `>` focus marker, and the status icons), declared once per renderer module (NFR-003). |
| **Reconciliation**         | The periodic merge of the authoritative task registry snapshot into `App::active_tasks`.                                                                                                               |

## Background - existing machinery to reuse

- **Window renderers.** `crates/ragent-tui/src/layout.rs`:
  `render_agents_window_overlay` (70 x 70 centred modal via `window_frame`, `Clear`
  widget, cyan `Agents` title, key legend, `Close` button),
  `render_teams_window_overlay` (90 x 70, blue `Teams` title), `window_frame`,
  `render_window_close_button`, `render_window_key_legend`, `draw_input_side_buttons`,
  `render_teammate_strip`.
- **Table renderers.** `crates/ragent-tui/src/layout_active_agents.rs`
  (`render_active_agents_subpanel`, `build_task_rows`, `cursor_line`) and
  `crates/ragent-tui/src/layout_teams.rs` (`render_teams_subpanel`, `status_label`,
  `count_cell`, `cursor_line`).
- **State.** `crates/ragent-tui/src/app/state.rs`: `show_agents_window`,
  `show_teams_window`, `agents_button_area`, `teams_button_area`,
  `agents_close_button_area`, `teams_close_button_area`, `output_view_close_button_area`,
  `active_agents_area`, `teams_area`, `teams_rows_area`,
  `active_agents_scroll_offset`, `active_agents_max_scroll`,
  `teams_scroll_offset`, `teams_max_scroll`, `agents_cursor_subagent`,
  `agents_subagent_task_ids`, `agents_row_task_ids`, `teams_cursor_teammate`,
  `teams_teammate_agent_ids`, `teams_row_agent_ids`, `row_action_confirm`,
  `focused_teammate`, `team_message_counts`.
- **Cursor helpers and key routing.** `crates/ragent-tui/src/app/windows.rs`
  (`move_agents_cursor`, `page_agents_cursor`, `agents_cursor_*`, `move_teams_cursor`,
  `page_teams_cursor`, `teams_cursor_*`, scroll-into-view helpers) and the
  `show_agents_window` / `show_teams_window` arms of `handle_key_event` in
  `crates/ragent-tui/src/app/input_handler.rs`; `crates/ragent-tui/src/input.rs` for
  `Alt+Up` / `Alt+Down` focus cycling.
- **Mouse handling.** `crates/ragent-tui/src/app/input_handler.rs`: wheel scroll arms
  over the popup bodies, scrollbar-gutter drag start/drag/release
  (`ScrollbarDragPane::AgentsWindow` / `TeamsWindow`), the Agents/Teams button and
  close-button hit-tests, and the consume arms that retire every other click inside a
  popup (FR-070, FR-076).
- **Actions.** `crates/ragent-tui/src/app/event_handler.rs`: `suspend_agent_task`,
  `resume_agent_task`, `kill_agent_task`, `cancel_bg_task` (dialog re-resolution path),
  `cycle_focused_teammate`, `focus_teammate_by_id`; `crates/ragent-tui/src/app/session_ops.rs`:
  `open_output_view_session`, `refresh_team_member_session_ids`.
- **Event handlers.** `crates/ragent-tui/src/app/event_handler.rs` arms for
  `SubagentStart`, `SubagentComplete`, `SubagentCancelled`, `SubagentSuspended`,
  `SubagentResumed`, `TeammateSpawned`, `TeammateMessage`, `TeammateP2PMessage`,
  `TeammateIdle`, `TeammateFailed`, `TeamTaskClaimed`, `TeamTaskCompleted`,
  `TeamCleanedUp`.
- **Reconciliation.** `App::poll_active_tasks_reconcile` and
  `App::apply_registry_reconciliation` in `crates/ragent-tui/src/app/state.rs`.
- **Shared helpers.** `format_elapsed` (compact `2m14s` / `1h5m`), `short_id`
  (8-character suffix), `short_session_id`, `centered_rect`.

## Functional Requirements

#### Ubiquitous requirements

### FR-001 - Agent and team buttons beside the input

`The ragent TUI shall render an Agents button and a Teams button inside the bordered button column immediately to the left of the message input on every chat-screen frame.`

The button column is sized by `input_side_buttons_total_width` so neither label is
truncated, and it narrows the input field rather than the reverse.

### FR-002 - Live task count in the Agents label

`The ragent TUI shall render the Agents button label as "Agents (<n>)" where the count is the sum of the active sub-agent task count and the background shell task count, and shall render the bare label "Agents" when that sum is 0.`

### FR-003 - Live member count in the Teams label

`The ragent TUI shall render the Teams button label as "Teams (<n>)" where the count is the number of team members whose status is not ShuttingDown, Stopped or Failed, and shall render the bare label "Teams" when that count is 0.`

### FR-004 - Disabled button appearance

`The ragent TUI shall render the Agents button dimmed when no sub-agent task and no background shell task exists, and shall render the Teams button dimmed when no team is active.`

### FR-005 - Active button appearance

`While a window is open and its subject exists, the ragent TUI shall render the corresponding button with a white foreground on a blue background and a bold modifier.`

### FR-006 - Agents window geometry

`The ragent TUI shall render the Agents window as a centred modal overlay occupying 70 percent of the terminal width and 70 percent of the terminal height, cleared beneath, framed with full box-drawing borders, and titled " Agents " in bold cyan.`

### FR-007 - Agents window close button

`The ragent TUI shall render a bordered " Close " button in the bottom-right corner of the Agents window inner area, clamped to at most 10 columns and 3 rows so it always fits, and shall render the FR-077 key legend in the separator band to the Close button's left.`

### FR-008 - Agents window content area

`The ragent TUI shall compute the Agents window table area as the inner area minus the close-button height and one separating row, and shall clamp that table area to a minimum height of 1 row.`

`The content area should be scrollable so that large numbers of agents or team memberes can be accessed by scrolling.`

### FR-009 - Agents table header

`The ragent TUI shall render a leading header row in the Agents table naming the columns id, name, type, elapsed, steps, [stop] and [x] in dimmed hint colour.`

### FR-010 - Primary agent row

`The ragent TUI shall render the primary agent as the first row of the Agents table, marked with a green asterisk, carrying the literal lead in the id column, the configured agent name in bold green, the literal primary in the type column, a "-" elapsed value and the live tool-call count for the primary session.`

### FR-011 - Sub-agent tree rows

`The ragent TUI shall render each sub-agent task as a tree row beneath its parent session, indenting by one two-column segment per ancestor and prefixing each row with a tree branch glyph chosen from the last-child and non-last-child forms, and shall append that row's task identifier to the cursor row map in paint order.`

### FR-012 - Sub-agent row content

`The ragent TUI shall render each sub-agent row with a status dot, the last 8 characters of the task identifier, an agent label of at most 32 columns including badges, the literal bg or fg in the type column, the compact elapsed time since task creation, and the live tool-call count for the child session.`

### FR-013 - Row badges

`The ragent TUI shall append a [C] badge to a row whose agent name resolves to a custom agent definition and a [T] badge to a row whose child session belongs to a member of the active team, reserving 4 columns per badge inside the 32-column agent label.`

### FR-014 - Row status presentation

`The ragent TUI shall colour a running row yellow, a suspended row dark grey with a " [||]" suffix, and a terminating row red with a " ..." suffix.`

### FR-015 - Row action control glyphs

`While a task row status is not terminal, the ragent TUI shall paint a six-column [stop] control at column 70 of that row and a four-column [x] control at column 77 of that row, replacing the [stop] glyph with ">" when the task is suspended.`

These glyphs are status indicators only (FR-037's cursor exclusion, FR-076): the
suspend/resume toggle, the kill confirmation and the output view are keyboard actions
on the cursor row (FR-035, FR-036, FR-034).

### FR-016 - Background shell task section

`The ragent TUI shall render a Background shell tasks heading followed by one row per background shell task, showing its status dot, short identifier, command truncated to 40 columns, the literal bg, elapsed time, and the same [stop] and [x] glyphs while the task is live.`

### FR-017 - Teams window geometry

`The ragent TUI shall render the Teams window as a centred modal overlay occupying 90 percent of the terminal width and 70 percent of the terminal height, cleared beneath, framed with full box-drawing borders, and titled " Teams " in bold blue.`

### FR-018 - Teams window close button

`The ragent TUI shall render a bordered " Close " button in the bottom-right corner of the Teams window inner area, clamped to at most 10 columns and 3 rows, shall dismiss the window when that button is clicked, and shall render the FR-077 key legend in the separator band to the Close button's left.`

### FR-019 - Teams window title

`The ragent TUI shall title the Teams table " Team: <name>  (lead + <n> teammate) " with the plural form "teammates" when the member count is not exactly 1.`

### FR-020 - Teams table header

`The ragent TUI shall render a leading header row in the Teams table naming the columns id, name, status, model, elapsed, steps, claimed, done, sent, recv, [stop] and [x] in dimmed hint colour.`

### FR-021 - Teams lead row

`The ragent TUI shall render the lead as the first row of the Teams table, marked with a green asterisk, showing the short lead session identifier, the configured agent name, the literal active in the status column, the literal lead in the model column, the elapsed time derived from the persisted session creation timestamp, the live tool-call count, and "-" in the claimed and done columns.`

### FR-022 - Teams teammate rows

`The ragent TUI shall render each teammate as a row beneath the lead, prefixed with a branch connector glyph, a ">" focus marker when the member is the focused teammate, the short agent identifier, and a 31-column label of the form <name>-<short id>, and shall append that row's agent identifier to the Teams cursor row map in paint order.`

### FR-023 - Teammate status labels

`The ragent TUI shall render each teammate status as one of the words spawning, working, idle, planning, blocked, suspended, stopping, stopped or failed, each in its own colour.`

### FR-024 - Teammate model column

`The ragent TUI shall render a teammate model override as "<provider>/<model>" truncated to 17 columns in magenta, and shall render the literal (inherited) in hint colour when the member carries no override.`

`The Teammamte model should only be rendered if it different to the current global selected provider/model.`

### FR-025 - Teammate task and message counters

`The ragent TUI shall render the claimed and done counts from the active team task store and the sent and recv counts from the per-agent message counter map, rendering the literal "-" for a zero sent or recv count and colouring a non-zero count in its category colour.`

### FR-026 - Teams empty state

`While the active team has no members, the ragent TUI shall render the hint line "(no teammates yet - use team_spawn tool or blueprint)" beneath the lead row.`

### FR-027 - Teammate strip

`While a team is active and has at least one member, the ragent TUI shall render a single-row strip directly below the status bar containing one pill per teammate with a status icon, a name truncated to 16 columns, a distinct background for the focused teammate, and the hint " Alt+^v:cycle " at the right edge.`

#### Event-driven requirements

### FR-028 - Agents button click toggles the window

`When the user left-clicks the Agents button while at least one sub-agent task or background shell task exists, the ragent TUI shall toggle the Agents window and shall close the Teams window if the Agents window is being opened.`

### FR-029 - Agents button click without tasks

`If the user left-clicks the Agents button while no sub-agent task and no background shell task exists, the ragent TUI shall perform no action and shall leave the Agents window in its current open or closed state.`

### FR-030 - Teams button click toggles the window

`When the user left-clicks the Teams button while a team is active, the ragent TUI shall toggle the Teams window and shall close the Agents window if the Teams window is being opened.`

### FR-031 - Teams button click without a team

`If the user left-clicks the Teams button while no team is active, the ragent TUI shall perform no action and shall leave the Teams window in its current open or closed state.`

### FR-032 - Close button click

`When the user left-clicks the close button of an open window, the ragent TUI shall close that window and leave every task, member, and input value unchanged.`

### FR-033 - The primary row is not selectable

`The ragent TUI shall exclude the primary agent row of the Agents window and the lead row of the Teams window from the keyboard cursor's row map, so no cursor navigation or row-action key can select, suspend or terminate them.`

The cursor range starts at the first sub-agent row (Agents) and the first teammate row
(Teams); `Up` from the first row clamps there.

### FR-034 - Enter on a cursor row opens the output view

`When the user presses Enter while the Agents window cursor is on a sub-agent row, the ragent TUI shall open the output view for that task's child session labelled with the agent name and the short task identifier.`

### FR-035 - Space toggles sub-agent stop/resume

`When the user presses Space while the Agents window cursor is on a sub-agent row, the ragent TUI shall request suspension of that running task, or resumption of that task when it is suspended, without a confirmation dialog.`

### FR-036 - Del arms the sub-agent kill confirmation

`When the user presses Del while the Agents window cursor is on a sub-agent row, the ragent TUI shall arm the destructive yes/no confirmation for terminating that task and shall terminate it only when the user answers Yes.`

The legacy `Shift+K` binding keeps the same behaviour.

### FR-037 - Background shell rows are outside the cursor

`The ragent TUI shall keep the keyboard cursor out of the background shell task section of the Agents window; the section's [stop] and [x] glyphs remain status indicators, and a background task's lifetime is driven by its completion or cancellation events.`

### FR-038 - Enter on a teammate row focuses and opens

`When the user presses Enter while the Teams window cursor is on a teammate row, the ragent TUI shall set that teammate as the focused teammate, open its output view, and set the status line to "team: focused -> <name>".`

### FR-039 - The lead row is outside the cursor

`The ragent TUI shall keep the lead row of the Teams window outside the cursor map, so no row-action key can focus or terminate the lead.`

Clearing focus remains possible through `Alt+Up` / `Alt+Down` cycling (FR-046) and the
`/team` slash surface.

### FR-040 - Space and Del act on the cursor teammate

`When the user presses Space while the Teams window cursor is on a teammate row, the ragent TUI shall request resumption of that suspended teammate without a dialog, and shall arm the destructive yes/no confirmation for suspending it otherwise; when the user presses Del, the ragent TUI shall arm the destructive yes/no confirmation for gracefully shutting that teammate down.`

`before the teammate is suspended or shut down, the panel should present a yes/no modal, to ensure that the action is as the user intended.`

### FR-041 - Up and Down move the block cursor

`While the Agents or Teams window is open, the ragent TUI shall move that window's block cursor one selectable row per Up or Down press, start the cursor on the first row when moving down and the last row when moving up, and clamp at both ends of the row list.`

### FR-042 - PgUp and PgDn page the cursor

`While the Agents or Teams window is open, the ragent TUI shall move that window's block cursor by one viewport of rows per PgUp or PgDn press, clamped to the row list.`

The scroll-into-view side of paging is FR-078.

### FR-043 - Cursor movement reports the landed row

`When cursor navigation lands on a row without performing an action, the ragent TUI shall report the landed row's identifier in the status line and set the redraw flag.`

### FR-044 - Enter opens the cursor row's output view

`When the user presses Enter while the Teams cursor is on a teammate row, the ragent TUI shall open that row's output view through the same destination chain the focus helpers use; in the Agents window Enter is defined by FR-034.`

(The Agents-chain wording is FR-034; this FR covers the Teams Enter + focus chain.)

### FR-045 - Stale cursor actions are consumed no-ops

`If a row-action key resolves to a cursor ordinal whose row no longer exists in the window's row map or live state, the ragent TUI shall consume the key, report the stale selection in the status line and mutate nothing.`

### FR-046 - Alt focus cycling remains

`When the user presses Alt+Down or Alt+Up, the ragent TUI shall move teammate focus forward or backward through the member list, shall open the newly focused teammate's output view, and shall clear focus together with the output view when the cycle moves past the last member.`

### FR-047 - Subagent start adds a row

`When a SubagentStart event arrives for the current session or a descendant session, the ragent TUI shall append a running task entry to the active task list so the Agents window shows the sub-agent on the next frame.`

### FR-048 - Subagent completion removes the row

`When a SubagentComplete event arrives for a tracked task, the ragent TUI shall record the report status derived from the finish reason, remove the row from the active task list, and log the completion with a truncation marker when the provider cut the reply.`

Removal covers every terminal route, including a task killed from the cursor
(FR-036) and a registry reconciliation that drops a finished ghost (FR-054): in each
case the removed row's cursor ordinal becomes stale and is treated per FR-045.

### FR-049 - Subagent lifecycle updates

`When a SubagentCancelled, SubagentSuspended or SubagentResumed event arrives for a tracked task, the ragent TUI shall remove, mark suspended, or mark running the corresponding row so the painted status matches the runtime status.`

### FR-050 - Teammate spawn adds a member

`When a TeammateSpawned event arrives for the current session, the ragent TUI shall append the member when it is not already present, hydrate its session identifier, status and current task from the on-disk team store, and make the team strip visible.`

### FR-051 - Teammate status events

`When a TeammateIdle, TeammateFailed, TeammateSuspended, TeammateResumed, TeamTaskClaimed or TeamTaskCompleted event arrives, the ragent TUI shall update that member's status and current task identifier, and shall record teammate progress so the watchdog does not flag an active teammate as hung; a member driven to a Stopped status by its shutdown request paints accordingly until the stop event or cleanup removes it.`

### FR-052 - Message counter events

`When a TeammateMessage or TeammateP2PMessage event arrives, the ragent TUI shall increment the sender's sent count and the recipient's received count, ignoring a sender or recipient named lead.`

### FR-053 - Team cleanup resets the panel

`When a TeamCleanedUp event arrives for the current session, the ragent TUI shall clear the active team, the member list, the message counters and the focused teammate, and shall hide the team strip and the Teams window.`

### FR-054 - Registry reconciliation

`When the event-bus lag counter changes or 1500 milliseconds have elapsed since the last reconciliation, the ragent TUI shall fetch the task registry off the render thread and merge it into the active task list, removing entries whose registry status is no longer running and adding registry entries that are running but unpainted.`

#### State-driven requirements

### FR-055 - Window keys take precedence over input

`While the Agents window or the Teams window is open, the ragent TUI shall route Up, Down, PgUp, PgDn, Space, Del and Enter to that window's cursor before any input-buffer handling so none of these keys can insert a character or move the text caret.`

### FR-056 - Cursor row maps track scrolling

`While a table's scroll offset is non-zero, the ragent TUI shall re-derive the painted row slice and its ordered identifier vector from the full row list plus the offset, so the cursor resolves to the row painted at its ordinal on every frame.`

The retired per-button hit-rect shift no longer exists: only the window buttons and
the close buttons carry `Rect` targets, and none of those scroll with the table.

### FR-057 - Areas are cleared when a panel is hidden

`While a side panel other than the log panel occupies the side column, and while no chat side column exists, the ragent TUI shall reset the active-agents area and the teams area to an empty rectangle so no hidden surface can receive input.`

### FR-058 - Focused teammate routes input

`While a teammate is focused, the ragent TUI shall title the input border " -> <name> (focused) " in bold yellow and shall deliver a submitted message to that teammate's mailbox instead of the lead session.`

### FR-059 - Scrollbar for a long list

`While the number of table lines exceeds the visible rows, the ragent TUI shall render a vertical scrollbar on the right edge of that table, shall clamp the scroll offset to the computed maximum, and shall start a scrollbar-gutter thumb drag (with jump-to-position) when a click lands on the scrollbar's column inside a popup table and keep it moving until the button is released.`

### FR-060 - Teammate hydration on render

`While rendering the Teams table, the ragent TUI shall refresh every member's session identifier, status and current task from the on-disk team store before painting, so a hydration race cannot leave a stale spawning status on screen.`

### FR-061 - Window state survives a running turn

`While a sub-agent or teammate is executing, the ragent TUI shall continue to service window open, window close and cursor row actions on every frame without waiting for the turn to finish.`

### FR-062 - Teammate badge derivation

`While rendering the Agents table, the ragent TUI shall derive the teammate session set from the current member list and the custom agent name set from the loaded custom agent definitions, so the [T] and [C] badges reflect the current configuration.`

#### Optional requirements

### FR-063 - Window closes when its list empties

`Where the Agents window is open and its last sub-agent task and background shell task have finished, the ragent TUI shall close the window.`

### FR-064 - Status line reporting

`Where an action outcome is reportable, the ragent TUI shall report it in the status line, including the cursor row landings, the stale-cursor no-ops, the teammate focus transitions, and the focus-cleared notice.`

### FR-065 - Cursor and focus survive a window reopen

`Where a window has been closed and is reopened within the same session, the ragent TUI shall preserve the previously placed cursor ordinal and the previously focused teammate.`

Where the preserved ordinal no longer resolves inside the reopened window's row map,
the ragent TUI shall treat it as stale (FR-045).

### FR-066 - Strip visibility

`Where a team exists with no members, the ragent TUI shall omit the teammate strip so no empty pill row occupies a screen line.`

#### Unwanted requirements

### FR-067 - No primary or lead termination

`If a suspend, resume or kill request would target the primary agent or the team lead, the ragent TUI shall refuse the action, report the refusal, and shall leave the running turn untouched.`

Under the cursor model this refusal is structural (FR-033, FR-039): the primary and
lead rows are absent from both row maps, so no navigation key can reach them. The
refusal branch remains as a defence for the `Shift+K` legacy path and for any future
selection surface that could name them.

### FR-068 - No render-thread blocking

`If a suspend, resume, kill, shutdown or cancellation action is requested, the ragent TUI shall dispatch it on the async runtime and shall never block the render loop while the request is in flight.`

### FR-069 - No panic on a stale cursor or index

`If a cursor ordinal, selected index or close-button rect no longer matches a live task or member, the ragent TUI shall ignore the action without panicking and without mutating unrelated state.`

### FR-070 - Popup clicks outside an interactive control are consumed

`If the user left-clicks inside an open Agents or Teams popup, the ragent TUI shall consume the click unless it lands on the scrollbar gutter (thumb drag, FR-059) or a window button, and shall perform no action on the surface beneath it.`

Row and control clicks inside the popup body are retired: the cursor owns selection.
A click on the dimmed background outside a popup is out of scope here; only the
popup's own area is claimed.

### FR-071 - Only chrome carries click targets

`If a Rect target inside either popup would map to a table row or a table control cell, the ragent TUI shall not record it; only the window buttons, the close buttons and the scrollbar gutter resolve clicks, so a retired control cell can never resolve to a live task or member.`

### FR-072 - No duplicate rows

`If a spawn event arrives for a task identifier or agent identifier that is already painted, the ragent TUI shall update the existing row rather than appending a second one.`

### FR-073 - Agents table content margin

`The ragent TUI shall paint every Agents table line with a five-column blank leading margin, so the header row, the primary row, the sub-agent tree, the background shell section and the zebra bands all indent behind that margin while the scrollbar stays anchored to the right edge of the table.`

The FR-015 `[stop]` and `[x]` control glyphs (columns 70 and 77) are painted behind the
margin, so a live row paints them at `table.x + 5 + 70` and `table.x + 5 + 77` on
screen. The glyphs are click-less status indicators (FR-037, FR-076).

### FR-074 - Output view close bar

`The ragent TUI shall render the output view overlay with a bordered " Close " button in the bottom-right corner of its inner area, clamped to at most 10 columns and 3 rows, and shall compute the overlay content area as the inner area minus the button height and one separating row, reusing the window frame helper the Agents and Teams popups share.`

### FR-075 - Output view dismissal and wheel ownership

`The ragent TUI shall close the output view overlay through one state transition that clears the overlay together with the selected agent session and index, shared by the Esc key, a click outside the overlay and the FR-074 Close button. Closing an already-closed overlay is a no-op. While the overlay is open, a mouse wheel over it scrolls the overlay even though the Agents or Teams window lies beneath it, because the overlay is the topmost surface, and the popup beneath must not scroll from that event.`

### FR-076 - Mouse input is limited to scroll and chrome

`The ragent TUI shall accept mouse input on the Agents and Teams windows only for wheel scrolling over the table bodies, scrollbar-gutter dragging on the two popup scrollbars, the input-side Agents and Teams buttons, the window Close buttons and the output view Close button; every other mouse event over the popups shall be consumed without effect.`

In particular no left-click selects a row, toggles a control or opens an output view:
all row interaction is keyboard-owned (FR-034, FR-035, FR-036, FR-038, FR-040).

### FR-077 - Window key legend

`While the Agents or Teams window is open, the ragent TUI shall render a dim key legend in the separator band to the left of the window's Close button listing the cursor bindings (movement, paging, stop/resume, kill/shut-down and open), clipped to the available width and painted empty when that width is zero.`

### FR-078 - Scroll follows the cursor

`When cursor navigation or paging moves the cursor outside the painted viewport, the ragent TUI shall adjust that table's scroll offset so the cursor row is painted, pinning it to the top or bottom edge as needed and keeping the offset clamped to the computed maximum.`

## Non-Functional Requirements

### NFR-001 - Deterministic offline tests

`The ragent TUI shall render both popup windows, the teammate strip and every row variant from synthetic in-memory state so the agent panel test suite runs offline and deterministically inside the standard test command.`

### NFR-002 - Bounded per-frame work

`The ragent TUI shall complete one Agents window or Teams window frame without issuing a database query per row beyond the single team task-store read and the single session lookup described, so a frame cost stays proportional to the visible row count.`

### NFR-003 - Single source for the glyph set

`The ragent TUI shall declare every structural and status glyph the panel paints in one place per renderer module, so a glyph change cannot leave the Agents window, the Teams window and the teammate strip visually inconsistent.`

### NFR-004 - Safe truncation

`The ragent TUI shall truncate every identifier, name, command and error string on a character boundary so a multi-byte sequence can never be split and no render path can panic on truncation.`

### NFR-005 - Explicit redraw signalling

`The ragent TUI shall set the redraw flag for every action that changes a window's painted content so the next frame reflects the change without waiting for an unrelated event.`

### NFR-006 - No unsafe code

`The ragent TUI shall implement the panel without unsafe code, relying on safe indexing and explicit bounds checks for every cursor ordinal and identifier-vector lookup.`

### NFR-007 - Poison-tolerant locks

`The ragent TUI shall read and write the shared reconciliation result through a poison-tolerant lock helper so a panic in one task cannot wedge the panel.`

### NFR-008 - Log attribution

`The ragent TUI shall attribute each panel-triggered log line to the affected child session or teammate session where that session is known, so the log panel and the output view agree on ownership.`

### NFR-009 - Stable status vocabulary

`The ragent TUI shall render teammate status words and task status words from the single canonical mapping in the agent crate so the panel cannot disagree with tool output or the persisted team store.`

### NFR-010 - Responsive minimum

`The ragent TUI shall clamp every popup area to the terminal rectangle so the panel remains fully visible and clickable at the smallest supported terminal size.`

### NFR-011 - Cursor map index alignment

`The ragent TUI shall keep each cursor row-identifier vector aligned with the painted row list of its window, in paint order, so a cursor ordinal always resolves to the row painted at that ordinal in the same frame.`

### NFR-012 - Test isolation

`The ragent TUI shall drive the panel tests without writing into the repository working tree, using only in-memory state and the ignored build scratch directory.`

## Interaction model

The windows are keyboard-first with a block cursor; the mouse keeps only the surfaces
keyboards cannot do naturally:

| Surface           | Keys                                                                                     | Mouse                                                                 |
| ----------------- | ----------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| Window open/close | the window buttons stay click-only (no key binding)                                        | click the same button, or the Close button                              |
| Row selection     | `Up` / `Down` move the block cursor; `PgUp` / `PgDn` page one viewport (FR-041, FR-042)    | none (FR-076); wheel scroll and scrollbar-gutter drag remain            |
| Sub-agent action  | `Space` = suspend/resume (FR-035), `Del` = kill confirm (FR-036), `Enter` = open (FR-034)  | none (FR-076)                                                           |
| Teammate action   | `Space` = resume / suspend-confirm (FR-040), `Del` = shutdown confirm, `Enter` = focus (FR-038) | none (FR-076)                                                      |
| Teammate focus    | `Alt+Down` / `Alt+Up` cycle (FR-046)                                                       | none (FR-076)                                                           |
| Scrolling         | the cursor drags the viewport with it (FR-078); PgUp/PgDn page                              | wheel over the popup body; scrollbar-gutter thumb drag (FR-059)         |
| Legend            | rendered in the separator band left of Close (FR-077)                                       | -                                                                       |

Both windows are mutually exclusive (A1). While a window is open its cursor keys are
consumed before the input editor sees them (FR-055), so no key can leak a character.
The scroll offset, cursor ordinal and row maps re-derive together on every frame, so
cursor-visible state can never disagree (FR-056, FR-078).

## Error handling

| Condition                             | Behaviour                                                                                                      |
| ------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Row-action key with no visible cursor | The key starts the cursor on the nearest end first (FR-041); a later press acts on that row.    |
| Action key on a stale cursor row      | Consumed no-op; the status line reports the stale selection; state is untouched (FR-045, FR-069). |
| Action key on the primary or lead     | Structurally unreachable (FR-033, FR-039, FR-067).                                                            |
| Button clicked with nothing to show   | No action; the window keeps its current state (FR-029, FR-031).                                                |
| Suspend / resume / kill request fails | Warn-level trace with the task identifier and error; the row is reconciled on the next registry poll (FR-054). |
| Teammate shutdown request fails       | Warn-level trace with the teammate identifier; the row keeps its current status until the next event.          |
| Task registry unavailable             | Reconciliation returns early; painted rows remain until an event arrives.                                      |
| Team store unreadable while rendering | Member hydration returns early; rows paint from in-memory state (FR-060).                                      |

## Acceptance criteria

1. The Agents and Teams buttons render beside the input on every chat frame, carry live
   counts, and are dimmed when their subject does not exist.
2. Left-clicking the Agents button opens the Agents window; clicking it again closes it;
   opening it closes the Teams window; the same holds in the reverse direction for Teams.
3. The Agents window is a bordered, centred modal titled `Agents` with a working `Close`
   button in its bottom-right corner.
4. The Agents table shows a header row, a `*` primary row, tree-indented sub-agent rows
   and a background shell task section when such tasks exist.
5. A sub-agent row shows the short identifier, the agent label, `bg`/`fg`, elapsed time,
   the live step count, and the `[C]` / `[T]` badges when applicable.
6. A live row paints `[stop]` (or `>`) and `[x]` as status glyphs; a terminal row paints
   neither.
7. Pressing `Enter` on the cursor's sub-agent row opens that task's output view
   (FR-034); the primary row is outside the cursor range (FR-033).
8. `Space` on the cursor's sub-agent row suspends a running task and resumes a suspended
   one (FR-035); `Del` arms the kill confirmation that terminates on `Yes` (FR-036); the
   window stays open.
9. `Up`/`Down` move the cursor with start-at-end and end-clamp behaviour (FR-041);
   `PgUp`/`PgDn` page one viewport (FR-042); every window action key is consumed by the
   window (FR-055).
10. The Teams window is a bordered, centred modal titled `Teams`, titled
    `Team: <name>  (lead + N teammate(s))` on its table, with a working close button.
11. The Teams table shows the documented columns, a `*` lead row with `active` and `lead`
    values, and one row per teammate with status word, model column, elapsed, steps,
    claimed, done, sent and recv values.
12. A teammate model override renders in magenta as `<provider>/<model>`; a member with no
    override renders `(inherited)`.
13. Pressing `Enter` on the cursor's teammate row focuses it, opens its output view and
    routes subsequent input to its mailbox (FR-038); `Alt+Down` / `Alt+Up` cycle focus
    and clear it past the ends (FR-046); the lead row is outside the cursor (FR-039).
14. `Space` on the cursor's suspended teammate resumes it without a dialog and arms the
    suspend confirmation otherwise; `Del` arms the shutdown confirmation (FR-040); the
    `[stop]`/`[x]` glyphs reflect the member's live status.
15. The teammate strip appears under the status bar when at least one teammate exists,
    highlights the focused teammate, and shows `Alt+^v:cycle` at the right edge.
16. `SubagentStart`, `SubagentComplete`, `SubagentCancelled`, `SubagentSuspended` and
    `SubagentResumed` add, remove and re-status rows without a manual refresh.
17. `TeammateSpawned`, `TeammateIdle`, `TeammateFailed`, `TeamTaskClaimed`,
    `TeamTaskCompleted`, `TeammateMessage`, `TeammateP2PMessage` and `TeamCleanedUp`
    update the Teams window, the counters and the strip in the same way.
18. A dropped spawn or completion event is repaired by the registry reconciliation within
    the documented interval.
19. A non-zero table scroll offset shifts the painted rows and re-derives the cursor row
    map with them (FR-056), the offset is clamped to the computed maximum, and a
    scrollbar appears when the content exceeds the viewport (FR-059); wheel scrolling and
    scrollbar-gutter dragging move the offset (FR-076), cursor paging drags the viewport
    along (FR-078), and the legend band left of each window's Close button lists the
    cursor keys (FR-077).
20. No panel action blocks the render loop, panics on a stale row, or terminates the
    primary agent or team lead.
21. Except for the scrollbar gutter, the window buttons and the close buttons, a click
    over an open popup is consumed without effect (FR-070, FR-076).

## Open Questions

1. **Scope of "all its functionality".** A8 reads the panel as the two popups plus the
   strip plus the buttons plus focus routing plus the event plumbing. If the strip should
   be specified separately (it is reachable only with a team), split it into its own spec.
2. ~~**Keyboard row selection in the Agents window.**~~ **Resolved by the cursor rework
   (T-022):** the block cursor driven by `Up`/`Down`/`PgUp`/`PgDn` is the selection
   surface; `Space`, `Del` and `Enter` act on it (FR-041 .. FR-046).
3. ~~**`Space` on a focused teammate shutting it down.**~~ **Resolved by the cursor
   rework (T-022):** `Space` on the cursor teammate resumes when suspended and arms the
   suspend confirm otherwise; shutdown moved to `Del` (FR-040).
4. **Retaining selection across a reopen.** FR-065 now preserves the cursor ordinal and
   focused teammate across a reopen, with a stale ordinal treated as a no-op. Confirm
   this is still wanted rather than resetting the cursor on close.
5. **Background shell task kill semantics.** With FR-037 the background section is
   outside the cursor: cancellation is driven by completion/cancellation events and the
   cancel dialog survives only as a re-resolution path. Confirm no persistent keyboard
   cancel binding is wanted for background rows.
6. ~~**No scrolling input.**~~ **Resolved by the cursor rework (T-022):** PgUp/PgDn page
   the cursor one viewport (FR-042), the cursor drags the viewport with it (FR-078), and
   wheel scrolling plus scrollbar-gutter dragging move the offset directly (FR-076,
   FR-059).

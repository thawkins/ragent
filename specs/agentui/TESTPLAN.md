---
status: accepted
---

# Manual Test Plan: Agent and Team Member Panel

**Spec:** [SPEC.md](SPEC.md) · **Plan:** [PLAN.md](PLAN.md)

This is a **manual** test plan. Every case is executed by a human in a real terminal
session. Where a case needs a running sub-agent, a background shell task, or a team,
the prerequisite is listed before the steps. This document contains no
automated test code and no reference to any test harness.

The panel is operated **keyboard-first through a block cursor**; the mouse keeps only
the scroll and chrome surfaces (window buttons, close buttons, wheel scrolling,
scrollbar-gutter dragging). Every key press is written explicitly, in order, and the
exact text to type into each field is given as **Test data**. Keys are written as
`Space`, `Del`, `Up`, `Down`, `PgUp`, `PgDn`, `Alt+Down`, `Alt+Up`, `Enter`, `Esc`,
and literal characters in `backticks`. A mouse step names the button and the
exact target: `left-click the Agents button beside the message input`.

## Prerequisites

1. **Build the binary.** Build a debug binary and confirm `--version` reports the
   expected version string.
2. **Configure a model.** Cases that need a live turn require a working provider (a
   local Ollama model is fine) and a confirmed short response.
3. **Mouse support is optional.** The windows are keyboard-first: all row interaction
   is keyboard-owned (cursor + `Space`/`Del`/`Enter`) and scrolling has keyboard
   bindings (PgUp/PgDn). The mouse is used only for the window buttons, the close
   buttons, wheel scrolling and scrollbar-gutter dragging (TC-027, TC-034); TC-013 is
   the keyboard-only case and needs no mouse reporting at all.
4. **Prepare a scratch root.** Create `~/scratch/agentui-tests/` and run every session
   from inside it so the project `.ragent/` directory is local to the test.
5. **Know how to produce a background shell task.** Ask the agent to run a long command,
   e.g. `sleep 45` through the bash tool, so a background task row exists for a while.
6. **Know how to produce a sub-agent.** Ask the agent to spawn a sub-agent, e.g.
   `spawn an explore agent to summarise the README`.
7. **Know how to produce a team.** Ask the agent to create a team with two teammates,
   or load a team blueprint, then confirm the teammate strip appears below the status
   bar.

Throughout, "status line" means the line immediately above the input area, and
"elapsed" means the compact duration such as `2m14s` or `1h5m`.

## Test Cases

### TC-001 - Buttons render beside the input with live counts

**Preconditions:** Fresh session, no sub-agents, no background tasks, no team.

**Steps:**
1. Start the TUI in `~/scratch/agentui-tests/`.
2. Observe the bordered column immediately left of the message input.
3. Ask the agent: `run sleep 30 in the background`.
4. Observe the Agents button label while the background task runs.
5. Wait for the background task to finish.
6. Observe the Agents button label again.

**Test data:** `run sleep 30 in the background`

**Expected results:**
- Step 2: two buttons are present, labelled exactly `Agents` and `Teams` (bare labels,
  no counts), both dimmed.
- Step 4: the Agents label becomes `Agents (1)`; neither label is truncated and the
  input field is narrower than a session with no buttons.
- Step 6: the label returns to `Agents` and the button is dimmed again.

---

### TC-002 - Agents button opens and closes the window

**Preconditions:** At least one sub-agent task or background shell task exists.

**Steps:**
1. Start a background shell task so `Agents (1)` is visible.
2. Left-click the Agents button.
3. Observe the popup.
4. Left-click the Agents button again.
5. Left-click the Agents button once more, then press nothing and left-click a point
   outside the popup but inside the chat area.

**Test data:** none

**Expected results:**
- Step 3: a centred modal appears, framed with full box-drawing borders, titled
  ` Agents ` in bold cyan; the area beneath is cleared (no chat text showing through).
- Step 4: the popup closes; the chat returns unchanged, and every task keeps running.
- Step 5: the popup reopens and stays open; a click outside the popup body does not
  dismiss it.

---

### TC-003 - Agents window is modal-sized and its content area is laid out correctly

**Preconditions:** Agents window can be opened (one background task or sub-agent).

**Steps:**
1. Open the Agents window.
2. Note the popup's width relative to the terminal width and its height relative to the
   terminal height.
3. Resize the terminal to roughly 80 columns by 24 rows.
4. Re-open the window if it closed.
5. Locate the ` Close ` button.

**Test data:** none

**Expected results:**
- Step 2: the popup occupies roughly 70% of the width and 70% of the height, centred.
- Step 3-4: the popup stays inside the terminal rectangle; nothing is clipped off
  screen; the table area is at least one row tall.
- Step 5: a bordered ` Close ` button sits in the bottom-right corner of the window's
  inner area and is at most 10 columns wide and 3 rows tall; the key legend reads
  `Up/Down select  PgUp/PgDn page  Space stop/resume  Del kill  Enter open` in the band
  to its left (FR-077).

---

### TC-004 - Agents table header and primary row

**Preconditions:** Agents window open.

**Steps:**
1. Open the Agents window with no sub-agents (a background task is enough).
2. Read the first two lines of the table.
3. Confirm the primary row carries no `[stop]` and no `[x]`.

**Test data:** none

**Expected results:**
- Step 2: the first line is a dimmed header naming `id`, `name`, `type`, `elapsed`,
  `steps`, `[stop]`, `[x]`.
- Step 2: the second line starts with a green `*`, then the literal `lead` in the id
  column, the configured agent name in bold green, the literal `primary` in the type
  column, `-` in the elapsed column, and a numeric live step count.
- Step 3: the primary row shows no action glyphs - and it never becomes the cursor row
  (FR-033).

---

### TC-005 - Sub-agent rows are tree-indented and carry a live status

**Preconditions:** Agents window open; two sub-agents spawned from the same parent, one
still running.

**Steps:**
1. Ask the agent: `spawn an explore agent to summarise the README, and spawn a
   plan agent to outline a refactor`.
2. Open the Agents window while both run.
3. Observe the sub-agent rows.
4. Wait for one sub-agent to finish and observe the row list again.

**Test data:** `spawn an explore agent to summarise the README, and spawn a plan agent to outline a refactor`

**Expected results:**
- Step 3: each sub-agent appears on its own row beneath the parent session, indented by
  two columns per ancestor, prefixed with a tree branch glyph (`|-` or `` `- ``).
- Step 3: each row shows a status dot, the last 8 characters of the task id, the agent
  name (at most 32 columns), `bg` or `fg` in the type column, a compact elapsed time and
  a live step count that increases over time.
- Step 3: the running row is yellow and paints `[stop]` and `[x]` at the right edge
  (status glyphs; the actions come from the cursor keys).
- Step 4: the finished sub-agent's row is gone from the list.

---

### TC-006 - Row badges and status styling

**Preconditions:** Agents window open; a custom agent definition exists (a custom agent
under the user or project agent directory); an active team has at least one teammate
that is also running a sub-agent.

**Steps:**
1. Spawn a sub-agent using the custom agent definition.
2. Spawn a sub-agent from inside a teammate session.
3. Open the Agents window.
4. Move the cursor onto a running sub-agent and press `Space`, then observe its row.
5. Kill a running sub-agent (`Del` + `Yes`) and observe the row immediately after
   confirming.

**Test data:** none

**Expected results:**
- Step 3: the custom-agent row shows a `[C]` badge appended to its agent name.
- Step 3: the teammate's sub-agent row shows a `[T]` badge appended to its agent name.
- Step 3: both badges fit inside the agent-label column; the name is truncated if needed
  and never splits a multi-byte character.
- Step 4: the suspended row is dark grey with a ` [||]` suffix and its control changes
  from `[stop]` to `>`.
- Step 5: the terminating row is red with a ` ...` suffix; once it reaches a terminal
  status the row disappears.

---

### TC-007 - Background shell task section

**Preconditions:** At least one background shell task is running.

**Steps:**
1. Ask the agent to run a long command in the background, e.g. `run a background shell
   command that sleeps for 60 seconds and prints a progress dot every 5 seconds`.
2. Open the Agents window.
3. Observe the section under the sub-agent rows.

**Test data:** `run a background shell command that sleeps for 60 seconds and prints a progress dot every 5 seconds`

**Expected results:**
- Step 3: a `Background shell tasks` heading appears, followed by one row per task.
- Step 3: each row shows a status dot, a short identifier, the command truncated to 40
  columns, the literal `bg`, a compact elapsed time, and both `[stop]` and `[x]`
  status glyphs (indicators only - the section is outside the cursor, FR-037).

---

### TC-008 - Opening a row's output view with the cursor

**Preconditions:** Agents window open with a running sub-agent.

**Steps:**
1. Press `Down` to place the cursor on a running sub-agent row.
2. Press `Enter` and observe the surface that opens and its label.
3. Close that view (Esc or the overlay's ` Close ` button) and reopen the Agents
   window if needed.
4. Move the cursor to another sub-agent row and press `Enter` again.
5. Observe the label on the opened view.

**Test data:** none

**Expected results:**
- Step 2: an output view opens for that task's child session, labelled with the agent
  name and the short task id (FR-034).
- Step 4-5: the newly opened view labels the row under the cursor. The primary row is
  never reachable by the cursor (FR-033), so no key opens the primary view.

---

### TC-009 - Sub-agent suspend, resume and kill on the cursor row

**Preconditions:** Agents window open with a running sub-agent.

**Steps:**
1. Press `Down` to place the cursor on the running sub-agent row.
2. Press `Space`.
3. Observe the row and the agent's output view.
4. Press `Space` again.
5. Press `Del`, read the confirmation, and dismiss it with `No` (or `Esc`).
6. Press `Del` again and choose `Yes`.
7. Observe the window.

**Test data:** none

**Expected results:**
- Step 2: no dialog; the task suspends directly (FR-035): the row turns dark grey,
  gains ` [||]`, and its control glyph becomes `>`. The status line reports the landed
  row (FR-043).
- Step 4: the task resumes (FR-035): the row returns to yellow with `[stop]`, and its
  step count starts increasing again.
- Step 5: dismissing the kill dialog (FR-036) changes nothing.
- Step 6: choosing `Yes` terminates the task; the row is marked terminating and then
  removed. The Agents window stays open.

---

### TC-010 - Cursor navigation, paging and key consumption

**Preconditions:** Agents window open with more rows than fit the viewport.

**Steps:**
1. Press `Down` (nothing selected yet) and read the status line.
2. Press `Up`.
3. Press `Up` again at the top row.
4. Press `PgDn`, then `PgUp`.
5. Open the message input and confirm it stayed empty throughout.

**Test data:** none

**Expected results:**
- Step 1: the cursor starts on the first row and the status line reports it
  (`agents: <short id>`, FR-041, FR-043).
- Step 2: the cursor moves to the last row (start-at-end on a first up-press).
- Step 3: the cursor clamps at the last row (FR-041).
- Step 4: the cursor pages one viewport per press, clamped, and the landed row is
  painted (FR-042, FR-078).
- Step 5: the input buffer contains no characters - every window key was consumed
  (FR-055).

---

### TC-011 - The primary row is outside the cursor

**Preconditions:** Agents window open with at least one sub-agent row.

**Steps:**
1. Press `Up` repeatedly to drive the cursor to the first sub-agent row.
2. Read the status line.
3. Press `Up` once more.
4. Read the status line; try `Del` and `Space`.
5. Confirm the primary turn is still running and the primary row never highlights.

**Test data:** none

**Expected results:**
- Step 2: the status line names the first sub-agent row.
- Step 3-4: the cursor clamps at the first sub-agent row; the status line keeps naming
  that row and no primary-refusal text appears because the primary row is simply not
  in the cursor list (FR-033, FR-067).
- Step 5: no suspension or termination occurred; the running turn is untouched and the
  chat continues to stream.

---

### TC-012 - Background shell task rows

**Preconditions:** Agents window open with at least one live background shell task.

**Steps:**
1. Observe the background task row's `[stop]` and `[x]` glyphs while the task runs.
2. Move the cursor to the first sub-agent row and press `Up` again, trying to reach
   the background section.
3. Wait for the background task to complete (or cancel it from the surface that
   spawned it).
4. Observe the row and the Agents window.

**Test data:** none

**Expected results:**
- Step 1: the glyphs paint while the task is live and disappear when it reaches a
  terminal status.
- Step 2: the cursor clamps at the first sub-agent row (FR-037): the background
  section is outside the cursor, so no key can place the cursor on a background row.
- Step 3-4: completion (or cancellation) removes or re-statuses the row through its
  event, exactly as for sub-agent rows; the window stays responsive.

---

### TC-013 - Keyboard-only operation (no mouse reporting)

**Preconditions:** A terminal session with mouse reporting unavailable. Agents window
opened beforehand or already open.

**Steps:**
1. Press `Down` to place the cursor on a sub-agent row.
2. Press `Space` to suspend it, then `Space` again to resume.
3. Press `Del`, read the confirmation, then press `Esc`.
4. Press `Del` again and confirm with `Enter` on `Yes`.
5. Press `PgDn`, then `PgUp`.
6. Press `Enter` with the cursor on a row.
7. Observe the input buffer.

**Test data:** none

**Expected results:**
- Steps 1-6: every action, including paging, is reachable without the mouse
  (FR-041, FR-042, FR-035, FR-036, FR-034).
- Step 3: `Esc` dismisses the confirmation without performing the action.
- Step 7: no character from any of the consumed keys appears in the input buffer
  (FR-055).

---

### TC-014 - Teams button opens the Teams window and both windows are exclusive

**Preconditions:** An active team with at least one teammate; a background task so the
Agents window can also open.

**Steps:**
1. Left-click the Teams button.
2. Observe the popup, its title and its table title.
3. Left-click the Agents button without closing the Teams window.
4. Observe which window is on screen.
5. Left-click the Teams button again.
6. Left-click the ` Close ` button.

**Test data:** none

**Expected results:**
- Step 2: a centred modal with full borders and a bold blue ` Teams ` title; the table
  title reads ` Team: <name>  (lead + 2 teammates) ` for two teammates and
  `... (lead + 1 teammate) ` for exactly one; the key legend in the band left of
  ` Close ` reads `Up/Down select  PgUp/PgDn page  Space stop/resume  Del shut down
  Enter open` (FR-077).
- Step 3-4: the Teams window closes and the Agents window opens — the two are never
  shown at once.
- Step 5-6: the Teams window reopens and the Close button dismisses it, leaving every
  member and the input unchanged.

---

### TC-015 - Teams table content: lead row and teammate rows

**Preconditions:** Active team with a lead and two teammates, one working and one idle.

**Steps:**
1. Open the Teams window.
2. Read the header row.
3. Read the first data row.
4. Read the teammate rows.

**Test data:** none

**Expected results:**
- Step 2: a dimmed header naming `id`, `name`, `status`, `model`, `elapsed`, `steps`,
  `claimed`, `done`, `sent`, `recv`, `[stop]`, `[x]`.
- Step 3: the lead row starts with a green `*`, shows the short lead session id, the
  configured agent name, the literal `active` in the status column, the literal `lead`
  in the model column, an elapsed time derived from the session start, a live step
  count, and `-` in claimed and done. The lead row is outside the cursor range
  (FR-039).
- Step 4: each teammate row is prefixed with a branch connector, shows the short agent
  id and a `<name>-<short id>` label, a status word, a model value, elapsed, steps,
  claimed, done, sent and recv. Zero sent/recv render as `-`; non-zero counts are
  coloured.

---

### TC-016 - Teammate status words and colours

**Preconditions:** Active team with at least one teammate you can drive through several
states (spawn it fresh, then let it work, then let it go idle).

**Steps:**
1. Open the Teams window and watch a teammate from the moment it spawns.
2. Record the status word at spawn, while working, while idle, when suspended and when
   stopping/stopped.
3. Compare the wording with the value shown by tool output for the same teammate.

**Test data:** none

**Expected results:**
- Step 2: each state renders one of `spawning`, `working`, `idle`, `planning`,
  `blocked`, `suspended`, `stopping`, `stopped`, `failed`, each in its own colour.
- Step 2: no stale `spawning` word persists after the teammate begins working.
- Step 3: the panel wording matches the tool output and the persisted team record.

---

### TC-017 - Teammate model column shows only a genuine override

**Preconditions:** Active team with one teammate that inherits the global model and one
that overrides it with a different provider/model.

**Steps:**
1. Note the model shown in the status bar (the global selected provider/model).
2. Open the Teams window.
3. Read the model column for the inheriting teammate.
4. Read the model column for the overriding teammate.
5. Change the global model from the status bar, then reopen the Teams window.

**Test data:** none

**Expected results:**
- Step 3: the inheriting teammate shows `(inherited)` in hint colour — it never repeats
  the status-bar label.
- Step 4: the overriding teammate shows `<provider>/<model>` in magenta, truncated to 17
  columns with no partial multi-byte character.
- Step 5: the columns re-derive against the new global model; the previously overriding
  teammate still shows its override.

---

### TC-018 - Teammate counters and the empty state

**Preconditions:** Active team; one case with members, one case with a team that has no
members yet.

**Steps:**
1. With members, send a message to a teammate and have a teammate message another
   teammate.
2. Open the Teams window and read the `sent` and `recv` columns.
3. Have a teammate claim and complete a task and read `claimed`/`done`.
4. Create a team with a lead and no teammates and open the Teams window.

**Test data:** a short message such as `status update please`

**Expected results:**
- Step 2: the sender's `sent` and the recipient's `recv` increment; a `lead` sender or
  recipient does not increment any counter; a zero count renders as `-`, a non-zero one
  in its category colour.
- Step 3: `claimed` and `done` reflect the team task store.
- Step 4: the hint line `(no teammates yet - use team_spawn tool or blueprint)` is
  painted beneath the lead row, and the teammate strip is absent.

---

### TC-019 - Teammate strip renders and highlights the focused teammate

**Preconditions:** Active team with at least two teammates.

**Steps:**
1. Observe the row directly below the status bar.
2. Move the cursor onto a teammate row in the Teams window and press `Enter` to focus
   it.
3. Observe the strip.

**Test data:** none

**Expected results:**
- Step 1: a single strip with one pill per teammate, each carrying a status icon and a
  name truncated to 16 columns, and the hint ` Alt+^v:cycle ` at the right edge.
- Step 3: the focused teammate's pill carries a distinct background while the others do
  not.

---

### TC-020 - Focusing a teammate routes input to that teammate

**Preconditions:** Active team with two teammates; Teams window open.

**Steps:**
1. Press `Down` to place the cursor on the first teammate row.
2. Press `Enter`, then observe the input border title and the status line.
3. Type `please report your progress` into the input and press `Enter`.
4. Observe the lead session and the teammate.
5. Press `Alt+Down` past the last member so focus clears.
6. Observe the input border title and the status line.
7. Type `hello lead` and press `Enter`.

**Test data:** `please report your progress`, then `hello lead`

**Expected results:**
- Step 2: the input border is titled ` -> <name> (focused) ` in bold yellow; the status
  line reads `team: focused -> <name>`; the teammate's output view is open (FR-038).
- Step 3-4: the message is delivered to that teammate's mailbox, not the lead session;
  the teammate responds in its own output view (FR-058).
- Step 5-6: focus clears (FR-046), the border title returns to normal.
- Step 7: the message goes to the lead session as usual.

---

### TC-021 - Alt+Down / Alt+Up cycle teammate focus

**Preconditions:** Active team with three teammates; no teammate focused.

**Steps:**
1. Press `Alt+Down` three times, observing after each press.
2. Press `Alt+Down` once more.
3. Press `Alt+Up`.
4. Press `Alt+Up` repeatedly until focus clears.

**Test data:** none

**Expected results:**
- Step 1: focus moves forward through the member list one teammate per press; the
  newly focused teammate's output view opens each time and its pill highlights.
- Step 2: the cycle moves past the last member and focus clears, together with the
  output view.
- Step 3: focus moves backward to the last member.
- Step 4: focus clears when the cycle runs past the first member.

---

### TC-022 - Space and Del on the cursor teammate

**Preconditions:** Teams window open; the cursor sits on a working (not suspended)
teammate row.

**Steps:**
1. Press `Space`, read the dialog, choose `Yes`.
2. Observe the teammate row.
3. Move the cursor to another teammate, press `Del`, and confirm with `Yes`.
4. Verify no character leak into the input buffer.

**Test data:** none

**Expected results:**
- Step 1-2: the suspend confirmation arms (FR-040); on `Yes` the member suspends, the
  row shows the suspended status word and its `[stop]` glyph becomes `>`.
- Step 3: the shutdown confirmation arms; on `Yes` the teammate shuts down gracefully
  and the row moves through `stopping` to `stopped`.
- Step 4: neither key inserted a character into the input buffer (FR-055).

---

### TC-023 - Teammate suspend/resume/shutdown with confirmation

**Preconditions:** Teams window open with a working teammate.

**Steps:**
1. Move the cursor onto the working teammate row, press `Space`, and answer `No`.
2. Press `Space` again and answer `Yes`.
3. Press `Space` on the now-suspended row (it resumes without a dialog).
4. Press `Del` on a teammate and answer `Yes`.

**Test data:** none

**Expected results:**
- Step 1-2: a yes/no confirmation naming the teammate and the suspend action appears
  (FR-040); `No` dismisses it and nothing changes; `Yes` suspends the teammate and its
  status word becomes `suspended`.
- Step 3: the suspended member resumes directly, without a dialog, and the status
  returns to `working` or `idle` (FR-040's direct-resume direction).
- Step 4: graceful shutdown is requested through the confirm dialog and the row
  progresses to `stopped`.

---

### TC-024 - Team cleanup resets the panel

**Preconditions:** Active team with members; Teams window open with focus on a teammate.

**Steps:**
1. Ask the agent to clean up the team, or use the team cleanup path.
2. Observe the Teams window, the strip and the Teams button.

**Test data:** `clean up the team`

**Expected results:**
- Step 2: the Teams window closes, the strip disappears, the members and counters are
  cleared, focus is cleared, and the Teams button becomes dimmed with its bare label.

---

### TC-025 - Event-driven sub-agent rows appear and disappear without a refresh

**Preconditions:** Agents window open.

**Steps:**
1. With the window open, ask the agent to spawn a sub-agent.
2. Without closing or reopening the window, watch for the new row.
3. Let the sub-agent finish and keep watching.

**Test data:** `spawn an explore agent to count the Rust files in the repo`

**Expected results:**
- Step 2: the new row appears on the next frame with no manual refresh and no key press.
- Step 3: the row disappears when the sub-agent completes, and a completion entry is
  visible in the log panel. If the provider cut the reply, the log line carries a
  truncation marker.

---

### TC-026 - Dropped event is repaired by reconciliation

**Preconditions:** A live sub-agent whose row is visible in the Agents window.

**Steps:**
1. Wait for roughly two seconds without touching anything.
2. Compare the painted rows with the actual running tasks reported by the agent tools
   (for example the agent list view).
3. If a row is missing or stale, wait for at least two more seconds and re-check.

**Test data:** none

**Expected results:**
- Step 2: within the documented reconciliation interval (about 1500 ms) the painted
  rows converge on the authoritative registry.
- Step 3: a row that a dropped event should have removed is removed; a running registry
  task that was never painted gains a row. No duplicate row is ever shown for the same
  task.

---

### TC-027 - Scrolling and paging a long list

**Preconditions:** Agents window open with more sub-agent and background rows than fit
in the popup (spawn enough sub-agents to overflow the table).

**Steps:**
1. Note the visible rows and confirm a vertical scrollbar is drawn at the table's right
   edge.
2. Scroll the wheel down over the popup body.
3. Note the newly visible rows.
4. Press `PgDn` and confirm the cursor lands on a newly visible row and the viewport
   followed it.
5. Click the scrollbar gutter near the bottom of the track, drag, and release.
6. Press `Down` at the bottom row.

**Test data:** none

**Expected results:**
- Step 1: a scrollbar is present because the content exceeds the viewport (FR-059).
- Step 2-3: the rows shift and later rows become reachable; the offset clamps at the
  maximum and does not scroll into blank space (FR-076).
- Step 4: the cursor pages one viewport and the landed row paints at the viewport edge
  (FR-042, FR-078).
- Step 5: the click jumps the thumb to that position and the drag moves it; release
  ends the drag (FR-076).
- Step 6: the cursor clamps at the last row.

---

### TC-028 - Popup clicks outside chrome are consumed; stale cursors never panic

**Preconditions:** Agents window open with a table that has empty area below the last
row.

**Steps:**
1. Left-click a blank cell in the empty part of the popup table area.
2. Observe the chat surface beneath.
3. Click on a painted `[stop]`/`[x]` control cell.
4. Kill a row (cursor `Del`, confirming `Yes`) and press `Del` again on the stale
   ordinal.
5. Repeat rapidly several times on different rows.
6. Watch the process for a panic message or a crash dump.

**Test data:** none

**Expected results:**
- Step 2: the click is consumed; nothing on the surface beneath reacts (FR-070).
- Step 3: the click is consumed without effect - the control glyph is a status
  indicator, not a button (FR-071, FR-076).
- Step 4-5: the stale ordinal is a consumed no-op: the status line reports it and
  nothing is dispatched; no other task is affected (FR-045, FR-069).
- Step 6: no panic, no crash marker, no crash dump file is produced.

---

### TC-029 - Panel stays serviceable during a running turn

**Preconditions:** A long-running turn that keeps spawning work (e.g. a multi-step task
with background commands).

**Steps:**
1. While the turn is streaming, left-click the Agents button.
2. Left-click the Teams button.
3. Move the cursor onto a row, press `Space` (or `Del`) and answer the dialog `Yes`.
4. Left-click ` Close `.
5. Observe the chat stream throughout.

**Test data:** none

**Expected results:**
- Steps 1-4: every action takes effect immediately on the next frame; the UI never
  freezes and never waits for the turn to finish.
- Step 5: the chat keeps streaming while the window is opened, used and closed.

---

### TC-030 - Panel is fully visible at the smallest supported terminal size

**Preconditions:** Agents window open with several rows; Teams window tested the same
way.

**Steps:**
1. Shrink the terminal to the smallest size you normally use (for example 60 columns by
   18 rows).
2. Open each window in turn.
3. Attempt to click ` Close `; check the legend band and the cursor row are still
   readable.

**Test data:** none

**Expected results:**
- Step 2: both popups stay inside the terminal rectangle, are fully framed, and their
  titles are readable (NFR-010).
- Step 3: the Close button stays clickable; the legend clips instead of wrapping; the
  cursor still navigates and every key still reaches the window.

---

### TC-031 - Agents window closes when its list empties

**Preconditions:** Agents window open with exactly one live sub-agent or background
task.

**Steps:**
1. Let the last task finish, or kill it from the panel (`Del` + `Yes`).
2. Observe the window and the Agents button.

**Test data:** none

**Expected results:**
- Step 2: the window closes on the frame after the last row is removed, and the Agents
  button returns to its dimmed bare `Agents` label.

---

### TC-032 - Cursor and focus survive a window reopen

**Preconditions:** At least one sub-agent row and an active team with a focused
teammate.

**Steps:**
1. Move the cursor onto a sub-agent row, then close the Agents window with ` Close `.
2. Reopen the Agents window and press `Space`.
3. Close the Teams window with the cursor on a member, reopen it, and press `Alt+Down`.
4. Observe which teammate becomes focused.

**Test data:** none

**Expected results:**
- Step 2: the preserved cursor ordinal still names the same task (FR-065), so `Space`
  acts on that row; if the row vanished while the window was closed, `Space` is a
  consumed no-op and the status line reports the stale selection (FR-045).
- Step 4: focus resumes from the previously focused teammate rather than starting again
  from the first member.
---

### TC-033 - Agents table content margin

**Preconditions:** Agents window open in a 160x44 terminal with at least one live
sub-agent or background shell task.

**Steps:**
1. Observe the table body and the control columns.
2. Confirm the cursor band still spans the full table width when placed on a row.

**Test data:** none

**Expected results:**
- Step 1: every painted line (header, primary row, sub-agent rows, background shell
  rows) indents five blank columns behind the table's left edge, while the
  `[stop]`/`[x]` columns keep their documented 70/77 positions in the indented row
  space and the scrollbar stays at the right edge.
- Step 2: the cursor band starts at the table's left edge (not at column 5 of the row
  text) and covers the full width, control columns included.

Automated coverage: `test_agents_window_table` (positions re-derived with
`columns::CONTENT_X`).

---

### TC-034 - Output view close bar and wheel ownership

**Preconditions:** The output view overlay open (cursor `Enter` on an agent row or
teammate row); the Agents window open beneath it for the wheel case.

**Steps:**
1. Observe the overlay's bottom-right corner.
2. Click the ` Close ` button.
3. Reopen the overlay, place the mouse over its centre, and roll the wheel up.
4. With the overlay open, press `Up`/`Down` and then `Esc`.

**Test data:** none

**Expected results:**
- Step 1: a bordered ` Close ` button sits pinned to the overlay's bottom-right corner,
  clamped to at most 10 columns and 3 rows, with one separator row between the content
  and the button.
- Step 2: the overlay closes, the selected agent session and index clear with it, and
  the Agents window beneath stays open exactly as it was.
- Step 3: the wheel scrolls the overlay's content (the popup beneath does not move),
  so the overlay is no longer pinned to its newest page.
- Step 4: while the overlay is open the window cursor keys are not consumed by the
  popup beneath; `Esc` closes the overlay through the same dismissal transition.

Automated coverage: `test_output_view_close_bar` (9 tests).

---

### TC-035 - Block cursor movement, paging and viewport follow

**Preconditions:** Agents and Teams windows each with more rows than the viewport
(spawn enough sub-agents / teammates to overflow).

**Steps:**
1. Press `Down` with no cursor: the cursor appears on the first row.
2. Press `Up`: the cursor appears on the last row.
3. Press `Up`/`Down` repeatedly to reach each end and one past it.
4. Press `PgDn` twice, then `PgUp` twice.
5. Observe the painted cursor band and the scrollbar thumb on every step.
6. Repeat on the Teams window.

**Test data:** none

**Expected results:**
- Step 1-2: start-at-end behaviour on first press (FR-041).
- Step 3: the cursor clamps at both ends without reporting an error.
- Step 4: each page moves one viewport, clamped (FR-042).
- Step 5: the cursor row always paints as a full-width reversed band, and the viewport
  follows when the cursor leaves the visible slice (FR-078); the scrollbar thumb
  tracks the offset.
- Step 6: identical behaviour in the Teams window over the teammate rows.

Automated coverage: `test_agents_teams_cursor` (navigation, paging, clamps,
scroll-into-view on both windows).

---

### TC-036 - Row-action keys and the key legend

**Preconditions:** Agents window open with a live sub-agent; the Teams window likewise.

**Steps:**
1. Read the legend band left of ` Close ` in both windows.
2. On the Agents window: `Space` (suspend), `Space` (resume), `Del` + `Yes` (kill).
3. On the Teams window: `Space` + `Yes` (suspend), `Space` (resume, no dialog),
   `Del` + `Yes` (shut down), `Enter` (focus + overlay).
4. Confirm a repeated `Del` while the dialog is open is absorbed.

**Test data:** none

**Expected results:**
- Step 1: the legend reads `Up/Down select  PgUp/PgDn page  Space stop/resume  Del
  kill  Enter open` (Agents) and `... Del shut down ...` (Teams), dim, left of Close,
  clipped to the band width (FR-077).
- Step 2-3: each key drives the documented action on the cursor row (FR-035, FR-036,
  FR-038, FR-040) and is consumed (FR-055).
- Step 4: the open dialog absorbs further row-action keys.

Automated coverage: `test_agents_teams_cursor` (Space/Del/Enter actions, status
splits, stale no-ops) + `test_row_action_confirm` (dialog answers).

---

### TC-037 - Mouse surface limited to scroll and chrome

**Preconditions:** Agents window open.

**Steps:**
1. Left-click a painted sub-agent row body.
2. Left-click the `[stop]`/`[x]` glyphs of a live row.
3. Left-click the scrollbar gutter, drag, release.
4. Wheel over the popup body.
5. Left-click ` Close `.
6. Confirm no text was selected in the chat beneath from any of the clicks.

**Test data:** none

**Expected results:**
- Step 1-2: every click inside the popup body is consumed without effect - no cursor
  move, no action, no focus change (FR-076, FR-070, FR-071).
- Step 3-4: the gutter drag moves the offset and the wheel scrolls it; nothing else
  reacts.
- Step 5: the window closes.
- Step 6: the chat pane is untouched; no selection or caret change occurred.

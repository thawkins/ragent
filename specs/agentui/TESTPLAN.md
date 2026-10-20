---
status: accepted
---

# Manual Test Plan: Agent and Team Member Panel

**Spec:** [SPEC.md](SPEC.md) · **Plan:** [PLAN.md](PLAN.md)

This is a **manual** test plan. Every case is executed by a human in a real terminal
session. Where a case needs a running sub-agent, a background shell task, a team, or
mouse support, the prerequisite is listed before the steps. This document contains no
automated test code and no reference to any test harness.

The panel is operated **both by keyboard and by mouse**. Every key press is written
explicitly, in order, and the exact text to type into each field is given as
**Test data**. Keys are written as `Space`, `Shift+K`, `Alt+Down`, `Alt+Up`, `Enter`,
`Esc`, and literal characters in `backticks`. A mouse step names the button and the
exact target: `left-click the Agents button beside the message input`.

## Prerequisites

1. **Build the binary.** Build a debug binary and confirm `--version` reports the
   expected version string.
2. **Configure a model.** Cases that need a live turn require a working provider (a
   local Ollama model is fine) and a confirmed short response.
3. **Confirm mouse support.** Run the session in a terminal that reports mouse events.
   TC-013 is the keyboard-only case and does not require mouse reporting.
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
- Step 2: the popup occupies roughly 90% of the width and 70% of the height, centred.
- Step 3-4: the popup stays inside the terminal rectangle; nothing is clipped off
  screen; the table area is at least one row tall.
- Step 5: a bordered ` Close ` button sits in the bottom-right corner of the window's
  inner area and is at most 10 columns wide and 3 rows tall.

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
- Step 3: the primary row shows no action controls.

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
- Step 3: the running row is yellow and paints `[stop]` and `[x]` at the right edge.
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
4. Suspend a running sub-agent (see TC-010), then observe its row.
5. Kill a running sub-agent and observe the row immediately after confirming.

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
  controls.

---

### TC-008 - Clicking a row opens that agent's output view

**Preconditions:** Agents window open with a running sub-agent.

**Steps:**
1. Left-click the primary agent row.
2. Observe the surface that opens and its label.
3. Close that view and reopen the Agents window.
4. Left-click a running sub-agent row.
5. Observe the label on the opened view.

**Test data:** none

**Expected results:**
- Step 2: an output view opens for the primary session and is labelled `primary`.
- Step 4-5: an output view opens for that task's child session, labelled with the agent
  name and the short task id.

---

### TC-009 - Sub-agent suspend, resume and kill from the mouse

**Preconditions:** Agents window open with a running sub-agent.

**Steps:**
1. Left-click the `[stop]` control on the running sub-agent row.
2. Confirm the dialog if one appears, choosing `Yes`.
3. Observe the row and the agent's output view.
4. Left-click the `>` control on the same row.
5. Confirm if asked.
6. Left-click the `[x]` control on the same row and choose `No`.
7. Left-click `[x]` again and choose `Yes`.
8. Observe the window.

**Test data:** none

**Expected results:**
- Step 2-3: a yes/no confirmation naming the task is presented first; on `Yes` the task
  is suspended, the row turns dark grey, gains ` [||]`, and its control becomes `>`.
- Step 4-5: the task resumes, the row returns to yellow with `[stop]`, and its step
  count starts increasing again.
- Step 6: choosing `No` dismisses the dialog and changes nothing.
- Step 7-8: choosing `Yes` terminates the task; the row is marked terminating and then
  removed. The Agents window stays open.

---

### TC-010 - Keyboard suspend, resume and kill on the selected agent row

**Preconditions:** Agents window open; a running sub-agent row has been selected by
left-clicking it once.

**Steps:**
1. Press `Space` with the sub-agent row selected.
2. Confirm if prompted.
3. Press `Space` again.
4. Press `Shift+K`.
5. Confirm with `Yes`.
6. Open a text editor field (the message input) and confirm no stray space or `k` was
   inserted at any point.

**Test data:** none

**Expected results:**
- Step 1-2: the selected task suspends; the row shows the suspended styling.
- Step 3: the task resumes.
- Step 4-5: the task is terminated after confirmation.
- Step 6: the input buffer contains no characters typed by `Space` or `Shift+K` — both
  keys were consumed by the window.

---

### TC-011 - Primary agent is protected from Space and Shift+K

**Preconditions:** Agents window open.

**Steps:**
1. Left-click the primary agent row so it becomes the selected row.
2. Press `Space`.
3. Read the status line.
4. Press `Shift+K`.
5. Read the status line.
6. Confirm the primary turn is still running.

**Test data:** none

**Expected results:**
- Step 3: the status line reads `Cannot suspend primary agent`.
- Step 5: the status line reads `Cannot kill primary agent`.
- Step 6: no suspension or termination occurred; the running turn is untouched and the
  chat continues to stream.

---

### TC-012 - Background shell task controls

**Preconditions:** Agents window open with at least one live background shell task.

**Steps:**
1. Left-click the `[stop]` control on the background task row.
2. Confirm if prompted.
3. Observe the task in the shell.
4. Repeat with the `[x]` control on another background task, first answering `No` and
   then `Yes`.

**Test data:** none

**Expected results:**
- Step 2-3: after confirmation the background task is cancelled through the background
  task service; the process stops.
- Step 4: `No` leaves the task running; `Yes` cancels it. In both cases the panel does
  not block and the chat remains responsive.

---

### TC-013 - Keyboard-only operation (no mouse reporting)

**Preconditions:** A terminal session with mouse reporting unavailable. Agents window
can be opened by mouse beforehand if needed, or is already open.

**Steps:**
1. With a sub-agent row selected, press `Space`.
2. Press `Shift+K` and confirm with `Enter` on `Yes`.
3. Press `Esc` while the confirmation is open.
4. Observe the input buffer.

**Test data:** none

**Expected results:**
- Steps 1-2: every action is reachable without the mouse.
- Step 3: `Esc` dismisses the confirmation without performing the action.
- Step 4: no character from any of the consumed keys appears in the input buffer.

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
  `... (lead + 1 teammate) ` for exactly one.
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
  count, and `-` in claimed and done.
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
2. Left-click a teammate row in the Teams window.
3. Observe the strip.

**Test data:** none

**Expected results:**
- Step 1: a single strip with one pill per teammate, each carrying a status icon and a
  name truncated to 16 columns, and the hint ` Alt+^v:cycle ` at the right edge.
- Step 3: the focused teammate's pill carries a distinct background while the others do
  not.

---

### TC-020 - Clicking a teammate routes input to that teammate

**Preconditions:** Active team with two teammates; Teams window open.

**Steps:**
1. Left-click the first teammate row.
2. Observe the input border title and the status line.
3. Type `please report your progress` into the input and press `Enter`.
4. Observe the lead session and the teammate.
5. Left-click the lead row.
6. Observe the input border title and the status line.
7. Type `hello lead` and press `Enter`.

**Test data:** `please report your progress`, then `hello lead`

**Expected results:**
- Step 2: the input border is titled ` -> <name> (focused) ` in bold yellow; the status
  line reads `team: focused -> <name>`; the teammate's output view is open.
- Step 3-4: the message is delivered to that teammate's mailbox, not the lead session;
  the teammate responds in its own output view.
- Step 5-6: focus clears, the border title returns to normal, and the status line reads
  `focus: lead (you)`.
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

### TC-022 - Space and Shift+K on a focused teammate

**Preconditions:** Teams window open; a teammate is focused.

**Steps:**
1. Press `Space`.
2. Confirm if prompted.
3. Observe the teammate row.
4. Press `Shift+K` on another focused teammate and confirm.
5. Verify no character leak into the input buffer.

**Test data:** none

**Expected results:**
- Step 2-3: a request for graceful shutdown of that teammate is made and the row moves
  through `stopping` to `stopped`.
- Step 4: the second teammate shuts down cleanly.
- Step 5: neither key inserted a character into the input buffer.

---

### TC-023 - Teammate [stop] and [x] controls with confirmation

**Preconditions:** Teams window open with a working teammate.

**Steps:**
1. Left-click the `[stop]` control on the working teammate row.
2. Answer `No`.
3. Left-click `[stop]` again and answer `Yes`.
4. Left-click `[stop]` on the now-suspended row and answer `Yes`.
5. Left-click `[x]` on a teammate and answer `Yes`.

**Test data:** none

**Expected results:**
- Step 1-2: a yes/no confirmation naming the teammate and the suspend action appears;
  `No` dismisses it and nothing changes.
- Step 3: the teammate suspends and its status word becomes `suspended`.
- Step 4: the same control resumes it and the status returns to `working` or `idle`.
- Step 5: graceful shutdown is requested and the row progresses to `stopped`.

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

### TC-027 - Scrolling a long list

**Preconditions:** Agents window open with more sub-agent and background rows than fit
in the popup (spawn enough sub-agents to overflow the table).

**Steps:**
1. Note the visible rows and confirm a vertical scrollbar is drawn at the table's right
   edge.
2. Scroll the wheel down over the popup body.
3. Note the newly visible rows.
4. Left-click the `[stop]` control of a row that only became visible after scrolling.
5. Confirm if prompted.

**Test data:** none

**Expected results:**
- Step 1: a scrollbar is present because the content exceeds the viewport.
- Step 2-3: the rows shift and later rows become reachable; the offset clamps at the
  maximum and does not scroll into blank space.
- Step 4-5: the click resolves to the control painted on that cell — the action targets
  the row the user actually clicked, not a row from the unscrolled list.

---

### TC-028 - Blank popup cells consume clicks; stale rows never panic

**Preconditions:** Agents window open with a table that has empty area below the last
row.

**Steps:**
1. Left-click a blank cell in the empty part of the popup table area.
2. Observe the chat surface beneath.
3. Kill a row (confirming `Yes`) and immediately left-click the space where its `[x]`
   was.
4. Repeat rapidly several times on different rows.
5. Watch the process for a panic message or a crash dump.

**Test data:** none

**Expected results:**
- Step 2: the click is consumed; nothing on the surface beneath reacts (no cursor move,
  no accidental focus, no message).
- Step 3-4: the stale click is ignored without panicking and without mutating unrelated
  state; no other task is affected.
- Step 5: no panic, no crash marker, no crash dump file is produced.

---

### TC-029 - Panel stays serviceable during a running turn

**Preconditions:** A long-running turn that keeps spawning work (e.g. a multi-step task
with background commands).

**Steps:**
1. While the turn is streaming, left-click the Agents button.
2. Left-click the Teams button.
3. Left-click a `[stop]` control, answer `Yes`.
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
3. Attempt to click ` Close ` and one `[stop]` control.

**Test data:** none

**Expected results:**
- Step 2: both popups stay inside the terminal rectangle, are fully framed, and their
  titles are readable.
- Step 3: both controls are reachable and clickable; nothing is drawn outside the
  terminal or wrapped over the border.

---

### TC-031 - Agents window closes when its list empties

**Preconditions:** Agents window open with exactly one live sub-agent or background
task.

**Steps:**
1. Let the last task finish, or kill it from the panel.
2. Observe the window and the Agents button.

**Test data:** none

**Expected results:**
- Step 2: the window closes on the frame after the last row is removed, and the Agents
  button returns to its dimmed bare `Agents` label.

---

### TC-032 - Selection and focus survive a window reopen

**Preconditions:** At least one sub-agent row and an active team with a focused
teammate.

**Steps:**
1. Select a sub-agent row by left-clicking it, then close the Agents window with
   ` Close `.
2. Reopen the Agents window and press `Space`.
3. Close the Teams window with a teammate focused, reopen it, and press `Alt+Down`.
4. Observe which teammate becomes focused.

**Test data:** none

**Expected results:**
- Step 2: the previously selected row is still the selected row, so `Space` acts on that
  task.
- Step 4: focus resumes from the previously focused teammate rather than starting again
  from the first member.

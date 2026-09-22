---
status: draft
audit:
  - { time: 1789920532, from: "none", to: "draft", actor: "system" }
---
# Specification: Message Input Queue - Queue User Messages While the Primary Agent Executes

## Overview

Today the TUI **locks the message-window input field** while the primary agent is
executing: the border turns red, typed characters are dropped, and pressing Enter
reports `busy - wait for the current turn to finish`. The user cannot stage the next
instruction until the current turn ends.

This specification defines an **input queue**: while the primary agent executes, the
input field stays editable and accepts messages. Each submitted message is appended to
a **FIFO (oldest-first) queue**. When the running turn ends, the oldest queued entry is
popped and executed as the next user turn; the next entry follows when that turn ends,
and so on until the queue is empty.

A **two-digit, zero-padded queue counter** is rendered immediately before the `>`
prompt in the input field (for example `03> refactor the parser`) so the user can see
at a glance how many messages are waiting. The counter decrements as each entry is
popped and executed. Entries are added to the **input history** at the moment they are
entered, not when they are executed.

## Scope

**In scope**

- The TUI message-window input field (`render_input`), its key handling in
  `crates/ragent-tui/src/input.rs`, and its submit path (`InputAction::SendMessage`).
- App state in `crates/ragent-tui/src/app/` (`App`), the `VecDeque` queue, the counter
  render, and the turn-boundary drain.
- Adding queued entries to the existing `input_history`.

**Out of scope**

- The HTTP server, the headless `ragent run` path, and the CLI one-shot mode. Those
  remain synchronous request/response and are unchanged.
- Sub-agent / teammate mailboxes. Teammate-routed messages keep their existing
  behaviour.
- Changing the agent loop's concurrency model. Only one primary turn runs at a time;
  the queue serialises turns, it does not parallelise them.

## Definitions

- **Primary agent** - the lead session whose turn drives the main chat conversation.
- **Executing / processing** - the window during which `App::is_processing` is `true`
  (set on `Event::MessageStart`, cleared on `Event::MessageEnd` or `Event::AgentError`).
- **Input queue** - an in-memory FIFO deque of messages submitted while the primary
  agent is executing.
- **Queued entry** - one item in the input queue: the message text plus any image
  attachments staged at submission time.
- **Queue counter** - the two-digit, zero-padded count of entries currently in the
  queue, rendered immediately before the `>` prompt.
- **Turn boundary** - the point at which the running turn finishes or fails, i.e. the
  handler for `Event::MessageEnd` or `Event::AgentError`.
- **Dispatch** - submitting a queued entry to the agent loop exactly as if the user had
  pressed Enter on it (the existing `App::dispatch_user_message` path).

## Assumptions

These decisions are made explicitly; each is testable and can be revised.

1. The queue is **bounded** to a default maximum of 32 entries; submissions beyond the
   cap are rejected with a status message rather than silently dropped.
2. The counter is shown **only when the queue is non-empty**. At zero entries the input
   field renders the existing bare `> ` prompt with no counter, keeping the common case
   visually unchanged.
3. The counter is **zero-padded to two digits** (`00`-`99`); values above 99 render as
   `99` because the queue is capped well below that.
4. Only **regular chat messages** (plain text and staged image attachments) are queued.
   Slash commands (`/...`), bang commands (`!...`), and teammate-targeted messages keep
   their current busy behaviour and are **not** queued.
5. After a turn ends for **any** reason - normal completion, agent error, or user cancel
   - the queue advances to the next entry. Cancelling stops the current turn, not the
   queue; the queue is only emptied by draining it or by an explicit clear action.
6. Queued entries are **not** echoed into the message window when queued; they appear in
   the conversation when they are dispatched.

## Requirements

### Ubiquitous requirements

FR-001: The TUI **shall** maintain an in-memory input queue that stores user messages
submitted while the primary agent is executing, and **shall** preserve their submission
order (oldest first).

FR-002: The TUI **shall** keep the message-window input field editable and submittable
while the primary agent is executing, rather than locking it.

FR-003: The TUI **shall** add every queued entry to the input history at the moment it
is entered, before it is executed.

FR-004: The TUI **shall** bound the input queue to a maximum of 32 entries and **shall**
reject submissions beyond that limit with a status message.

### Event-driven requirements

FR-005: When the user presses Enter with non-empty input while the primary agent is
executing, the TUI **shall** append the message (with any staged image attachments) to
the tail of the input queue, clear the input field, and add the entry to the input
history.

FR-006: When the primary agent finishes a turn with a non-cancelled finish reason, the
TUI **shall** pop the oldest entry from the head of the input queue and dispatch it as
the next user turn.

FR-007: When the primary agent reports an error for the current turn, the TUI **shall**
pop the oldest entry from the head of the input queue and dispatch it as the next user
turn.

FR-008: When an entry is popped from the input queue, the TUI **shall** decrement the
queue counter and update the rendered prompt before the next frame is painted.

### State-driven requirements

FR-009: While the input queue contains one or more entries, the TUI **shall** render a
two-digit, zero-padded queue counter immediately before the `>` prompt in the input
field (for example `03> refactor the parser`).

FR-010: While the input queue is empty, the TUI **shall** render the input prompt
without a queue counter.

FR-011: While the primary agent is executing and the input field is accepting messages,
the TUI **shall** render the input border in its unlocked style rather than the locked
red style.

FR-012: While the primary agent is executing with queued entries pending, the TUI
**shall** keep the queue counter and the input field visible and responsive to typing,
cursor movement, and submission.

### Optional requirements

FR-013: The TUI **may** expose a `/queue` slash command that lists the queued entries,
clears the queue, or immediately runs the next queued entry.

FR-014: The TUI **may** echo each queued entry into the log panel with an
`queued (N in queue)` notice when it is enqueued.

FR-015: The TUI **may** allow the queue capacity to be configured; when unset, the
default of 32 applies.

### Unwanted requirements

FR-016: The TUI **shall not** dispatch a queued entry while the primary agent is
executing (`is_processing` is `true`) or while a compaction run is in progress.

FR-017: The TUI **shall** queue slash commands (`/…`) submitted while the primary
agent is executing, exactly as it queues a plain message, and **shall not** queue bang
commands (`!…`) or teammate-targeted messages — those two paths **shall** retain their
existing busy behaviour and are refused while a turn is running. A queued slash command
**shall** be dispatched at the next turn boundary (or by an explicit queue-control
`Next` / `/queue next`), routed through the slash-command executor rather than the chat
path.

FR-018: The TUI **shall not** discard queued entries when the running turn is cancelled,
errors, or triggers compaction; entries **shall** be retained until they are dispatched
or explicitly cleared.

FR-019: The TUI **shall not** reorder the queue; an entry **shall** never be dispatched
ahead of an older entry that is still queued. The only exception is an explicit,
user-initiated reorder from the `Show` panel (`Enter` moves the highlighted entry one
step toward the front, FR-040); no automatic path **shall** reorder the queue.

FR-020: The TUI **shall not** insert the queue counter into the editable input text, and
the counter **shall not** be copied, cut, or selected as message content.

### Queue-control menu (ALT-Q)

FR-021: The TUI **shall** bind the keystroke ALT-Q to open a queue-control menu that
presents exactly four options: `Next`, `Stop`, `Clear`, and `Show`.

FR-022: When the user presses ALT-Q, the TUI **shall** open the queue-control menu
overlay without altering the current input buffer, the staged attachments, or the
running turn.

FR-023: While the input queue contains one or more entries, the queue-control menu
**shall** render the `Next` option as visible and selectable; while the input queue is
empty, the `Next` option **shall** be hidden or rendered non-selectable.

FR-024: When the user selects the `Next` option, the TUI **shall** stop the currently
executing turn and immediately dispatch the oldest queued entry as the next user turn,
then close the menu.

FR-025: When the user selects the `Stop` option, the TUI **shall** halt the primary
agent exactly as `InputAction::CancelAgent` does and **shall not** advance to the next
queued entry.

FR-026: While the primary agent is executing, the queue-control menu **shall** label the
halt option `Stop`; while the primary agent is already stopped (no turn executing), the
menu **shall** label the same option `Resume`.

FR-027: When the primary agent is already stopped and the user selects the `Resume`
option, the TUI **shall** resume the interrupted work rather than halting the agent.

FR-028: When the user selects the `Clear` option, the TUI **shall** remove all entries
from the input queue and update the queue counter to reflect the now-empty queue.

FR-029: The TUI **shall not** dispatch a queued entry as a result of selecting `Stop`;
halting the agent **shall not** advance the queue.

FR-030: The TUI **shall not** dispatch a queued entry from the `Next` option while a
compaction run is in progress or while `pending_send_after_compact.is_some()`; the action
**shall** be deferred to the next safe turn boundary or refused with a status message.

FR-031: The queue-control menu **shall not** insert any characters into, or otherwise
mutate, the editable input buffer.

FR-032: The TUI **may** allow the queue-control menu to be dismissed with Esc without
taking any action.

### Clear-confirmation dialog (ALT-Q `Clear` option)

FR-033: When the user selects the `Clear` option from the queue-control menu, the TUI
**shall** present a confirmation dialog with the message `Clear the input queue?` and
exactly two options, `Yes` and `No`.

FR-034: While the clear-confirmation dialog is displayed, the TUI **shall** select the
`No` option by default, so that pressing Enter without changing the selection does not
clear the queue.

FR-035: When the user selects the `No` option or dismisses the clear-confirmation dialog
(for example with Esc), the TUI **shall** close the dialog and **shall** leave the input
queue unchanged.

FR-036: When the user selects the `Yes` option, the TUI **shall** remove all entries from
the input queue and update the queue counter to reflect the now-empty queue, then close
the dialog.

FR-037: The TUI **shall not** remove any entry from the input queue as a result of
selecting the `Clear` option until the user has confirmed with `Yes` in the
clear-confirmation dialog.

### Menu navigation and the `Show` queue-entry panel

FR-038: The TUI **shall** allow the queue-control menu rows to be navigated with the Up
and Down keys, skipping any non-selectable row (the empty-queue `Next` row, FR-023), and
**shall** activate the highlighted row with Enter.

FR-039: When the user selects the `Show` option, the TUI **shall** open a panel listing
every entry currently in the input queue, oldest first, with one entry highlighted by a
block cursor; Up and Down **shall** move the highlight, scrolling the panel to keep the
highlighted entry visible.

FR-040: When the panel is displayed and the user presses Enter, the TUI **shall** move
the highlighted entry one position toward the front of the queue (so it runs sooner) and
**shall** keep the highlight on that entry; the order of every other entry **shall** be
preserved.

FR-041: When the panel is displayed and the user presses Del, the TUI **shall** remove
the highlighted entry from the input queue and **shall** keep the highlight within range.

FR-042: While the panel is displayed, only Esc **shall** dismiss it; Enter, Del, and the
navigation keys **shall** keep it open. Any change made from the panel (Enter or Del)
**shall** set the TUI redraw flag so the panel and the queue counter repaint on the next
frame.

FR-043: The queue-entry panel **shall not** insert any characters into, or otherwise
mutate, the editable input buffer or the staged attachments.

## Non-Functional Requirements

NFR-001: Enqueue and dequeue operations **shall** be O(1); the queue **shall** be
implemented with a double-ended queue rather than a shifting vector.

NFR-002: The queue counter **shall** occupy a fixed two-column prefix so the cursor
column and wrapped-row geometry remain stable and correct while the counter is shown.

NFR-003: Enqueuing a message and updating the counter **shall** set the TUI redraw flag
so the change is visible on the next frame without a forced repaint loop.

NFR-004: The feature **shall not** introduce new `unsafe` code or block the UI thread;
turn-boundary dispatch **shall** reuse the existing asynchronous dispatch path.

NFR-005: The queue **shall** be cleared when the TUI session is reset (new session,
session switch, or process exit); it **shall not** be persisted to disk.

NFR-006: Opening, navigating, and dismissing the queue-control menu **shall not** block
the UI thread and **shall not** introduce new `unsafe` code.

NFR-007: The queue-control menu **shall** reuse the existing overlay/modal rendering and
key-dispatch machinery rather than adding a parallel input path.

NFR-008: The `Stop`/`Resume` and `Clear` actions **shall** take effect on the next
painted frame, setting the TUI redraw flag so the label change and the cleared counter
are visible without a forced repaint loop.

NFR-009: The ALT-Q keybinding **shall not** shadow or consume existing input handling;
when the queue-control menu is not open, ALT-Q **shall** be the only new keystroke
consumed, and all other keys **shall** retain their current behaviour.

NFR-010: The clear-confirmation dialog **shall** reuse the existing overlay/modal
rendering and key-dispatch machinery rather than adding a parallel input path, and
opening, navigating, and dismissing it **shall not** block the UI thread or introduce new
`unsafe` code.

NFR-011: Presenting, selecting, or dismissing the clear-confirmation dialog **shall** set
the TUI redraw flag so the dialog and the resulting cleared counter are visible on the
next painted frame without a forced repaint loop.

NFR-012: The queue-entry panel **shall** reuse the existing overlay/modal rendering and
key-dispatch machinery rather than adding a parallel input path, and scrolling,
reordering, and deleting from it **shall not** block the UI thread or introduce new
`unsafe` code.

## Acceptance Criteria

1. With the agent executing, typing and pressing Enter appends the message to the queue
   instead of being rejected.
2. The counter renders as two zero-padded digits immediately before `>` and shows the
   live queue length.
3. Each entry is present in the input history immediately after it is entered (before
   the previous turn finishes).
4. When a turn ends, the oldest queued entry is dispatched and the counter decrements.
5. With an empty queue the prompt shows no counter and the input border is not red.
6. An entry popped from the queue is dispatched in FIFO order; ordering never changes
   except through an explicit `Enter` reorder in the `Show` panel.
7. Queue contents survive a cancelled turn or an agent error and continue draining at
   the following turn boundary.
8. Slash and bang commands submitted while busy are not queued.
9. `Alt+Q` opens a four-row menu (`Next`, `Stop`/`Resume`, `Clear`, `Show`); `Up`/`Down`
   move the highlight and `Enter` activates the highlighted row.
10. The `Show` row opens a panel listing every queued entry oldest-first with a block
    cursor; `Enter` moves the highlighted entry one step toward the front, `Del` removes
    it, and only `Esc` dismisses the panel. The input draft and staged attachments are
    never altered by the panel.

## References

- `crates/ragent-tui/src/app/session_ops.rs` - `is_input_blocked`, `dispatch_user_message`.
- `crates/ragent-tui/src/app/input_handler.rs` - `InputAction::SendMessage` handling.
- `crates/ragent-tui/src/input.rs` - `handle_key`, Enter/char busy guards.
- `crates/ragent-tui/src/layout.rs` - `render_input`, `input_lines_with_kb_selection`,
  `input_widget_height`, `input_cursor_display_pos`, `refresh_input_render_cache`.
- `crates/ragent-tui/src/app/event_handler.rs` - `MessageStart` / `MessageEnd` /
  `AgentError` handlers.
- `crates/ragent-tui/src/lib.rs` - housekeeping poll loop and `HOUSEKEEPING_INTERVAL`.
- `crates/ragent-tui/src/app/models.rs` - `add_to_history`.
- `crates/ragent-tui/src/app/state.rs` - `is_processing`, `input_history`,
  `pending_send_after_compact`, `InputRenderCache`.

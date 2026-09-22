# Implementation Plan: Message Input Queue

**Spec:** [SPEC.md](SPEC.md) - **Test plan:** [TESTPLAN.md](TESTPLAN.md)

## Approach

Add a bounded FIFO input queue to the TUI `App`, keep the input field editable while
the primary agent executes, render a two-digit queue counter before the `>` prompt, and
drain the queue at each turn boundary. Layer a keyboard-driven control menu (ALT-Q) over
the queue so the user can advance, halt, or clear without leaving the chat screen, and
guard the destructive `Clear` action behind a default-`No` confirmation dialog. All
work is confined to `crates/ragent-tui`; no other crate changes are required.

Six integration points:

1. **State** (`app/state.rs`) - a `VecDeque<QueuedInput>` field plus a `MAX_INPUT_QUEUE`
   constant and an `input_queue_len()` helper. Cleared on new session and session switch.
   A `queue_menu_open` flag (or an overlay enum variant) tracks the ALT-Q menu, and a
   `queue_clear_confirm_open` flag tracks the clear-confirmation dialog.
2. **Submit path** (`input.rs` + `app/input_handler.rs`) - while executing, Enter appends
   to the queue (text + staged attachments), adds to history, clears the input, and sets
   the redraw flag instead of returning the busy status. The char/Enter `is_input_blocked`
   guards are relaxed for plain messages; slash commands are queued the same way
   (FR-017 amendment), while bang commands keep the busy guard.
3. **Render** (`layout.rs`) - `render_input` and `input_lines_with_kb_selection` emit a
   two-column, zero-padded counter prefix when the queue is non-empty; the input border
   uses the unlocked style. Cursor and wrap geometry stay aligned to the fixed prefix.
4. **Drain** (`app/event_handler.rs`) - the `MessageEnd` (non-cancelled) and `AgentError`
   handlers pop the head entry and dispatch it, guarded by `is_processing` and compaction
   state so a queued entry never overlaps a running turn.
5. **Queue-control menu** (`input.rs` + `app/input_handler.rs` + `layout.rs`) - ALT-Q opens
   a four-option overlay (`Next`, `Stop`/`Resume`, `Clear`, `Show`) that reuses the existing
   modal rendering and key-dispatch machinery; `Up`/`Down`/`Enter` navigate and activate the
   rows, `Next` stops the turn and advances the queue, `Stop` halts without advancing,
   `Clear` opens the confirmation dialog, and `Show` opens a scrollable queue-entry panel
   where `Enter` moves an entry toward the front and `Del` removes it.
6. **Clear-confirmation dialog** (`app/input_handler.rs` + `layout.rs`) - selecting `Clear`
   opens a `Clear the input queue?` dialog with exactly `Yes` and `No`, defaulting to `No`;
   only `Yes` empties the queue, while `No` or `Esc` closes the dialog and changes nothing.

## Summary

| Item | Value |
|------|-------|
| Spec ID | `inputqueue` |
| Crate | `crates/ragent-tui` |
| New state | `App::input_queue: VecDeque<QueuedInput>`, `App::queue_menu_open: bool`, `App::queue_clear_confirm_open: bool` |
| Default capacity | 32 entries (`MAX_INPUT_QUEUE`) |
| Counter format | two-digit zero-padded, e.g. `03` |
| Dispatch strategy | reuse `App::dispatch_user_message` at turn boundaries |
| Menu binding | `ALT-Q` opens the queue-control menu |
| Menu options | `Next`, `Stop`/`Resume`, `Clear`, `Show` |
| Clear confirmation | `Clear the input queue?` with `Yes` / `No`, default `No` |
| `Show` panel | scrollable entry list; `Enter` moves toward the front, `Del` removes, `Esc` dismisses |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `QueuedInput` type, `VecDeque` field, capacity constant, and clear hooks | FR-001, FR-004, NFR-001, NFR-005 | S | High | completed | — |
| T-002 | Relax input guards so plain messages are accepted while executing | FR-002, FR-011, FR-012, FR-017 | M | Critical | completed | T-001 |
| T-003 | Enqueue on Enter while executing, add to history, cap at 32 | FR-003, FR-005, FR-018, FR-019 | M | Critical | completed | T-001, T-002 |
| T-004 | Render the two-digit queue counter before the `>` prompt | FR-008, FR-009, FR-010, NFR-002, NFR-003 | M | High | completed | T-001 |
| T-005 | Keep cursor and wrapped-row geometry aligned to the counter prefix | NFR-002, FR-020 | M | High | completed | T-004 |
| T-006 | Drain the queue at the `MessageEnd` turn boundary | FR-006, FR-016, FR-019, NFR-004 | M | Critical | completed | T-003 |
| T-007 | Drain the queue at the `AgentError` turn boundary | FR-007, FR-018 | S | High | completed | T-006 |
| T-008 | Guard dispatch against processing and compaction overlap | FR-016 | S | High | completed | T-006, T-007 |
| T-009 | Add optional `/queue` list/clear/run-next slash command | FR-013 | M | Low | completed | T-003, T-006 |
| T-010 | Log-panel `queued (N in queue)` notice on enqueue | FR-014 | S | Low | completed | T-003 |
| T-011 | Configurable queue capacity with a 32 default | FR-015 | S | Low | completed | T-001 |
| T-012 | Update `TUI-QUICKSTART.md` and `docs/` with the queue behaviour | FR-002, FR-009 | S | Medium | completed | T-004, T-006 |
| T-013 | Bind ALT-Q to open the queue-control menu overlay | FR-021, FR-022, NFR-009 | M | High | completed | T-001, T-004 |
| T-014 | Render the three-option menu with `Next` visible only when the queue is non-empty | FR-021, FR-023, NFR-007 | M | High | completed | T-013 |
| T-015 | Implement the `Next` action: stop the turn and dispatch the oldest entry | FR-024, FR-029, FR-030, NFR-006 | M | Critical | completed | T-006, T-013 |
| T-016 | Implement the `Stop`/`Resume` action mirroring `CancelAgent` | FR-025, FR-026, FR-027, FR-029, NFR-008 | M | High | completed | T-013 |
| T-017 | Implement the `Clear` action to open the confirmation dialog | FR-028, FR-033, NFR-008 | S | High | completed | T-001, T-013 |
| T-018 | Support Esc-to-dismiss without mutating the input buffer | FR-031, FR-032 | S | Medium | completed | T-013 |
| T-019 | Add automated tests for the ALT-Q menu and its three actions | FR-021, FR-023, FR-024, FR-025, FR-026, FR-027, FR-028, FR-029, FR-031 | M | High | completed | T-014, T-015, T-016, T-017, T-018 |
| T-020 | Update `TUI-QUICKSTART.md` and `docs/` with the ALT-Q menu behaviour | FR-021, FR-023, FR-026 | S | Medium | completed | T-013 |
| T-021 | Add a `Clear the input queue?` Yes/No confirmation dialog with `No` as the default | FR-033, FR-034, NFR-010, NFR-011 | M | High | completed | T-017 |
| T-022 | Gate queue clearing on the dialog result: `Yes` clears, `No`/Esc leaves it unchanged | FR-035, FR-036, FR-037 | M | High | completed | T-021 |
| T-023 | Wire Up/Down/Enter navigation for the queue-control menu rows | FR-038 | S | High | completed | T-014 |
| T-024 | Add a `Show` row that opens a scrollable queue-entry panel | FR-021, FR-039 | M | High | completed | T-023 |
| T-025 | Implement `Enter` (move toward front) and `Del` (remove) in the `Show` panel | FR-040, FR-041, FR-042, FR-019 | M | High | completed | T-024 |
| T-026 | Add automated tests for menu navigation and the `Show` panel | FR-023, FR-038, FR-039, FR-040, FR-041, FR-042, FR-043 | M | High | completed | T-024, T-025 |
| T-027 | Update `TUI-QUICKSTART.md` and `docs/` with the `Show` panel behaviour | FR-039, FR-040, FR-041 | S | Medium | completed | T-024 |
| T-028 | Queue slash commands while the agent is busy (FR-017 amendment) | FR-017 | M | High | completed | T-003, T-008 |

## Task Details

### T-001 - Add `QueuedInput` type, `VecDeque` field, capacity constant, and clear hooks

- Add `pub struct QueuedInput { pub text: String, pub image_paths: Vec<std::path::PathBuf> }`
  in `app/state.rs`.
- Add `pub input_queue: std::collections::VecDeque<QueuedInput>` to `App`, initialised
  empty in `app/init.rs`.
- Add `pub const MAX_INPUT_QUEUE: usize = 32;` and `pub fn input_queue_len(&self) -> usize`.
- Clear `input_queue` in the same paths that reset a session (`load_session`,
  new-session creation, and TUI teardown).

### T-002 - Relax input guards so plain messages are accepted while executing

- In `crates/ragent-tui/src/input.rs`, the Enter arm currently returns the busy status
  when `is_input_blocked()`. Change it so Enter with non-empty input proceeds to
  `InputAction::SendMessage` regardless of the blocked state, except when the text starts
  with `/` or `!` (those keep the busy guard).
- Relax the generic `KeyCode::Char(c)` guard so characters can still be typed into the
  buffer while executing; keep `is_input_blocked` for dialogs that must not receive text.
- Update `render_input` border colour selection so the border is only red when the input
  is genuinely unusable (for example, a modal is open), not merely while executing.

### T-003 - Enqueue on Enter while executing, add to history, cap at 32

- In `app/input_handler.rs`, extend the `InputAction::SendMessage` arm: accept the
  message while executing, call `add_to_history(text.clone())`, clear the input and
  cursor, then push a `QueuedInput` (with the staged attachments taken from
  `pending_attachments`).
- Reject when `input_queue_len() >= MAX_INPUT_QUEUE` with a status message and leave the
  input text in place so the user does not lose it.
- Preserve FIFO order (append to the back only).

### T-004 - Render the two-digit queue counter before the `>` prompt

- In `render_input`, build the prefix as `format!("{:02}> ", len.min(99))` when the queue
  is non-empty, else `"> "`.
- Also apply the prefix in the empty-input placeholder path so the counter is visible on
  an empty buffer.
- Set `app.needs_redraw = true` on enqueue/dequeue (NFR-003).

### T-005 - Keep cursor and wrapped-row geometry aligned to the counter prefix

- `input_lines_with_kb_selection` hardcodes the two-character prefix `"> "`/`"  "`.
  Parameterise the prefix to three fixed columns (`"NN> "`/`"   "`) and mirror the change
  in `input_cursor_display_pos` and `input_widget_height` so the cursor column and wrapped
  rows stay correct.
- Ensure prefix characters are non-selectable (carry `None` in the flat char list) so the
  counter is never part of a copy/cut/selection (FR-020).

### T-006 - Drain the queue at the `MessageEnd` turn boundary

- In `app/event_handler.rs`, after the existing `MessageEnd` handling clears
  `is_processing`, call a new `advance_input_queue()` that pops the head entry and calls
  `dispatch_user_message(text, image_paths)`.
- Only advance when the finish reason is not `Cancelled`.

### T-007 - Drain the queue at the `AgentError` turn boundary

- In the `AgentError` handler, after `is_processing = false`, call `advance_input_queue()`
  so an errored turn does not strand the queue (FR-018).

### T-008 - Guard dispatch against processing and compaction overlap

- `advance_input_queue` must no-op when `is_processing` is `true`,
  `compact_in_progress` is `true`, or `pending_send_after_compact.is_some()`, deferring to
  the next safe boundary (FR-016).

### T-009 - Add optional `/queue` list/clear/run-next slash command

- Register a `/queue` command in `app/state.rs` `SLASH_COMMANDS` and dispatch in
  `app/slash.rs`: `list`, `clear`, `next`.

### T-010 - Log-panel `queued (N in queue)` notice on enqueue

- Call `push_log_no_agent(LogLevel::Info, ...)` with the queue length when an entry is
  enqueued.

### T-011 - Configurable queue capacity with a 32 default

- Read an optional `input_queue_capacity` from config; clamp to `1..=99` and fall back to
  `MAX_INPUT_QUEUE`.

### T-012 - Update `TUI-QUICKSTART.md` and `docs/` with the queue behaviour

- Document that the input field accepts messages while the agent runs, describe the
  counter, and note that queued entries go to history on entry.

### T-013 - Bind ALT-Q to open the queue-control menu overlay

- In `crates/ragent-tui/src/input.rs`, detect the ALT-Q chord (`KeyCode::Char('q')` with
  `KeyModifiers::ALT`) and return a new `InputAction::OpenQueueMenu`.
- Add `pub queue_menu_open: bool` (or an overlay enum variant) to `App` in `app/state.rs`,
  initialised `false`.
- Handle `InputAction::OpenQueueMenu` in `app/input_handler.rs`: set the menu-open flag
  and the redraw flag without touching the input buffer or staged attachments (FR-022).
- Confirm ALT-Q is not already bound; no other key behaviour changes (NFR-009).

### T-014 - Render the three-option menu with `Next` visible only when the queue is non-empty

- Render a small centred overlay via the existing modal/overlay drawing path with three
  rows: `Next`, `Stop`/`Resume`, `Clear`.
- When `input_queue_len() == 0`, render `Next` as hidden or dimmed and non-selectable
  (FR-023); keep `Stop`/`Resume` and `Clear` available.
- Navigation (`Up`/`Down`), `Enter` to select, and `Esc` to dismiss reuse the existing
  menu key handling (NFR-007).

### T-015 - Implement the `Next` action: stop the turn and dispatch the oldest entry

- On selecting `Next`: issue the existing cancel path for the running turn, then call
  `advance_input_queue()` to pop and dispatch the head entry (FR-024).
- Guard against compaction overlap: if `compact_in_progress` or
  `pending_send_after_compact.is_some()`, defer to the next safe boundary or report a
  status message (FR-030).
- Never dispatch while `is_processing` is still `true`; wait for the cancel to settle,
  then dispatch at the turn boundary rather than halting the queue (FR-029).
- Keep the action off the blocking UI thread (NFR-006); close the menu after the action.

### T-016 - Implement the `Stop`/`Resume` action mirroring `CancelAgent`

- On selecting `Stop` while executing: perform exactly the `InputAction::CancelAgent`
  behaviour and do **not** advance the queue (FR-025, FR-029).
- When no turn is executing, label the option `Resume` (FR-026); selecting it resumes the
  interrupted work instead of halting the agent (FR-027).
- Set the redraw flag so the label change paints on the next frame (NFR-008).
- Close the menu after the action.

### T-017 - Implement the `Clear` action to open the confirmation dialog

- On selecting `Clear`: open the `Clear the input queue?` confirmation dialog instead of
  emptying the queue immediately (FR-033).
- Set the redraw flag so the dialog paints on the next frame (NFR-008).
- Do not touch the current turn, the input buffer, or the queue in this step; the queue is
  emptied only by T-022 after the user confirms with `Yes` (FR-028, FR-037).

### T-018 - Support Esc-to-dismiss without mutating the input buffer

- `Esc` closes the queue-control menu taking no action; the input buffer, attachments,
  and queue are unchanged (FR-031, FR-032).
- Ensure `Esc` still performs its normal behaviour when the menu is not open.

### T-019 - Add automated tests for the ALT-Q menu and its three actions

- Tests live in `crates/ragent-tui/tests/`. Cover: ALT-Q opens the menu; `Next` hidden at
  empty queue and visible otherwise; `Next` stops and dispatches the oldest entry in FIFO
  order; `Stop` halts without advancing; `Resume` label when stopped and resumes; `Clear`
  opens the confirmation dialog; `Esc` dismisses without mutating input.

### T-020 - Update `TUI-QUICKSTART.md` and `docs/` with the ALT-Q menu behaviour

- Document the ALT-Q binding, the three options, the `Stop`/`Resume` label switch, the
  non-advancing semantics of `Stop`, and the `Clear` confirmation dialog.

### T-021 - Add a `Clear the input queue?` Yes/No confirmation dialog with `No` as the default

- Intercept the `Clear` selection in `app/input_handler.rs` (the arm added by T-017) so it
  opens a confirmation dialog instead of clearing immediately (FR-033).
- Add a `pub queue_clear_confirm_open: bool` flag (or an overlay enum variant) to `App` in
  `app/state.rs`, initialised `false`, and a default selection index initialised to the
  `No` option (FR-034).
- Render the dialog via the existing modal/overlay drawing path in `layout.rs` with the
  message `Clear the input queue?` and exactly two options, `Yes` and `No`, with `No`
  highlighted as the default (NFR-010).
- Set the redraw flag when the dialog opens, when the selection changes, and when it
  closes so the dialog paints on the next frame (NFR-011).
- Reuse the existing `Up`/`Down`/`Left`/`Right` navigation, `Enter` to select, and `Esc`
  to dismiss machinery; do not add a parallel input path (NFR-010).

### T-022 - Gate queue clearing on the dialog result: `Yes` clears, `No`/Esc leaves it unchanged

- On selecting `Yes`: clear the input queue, reset the counter render to the bare `> `
  prompt, close the dialog, and set the redraw flag (FR-036).
- On selecting `No` or dismissing with `Esc`: close the dialog and leave the input queue,
  the input buffer, and the staged attachments unchanged (FR-035).
- Ensure no entry is removed before the user confirms with `Yes`; the `Clear` selection
  alone must not mutate the queue (FR-037).
- Do not touch the currently executing turn while the dialog is open or after it closes.

### T-023 - Wire Up/Down/Enter navigation for the queue-control menu rows

- Intercept `Up`/`Down` in the queue-menu branch of `input::handle_key` to move
  `queue_menu_selected`, skipping any non-selectable row (the empty-queue `Next` row,
  FR-023), and `Enter` to activate the highlighted row (FR-038).
- Route each row to its existing action (`queue_menu_select_next`,
  `queue_menu_select_halt`, `queue_menu_select_clear`) and to the new `Show` panel.
- Set the redraw flag on every move so the highlight repaints (NFR-008).

### T-024 - Add a `Show` row that opens a scrollable queue-entry panel

- Add `Show` as the fourth queue-control menu row (FR-021) and a `queue_show_open` flag
  plus a `queue_show_selected` index in `app/state.rs`, initialised in `app/init.rs`.
- Render the panel via the existing modal/overlay drawing path (`Clear` + a bordered
  `List`) in `layout.rs`, listing every entry oldest-first with a block-cursor highlight;
  Up/Down scroll it so the highlighted entry stays visible (FR-039, NFR-012).
- `Esc` is the only key that dismisses the panel (FR-042); every other key is swallowed
  without mutating the input buffer or the attachments (FR-043).

### T-025 - Implement `Enter` (move toward front) and `Del` (remove) in the `Show` panel

- `Enter` swaps the highlighted entry with the one before it so it runs sooner and keeps
  the highlight on it; a no-op at the front entry (FR-040). The order of every other
  entry is preserved (FR-019).
- `Del` removes the highlighted entry and clamps the highlight into range; an emptied
  queue closes the panel (FR-041).
- Set the redraw flag on every change so the panel and the queue counter repaint
  (FR-042).

### T-026 - Add automated tests for menu navigation and the `Show` panel

- Tests live in `crates/ragent-tui/tests/`. Cover: Up/Down move and clamp the highlight;
  navigation skips the non-selectable empty-queue `Next` row; Enter activates each row
  and is a no-op on the dead `Next` row; the `Show` panel lists entries oldest-first with
  a block cursor; Enter walks an entry toward the front and preserves the others; Del
  removes the highlighted entry and clamps; only Esc dismisses the panel; the panel never
  mutates the draft, the attachments, or the running turn.

### T-027 - Update `TUI-QUICKSTART.md` and `docs/` with the `Show` panel behaviour

- Document the `Show` row, the panel content, the block cursor, the Up/Down scrolling,
  the `Enter` reorder-toward-front semantics, the `Del` removal, and the Esc-only
  dismissal.

### T-028 - Queue slash commands while the agent is busy (FR-017 amendment)

- Relax the slash-command busy guard: the slash-menu `Enter` arm and the plain `Enter`
  arm in `input.rs` emit `InputAction::SlashCommand` even while `is_input_blocked()`
  (bang commands and teammate-targeted messages keep the refusal).
- In `app/input_handler.rs`, the `SlashCommand` arm enqueues via `enqueue_input` when
  the turn is busy (restoring the text on a capacity rejection) and dispatches directly
  otherwise. `enqueue_input` also dismisses the slash completion menu.
- In `app/session_ops.rs`, `advance_input_queue` routes a `/`-prefixed entry through
  `execute_slash_command` (new `dispatch_queued_input` helper) and re-enters the drain
  when a synchronous slash command leaves the boundary free, so consecutive queued
  commands run back-to-back without stalling the tail.
- Tests in `crates/ragent-tui/tests/test_input_queue_slash.rs` and the amended
  `test_busy_send_guard.rs`.

## Risks

- **Cursor geometry drift.** The counter changes the fixed prefix width; if
  `input_lines_with_kb_selection`, `input_cursor_display_pos`, and `input_widget_height`
  are not updated together the cursor will be misplaced. T-005 isolates this.
- **Dispatch re-entrancy.** Draining inside the event handler must not overlap a running
  turn or a compaction; T-008 centralises that guard. The `Next` menu action (T-015) must
  reuse the same guard rather than dispatching directly.
- **Stop/Resume label state.** The label depends on whether a turn is currently
  executing; reading `is_processing` at render time keeps it correct without extra state.
  T-016 owns the switch.
- **Keybinding collision.** ALT-Q must not shadow an existing chord; T-013 verifies the
  binding is free before wiring it (NFR-009).
- **Destructive-action default.** The `Clear` confirmation must default to `No` and must
  not delete any entry before `Yes` is chosen; T-021 and T-022 split the dialog from the
  deletion so a mis-keyed `Enter` cannot lose the queue. A dismissed dialog (`No`/`Esc`)
  must be a strict no-op.
- **Autopilot interaction.** Autopilot continuation already dispatches at turn end; the
  queue must not race it. Keep a single drain point and check `is_processing` first.
- **Panel reorder vs. FIFO.** The `Show` panel's `Enter` reorder is the one explicit,
  user-initiated exception to FIFO (FR-019/FR-040); it must swap only the neighbouring
  pair so the relative order of every other entry is untouched, and it must never be
  reachable from an automatic path. The panel's `Del` must clamp the highlight so a
  subsequent `Enter` cannot act on a stale index.
---
status: draft
---
# Manual Test Plan: Message Input Queue

**Spec:** [SPEC.md](SPEC.md) - **Plan:** [PLAN.md](PLAN.md)

This is a **manual** test plan. Every case is executed by a human in a real terminal
session using the interactive TUI. It contains no automated test code; every case is
performed by hand.

## Prerequisites

1. Build the binary. Run `cargo build` (allow up to 1000 seconds) and confirm
   `./target/debug/ragent --version` prints the current version.
2. Use a **deliberately slow** model so there is time to queue messages while a turn
   runs. A small local Ollama model (for example `qwen2.5:0.5b` with a low token cap) or
   any provider with noticeable latency works. Confirm a plain turn takes at least ~3
   seconds.
3. Use a terminal at least **100x30** so the input field and counter are fully visible.
4. Start every TUI session inside a disposable working directory, for example
   `~/scratch/inputqueue-tests/`, so session state does not pollute a real project.
5. Have a stopwatch or mental count available where the case asks you to observe timing.
6. Know the two input-field navigation idioms used below:
   - The prompt line is at the **bottom** of the screen, inside the box titled `Input`.
   - `Enter` submits; `Shift+Enter` inserts a newline; `Up`/`Down` browse input history.
7. Know the ALT-Q menu interaction: `ALT-Q` opens it, `Up`/`Down` move the selection,
   `Enter` chooses, and `Esc` dismisses.
8. Know the clear-confirmation dialog interaction: selecting `Clear` opens a
   `Clear the input queue?` dialog with `Yes` and `No`; `Left`/`Right` (or `Tab`) move the
   selection, `Enter` chooses, and `Esc` dismisses. The `No` option is selected by
   default.

## Test Cases

### TC-001 - Input field accepts a message while the agent is executing

**Title:** Verify the input is no longer locked while the primary agent runs.

**Preconditions:**
- Provider and model configured (see Prerequisites).
- TUI open at the main chat screen with a fresh session.

**Steps:**
1. Launch the TUI:
   ```
   ./target/debug/ragent
   ```
2. Type the first prompt: `write a haiku about compilers`
3. Press `Enter`.
4. Immediately (while the status bar shows `processing`) type the second prompt:
   `now write a haiku about linkers`
5. Observe the input box **before** pressing Enter.

**Test data to enter:**
- Prompt 1: `write a haiku about compilers`
- Prompt 2: `now write a haiku about linkers`

**Expected results:**
- The typed characters of prompt 2 appear in the input field. No characters are dropped
  (FR-002).
- The status bar does **not** show `busy - wait for the current turn to finish`.
- The input box border is **not** red while the agent is executing (FR-011).

---

### TC-002 - Queued message shows a two-digit counter before the prompt

**Title:** Verify the counter renders as two zero-padded digits immediately before `>`.

**Preconditions:**
- As TC-001, at the point where prompt 2 is still typed but not submitted.

**Steps:**
1. With prompt 2 typed and the agent still executing, press `Enter`.
2. Look at the **empty** input field that remains.
3. Count the characters between the left border of the `Input` box and the `>` glyph.

**Test data to enter:**
- Prompt 2: `now write a haiku about linkers` (submitted while busy).

**Expected results:**
- Pressing `Enter` while executing appends the entry to the queue, clears the input field,
  and adds the entry to history (FR-005).
- The input field is cleared after submission.
- The prompt now reads `01> ` followed by the dimmed placeholder text: a two-digit
  counter `01` immediately before `>` (FR-009); the render updates on the next frame
  (NFR-003).
- The counter is exactly two columns wide, with a single space between it and `>`.
- Optional (FR-014): an info line such as `queued (1 in queue)` may appear in the log
  panel when the entry is enqueued.

---

### TC-003 - Counter decrements and the oldest entry executes first

**Title:** Verify FIFO dispatch and that the counter decrements as entries are popped.

**Preconditions:**
- As TC-001, with a slow model and an empty queue.

**Steps:**
1. Submit prompt `A: list three Rust crates` with `Enter`.
2. While processing, type `B: list three Rust tools` and press `Enter`. Observe `01>`.
3. While still processing, type `C: list three Rust editors` and press `Enter`. Observe
   `02>`.
4. While still processing, type `D: list three Rust linters` and press `Enter`. Observe
   `03>`.
5. Wait for turn A to finish and observe the counter.
6. Wait for turn B to finish and observe the counter.
7. Wait for turn C to finish and observe the counter.
8. Wait for turn D to finish and observe the counter.

**Test data to enter:**
- A: `A: list three Rust crates`
- B: `B: list three Rust tools`
- C: `C: list three Rust editors`
- D: `D: list three Rust linters`

**Expected results:**
- After step 4 the counter reads `03`.
- Responses appear in the conversation order A, B, C, D - never reordered (FR-001,
  FR-006, FR-019).
- After A finishes the counter reads `02`; after B it reads `01`; after C it is gone;
  after D it is gone (FR-008, FR-010).
- While the queue is non-empty the counter stays visible and the field stays editable
  (FR-012).
- No queued entry is dispatched while the current turn is still executing; each entry is
  held until the turn boundary (FR-016).

---

### TC-004 - Queued entries are added to history when entered

**Title:** Verify entries reach input history at submission, before execution.

**Preconditions:**
- As TC-001.

**Steps:**
1. Submit a long-running prompt: `count slowly to ten with explanations`, then `Enter`.
2. While it runs, type `HISTORY-PROBE-1` and press `Enter`.
3. While it still runs, type `HISTORY-PROBE-2` and press `Enter`.
4. Press the `Up` arrow key once and read the input field.
5. Press `Up` again and read the input field.
6. Press `Up` a third time and read the input field.
7. Press `Esc` only if the agent is still running and you want to cancel it (optional).

**Test data to enter:**
- Baseline prompt: `count slowly to ten with explanations`
- Probe 1: `HISTORY-PROBE-1`
- Probe 2: `HISTORY-PROBE-2`

**Expected results:**
- Before the running turn has finished, `Up` already retrieves the queued entries from
  history (FR-003): the newest entry appears first, so the first `Up` shows
  `HISTORY-PROBE-2`, the second shows `HISTORY-PROBE-1`, and the third shows the baseline
  prompt.
- No `HISTORY-PROBE` entry is missing from history, even if the running turn is
  cancelled.

---

### TC-005 - Empty queue renders no counter

**Title:** Verify the prompt has no counter when the queue is empty.

**Preconditions:**
- TUI at the main chat screen, agent idle, queue empty.

**Steps:**
1. Do not type anything. Look at the input field.
2. Type `/model`, then `Esc` to close the model picker (no change needed).
3. Confirm the queue is empty (no counter is visible).

**Test data to enter:**
- `/model` (then `Esc`).

**Expected results:**
- The idle prompt is `> ` immediately followed by the dimmed placeholder. No digits are
  rendered before `>` (FR-010).

---

### TC-006 - Queue survives an agent error and continues draining

**Title:** Verify a failed turn does not strand the queue.

**Preconditions:**
- A way to force an agent error, for example an intentionally invalid model id or a
  temporarily blocked network path.
- Queue containing at least two entries.

**Steps:**
1. Submit a first prompt that will fail, then `Enter`.
2. While it runs, enqueue `ERROR-QUEUE-1` with `Enter`, then `ERROR-QUEUE-2` with `Enter`.
3. Let the failing turn end and observe.
4. Wait for the next entry to run.

**Test data to enter:**
- First prompt: `this will fail`
- Queued: `ERROR-QUEUE-1`, `ERROR-QUEUE-2`

**Expected results:**
- When the error is reported, the queue is **not** cleared (FR-018): the counter still
  reflects the remaining entries.
- The oldest queued entry (`ERROR-QUEUE-1`) is dispatched next (FR-007), then
  `ERROR-QUEUE-2`; the drain uses the normal asynchronous dispatch path without blocking
  the UI (NFR-004).
- If that entry also errors, the queue still drains the remaining entries.

---

### TC-007 - Cancelling a turn keeps the queue

**Title:** Verify Esc stops the current turn but not the queue.

**Preconditions:**
- A slow model so a turn can be cancelled mid-flight.
- Queue containing two entries.

**Steps:**
1. Submit a long-running prompt: `write a 2000-word essay on memory safety`, `Enter`.
2. Enqueue `CANCEL-QUEUE-1` with `Enter`, then `CANCEL-QUEUE-2` with `Enter`.
3. Observe the counter (`02`).
4. Press `Esc` to cancel the running turn.
5. Observe the counter and the conversation.
6. Wait for the next entry to be dispatched.

**Test data to enter:**
- Essay prompt: `write a 2000-word essay on memory safety`
- Queued: `CANCEL-QUEUE-1`, `CANCEL-QUEUE-2`

**Expected results:**
- `Esc` halts the running turn and the status shows `halted`.
- The queue is retained: the counter still shows the remaining entries (FR-018).
- The next boundary dispatches `CANCEL-QUEUE-1` in FIFO order.

---

### TC-008 - Slash and bang commands are not queued while busy

**Title:** Verify only plain messages are queued.

**Preconditions:**
- As TC-001, agent executing.

**Steps:**
1. Submit a long-running prompt, then `Enter`.
2. While it runs, type `/status` and press `Enter`. Observe the status line.
3. While it still runs, type `! ls -la` and press `Enter`. Observe.
4. Type `PLAIN-AFTER-COMMANDS` and press `Enter`. Observe the counter.

**Test data to enter:**
- Running prompt: e.g. `summarise the history of the Rust logo`
- `/status`
- `! ls -la`
- `PLAIN-AFTER-COMMANDS`

**Expected results:**
- `/status` and `! ls -la` are **not** queued: the counter does not increase for them
  (FR-017). They behave as before while the agent is busy.
- `PLAIN-AFTER-COMMANDS` is queued and the counter reads `01`.

---

### TC-009 - Counter prefix does not affect cursor position or selection

**Title:** Verify cursor placement and copy/cut ignore the counter prefix.

**Preconditions:**
- Queue contains at least one entry (counter visible, e.g. `02> `).

**Steps:**
1. Type `alpha beta gamma` into the field.
2. Press `Home`, then `Right` twice. Press `X`. The field should read `alXpha beta gamma`
   if the cursor is correctly placed after the prompt.
3. Press `Ctrl+A` to select all text, then watch the highlighted region.
4. Press `Ctrl+C`, then `Ctrl+A`, then `Ctrl+X` and observe the resulting field.
5. Type the same text again on a **wrapped** narrow terminal (resize to ~60 columns) and
   verify the cursor stays on the correct row and column.

**Test data to enter:**
- `alpha beta gamma`
- Keystrokes: `Home`, `Right`, `Right`, `X`, `Ctrl+A`, `Ctrl+C`, `Ctrl+A`, `Ctrl+X`

**Expected results:**
- The cursor lands inside the typed text, not on the counter digits (NFR-002).
- `Ctrl+A` selects only the message text; the counter digits are never highlighted and
  are never copied or cut (FR-020).
- At a narrow width, the cursor row/column stays correct after wrapping.

---

### TC-010 - Queue capacity cap rejects overflow without losing input

**Title:** Verify the 32-entry cap is enforced.

**Preconditions:**
- A model slow enough to accept 33 submissions while a turn runs, or a scripted rapid
  sequence of typed entries.

**Steps:**
1. Submit a very long-running prompt, then `Enter`.
2. Enqueue `Q-01` with `Enter`, then `Q-02`, and continue enqueuing `Q-03` ... `Q-33`,
   each followed by `Enter`.
3. After the 33rd attempt, read the status line and the input field.

**Test data to enter:**
- Running prompt: e.g. `list every country in the world with its capital`
- Queued: `Q-01` through `Q-33`

**Expected results:**
- The queue accepts up to **32** entries: the counter shows `32` at most (FR-004).
- The 33rd submission is rejected with a status message; the typed text remains in the
  input field so it is not lost.
- The counter never exceeds `32`, and the field stays editable.
- The cap takes effect without observable lag even with a long queue, consistent with the
  O(1) enqueue/dequeue requirement (NFR-001).
- If the configurable capacity is set (FR-015), the enforced maximum matches the configured
  value instead of 32.

---

### TC-011 - Queued message with an image attachment preserves the attachment

**Title:** Verify staged attachments travel with a queued entry.

**Preconditions:**
- A PNG or JPEG image file on the clipboard, or a path that `Alt+V` can paste as an
  attachment.
- Slow model.

**Steps:**
1. Submit a long-running prompt.
2. While it runs, press `Alt+V` to stage an image attachment. Confirm the box title shows
   the attachment name.
3. Type `describe this image` and press `Enter`. Observe the counter.
4. Let the running turn finish and wait for the queued entry to be dispatched.

**Test data to enter:**
- Running prompt: e.g. `summarise the last ten US presidents`
- `Alt+V` to stage an image.
- `describe this image`

**Expected results:**
- The queued entry is accepted and the counter shows `01`.
- When dispatched, the message in the conversation shows the attachment marker
  (`[attach <name>] describe this image`) and the model receives the image.

---

### TC-012 - Queue clears on new or switched session

**Title:** Verify the queue is not persisted and resets with the session.

**Preconditions:**
- A slow model; queue containing at least two entries.

**Steps:**
1. Submit a long-running prompt, then enqueue `SESSION-QUEUE-1` and `SESSION-QUEUE-2`.
2. Observe the counter (`02`).
3. Cancel the running turn with `Esc`.
4. Start a new session with `/new` (or switch away and back with `/session`), following
   any confirmation prompts.
5. Observe the input field.

**Test data to enter:**
- Running prompt: e.g. `explain the CAP theorem`
- Queued: `SESSION-QUEUE-1`, `SESSION-QUEUE-2`
- `/new`

**Expected results:**
- After the session is reset, the counter is gone and the queue is empty (NFR-005).
- Quitting and restarting the TUI also starts with an empty queue (no persistence).

---

### TC-013 - Optional `/queue` command lists and clears entries

**Title:** Verify the optional queue inspection command (skip if not implemented).

**Preconditions:**
- The `/queue` command is implemented (FR-013). If it is not, mark this case N/A.

**Steps:**
1. Submit a long-running prompt, then enqueue `QUEUE-CMD-1` and `QUEUE-CMD-2`.
2. Type `/queue list` and press `Enter`. Read the report.
3. Type `/queue clear` and press `Enter`. Observe the counter.
4. Type `/queue help` and press `Enter`.

**Test data to enter:**
- Queued: `QUEUE-CMD-1`, `QUEUE-CMD-2`
- Commands: `/queue list`, `/queue clear`, `/queue help`

**Expected results:**
- `/queue list` names both queued entries in order.
- `/queue clear` empties the queue and the counter disappears (FR-010).
- `/queue help` prints the subcommand usage.

---

### TC-014 - ALT-Q opens the queue-control menu without disturbing state

**Title:** Verify ALT-Q opens the four-option menu and leaves the input untouched.

**Preconditions:**
- TUI at the main chat screen with a fresh session.
- Slow model.

**Steps:**
1. Submit a long-running prompt with `Enter`.
2. While it runs, type `MENU-OPEN-PROBE` but do **not** press `Enter`.
3. Press `ALT-Q`.
4. Read the menu contents.
5. Press `Esc` to dismiss the menu.
6. Read the input field.

**Test data to enter:**
- Running prompt: e.g. `explain monomorphisation`
- Typed-in-buffer text: `MENU-OPEN-PROBE`
- Keys: `ALT-Q`, then `Esc`

**Expected results:**
- The menu appears with exactly four options: `Next`, `Stop`, `Clear`, and `Show` (FR-021).
- The input buffer still contains `MENU-OPEN-PROBE`; no characters were inserted or lost
  (FR-022, FR-031).
- `Esc` dismisses the menu and takes no action; the running turn continues (FR-032).
- Opening, navigating, and dismissing the menu never freezes the UI or the running turn
  (NFR-006).
- The menu renders as a standard overlay, consistent with the app's existing dialogs
  (NFR-007).
- `ALT-Q` opens the menu and does not shadow other keys: while the menu is closed, every
  other key behaves exactly as before (NFR-009).

---

### TC-015 - `Next` option visibility tracks the queue

**Title:** Verify `Next` is hidden or non-selectable when the queue is empty.

**Preconditions:**
- Slow model.
- Empty queue at the start.

**Steps:**
1. Press `ALT-Q` with an empty queue.
2. Read the menu; note whether `Next` is shown.
3. Press `Esc` to dismiss.
4. Submit a long-running prompt, then enqueue `NEXT-VISIBLE-1` with `Enter`.
5. Press `ALT-Q` again.
6. Read the menu.

**Test data to enter:**
- Keys: `ALT-Q`, `Esc`, `ALT-Q`
- Queued: `NEXT-VISIBLE-1`

**Expected results:**
- With an empty queue, `Next` is hidden or rendered non-selectable (FR-023); `Stop`/`Clear`
  remain available.
- With one queued entry, `Next` is visible and selectable (FR-023).
- `Esc` dismisses cleanly in both cases.

---

### TC-016 - `Next` stops the current turn and runs the oldest queued entry

**Title:** Verify selecting `Next` halts the turn and immediately advances the queue.

**Preconditions:**
- Slow model so a turn can be interrupted mid-flight.
- Queue containing at least two entries.

**Steps:**
1. Submit a long-running prompt: `write a 3000-word essay on CPU caches`, `Enter`.
2. Enqueue `NEXT-QUEUE-1` with `Enter`, then `NEXT-QUEUE-2` with `Enter`. Observe `02>`.
3. Press `ALT-Q`.
4. Select `Next` and press `Enter`.
5. Observe the status, the conversation, and the counter.
6. Wait for the next entry to run.

**Test data to enter:**
- Essay prompt: `write a 3000-word essay on CPU caches`
- Queued: `NEXT-QUEUE-1`, `NEXT-QUEUE-2`
- Keys: `ALT-Q`, select `Next`, `Enter`

**Expected results:**
- The running turn stops (status shows `halted`) and `NEXT-QUEUE-1` is dispatched
  immediately as the next user turn (FR-024).
- The menu closes after the action.
- The counter drops from `02` to `01`; `NEXT-QUEUE-2` runs after `NEXT-QUEUE-1` finishes.
- FIFO order is preserved - `NEXT-QUEUE-1` before `NEXT-QUEUE-2`.
- If the turn is in the middle of a compaction run, `Next` does not dispatch a queued entry
  immediately: the action is deferred to the next safe boundary or refused with a status
  message (FR-030).

---

### TC-017 - `Stop` halts the agent without advancing the queue

**Title:** Verify selecting `Stop` does not consume a queued entry.

**Preconditions:**
- Slow model.
- Queue containing at least two entries.

**Steps:**
1. Submit a long-running prompt with `Enter`.
2. Enqueue `STOP-QUEUE-1` with `Enter`, then `STOP-QUEUE-2` with `Enter`. Observe `02>`.
3. Press `ALT-Q`.
4. Select `Stop` and press `Enter`.
5. Observe the status and the counter.
6. Wait a few seconds and confirm no queued entry is auto-dispatched.

**Test data to enter:**
- Running prompt: e.g. `list the first 50 prime numbers with proofs`
- Queued: `STOP-QUEUE-1`, `STOP-QUEUE-2`
- Keys: `ALT-Q`, select `Stop`, `Enter`

**Expected results:**
- The running turn stops exactly as `Esc`/`CancelAgent` would (status shows `halted`)
  (FR-025).
- The queue is **not** advanced: the counter still reads `02` (FR-029).
- No queued entry is dispatched automatically after the halt.

---

### TC-018 - `Stop` label becomes `Resume` when stopped, and `Resume` resumes

**Title:** Verify the halt option label switches and resume rebuilds the interrupted work.

**Preconditions:**
- Slow model.
- Queue containing at least one entry.

**Steps:**
1. Submit a long-running prompt with `Enter`.
2. Enqueue `RESUME-QUEUE-1` with `Enter`. Observe `01>`.
3. Press `ALT-Q` and read the halt option label while the agent is executing.
4. Select the halt option (`Stop`) and press `Enter`. Observe the status.
5. Press `ALT-Q` again and read the halt option label now.
6. Select the option (now `Resume`) and press `Enter`.
7. Observe the conversation and the counter.

**Test data to enter:**
- Running prompt: e.g. `explain branch prediction in depth`
- Queued: `RESUME-QUEUE-1`
- Keys: `ALT-Q`, select `Stop`, `Enter`, `ALT-Q`, select `Resume`, `Enter`

**Expected results:**
- While executing, the option is labelled `Stop` (FR-026).
- After stopping, the same option is labelled `Resume` (FR-026).
- Selecting `Resume` resumes the interrupted work rather than halting it (FR-027); the
  queue is not consumed and the counter still reads `01`.

---

### TC-019 - `Clear` option opens the confirmation dialog

**Title:** Verify selecting `Clear` presents the confirmation dialog.

**Preconditions:**
- Slow model.
- Queue containing at least two entries.

**Steps:**
1. Submit a long-running prompt with `Enter`.
2. Enqueue `CLEAR-QUEUE-1` with `Enter`, then `CLEAR-QUEUE-2` with `Enter`. Observe `02>`.
3. Press `ALT-Q`.
4. Select `Clear` and press `Enter`.
5. Read the dialog that appears.

**Test data to enter:**
- Running prompt: e.g. `summarise the entire Rust standard library`
- Queued: `CLEAR-QUEUE-1`, `CLEAR-QUEUE-2`
- Keys: `ALT-Q`, select `Clear`, `Enter`

**Expected results:**
- A confirmation dialog appears with the message `Clear the input queue?` and exactly two
  options, `Yes` and `No` (FR-033).
- The queue is **not** emptied yet: the counter still reads `02` while the dialog is open
  (FR-037).
- The dialog paints on the next frame without a forced repaint loop (NFR-008, NFR-011).
- The currently executing turn is unaffected.

---

### TC-020 - Esc dismisses the menu without mutating input

**Title:** Verify dismissing the queue-control menu with Esc is a no-op.

**Preconditions:**
- Slow model.
- Queue containing at least one entry.

**Steps:**
1. Submit a long-running prompt with `Enter`.
2. Enqueue `ESC-QUEUE-1` with `Enter`. Observe `01>`.
3. Type `ESC-PROBE` in the input field but do **not** press `Enter`.
4. Press `ALT-Q` to open the menu.
5. Move the selection with `Up`/`Down` but do not press `Enter`.
6. Press `Esc` to dismiss.
7. Read the input field, the counter, and the conversation.

**Test data to enter:**
- Running prompt: e.g. `describe the difference between threads and async tasks`
- Queued: `ESC-QUEUE-1`
- Buffer text: `ESC-PROBE`
- Keys: `ALT-Q`, `Down`, `Esc`

**Expected results:**
- The menu closes and no action is taken (FR-032).
- The input buffer still reads `ESC-PROBE`; no characters were inserted or removed
  (FR-031).
- The queue is unchanged: the counter still reads `01` and the running turn continues.

---

### TC-021 - Confirmation dialog defaults to `No`

**Title:** Verify pressing Enter on the dialog without changing the selection keeps the queue.

**Preconditions:**
- Slow model.
- Queue containing at least two entries.

**Steps:**
1. Submit a long-running prompt with `Enter`.
2. Enqueue `DEFAULT-QUEUE-1` with `Enter`, then `DEFAULT-QUEUE-2` with `Enter`. Observe
   `02>`.
3. Press `ALT-Q`, select `Clear`, and press `Enter` to open the confirmation dialog.
4. Without pressing any arrow key, read which option is highlighted.
5. Press `Enter` immediately.
6. Observe the input field, the counter, and the conversation.

**Test data to enter:**
- Running prompt: e.g. `list every Rust RFC in order`
- Queued: `DEFAULT-QUEUE-1`, `DEFAULT-QUEUE-2`
- Keys: `ALT-Q`, select `Clear`, `Enter`, `Enter`

**Expected results:**
- On open, the `No` option is highlighted as the default selection (FR-034).
- The dialog renders as a standard overlay reusing the app's existing dialog machinery, and
  it never blocks the UI (NFR-010).
- Pressing `Enter` without changing the selection chooses `No`, closing the dialog.
- The queue is **unchanged**: the counter still reads `02` and no entry is dispatched
  (FR-035, FR-037).
- The dialog and its closing paint on the next frame without a forced repaint loop
  (NFR-011).

---

### TC-022 - Selecting `No` leaves the queue unchanged

**Title:** Verify the `No` option is a strict no-op for the queue.

**Preconditions:**
- Slow model.
- Queue containing at least two entries.

**Steps:**
1. Submit a long-running prompt with `Enter`.
2. Enqueue `NO-QUEUE-1` with `Enter`, then `NO-QUEUE-2` with `Enter`. Observe `02>`.
3. Press `ALT-Q`, select `Clear`, and press `Enter` to open the confirmation dialog.
4. Move the selection to `No` (if it is not already selected) and press `Enter`.
5. Observe the input field, the counter, the running turn, and the conversation.
6. Wait for the running turn to finish and confirm the queued entries still run.

**Test data to enter:**
- Running prompt: e.g. `explain the borrow checker with examples`
- Queued: `NO-QUEUE-1`, `NO-QUEUE-2`
- Keys: `ALT-Q`, select `Clear`, `Enter`, select `No`, `Enter`

**Expected results:**
- The dialog closes and no action is taken (FR-035).
- The queue is unchanged: the counter still reads `02` and both entries remain.
- The running turn is unaffected; after it ends, `NO-QUEUE-1` then `NO-QUEUE-2` are
  dispatched in FIFO order.

---

### TC-023 - Selecting `Yes` empties the queue and resets the counter

**Title:** Verify the `Yes` option clears the whole queue.

**Preconditions:**
- Slow model.
- Queue containing at least two entries.

**Steps:**
1. Submit a long-running prompt with `Enter`.
2. Enqueue `YES-QUEUE-1` with `Enter`, then `YES-QUEUE-2` with `Enter`. Observe `02>`.
3. Press `ALT-Q`, select `Clear`, and press `Enter` to open the confirmation dialog.
4. Move the selection to `Yes` and press `Enter`.
5. Observe the input field and the counter.
6. Wait for the running turn to finish and confirm no queued entry runs.

**Test data to enter:**
- Running prompt: e.g. `summarise the entire Rust standard library`
- Queued: `YES-QUEUE-1`, `YES-QUEUE-2`
- Keys: `ALT-Q`, select `Clear`, `Enter`, select `Yes`, `Enter`

**Expected results:**
- The dialog closes and the queue is emptied - all entries are removed (FR-028, FR-036).
- The prompt reverts to the bare `> ` form with no counter (FR-036, FR-010).
- The currently executing turn is unaffected; after it ends, no `YES-QUEUE-*` entry is
  dispatched.

---

### TC-024 - Esc dismisses the confirmation dialog without clearing

**Title:** Verify Esc on the confirmation dialog leaves the queue unchanged.

**Preconditions:**
- Slow model.
- Queue containing at least one entry.

**Steps:**
1. Submit a long-running prompt with `Enter`.
2. Enqueue `CONFIRM-ESC-1` with `Enter`. Observe `01>`.
3. Type `CONFIRM-ESC-PROBE` in the input field but do **not** press `Enter`.
4. Press `ALT-Q`, select `Clear`, and press `Enter` to open the confirmation dialog.
5. Press `Left`/`Right` to move the selection but do not press `Enter`.
6. Press `Esc` to dismiss the dialog.
7. Read the input field, the counter, and the conversation.

**Test data to enter:**
- Running prompt: e.g. `describe how cargo resolves dependencies`
- Queued: `CONFIRM-ESC-1`
- Buffer text: `CONFIRM-ESC-PROBE`
- Keys: `ALT-Q`, select `Clear`, `Enter`, `Right`, `Esc`

**Expected results:**
- The dialog closes and no action is taken (FR-035).
- The queue is unchanged: the counter still reads `01` and `CONFIRM-ESC-1` remains
  (FR-037).
- The input buffer still reads `CONFIRM-ESC-PROBE`; the running turn continues.

---

### TC-025 - Menu rows navigate with `Up`/`Down` and activate with `Enter`

**Title:** Verify keyboard navigation skips the dead `Next` row and activates the
highlighted row.

**Preconditions:**
- Slow model.
- Empty queue at the start.

**Steps:**
1. Press `ALT-Q` with an empty queue (selection starts on `Next`).
2. Press `Down`; note the highlighted row.
3. Press `Down` twice more; note the highlighted row each time.
4. Press `Up`; note the highlighted row.
5. Press `Esc`. Submit a long-running prompt and enqueue `NAV-1` with `Enter`.
6. Press `ALT-Q`, press `Down` to highlight `Stop`, press `Enter`.

**Test data to enter:**
- Queued: `NAV-1`
- Keys: `ALT-Q`, `Down`, `Down`, `Down`, `Up`, `Esc`, `ALT-Q`, `Down`, `Enter`

**Expected results:**
- With an empty queue, `Down` from `Next` skips straight to `Stop`/`Resume` and never
  lands on the non-selectable `Next` row (FR-023, FR-038).
- The highlight stays within the four rows; `Down` on the last row is a no-op.
- With a queued entry, `Enter` on the `Stop` row halts the running turn exactly as
  pressing `Escape` would (FR-025) and the queue is not advanced (FR-029).
- Navigation never inserts characters into the input buffer (FR-031).

---

### TC-026 - The `Show` panel lists the queue with a block cursor

**Title:** Verify the `Show` row opens a scrollable panel of the queued entries.

**Preconditions:**
- Slow model.
- At least three entries queued.

**Steps:**
1. Submit a long-running prompt, then enqueue `SHOW-A`, `SHOW-B`, `SHOW-C` with `Enter`.
2. Press `ALT-Q`, `Down` to `Show`, and `Enter`.
3. Read the panel.
4. Press `Down` twice; read the highlighted row.
5. Press `Up`; read the highlighted row.

**Test data to enter:**
- Queued: `SHOW-A`, `SHOW-B`, `SHOW-C`
- Keys: `ALT-Q`, `Down`, `Down`, `Down`, `Enter`, `Down`, `Down`, `Up`

**Expected results:**
- The panel lists every queued entry oldest-first (`SHOW-A`, `SHOW-B`, `SHOW-C`) with one
  entry highlighted by a block cursor (FR-039).
- `Down`/`Up` move the highlight up and down the list, scrolling it so the highlighted
  entry stays visible (FR-039).
- The panel reuses the standard overlay rendering (NFR-012).
- The input buffer and staged attachments are untouched (FR-043).

---

### TC-027 - `Enter` moves the highlighted entry toward the front; `Del` removes it

**Title:** Verify the `Show` panel reorders and deletes entries.

**Preconditions:**
- Slow model.
- Three entries queued.

**Steps:**
1. Enqueue `ORD-A`, `ORD-B`, `ORD-C`.
2. Press `ALT-Q`, `Down` to `Show`, and `Enter`.
3. Press `Down` to highlight `ORD-B`, then press `Enter`.
4. Read the panel.
5. Press `Enter` twice more on the highlighted row.
6. Read the panel.
7. Press `Del` and read the panel and the counter.

**Test data to enter:**
- Queued: `ORD-A`, `ORD-B`, `ORD-C`
- Keys: `ALT-Q`, `Down`, `Down`, `Down`, `Enter`, `Down`, `Enter`, `Enter`, `Enter`, `Del`

**Expected results:**
- The first `Enter` swaps `ORD-B` with the entry before it, moving it one step toward the
  front; the order of every other entry is preserved (FR-040, FR-019).
- Repeated `Enter` walks the highlighted entry to the front one step at a time; at the
  front the key is a no-op (FR-040).
- `Del` removes the highlighted entry and the queue counter decrements (FR-041, FR-042).
- The panel stays open after every change; only `Esc` dismisses it (FR-042).

## Cleanup

1. Quit the TUI with `Ctrl+C` then `Ctrl+D` (the guarded quit sequence).
2. Delete the scratch working directory, for example:
   `rm -rf ~/scratch/inputqueue-tests`.
3. If any provider credentials were entered during setup, remove them with
   `/provider` -> reset, or leave them if you intend to keep using the provider.
4. Remove any test image staged for TC-011 from the clipboard or scratch directory.

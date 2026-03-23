# INPUT_FIX — Input Reliability Recovery Plan

## Context

Input behavior currently diverges between Home and Chat paths and mixes byte- and char-based indexing in multiple places. This causes cursor drift, incorrect insertion/deletion positions, and unreliable mouse cut/copy/paste interactions.

This plan defines a staged, test-driven hardening program.

---

## Root-Cause Summary (Deep-Dive Findings)

### 1) Mixed byte vs char indexing (high risk)

- `input_cursor` is intended to be a **char index**, but several paths still use byte length:
  - `App::handle_history_picker_key` sets `self.input_cursor = self.input.len()` (bytes).
  - Home input render uses `app.input.len()` for cursor placement.
  - Dynamic input height calculations use `app.input.len()`.
- `char_wrap` slices with `text[start..end]` using byte offsets derived from widths, unsafe for Unicode boundaries.
- `copy_selection` input wrapping uses `as_bytes().chunks(inner_w)` and later screen-column-to-byte extraction, which breaks with multibyte characters and wide glyphs.

### 2) Inconsistent editing model across modes

- Normal input path is cursor-aware (`cursor_byte_pos` + `insert/remove` at cursor).
- File-menu active path still uses `push/pop` (append/remove at end), ignoring cursor position.
- Paste action appends via `self.input.push_str(...)` instead of inserting at cursor.
- Cut action clears the whole input after copy, not just selected region.

### 3) Home vs Chat rendering divergence

- Chat render uses `input_cursor` (char index).
- Home render uses byte length and independent cursor math.
- Shared assumptions (`> ` prefix, wrap width, cursor row/col mapping) are duplicated and drift-prone.

### 4) Mouse/context-menu flow regressions

- Right-click behavior currently conflates “quick copy” and context-menu opening in ways that can consume selection unexpectedly.
- Selection extraction relies on cached render lines that are not guaranteed to align with visual width semantics for Unicode/wide chars.
- Context-menu availability checks are broad, but action semantics are weakly scoped (e.g., Cut behavior in input panes).

### 5) Test coverage gaps

- Good baseline tests exist for text selection and scrolling.
- Missing targeted tests for:
  - Home-screen cursor behavior
  - Unicode and wide-character editing/wrapping
  - Paste-at-cursor semantics
  - Cut replacing selected range
  - File-menu edit behavior while cursor is mid-line

---

## Milestone M1 — Build a Unified Input Editing Core

Goal: single source of truth for cursor/index/edit operations.

### Tasks

- Create shared input-edit helpers in `App` (or dedicated `input_model.rs`):
  - `insert_text_at_cursor(&str)`
  - `delete_prev_grapheme()`
  - `delete_next_grapheme()`
  - `replace_range_by_selection(...)` for cut/paste replacement
  - `set_cursor_char_index_clamped(usize)`
- Replace all `push/pop/remove` direct mutation sites in key handlers and context actions with helpers.
- Ensure file-menu and slash-menu modes both call same editing helpers.
- Normalize cursor invariant: `0 <= input_cursor <= input_len_chars()`.
- Add debug assertions for cursor invariant after every mutation in debug builds.

### Exit criteria

- No direct `push/pop` editing remains in user input paths (except explicit full-clear operations).
- Cursor position remains valid after all editing operations.

---

## Milestone M2 — Unicode-Safe Cursor/Wrap/Selection

Goal: eliminate byte-boundary bugs and make behavior stable for non-ASCII.

### Tasks

- Rework `char_wrap` to iterate by char boundaries (or grapheme clusters) instead of byte slicing.
- Replace any `input.len()` usage in cursor/height calculations with char-aware length helpers.
- Fix Home render cursor math to use `input_cursor` exactly like Chat path.
- Introduce `display_line_model(input, width)` helper shared by Home/Chat and selection extraction.
- Replace `as_bytes().chunks(...)` in input selection copy with Unicode-safe wrapping model.
- Ensure selection extraction maps visual columns to stable character boundaries.

### Exit criteria

- Cursor movement/editing on Unicode text (emoji/CJK/accented chars) is deterministic.
- No panics or boundary errors from non-char-boundary slicing.

---

## Milestone M3 — Mouse Selection + Context Menu Reliability

Goal: restore and harden cut/copy/paste UX with explicit semantics.

### Tasks

- Define explicit right-click behavior contract:
  - Option A: always open context menu; Copy requires action selection.
  - Option B: quick-copy only when selection exists, else open menu.
  - Document and test chosen behavior.
- Implement true Cut semantics for input panes:
  - remove selected range only, not full input clear.
- Implement Paste semantics:
  - insert at cursor when no selection
  - replace selection range when selection exists in input pane
- Ensure context menu enabled/disabled state matches pane + selection + clipboard reality.
- Keep selection persistence rules consistent across mouse down/drag/up and menu actions.

### Exit criteria

- Mouse copy/cut/paste works in both Home and Chat input areas.
- No unexpected selection loss unless action explicitly consumes it.

---

## Milestone M4 — Home/Chat Parity and State Decoupling

Goal: identical editing semantics independent of active screen.

### Tasks

- Centralize render-time cursor/line computation shared by `render_home_input` and `render_input`.
- Ensure `pane_at`, selection highlight, and input area geometry are consistent with centered home input vs chat full-width input.
- Verify transitions Home→Chat and Chat→Home preserve input buffer and cursor correctly.
- Validate overlay interactions (slash menu, file menu, provider dialogs, shortcuts) don’t corrupt cursor/input state.

### Exit criteria

- Same keystroke sequence yields same buffer/cursor outcome in Home and Chat.

---

## Milestone M5 — Regression Test Expansion

Goal: lock in behavior and prevent recurrence.

### Tasks

- Add tests in `test_slash_commands.rs` and/or new `test_input_editing.rs`:
  - insertion at cursor (ASCII + Unicode)
  - delete/backspace near multibyte chars
  - paste at cursor and paste replacing selected range
  - file-menu mode editing respects cursor position
  - history-picker sets char-index cursor (not byte index)
- Extend `test_text_selection.rs`:
  - cut in input removes only selected span
  - right-click contract tests (quick-copy/menu behavior)
  - selection extraction with Unicode content
- Add Home-screen-specific cursor tests (render/model-level where possible).
- Add stress test cases for repeated mode switches + edits.

### Exit criteria

- Tests cover key regressions and pass consistently.

---

## Milestone M6 — Observability and Guard Rails

Goal: improve diagnosability of future input regressions.

### Tasks

- Add structured debug logs for key input transitions in debug mode:
  - key event → action → buffer length/cursor before/after
- Add feature-flagged diagnostics command (or temporary internal hook) to dump input state for reproduction.
- Add lightweight invariants:
  - cursor range validity
  - selection coordinates sane for active pane

### Exit criteria

- Reproduction and diagnosis of input bugs is fast and deterministic.

---

## Proposed Execution Order

1. M1 (editing core)
2. M2 (Unicode-safe math/render)
3. M3 (mouse/context-menu semantics)
4. M4 (Home/Chat parity polish)
5. M5 (tests)
6. M6 (observability)

---

## Definition of Done

- Cursor movement and insertion points are consistent and correct in Home and Chat.
- Mouse cut/copy/paste workflows are stable and predictable.
- Unicode input works without drift/panic/corruption.
- New regression suite covers previous failure modes.
- No existing key UX flows regress (slash/file menus, history, provider dialog interactions).


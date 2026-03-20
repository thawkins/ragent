# BROWSE_FIX — `@` File Picker Usability Hardening Plan

## Context

The `@` picker works for basic selection, but the current UX has friction in real editing flows (mid-line edits, multiple mentions, large repos, and mixed keyboard/mouse usage).  
This plan documents a deep-dive review and milestones to make browsing/mentioning files fast, predictable, and reliable.

---

## Deep-Dive Findings (Current Gaps)

### 1) Mention token targeting is tied to the **last** `@`, not the cursor

- `update_file_menu()` and accept logic operate on `rfind('@')`.
- If a prompt contains multiple mentions, edits near an earlier mention can still target the last one.
- Insertion after choosing an entry truncates from the last `@`, which is incorrect for mid-line / multi-mention editing.

### 2) File-menu key handling is incomplete while open

- While the menu is open, only a small key subset is handled (`Up/Down/Tab/Enter/Esc/Char/Backspace`).
- Common editor motions (`Left/Right/Home/End/Delete`, word motions, selection shortcuts) are blocked in this state.
- This creates mode friction and makes correction workflows harder.

### 3) Enter/Tab behavior is inconsistent and surprising

- `Enter` in file menu inserts selection and may immediately send the whole message.
- `Tab` inserts/navigates without send.
- Users expect mention accept and message send to be separate actions unless explicitly configured.

### 4) Picker feedback is weak for “no match” / boundary states

- When query has no matches, popup disappears rather than showing a “no matches” row.
- There is no inline hint for directory mode, max results, or how to navigate back.
- Long paths can be hard to disambiguate in a narrow popup.

### 5) First-use performance and cache behavior can degrade UX

- First `@` can trigger synchronous full project scan.
- Cache is lazily populated but not clearly invalidated for filesystem changes/cwd transitions.
- Large repos can feel stale or laggy without progressive indexing or refresh cues.

### 6) Mouse interactions are functional but not fully polished

- Popup hit-testing mirrors render geometry, but off-popup clicks may not clearly dismiss picker.
- Hover/selection behavior lacks explicit scroll-window support for longer result sets.
- No explicit visual affordance for current mention span in input text.

### 7) Directory navigation is useful but discoverability is low

- Directory mode exists, including parent entry, but it is not obvious to users.
- Querying inside a browsed directory and “jump back to fuzzy” behavior are unclear.

### 8) Test coverage misses key usability regressions

- Existing tests cover some file-menu edit basics, but not:
  - multiple `@` mentions with cursor-local targeting
  - enter-vs-send semantics
  - no-match visible state
  - cache invalidation/staleness
  - directory-mode transitions + keyboard parity
  - end-to-end mouse dismissal and focus transitions

---

## Milestone M1 — Cursor-Scoped Mention Targeting

Goal: picker always edits the mention token at/nearest the cursor, never a different one.

### Tasks

- Introduce a mention-span parser for the active input that returns:
  - all mention spans
  - active mention span from cursor position
- Replace `rfind('@')` paths with active-span logic in:
  - menu query extraction
  - accept/replace behavior
- Ensure insertion replaces only the active mention token.
- Preserve cursor around inserted path with deterministic placement.
- Add unit tests for:
  - multiple mentions in one line
  - editing mention in middle of prompt
  - cursor before/inside/after mention span

### Exit Criteria

- Selecting from picker only affects the mention under current cursor context.

---

## Milestone M2 — Unified Editing Semantics While Picker Is Open

Goal: opening the picker should not “break” normal text editing.

### Tasks

- Expand key handling in file-menu-open mode to support:
  - `Left/Right/Home/End/Delete`
  - existing word motions/deletes from input core
  - selection-aware edit paths where applicable
- Keep navigation keys reserved for picker (`Up/Down`, optional `Ctrl+N/P`).
- Recompute picker query from active mention after every edit mutation.
- Ensure slash-menu/file-menu precedence rules are explicit and tested.

### Exit Criteria

- Users can fully edit prompt text without closing picker first.

---

## Milestone M3 — Accept/Send UX Contract and Discoverability

Goal: predictable mention acceptance and explicit send behavior.

### Tasks

- Define and implement one clear contract:
  - `Enter` accepts mention only (recommended), send requires second `Enter`, or
  - configurable behavior in settings.
- Keep `Tab` as accept-next-action with no send side effect.
- Add footer hints in popup (e.g., `Enter accept`, `Tab accept`, `Esc close`).
- Show no-match row instead of silently hiding popup.
- Improve title context:
  - fuzzy mode query
  - directory mode path
  - result count cap indicator

### Exit Criteria

- Mention acceptance never unexpectedly sends unless explicitly configured.

---

## Milestone M4 — Performance, Caching, and Freshness

Goal: low-latency picker with reliable, fresh candidates.

### Tasks

- Move project scan/index refresh off hot key path (async/background warmup).
- Add bounded incremental refresh strategy for cache invalidation.
- Track cache metadata (cwd, build stamp, refresh time, file count).
- Add manual refresh command/hotkey for picker cache.
- Add stale-cache guard when cwd/session changes.

### Exit Criteria

- First `@` is responsive, and picker results stay current during active sessions.

---

## Milestone M5 — Directory Mode UX and Path Presentation

Goal: browsing directories feels intentional, legible, and controllable.

### Tasks

- Add optional in-directory filtering while staying in current directory scope.
- Add explicit “back to fuzzy search” action from directory mode.
- Improve path rendering:
  - stable truncation with ellipsis preserving basename
  - clear directory/file visual treatment without width ambiguity
- Add optional toggle to include hidden files/dirs when needed.

### Exit Criteria

- Users can navigate and select deep files quickly without losing context.

---

## Milestone M6 — Mouse/Keyboard Parity + Regression Suite

Goal: robust UX across all interaction styles with guardrail tests.

### Tasks

- Tighten popup lifecycle:
  - click outside popup should close (configurable if needed)
  - focus transitions are explicit
- Add popup scroll-window support for longer match lists.
- Expand tests for:
  - cursor-scoped multi-mention replacement
  - enter/tab semantics and send separation
  - no-match visible state
  - directory-mode transitions
  - cache staleness refresh behavior
  - mouse hover/click/outside-dismiss paths
- Add lightweight diagnostic state output for picker (`query`, `mode`, `active_span`, `cache_age`, `result_count`).

### Exit Criteria

- Picker behavior is consistent, test-protected, and diagnosable.

---

## Proposed Execution Order

1. M1 (correctness of target span)
2. M2 (editing parity while open)
3. M3 (UX contract + feedback)
4. M4 (performance/freshness)
5. M5 (directory and path UX)
6. M6 (parity + regressions + diagnostics)

---

## Definition of Done

- Active mention targeting is cursor-correct in all multi-mention scenarios.
- File picker does not block standard editing semantics.
- Accept/send behavior is explicit and unsurprising.
- First-use and steady-state performance are responsive in large repos.
- Mouse and keyboard interactions are consistent.
- Regression tests cover critical mention/picker workflows end-to-end.

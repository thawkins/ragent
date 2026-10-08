---
status: draft
---

# Manual Test Plan: Plugin Store Category Navigator

**Spec:** [SPEC.md](SPEC.md) · **Plan:** [PLAN.md](PLAN.md)

This is a **manual** test plan. Every case is executed by a human in a real terminal
session. Where a case needs a fixture store index, a configured provider, or mouse
support, the prerequisite is listed before the steps. This document contains no
automated test code, no `#[test]` functions, and no reference to `cargo test`.

The category navigator is operated **both by keyboard and by mouse**. Every key press is
written explicitly, in order, and the exact text to type into each field is given as
**Test data**. Keys are written as `Tab`, `Up`, `Down`, `ENTER`, `ESC`, `Backspace`, and
literal characters in `backticks`. A mouse step names the button and the exact target:
`left-click the Database row in the category list`, `scroll the wheel down over the
category list`.

## Prerequisites

1. **Build the binary.** Run `cargo build` (debug is sufficient; allow up to 1000
   seconds). Confirm `./target/debug/ragent --version` runs and reports the version.
2. **Configure a model.** Some read-only cases need only the TUI, but the "agent turn
   stays undisturbed" checks need an in-progress turn, so configure a provider (or a
   local Ollama model) and confirm a short prompt responds.
3. **Confirm mouse support.** Run the session in a terminal that reports mouse events
   (most modern terminals do). For the keyboard-only regression case (TC-013) mouse
   reporting is deliberately not required.
4. **Prepare a scratch root.** Create `~/scratch/catnav-tests/` and run every TUI session
   from inside it, so the project plugin store is
   `~/scratch/catnav-tests/.ragent/plugins/`.
5. **Stage the fixture plugins** (used as install targets): copy the repository fixtures
   under `~/scratch/catnav-tests/fixtures-src/` - `codex-weather/` (valid Codex dialect),
   `codex-db/` (valid Codex dialect), `claude-todo/` (valid Claude dialect).
6. **Stage the category fixture store index.** Copy the committed fixture indexes
   from the repository into `~/scratch/catnav-tests/store/`:
   `assets/plugins/fixtures/stores/index-codex.json`,
   `assets/plugins/fixtures/stores/index-claude.json`, and (for TC-009)
   `assets/plugins/fixtures/stores/index-nocat.json`. These are the same artifacts
   the `test_plugin_store_category_acceptance.rs` harness drives, so the documented
   walk and the automated walk cannot drift. Because a store endpoint must be
   `https://` (spec `pluginstores` FR-024), serve the directory over a local HTTPS
   endpoint, for example `python3 -m http.server --directory
   ~/scratch/catnav-tests/store 8443` behind a TLS terminator, or any reachable HTTPS
   host. Record the two URLs as `<CODEX_URL>` and `<CLAUDE_URL>`.
7. **Point the stores at the fixtures.** In `~/scratch/catnav-tests/.ragent/ragent.json`
   add:
   ```jsonc
   {
     "plugins": {
       "enabled": true,
       "stores": {
         "codex":  { "url": "<CODEX_URL>" },
         "claude": { "url": "<CLAUDE_URL>" }
       }
     }
   }
   ```
8. **Install one fixture first**, so the installed-colour path inside a category has
   data: run the TUI and `/plugins add ~/scratch/catnav-tests/fixtures-src/codex-weather`,
   then `/plugins list` to confirm `codex-weather` is present.
9. **Preserve config.** Back up `~/scratch/catnav-tests/.ragent/ragent.json` before
   editing so any temporary change can be reverted in Cleanup.

### Test data for the fixture store index

The canonical fixtures are the committed files
`assets/plugins/fixtures/stores/index-codex.json`,
`index-claude.json`, and `index-nocat.json`; copy them verbatim (Prerequisite 6).
The shape below mirrors the committed `index-codex.json` (its `source` values are
illustrative here; the committed fixture uses placeholder URLs). The harness
`test_plugin_store_category_acceptance.rs` parses exactly these committed bytes,
so a change to a fixture that breaks the documented walk also breaks the
automated walk.

```json
{
  "store": "codex",
  "plugins": [
    { "id": "codex-weather", "name": "Codex Weather", "version": "1.2.0",
      "description": "Weather lookups for the agent", "category": "Web",
      "source": "~/scratch/catnav-tests/fixtures-src/codex-weather",
      "dialect": "codex", "tags": ["weather", "http"] },
    { "id": "codex-time", "name": "Codex Time", "version": "0.9.1",
      "description": "Time and timezone helpers", "category": "Utility",
      "source": "~/scratch/catnav-tests/fixtures-src/codex-weather",
      "dialect": "codex" },
    { "id": "codex-shell", "name": "Codex Shell", "version": "2.0.0",
      "description": "Shell command wrappers", "category": "utility",
      "source": "~/scratch/catnav-tests/fixtures-src/codex-weather",
      "dialect": "codex" },
    { "id": "codex-db", "name": "Codex Database", "version": "1.4.2",
      "description": "Postgres and SQLite helpers", "category": "Database",
      "source": "~/scratch/catnav-tests/fixtures-src/codex-db",
      "dialect": "codex" },
    { "id": "codex-lsp", "name": "Codex LSP", "version": "0.3.0",
      "description": "Language server integration", "category": "Language",
      "source": "~/scratch/catnav-tests/fixtures-src/codex-db",
      "dialect": "codex" },
    { "id": "codex-uncat", "name": "Codex Uncategorised", "version": "0.1.0",
      "description": "No category field at all",
      "source": "~/scratch/catnav-tests/fixtures-src/codex-weather",
      "dialect": "codex" }
  ]
}
```

Notes on the fixture (kept intentionally small):

- `Web`, `Utility`, `utility` together force the distinct-set to fold case: exactly one
  `Utility`-equivalent row must appear, not two.
- `codex-uncat` carries no `category`, so it must appear under `ALL` only.
- `index-claude.json` mirrors the shape with `store: "claude"` and entries `claude-todo`
  (category `Productivity`), `claude-db` (category `Database`), `claude-uncat` (no
  category).
- `index-nocat.json`: a copy of `index-codex.json` with every `category` field removed,
  for the `ALL`-only case (TC-009).

## Test Cases

### TC-001 - The category navigator appears and lists ALL plus the distinct categories

**Requirement:** FR-001, FR-004, FR-006, FR-018

**Preconditions:** TUI running from the scratch root; `<CODEX_URL>` points at
`index-codex.json`; `/plugins codex` panel open and the index loaded (the loading row is
gone and the result count is non-zero).

**Steps:**
1. Read the panel at a normal terminal size (`120x40` or larger is convenient).
2. Confirm a left-hand column is present, separated from the result list.
3. Read the top row of the left column.
4. Read the remaining rows of the left column top to bottom.
5. Compare the set of category rows with the `category` values declared in
   `index-codex.json`.
6. Press `ESC` to dismiss; open `/plugins claude` and read its left column.

**Test data:** none typed; read-only.

**Expected results:**
- A left-hand navigator column exists with the `ALL` sentinel as its first row (FR-001).
- The remaining rows are exactly the distinct non-empty categories `Database`,
  `Language`, `Utility`, `Web`, sorted case-insensitively, with the two case-variant
  `Utility`/`utility` entries folded into a single row (FR-001, A5).
- Because FR-021 is implemented, the distinct non-empty `tags` of every entry
  that *also declares a category* follow the declared categories as extra rows -
  here `weather` then `http` (from `codex-weather`). The tag-less `codex-uncat`
  contributes nothing, so the navigator is `ALL`, the four declared categories,
  then `weather`, `http`.
- The `codex-uncat` entry's absent category produces no navigator row (FR-003, FR-027).
- `ALL` is the initially selected row and is drawn in the active-category colour (FR-002).
- The Claude panel shows its own categories (`Database`, `Productivity`, plus the
  `todo` tag row from `claude-todo`) from the same navigator implementation (FR-006).
- The panel keeps its title, search field, and footer key hints; only ASCII glyphs are
  used (FR-004).

### TC-002 - Keyboard: Down/Up move the category cursor and narrow the list on the same press

**Requirement:** FR-002, FR-007, FR-008, FR-014

**Preconditions:** `/plugins codex` panel open with `index-codex.json` loaded; no query
typed; the navigator is reachable (follow the focus key named in the footer).

**Steps:**
1. Note the visible/total count in the title (`n of m`) and the full result list.
2. Press `Tab` to move focus to the category navigator.
3. Press `Down` once. Read the category cursor row and the result list.
4. Press `Down` again. Read both again.
5. Press `Up` once. Read both again.
6. Press `Up` repeatedly until the cursor is on `ALL`; press `Up` once more.

**Test data:** `Tab`, then `Down`, `Down`, `Up`, `Up` x N.

**Expected results:**
- Step 2 the focus moves to the navigator; its cursor is drawn distinctly from the
  inactive pane (FR-009 context; focus model per FR-002).
- Step 3 the category cursor moves one row down and the newly highlighted category is
  applied on that same keypress (A4, FR-007); the result list narrows to the entries in
  that category and the visible count updates (FR-014).
- Step 4 the cursor and result list advance to the next category the same way.
- Step 5 `Up` moves the cursor back one row and re-applies the previous category
  (FR-008).
- Step 6 the cursor clamps on `ALL` and does not wrap (FR-007, FR-008).
- No network request is issued at any step (the loading row never reappears; FR-014).

### TC-003 - Selecting a category narrows correctly and ALL restores the full set

**Requirement:** FR-001, FR-003, FR-014, FR-015, FR-018

**Preconditions:** `/plugins codex` panel open with `index-codex.json` loaded.

**Steps:**
1. Press `Tab` to focus the navigator, then use `Down`/`Up` to place the cursor on
   `Database`.
2. Read the result list and confirm every visible entry declares `category: "Database"`.
3. Move the cursor to `ALL`.
4. Read the result list.
5. Move the cursor to a category, then press the clear-category key named in the footer
   (`c` by default).
6. Read the navigator cursor and the result list.

**Test data:** `Tab`, `Down`/`Up` to `Database`, read; move to `ALL`, read; move to
`Web`, press `c`.

**Expected results:**
- Step 2 only `codex-db` (category `Database`) is shown (FR-001, FR-014).
- Step 3 under `ALL` every entry is shown again, including `codex-uncat`, the entry with
  no category (FR-003, FR-018).
- Step 5 the clear-category key sets the active category to `ALL` and re-derives the
  visible set (FR-015).
- Step 6 the navigator cursor is back on `ALL` and the full list is shown.

### TC-004 - Category and query compose; clearing the query restores the category set

**Requirement:** FR-014, FR-019

**Preconditions:** `/plugins codex` panel open with `index-codex.json` loaded.

**Steps:**
1. Press `Tab` to focus the navigator; move the cursor to `Utility`.
2. Type `time` into the search field (press `Tab` back to the search field first if the
   navigator consumes characters).
3. Read the result list.
4. Press `Backspace` four times to clear the query.
5. Read the result list.
6. Move the cursor to a category that has no entries under the current query (for
   example `Language`) while the query is non-empty.

**Test data:** `Utility`, then `time`, then `Backspace` x4, then `Language`.

**Expected results:**
- Step 3 only entries that are in `Utility` *and* match `time` are shown - that is
  `codex-time` alone (FR-014).
- Step 5 clearing the query restores every `Utility` entry (`codex-time`,
  `codex-shell`) while `ALL` is not selected (FR-014).
- Step 6 when the combination matches nothing, the result area shows the explicit
  empty-state line and the navigator remains visible and operable (FR-019).

### TC-005 - Mouse: left-click a category row selects it

**Requirement:** FR-002, FR-011, FR-013

**Preconditions:** `/plugins codex` panel open with `index-codex.json` loaded; terminal
reports mouse events.

**Steps:**
1. Note the panel area and the position of the navigator column and its rows.
2. Move the pointer over the `Database` row in the category column and left-click once.
3. Read the active-category colour row and the result list.
4. Move the pointer over the `ALL` row and left-click once.
5. Read the result list.
6. Move the pointer over a result row (for example `codex-time`) in the result column
   and left-click once.
7. Read the active category and the result cursor.

**Test data:** left-click `Database`; left-click `ALL`; left-click `codex-time`.

**Expected results:**
- Step 2 the left-click moves the category cursor to `Database`, applies `Database`, and
  focuses the navigator (FR-011).
- Step 3 `Database` is painted in the active-category colour and only `codex-db` is
  shown (FR-002, FR-011).
- Step 4-5 clicking `ALL` restores the full result set (FR-011).
- Step 6-7 clicking a result row moves the result cursor to `codex-time` and leaves the
  active category unchanged at `ALL` (FR-013).
- No click in the navigator installs, downloads, or executes anything (FR-024).

### TC-006 - Mouse wheel scrolls the navigator without changing the category

**Requirement:** FR-012

**Preconditions:** `/plugins codex` panel open; the fixture index extended (temporarily)
with enough distinct categories to overflow the navigator column height, or run in a very
short terminal so the existing categories overflow.

**Steps:**
1. Place the pointer over the navigator column and note the active category.
2. Scroll the wheel down several notches over the navigator.
3. Read the navigator rows and the active category.
4. Scroll the wheel up several notches over the navigator.
5. Place the pointer over the result list and scroll the wheel down.

**Test data:** wheel down x3 (over navigator), wheel up x3 (over navigator), wheel down
x3 (over results).

**Expected results:**
- Steps 2-4 the navigator viewport scrolls, revealing categories further down and back
  up, and the active category is unchanged by the wheel (FR-012).
- Step 5 the wheel over the result list scrolls the result list, not the navigator.

### TC-007 - Focus transfer and Enter apply/install behaviour

**Requirement:** FR-009, FR-010, FR-024

**Preconditions:** `/plugins codex` panel open with `index-codex.json` loaded;
`codex-weather` already installed; `codex-db` not installed.

**Steps:**
1. With the search field focused, press the focus key (`Tab`) to focus the navigator.
2. Read which pane's cursor is highlighted.
3. Press `Tab` again to focus the result list; read which pane's cursor is highlighted.
4. Press `Tab` to return to the navigator and move the cursor to `Database`.
5. Press `ENTER`. Read the focused pane and the result list.
6. With the result list focused and `codex-db` highlighted, press `ENTER`.
7. After the install completes, re-open `/plugins codex`, focus the navigator, select
   `Database`, and read the `codex-db` row.

**Test data:** `Tab`, `Tab`, `Tab`, `Down`/`Up` to `Database`, `ENTER`, `ENTER`.

**Expected results:**
- Steps 2-3 the focused pane's cursor is rendered distinctly and `Up`/`Down` would route
  to that pane (FR-009).
- Step 5 `ENTER` over the navigator applies `Database` and moves focus to the result
  list without installing anything (FR-010, FR-024).
- Step 6 `ENTER` over the result list runs the normal install and the report appears in
  the message window (carries forward spec `pluginstores` FR-006).
- Step 7 within the `Database` category the `codex-db` row now carries the installed
  colour and `[installed]` marker (FR-002 with the existing installed marker).

### TC-008 - Loading state shows an ALL-only navigator; no category is selectable early

**Requirement:** FR-016

**Preconditions:** `<CODEX_URL>` points at a slow or unreachable endpoint so the fetch
takes a moment (or the panel is opened and read in the first frames).

**Steps:**
1. Open `/plugins codex` and immediately read the left column before the index lands.
2. Wait for the loading row to be replaced by results.
3. Read the left column again.

**Test data:** none typed.

**Expected results:**
- Step 1 while the fetch is in flight the navigator shows `ALL` only and no other
  selectable category rows (FR-016); the body shows the loading row.
- Step 3 once the index lands the navigator lists the distinct categories from the
  loaded entries (FR-001).

### TC-009 - A store with no categories browses normally with ALL only

**Requirement:** FR-018, FR-019

**Preconditions:** `<CODEX_URL>` temporarily pointed at `index-nocat.json` (every
`category` field removed); `/plugins codex` panel open.

**Steps:**
1. Read the left column.
2. Type `time` into the search field.
3. Read the result list.
4. Clear the query and press the focus key to navigate categories with `Down`.

**Test data:** `time`, then `Backspace` x4, then `Tab`, `Down`.

**Expected results:**
- Step 1 the navigator shows the `ALL` row only and no error is shown (FR-018).
- Step 2-3 the query still filters normally; the panel behaves exactly as before this
  feature (FR-018).
- Step 4 pressing `Down` on an `ALL`-only navigator leaves the selection on `ALL` and
  shows no empty list (FR-008, FR-019).

### TC-010 - Selecting a category matches only its own entries (exact, case-insensitive)

**Requirement:** FR-003, FR-027

**Preconditions:** `/plugins codex` panel open with `index-codex.json` loaded.

**Steps:**
1. Focus the navigator and place the cursor on the `Utility` row.
2. Confirm exactly one `Utility`-equivalent row exists (not two for `Utility` and
   `utility`).
3. Read the result list.
4. Confirm `codex-uncat` is not shown.
5. Move the cursor to `ALL` and confirm `codex-uncat` reappears.

**Test data:** `Tab`, `Down`/`Up` to `Utility`.

**Expected results:**
- Step 2 the two entries `codex-time` (`category: "Utility"`) and `codex-shell`
  (`category: "utility"`) both match the single `Utility` row: the comparison is exact
  and case-insensitive (A5, FR-003).
- Step 3 both `codex-time` and `codex-shell` are shown (FR-003).
- Step 4 the uncategorised `codex-uncat` is not shown under a specific category
  (FR-003, FR-027).
- Step 5 `ALL` shows `codex-uncat` again (FR-018).

### TC-011 - Empty / error states keep the navigator visible and usable

**Requirement:** FR-019, FR-026

**Preconditions:** `/plugins codex` panel open.

**Steps:**
1. With a valid index loaded, focus the navigator and select a category; type a query no
   entry in that category matches (for example `zzzz`).
2. Read the result area and the navigator.
3. Clear the query; read the result area.
4. Press `ESC` to dismiss, then point `<CODEX_URL>` at a deliberately malformed index
   (`{ "store": "codex", "plugins": [`) and reopen `/plugins codex`; read the body and
   the navigator.
5. Press `ESC` to dismiss; restore the valid `<CODEX_URL>` and reopen.

**Test data:** category `Database`, query `zzzz`; malformed index URL.

**Expected results:**
- Step 2 the result area shows the explicit "no matching plugins" empty-state line, and
  the navigator stays visible and operable (FR-019).
- Step 3 clearing the query restores the category's full set.
- Step 4 the malformed index renders the inline "store index failed" line (carries
  forward spec `pluginstores` FR-013); the navigator shows `ALL` only and the process
  neither panics nor exits (FR-026).

### TC-012 - Terminal resize re-derives the navigator width

**Requirement:** FR-004, FR-020

**Preconditions:** `/plugins codex` panel open with `index-codex.json` loaded.

**Steps:**
1. Note the navigator column width and the result-list width.
2. Resize the terminal window narrower and read the panel.
3. Resize the terminal window wider and read the panel.
4. Resize to a very short height and confirm both columns remain usable.

**Test data:** none typed.

**Expected results:**
- Steps 2-3 the navigator column width and the panel area are re-derived so the
  navigator and result list stay fully visible; the navigator is never clipped off the
  panel (FR-020).
- Only ASCII glyphs are used throughout (FR-004).

### TC-013 - Keyboard-only regression: every category is reachable without a mouse

**Requirement:** FR-023, FR-024

**Preconditions:** a terminal session that does **not** report mouse events (or mouse
reporting disabled); `/plugins codex` panel open with `index-codex.json` loaded.

**Steps:**
1. Using only the keyboard, focus the navigator and move the category cursor through
   every row, top to bottom, and back.
2. Apply each category in turn; read the result list after each.
3. Select a result with `ENTER`.

**Test data:** `Tab`, `Down` x N, `Up` x N, `ENTER`.

**Expected results:**
- Every category, including `ALL` and the last row, is reachable and applicable using
  the keyboard alone; no interaction requires a mouse (FR-023).
- Selecting a result with `ENTER` uses the normal keyboard install path, unchanged
  (FR-024).

## Cleanup

1. Restore `~/scratch/catnav-tests/.ragent/ragent.json` from the backup made in
   Prerequisite 9, so no test `stores` block is left behind.
2. Remove the installed fixture `codex-weather` from the scratch plugin store:
   `/plugins remove codex-weather`, then `/plugins list` to confirm it is gone. Repeat
   for any other fixture installed during TC-007.
3. Stop the local HTTPS server started for the fixture store index.
4. Delete the scratch root `~/scratch/catnav-tests/` (fixtures, store index, and the
   scratch plugin store).

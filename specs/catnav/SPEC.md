---
status: draft
audit:
  - { time: 1791418027, from: "none", to: "draft", actor: "system" }
---
# Specification: Plugin Store Category Navigator - Sidebar Category Filter for the Codex and Claude Plugin Marketplaces

## Overview

This specification extends the two existing plugin-marketplace browse panels - the
panel opened by `/plugins codex` (the Codex store) and the panel opened by `/plugins
claude` (the Claude store, spec `pluginstores`) - with a **category navigator**: a
left-hand vertical list of the plugin categories present in the fetched store index.
Selecting a category row narrows the result list to plugins in that category, layering
a category predicate underneath the existing free-text query. The navigator is operated
both by **keyboard** (arrow keys move a category cursor; the category applies on `Enter`
or immediately on cursor move) and by the **mouse** (a left-click on a category row
selects it; the mouse wheel scrolls a long category list).

The feature changes no install logic. `ENTER` on a result still resolves the entry's
`source` and runs the existing install pipeline; opening the navigator, moving the
category cursor, and clicking a category row never install, download, or execute
anything.

### Why this exists

The current Codex and Claude browse panels list every plugin the store returns and
narrow only by the typed query. Both official catalogues are large (hundreds of
entries) and are organised into categories in their marketplace documents, but that
category is discarded at browse time. A user who wants "the LSP plugins" or "the
database plugins" has to guess a query substring and hope the description mentions it.
A left-hand category list makes the store's own taxonomy the primary navigation axis,
so narrowing is one click or two keystrokes instead of a keyword guess.

### Worked example

```text
+-- Codex Plugin Store ------------------------- search: -------- 12 of 340 --+
| Categories        |  > codex-weather     1.2.0   Weather lookups          |
|                   |    codex-time        0.9.1   Time and timezone         |
| > ALL             |    codex-shell       2.0.0   [installed] Shell wraps   |
|   Language        |    codex-db          1.4.2   Postgres helpers          |
|   Database        |                                                         |
|   Web             |                                                         |
|   Utility         |                                                         |
|                                                                             |
| Tab categories  Up/Down move  Enter install  Esc close                      |
+-----------------------------------------------------------------------------+
```

The left column is the category navigator. `ALL` is the default selection; the
highlighted category row carries a block cursor independent of the result-list cursor.
The right column is the existing result list, unchanged.

## Assumptions and interpretation (read before implementing)

- **A1 - Category source.** A plugin's category is read from the store index entry.
  The store index model gains an optional `category` field (an absent field leaves the
  plugin categorised only under `ALL`). This mirrors the optional-field handling the
  store index already applies to `description`, `dialect`, `tags`, and `homepage`
  (spec `pluginstores` A1).
- **A2 - Category set is derived, not configured.** The navigator offers exactly the
  distinct, non-empty categories present in the fetched index, plus the `ALL`
  sentinel, sorted case-insensitively. No category list is hard-coded and no new
  configuration key is introduced. This mirrors the connector-catalogue category filter
  (`CategoryFilterState` in `ragent-connectors`).
- **A3 - Category and query compose.** A result is visible when it matches both the
  current category selection and the current free-text query. An empty query matches
  every plugin; the `ALL` category matches every plugin.
- **A4 - Selection is immediate.** Moving the category cursor applies the newly
  highlighted category on the same keypress (there is no separate "confirm category"
  step); a mouse click on a category row both moves the cursor and applies the
  category. This avoids a second keystroke to see the effect and keeps mouse and
  keyboard behaviour identical.
- **A5 - Category matching is exact and case-insensitive.** A plugin belongs to a
  category when the category names are equal ignoring ASCII case; a plugin with an
  empty or absent category is visible only under `ALL`.
- **A6 - One navigator, two stores.** The navigator is one implementation shared by
  the Codex and Claude panels, exactly as the panels themselves are (spec
  `pluginstores` A2).
- **A7 - The navigator is a refinement, never a gate.** The navigator is additive: a
  store index with no usable categories still browses normally, showing only `ALL`.
- **A8 - Mouse support is additive.** The navigator is fully operable by keyboard
  alone; mouse input is a convenience on top of the keyboard path, never the only way
  to reach a category. This guarantees the navigator works on terminals without mouse
  reporting.

## Definitions

| Term | Meaning |
| ---- | ------- |
| **Category navigator** | The left-hand vertical category list rendered inside the plugin-store browse panel. |
| **Category row** | One selectable line in the navigator: the `ALL` sentinel or one distinct non-empty category name. |
| **Category cursor** | The block-cursor position within the navigator, independent of the result-list cursor. |
| **Active category** | The category currently selected in the navigator; `ALL` by default. |
| **Category predicate** | The rule that a plugin is visible under the active category (A5). |
| **Result cursor** | The existing block cursor over the filtered result list (spec `pluginstores` FR-010). |
| **Category field** | The optional per-entry `category` string in the store index (A1). |

## Background - existing machinery to reuse

- **`/plugins codex` and `/plugins claude` panels.** `crates/ragent-tui/src/layout.rs`
  (`render_plugin_store_panel`) draws the existing modal;
  `crates/ragent-tui/src/app/state.rs` (`PluginStoreBrowser`) is its state; and
  `crates/ragent-tui/src/input.rs` routes its keys. The navigator extends each of
  these rather than adding a second panel.
- **Store index model.** `crates/ragent-plugins/src/store_index.rs` (`StoreEntry`)
  already carries optional `description`, `dialect`, `tags`, and `homepage`; the
  `category` field joins them.
- **Filtering.** `crates/ragent-tui/src/app/state.rs` (`entry_matches`) is the existing
  query predicate; the category predicate is applied alongside it.
- **Prior art for a category list.** `crates/ragent-connectors/src/browse.rs`
  (`CategoryFilter`, `CategoryFilterState`, `ALL_CATEGORY`) implements a cycling
  category filter for the connector catalogue. The navigator reuses this shape - a
  sentinel plus a derived, sorted, case-insensitive distinct set - but exposes it as a
  selectable visible list rather than a single `c`-key cycle.
- **Mouse routing.** `crates/ragent-tui/src/app/input_handler.rs` (`handle_mouse_event`)
  is where panel hit-testing lives; the navigator adds a category-column hit test
  alongside the existing result-list handling.

## Requirements

### Ubiquitous requirements

**FR-001** - The system shall render, inside the plugin-store browse panel opened by
`/plugins codex` and `/plugins claude`, a left-hand category navigator listing the
`ALL` sentinel and every distinct non-empty category present in the fetched store
index, sorted case-insensitively, with `ALL` first.

**FR-002** - The system shall maintain an active category for the browse panel,
defaulting to `ALL`, and shall render the navigator row whose category is active in a
distinct **active-category colour**, with the row under the category cursor carrying a
**block cursor** independent of the result-list cursor.

**FR-003** - The system shall derive a store entry's category from the optional
`category` field of the store index entry (A1), treating an absent or empty field as
"uncategorised" so that such an entry is visible under `ALL` only.

**FR-004** - The system shall render the navigator and the result list in an ASCII-only
layout, and shall keep the existing panel title, search field, and footer key hints
visible.

**FR-005** - The system shall extend the panel footer key hints to name the category
navigation keys, and shall report the active category in the panel title line.

**FR-006** - The system shall offer the category navigator identically in both the
Codex and the Claude browse panels, sharing one navigator implementation (A6).

### Event-driven requirements

**FR-007** - When the user presses `Down` while the category navigator holds keyboard
focus, the system shall move the category cursor one category row toward the end of
the navigator, shall apply the newly highlighted category to the result list on the
same keypress (A4), and shall scroll the navigator so the cursor remains visible.

**FR-008** - When the user presses `Up` while the category navigator holds keyboard
focus, the system shall move the category cursor one category row toward the start of
the navigator, shall apply the newly highlighted category (A4), and shall scroll the
navigator so the cursor remains visible.

**FR-009** - When the user presses a key that transfers focus between the category
navigator and the result list, the system shall move focus to the named pane, shall
render the focused pane's cursor distinctly, and shall route subsequent `Up`/`Down` to
the focused pane.

**FR-010** - When the user presses `Enter` while the category navigator holds focus,
the system shall apply the highlighted category and move focus to the result list,
without installing anything.

**FR-011** - When the user left-clicks a category row, the system shall move the
category cursor to that row, apply that category to the result list, and move keyboard
focus to the navigator.

**FR-012** - When the user scrolls the mouse wheel with the pointer over the category
navigator, the system shall scroll the navigator's viewport without changing the active
category.

**FR-013** - When the user left-clicks a result row while the navigator is shown, the
system shall keep the current active category unchanged and shall move the result
cursor to the clicked row.

**FR-014** - When the active category changes, the system shall re-derive the visible
result set from the full fetched index, shall reset the result cursor to the first
surviving result, and shall update the visible/total count without issuing a network
request.

**FR-015** - When the user presses a key that clears the category filter, the system
shall set the active category to `ALL` and shall re-derive the visible result set
(FR-014).

### State-driven requirements

**FR-016** - While a store-index fetch is still in flight with no entries loaded, the
system shall render the navigator in a **loading** state carrying no selectable category
rows other than `ALL`, so no category can be selected before the categories are known.

**FR-017** - While the category navigator holds keyboard focus, the system shall route
`Up`, `Down`, `Enter`, and the clear-category key to the navigator, and shall route all
other printable characters to the search field only when the navigator does not consume
them.

**FR-018** - While the fetched index declares no non-empty category, the system shall
render the navigator with the `ALL` row only, and shall continue to browse normally
(A7).

**FR-019** - While the active category and the query together match no entry, the system
shall render the existing explicit empty-state line in the result area and shall keep
the navigator visible and operable.

**FR-020** - While the browse panel is open and the terminal is resized, the system shall
re-derive the navigator's column width and area together with the rest of the panel, so
the navigator stays fully visible.

### Optional requirements

**FR-021** - Where the store index entry supplies one or more `tags` in addition to a
category, the system may list each distinct non-empty tag as a category row following
the declared categories.

**FR-022** - Where `/plugins codex` or `/plugins claude` is invoked with a
`--category <name>` argument, the system may open the panel with that category
pre-selected and the navigator cursor placed on that row.

**FR-023** - Where the terminal does not report mouse events, the system may omit the
mouse-driven behaviours (FR-011, FR-012) while preserving every keyboard behaviour
(A8).

### Unwanted requirements

**FR-024** - The system shall not install, download, or write to the plugin store in
response to opening the navigator, moving the category cursor, clicking a category row,
or scrolling the navigator; installation shall occur only on an explicit `ENTER` over a
result row.

**FR-025** - The system shall not execute plugin JavaScript at any point while building,
rendering, navigating, or filtering by category.

**FR-026** - The system shall never panic or terminate the ragent process because a store
index entry carries an unexpected, empty, or oversized category string; every such entry
shall be handled by the ordinary skip-and-count path or by treating the category as
uncategorised.

**FR-027** - The system shall not present a category row for a category that no fetched
entry declares, and shall not show a category row that selects no entry when the
catalogue loads.

**FR-028** - The system shall not block the TUI event loop or an in-progress agent turn
while filtering by category or while scrolling the navigator.

## Interaction model

Focus cycles between two panes inside the panel:

| Pane | `Up`/`Down` | `Enter` | Wheel | Left-click |
| ---- | ----------- | ------- | ----- | ---------- |
| Category navigator | move category cursor; apply category | apply category, focus result list | scroll navigator | select clicked category row |
| Result list | move result cursor | install highlighted result | scroll result list | move result cursor |

- The initial focus is the search field; typing filters as before. A focus key moves to
  the navigator.
- The active category is shown in the panel title (`-- category <name>`), matching the
  connector-catalogue title convention.
- The footer hint line reads, when the navigator is present:
  `Tab focus  Up/Down move  Enter select/install  Esc close`, extended with
  `c clear category` when a non-`ALL` category is active.

## Scope

**In scope**

- A category navigator (left-hand list) in the Codex and Claude plugin-store panels.
- Deriving the category set from the fetched store index; a new optional `category`
  field on the store index entry.
- Applying a category predicate in combination with the existing query predicate.
- Keyboard navigation (move, apply, focus transfer, clear) and mouse navigation
  (click-to-select, wheel scroll) of the navigator.
- Rendering the navigator, its cursor, and the active-category colour.
- Reporting the active category in the title and footer.

**Out of scope**

- Any change to the install pipeline or to plugin execution.
- Persisting the active category across panel opens (beyond the optional FR-022 flag).
- Server-side or remote category filtering; all filtering stays in memory after the
  single index fetch.
- Changing the connector-catalogue panel, except that the shared category-model shape
  may be factored for reuse.

## Acceptance criteria

1. `/plugins codex` on a store whose index declares categories renders a left-hand list
   beginning with `ALL` and listing each distinct category once, sorted.
2. `Down`/`Up` while the navigator has focus moves the category cursor and narrows the
   result list on the same keypress; the visible/total count updates.
3. A left-click on a category row selects it; the active-category colour and the result
   list update to match.
4. A left-click on a result row moves the result cursor and leaves the active category
   unchanged.
5. Typing a query after selecting a category narrows within that category; clearing the
   query restores the category's full set.
6. Selecting `ALL` restores the full result set (within the current query).
7. A store index with no categories renders `ALL` only and browses normally.
8. The result area shows the explicit empty-state line when the category and query match
   nothing, and the navigator stays operable.
9. No keystroke or click in the navigator installs, downloads, or executes anything.
10. The panel, navigator included, uses only ASCII glyphs.

## Open Questions

1. **Focus-transfer key.** FR-009 names a focus-transfer key but does not fix it. The
   default is `Tab` (cycler between navigator and result list), which does not collide
   with the existing printable-query routing; a reviewer may substitute a different key.
2. **Apply-on-move vs. apply-on-confirm.** A4 fixes selection as immediate on cursor
   move. If a reviewer prefers an explicit confirm step, FR-007/FR-008 are the
   requirements to change.
3. **Tag-derived rows.** FR-021 offers tags as extra category rows. If tag lists are
   long, the navigator may become unwieldy; a reviewer may drop FR-021 entirely.

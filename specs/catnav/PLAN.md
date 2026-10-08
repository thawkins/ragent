# Implementation Plan: Plugin Store Category Navigator

**Spec:** [SPEC.md](SPEC.md) · **Test plan:** [TESTPLAN.md](TESTPLAN.md)

## Approach

Add a **category navigator** to the existing plugin-store browse panel. No install or
execution code changes: selecting a category only re-derives the in-memory result set,
and `ENTER` over a result still runs the existing `ragent_plugins::add` pipeline. The
work splits into four layers:

1. **Store-index model (`crates/ragent-plugins/src/store_index.rs`)** - add an optional
   `category` field to `StoreEntry`, parsed through the same skip-and-count path that
   already handles `description`, `dialect`, `tags`, and `homepage`. An absent or empty
   category leaves the entry uncategorised (FR-003, FR-025/FR-026, A1).

2. **Category model (`crates/ragent-connectors/src/browse.rs` or a shared crate)** -
   reuse the existing `CategoryFilter` / `CategoryFilterState` shape (the `ALL`
   sentinel, a derived sorted case-insensitive distinct set, exact case-insensitive
   matching) so the plugin navigator and the connector catalogue agree on what a
   category is (FR-001, FR-003, FR-018, A2, A5). Factor the shared shape if the two
   crates can share it without a new dependency edge (A6).

3. **Browser state (`crates/ragent-tui/src/app/state.rs`, `PluginStoreBrowser`)** - add
   the category set, the active category, the category cursor, the navigator scroll
   offset, and the focused-pane flag; extend `apply_filter` to compose the category
   predicate with the existing query predicate and to reset the result cursor (FR-002,
   FR-007..FR-015). All navigation is pure and unit-testable without a terminal.

4. **Render + input routing (`crates/ragent-tui/src/layout.rs`,
   `crates/ragent-tui/src/input.rs`, `crates/ragent-tui/src/app/input_handler.rs`)** -
   split the panel inner area into a left navigator column and the existing result
   column; render the navigator with its own `List::highlight_style` block cursor and
   active-category colour; route `Up`/`Down`/`Enter`/clear-key and the focus key to the
   focused pane; add a mouse hit test for the navigator column (left-click selects a
   row, wheel scrolls the navigator) alongside the existing result-list mouse handling
   (FR-001, FR-004..FR-013, FR-016..FR-020, FR-028).

The category filter operates entirely in memory after the single index fetch (spec
`pluginstores` A3); changing category issues no network request (FR-014, FR-028).

### Panel layout

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
| Tab focus  Up/Down move  Enter select/install  c clear category  Esc close |
+-----------------------------------------------------------------------------+
```

The left column is the navigator: `ALL` first, then the distinct categories sorted
case-insensitively. The active category is painted in the active-category colour; the
row under the category cursor carries a block cursor independent of the result-list
cursor. Only ASCII glyphs are used (FR-004).

## Requirement Coverage Map

| Requirement | Task(s) |
| ----------- | ------- |
| FR-001 Navigator lists ALL + distinct categories, sorted | T-003, T-004 |
| FR-002 Active category state + cursor + colour | T-003, T-004 |
| FR-003 Category read from entry, empty means uncategorised | T-001, T-002 |
| FR-004 ASCII layout, title/search/footer kept | T-004 |
| FR-005 Footer hints + title active category | T-004 |
| FR-006 One navigator for both stores | T-003, T-004, T-005 |
| FR-007 Down moves category cursor + applies | T-003, T-005 |
| FR-008 Up moves category cursor + applies | T-003, T-005 |
| FR-009 Focus transfer between panes | T-005 |
| FR-010 Enter applies category, focuses results | T-005 |
| FR-011 Left-click category row selects it | T-006 |
| FR-012 Wheel over navigator scrolls it | T-006 |
| FR-013 Left-click result row moves result cursor | T-006 |
| FR-014 Category change re-derives results off-network | T-003, T-007 |
| FR-015 Clear-category key sets ALL | T-005 |
| FR-016 Loading state: ALL-only navigator | T-004 |
| FR-017 Key routing while navigator focused | T-005 |
| FR-018 No categories: ALL-only, browse normally | T-003, T-004 |
| FR-019 Empty result state with navigator visible | T-004 |
| FR-020 Resize re-derives navigator width | T-004 |
| FR-021 Optional tag-derived rows | T-008 |
| FR-022 Optional `--category <name>` pre-selection | T-008 |
| FR-023 Mouse-less terminals keep keyboard path | T-006 |
| FR-024 No install/download from navigator | T-005, T-006 |
| FR-025 No JS execution during category filtering | T-003 |
| FR-026 No panics on odd category strings | T-001, T-003 |
| FR-027 No category row that selects nothing | T-003, T-004 |
| FR-028 No event-loop blocking on filter/scroll | T-003, T-006 |
| Acceptance 1-10 | T-009 |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add optional `category` field to `StoreEntry` and parse it through the existing skip-and-count path | FR-003, FR-026 | S | High | completed | — |
| T-002 | Unit tests for `category` parsing: present, absent, empty, and oversized/odd values do not panic | FR-003, FR-026 | S | High | completed | T-001 |
| T-003 | Add category-filter model (sentinel + derived sorted distinct set + exact case-insensitive predicate) and compose it into `PluginStoreBrowser::apply_filter` | FR-001, FR-003, FR-006, FR-007, FR-008, FR-014, FR-018, FR-025, FR-027, FR-028 | M | Critical | completed | T-001 |
| T-004 | Render the left-hand navigator column: rows, block cursor, active-category colour, loading/ALL-only states, title + footer updates, resize-aware width | FR-001, FR-002, FR-004, FR-005, FR-006, FR-016, FR-018, FR-019, FR-020, FR-027 | M | Critical | completed | T-003 |
| T-005 | Keyboard routing: focus transfer, Up/Down move + apply, Enter apply + focus results, clear-category key, navigator scroll | FR-007, FR-008, FR-009, FR-010, FR-015, FR-017, FR-024 | M | Critical | completed | T-003, T-004 |
| T-006 | Mouse routing: hit-test the navigator column, left-click select, wheel scroll navigator, result-row click keeps category | FR-011, FR-012, FR-013, FR-023, FR-024, FR-028 | M | High | completed | T-004 |
| T-007 | Unit tests for category filtering composition, cursor movement, clear-to-ALL, and re-derivation on category change | FR-007, FR-008, FR-014, FR-015, FR-018 | M | High | completed | T-003, T-005 |
| T-008 | Optional: `--category <name>` pre-selection on launch and tag-derived category rows | FR-021, FR-022 | S | Low | completed | T-005 |
| T-009 | Fixture store index with categories + manual acceptance walk of the test plan + quality gates (`fmt`, `clippy`, `check --workspace`) | Acceptance 1-10 | M | High | completed | T-004, T-005, T-006 |
## Task Details

### T-001 - `category` field on the store index entry

Add `category: Option<String>` (or `#[serde(default)] String`) to `StoreEntry` in
`crates/ragent-plugins/src/store_index.rs`, alongside the existing optional fields. It is
deserialised by the same `StoreEntry::from_value` path, so a wrong-typed value is refused
and counted as malformed rather than panicking. An absent or whitespace-only value leaves
the entry uncategorised (FR-003, FR-026).

### T-002 - `category` parsing tests

Cover: a present category is carried through; an absent category leaves the entry valid
and uncategorised; an empty string is treated as uncategorised; an oversized or unusual
category string parses without panic and without truncating the entry itself. These are
pure model tests in the crate's `tests/` directory (FR-003, FR-026).

### T-003 - Category-filter model and filter composition

Introduce a category filter value and state modelled on the connector-catalogue
`CategoryFilter` / `CategoryFilterState`: an `ALL` sentinel, a distinct non-empty set
derived from the loaded entries (sorted case-insensitively, `ALL` implied), and an exact
case-insensitive match. Add the active category, the derived category set, the category
cursor, and the navigator scroll offset to `PluginStoreBrowser`. Extend `apply_filter` to
keep a row only when it matches both the query and the active category, and to reset the
result cursor. Rebuild the category set when an index lands, and drop any selected
category that the new index no longer declares so no row can select nothing (FR-001,
FR-003, FR-006, FR-007, FR-008, FR-014, FR-018, FR-025, FR-027, FR-028).

### T-004 - Navigator rendering

In `render_plugin_store_panel` (`crates/ragent-tui/src/layout.rs`), split the inner area
into a fixed-width left navigator column and the existing result column. Render the
navigator as a `List` with a block cursor via `highlight_style`; paint the active
category in the active-category colour. Render `ALL` only while the fetch is loading
(FR-016) and when the index declares no categories (FR-018). Keep the result area's
empty-state line (FR-019). Extend `plugin_store_title` with `-- category <name>` and
`plugin_store_footer` with the navigation hints. Recompute the navigator width from
`Frame::area` every frame so a resize stays correct (FR-020). ASCII only (FR-004).

### T-005 - Keyboard routing

Extend the plugin-store key block in `crates/ragent-tui/src/input.rs` with a
focused-pane flag: `Tab` (or the chosen focus key, Open Question 1) transfers focus;
`Up`/`Down` move the focused pane's cursor and, for the navigator, apply the highlighted
category immediately (A4); `Enter` over the navigator applies the category and focuses
the result list, while `Enter` over the result list keeps the existing install behaviour;
`c` (or the chosen clear key) sets the category to `ALL`. Keep the input field locked and
swallow unhandled keys (FR-007, FR-008, FR-009, FR-010, FR-015, FR-017, FR-024).

### T-006 - Mouse routing

In `handle_mouse_event` (`crates/ragent-tui/src/app/input_handler.rs`), add a hit test for
the navigator column: a left-click maps the row under the pointer to a category row,
moves the cursor, applies the category, and focuses the navigator; the wheel over the
navigator scrolls its viewport without changing the active category. A left-click on a
result row moves the result cursor and leaves the active category unchanged. Every mouse
path is a convenience over the keyboard path, so a terminal without mouse reporting still
reaches every category (FR-011, FR-012, FR-013, FR-023, FR-024, FR-028).

### T-007 - Navigation and filtering unit tests

Pure tests over `PluginStoreBrowser`: composing query and category predicates; `Up`/`Down`
moving the category cursor and applying on move; clearing to `ALL` restoring the set;
selecting a category resetting the result cursor; an index with no categories yielding an
`ALL`-only navigator; a category selecting nothing after a re-fetch being dropped
(FR-007, FR-008, FR-014, FR-015, FR-018).

### T-008 - Optional launch flag and tag-derived rows

Parse an optional `--category <name>` on `/plugins codex` / `/plugins claude` and
pre-select the named category once the index lands (validated like an in-panel selection,
refused name reported in the footer). Optionally list distinct non-empty `tags` as extra
category rows after the declared categories. Both are optional and may be dropped by a
reviewer (FR-021, FR-022).

### T-009 - Fixtures, acceptance walk, quality gates

Add a fixture store index declaring several categories, with entries both categorised and
uncategorised, and walk the manual test plan end to end. Run `cargo fmt`, `cargo clippy`,
and `cargo check --workspace` clean before considering the feature done (Acceptance 1-10).

## Risks and Mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Focus model confuses users (which pane has Up/Down) | Distinct cursor colours per pane (FR-002, FR-009) and a footer hint naming the focus key. |
| Category set differs between stores or drifts from the connector catalogue | One shared category model (A2, A6), derived identically from the loaded entries. |
| A selected category vanishes after `--refresh` | T-003 drops a selection no longer present, falling back to `ALL` (FR-027). |
| Long category lists overflow the column | Navigator has its own scroll offset and wheel support (FR-012, T-003, T-006). |
| Mouse-only workflows on terminals without mouse reporting | Keyboard path is complete and authoritative (A8, FR-023). |

## Notes on Assumptions

- The navigator reuses the connector-catalogue category shape (A2, A6); if sharing the
  type across crates would add an unwanted dependency edge, the same shape is
  re-implemented in `ragent-tui` with identical semantics.
- Category comparison is exact and case-insensitive (A5); substring category matching is
  deliberately not offered, to keep the derived set and the predicate in agreement.
- Filtering stays entirely in memory after the single index fetch (spec `pluginstores`
  A3); category navigation never triggers a network request (FR-014, FR-028).
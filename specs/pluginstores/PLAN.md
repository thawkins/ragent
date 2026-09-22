# Implementation Plan: Plugin Store Browser - Search and Install from the Codex and Claude Stores

**Spec:** [SPEC.md](SPEC.md) · **Test plan:** [TESTPLAN.md](TESTPLAN.md)

## Approach

Add a **store browser** on top of the existing `plugins` subsystem. No new install code
is written: the browser resolves a catalogue entry's `source` and calls the existing
`ragent_plugins::add(dir, workdir, source, force=false)`, which already applies the
HTTPS, traversal, and size guards. The work splits into four layers:

1. **Catalogue layer (`crates/ragent-plugins/src/store_index.rs`)** - a new module that
   owns the store registry (`StoreCatalog`: name, dialect label, endpoint URL, timeout),
   the store-index JSON model, and a fetch function that returns
   `Result<Vec<StoreEntry>, StoreError>` with `reqwest` and a wall-clock + byte-size
   budget. It performs no install and no execution (FR-003, FR-023, FR-025).
2. **Installed-set derivation** - reuse `ragent_plugins::store` (`store_dirs` + `scan`)
   to collect the installed plugin ids the panel colourises against (A5, FR-005).
3. **TUI browser state machine (`crates/ragent-tui/src/app/plugin_store.rs`)** - the
   `PluginStoreBrowser` modal: query string, full result list, filtered result set,
   cursor index, scroll offset, fetch status (idle / loading / error), and the last
   install outcome. Pure navigation and filtering logic lives here and is unit-testable
   without a terminal (FR-008, FR-010, FR-012, FR-014, FR-015, FR-017, FR-020).
4. **Render + key routing** - a new `render_plugin_store_panel` in
   `crates/ragent-tui/src/layout.rs` (modelled on `render_queue_show_panel`, including
   its `List::highlight_style` block-cursor technique), a key block in
   `crates/ragent-tui/src/input.rs` modelled on the `queue_show_open` block, and the
   launch arm in `crates/ragent-tui/src/app/slash.rs` + autocomplete entry in
   `crates/ragent-tui/src/app/state.rs` (FR-002, FR-004, FR-005, FR-018).

A fifth concern is **endpoint resolution**: each store carries a compiled-in default
store-index URL so the browsers work out of the box, resolved at every launch so a
configured `plugins.stores.<name>.url` overrides it wholesale and its absence never
fails (FR-027..FR-031, NFR-001, NFR-002, T-014..T-018).

A sixth concern is **default-endpoint verification**: the spec now requires that the
browsers be proven functional against the compiled defaults, offline, through an
injectable fetch seam rather than live store access (FR-032..FR-038, NFR-003, NFR-004,
T-019..T-021).

The fetch is started off the event loop and delivered back as a TUI message so the agent
turn keeps animating (FR-016, FR-026); until it lands the panel renders a loading row.
`ENTER` runs the install on the same off-loop path, then re-derives the installed set
(FR-011). Configuration gains a `stores` sub-object on the existing `plugins` block
(FR-019).

### Panel layout

```text
+-- Codex Plugin Store ---- search: weather ----------- 12 of 340 --+
|  > codex-weather   1.2.0   Weather lookups for the agent          |
|    codex-time      0.9.1   Time and timezone helpers              |
|    codex-shell     2.0.0   [installed] Shell command wrappers     |
|                                                                   |
| Up/Down move  Enter install  Esc close                            |
+-------------------------------------------------------------------+
```

The highlighted row is painted with a full-row block cursor via
`List::highlight_style`; rows whose id is in the installed set use the installed colour
and an `[installed]` marker (FR-005). Only ASCII glyphs are used (FR-004).

## Requirement Coverage Map

| Requirement | Task(s) |
| ----------- | ------- |
| FR-001 Store registry | T-002, T-003 |
| FR-002 `/plugins codex` + `/plugins claude` | T-004, T-011 |
| FR-003 Fetch and parse store index | T-003 |
| FR-004 Browse panel layout | T-005, T-006 |
| FR-005 Block cursor + installed colour | T-006, T-007 |
| FR-006 Install via existing entry point | T-009 |
| FR-007 Launch opens panel and fetches | T-004, T-008 |
| FR-008 Type filters in memory | T-005, T-012 |
| FR-009 Backspace / Esc query editing | T-004, T-012 |
| FR-010 Up/Down move + scroll | T-005, T-012 |
| FR-011 ENTER installs and re-colours | T-006, T-009 |
| FR-012 Esc dismisses, no side effect | T-004, T-005 |
| FR-013 Fetch error rendered inline | T-003, T-008, T-010 |
| FR-014 ENTER on installed result | T-006, T-009 |
| FR-015 Key routing, input lock | T-004, T-007 |
| FR-016 Non-blocking fetch | T-003, T-008 |
| FR-017 Empty / error states | T-005, T-006 |
| FR-018 Resize re-derives area | T-006 |
| FR-019 Configurable endpoints | T-001 |
| FR-020 Pre-filled query | T-004, T-011 |
| FR-021 `--refresh` | T-003, T-011 |
| FR-022 No install without ENTER | T-005, T-009 |
| FR-023 No execution during browse | T-003 |
| FR-024 Endpoint/source guards | T-002, T-003, T-009, T-015 |
| FR-025 No panics from store data | T-003, T-009, T-010 |
| FR-026 Non-blocking install | T-008, T-009 |
| FR-027 Compiled default endpoint per store | T-014, T-015 |
| FR-028 Configured URL overrides default | T-015, T-018 |
| FR-029 Default endpoint is HTTPS | T-014, T-016 |
| FR-030 Launch works with no `plugins.stores` block | T-016, T-018 |
| FR-031 `/plugins stores` reports effective endpoint + source | T-017 |
| FR-032 Default-store browser works out of the box (tests) | T-019, T-020 |
| FR-033 Launch with no `plugins.stores` uses compiled default (tests) | T-019 |
| FR-034 Fetch from default endpoint populates + drives browser (tests) | T-020 |
| FR-035 Configured URL overrides default in the browser (tests) | T-020 |
| FR-036 No-config launch opens both stores, no error (tests) | T-019, T-021 |
| FR-037 Default-endpoint tests use an offline fixture fetch seam | T-019, T-021 |
| FR-038 `/plugins stores` tags default-sourced endpoints (tests) | T-021 |
| NFR-001 Default URL literals declared once | T-014 |
| NFR-002 Endpoint resolved at launch, absence never fails | T-015, T-018 |
| NFR-003 Default-endpoint tests deterministic and offline | T-019, T-021 |
| NFR-004 Both Codex and Claude defaults covered | T-019, T-020 |
| Config schema (`stores`) | T-001 |
| Acceptance criteria 1-10 | T-013 |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `plugins.stores` block to `ragent-config` (`codex`/`claude` URLs, `timeout_ms`, `max_index_bytes`, `cache_ttl_secs`) | FR-019 | S | High | completed | — |
| T-002 | Define `StoreCatalog` and store entry model in `ragent-plugins::store_index` | FR-001, FR-024 | S | High | completed | — |
| T-003 | Implement store-index fetch: HTTPS-only, timeout, byte cap, JSON parse, per-entry validation, no execution | FR-003, FR-013, FR-016, FR-023, FR-024, FR-025 | M | Critical | completed | T-002 |
| T-004 | Add `PluginStoreBrowser` state and launch/close API on `App` (query, results, cursor, status) plus key routing and input lock | FR-002, FR-007, FR-009, FR-012, FR-015, FR-020 | M | Critical | completed | T-002 |
| T-005 | Implement search filtering, cursor clamp, and scrolling logic (pure, unit-testable) | FR-008, FR-010, FR-017, FR-022 | M | High | completed | T-004 |
| T-006 | Render `render_plugin_store_panel`: block cursor via `List::highlight_style`, installed colour + `[installed]`, empty/error/loading states, resize handling | FR-004, FR-005, FR-011, FR-017, FR-018 | M | Critical | completed | T-004, T-005 |
| T-007 | Derive the installed set from the store scan and wire key routing for the modal into `input.rs` | FR-005, FR-015 | S | High | completed | T-004 |
| T-008 | Wire the off-loop fetch: spawn, deliver a TUI message, update panel status without blocking the event loop | FR-007, FR-013, FR-016, FR-026 | M | Critical | completed | T-003, T-004 |
| T-009 | Wire `ENTER` install: call `ragent_plugins::add(force=false)`, handle `AddOutcome`/`AddError`, re-derive installed set, guard double-install | FR-006, FR-011, FR-014, FR-022, FR-024, FR-025, FR-026 | M | Critical | completed | T-003, T-007, T-008 |
| T-010 | Format fetch and install failures as panel/message reports with no panics on malformed store data | FR-013, FR-025 | S | High | completed | T-003, T-009 |
| T-011 | Register `/plugins codex` and `/plugins claude` in the dispatch arm, `SLASH_COMMANDS`, autocomplete, and `/plugins help`; parse `[query]` and `--refresh` | FR-002, FR-020, FR-021 | M | High | completed | T-003, T-004 |
| T-012 | Unit tests for filtering, cursor movement, query editing, and state transitions | FR-008, FR-009, FR-010, FR-012 | M | High | completed | T-005 |
| T-013 | Fixture store index + acceptance walk of the manual test plan and quality gates (`fmt`, `clippy`, `check --workspace`) | Acceptance 1-10 | M | High | completed | T-006, T-008, T-009, T-011 |
| T-014 | Define compiled-in default store-index URL constants for the Codex and Claude stores in `ragent-plugins::store_index` | FR-027, FR-029, NFR-001 | S | High | completed | T-002 |
| T-015 | Resolve the effective endpoint per store: a non-empty `plugins.stores.<name>.url` overrides, otherwise the compiled default; re-resolved at each launch | FR-027, FR-028, FR-029, NFR-002 | S | High | completed | T-001, T-002 |
| T-016 | Wire the default-endpoint fallback into the launch path so `/plugins codex` and `/plugins claude` open with no `plugins.stores` block configured | FR-029, FR-030 | S | High | completed | T-014, T-015 |
| T-017 | Add the `/plugins stores` subcommand reporting each store's effective endpoint and whether it came from config or the compiled default | FR-031 | S | Medium | completed | T-011, T-015 |
| T-018 | Unit tests for endpoint resolution precedence (override wins, default on absence, empty override ignored) and the no-config launch path | FR-027, FR-028, FR-029, FR-030, NFR-001, NFR-002 | S | High | completed | T-015 |
| T-019 | Add an injectable store-fetch seam and offline fixture index so the browser can be tested against each compiled default endpoint with no network | FR-032, FR-033, FR-036, FR-037, NFR-003, NFR-004 | M | High | completed | T-008, T-014, T-015, T-016 |
| T-020 | Tests that a default-endpoint store index populates the browser result set and drives filtering, cursor movement, and `ENTER` install for both Codex and Claude | FR-032, FR-034, FR-035, NFR-004 | M | High | completed | T-019, T-005, T-006, T-009 |
| T-021 | Tests for the no-config launch path (no error, panel opens) and that `/plugins stores` tags default-sourced endpoints | FR-036, FR-037, FR-038, NFR-003 | S | High | completed | T-019, T-011, T-017 |
| T-022 | Add the opt-in `/plugins stores --check` availability probe reporting per-store availability and plugin count | FR-039, FR-040, FR-041, FR-042 | S | Medium | completed | T-017, T-019 |
| T-023 | Add the `StoreProvider` trait with per-store Codex and Claude implementations normalising each vendor marketplace shape | FR-043, FR-044, NFR-005 | M | High | completed | T-002, T-014 |
| T-024 | Thread the store kind through the fetch seam so every fetch parses through the store's provider | FR-043, FR-044 | S | High | completed | T-003, T-023 |
| T-025 | Accept `git+<https-url>#<ref>[:<subpath>]` sources in the install pipeline via a shallow sparse git clone | FR-046, NFR-005 | M | High | completed | T-006, T-023 |
## Task Details

### T-001 - `plugins.stores` configuration block

Add `stores: Option<PluginStoresConfig>` to the existing `plugins` block in
`ragent-config`, with `codex`/`claude` entries (`url`, optional per-store override),
plus `timeout_ms` (default 10000), `max_index_bytes` (default 2 MiB) and
`cache_ttl_secs` (default 3600). Merge with the same precedence as other sections.
`plugins.enabled: false` makes the browser inert.

### T-002 - Store registry and entry model

New `crates/ragent-plugins/src/store_index.rs`. `StoreKind { Codex, Claude }` with a
label; `StoreCatalog` holding the endpoint per kind resolved from config with a compiled
default; `StoreEntry { id, name, version, description, source, dialect, tags, homepage }`
with `serde` defaults for the optional fields (FR-003). A `stores()` accessor returns
both kinds. Reject a non-`https` endpoint at construction (FR-024).

### T-003 - Store-index fetch

`fetch_index(catalog, kind, refresh) -> Result<Vec<StoreEntry>, StoreError>`. Use
`reqwest::blocking` **inside** the off-loop task (T-008), with a request timeout from
`timeout_ms` and a streamed byte counter that aborts past `max_index_bytes`. Parse JSON;
skip entries missing a required field, counting the skips (FR-013). No JavaScript is
executed at any point (FR-023). Optional in-memory cache keyed by kind with TTL, bypassed
by `--refresh` (FR-021). Every failure variant is a `StoreError`, never a panic (FR-025).

### T-004 - `PluginStoreBrowser` state and lifecycle

Add to `crates/ragent-tui/src/app/state.rs`: `plugin_store: Option<PluginStoreBrowser>`
with `kind`, `query`, `all: Vec<StoreEntry>`, `filtered: Vec<usize>`, `cursor`,
`scroll`, `status` (`Loading` / `Ready` / `Failed(String)` / `Empty`), and
`last_install: Option<String>`. `open_plugin_store(app, kind, prefill, refresh)` pushes
the browser and starts the fetch (FR-007); `close_plugin_store` clears it (FR-012).
Initialisers go in `init.rs`; extend `is_input_locked()` so the panel locks the input
field and the queue (FR-015). Pre-fill from a trailing query argument (FR-020).

### T-005 - Filtering, cursor, scrolling

Pure methods on `PluginStoreBrowser`: `set_query` (case-insensitive substring over id,
name, description, tags), `move_up` / `move_down` (clamp within the filtered set),
`ensure_visible` for scrolling, and `apply_query` resetting the cursor to the first
match (FR-008, FR-010). No I/O, no install; opening / typing / moving never touches the
store (FR-022). Empty result set yields the empty state (FR-017).

### T-006 - Panel rendering

`render_plugin_store_panel(frame, app)` in `crates/ragent-tui/src/layout.rs`, modelled
on `render_queue_show_panel`: bordered Magenta block, title with store name and
`n of m`, ASCII-only body, footer key hints. Use ratatui `List` with `highlight_style`
for the full-row block cursor - a per-item pre-styled span does **not** paint the row
background. Installed rows (id in the installed set) use the installed colour and an
`[installed]` marker (FR-005). Render `Loading` / `Failed(reason)` / empty states in the
body (FR-013, FR-017). Wire into `render()` beside the queue modal, storing
`plugin_store_area` and re-deriving on resize (FR-018).

### T-007 - Installed set and key routing

Derive the installed set once per render/refresh from `ragent_plugins::store` (`scan`
over `store_dirs`) and hand it to the renderer (A5, FR-005). In
`crates/ragent-tui/src/input.rs`, add a `plugin_store` block mirroring the
`queue_show_open` block: `Up`/`Down` move, `ENTER` installs, `Backspace` edits, `Esc`
edits-or-dismisses, printable characters append; everything else is swallowed
(FR-015).

### T-008 - Off-loop fetch

Spawn the fetch on the TUI's background-task path (the same pattern used for other
long-running work) and deliver the result back as a message that updates
`plugin_store.status`. The event loop and the agent turn continue animating while the
fetch is in flight (FR-016, FR-026). The panel renders the loading row until the message
arrives.

### T-009 - ENTER install

On `ENTER`, if the highlighted id is already installed, set `last_install` to the
already-installed notice and do nothing (FR-014). Otherwise spawn the install off-loop:
resolve `store_dirs` + config, call `ragent_plugins::add(&dirs, &workdir, &entry.source,
false)`, and on success re-derive the installed set so the row re-colours (FR-006,
FR-011). Map every `AddError` to a report; never panic (FR-025). The store-supplied
source receives no elevated trust: the existing HTTPS / traversal / size guards apply
unchanged (FR-024). No install happens without an explicit `ENTER` (FR-022).

### T-010 - Failure reporting

Formatting helpers for fetch and install failures: an inline panel line for fetch
errors and a message-window report for install errors, using the existing report
conventions. All paths return data, never panic (FR-013, FR-025).

### T-011 - Command surface

Extend `handle_plugins_command` in `crates/ragent-tui/src/app/plugin.rs` (and the
`/plugins` dispatch arm in `slash.rs`) to recognise `codex` and `claude`: parse an
optional trailing query and an optional `--refresh`, then call `open_plugin_store`. Add
both triggers to `SLASH_COMMANDS`, the autocomplete table, and `/plugins help` in
`crates/ragent-plugins/src/help.rs` (FR-002). `plugins.enabled: false` reports the
subsystem is disabled and opens no panel.

### T-012 - Unit tests

Tests for `set_query` matching, cursor clamping and scrolling, query editing on
`Backspace`/`Esc`, and status transitions. Placed in `crates/ragent-tui/tests/` per the
workspace test convention.

### T-013 - Fixtures, acceptance walk, quality gates

Add a fixture store index under `assets/plugins/fixtures/` (entries including one whose
id matches an installed fixture, one with a missing optional field, one malformed) for
the manual walk. Execute the [TESTPLAN.md](TESTPLAN.md) cases. Run `cargo fmt --check`,
`cargo clippy --all-targets`, and `cargo check --workspace --all-targets`; all must be
clean of new warnings.

### T-014 - Compiled default endpoint constants

Add one named `const` per store (`DEFAULT_CODEX_STORE_URL`, `DEFAULT_CLAUDE_STORE_URL`)
in `crates/ragent-plugins/src/store_index.rs`, each a literal `https` store-index URL.
These are the single source of the compiled defaults (FR-027, FR-029, NFR-001). No other
module carries a default-endpoint literal.

### T-015 - Effective endpoint resolution

Add `effective_endpoint(kind, config) -> Url` on the registry: use
`plugins.stores.<name>.url` when present and non-empty, otherwise the compiled default
from T-014 (FR-027, FR-028). Parse and scheme-check the result with the FR-024 guard so
the default path is guarded exactly like the configured path (FR-029). Resolve at each
`open_plugin_store` call so config changes take effect without a rebuild and a missing
`plugins.stores` block never errors (NFR-002, FR-030).

### T-016 - Launch fallback

Update the launch path so `/plugins codex` and `/plugins claude` build the catalog from
the resolved endpoints (T-015) with no dependency on a `plugins.stores` block; a fully
absent block yields the compiled defaults and opens the panel normally (FR-029,
FR-030).

### T-017 - `/plugins stores` report

Add the `stores` subcommand to `handle_plugins_command` and `/plugins help`: list each
store's effective endpoint and tag each as `config` or `default` (FR-031). ASCII-only
output, consistent with the other `/plugins` reports.

### T-018 - Endpoint resolution tests

Tests in `crates/ragent-plugins/tests/` for: configured URL wins; absent block yields
the compiled default; empty-string override is ignored and yields the default; the
default is `https` and passes the FR-024 guard; launch with no `plugins.stores` block
succeeds (FR-027, FR-028, FR-029, FR-030, NFR-001, NFR-002).

### T-019 - Offline fetch seam and default-endpoint fixture

Introduce an injectable fetch seam on the browser/registry so a test can supply the
store-index bytes for a given effective endpoint instead of performing a live request
(FR-037, NFR-003). Add a fixture store index under `assets/plugins/fixtures/` keyed to
each compiled default endpoint (one for Codex, one for Claude) whose entries include at
least one whose id matches an installed fixture and one with an omitted optional field
(FR-032, NFR-004). The seam is production-transparent: the real launch path still uses
`reqwest`, and only tests redirect it to the fixture. The seam must be exercised for
both stores, so neither compiled default is left unverified (NFR-003, NFR-004).

### T-020 - Default-endpoint browser behaviour tests

With the fixture seam in place, assert for both stores that: launching with no
`plugins.stores` block resolves the effective endpoint to the compiled default and
starts a fetch (FR-033); the fetched fixture entries populate `all`/`filtered`;
`set_query` filters them; `move_up`/`move_down` clamp within them; and an `ENTER` action
targets the default-sourced entry's source (FR-034). Also assert that a configured
`plugins.stores.<name>.url` makes the browser target the configured endpoint instead of
the default (FR-035). Tests live in `crates/ragent-tui/tests/` (browser state) and
`crates/ragent-plugins/tests/` (resolution/fetch), per the workspace test convention.

### T-021 - No-config launch and `/plugins stores` tests

Assert that launching `/plugins codex` and `/plugins claude` with a fully absent
`plugins.stores` block emits no configuration error, opens the browse panel, and renders
the fixture results (FR-036). Assert that `/plugins stores` tags each store's effective
endpoint as `default` when no override is configured (FR-038). Both tests must run
offline and without touching the real store or installing anything (NFR-003).

## Risks and Mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Block-cursor background not painting on the highlighted row | Use `List::highlight_style`, exactly as `render_queue_show_panel` does; assert on `buffer[(x,y)].bg` in tests. |
| A blocking fetch stalling the event loop | All fetch/install work runs off-loop and returns via a message (T-008, T-009). |
| Untrusted store data panicking the TUI | Every parse/format path returns `Result`/`Option`; malformed entries are skipped and counted (FR-025). |
| Store-supplied source bypassing install guards | The browser calls the existing `add`, which owns the guards; no new download path is introduced (FR-024). |
| Installed-colour ambiguity | Recorded as Open Question 1; implemented as A7 with an `[installed]` marker. |
| A compiled default endpoint drifting or being duplicated | Declared once as named constants in the registry module (T-014, NFR-001); resolved from config-with-fallback at each launch (T-015, NFR-002). |
| An absent `plugins.stores` block failing launch | Resolution falls back to the compiled default and never errors on absence (T-016, FR-030). |
| Default-endpoint tests depending on live store access | An injectable fetch seam serves a fixture index offline so the tests stay deterministic (T-019, FR-037, NFR-003). |
| Only one store's default being verified | The default-endpoint tests cover both Codex and Claude explicitly (T-019, T-020, NFR-004). |

## Notes on Assumptions

- Install reuses `ragent_plugins::add` verbatim (A4); no archive or download code is
  duplicated.
- The installed set is derived from the store scan on each refresh, not stored twice
  (A5).
- One browser implementation serves both stores, parameterised by `StoreKind` (A2).
- Client-side filtering after a single fetch (A3); `--refresh` re-fetches.
- Block-cursor and modal precedent is `render_queue_show_panel` / the `queue_show_open`
  key block; the browser mirrors that structure for consistency.
- Each store carries a compiled default endpoint so the browsers work out of the box;
  a configured `plugins.stores.<name>.url` overrides it wholesale (FR-027, FR-028).
- The default endpoint is resolved at each launch, so Open Question 2 (whether to ship a
  compiled default) is settled in favour of shipping one; the exact URLs need only be
  fixed before T-014.### T-022 - `/plugins stores --check` availability probe

Add an opt-in `--check` flag to `/plugins stores` on both surfaces. When present, resolve
each store's effective endpoint through the existing config-over-compiled-default
precedence (FR-027, FR-028), fetch it once through the injectable `StoreIndexFetcher`
seam, and append `[ok] available, N plugins` or `[err] unavailable: <reason>` to each
line. The plain report (no `--check`) stays a pure configuration read with no network
request (FR-040). The TUI probe runs off the event loop (spawned, then drained by
`poll_plugin_store_probe_result`) and degrades to the plain report when no async reactor
is present, so headless tests do not block (FR-042). Every failure is contained as
`[err]`, never a panic (FR-041). Tests: `crates/ragent-plugins/tests/test_stores_check.rs`,
`crates/ragent-tui/tests/test_plugin_store_probe.rs`, and a CLI usage case in
`tests/test_plugins_cli_command.rs` (FR-039..FR-042).
### T-023 - `StoreProvider` trait and vendor transforms

Add `store_provider.rs` with a `StoreProvider` trait (`kind`, `parse_index(bytes,
origin)`), `CodexStoreProvider`/`ClaudeStoreProvider`, and a `provider_for(kind)`
selector. Each provider normalises its store's real marketplace shape (`name`-as-id,
optional `version`, `local`/`url`/`git-subdir` source objects, repo-relative source
strings) into the internal `StoreEntry`, resolving repo-relative sources against the
marketplace origin's GitHub repository as installable `git+<repo>#<ref>:<path>` sources
and git sources into `git+<https-url>#<ref>[:<path>]` (FR-043, FR-044). A native index
(any entry carrying an `id`) delegates to the strict parser so existing fixtures are
unaffected. Unresolvable entries are skipped and counted, never fatal (FR-013, FR-025).
Tests: `crates/ragent-plugins/tests/test_store_provider.rs` (inline vendor documents,
NFR-005).

### T-024 - Store-aware fetch seam

Thread `StoreKind` through `StoreIndexFetcher::fetch_index` and route
`NetworkStoreFetcher`, `FixtureStoreFetcher`, `store_fetch::fetch_index`, `probe_stores`
and the TUI fetch path through `provider_for(kind)` (FR-043, FR-044). The native default
fixtures still parse strictly; the offline seam tests are updated to the new signature.

### T-025 - `git+` install source

Extend `add` with a `git+<https-url>#<ref>[:<subpath>]` source: shallow `git clone
--sparse` and, when a subpath is present, `git sparse-checkout set <subpath>` before
committing the result into the store. The ref defaults to the remote default branch when
absent or `HEAD`; a non-`https` remote is refused before cloning (FR-024) and git never
prompts for credentials. Tests:
`crates/ragent-plugins/tests/test_store_git_source.rs` (local `file://` remote, NFR-005).

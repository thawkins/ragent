---
status: draft
audit:
  - { time: 1789986388, from: "none", to: "draft", actor: "system" }
---
# Specification: Plugin Store Browser - Search and Install from the Codex and Claude Plugin Stores

## Overview

This specification extends the existing `/plugins` slash-command family (spec
`plugins`) with a **store browser**: two new subcommands, `/plugins codex` and
`/plugins claude`, that open a full-screen, scrollable browse/search panel for the
corresponding remote plugin store. The panel lists the catalogue entries returned by
the store, lets the user filter them by typing a search query, moves a **block cursor**
over the results with the arrow keys, installs the highlighted entry with `ENTER`, and
dismisses itself with `ESC`. Entries that are already installed are rendered with the
block cursor in a **different colour** from the not-installed rows, so the user can see
at a glance what is already present before pressing `ENTER`.

The feature is a **discovery and install front end**. It performs no new install logic:
`ENTER` on a result resolves to the same source that `/plugins add <source>` accepts and
runs the existing install pipeline (download, extract, manifest validation, store write).
Browse and search never execute plugin JavaScript (carrying forward FR-023 of spec
`plugins`).

### Why this exists

Spec `plugins` gives ragent a way to install a plugin when the user already knows its
path or URL. It gives no way to *find* one. Codex and Claude Code/Desktop each maintain
a public plugin catalogue; today a ragent user must leave the terminal, browse the
catalogue in a web browser, copy a URL, and paste it into `/plugins add`. A store browser
turns that into: type the command, type a few letters, press `ENTER`.

### Worked examples

```text
/plugins codex                 # browse the Codex catalogue
/plugins claude                # browse the Claude catalogue
/plugins codex weather         # open the Codex catalogue with the query pre-filled
/plugins stores                # (optional, FR-022) report the configured store endpoints
```

```text
+-- Codex Plugin Store ---- search: weather ----------- 12 of 340 --+
|  > codex-weather   1.2.0   Weather lookups for the agent          |
|    codex-time      0.9.1   Time and timezone helpers              |
|    codex-shell     2.0.0   [installed] Shell command wrappers     |
|                                                                   |
| Up/Down move  Enter install  Esc close                            |
+-------------------------------------------------------------------+
```

The `>` block cursor marks the highlighted row; an already-installed row is painted in
the installed colour and carries an `[installed]` marker (FR-011, FR-012).

## Assumptions and interpretation (read before implementing)

- **A1 - Catalogue format.** Each store is addressed by an HTTPS URL returning a JSON
  **store index**: an object with a `plugins` array whose entries carry at least
  `id`, `name`, `version`, `description`, and `source` (the install source string that
  `/plugins add` accepts). Optional per-entry fields (`dialect`, `homepage`, `tags`,
  `checksum`) are rendered when present and ignored when absent. This format is fixed
  by this spec so the feature is testable; a future revision may add an adapter per
  vendor.
- **A2 - Two stores, one browser.** The Codex and Claude browsers are the *same* panel
  implementation parameterised by the store endpoint and the dialect label. There is
  one `PluginStoreBrowser` state machine, not two.
- **A3 - Client-side filtering after one fetch.** The panel fetches the store index
  once when it opens and filters it in memory while the user types. It does not issue a
  request per keystroke. A `--refresh` flag (FR-016) forces a re-fetch.
- **A4 - Install reuses the existing pipeline.** `ENTER` maps a result's `source` to a
  call to the existing `ragent_plugins::add` entry point with the same
  traversal/size/HTTPS guards. No new archive or download code is introduced.
- **A5 - Installed state is derived, not stored twice.** A result is "installed" when
  its plugin id is present in the current store scan (`ragent_plugins::store::scan`).
  The browser keeps no separate install registry.
- **A6 - Network is optional and fallible.** If the store endpoint is unreachable the
  panel opens and reports the failure inline; ragent never blocks the event loop on the
  fetch and never panics on a network error (FR-021).
- **A7 - Ambiguity in "installed plugins shown with a block cursor in a different
  colour".** This is read as: the highlighted row always carries a block cursor; when
  the highlighted (or any) row is already installed it is painted in the *installed*
  colour rather than the default result colour, and carries an `[installed]` marker.
  Recorded as Open Question 1 for a reviewer to overturn.

## Definitions

| Term | Meaning |
| ---- | ------- |
| **Store index** | The JSON catalogue document served by a store endpoint (A1). |
| **Store endpoint** | The HTTPS URL of a store index; configurable per store (FR-017). |
| **Result set** | The index entries remaining after the current search query is applied. |
| **Block cursor** | A whole-row highlighted marker (background-filled row) indicating the entry `ENTER` will act on. |
| **Installed colour** | The distinct foreground/background pair used for rows whose plugin id is already in the store. |
| **Browse panel** | The modal overlay opened by `/plugins codex` / `/plugins claude`. |

## Background - existing machinery to reuse

- **`/plugins` family.** `crates/ragent-tui/src/app/plugin.rs`
  (`handle_plugins_command`) parses the subcommand and dispatches through
  `ragent_plugins::run_plugin_subcommand` (`crates/ragent-plugins/src/surface.rs`).
  The browser subcommands are added to this ladder for the *launch* step, then hand off
  to a new modal state machine.
- **Install pipeline.** `ragent_plugins::add(dir, workdir, source, force) ->
  Result<AddOutcome, AddError>` (`crates/ragent-plugins/src/add.rs`) already handles
  local dirs, local archives and HTTPS URLs with traversal and size guards. The browser
  calls it; it does not reimplement it.
- **Store scan.** `ragent_plugins::store` resolves the store directories and `scan`
  returns the installed descriptors; this is the source of the installed set (A5).
- **Modal state precedent.** `queue_show_open` / `queue_show_selected` /
  `queue_show_area` in `crates/ragent-tui/src/app/state.rs`, rendered by
  `render_queue_show_panel` in `crates/ragent-tui/src/layout.rs`, and the key routing in
  `crates/ragent-tui/src/input.rs`, are the working precedent for a modal list with a
  block cursor, `Up`/`Down` movement, `Enter` action and `Esc` dismissal. The ratatui
  `List::highlight_style` block-cursor technique from that panel is reused verbatim
  (a per-item pre-styled span does not paint a full-row background).
- **Async fetch.** `reqwest` (already a dependency of `ragent-plugins`, blocking
  feature enabled) and the existing tokio runtime provide the fetch; the TUI's
  background-task pattern (spawn, poll, deliver a message on completion) keeps the
  event loop responsive (FR-014).
- **Slash registry.** `SLASH_COMMANDS` and the autocomplete table in
  `crates/ragent-tui/src/app/slash.rs` and `crates/ragent-tui/src/app/state.rs`.

## Requirements

### Ubiquitous requirements

**FR-001** - The system shall maintain a registry of plugin stores consisting of the
Codex store and the Claude store, each addressed by a configurable HTTPS store-index URL
(A1), and shall expose both to the `/plugins` command family.

**FR-002** - The system shall provide the `/plugins codex` and `/plugins claude`
subcommands, registered in `SLASH_COMMANDS`, reachable through the slash-command
dispatch arm, listed in the autocomplete menu, and documented in `/plugins help`.

**FR-003** - The system shall fetch and parse a store index into a list of result
entries carrying at minimum the plugin id, display name, version, description, and
install source, and shall tolerate absent optional fields.

**FR-004** - The system shall render the browse panel as a bordered, titled modal
overlay containing a title line with the store name, a search-query field, a scrollable
result list, and a footer listing the available keys, using only ASCII glyphs.

**FR-005** - The system shall render the highlighted result row with a **block cursor**
(a background-filled, full-width row) and shall render every result whose plugin id is
already present in the store scan in a distinct **installed colour** with an
`[installed]` marker.

**FR-006** - The system shall install a selected result by passing its install source to
the existing plugin install entry point with `force = false`, and shall report the
resultant plugin id and dialect in the message window.

### Event-driven requirements

**FR-007** - When `/plugins codex` or `/plugins claude` is invoked, the system shall
open the browse panel for that store, start fetching its store index, and place the
keyboard focus in the search-query field, leaving any in-progress agent turn
undisturbed.

**FR-008** - When the user types a character into the search field, the system shall
apply the query to the in-memory result set (case-insensitive substring match over id,
name, description and tags), shall reset the block cursor to the first surviving result,
and shall update the visible count without issuing a network request.

**FR-009** - When the user presses `Backspace` while the search field is non-empty, or
`Esc` while the search field is non-empty, the system shall remove the last query
character; `Esc` on an empty query shall dismiss the panel.

**FR-010** - When the user presses `Up` or `Down`, the system shall move the block cursor
one result toward the top or bottom within the current result set and shall scroll the
list so the cursor remains visible.

**FR-011** - When the user presses `ENTER` with a result highlighted, the system shall
run the install pipeline for that result's source, shall show progress while it runs,
and on success shall re-derive the installed set so the newly installed entry renders in
the installed colour.

**FR-012** - When the user presses `ESC` on an empty query, the system shall dismiss the
panel, shall restore the previous screen, and shall leave the plugin store and the input
queue unchanged.

**FR-013** - When the store-index fetch fails (network error, non-2xx status, malformed
JSON, or timeout), the system shall render an inline error row naming the cause in the
panel body and shall leave the panel open and dismissible with `ESC`.

**FR-014** - When the user presses `ENTER` on a result already in the installed set, the
system shall not re-run the install and shall report that the plugin is already
installed.

### State-driven requirements

**FR-015** - While the browse panel is open, the system shall route `Up` / `Down` /
`ENTER` / `ESC` / `Backspace` and printable characters to the panel, shall swallow all
other keys, and shall lock the message input field and the input queue.

**FR-016** - While a store-index fetch is in flight, the system shall render a loading
indicator in the panel and shall keep the event loop responsive so the agent turn and
the TUI continue to animate.

**FR-017** - While a result set is empty, whether because the index is empty, the query
matches nothing, or the fetch failed, the system shall render an explicit empty-state or
error line rather than a blank list.

**FR-018** - While the panel is open and the terminal is resized, the system shall
re-derive the panel area so the modal stays centred and fully visible.

### Optional requirements

**FR-019** - Where `plugins.stores.<name>.url` is configured in `ragent.json`, the
system shall use that endpoint for the store instead of the compiled default, and where
it is absent the system shall use the compiled default.

**FR-020** - Where the invocation includes a trailing query (for example
`/plugins codex weather`), the system may open the panel with the search field
pre-filled and the filter already applied.

**FR-021** - Where the invocation includes `--refresh`, the system may bypass any cached
store index and re-fetch from the endpoint.

### Unwanted requirements

**FR-022** - The system shall not install any plugin except in response to an explicit
`ENTER` on a highlighted result; opening the panel, typing a query, and moving the block
cursor shall never install, download, or write to the plugin store.

**FR-023** - The system shall not execute plugin JavaScript at any point during
browsing, searching, or rendering the store index; only the install pipeline may load a
plugin, and then only after the normal enable step.

**FR-024** - The system shall not accept a non-`https` store endpoint, and shall not
pass any source to the install pipeline that the existing pipeline would reject for
traversal, size, or scheme reasons; store-supplied sources receive no elevated trust.

**FR-025** - The system shall never panic or terminate the ragent process because of a
store-index fetch, a malformed index entry, an oversized index, or a failed install;
every such failure shall be contained and reported in the panel or the message window.

**FR-026** - The system shall not block the TUI event loop or the agent turn while
fetching a store index or running an install from the panel.

## Configuration schema

The existing `plugins` block gains a `stores` object:

```jsonc
{
  "plugins": {
    "enabled": true,
    "stores": {
      "codex":  { "url": "https://example.org/codex/index.json" },
      "claude": { "url": "https://example.org/claude/index.json" },
      "cache_ttl_secs": 3600,   // 0 disables the index cache
      "timeout_ms": 10000,      // per-fetch wall-clock budget
      "max_index_bytes": 2097152
    }
  }
}
```

- Merged with the same precedence as the rest of the config (project overrides
  user-global).
- `plugins.enabled: false` makes the store browser inert: `/plugins codex` and
  `/plugins claude` report that the plugin subsystem is disabled and open no panel.

## Store index format

```jsonc
{
  "store": "codex",
  "plugins": [
    {
      "id": "codex-weather",
      "name": "Codex Weather",
      "version": "1.2.0",
      "description": "Weather lookups for the agent",
      "source": "https://example.org/plugins/codex-weather.zip",
      "dialect": "codex",
      "tags": ["weather", "http"],
      "homepage": "https://example.org/codex-weather"
    }
  ]
}
```

`id`, `name`, `version`, and `source` are required; every other field is optional (FR-003).

## Error handling

| Condition | Behaviour |
| --------- | --------- |
| Endpoint unreachable / DNS failure | Inline error row naming the cause; panel stays open (FR-013). |
| Non-2xx HTTP status | Inline error row carrying the status code (FR-013). |
| Index larger than `max_index_bytes` | Fetch aborted; inline error row (FR-025). |
| Malformed JSON | Inline error row with the parse position (FR-013). |
| Entry missing a required field | Entry skipped and counted; the remainder still renders. |
| `ENTER` on an installed result | "already installed" notice; no install (FR-014). |
| Install failure | The `AddError` message is shown in the panel footer and the message window (FR-006, FR-025). |
| Non-`https` configured endpoint | Refused at launch with a usage-style notice (FR-024). |

## Scope

**In scope:** the two browser subcommands; the store registry and index format; the
modal browse/search panel with block cursor and installed colouring; `ENTER` install via
the existing pipeline; `ESC` dismissal; configuration of store endpoints; and the manual
test plan for all of the above.

**Out of scope:** publishing or uploading to a store; a ragent store; authentication or
paid store access; per-vendor catalogue adapters; plugin updates/upgrades; a remote
rating or download-count metric; and any change to the install pipeline's own security
guards.

## Acceptance criteria

1. `/plugins codex` and `/plugins claude` appear in the autocomplete menu and in
   `/plugins help`, and each opens the correct store's browse panel.
2. The panel shows the fetched catalogue entries in a scrollable list; `Up` / `Down`
   move the block cursor and keep it visible.
3. Typing filters the list in memory without a network request; `Backspace` extends the
   matched set back.
4. `ENTER` on a not-installed result installs it through the existing pipeline and
   reports the plugin id; the entry then renders in the installed colour.
5. `ENTER` on an installed result reports "already installed" and performs no write.
6. `ESC` with an empty query dismisses the panel and leaves the store and queue
   unchanged; `ESC` with a non-empty query removes one character.
7. A failed fetch renders an inline error and leaves the panel open and dismissible.
8. Opening the panel, typing, and moving the cursor install nothing and execute no
   plugin code.
9. The panel never blocks the event loop: the agent turn and TUI animation continue
   during fetch and install.
10. No non-ASCII glyphs are used anywhere in the panel.

## Open Questions

1. **Installed-row presentation.** "Installed plugins shown with a block cursor in a
   different colour" admits two readings: (a) installed rows carry a distinct colour and
   the cursor is separate, or (b) the cursor itself changes colour when it sits on an
   installed row. A7 selects (a) with an `[installed]` marker. Confirm.
2. **Default endpoints.** This spec assumes the store endpoints are configured or
   supplied by the build. If ragent should ship a compiled default per store, that URL
   must be fixed before T-002.
3. **Index caching.** FR-021 mentions a cache and the schema exposes `cache_ttl_secs`.
   Confirm a disk cache is wanted now rather than a per-session in-memory cache only.
4. **Search semantics.** FR-008 specifies a case-insensitive substring match over id,
   name, description and tags. Confirm this is sufficient rather than fuzzy ranking.

## Requirements (continued)

### Default store endpoints

**FR-027** - Where no `plugins.stores.<name>.url` is configured for the Codex store or
the Claude store, the system shall use a compiled-in default store-index URL for that
store, so both store browsers work out of the box with no `plugins.stores`
configuration present.

**FR-028** - Where `plugins.stores.<name>.url` is configured and non-empty, the system
shall use that URL as the effective endpoint for that store in place of the compiled
default; the configured value shall win wholesale and the default shall not be merged
with or appended to it.

**FR-029** - The system shall define the compiled default endpoint for each store as an
`https` URL that satisfies the endpoint scheme guard of FR-024, on the default path as
well as on the configured path.

**FR-030** - When `/plugins codex` or `/plugins claude` is invoked and no
`plugins.stores` block is present in the resolved configuration, the system shall open
the browse panel and begin fetching the compiled default endpoint without reporting a
configuration error and without refusing to launch.

**FR-031** - The system shall report, for each store, the effective endpoint and whether
it was sourced from configuration or from the compiled default, through the
`/plugins stores` subcommand, so a user can confirm whether an override is in effect.

### Store availability check

**FR-039** - Where `/plugins stores` is invoked with a `--check` flag, the system shall
additionally attempt to reach each store's effective endpoint and report, per store,
whether that store is available and, when it is, how many plugins it advertises.

**FR-040** - The plain `/plugins stores` report (no `--check`) shall remain a pure
configuration read that makes no network request, so the default report is unchanged and
its tests stay offline and deterministic.

**FR-041** - The `--check` probe shall resolve each store's endpoint through the same
config-over-compiled-default precedence as the plain report (FR-027, FR-028), and a
store that cannot be reached or parsed shall be reported as unavailable with a short,
contained reason rather than failing the whole report or panicking.

**FR-042** - The `--check` probe shall run off the event loop on the TUI surface and
shall route through the injectable store-fetch seam (`StoreIndexFetcher`), so it is
testable offline with a fixture index and no live store access.

### Store providers

**FR-043** - Each store shall have a provider that parses that store's own
marketplace document shape into the internal store-index model, so the browser,
fetch, and install paths stay store-agnostic. The provider shall be reached
through a `StoreProvider` trait (one implementation per store, selected by
`StoreKind`) rather than branching on the store at each call site.

**FR-044** - A provider shall normalise a vendor marketplace entry into the
internal model without requiring the native `id` field: the entry `name` supplies
the id, the `version` is optional (defaulting when absent), and the `source` may
be a repo-relative string or an object (`local`, `url`, or `git-subdir`).
Repo-relative sources shall be resolved against the marketplace document's
repository root **as an installable git source** (`git+<https-repo>#<ref>:<path>`)
when that root is a GitHub repository (`raw.githubusercontent.com` or
`github.com`), since the install pipeline accepts no bare `https` directory URL;
a repo-relative source with no GitHub repository behind its origin, or any other
source that cannot be made installable, shall be counted as a skipped entry
rather than failing the whole index (FR-013, FR-025).

**FR-045** - The compiled default endpoints for the Codex and Claude stores shall
address those stores' own official marketplaces (`openai/plugins` and
`anthropics/claude-plugins-official` respectively), so the default browse and
install paths resolve entries from the real catalogues.

**FR-046** - The install pipeline shall accept a git source of the form
`git+<https-url>#<ref>[:<subpath>]`, installing the whole repository or a single
subdirectory through a shallow, sparse `git clone`. A git source with a non-`https`
remote (other than a local `file://` mirror, which mirrors the accepted local
directory source) shall be refused before any clone is attempted (FR-024).

### Default store browser functional tests

**FR-032** - The system shall provide automated tests that verify the store browser is
functional against each compiled default store endpoint, so that `/plugins codex` and
`/plugins claude` open, fetch, and populate results out of the box with no
`plugins.stores` configuration present.

**FR-033** - When the store browser is launched with no `plugins.stores` block in the
resolved configuration, the test suite shall verify that the effective endpoint for the
requested store equals the compiled default for that store and that a fetch of that
endpoint is started.

**FR-034** - When a store index served from a compiled default endpoint is fetched, the
test suite shall verify that its entries populate the browser's result set and that
search filtering, cursor movement, and the `ENTER`-install action all operate on those
default-sourced entries.

**FR-035** - Where `plugins.stores.<name>.url` is configured and non-empty, the test
suite shall verify that the browser uses the configured endpoint rather than the
compiled default for that store.

**FR-036** - While no `plugins.stores` block is present, the test suite shall verify that
the launch path emits no configuration error and opens the browse panel for both the
Codex and the Claude store.

**FR-037** - The system's default-endpoint browser tests shall not depend on live access
to the real Codex or Claude store endpoints; the default-endpoint behaviour shall be
exercised through an injectable fetch seam backed by a fixture store index.

**FR-038** - When `/plugins stores` is invoked with no `plugins.stores` block present,
the test suite shall verify that the report tags each store's effective endpoint as
sourced from the compiled default.

## Non-Functional Requirements

**NFR-001** - The compiled default store-index URLs shall be declared once as named
constants in the store registry module (`ragent_plugins::store_index`) and shall be the
only location in the codebase where a default endpoint literal appears.

**NFR-002** - The effective endpoint for each store shall be resolved at runtime each
time the store browser is launched, so that adding, changing, or removing
`plugins.stores` takes effect without recompiling and its absence never causes a launch
failure.

**NFR-003** - The default-endpoint browser tests shall run deterministically and offline
within `cargo test`, performing no live network access and no real plugin install, so the
suite is repeatable on any machine and in CI.

**NFR-004** - The default-endpoint browser tests shall cover both the Codex store and the
Claude store, so neither compiled default is left unverified.

**NFR-005** - The store-provider transform and the `git+` install path shall be exercised
offline: provider tests shall use inline marketplace documents captured from the real
vendor schemas, and `git+` install tests shall clone from a local `file://` repository, so
the suite performs no live network access and installs nothing from the real catalogues.

## Plan Tasks (store availability check)

- `T-022` - Add the opt-in `/plugins stores --check` availability probe reporting per-store availability and plugin count (FR-039, FR-040, FR-041, FR-042).

### T-022 - `/plugins stores --check` availability probe

Extend the `/plugins stores` report with an opt-in `--check` flag: on both the TUI
(`/plugins stores --check`) and the CLI (`ragent plugins stores --check`) surface, resolve
each store's effective endpoint through the existing precedence (FR-027, FR-028), fetch
it once through the injectable `StoreIndexFetcher` seam, and append `[ok] available, N
plugins` or `[err] unavailable: <reason>` to each line. The plain report (no `--check`)
stays a pure configuration read with no network request (FR-040); the TUI probe runs
off-loop and is polled on a later frame (FR-042). Covered by offline tests in
`crates/ragent-plugins/tests/test_stores_check.rs` (probe + rendering),
`crates/ragent-tui/tests/test_plugin_store_probe.rs` (off-loop TUI path) and a CLI usage
case in `tests/test_plugins_cli_command.rs` (FR-039..FR-042).

## Plan Tasks (store providers)

- `T-023` - Add the `StoreProvider` trait with per-store Codex and Claude implementations that normalise each vendor marketplace shape into the internal store-entry model (FR-043, FR-044, NFR-005).
- `T-024` - Thread the store kind through the store-fetch seam (`StoreIndexFetcher`), the network and fixture fetchers, `probe_stores`, and the TUI fetch path, so every fetch parses through the store's provider (FR-043, FR-044).
- `T-025` - Accept `git+<https-url>#<ref>[:<subpath>]` sources in the install pipeline via a shallow, sparse `git clone`, refusing non-`https` remotes before cloning (FR-046, NFR-005).

### T-023 - `StoreProvider` trait and vendor transforms

Add `crates/ragent-plugins/src/store_provider.rs`: a `StoreProvider` trait (`kind`,
`parse_index(bytes, origin)`), a `CodexStoreProvider` and `ClaudeStoreProvider`, and a
`provider_for(StoreKind)` selector. Each provider normalises its store's real marketplace
shape: `name` supplies the id, `version` is optional, `description`/`homepage`/`keywords`
are mapped, and `source` may be a repo-relative string or a `local`/`url`/`git-subdir`
object. Repo-relative sources resolve against the marketplace repository root (derived
from the endpoint) into `https` directory URLs; git sources become
`git+<https-url>#<ref>[:<path>]`. A native ragent index (any entry with an `id`) is
delegated to the strict parser, so existing fixtures are unaffected. Unresolvable entries
are counted as skipped, never fatal (FR-013, FR-025). Covered by
`crates/ragent-plugins/tests/test_store_provider.rs` using inline vendor-schema documents
(NFR-005).

### T-024 - Store-aware fetch seam

Change `StoreIndexFetcher::fetch_index` to take the `StoreKind`, and route
`NetworkStoreFetcher`, `FixtureStoreFetcher`, `store_fetch::fetch_index`, `probe_stores`
and the TUI fetch path through `provider_for(kind)`, so a fetch of either store parses
with that store's provider rather than assuming a single catalogue format (FR-043,
FR-044). The native default fixtures still parse strictly (their entries carry ids), and
the existing offline seam tests are updated to the new signature.

### T-025 - `git+` install source

Extend `ragent-plugins::add` with a `git+<https-url>#<ref>[:<subpath>]` source form:
shallow-clone with `--sparse` and, when a subpath is present, `git sparse-checkout set
<subpath>` before committing the resulting directory into the store. The ref defaults to
the remote default branch when absent or `HEAD`. A non-`https` remote is refused before
any clone (FR-024), git never prompts for credentials, and every failure is a contained
`AddError`. Covered by `crates/ragent-plugins/tests/test_store_git_source.rs`, which
clones from a local `file://` repository offline (NFR-005).

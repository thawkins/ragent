---
status: draft
---

# Manual Test Plan: Plugin Store Browser - `/plugins codex` and `/plugins claude`

**Spec:** [SPEC.md](SPEC.md) · **Plan:** [PLAN.md](PLAN.md)

This is a **manual** test plan. Every case is executed by a human in a real terminal
session. Where a case needs a fixture store index, a configured provider, or network
access, the prerequisite is listed before the steps. This document contains no automated
test code, no `#[test]` functions, and no reference to `cargo test`.

The browser is keyboard-driven. Every key press is written explicitly, in order, and the
exact text to type into each field is given as **Test data**. Keys are written as
`Up`, `Down`, `ENTER`, `ESC`, `Backspace`, and literal characters in `backticks`.

The two store endpoints have **compiled-in defaults**, so the browsers work out of the
box with no `plugins.stores` configuration (FR-027, FR-029, FR-030). Most cases below
point the stores at a local fixture index so the content is deterministic; the
default-endpoint cases (TC-013, TC-014, TC-016, TC-017, TC-018) exercise the no-config
and override paths.

## Prerequisites

1. **Build the binary.** Run `cargo build` (debug is sufficient; allow up to 1000
   seconds). Confirm `./target/debug/ragent --version` runs and reports the version.
2. **Configure a model.** Some read-only cases need the TUI but not the model; the
   "event loop stays responsive" checks need an in-progress turn, so configure a
   provider (or a local Ollama model) and confirm a short prompt responds.
3. **Prepare a scratch root.** Create `~/scratch/pluginstores-tests/` and run every TUI
   session from inside it, so the project plugin store is
   `~/scratch/pluginstores-tests/.ragent/plugins/`.
4. **Stage the fixture plugins** (used as install targets and as installed-set members):
   copy the repository fixtures into the scratch root:
   - `~/scratch/pluginstores-tests/fixtures-src/codex-weather/` - Codex dialect
     (`codex-plugin.json`), one tool `get_weather`.
   - `~/scratch/pluginstores-tests/fixtures-src/claude-todo/` - Claude dialect
     (`.claude-plugin/plugin.json`), one tool and one slash command.
   - `~/scratch/pluginstores-tests/fixtures-src/bad-manifest/` - invalid manifest.
   - `~/scratch/pluginstores-tests/fixtures-src/escape-attempt/` - traversal fixture.
   - `~/scratch/pluginstores-tests/fixtures-src/future-api/` - `api_version: 99`.
5. **Stage the fixture store index** used by the browser. Create
   `~/scratch/pluginstores-tests/store/index-codex.json` and
   `~/scratch/pluginstores-tests/store/index-claude.json` with the contents in
   **Test data for the fixture store index** below, and record their URL. Because the
   store endpoint must be `https://` (FR-024), serve the directory over a local HTTPS
   endpoint for the live cases, e.g.
   `python3 -m http.server --directory ~/scratch/pluginstores-tests/store 8443` behind a
   TLS terminator, or any reachable HTTPS host. Cases that only need error and offline
   behaviour use a deliberately bad URL instead.
6. **Install one fixture first**, so the installed-colour path has data: run the TUI and
   `/plugins add ~/scratch/pluginstores-tests/fixtures-src/codex-weather`, then
   `/plugins list` to confirm `codex-weather` is present in the store scan.
7. **Preserve config.** Back up `~/scratch/pluginstores-tests/.ragent/ragent.json` (if it
   exists) before editing it in TC-009, TC-014, and TC-017 so the temporary `stores`
   block can be reverted in Cleanup.
8. **Confirm the compiled defaults are reachable.** For the default-endpoint cases the
   machine must have outbound HTTPS access to the default Codex and Claude store
   endpoints. Record the two endpoints reported by `/plugins stores` (TC-015) before
   starting, so the "no config" run can be compared against them.

### Test data for the fixture store index

`index-codex.json` (served at the `<CODEX_URL>` from Prerequisite 5):

```json
{
  "store": "codex",
  "plugins": [
    { "id": "codex-weather", "name": "Codex Weather", "version": "1.2.0",
      "description": "Weather lookups for the agent",
      "source": "~/scratch/pluginstores-tests/fixtures-src/codex-weather",
      "dialect": "codex", "tags": ["weather", "http"] },
    { "id": "codex-time", "name": "Codex Time", "version": "0.9.1",
      "description": "Time and timezone helpers",
      "source": "~/scratch/pluginstores-tests/fixtures-src/claude-todo", "dialect": "codex" },
    { "id": "codex-shell", "name": "Codex Shell", "version": "2.0.0",
      "description": "Shell command wrappers",
      "source": "~/scratch/pluginstores-tests/fixtures-src/codex-weather", "dialect": "codex" },
    { "id": "codex-nofield", "name": "Missing Version", "description": "No version field",
      "source": "~/scratch/pluginstores-tests/fixtures-src/codex-weather" },
    { "id": "codex-evil", "name": "Traversal Attempt", "version": "0.1.0",
      "description": "Points at a traversal fixture",
      "source": "~/scratch/pluginstores-tests/fixtures-src/escape-attempt" },
    { "id": "codex-future", "name": "Future API", "version": "0.2.0",
      "description": "Entry declares api_version 99",
      "source": "~/scratch/pluginstores-tests/fixtures-src/future-api" }
  ]
}
```

`index-claude.json` (served at the `<CLAUDE_URL>` from Prerequisite 5): the same shape
with `store: "claude"` and entries `claude-todo`, `claude-time`, `claude-nofield`.

`index-bad.json`: one line `{ "store": "codex", "plugins": [` (truncated) for the
malformed-JSON case. `index-huge.json`: a syntactically valid file larger than the
configured `max_index_bytes` for the oversize case.

## Test Cases

### TC-001 - `/plugins codex` and `/plugins claude` are discoverable and open the right panel

**Requirement:** FR-001, FR-002, FR-004, FR-007

**Preconditions:** TUI running from the scratch root; one store pointed at the fixture
index (`<CODEX_URL>`) so the panel has content.

**Steps:**
1. Type `/plugins ` and read the autocomplete menu.
2. Confirm `codex` and `claude` appear as suggestions.
3. Press `ESC`, then type `/plugins help` and press `ENTER`; read the help output.
4. Press `ESC`, then type `/plugins codex` and press `ENTER`.
5. Read the panel border, title line, body, and footer.
6. Press `ESC` to dismiss; type `/plugins claude` and press `ENTER`; read the title.

**Test data:** `/plugins help`, `/plugins codex`, `/plugins claude`.

**Expected results:**
- Step 2 both `codex` and `claude` are listed in the autocomplete menu (FR-002).
- Step 3 `/plugins help` documents `codex`, `claude`, and `stores` (FR-002).
- Step 4 the Codex panel opens; step 6 the Claude panel opens, each with the matching
  store name in the title (FR-001, FR-004).
- The panel is a bordered, titled modal with a search field, a result list, and a footer
  of key hints; only ASCII glyphs are used (FR-004).
- Opening the panel starts the fetch and places focus in the search field without
  disturbing any running agent turn (FR-007).

### TC-002 - Search filters the result set in memory

**Requirement:** FR-008, FR-017

**Preconditions:** `/plugins codex` panel open with the fixture index loaded.

**Steps:**
1. Note the visible count in the title (`n of m`).
2. Type `time` character by character.
3. Read the visible entries and the count after each keystroke.
4. Press `Backspace` once and read the result set.
5. Continue until the query is empty; read the count.

**Test data:** `time`, then `Backspace`.

**Expected results:**
- Step 3 only entries whose id, name, description, or tags contain `time`
  (case-insensitively) remain; the cursor resets to the first survivor; the visible count
  updates (FR-008).
- No network request is issued while typing (the loading row does not reappear).
- Step 4 removing a character widens the matched set back.
- Step 5 an empty query shows the full set again; if nothing matches at any point an
  explicit empty-state line is shown, never a blank list (FR-017).

### TC-003 - Block cursor moves and scrolls with the arrow keys

**Requirement:** FR-010

**Preconditions:** `/plugins codex` panel open with more entries than fit in the body.

**Steps:**
1. Press `Down` repeatedly until the cursor reaches the bottom visible row; continue.
2. Read which row carries the block cursor after each press.
3. Press `Up` repeatedly past the top of the list.

**Test data:** `Down` x N, then `Up` x N.

**Expected results:**
- The block cursor (a background-filled full-width row) moves exactly one entry per
  press (FR-010).
- When the cursor would leave the visible window the list scrolls so the cursor stays
  visible.
- At the top and bottom of the set the cursor clamps and does not wrap.

### TC-004 - Installed entries render in the installed colour with `[installed]`

**Requirement:** FR-005

**Preconditions:** `codex-weather` installed (Prerequisite 6); `/plugins codex` panel open
with the fixture index loaded.

**Steps:**
1. Locate the `codex-weather` row in the list.
2. Read its foreground/background colour and any trailing marker.
3. Move the block cursor onto the `codex-weather` row.
4. Compare with a not-installed row (for example `codex-time`).

**Test data:** none (read-only observation).

**Expected results:**
- The `codex-weather` row is painted in the distinct installed colour and carries an
  `[installed]` marker (FR-005).
- Not-installed rows use the default result colour and no marker.
- The block cursor remains a full-width background fill on whichever row is highlighted,
  whether installed or not (FR-005).

### TC-005 - `ENTER` installs a not-installed entry and it re-colours

**Requirement:** FR-006, FR-011, FR-014

**Preconditions:** `/plugins codex` panel open; `codex-time` not installed; its `source`
points at a valid fixture.

**Steps:**
1. Press `Down` to highlight `codex-time`.
2. Press `ENTER` and watch the panel for a progress indicator.
3. Read the message window after the install completes.
4. Read the `codex-time` row colour and marker.
5. Press `ENTER` again on the now-installed `codex-time` row.
6. Read the message window.

**Test data:** `ENTER` on `codex-time`.

**Expected results:**
- Step 2 progress is shown while the install runs (FR-011).
- Step 3 the message window reports the installed plugin id and dialect (FR-006).
- Step 4 the row now renders in the installed colour with `[installed]` (FR-011).
- Step 5/6 the second `ENTER` reports "already installed" and performs no write
  (FR-014).

### TC-006 - `ESC` dismisses and one keystroke of query editing is undone first

**Requirement:** FR-009, FR-012, FR-015

**Preconditions:** `/plugins codex` panel open.

**Steps:**
1. Type `time`.
2. Press `ESC`; read whether the panel is still open and whether the query changed.
3. Press `ESC` again (query now empty).
4. Read the screen after dismissal.
5. Re-open with `/plugins codex`, type `x`, press `Backspace`, then `ESC`, then `ESC`.

**Test data:** `time`, `ESC`, `Backspace`, `/plugins codex`.

**Expected results:**
- Step 2 `ESC` with a non-empty query removes one character and keeps the panel open
  (FR-009).
- Step 3 `ESC` on an empty query dismisses the panel (FR-009, FR-012).
- Step 4 the previous screen is restored and the plugin store and input queue are
  unchanged (FR-012).
- While the panel was open the message input field and input queue were locked, and only
  `Up`/`Down`/`ENTER`/`ESC`/`Backspace`/printable keys were routed to the panel; any
  other key was swallowed (FR-015).

### TC-007 - Fetch failure renders inline and remains dismissible

**Requirement:** FR-013, FR-017, FR-025

**Preconditions:** the `codex` store pointed at a deliberately unreachable or malformed
URL: first `https://127.0.0.1:9/index.json`, then `<BAD_URL>` serving `index-bad.json`.

**Steps:**
1. Point `plugins.stores.codex.url` at `https://127.0.0.1:9/index.json`, restart, open
   `/plugins codex`; read the panel body.
2. Press `ESC` to dismiss; confirm the panel closes.
3. Point the URL at `<BAD_URL>` (serving `index-bad.json`), restart, open
   `/plugins codex`; read the body.
4. Point the URL at a host serving `index-huge.json`, restart, open `/plugins codex`;
   read the body.

**Test data:** an unreachable HTTPS endpoint; `<BAD_URL>` with a malformed index; a host
serving an oversized index.

**Expected results:**
- Step 1 an inline error row names the network cause; the panel stays open (FR-013).
- Step 2 `ESC` dismisses the erroring panel (FR-013).
- Step 3 the malformed index yields an inline error naming the parse position (FR-013).
- Step 4 the oversize index aborts the fetch and shows an inline error naming the size
  cause (FR-025).
- In every case an explicit error line is shown instead of a blank list (FR-017), and
  the ragent process does not panic or terminate (FR-025).

### TC-008 - In-flight fetch leaves the event loop responsive

**Requirement:** FR-016, FR-026

**Preconditions:** a configured model; a slow or delayed store endpoint so the fetch is
in flight for a few seconds.

**Steps:**
1. Start a prompt in the message window so an agent turn is running.
2. Open `/plugins codex` while the turn is still running.
3. Watch the TUI animation and the status bar during the fetch.
4. Wait for the fetch to resolve; confirm the panel populates.

**Test data:** any slow `<CODEX_URL>`; a short in-progress prompt.

**Expected results:**
- Step 2 the panel opens with a loading indicator while the fetch is in flight
  (FR-016).
- Step 3 the event loop, agent turn, and TUI animation keep running; nothing freezes
  (FR-016, FR-026).
- Step 4 the panel fills once the result arrives.

### TC-009 - Store endpoints are configurable, and a non-`https` endpoint is refused

**Requirement:** FR-019, FR-024

**Preconditions:** `ragent.json` backed up (Prerequisite 7); `<CODEX_URL>` and an
`http://` URL available.

**Steps:**
1. With the `codex` store at `<CODEX_URL>` (https), restart, open `/plugins codex`; read
   the entries.
2. `ESC`, edit `ragent.json` to set `plugins.stores.codex.url` to an `http://...` URL,
   restart, and open `/plugins codex`.
3. Read the panel/message output.
4. Restore `<CODEX_URL>`, restart.

**Test data:** `<CODEX_URL>`; an `http://` (non-TLS) URL.

**Expected results:**
- Step 1 the configured HTTPS endpoint is used and its entries render (FR-019).
- Step 2/3 the non-`https` endpoint is refused at launch with a usage-style notice; no
  fetch is attempted and no panel content is shown for that store (FR-024).

### TC-010 - Opening, typing, and moving install nothing and execute no plugin code

**Requirement:** FR-022, FR-023

**Preconditions:** `/plugins codex` open with the fixture index; `~/scratch/pluginstores-tests/.ragent/plugins/`
contents noted before starting.

**Steps:**
1. Note the contents of the plugin store directory.
2. Open the panel, type a query, move the cursor up and down several times.
3. Press `ESC` to dismiss.
4. Note the store directory contents again and read the log for any plugin execution.

**Test data:** `/plugins codex`, several characters, `Up`/`Down`.

**Expected results:**
- Step 4 the plugin store directory is unchanged: nothing was installed or written
  (FR-022).
- No plugin JavaScript was executed during browsing, searching, or rendering (FR-023);
  the log shows no plugin load during the case.

### TC-011 - `--refresh` and pre-filled query on launch

**Requirement:** FR-020, FR-021

**Preconditions:** `/plugins codex` opened at least once so a cached index exists (if the
cache is enabled); `<CODEX_URL>` serves a changed index.

**Steps:**
1. Open `/plugins codex` and note the entries.
2. Change `index-codex.json` on the host (for example alter a description).
3. `ESC`, then open `/plugins codex` again; note whether the change appears.
4. `ESC`, then open `/plugins codex --refresh`; read the entries.
5. Open `/plugins codex weather` and read the search field and result set.

**Test data:** `/plugins codex`, `/plugins codex --refresh`, `/plugins codex weather`.

**Expected results:**
- Step 3 the cached index may be used, so the change may not appear.
- Step 4 `--refresh` bypasses the cache and re-fetches, so the change appears (FR-021).
- Step 5 the search field is pre-filled with `weather` and the filter is already applied
  on open (FR-020).

### TC-012 - `plugins.enabled: false` makes the browser inert

**Requirement:** FR-019

**Preconditions:** `ragent.json` editable; `plugins.enabled` currently `true`.

**Steps:**
1. Set `plugins.enabled` to `false`, restart, and open `/plugins codex`.
2. Read the message window.
3. Repeat for `/plugins claude`.

**Test data:** `plugins.enabled: false`; `/plugins codex`, `/plugins claude`.

**Expected results:**
- Step 1 no browse panel opens.
- Step 2 the message window reports that the plugin subsystem is disabled.
- Step 3 the Claude command behaves the same way.

### TC-013 - Stores work out of the box with no `plugins.stores` block

**Requirement:** FR-027, FR-029, FR-030, NFR-001, NFR-002

**Preconditions:** `~/scratch/pluginstores-tests/.ragent/ragent.json` contains **no**
`plugins.stores` block (or no `stores` key at all); outbound HTTPS access is available
(Prerequisite 8); the default endpoints reported by `/plugins stores` (TC-015) are
recorded.

**Steps:**
1. Confirm the config has no `plugins.stores` key: open `ragent.json` and inspect the
   `plugins` block.
2. Start the TUI from `~/scratch/pluginstores-tests/`.
3. Type `/plugins codex` and press `ENTER`.
4. Wait for the loading row to resolve; read the panel title and body.
5. Press `ESC`; type `/plugins claude` and press `ENTER`; read the panel.
6. Press `ESC` to dismiss.
7. Read the message window and the log for any configuration error.

**Test data:** config with the `plugins` block present but no `stores` key;
`/plugins codex`, `/plugins claude`.

**Expected results:**
- Step 4 the Codex panel opens and shows a loading row, then either the default store's
  entries or an inline network error (a network failure of the default endpoint is
  still a valid result, provided it is reported inline and the panel stays open).
- Step 5 the Claude panel behaves the same way.
- Step 7 no "missing endpoint", "not configured", or configuration error is reported;
  the browsers launch without requiring any `plugins.stores` configuration (FR-030).
- The fetch goes to the compiled default endpoint, not to a configured URL (FR-027),
  and that default is an `https` URL that passes the scheme guard (FR-029, NFR-001).

### TC-014 - A configured URL overrides the compiled default wholesale

**Requirement:** FR-027, FR-028, FR-029, NFR-002

**Preconditions:** `ragent.json` backed up (Prerequisite 7); both the default endpoints
and the fixture URLs (`<CODEX_URL>`, `<CLAUDE_URL>`) are reachable; the fixture index is
served.

**Steps:**
1. With no `stores` block, run `/plugins stores` and record the default endpoint for
   `codex`.
2. Open `/plugins codex` and note the entries (default store) or the inline error.
3. `ESC`, then edit `ragent.json` to add
   `"stores": { "codex": { "url": "<CODEX_URL>" } }` (no change to the default itself).
4. Restart the TUI (a restart, **not** a rebuild).
5. Run `/plugins stores` and confirm `codex` now reports `<CODEX_URL>` tagged as
   `config`.
6. Open `/plugins codex` and confirm the entries come from `<CODEX_URL>` (the fixture
   index), not the default store.
7. `ESC`, then set `plugins.stores.codex.url` to an empty string `""` and restart.
8. Run `/plugins stores` and open `/plugins codex`; read both.

**Test data:** the `stores` block with `<CODEX_URL>`; then an empty-string `url`.

**Expected results:**
- Step 3/4 the change takes effect after a restart with no recompile (NFR-002).
- Step 5 the report names `<CODEX_URL>` and tags it `config`; the default is not merged
  with or appended to it (FR-028).
- Step 6 the panel shows the fixture entries, confirming the configured URL won wholesale.
- Step 8 the empty-string override is ignored and the compiled default is used again
  (tagged `default`), and the panel still launches (FR-027, FR-030).

### TC-015 - `/plugins stores` reports the effective endpoint and its source

**Requirement:** FR-031, NFR-001, NFR-002

**Preconditions:** TUI running; `ragent.json` currently has exactly one store overridden
(for example `claude`) and the other left to the default.

**Steps:**
1. Type `/plugins stores` and press `ENTER`.
2. Read the output block.
3. Confirm it lists an entry for `codex` and an entry for `claude`.
4. For each, read the effective endpoint URL and whether it is tagged `config` or
   `default`.
5. Remove the `plugins.stores` block from `ragent.json`, restart the TUI, and repeat
   steps 1-4.
6. Confirm the output uses only ASCII characters.

**Test data:** `/plugins stores` with one override present; `/plugins stores` with no
`stores` block.

**Expected results:**
- Step 3 both stores are listed, even when only one is configured.
- Step 4 the overridden store is tagged `config` and names its configured URL; the
  other is tagged `default` and names its compiled default URL.
- Step 5 with no `stores` block both stores are tagged `default` and still report a
  non-empty `https` endpoint, so the user can confirm the out-of-the-box defaults
  (FR-027, FR-031).
- The endpoint shown passes the `https` requirement in every case (FR-029).
- The output is plain ASCII with no box-drawing or emoji glyphs.

### TC-016 - Default-endpoint browsers are functional for both stores (no `plugins.stores`)

**Requirement:** FR-032, FR-033, FR-034, FR-036, NFR-004

**Preconditions:** `ragent.json` has **no** `plugins.stores` block; the fixture store
index served at a local HTTPS host is reachable (Prerequisite 5); a temporary config is
prepared so that the compiled default for one store is pointed at the fixture for the
duration of this case (the default is only *observed*, never edited in the real config).

**Steps:**
1. Confirm the config contains no `plugins.stores` key.
2. Start the TUI from `~/scratch/pluginstores-tests/`.
3. Type `/plugins codex` and press `ENTER`; wait for the panel to resolve.
4. Read the panel title and body; note at least one entry and its id.
5. Type two or three characters of a query and press `ENTER` to install the highlighted
   default-sourced entry; read the message window.
6. `ESC` on an empty query, then type `/plugins claude` and press `ENTER`; repeat steps
   3-4 for the Claude store.
7. Read the message window and the log for any configuration error on either launch.

**Test data:** config with the `plugins` block but no `stores` key; `/plugins codex`,
`/plugins claude`; a query matching at least one default-sourced entry.

**Expected results:**
- Step 3/6 both panels open with the compiled default endpoint (FR-033) and no
  "endpoint not configured" error (FR-036).
- Step 4 the fetched default-endpoint entries populate the result list, and the visible
  count reflects the filtered set (FR-034).
- Step 5 the query filters the default-sourced entries in memory, the block cursor moves
  within them, and `ENTER` installs the highlighted one through the normal pipeline
  (FR-034).
- Both the Codex and the Claude default are exercised, so neither is left unverified
  (NFR-004).
- The behaviour is repeatable: repeating the case in the same session yields the same
  result (no dependence on the previous run's cache or install).

### TC-017 - A configured URL takes precedence over the default in the browser

**Requirement:** FR-035

**Preconditions:** `ragent.json` backed up (Prerequisite 7); the fixture index served at
`<CODEX_URL>` is reachable; the default endpoint for `codex` is recorded.

**Steps:**
1. With no `stores` block, open `/plugins codex` and note the entries (default store).
2. `ESC`, then add `"stores": { "codex": { "url": "<CODEX_URL>" } }` to `ragent.json`.
3. Restart the TUI (restart, **not** rebuild).
4. Open `/plugins codex` and compare the entries with the fixture index.
5. Run `/plugins stores` and read the `codex` row.

**Test data:** the `stores` block with `<CODEX_URL>`; the fixture index contents.

**Expected results:**
- Step 4 the panel shows the fixture entries, not the default store's entries, so the
  configured URL is the endpoint the browser used (FR-035).
- Step 5 the `codex` row names `<CODEX_URL>` and is tagged `config`.
- The default is not merged with, or appended to, the configured URL (the entry list is
  exactly the fixture's).

### TC-018 - No-config launch and `/plugins stores` default tagging are error-free

**Requirement:** FR-036, FR-038, NFR-003

**Preconditions:** `ragent.json` has no `plugins.stores` block; TUI running.

**Steps:**
1. Read the startup log for any configuration or endpoint warnings.
2. Type `/plugins codex` and press `ENTER`; confirm the panel opens.
3. `ESC`; type `/plugins claude` and press `ENTER`; confirm the panel opens.
4. `ESC`; type `/plugins stores` and press `ENTER`; read each store's row.
5. Confirm the whole run performed no real plugin install and touched no store files
   unless `ENTER` was pressed on a highlighted result in step 2 or 3.

**Test data:** `/plugins codex`, `/plugins claude`, `/plugins stores` with no `stores`
block.

**Expected results:**
- Step 1 no "missing endpoint" or "not configured" error appears (FR-036).
- Step 2/3 both panels open without any `plugins.stores` configuration (FR-036).
- Step 4 both stores are tagged `default`, confirming the report attributes a
  default-sourced endpoint correctly (FR-038).
- The default-endpoint behaviour is exercised without live access to the real Codex or
  Claude store and without installing anything (NFR-003).

### TC-019 - Vendor marketplace documents normalise into browsable entries

**Requirement:** FR-043, FR-044, NFR-005

**Preconditions:** the automated suite is run offline; no store is reached.

**Steps:**
1. Run `cargo test -p ragent-plugins --test test_store_provider`.
2. Read the assertions on the inline Codex and Claude marketplace documents.
3. Run `cargo test -p ragent-plugins --test test_store_seam`.

**Expected results:**
- The Codex provider normalises a `local` path source into an installable
  `git+<repo>#<ref>:<path>` source under the `openai/plugins` repository root, a `url`
  source into `git+<https-url>#HEAD`, and a `git-subdir` source into
  `git+<https-url>#<ref>:<path>` (FR-044).
- The Claude provider normalises a repo-relative string source against the
  `claude-plugins-official` root as a `git+<repo>#<ref>:<path>` source and maps
  `git-subdir` sources the same way (FR-044).
- A repo-relative source is only anchored when the origin is a GitHub repository
  (`raw.githubusercontent.com` or `github.com`); a `tree/<ref>` view segment supplies the
  ref, and an origin with no GitHub repository behind it skips the entry (FR-044).
- Entries with an unusable source (ftp, empty, ssh, non-GitHub origin) are skipped and
  counted, and the document still parses (FR-013, FR-025).
- A native index (entries carrying `id`) is parsed strictly, so existing fixtures are
  unchanged (FR-044).
- The fixture seam routes vendor-shaped fixture bytes through the store's provider, while
  the bundled native default fixtures still populate both stores (FR-032, NFR-004).
- No test contacts the network (NFR-005).

### TC-020 - `git+` install source clones and installs a subdirectory

**Requirement:** FR-046, NFR-005

**Preconditions:** `git` is on `PATH`; the automated suite runs offline.

**Steps:**
1. Run `cargo test -p ragent-plugins --test test_store_git_source`.
2. Read the end-to-end install assertions against a local `file://` remote.

**Expected results:**
- A `git+<file-url>#HEAD:plugins/weather` source installs the plugin from that
  subdirectory into the store (FR-046).
- A `git+<file-url>#HEAD` source installs the plugin at the repository root (FR-046).
- A `git+http://...` source is refused as `UnknownSource` before any clone, and the store
  is left untouched (FR-024).
- No test clones a real marketplace repository and no network is contacted (NFR-005).

## Cleanup

1. Remove the scratch tree: `rm -rf ~/scratch/pluginstores-tests/`.
2. Restore `~/scratch/pluginstores-tests/.ragent/ragent.json` if it was backed up before
   TC-009, TC-014, or TC-017; if the whole scratch tree is removed this is unnecessary.
3. Confirm no `plugins.stores` block remains in any real project config used during
   testing.
4. Stop any local HTTPS serving process started in Prerequisite 5.
5. Confirm `~/scratch/pluginstores-tests/` (and its `.ragent/plugins/`) no longer exists.

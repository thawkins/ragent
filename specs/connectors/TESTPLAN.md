---
status: draft
---

# Manual Test Plan: Connector System - `/connectors` Slash-Command Family and MCP-Backed Connectors

**Spec:** [SPEC.md](SPEC.md) · **Plan:** [PLAN.md](PLAN.md)

This is a **manual** test plan. Every case is executed by a human in a real terminal
session. Where a case needs a fixture connector, a configured LLM provider, a credential,
or network access, the prerequisite is listed before the steps. This document contains no
automated test code.

## Prerequisites

1. **Build the binary.** Run `cargo build` (debug is sufficient; allow up to 1000 seconds).
   Confirm `./target/debug/ragent --version` runs and reports the current version.
2. **Configure an LLM provider.** Cases that invoke a connector tool from the agent loop
   need a model. Set the API key for your configured provider in the environment, or point
   ragent at a local Ollama model. Confirm a plain `ragent run "say hello"` returns a
   response.
3. **Prepare a scratch root.** Create `~/scratch/connectors-tests/` and run every TUI
   session from inside it, so the project connector store is
   `~/scratch/connectors-tests/.ragent/connectors/`.
4. **Stage the fixture connectors** as *add sources* (do not pre-install them). Each fixture
   is a directory containing a `connector.json` manifest. The canonical artifacts are the
   repository fixtures under `assets/connectors/fixtures/`; copy each directory into
   `~/scratch/connectors-tests/fixtures-src/` rather than authoring it by hand, so the walk
   exercises the same bytes the automated walk (`test_connector_fixtures.rs`) uses:
   - `~/scratch/connectors-tests/fixtures-src/echo/` - one stdio server whose entry point is
     a small local MCP server (see step 5) exposing one tool `echo` that returns its input.
   - `~/scratch/connectors-tests/fixtures-src/two-server/` - two stdio servers, `alpha` and
     `beta`, each exposing one tool.
   - `~/scratch/connectors-tests/fixtures-src/needs-token/` - one stdio server plus an auth
     shape `token` and credential name `NEEDS_TOKEN_VALUE`.
   - `~/scratch/connectors-tests/fixtures-src/needs-env/` - one stdio server plus an auth
     shape `env` requiring the environment variable `ECHO_TOKEN`.
   - `~/scratch/connectors-tests/fixtures-src/bad-server/` - one stdio server whose command
     does not exist, so the connect fails.
   - `~/scratch/connectors-tests/fixtures-src/unsupported/` - a directory whose manifest
     declares a transport ragent does not speak (for example `"type": "grpc"`).
   - `~/scratch/connectors-tests/fixtures-src/duplicate/` - a directory whose manifest uses
     the id `echo`.
5. **Provide the local MCP server** the `echo` fixture launches. The staged fixtures ship
   the placeholder command `mcp-server-filesystem`; replace it in each staged manifest with
   the absolute path of any installed MCP server executable (or the
   `mcp-server-filesystem` binary found by `/connectors stores` discovery). The
   `bad-server` fixture intentionally keeps a command that does not resolve. Record the
   absolute path for reuse.
6. **Prepare a fixture catalogue.** Copy the repository fixtures
   `assets/connectors/fixtures/stores/index.json` (three entries: `echo`, `unsupported`,
   `github`), `index-bad.json` (truncated), and `index-big.json` (oversize) into
   `~/scratch/connectors-tests/catalogue/`. Serve `index.json` over HTTPS (the connector
   system refuses non-`https`), or, if you cannot produce an HTTPS endpoint, keep it local
   and skip the catalogue cases (`TC-005`, `TC-006`), recording them as not run.
7. **Prepare a packaged fixture.** Copy `assets/connectors/fixtures/archives/echo.zip` and
   `assets/connectors/fixtures/archives/needs-token.tar.gz` into
   `~/scratch/connectors-tests/fixtures-src/`.
8. **Prepare a poisoned archive.** Copy `assets/connectors/fixtures/archives/escape.zip`
   (one entry whose path is `../../escape.json`) into
   `~/scratch/connectors-tests/fixtures-src/` to exercise archive path-traversal
   protection.
9. **Preserve config.** Back up `~/scratch/connectors-tests/.ragent/ragent.json` (if it
   exists) and `<state dir>/mcp_state.json` before starting, so temporary flags and ledger
   entries can be reverted in Cleanup.
10. **Record the compiled default endpoint.** Before any config edit, note the endpoint the
    Claude catalogue uses out of the box (shown by `/connectors stores` in TC-019). Cases
    TC-019 to TC-022 compare against this value.
11. **Confirm network reachability.** Cases that check `--check` need outbound HTTPS to the
    default endpoint; if the machine is offline, run those steps with `--check` omitted and
    record the reachability step as not run, but still verify the endpoint and its provenance
    tag.
12. **Confirm the fixture categories.** The staged fixtures already carry distinct
    `category` values so the category filter has something to separate: `echo` and the
    catalogue `echo` entry use `productivity`; `two-server`, `bad-server`, `unsupported`,
    and the catalogue `github` entry use `developer`; `needs-token`, `needs-env`, and the
    `duplicate` fixture use `data`. Cases TC-023 and TC-024 filter on these values and on
    the `ALL` sentinel.

## Test Cases

### TC-001 - `/connectors help` and usage fallbacks

**Requirement:** FR-004, FR-006, FR-017

**Preconditions:** ragent TUI running from `~/scratch/connectors-tests/`.

**Steps:**
1. Type `/connectors ` (with a trailing space) into the message input and observe the
   autocomplete menu.
2. Press `Down` repeatedly to scroll the menu and confirm `list`, `claude`, `add`, `remove`,
   `enable`, `disable`, `connect`, `disconnect`, `auth`, `test`, `stores`, and `help` appear;
   press `Esc` to dismiss the menu.
3. Type `/connectors help` and press `Enter`; read the usage block.
4. Type `/connectors` and press `Enter`; read the usage block.
5. Type `/connectors bogus` and press `Enter`; read the usage block.

**Test data:** the literal strings `/connectors help`, `/connectors`, `/connectors bogus`.

**Expected results:**
- All twelve subcommands appear in the autocomplete menu and in the usage block.
- The usage block documents `list` (with `--verbose` and `--category <name>`),
  `claude [query] [--category <name>] [--refresh]`, `add <id|source> [--force]`, `remove <id>`,
  `enable <id>`, `disable <id>`, `connect <id>`, `disconnect <id>`, `auth <id>`,
  `test <id>`, and `stores` (with `--check`), plus the accepted `<source>` forms (catalogue
  id, local directory, local zip/tar.gz, https URL).
- All three invocations print the same usage block, prefixed `From: /connectors ...`.
- A second terminal shows `ls -la ~/scratch/connectors-tests/` unchanged - no files or
  directories created.
- The output contains no non-ASCII characters.

### TC-002 - Add a connector from a local directory (installed enabled)

**Requirement:** FR-002, FR-011, FR-018, FR-027

**Preconditions:** TUI running; `fixtures-src/echo/` staged and its `command` set to the
real MCP server path; the project connector store empty.

**Steps:**
1. Note the current time. Leave the MCP connection count column empty; this is a
   no-connection check.
2. Type `/connectors add ~/scratch/connectors-tests/fixtures-src/echo` and press `Enter`.
3. Approve any permission prompt shown.
4. Read the report line in the message window.
5. In a second terminal, run
   `ls -R ~/scratch/connectors-tests/.ragent/connectors/` and open the installed manifest
   with your editor.
6. Back in the TUI, type `/connectors list` and press `Enter`.
7. Press `Alt+M` to open the MCP panel (or type `/mcp` and press `Enter`) and confirm no
   `echo.` server is listed.

**Test data:** source path `~/scratch/connectors-tests/fixtures-src/echo`.

**Expected results:**
- The report names the connector id `echo` and states it was installed and is **enabled**.
- The store directory now contains the connector manifest; the manifest records the MCP
  server definition but **no** secret value.
- `/connectors list` shows one row for `echo` with state `enabled`, an auth state of
  `none`, and a server count of `1`.
- The MCP panel shows no `echo.*` server: no connection was made at install time.

### TC-003 - Add a connector from a packaged archive

**Requirement:** FR-011, FR-023, FR-027

**Preconditions:** TUI running; `fixtures-src/echo.zip` and `fixtures-src/needs-token.tar.gz`
staged.

**Steps:**
1. Type `/connectors add ~/scratch/connectors-tests/fixtures-src/echo.zip --force` and press
   `Enter`.
2. Read the report line.
3. Type `/connectors add ~/scratch/connectors-tests/fixtures-src/needs-token.tar.gz` and
   press `Enter`.
4. Read the report line.
5. Type `/connectors list` and press `Enter`.

**Test data:** the two archive paths above.

**Expected results:**
- The zip install reports id `echo` installed and enabled, re-using the id even though a
  directory install already used it because `--force` was given.
- The tar.gz install reports id `needs-token` installed and enabled, and surfaces its
  credential requirement `NEEDS_TOKEN_VALUE` (FR-023).
- `/connectors list` shows both connectors with state `enabled` (no MCP connection is
  made at install time, so both list as `enabled`, not `connected`).

### TC-004 - Refusal cases for `/connectors add`

**Requirement:** FR-027, FR-028, FR-029

**Preconditions:** TUI running; `fixtures-src/duplicate/` and `escape.zip` staged; the
`echo` connector installed from TC-002.

**Steps:**
1. Type `/connectors add ~/scratch/connectors-tests/fixtures-src/duplicate` and press
   `Enter`; read the report.
2. Type `/connectors add http://example.org/connectors.zip` and press `Enter`; read the
   report.
3. Type `/connectors add ~/scratch/connectors-tests/fixtures-src/escape.zip` and press
   `Enter`; read the report.
4. In a second terminal, run `ls ~/scratch/connectors-tests/` and
   `ls -a ~/scratch/connectors-tests/..` to check nothing escaped the store.

**Test data:** the duplicate directory path, the `http://` URL, and the escape archive path.

**Expected results:**
- Step 1 is refused with an id-collision message naming `echo`; no file changes.
- Step 2 is refused with a message naming the rejected `http` scheme.
- Step 3 is refused with a message naming the rejected archive entry `../../escape.json`; no
  file is written outside `~/scratch/connectors-tests/.ragent/connectors/`.
- Step 4 finds no new file outside the connector store.

### TC-005 - Catalogue browsing (`claude`) and add-by-id

**Requirement:** FR-010, FR-011, FR-024, FR-025, FR-031

**Preconditions:** TUI running; the fixture catalogue served over HTTPS and its URL set in
`connectors.stores.community.url` in `ragent.json`; TUI restarted after the config edit.

**Steps:**
1. Type `/connectors claude echo` and press `Enter`; read the panel rows, then `Esc` to close.
2. Type `/connectors claude zzzznomatch` and press `Enter`; read the empty-state line, then
   `Esc` to close.
3. Type `/connectors add echo --force` and press `Enter`; read the report.
4. Type `/connectors list --verbose` and press `Enter`; read the rows.
5. Stop the HTTPS endpoint, restart the TUI, and type `/connectors claude echo` and press
   `Enter`; read the panel's failure line.

**Test data:** the queries `echo` and `zzzznomatch`; the connector id `echo`.

**Expected results:**
- Step 1 lists the catalogue `echo` row with id, category, and description.
- Step 2 prints a "no matching connectors" line, not an error.
- Step 3 installs `echo` and reports the unsupported `unsupported` catalogue entry was
  skipped (a skipped-entry count is shown).
- Step 4 shows the `unsupported` entry's capability label if it was retained locally
  (FR-025); otherwise it shows the skipped count only.
- Step 5 (endpoint down) reports a fetch failure with a reason and leaves the cached
  catalogue and the store untouched (FR-031).

### TC-006 - Malformed and oversize catalogue documents

**Requirement:** FR-024, FR-031

**Preconditions:** TUI running; a second fixture catalogue file that is truncated JSON
(`index-bad.json`) and one larger than `connectors.max_index_bytes` (`index-big.json`)
prepared; the TUI pointed at each in turn via `connectors.stores.community.url`.

**Steps:**
1. Set `connectors.stores.community.url` to the truncated catalogue, restart the TUI, type
   `/connectors claude echo` and press `Enter`; read the panel's failure line, then `Esc`.
2. Set the URL to the oversize catalogue, restart the TUI, repeat the browse; read the
   panel's failure line.
3. In a second terminal, check the on-disk catalogue cache directory is unchanged in size
   and mtime.

**Test data:** the two catalogue URLs; the query `echo`.

**Expected results:**
- Step 1 reports a malformed-JSON failure naming the parse cause.
- Step 2 reports an oversize-body failure naming `max_index_bytes`.
- Step 3 shows the previously cached catalogue untouched; the subsystem did not silently
  replace it with a bad document.

### TC-007 - Enable a connector and invoke its tool from the agent loop

**Requirement:** FR-003, FR-008, FR-012, FR-020, FR-026

**Preconditions:** TUI running; `echo` and `two-server` installed (TC-002, add `two-server`
the same way); a configured LLM provider.

**Steps:**
1. Type `/connectors enable echo` and press `Enter`; read the report.
2. Type `/tools` and press `Enter`; scan the list for the bridged tool name.
3. Type `/connectors enable two-server` and press `Enter`; read the report.
4. Type `/mcp` and press `Enter`; confirm both `two-server.alpha` and `two-server.beta`
   appear with their tool counts.
5. Type the prompt `Use the echo tool to echo the text "connector-ok" and show me the
   result` and press `Enter`.
6. Approve the tool-permission prompt when it appears.
7. Read the assistant's reply and the tool-call entry in the log panel.

**Test data:** the prompt text in step 5; the subcommand arguments `echo` and `two-server`.

**Expected results:**
- Step 1 reports `echo` enabled and connected, with a count of tools the server exposed.
- Step 2 lists a tool named `mcp_echo.echo_echo` (or the equivalent
  `mcp_<bridged-server>_<tool>` name).
- Step 3 reports `two-server` enabled; step 4 shows both `alpha` and `beta` connected, each
  with its own tool count (FR-026).
- Step 7 shows the tool returned `connector-ok`, proving the tool was invoked through the
  normal path.

### TC-008 - Disable a connector mid-session and verify deregistration

**Requirement:** FR-013, FR-018, FR-019

**Preconditions:** TUI running; `echo` enabled from TC-007; its tool visible in `/tools`.

**Steps:**
1. Type `/connectors disable echo` and press `Enter`; read the report.
2. Type `/tools` and press `Enter`; search for the `echo` tool.
3. Type `/mcp` and press `Enter`; check for an `echo.*` server.
4. Type `/connectors list` and press `Enter`; read the `echo` row.
5. Type `/connectors connect echo` and press `Enter`; read the report.
6. Type `/connectors enable echo` and press `Enter`, then `/connectors disconnect echo` and
   press `Enter`; read both reports.
7. Type `/tools` and press `Enter`; confirm the `echo` tool is still listed.

**Test data:** the subcommand argument `echo`.

**Expected results:**
- Step 1 reports `echo` disabled and states how many servers and tools were disconnected
  (FR-013).
- Steps 2 and 3 show the `echo` tool and the `echo.*` server gone.
- Step 4 shows the `echo` row with state `disabled` and its server count still present
  (FR-018).
- Step 5 is refused or reports the connector is disabled (a disabled connector cannot be
  connected without being enabled).
- Step 6's `enable` connects the server; the following `disconnect` drops the connection
  while leaving the connector enabled (FR-019).
- Step 7 shows the `echo` tool still listed after the `disconnect` (connector remains
  enabled).

### TC-009 - Authentication: token shape and `needs auth` reporting

**Requirement:** FR-005, FR-014, FR-022, FR-023, FR-032

**Preconditions:** TUI running; `needs-token` installed and enabled but not connected (TC-003).

**Steps:**
1. Type `/connectors list` and press `Enter`; read the `needs-token` row.
2. Type `/connectors enable needs-token` and press `Enter`; read the report.
3. Type `/connectors auth needs-token` and press `Enter`; observe the prompt dialog.
4. When the dialog asks for the token value, type `test-token-12345` into the field and
   press `Enter`.
5. Read the report line.
6. In a second terminal, open the encrypted credential store file for the current user and
   confirm `test-token-12345` does **not** appear in plaintext anywhere under
   `~/scratch/connectors-tests/` or in `ragent.json`.
7. Type `/connectors list` and press `Enter`; read the `needs-token` row again.
8. Type `/connectors enable needs-token` and press `Enter`; read the report.

**Test data:** the token value `test-token-12345` typed into the auth dialog field.

**Expected results:**
- Step 1 shows the `needs-token` row with auth state `needs auth`.
- Step 2 refuses to connect and reports `needs auth` and the authentication guidance
  (FR-022).
- Step 3 opens an auth prompt dialog naming the credential `NEEDS_TOKEN_VALUE`.
- Step 5 reports the token stored and the resulting auth state **without echoing the token
  value** (FR-014).
- Step 6 finds no plaintext secret in the project tree.
- Step 7 shows the auth state now satisfied (for example `authenticated` or `token set`).
- Step 8 either connects the server or reports the server's own failure; it does not loop
  retrying (FR-032).

### TC-010 - A failing server records `errored` and leaves the session stable

**Requirement:** FR-016, FR-026, FR-032

**Preconditions:** TUI running; `fixtures-src/bad-server/` installed (add it as in TC-002).

**Steps:**
1. Type `/connectors enable bad-server` and press `Enter`; read the report.
2. Wait 10 seconds and confirm the TUI is still responsive: press `Ctrl+C` twice to open the
   exit confirmation and press `Esc` to cancel it.
3. Type `/connectors list` and press `Enter`; read the `bad-server` row.
4. Type `/connectors test bad-server` and press `Enter`; read the per-step results.

**Test data:** the subcommand argument `bad-server`.

**Expected results:**
- Step 1 reports `bad-server` as `errored` with a connection-failure cause naming the
  missing command.
- Step 2 shows the TUI still responsive; ragent did not crash or hang.
- Step 3 shows the `bad-server` row with state `errored`.
- Step 4's harness reports `[fail]` on the connect step with the cause and makes no change
  to the live session.

### TC-011 - `/connectors test` isolated harness on a healthy connector

**Requirement:** FR-015, FR-016

**Preconditions:** TUI running; `echo` installed; note the current `/tools` list first.

**Steps:**
1. Type `/tools` and press `Enter`; note the tool names present.
2. Type `/connectors test echo` and press `Enter`; read the per-step results.
3. Type `/tools` and press `Enter` again; compare the list to step 1.
4. Type `/mcp` and press `Enter`; check no extra persistent server was left connected by the
   test.

**Test data:** the subcommand argument `echo`.

**Expected results:**
- Step 2 prints a `[ ok ]` connect step with wall-clock time and a `[ ok ]` tool-invocation
  step naming the tool and its sample arguments.
- Step 3's tool list is identical to step 1: the test connection did not leak into the live
  session.
- Step 4 shows no additional persistent `echo.*` server beyond what the connector's own
  state dictates.

### TC-012 - Unsupported connector is reported, not dropped

**Requirement:** FR-025

**Preconditions:** TUI running; the `unsupported` fixture installed from the catalogue
(TC-005) or from its local directory.

**Steps:**
1. Type `/connectors list` and press `Enter`; read the `unsupported` row.
2. Type `/connectors enable unsupported` and press `Enter`; read the report.

**Test data:** the subcommand argument `unsupported`.

**Expected results:**
- Step 1 shows the `unsupported` connector with a recorded unsupported-capability label
  (naming the unexpressible transport), not a silent omission.
- Step 2 reports the connector cannot be connected because no supported server is declared,
  with the label as the reason.

### TC-013 - Server-id collision is refused

**Requirement:** FR-033

**Preconditions:** TUI running; `echo` installed and enabled; the global `mcp` ledger and
`ragent.json` inspected to confirm no `echo.echo` key exists.

**Steps:**
1. Open `~/scratch/connectors-tests/.ragent/ragent.json` in your editor and add an MCP
   server whose key is exactly `echo.echo` with a valid `command`.
2. Restart the TUI.
3. Type `/connectors enable echo` and press `Enter`; read the report.
4. Type `/mcp` and press `Enter`; check which server owns the `echo.echo` id.

**Test data:** the `ragent.json` `mcp` key `echo.echo`.

**Expected results:**
- Step 3 rejects the connector's server registration with a collision message naming
  `echo.echo`; the pre-existing `ragent.json` server is not overwritten.
- Step 4 shows the `ragent.json` server still owns the id.

### TC-014 - Remove refuses while enabled

**Requirement:** FR-030

**Preconditions:** TUI running; `echo` installed and enabled.

**Steps:**
1. Type `/connectors remove echo` and press `Enter`; read the report.
2. Type `/connectors disable echo` and press `Enter`; read the report.
3. Type `/connectors remove echo` and press `Enter`; read the report.
4. In a second terminal, confirm
   `~/scratch/connectors-tests/.ragent/connectors/echo/` no longer exists.

**Test data:** the subcommand argument `echo`.

**Expected results:**
- Step 1 refuses the removal, changes no files, and instructs the user to disable the
  connector first.
- Step 3 removes the connector and reports success.
- Step 4 confirms the connector directory is gone.

### TC-015 - Master switch `connectors.enabled: false`

**Requirement:** FR-021

**Preconditions:** TUI running; `echo` installed.

**Steps:**
1. Open `~/scratch/connectors-tests/.ragent/ragent.json` in your editor and add:
   `{ "connectors": { "enabled": false } }`.
2. Restart the TUI.
3. Type `/connectors list` and press `Enter`; read the report.
4. Type `/connectors claude echo` and press `Enter`; read the panel, then `Esc`.
5. Type `/mcp` and press `Enter`; check no connector-bridged server is connected.
6. Type `/connectors help` and press `Enter`; read the usage block.

**Test data:** the `connectors.enabled: false` config block.

**Expected results:**
- Steps 3 and 4 report that the connector system is disabled and perform no discovery or
  catalogue fetch.
- Step 5 shows no connector-bridged server connected.
- Step 6 still prints the normal usage block (help remains available).

### TC-016 - `ragent connectors` CLI parity

**Requirement:** FR-004, FR-006, FR-041

**Preconditions:** the binary built; the `echo` connector installed and enabled; the
catalogue URL configured; the category-tagged fixtures from Prerequisite 12 installed.

**Steps:**
1. In a terminal at `~/scratch/connectors-tests/`, run
   `./target/debug/ragent connectors help` and read the output.
2. Run `./target/debug/ragent connectors list` and read the output.
3. Run `./target/debug/ragent connectors enable echo` and read the output.
4. Run `./target/debug/ragent connectors disable echo` and read the output.
5. Compare the `help` output word-for-word with the TUI `/connectors help` output captured in
   TC-001.
6. Run `./target/debug/ragent connectors list --category developer` and read the output.

**Test data:** the CLI subcommands and their arguments above.

**Expected results:**
- Steps 1-4 print the same wording (including the attribution prefix) as the TUI family.
- Step 5's `help` text is identical to the TUI usage block.
- Step 6 prints the same rows and filter line as the TUI
  `/connectors list --category developer` report (FR-041).
- No non-ASCII characters appear in any output.

### TC-017 - Session-start loading of enabled connectors

**Requirement:** FR-008, FR-012

**Preconditions:** TUI running; `echo` enabled from a previous session, then the TUI closed
normally.

**Steps:**
1. Start a fresh TUI session from `~/scratch/connectors-tests/`.
2. Press `Alt+M` to open the MCP panel (or type `/mcp` and press `Enter`).
3. Type `/tools` and press `Enter`.
4. Type `/connectors list` and press `Enter`.

**Test data:** none beyond starting the session.

**Expected results:**
- Step 1 connects the `echo` connector's server automatically at session start (FR-008).
- Step 2 shows the `echo.echo` bridged server connected.
- Step 3 lists the `echo` tool.
- Step 4 shows `echo` with state `connected`.

### TC-018 - Multi-server connector reports per-server state

**Requirement:** FR-026

**Preconditions:** TUI running; a variant of `two-server` staged where `beta`'s command does
not exist (`two-server-bad/`).

**Steps:**
1. Type `/connectors add ~/scratch/connectors-tests/fixtures-src/two-server-bad --force` and
   press `Enter`; read the report.
2. Type `/connectors enable two-server-bad` and press `Enter`; read the report.
3. Type `/connectors list` and press `Enter`; read the row.
4. Type `/mcp` and press `Enter`; check both `two-server-bad.alpha` and
   `two-server-bad.beta`.

**Test data:** the `two-server-bad` source path.

**Expected results:**
- Step 2 reports `alpha` connected and `beta` errored, each with its own state and cause
  (FR-026).
- Step 3 shows the connector with a per-server breakdown, not a single opaque state.
- Step 4 shows `alpha` connected and `beta` failed, both listed independently.

### TC-019 - `/connectors stores` shows the compiled default Claude endpoint

**Requirement:** FR-034, FR-036, FR-037

**Preconditions:** TUI running from `~/scratch/connectors-tests/`; no `connectors` block in
`ragent.json` and no `connectors.stores` entry anywhere in the config; outbound HTTPS
available for the `--check` step.

**Steps:**
1. Type `/connectors stores` and press `Enter`; read the report.
2. Note the Claude catalogue row: its endpoint and its provenance tag.
3. Type `/connectors stores --check` and press `Enter`; read the result.
4. Confirm the endpoint shown equals the value recorded in Prerequisite 10.
5. Disconnect the network (or block outbound HTTPS), repeat step 3, and read the result.

**Test data:** the subcommand `/connectors stores` and flags `--check`.

**Expected results:**
- Step 1 lists a Claude catalogue row even though no `connectors` block is configured: the
  compiled default endpoint is present out of the box (FR-034).
- Step 2 tags that row `default`, because no override is in effect (FR-036).
- Step 3's `--check` probes the endpoint it reported and prints a reachability result
  (reachable, or a network failure reason); it performs the network call only because
  `--check` explicitly requested it.
- Step 4's endpoint equals the Prerequisite 10 value - the compiled default, not an empty or
  unvalidated string.
- Step 5 reports the fetch failure with a reason and leaves the store and cache untouched
  (FR-037); it does not fall back to some other, unvalidated value.
- The output contains no non-ASCII characters and is prefixed `From: /connectors stores`.

### TC-020 - A configured Claude endpoint overrides the default and is tagged `config`

**Requirement:** FR-035, FR-036, FR-037

**Preconditions:** TUI running; the fixture catalogue (Prerequisite 6) available over HTTPS.

**Steps:**
1. Open `~/scratch/connectors-tests/.ragent/ragent.json` and add:

   ```jsonc
   {
     "connectors": {
       "stores": {
         "claude": { "url": "https://<your-fixture-host>/catalogue/index.json" }
       }
     }
   }
   ```

2. Restart the TUI (no rebuild).
3. Type `/connectors stores` and press `Enter`; read the Claude row.
4. Type `/connectors claude echo` and press `Enter`; read the panel rows, then `Esc`.
5. Clear the override back to an empty string
   (`"claude": { "url": "" }`), restart the TUI, and repeat step 3.
6. Delete the `claude` entry entirely, restart the TUI, and repeat step 3.

**Test data:** the fixture-catalogue HTTPS URL; then an empty (`""`) override; then no
override at all.

**Expected results:**
- Step 3 tags the Claude row `config` and prints the override URL, not the compiled default
  (FR-035, FR-036).
- Step 4 fetches from the override and returns the fixture catalogue's `echo` entry.
- Step 5 shows the empty override falling back to the compiled default with tag `default` and
  no error (FR-035).
- Step 6 shows the same compiled default with tag `default`: an absent endpoint is not an
  error.

### TC-021 - A malformed or non-`https` Claude endpoint is refused, not substituted

**Requirement:** FR-037

**Preconditions:** TUI running; note the compiled default endpoint recorded in
Prerequisite 10; know the on-disk location of the catalogue cache so you can inspect it.

**Steps:**
1. Set `connectors.stores.claude.url` to `http://example.org/connectors/index.json`, restart
   the TUI, type `/connectors stores` and press `Enter`; read the report.
2. Set the URL to `not-a-url`, restart, repeat the `/connectors stores` command; read the
   report.
3. Set the URL to `https://example.org/connectors/index.json`, restart, type
   `/connectors claude echo` and press `Enter`; read the panel, then `Esc`.
4. In a second terminal, inspect the catalogue cache directory and its most recent file
   mtime from before these steps.

**Test data:** the URL values `http://example.org/connectors/index.json`, `not-a-url`, and
`https://example.org/connectors/index.json`.

**Expected results:**
- Step 1 is refused with a reason naming the rejected `http` scheme; no fetch is attempted.
- Step 2 is refused with a malformed-URL reason naming the offending input.
- In both cases the report does **not** silently substitute the compiled default as though it
  were the configured value, and no partial or unvalidated value is used (FR-037).
- Step 3 (a valid `https` endpoint that returns no usable catalogue) reports the fetch
  outcome without corrupting state.
- Step 4 shows the catalogue cache unchanged in size and mtime: a refused endpoint left any
  previously cached catalogue untouched.

### TC-022 - The default endpoint takes effect per launch with no rebuild

**Requirement:** FR-035, NFR-002, NFR-003

**Preconditions:** TUI running; the fixture catalogue (Prerequisite 6) over HTTPS; the
binary already built once (no rebuild during this case).

**Steps:**
1. With a configured Claude override pointing at the fixture catalogue, run
   `/connectors stores` and note the endpoint and tag.
2. Without rebuilding, edit `ragent.json` to remove the override; close and relaunch the
   TUI.
3. Run `/connectors stores` and note the endpoint and tag.
4. Run `/connectors stores` again immediately (no config change, same launch) and confirm the
   endpoint and tag are stable within the launch.
5. With the compiled default active, run `/connectors stores` once and then, in a second
   terminal, use the operating system's connection list or the endpoint's access log to
   confirm no catalogue connection was made by that command alone.

**Test data:** the fixture-catalogue HTTPS URL in an override, then its removal.

**Expected results:**
- Step 1 reports the override with tag `config`.
- Step 3, after a relaunch with the override removed, reports the compiled default with tag
  `default` - the change took effect on the next launch, with no rebuild (NFR-002).
- Step 4 shows the same endpoint and tag on both calls: resolution is stable per launch.
- Step 5 shows that resolving and reporting the endpoint performs no network access; the
  endpoint is contacted only for a fetch or an explicit `--check` (NFR-003).

### TC-023 - The browser category filter restricts to one category and `ALL` resets it

**Requirement:** FR-039, FR-040

**Preconditions:** TUI running from `~/scratch/connectors-tests/`; the `echo` (`productivity`),
`two-server` (`developer`), and `needs-token` (`data`) fixtures installed (Prerequisite 12);
the connector browser open.

**Steps:**
1. Open the connector browser with no category selected and note the visible rows.
2. Select the `developer` category and read the visible rows and the reported match count.
3. Select the `data` category and read the visible rows and the reported match count.
4. Select the `ALL` category and read the visible rows and the reported match count.
5. Close and reopen the browser; note which category is selected on open.

**Test data:** the categories `productivity`, `developer`, `data`, and the sentinel `ALL`.

**Expected results:**
- Step 1 shows every installed connector; the report or header indicates the active category
  is `ALL`.
- Step 2 shows only `two-server` and reports the active category `developer` with a match
  count of one; `echo` and `needs-token` are absent (FR-039).
- Step 3 shows only `needs-token` and reports `data` with a match count of one.
- Step 4 shows every connector again and the match count returns to the full total; the active
  filter is reported as `ALL` (FR-040).
- Step 5 shows the browser reopened on `ALL`, the documented default (FR-040).
- No non-ASCII characters appear in the output; the session stays stable throughout.

### TC-024 - `list`/`claude` honour `--category` and the `ALL` reset

**Requirement:** FR-039, FR-040, FR-041

**Preconditions:** TUI running; the same three categorised fixtures installed as in TC-023.

**Steps:**
1. Run `/connectors list --category developer` and read the rows and the reported filter.
2. Run `/connectors list --category ALL` and read the rows and the reported filter.
3. Run `/connectors list --category nosuch` and read the output.
4. Run `ragent connectors list --category data` from a second terminal and read the output.
5. Run `/connectors claude echo --category productivity` and then
   `/connectors claude echo --category developer`, reading each panel's rows and title
   (closing each panel with `Esc`).

**Test data:** the categories `developer`, `data`, `productivity`, `ALL`, and the unknown
value `nosuch`.

**Expected results:**
- Step 1 prints only `two-server`, states the active category `developer`, and shows a match
  count of one (FR-041).
- Step 2 prints every connector and reports the filter as `ALL` (FR-040).
- Step 3 prints an `[err] Unknown category` row naming the unknown category; it is not an
  error and no state changes (FR-041).
- Step 4 prints the same rows and wording as the TUI `/connectors list --category data`
  report (CLI parity).
- Step 5 shows the `echo` row under `productivity` and a "no matching connectors" line under
  `developer`, confirming the query is combined with the category filter. An unknown launch
  category leaves every row visible and reports the refusal in the panel footer.

## Cleanup

1. Disable every enabled connector: for each id shown by `/connectors list` (or
   `ragent connectors list`), run `/connectors disable <id>` (or
   `ragent connectors disable <id>`).
2. Remove the installed connectors: run `/connectors remove <id>` for each id, then confirm
   `~/scratch/connectors-tests/.ragent/connectors/` is empty or absent.
3. Restore configuration: if you edited `~/scratch/connectors-tests/.ragent/ragent.json`
   during TC-005, TC-006, TC-013, TC-015, TC-020, TC-021, or TC-022, restore it from the
   backup made in Prerequisite 9, or delete the temporary `connectors` block, the temporary
   `connectors.stores.community.url` entry, and every temporary
   `connectors.stores.claude.url` override.
4. Restore the shared MCP ledger: restore `<state dir>/mcp_state.json` from the backup made
   in Prerequisite 9, or manually remove the `echo.echo`, `two-server.*`, and
   `echo.echo` collision entries added during the tests.
5. Stop any local HTTPS endpoint started for the fixture catalogue.
6. Delete the scratch tree: `rm -rf ~/scratch/connectors-tests/`.
7. Confirm `git status` in the ragent repository is clean for any file the tests may have
   touched (the tests are designed to write only under the scratch root and the global state
   directory).

---
status: draft
---
# Manual Test Plan: GCF Protocol Integration

Manual verification for [`SPEC.md`](SPEC.md) / [`PLAN.md`](PLAN.md). Each case
maps to acceptance criteria AC-1..AC-8.

## Prerequisites

- Debug build of ragent with the GCF feature implemented:
  `cargo build` (binary at `target/debug/ragent`).
- A working LLM provider: at least one of `ANTHROPIC_API_KEY`,
  `OPENAI_API_KEY`, or a local Ollama endpoint reachable
  (`ollama list` shows a model). Configure via `ragent auth` or environment.
- A scratch project directory with a project-local config so config writes
  are easy to inspect:
  ```bash
  mkdir -p /tmp/gcf-test/.ragent
  cd /tmp/gcf-test
  cat > .ragent/ragent.json <<'EOF'
  {
    "provider": {
      "ollama": {
        "models": { "qwen2.5:7b": {} }
      }
    },
    "defaultAgent": "coder"
  }
  EOF
  ```
  (Substitute any provider you have credentials for; the `provider` block
  content does not matter for the GCF cases.)
- A tool that returns JSON-dense output. Recommended: `stock_quote` (Yahoo
  Finance) or, offline, `task_list` / `codeindex_search` after running
  `/codeindex on`. A provider that produces tool calls on request is needed;
  prompt example given per case.
- No `gcf` key present in `.ragent/ragent.json` before TC-001.

## Test Cases

### TC-001 — Default-off: no config section means GCF disabled

**Requirement(s):** FR-001, FR-002 · **AC:** AC-1

**Preconditions**
- `.ragent/ragent.json` in the scratch project has NO `gcf` section.
- ragent is not yet running.

**Steps**
1. Launch the TUI from the scratch project: `ragent`.
2. In the message input, type `/gcf` and press Enter, then press Enter on the
   autocomplete menu if it appears (bare `/gcf` = help; no state change).
3. Read the help notice bubble that appears.
4. Type `/gcf show` and press Enter.
5. Read the state notice.

**Test data**
- Input 1: `/gcf`
- Input 2: `/gcf show`

**Expected results**
- Step 3: help notice shows the GCF purpose and subcommands `on`, `off`,
  `show`, `help`.
- Step 5: state notice says GCF is **off**, and states the source is the
  default (no config section). Status bar shows `gcf: show`.
- `.ragent/ragent.json` remains without a `gcf` section
  (`cat .ragent/ragent.json | grep gcf` finds nothing).
- No tool result in any later turn is encoded (TC-003 style checks confirm).

### TC-002 — `/gcf on` persists and survives restart

**Requirement(s):** FR-002, FR-003, FR-009 · **AC:** AC-2

**Preconditions**
- Continuing from TC-001 session (or a fresh launch with no `gcf` section).

**Steps**
1. In the TUI type `/gcf on` and press Enter.
2. Read the confirmation notice.
3. Exit the TUI: press `Ctrl+C`, then confirm quit (or type `/quit` and
   Enter).
4. Inspect the config file: `cat .ragent/ragent.json`.
5. Relaunch: `ragent`.
6. Type `/gcf show` and press Enter.

**Test data**
- Input 1: `/gcf on`
- Config inspection after step 4: expect a `gcf` section.

**Expected results**
- Step 2: notice `[ok] GCF: enabled (saved to <config path>)`; status bar
  shows `gcf: on`.
- Step 4: file contains `"gcf": { "enabled": true }`.
- Step 6: `/gcf show` reports **on** with source `persisted config`.

### TC-003 — `/gcf off` persists and disables encoding

**Requirement(s):** FR-002, FR-003 · **AC:** AC-2, AC-3

**Preconditions**
- Session running with GCF currently on (state after TC-002).

**Steps**
1. Type `/gcf off` and press Enter.
2. Read the confirmation notice.
3. Type `/gcf show` and press Enter.
4. Exit (`/quit`), then `cat .ragent/ragent.json`.
5. Relaunch and `/gcf show` again.

**Test data**
- Input 1: `/gcf off`

**Expected results**
- Step 2: notice `[ok] GCF: disabled (saved to <config path>)`; status bar
  `gcf: off`.
- Step 3: reports **off**, source persisted config.
- Step 4: file contains `"gcf": { "enabled": false }`.
- Step 5: still **off** after restart.

### TC-004 — Encoding changes the LLM-visible tool result (on vs off)

**Requirement(s):** FR-004, FR-005 · **AC:** AC-3, AC-5

**Preconditions**
- ragent running in the scratch project, GCF **off** (after TC-003).
- A JSON-dense tool available (e.g. `stock_quote`); or use `task_list` /
  `codeindex_search` offline. This case uses `stock_quote`.

**Steps**
1. With GCF off, submit a prompt that triggers a JSON-dense tool:
   `Use stock_quote to fetch AAPL and show me the data`.
2. Wait for the tool call to complete. Open the prompt inspector:
   type `/prompt` and press Enter. Read the rendered tool-surface
   information (this confirms the session is live; encoding visibility is
   checked in step 4-6 via the transcript and log).
3. Type `/gcf on` and press Enter.
4. Submit the same prompt again: `Use stock_quote to fetch AAPL and show
   me the data`.
5. Compare the two assistant responses:
   - with GCF off (step 1-2) the tool result the model received was raw JSON;
   - with GCF on (step 4) the model received a GCF block; the model's answer
     should still quote the correct price/change figures read from the GCF
     block.
6. Check the activity/log view (type `/log` and press Enter, if available in
   the build, else inspect `log/` files) and confirm the raw JSON value was
   logged (non-LLM consumers still see raw, FR-005).

**Test data**
- Prompt (both passes): `Use stock_quote to fetch AAPL and show me the data`
- Toggle inputs: `/gcf off` state at pass 1; `/gcf on` at pass 2.

**Expected results**
- Both passes produce a successful `stock_quote` tool call.
- Pass 2's model answer is factually consistent with pass 1 (same price,
  change, volume — market data may differ slightly between calls; compare
  structure not exact timestamps).
- With GCF on, the transcript's tool-call part shows the encoded block form
  for the LLM view while the TUI/log views still show raw JSON
  (FR-004 + FR-005).
- No tool errors are introduced by encoding (fallback safety, FR-004).

### TC-005 — `/gcf help` and help aliases

**Requirement(s):** FR-003 · **AC:** AC-8

**Preconditions**
- ragent running (any GCF state).

**Steps**
1. Type `/gcf help` and press Enter. Read the notice.
2. Type `/gcf --help` and press Enter. Read the notice.
3. Type `/gcf -h` and press Enter. Read the notice.
4. Check the status bar after each.

**Test data**
- Inputs: `/gcf help`, `/gcf --help`, `/gcf -h`.

**Expected results**
- Each input shows the same help notice: purpose of GCF encoding, the block
  marker explanation, and the four subcommands with one-line meanings.
- Status bar shows `gcf: help` (or equivalent non-mutating status).
- GCF state is unchanged after all three (verify with `/gcf show`).

### TC-006 — Unknown subcommand rejected

**Requirement(s):** FR-003 · **AC:** AC-7

**Preconditions**
- ragent running; `.ragent/ragent.json` currently has a known `gcf` value.

**Steps**
1. Record current state: `/gcf show`.
2. Type `/gcf enable-now` and press Enter.
3. Read the rejection notice.
4. Type `/gcf show` again; compare with step 1.
5. Exit and `cat .ragent/ragent.json`; confirm the `gcf` section matches the
   value from step 1's source.

**Test data**
- Input: `/gcf enable-now`

**Expected results**
- Step 3: usage notice lists valid subcommands `on`, `off`, `show`, `help`
  and states the subcommand was rejected.
- Step 4: state identical to step 1.
- Step 5: config file unchanged by the rejected command.

### TC-007 — Malformed `gcf` config values rejected

**Requirement(s):** FR-008 · **AC:** AC-7

**Preconditions**
- ragent NOT running.

**Steps**
1. Edit `.ragent/ragent.json`, change the gcf section to:
   `"gcf": { "enabled": "yes" }`
2. Launch `ragent`.
3. Read the startup/config diagnostic.
4. Exit. Change the value to `"gcf": true` (wrong shape).
5. Launch `ragent` again and read diagnostics.
6. Restore a valid config (`"gcf": { "enabled": true }` or remove the
   section) and relaunch; `/gcf show`.

**Test data**
- Malformed value 1: `"gcf": { "enabled": "yes" }`
- Malformed value 2: `"gcf": true`

**Expected results**
- Steps 2-3 and 4-5: load fails with the actionable parse diagnostics (file
  path, line, column, caret marker pointing at the offending value); ragent
  does not silently coerce to default-off.
- Step 6: valid config loads, `/gcf show` reports the restored state.

### TC-008 — `/gcf` autocomplete suggestions

**Requirement(s):** FR-003 · **AC:** AC-8

**Preconditions**
- ragent running at the TUI message input.

**Steps**
1. Type `/gcf` in the message input but DO NOT press Enter.
2. Observe the autocomplete menu entries and parameter hints.
3. Press Enter (or Tab) to accept the top suggestion if offered; then
   without submitting further, verify the suggested subcommand list contains
   the four entries.

**Test data**
- Typed text: `/gcf`

**Expected results**
- The autocomplete menu lists the `gcf` command with suggestions `on`,
  `off`, `show`, `help` and a description mentioning the toggle purpose.
- Pressing Enter on the bare `/gcf` suggestion runs the help path (no state
  change).

---

## Cleanup

1. Exit the ragent TUI (`/quit`).
2. Restore the scratch config: remove the `gcf` section from
   `.ragent/ragent.json`, or delete the scratch directory:
   `rm -rf /tmp/gcf-test`.
3. If `stock_quote` network calls were made, no cleanup is required (public
   API, no credentials).
4. If any test sessions were created in the scratch project, they are
   removed with the directory.

---

## Result Recording

Record per case: PASS / FAIL / BLOCKED, the build hash, config file content
at assertion points, and any notice-text screenshots (or pasted text) into
`specs/gcf/TESTRESULTS.md` (create when executing this plan).
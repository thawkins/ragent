---
status: draft
audit:
  - { time: 1789491242, from: "none", to: "draft", actor: "system" }
---
# Research scholarly-engine exclusion — Manual Test Plan

This is a **manual** test plan for spec `researchnoacc`. It is executed by a
human operator against a built `ragent` binary. It contains no automated test
code.

Coverage: `mf_search` engine exclusion (FR-001, FR-002, FR-003, FR-007, FR-008,
FR-011, FR-013, FR-014, NFR-003, NFR-004), research `--no-papers` end-to-end
(FR-004, FR-006, FR-010, FR-012, NFR-001), flag spelling (FR-005),
`POST /research` parity (FR-009), non-academic engines preserved (FR-015), and
quality gates (NFR-002).

## Prerequisites

1. Build the current binary:
   - `cargo build` (debug is sufficient; allow up to 1000 seconds).
   - Confirm `./target/debug/ragent --version` runs.
2. Confirm the default engine set: `openalex` and `wikipedia` are keyless and
   always present. Optionally configure a web engine by adding a
   `langsearch_api_key` (or `tavily_api_key` / `serper_api_key`) to
   `.ragent/ragent.json`; a keyless-only run (openalex + wikipedia) is
   acceptable for most cases.
3. Confirm `OPENALEX_EMAIL` is set (or `openalex_email` in `ragent.json`) so the
   OpenAlex polite pool responds reliably; this makes "engine was queried vs
   skipped" observations easier.
4. Ensure network access to `api.openalex.org` and `en.wikipedia.org`.
5. Start with `ragent.json` **without** `research.exclude_academic_engines` so
   the default-off path is exercised first.
6. Locate the research run log directory (default `logs/research/`) so produced
   `RESEARCH.md` and gather logs can be inspected.
7. Have an HTTP client available (for example `curl`) for the server cases.

## Test Cases

### TC-001 — `mf_search` excludes OpenAlex when `exclude_engines` is set

**Covers:** FR-001, FR-007, FR-015, NFR-001, NFR-003.

**Preconditions:** Binary built; at least one runnable agent session. OpenAlex
reachable.

**Steps:**

1. Launch the TUI: run `./target/debug/ragent` and press Enter at the prompt.
2. In the message input, type exactly:
   `Use the mf_search tool with query "rust async runtime" and exclude_engines ["openalex"]. Show me the engine list in the metadata.`
3. Send the message (Enter).
4. If a permission prompt for the `web` category appears, approve it (press the
   allow key shown in the dialog).
5. Wait for the tool result to render in the log panel.
6. Open the tool result detail and inspect the `engines_used` /
   `engines_consensus` / `search_engine` fields in the JSON metadata.
7. Repeat steps 2–5 with the same query but **without** the `exclude_engines`
   clause, for comparison.

**Test data:**

- Query: `rust async runtime`
- Exclusion list: `["openalex"]`

**Expected results:**

- With `exclude_engines` set: no result line carries `search_engine` containing
  `openalex`; the metadata reports openalex as excluded/skipped; results from
  `wikipedia` (and any configured web engine) still appear.
- Without `exclude_engines`: openalex rows are present again (baseline
  unchanged), confirming NFR-003.

### TC-002 — `mf_search` with an empty/absent exclusion is unchanged

**Covers:** FR-011, NFR-003.

**Preconditions:** Same as TC-001.

**Steps:**

1. In the TUI input, type exactly:
   `Use the mf_search tool with query "distributed consensus" and no exclude_engines parameter. Return the raw results.`
2. Send the message and approve the `web` permission prompt if shown.
3. Note the ordered result list and the `engines_used` metadata.
4. Run the same query again with `exclude_engines []` explicitly.

**Test data:**

- Query: `distributed consensus`

**Expected results:**

- Both runs return the same engine set and the same ordering/dedup behaviour;
  no engine is skipped; the response is identical to the pre-change baseline.

### TC-003 — All engines excluded returns an explicit result (no panic/hang)

**Covers:** FR-014, NFR-004.

**Preconditions:** Same as TC-001.

**Steps:**

1. Determine the configured engine set (from the metadata captured in TC-001).
2. In the TUI input, type exactly:
   `Use the mf_search tool with query "test" and exclude_engines ["openalex","wikipedia"]. Include every configured engine name in the exclusion list.`
   (Add any configured web engine names — e.g. `"langsearch"` — to the list so
   every engine is named.)
3. Send the message and approve the prompt if shown.
4. Observe the tool result and the TUI state.

**Test data:**

- Exclusion list: every configured engine name, e.g.
  `["openalex","wikipedia"]` (plus any configured web engine).

**Expected results:**

- The tool returns an explicit message that all engines were excluded (or a
  clear empty-result notice), not a silent success.
- The TUI does not panic, freeze, or hang; the session remains responsive.
- The log records the all-engines-excluded condition.

### TC-004 — Unknown engine name in `exclude_engines` is ignored

**Covers:** FR-008.

**Preconditions:** Same as TC-001.

**Steps:**

1. In the TUI input, type exactly:
   `Use the mf_search tool with query "webassembly" and exclude_engines ["openalex","not_a_real_engine"].`
2. Send the message and approve the prompt if shown.
3. Inspect the result and metadata.

**Test data:**

- Exclusion list: `["openalex","not_a_real_engine"]`

**Expected results:**

- The call succeeds (no error about the unknown name).
- `openalex` is excluded; `wikipedia` (and other engines) still run; the
  unknown name has no effect.

### TC-005 — `/research create --no-papers` does not query OpenAlex

**Covers:** FR-004, FR-006, FR-010, FR-015, NFR-001.

**Preconditions:** A `.ragent/ragent.json` with `research.exclude_academic_engines`
absent (default off). Sufficient time for a research run.

**Steps:**

1. Launch the TUI: `./target/debug/ragent`.
2. Type a leading slash to open command completion; enter the research-create
   flow by typing exactly:
   `/research create`
   and pressing space. Confirm the completion list shows `--no-papers`.
3. Continue typing the full command on one line:
   `/research create rustconcurrency "Rust async concurrency patterns" --no-papers`
4. Press Enter to run.
5. If the interactive clarification gate appears (a prompt asking to confirm or
   refine the research question), answer it: type `yes` and press Enter, or
   select the confirm option shown.
6. Allow the run to complete; watch the progress lines in the message window.
7. Open the produced `RESEARCH.md` under `logs/research/` (use the `read` tool or
   a shell `cat`) and inspect the source/engine table.
8. Inspect the corresponding gather log for per-engine search dispatch lines.

**Test data:**

- Spec/topic argument: `rustconcurrency`
- Research question: `Rust async concurrency patterns`
- Flag: `--no-papers`

**Expected results:**

- The run completes normally and produces `RESEARCH.md`.
- No `openalex` search dispatch appears in the gather log; no per-engine
  `openalex` line in the source table.
- Results from `wikipedia` and any configured web engine are present and
  processed normally (FR-010).
- No fetch/processing of discarded OpenAlex candidates occurs.

### TC-006 — `/research create` without `--no-papers` still uses OpenAlex

**Covers:** FR-015 (negative case), NFR-003.

**Preconditions:** Same as TC-005.

**Steps:**

1. In the TUI, type exactly:
   `/research create rustconcurrency2 "Rust async concurrency patterns"`
   (no `--no-papers`).
2. Press Enter and answer the clarification gate as in TC-005 if it appears.
3. When complete, inspect `RESEARCH.md` and the gather log.

**Test data:**

- Spec/topic argument: `rustconcurrency2`
- Research question: `Rust async concurrency patterns`

**Expected results:**

- The gather log shows an `openalex` search dispatch for at least one sub-query
  (when the query matches scholarly works).
- OpenAlex results participate in ranking, confirming the default path was not
  changed.

### TC-007 — Config-only exclusion via `ragent.json`

**Covers:** FR-012.

**Preconditions:** Ability to edit `.ragent/ragent.json`.

**Steps:**

1. Quit the TUI (if running).
2. Edit `.ragent/ragent.json` and add:
   `"research": { "exclude_academic_engines": true }`
   (merge into the existing `research` object if present).
3. Relaunch `./target/debug/ragent`.
4. Run a research creation **without** `--no-papers`, exactly:
   `/research create cfgtest "Rust async concurrency patterns"`.
5. Answer the clarification gate if it appears.
6. Inspect the gather log for `openalex` dispatch lines.
7. Now run the same research again **with** `--no-papers` and a different topic
   id: `/research create cfgtest2 "Rust async concurrency patterns" --no-papers`.
8. Inspect its gather log too.

**Test data:**

- Config: `research.exclude_academic_engines = true`
- Run A: `cfgtest`, no flag
- Run B: `cfgtest2`, with `--no-papers`

**Expected results:**

- Run A (config only): no `openalex` dispatch — the config alone excludes the
  academic engine.
- Run B (config + flag): same behaviour (no `openalex` dispatch); the flag is
  redundant but consistent.

### TC-008 — Per-run flag overrides config when config is off

**Covers:** FR-012 (precedence).

**Preconditions:** `.ragent/ragent.json` with
`research.exclude_academic_engines` either absent or `false`.

**Steps:**

1. Confirm the config value is off.
2. Run `/research create override1 "Rust async concurrency patterns" --no-papers`.
3. Inspect the gather log.
4. Then set `research.exclude_academic_engines` to `false` explicitly in the
   config, relaunch, and repeat with `--no-papers` (topic `override2`).

**Test data:**

- Config: off / false
- Flag: `--no-papers`

**Expected results:**

- In both cases the flag causes the academic engine to be excluded, proving the
  per-run flag wins over the off config.

### TC-009 — `mf_search` direct exclusion reports skipped engines in metadata

**Covers:** NFR-004, FR-001.

**Preconditions:** Same as TC-001.

**Steps:**

1. In the TUI, type exactly:
   `Use the mf_search tool with query "quantum computing" and exclude_engines ["openalex"]. Print the complete JSON metadata block.`
2. Send and approve the prompt if shown.
3. Read the returned metadata JSON.

**Test data:**

- Query: `quantum computing`
- Exclusion list: `["openalex"]`

**Expected results:**

- The metadata exposes which engines were skipped/excluded and why (e.g. a
  skipped/excluded list containing `openalex`), so an operator can confirm the
  exclusion without reading logs.

### TC-010 — `POST /research` honours the scholarly-exclusion field

**Covers:** FR-009.

**Preconditions:** A running server: `./target/debug/ragent serve --port 9100`.
An HTTP client available.

**Steps:**

1. In a terminal, start the server: `./target/debug/ragent serve --port 9100`.
2. Submit a research request that sets the scholarly-exclusion field (field name
   `no_scholarly`), for example:
   `curl -s -X POST http://127.0.0.1:9100/research -H 'Content-Type: application/json' -d '{"topic":"srvtest","question":"Rust async concurrency patterns","no_scholarly":true}'`
3. Capture the JSON response, including the `invocation_summary` (or equivalent
   echo of the built invocation).
4. Inspect the spawned run's gather log for `openalex` dispatch lines.
5. Repeat with `"no_scholarly": false` and topic `srvtest2`.

**Test data:**

- Run A: `no_scholarly = true`
- Run B: `no_scholarly = false`

**Expected results:**

- Run A: the invocation summary contains the `--no-papers` (or equivalent)
  flag, and the run issues no `openalex` dispatch.
- Run B: no such flag, and the run issues `openalex` dispatch as normal.

### TC-011 — `--no-papers` spelling accepted on the root CLI

**Covers:** FR-005.

**Preconditions:** Built binary.

**Steps:**

1. In a shell, run:
   `./target/debug/ragent research create --help`
2. Confirm the help text lists `--no-papers`.
3. Run a short research creation from the CLI:
   `./target/debug/ragent research create clitest "Rust async concurrency patterns" --no-papers`
   and press Enter; allow it to complete or interrupt after the gather phase
   starts.
4. Inspect the gather log for `openalex` dispatch lines.

**Test data:**

- Command: `ragent research create clitest "Rust async concurrency patterns" --no-papers`

**Expected results:**

- The command is accepted (no "unexpected argument" / "unknown flag" error).
- No `openalex` dispatch occurs for the run.

### TC-012 — TUI completion and help list `--no-papers`

**Covers:** FR-005.

**Preconditions:** TUI launched.

**Steps:**

1. Launch `./target/debug/ragent`.
2. Type `/research create ` (with a trailing space) and read the completion
   dropdown and any inline help.
3. Run the TUI help for research (for example type `/research` and read the help
   line, or open the research help entry) and confirm the flag is documented.

**Test data:** none (navigation only).

**Expected results:**

- The completion list includes `--no-papers`.
- The help line lists `--no-papers`.

### TC-013 — Quality gates pass (regression)

**Covers:** NFR-002.

**Preconditions:** Clean working tree for the touched crates.

**Steps:**

1. From the workspace root, run `cargo fmt --check` and confirm it reports no
   diffs.
2. Run `cargo clippy` for the touched crates
   (`-p ragent-tools-extended -p ragent-research -p ragent-server -p ragent-tui`)
   and confirm no new warnings.
3. Run `cargo test -p ragent-tools-extended -p ragent-research` (and the server
   tests) and confirm all pass.

**Test data:** none.

**Expected results:**

- `cargo fmt --check` is clean.
- No new clippy warnings attributable to this change.
- All tests pass.

## Cleanup

1. Remove any test additions from `.ragent/ragent.json`:
   `research.exclude_academic_engines`, and any temporary `langsearch_api_key`
   / `tavily_api_key` / `serper_api_key` added for testing, unless they are to be
   kept.
2. Delete or archive the throwaway research specs/topics created by the tests:
   `rustconcurrency`, `rustconcurrency2`, `cfgtest`, `cfgtest2`, `override1`,
   `override2`, `srvtest`, `srvtest2`, `clitest`, and their `RESEARCH.md` /
   gather-log artefacts under `logs/research/`.
3. Stop the server started in TC-010 (Ctrl+C in its terminal).
4. If the test created `.ragent/agents/` or other scratch files, remove them.
5. Re-run `cargo fmt --check` once more if any source edits were made during
   manual testing.

---
status: draft
audit:
  - { time: 1789522081, from: "none", to: "draft", actor: "system" }
---
# Concept and finding output limits — Manual Test Plan

This is a **manual** test plan for spec `researchmax`. It is executed by a human
operator against a built `ragent` binary. It contains no automated test code.

Coverage: concept and finding limits (FR-001, FR-002, FR-006, FR-007),
reverse-relevance ordering (FR-003, FR-004, FR-005), CLI/hand-parser/TUI/server
plumbing (FR-012, FR-013), `ragent.json` defaults and precedence (FR-008,
FR-017), zero/unbounded behaviour (FR-016), boundary and unwanted cases
(FR-015, FR-019, FR-020, FR-022), and documentation (NFR-003).

## Prerequisites

1. Build the current binary:
   - Run `cargo build` (debug is sufficient; allow up to 1000 seconds).
   - Confirm `./target/debug/ragent --version` runs.
2. Configure a working LLM provider so concept extraction and synthesis run
   (the concepts engine is only wired when a model is available). Set the
   relevant API key for your configured provider in the environment, or use a
   local Ollama model.
3. Ensure network access for web gathering (or plan to use `--use-local` on a
   project with several files, which still produces findings via the mechanical
   fallback).
4. Prepare a topic that will gather a decent number of sources, for example a
   broad, well-documented topic such as `rust async runtimes`. Choose a topic
   likely to yield more than 5 concepts and more than 20 findings to exercise
   the caps.
5. Locate the research output folder (default `research/<name>/`) so the
   produced `RESEARCH.md` can be inspected. Also note `logs/research/` for run
   logs.
6. Remove any `research.max_concepts` / `research.max_findings` keys from
   `.ragent/ragent.json` before the default cases so the built-in defaults are
   exercised first. Keep a copy of the file to restore afterwards.
7. Have an HTTP client available (for example `curl`) for the server case.

## Test Cases

### TC-001 — Default run caps concepts at 5 and findings at 20

**Covers:** FR-001, FR-002, FR-009, FR-014, FR-015, FR-022.

**Preconditions:** Binary built; LLM provider configured; `.ragent/ragent.json`
has no `max_concepts`/`max_findings`; topic likely to exceed both limits.

**Steps:**

1. Launch the TUI: run `./target/debug/ragent` and press Enter at the prompt.
2. Type exactly:
   `/research create rust-async-defaults --topic "rust async runtimes" --mode standard`
3. Press Enter to submit.
4. Approve any permission prompts (press the allow key shown) and wait for the
   run to complete.
5. Open `research/rust-async-defaults/RESEARCH.md` in a text editor.
6. Locate the `## Concepts` section. Count the `### N.` concept headings.
7. Locate the `## Findings` section. Count the `### **Finding N**` headings.
8. Confirm the concept numbering runs `1..k` with no gaps and the finding
   numbering runs `1..m` with no gaps.

**Test data:**

- Name: `rust-async-defaults`
- Topic: `rust async runtimes`
- Mode: `standard`

**Expected results:**

- The `## Concepts` section contains at most 5 `### N.` concept headings.
- The `## Findings` section contains at most 20 `### **Finding N**` headings.
- Numbering is contiguous in both sections (no skipped or duplicated `N`).
- If the run genuinely produced fewer than these counts, all of them appear
  (no padding).

### TC-002 — `--max-concepts` and `--max-findings` set exact counts

**Covers:** FR-006, FR-007, FR-010, FR-011.

**Preconditions:** As TC-001; a topic known to yield more than 3 findings and
more than 2 concepts.

**Steps:**

1. In the TUI prompt, type exactly:
   `/research create rust-async-small --topic "rust async runtimes" --max-concepts 2 --max-findings 3`
2. Press Enter; approve prompts; wait for completion.
3. Open `research/rust-async-small/RESEARCH.md`.
4. Count the `### N.` concept headings in `## Concepts`.
5. Count the `### **Finding N**` headings in `## Findings`.
6. Confirm the surviving headings are numbered `1`, `2` for concepts and `1`,
   `2`, `3` for findings.

**Test data:**

- Name: `rust-async-small`
- Topic: `rust async runtimes`
- `--max-concepts 2`
- `--max-findings 3`

**Expected results:**

- Exactly 2 concept headings appear (when at least 2 were available).
- Exactly 3 finding headings appear (when at least 3 were available).
- Headings are renumbered contiguously from 1.

### TC-003 — Retained entries are the highest-relevance ones (reverse order)

**Covers:** FR-003, FR-004, FR-005.

**Preconditions:** Completed TC-001 `rust-async-defaults` (unbounded-by-limit
baseline not available, so instead run a large-limit baseline) and TC-002.

**Steps:**

1. In the TUI prompt, type exactly:
   `/research create rust-async-large --topic "rust async runtimes" --max-concepts 50 --max-findings 50`
2. Press Enter; approve prompts; wait for completion.
3. Open `research/rust-async-large/RESEARCH.md` and record, in order, the
   headline of every `### **Finding N**` entry and each entry's `[#N]`
   citations.
4. Open `research/rust-async-small/RESEARCH.md` from TC-002.
5. Compare the 3 surviving finding headlines/citations against the large-limit
   list.
6. Repeat the comparison for the 2 surviving concept headings.

**Test data:**

- Baseline (`rust-async-large`): `--max-concepts 50 --max-findings 50`
- Capped (`rust-async-small`): `--max-concepts 2 --max-findings 3`

**Expected results:**

- The 3 findings kept in the capped run are the 3 highest-relevance entries from
  the baseline run (by cited-source relevance rank, ties broken by citation
  count then original order).
- The 2 concepts kept in the capped run are the 2 highest-relevance concepts
  from the baseline run.
- No lower-relevance entry is retained while a higher-relevance entry is
  dropped.

### TC-004 — `0` disables the limit (unbounded)

**Covers:** FR-016, FR-022.

**Preconditions:** As TC-001.

**Steps:**

1. In the TUI prompt, type exactly:
   `/research create rust-async-unbounded --topic "rust async runtimes" --max-concepts 0 --max-findings 0`
2. Press Enter; approve prompts; wait for completion.
3. Open `research/rust-async-unbounded/RESEARCH.md`.
4. Count the concepts and findings.

**Test data:**

- Name: `rust-async-unbounded`
- `--max-concepts 0`
- `--max-findings 0`

**Expected results:**

- No truncation is applied: every concept and finding the model produced is
  rendered (counts are not forced to 0 and not capped at 5 / 20).
- The report remains well-formed with contiguous numbering.

### TC-005 — `ragent.json` defaults apply when flags are omitted

**Covers:** FR-008, FR-014, FR-017.

**Preconditions:** `.ragent/ragent.json` is editable; a config key block is
available.

**Steps:**

1. Edit `.ragent/ragent.json` and add:

   ```json
   {
     "research": {
       "max_concepts": 1,
       "max_findings": 2
     }
   }
   ```

   (merge into the existing `research` object if one is present).
2. Save the file.
3. In the TUI prompt, type exactly:
   `/research create rust-async-config --topic "rust async runtimes"`
4. Press Enter; approve prompts; wait for completion.
5. Open `research/rust-async-config/RESEARCH.md` and count concepts and
   findings.
6. Re-run the same command with `--max-concepts 4 --max-findings 7` appended and
   inspect the new report.

**Test data:**

- Config: `research.max_concepts = 1`, `research.max_findings = 2`
- Step 3 command: name `rust-async-config`, no flags
- Step 6 command: name `rust-async-config-override`, flags `--max-concepts 4
  --max-findings 7`

**Expected results:**

- The no-flag run renders at most 1 concept and at most 2 findings.
- The flag run overrides the config and renders at most 4 concepts and 7
  findings (the per-run flags take precedence).

### TC-006 — CLI and hand parser reject malformed values

**Covers:** FR-019.

**Preconditions:** Binary built; a terminal outside the TUI.

**Steps:**

1. In a terminal, run:
   `./target/debug/ragent research create bad-limit --topic "test" --max-concepts abc`
2. Observe the exit status and error message.
3. In the TUI prompt, type exactly:
   `/research create bad-limit --topic "test" --max-findings -1`
4. Press Enter and observe the response.
5. In the TUI prompt, type exactly `/research help` and press Enter.

**Test data:**

- `--max-concepts abc`
- `--max-findings -1`

**Expected results:**

- Step 1–2: the CLI rejects `abc` with a clear error naming the invalid value;
  it does not silently run with the default.
- Step 3–4: the hand parser rejects `-1` (or reports it as invalid) rather than
  silently applying the default.
- Step 5: `/research help` lists both `--max-concepts N` and `--max-findings N`
  with their default values described.

### TC-007 — `POST /research` parity and invocation round-trip

**Covers:** FR-012, FR-013.

**Preconditions:** Binary built; LLM provider configured; an HTTP client is
available; the server can be started.

**Steps:**

1. In a terminal, start the server: `./target/debug/ragent serve --port 9100`
   (or the port configured in your setup).
2. Send a create request with the caps set:

   ```
   curl -s -X POST http://127.0.0.1:9100/research \
     -H 'Content-Type: application/json' \
     -d '{"name":"api-caps","topic":"rust async runtimes","max_concepts":2,"max_findings":3}'
   ```

3. Capture the JSON response and locate the recorded invocation summary string.
4. Confirm the summary contains `--max-concepts 2` and `--max-findings 3`.
5. Open the produced `research/api-caps/RESEARCH.md` and count concepts and
   findings.

**Test data:**

- JSON body: `{"name":"api-caps","topic":"rust async runtimes","max_concepts":2,"max_findings":3}`

**Expected results:**

- The request is accepted (no 4xx for the new fields).
- The invocation summary emitted for the run includes both flags with the
  supplied values.
- The produced report honours the caps (<= 2 concepts, <= 3 findings).

### TC-008 — Report stays stable when below the limits

**Covers:** NFR-002, FR-015, FR-022.

**Preconditions:** A small input where the model is unlikely to exceed the caps,
for example `--use-local` on a project with a handful of files, or a narrow
topic.

**Steps:**

1. In the TUI prompt, type exactly:
   `/research create rust-async-tiny --topic "what is tokio" --use-local`
2. Press Enter; approve prompts; wait for completion.
3. Open `research/rust-async-tiny/RESEARCH.md` and count concepts and findings.
4. Compare the section headings and overall structure against an equivalent run
   that does not set the flags.

**Test data:**

- Name: `rust-async-tiny`
- Topic: `what is tokio`
- `--use-local`

**Expected results:**

- All entries the run produced appear (counts at or below 5 / 20 are not
  reduced further).
- Section headings (`## Concepts`, `## Findings`) and surrounding structure are
  unchanged from a run without the flags.
- No blank or placeholder entries are inserted to reach a limit.

## Cleanup

1. Restore `.ragent/ragent.json` to its original contents (remove the
   `research.max_concepts` / `research.max_findings` keys added in TC-005).
2. Remove the scratch research items created by these tests if they are not
   wanted:
   - `research/rust-async-defaults/`
   - `research/rust-async-small/`
   - `research/rust-async-large/`
   - `research/rust-async-unbounded/`
   - `research/rust-async-config/`
   - `research/rust-async-config-override/`
   - `research/rust-async-tiny/`
   - `research/api-caps/`
3. Stop the HTTP server started in TC-007 (Ctrl-C in its terminal).
4. Remove any temporary request/response files captured for TC-007.

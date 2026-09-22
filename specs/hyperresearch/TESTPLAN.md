---
spec_id: hyperresearch
status: draft
title: "Manual test plan for Hyperresearch integration"
author: "ragent"
date: "2025-08-15"
---

# Manual Test Plan: Hyperresearch integration for ragent /research

## Prerequisites

1. Build ragent from source in the current workspace:
   ```bash
   cargo build --release
   ```
2. Ensure an LLM provider is configured (e.g., `ANTHROPIC_API_KEY` or `OLLAMA_HOST`).
3. For open-access recovery tests, set a valid contact email in one of:
   - `ragent.json` under `research.contact_email`
   - The `UNPAYWALL_EMAIL` environment variable
4. Prepare a clean temporary project directory to avoid contaminating the main ragent vault.
5. Have a known paywalled DOI that has a legal open-access copy available (e.g., a PMC article or an Unpaywall-listed repository copy).

## Cleanup

1. After each test, remove the run directories under `.ragent/research_vault/` created during the test.
2. Delete any `RESEARCH.md` files written by the tests.
3. Unset `UNPAYWALL_EMAIL` if it was set only for testing.

## Test Cases

### TC-001: Default tier is full

**Title:** Research command defaults to full tier

**Preconditions:**
- ragent is built and runnable via `target/release/ragent` or `cargo run --`.
- A working directory without prior vault data for the chosen query is available.
- Provider is configured.

**Steps:**
1. Open a terminal in a clean temporary directory.
2. Run:
   ```bash
   /path/to/ragent research "GLP-1 drugs cardiovascular outcomes"
   ```
   (If using the TUI, launch ragent, type `/research GLP-1 drugs cardiovascular outcomes`, and press Enter.)
3. Wait for the command to finish, or interrupt it after the width sweep step is logged.
4. Open `.ragent/research_vault/<run_tag>/manifest.json`.

**Test data to enter:**
- Query text: `GLP-1 drugs cardiovascular outcomes`
- No tier flag supplied

**Expected results:**
- `manifest.json` field `tier` equals `full`.
- The log output or TUI log panel shows adversarial steps such as contradiction graph or corpus critic.
- The manifest lists steps beyond the light-tier subset.

---

### TC-002: Light tier completes a bounded query

**Title:** Light tier research pipeline

**Preconditions:**
- Same as TC-001.

**Steps:**
1. Open a terminal in a clean temporary directory.
2. Run:
   ```bash
   /path/to/ragent research --tier light "what is retrieval-augmented generation"
   ```
   (TUI equivalent: `/research --tier light what is retrieval-augmented generation` then Enter.)
3. Wait for the command to finish.
4. List the contents of `.ragent/research_vault/`:
   ```bash
   ls -la .ragent/research_vault/
   ```
5. Open the generated `RESEARCH.md`.
6. Open `.ragent/research_vault/<run_tag>/manifest.json`.

**Test data to enter:**
- Query text: `what is retrieval-augmented generation`
- Tier flag: `light`

**Expected results:**
- A `RESEARCH.md` is produced.
- `.ragent/research_vault/` contains at least one run directory with `sources/` and `manifest.json`.
- The report does not contain a contradiction graph or source tensions section.
- The manifest `tier` field equals `light` and `completed_steps` does not include contradiction, loci, depth, critics, gap-fill, or readability audit.
- The elapsed wall-clock time is under 45 minutes.

---

### TC-003: Full tier runs the adversarial pipeline

**Title:** Full tier research pipeline with audit steps

**Preconditions:**
- Same as TC-001.
- At least 60 minutes of uninterrupted runtime is available.

**Steps:**
1. In a terminal, run:
   ```bash
   /path/to/ragent research --tier full "GLP-1 drugs cardiovascular outcomes"
   ```
   (TUI equivalent: `/research --tier full GLP-1 drugs cardiovascular outcomes` then Enter.)
2. Wait for each pipeline step to log to the terminal or TUI log panel.
3. When prompted by the cite-check gate (if any citation fails), approve or reject each flagged sentence.
4. Open `.ragent/research_vault/<run_tag>/manifest.json`.
5. Open `RESEARCH.md`.

**Test data to enter:**
- Query text: `GLP-1 drugs cardiovascular outcomes`
- Tier flag: `full`
- Cite-check gate decisions: approve any flagged items that are actually correct, reject any that are unsupported.

**Expected results:**
- `manifest.json` shows `tier: full` and lists all 16 steps as completed.
- `RESEARCH.md` contains sections for contradiction graph, source tensions, and cite-check summary.
- No citation in the shipped report has `CITATION_VERIFICATION_FAILED` unresolved.

---

### TC-004: Resume an interrupted full-tier run

**Title:** Resumable research run

**Preconditions:**
- TC-003 has been started but interrupted before completion, OR a new full-tier run is started and forcibly stopped with Ctrl+C after the width sweep.

**Steps:**
1. Identify the `run_tag` from the run directory name under `.ragent/research_vault/`.
2. Run:
   ```bash
   /path/to/ragent research resume <run_tag>
   ```
   (TUI equivalent: `/research resume <run_tag>` then Enter.)
3. Wait for completion.
4. Compare the `manifest.json` timestamps of the first completed step with the resumed steps.
5. Inspect the vault for duplicate source files.

**Test data to enter:**
- `run_tag` value from the directory name.

**Expected results:**
- The run completes without re-fetching sources already present in the vault.
- The manifest shows the earlier steps have older timestamps and later steps have newer timestamps.
- No source file in the vault appears twice with different content hashes.

---

### TC-005: Open-access recovery of a paywalled DOI

**Title:** Paywalled source recovered via Unpaywall / Europe PMC

**Preconditions:**
- `research.open_access_recovery` is `true` in `ragent.json`, or `UNPAYWALL_EMAIL` is set.
- A known paywalled DOI with an open-access copy is identified.
- The provider has web-fetch capability.

**Steps:**
1. In a terminal, run:
   ```bash
   /path/to/ragent research --tier full "effects of sleep deprivation on cognitive performance DOI:10.xxxx/xxxxx"
   ```
   Replace `10.xxxx/xxxxx` with the real paywalled-but-OA DOI.
2. Watch the terminal or TUI log panel for messages containing `oa_recovery`, `Unpaywall`, or `Europe PMC`.
3. Open the vault source note for the recovered DOI:
   ```bash
   cat .ragent/research_vault/<run_tag>/sources/<source_id>.md
   ```
4. Open `RESEARCH.md` and locate the frontmatter and any citation of the recovered source.

**Test data to enter:**
- Query text including the paywalled DOI.
- `contact_email` in `ragent.json` (if not using env var).

**Expected results:**
- The source body length exceeds the configured `oa_min_full_text_chars` default.
- The source note contains `oa_recovery: true` and `oa_version`.
- The report frontmatter or a dedicated section discloses that an open-access copy was substituted.
- The citation does not claim the source was read from the original publisher URL.

---

### TC-006: Vault reuse avoids redundant fetching

**Title:** Reusing already-vaulted sources

**Preconditions:**
- TC-003 has completed, leaving a populated vault for a related query.

**Steps:**
1. Run a second research command with a closely related query:
   ```bash
   /path/to/ragent research --tier light "semaglutide heart failure"
   ```
2. Observe the terminal or TUI log for messages indicating `vault_hit`, `source reused`, or no new web fetch for sources already in the vault.
3. Compare the run directory of the second query with the first.

**Test data to enter:**
- Second query: `semaglutide heart failure`
- Tier flag: `light`

**Expected results:**
- The second run completes faster than the first because existing sources are reused.
- The vault manifest for the second run references source IDs that also exist in the first run's vault.
- The number of new web fetches is less than the number of reused sources.

---

### TC-007: Dissertation tier partitions chapters

**Title:** Dissertation tier chaptered report

**Preconditions:**
- Same as TC-003.
- At least 4 hours of uninterrupted runtime is available.

**Steps:**
1. Run:
   ```bash
   /path/to/ragent research --tier dissertation --chapter-count 5 "history of programming language type systems"
   ```
   (TUI equivalent: `/research --tier dissertation --chapter-count 5 history of programming language type systems`.)
2. Wait for the run to announce chapter boundaries in the log.
3. Open `RESEARCH.md`.

**Test data to enter:**
- Query text: `history of programming language type systems`
- Tier flag: `dissertation`
- Chapter count: `5`

**Expected results:**
- `RESEARCH.md` contains at least 5 chapter headings.
- The manifest `tier` field equals `dissertation` and includes a `chapters` array with 5 entries.
- Each chapter has its own subdirectory or manifest section under the run directory.

---

### TC-008: Cite-check failure gate blocks unsupported citations

**Title:** Cite-check gate rejects hallucinated citation

**Preconditions:**
- A full-tier run is available that produced at least one `CITATION_VERIFICATION_FAILED` flag during cite-check.
- The user has access to the TUI approval dialog or the CLI `--yes` / `--no` flags.

**Steps:**
1. Re-run or resume the full-tier query until the cite-check step produces a flagged citation.
2. In the TUI, when the permission dialog appears for the flagged citation, select **Deny**.
   In the CLI, run with `--no` (auto-deny) if supported, or manually reject when prompted.
3. Allow the run to complete.
4. Open `RESEARCH.md`.

**Test data to enter:**
- Cite-check decision: **Deny** / `--no`

**Expected results:**
- The unsupported claim is removed or rewritten in the final report.
- `RESEARCH.md` does not contain the flagged sentence in its original form.
- The cite-check summary section lists the number of rejected citations as at least one.

---

### TC-009: OA recovery disabled by default

**Title:** Open-access recovery defaults to off

**Preconditions:**
- No `research.open_access_recovery` key in `ragent.json`.
- No `UNPAYWALL_EMAIL` environment variable set.
- A paywalled DOI is available for testing.

**Steps:**
1. Run:
   ```bash
   /path/to/ragent research --tier full "effects of sleep deprivation on cognitive performance DOI:10.xxxx/xxxxx"
   ```
2. Inspect the log and the source note for the DOI.

**Test data to enter:**
- Query text including the paywalled DOI.
- No OA recovery configuration.

**Expected results:**
- The log does not contain `oa_recovery`, `Unpaywall`, or `Europe PMC`.
- The source note does not contain an `oa_recovery` field.
- The source body length is whatever the original fetch returned (possibly only an abstract).

---

## Sign-off

| Role | Name | Date | Result |
|------|------|------|--------|
| Tester | | | |
| Reviewer | | | |

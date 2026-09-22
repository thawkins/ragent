---
status: draft
---

# Research Cluster Manual Test Plan

## Overview

This document describes manual test procedures for the `/research cluster` slash command. All tests assume a working ragent build and a configured LLM provider.

## Prerequisites

1. A local ragent binary exists at `./target/release/ragent` (or equivalent).
2. An LLM provider is configured with a valid API key (e.g., `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, or a local Ollama endpoint).
3. A completed research run with downloaded web sources exists under `research/<run_name>/sources/`.
   - If none exists, create one with a command such as:
     ```bash
     ./target/release/ragent run --no-tui "research the Rust programming language to docs/research/rust-history"
     ```
4. The `sources/` folder for that run contains at least two non-empty source documents.

## Cleanup

1. Delete or archive any `CONCEPTS.md` files created during testing if they are no longer needed.
2. Remove any test research folders created specifically for negative-path testing.

## Test Cases

### TC-001: Successful cluster command

**Title:** Generate `CONCEPTS.md` from a research run with sources.

**Preconditions:**
- Research run `rust-history` exists.
- `research/rust-history/sources/` contains two or more source documents.
- `research/rust-history/CONCEPTS.md` does not already exist.

**Steps:**
1. Launch the TUI by running `./target/release/ragent`.
2. Wait for the main chat interface to render.
3. Press the input-focus key (default `i` or click the input bar) and type `/research cluster rust-history`.
4. Press `Enter`.

**Test data to enter:**
- Slash command: `/research cluster rust-history`

**Expected results:**
- The TUI displays a progress indicator such as "Reading sources..." followed by "Extracting concepts...".
- After the LLM returns, the TUI shows a message stating that `CONCEPTS.md` was created.
- The file `research/rust-history/CONCEPTS.md` exists on disk.
- The file contains up to 10 concepts, each with a concise name (2-4 words), a definition/description, and 1-2 evidence bullets, formatted with markdown headings and bullets.

---

### TC-002: Research folder does not exist

**Title:** Error handling for missing research folder.

**Preconditions:**
- No folder named `research/nonexistent-run` exists.

**Steps:**
1. Launch the TUI by running `./target/release/ragent`.
2. Wait for the main chat interface to render.
3. Focus the input bar and type `/research cluster nonexistent-run`.
4. Press `Enter`.

**Test data to enter:**
- Slash command: `/research cluster nonexistent-run`

**Expected results:**
- No LLM call is initiated.
- The TUI displays an error message such as "Research run 'nonexistent-run' not found.".
- No `CONCEPTS.md` file is created anywhere under `research/`.

---

### TC-003: Research folder exists but has no sources folder

**Title:** Error handling for missing `sources/` folder.

**Preconditions:**
- Research run `empty-run` exists.
- `research/empty-run/` exists but contains no `sources/` sub-folder.

**Steps:**
1. Launch the TUI by running `./target/release/ragent`.
2. Wait for the main chat interface to render.
3. Focus the input bar and type `/research cluster empty-run`.
4. Press `Enter`.

**Test data to enter:**
- Slash command: `/research cluster empty-run`

**Expected results:**
- No LLM call is initiated.
- The TUI displays an error message such as "No sources folder found for research run 'empty-run'.".
- No `CONCEPTS.md` file is created.

---

### TC-004: Sources folder is empty

**Title:** Error handling for empty `sources/` folder.

**Preconditions:**
- Research run `no-sources-run` exists.
- `research/no-sources-run/sources/` exists but contains no files.

**Steps:**
1. Launch the TUI by running `./target/release/ragent`.
2. Wait for the main chat interface to render.
3. Focus the input bar and type `/research cluster no-sources-run`.
4. Press `Enter`.

**Test data to enter:**
- Slash command: `/research cluster no-sources-run`

**Expected results:**
- No LLM call is initiated.
- The TUI displays an error message such as "No source documents found for research run 'no-sources-run'.".
- No `CONCEPTS.md` file is created.

---

### TC-005: Overwrite guard without `--force`

**Title:** Existing `CONCEPTS.md` triggers confirmation.

**Preconditions:**
- Research run `rust-history` exists with a populated `sources/` folder.
- `research/rust-history/CONCEPTS.md` already exists.
- Note the current modification time and first line of the existing file.

**Steps:**
1. Launch the TUI by running `./target/release/ragent`.
2. Wait for the main chat interface to render.
3. Focus the input bar and type `/research cluster rust-history`.
4. Press `Enter`.
5. If a confirmation prompt appears asking whether to overwrite `CONCEPTS.md`, type `n` and press `Enter`.

**Test data to enter:**
- Slash command: `/research cluster rust-history`
- Confirmation response: `n`

**Expected results:**
- A confirmation prompt is displayed before any LLM call.
- After selecting "no", the existing `CONCEPTS.md` is unchanged (modification time and content match the pre-test state).
- No LLM call is made.

---

### TC-006: Overwrite with `--force`

**Title:** Force overwrite of existing `CONCEPTS.md`.

**Preconditions:**
- Research run `rust-history` exists with a populated `sources/` folder.
- `research/rust-history/CONCEPTS.md` already exists.
- Note the current modification time of the existing file.

**Steps:**
1. Launch the TUI by running `./target/release/ragent`.
2. Wait for the main chat interface to render.
3. Focus the input bar and type `/research cluster rust-history --force`.
4. Press `Enter`.

**Test data to enter:**
- Slash command: `/research cluster rust-history --force`

**Expected results:**
- No confirmation prompt is shown.
- The TUI displays progress messages for reading sources and extracting concepts.
- The LLM is called and a new `CONCEPTS.md` is written.
- The modification time of `CONCEPTS.md` is newer than the pre-test value.

---

### TC-007: CLI runner invocation

**Title:** Run cluster from the non-TUI runner.

**Preconditions:**
- Research run `rust-history` exists with a populated `sources/` folder.
- `research/rust-history/CONCEPTS.md` does not already exist.

**Steps:**
1. Open a terminal in the project root.
2. Run the command:
   ```bash
   ./target/release/ragent run --no-tui "research cluster rust-history"
   ```

**Test data to enter:**
- Prompt string: `research cluster rust-history`

**Expected results:**
- The command exits with status code 0.
- The file `research/rust-history/CONCEPTS.md` is created.
- The generated file contains up to 10 concepts with names, definitions, and evidence bullets.

---

### TC-008: Generated artifact structure and limits

**Title:** Verify `CONCEPTS.md` structure and content limits.

**Preconditions:**
- A `CONCEPTS.md` artifact has been generated by one of the successful test cases above.

**Steps:**
1. Open `research/rust-history/CONCEPTS.md` in a text editor or view it with a file-read tool.
2. Count the number of top-level concept headings or entries.
3. Spot-check three concept entries for the required sub-fields.

**Expected results:**
- The file is valid markdown.
- The file contains at least 3 and at most 10 concept entries.
- Each concept entry has a concise name (2-4 words), a definition/description, and 1-2 evidence bullets.
- No two concepts repeat the same verbatim name or definition.

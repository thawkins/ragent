---
status: draft
---

# Research Web-Gather Phase Deadline Fix Manual Test Plan

## Overview

This document describes manual test procedures for the web-gathering phase deadline of
`/research create`: default 60-second bounding, partial-corpus continuation, single
deadline notification, and the live status-bar countdown.

## Prerequisites

1. A local ragent binary exists at `./target/release/ragent` (or equivalent).
2. An LLM provider is configured with a valid API key (e.g. `ANTHROPIC_API_KEY`,
   `OPENAI_API_KEY`, or a local Ollama endpoint) — research runs call the LLM for
   decomposition and synthesis.
3. Web search is available through at least one configured engine (DuckDuckGo, Brave,
   Tavily, Exa, LangSearch, Wikipedia) and network access works.
4. A terminal window of at least 120 columns wide so the full status bar is visible.
5. For TC-005, a way to observe the iterative engine: a run with
   `--iterations 2` (or `--deep`).
6. A stopwatch or the terminal clock for verifying countdown cadence.

## Cleanup

1. Delete any test research folders created under `research/` (e.g.
   `research/webtime-test-*`) after the tests complete.
2. Return any modified `ragent.json` configuration to its original state.
3. Close any TUI sessions used for testing.

## Test Cases

### TC-001: Web phase stops at the 60-second default deadline

**Title:** Default deadline bounds the web-gather phase and the run continues.

**Preconditions:**
- ragent builds and launches.
- Default configuration (no `--web-time` override).
- Network is reachable.

**Steps:**
1. Launch the TUI by running `./target/release/ragent`.
2. Wait for the main chat interface to render.
3. Focus the input bar and type `/research create webtime-test-default <topic>` where
   `<topic>` is a broad query such as `Rust async runtime internals`.
4. Press `Enter`.
5. Start a stopwatch when the run begins.
6. Watch the status bar and the research progress message in the chat log.
7. When the web phase ends, note the elapsed time and whether the run proceeds to the
   analysis/synthesis stages.

**Test data to enter:**
- Slash command: `/research create webtime-test-default Rust async runtime internals`

**Expected results:**
- The web-gathering phase ends at or shortly after 60 seconds (an overshoot of a few
  seconds is acceptable while an in-flight fetch completes; it must not exceed the
  per-fetch timeout).
- The run moves to the next stage (ingestion/analysis/synthesis) without user
  intervention.
- The run completes and writes `research/webtime-test-default/RESEARCH.md`.
- The research progress message contains exactly one deadline notice stating the web-phase
  deadline was reached and how many sources were captured.

---

### TC-002: Partial sources are used as the corpus after truncation

**Title:** Sources captured before the deadline feed synthesis.

**Preconditions:**
- ragent builds and launches.
- Default configuration (60-second deadline).
- Network is reachable; a slow or large topic is chosen so that more candidate sources
  exist than can be fetched in 60 seconds.

**Steps:**
1. Launch the TUI by running `./target/release/ragent`.
2. Focus the input bar and type
   `/research create webtime-test-partial <slow topic>` where `<slow topic>` is a query
   likely to yield many results, e.g. `history of distributed systems`.
3. Press `Enter` and let the run proceed past the deadline.
4. Wait for the run to complete.
5. Open `research/webtime-test-partial/RESEARCH.md` in an editor.
6. Inspect the References section and the in-text citations.

**Test data to enter:**
- Slash command: `/research create webtime-test-partial history of distributed systems`

**Expected results:**
- The capture table in the progress message shows a non-zero captured count at the moment
  the deadline notice appears.
- `RESEARCH.md` exists and is non-empty.
- Every reference listed in the References section corresponds to a source captured
  before the deadline; no references to sources that were only discovered but not
  captured.
- The report body cites those captured sources.

---

### TC-003: No run is aborted and no empty corpus is produced on truncation

**Title:** Deadline truncation never aborts the run or discards partial results.

**Preconditions:**
- Same as TC-002, with a topic guaranteed to produce at least one captured source within
  60 seconds (verify with a quick prior run or by watching the capture table).

**Steps:**
1. Launch the TUI.
2. Focus the input bar and type
   `/research create webtime-test-noabort <topic with quick results>`.
3. Press `Enter`.
4. Observe the full run from web phase through completion.

**Test data to enter:**
- Slash command: `/research create webtime-test-noabort what is Rust ownership`

**Expected results:**
- The run does not fail, restart, or present an error at the 60-second mark.
- At least one source is captured before the deadline and appears in the final report.
- The final message reports completion with a non-zero source count.

---

### TC-004: `--web-time 0` disables the deadline

**Title:** Deadline disabled runs the web phase to natural completion.

**Preconditions:**
- A CLI session (TUI not required).
- Network reachable.

**Steps:**
1. In a terminal, run:
   `./target/release/ragent research create webtime-test-nolimit --web-time 0 "Rust async runtime internals"`
   (adjusting flags to the CLI's accepted form if it prompts for a research id).
2. Monitor the run until the web phase finishes.
3. Note whether any deadline notice appears and how long the web phase ran.

**Test data to enter:**
- Command: `./target/release/ragent research create webtime-test-nolimit --web-time 0 "Rust async runtime internals"`

**Expected results:**
- No web-phase deadline notice appears at 60 seconds.
- The web phase runs to natural completion (may exceed 60 seconds).
- The run completes normally with `RESEARCH.md` written.

---

### TC-005: Iterative engine runs also respect the deadline

**Title:** Each iteration of a multi-iteration run is bounded by the web-phase deadline.

**Preconditions:**
- ragent builds and launches.
- The CLI/TUI front-end supports `--iterations 2` or deep mode for `/research create`.

**Steps:**
1. Launch the TUI.
2. Focus the input bar and type
   `/research create webtime-test-iter <topic> --iterations 2` (or invoke the equivalent
   deep-mode form).
3. Press `Enter` and observe each iteration's web-gathering phase.
4. Note the duration of each iteration's web phase.

**Test data to enter:**
- Slash command: `/research create webtime-test-iter recent AI safety research --iterations 2`

**Expected results:**
- Each iteration's web-gathering phase ends at or shortly after the configured deadline
  (60 seconds by default), not at an unbounded duration.
- Each iteration proceeds to its next stage with its partial sources.
- The run completes overall.

---

### TC-006: Exactly one deadline notification per run

**Title:** The deadline notice is emitted once, with the correct captured count.

**Preconditions:**
- ragent builds and launches.
- A topic likely to be truncated mid-search (many results, slow network or slow fetches).

**Steps:**
1. Launch the TUI.
2. Focus the input bar and type
   `/research create webtime-test-dedup <topic with many results>`.
3. Press `Enter` and let the run pass the deadline.
4. Scroll the research progress message and count occurrences of the deadline notice.

**Test data to enter:**
- Slash command: `/research create webtime-test-dedup machine learning interpretability surveys`

**Expected results:**
- Exactly one deadline notice appears in the progress message.
- The notice reports the number of sources actually captured (matching the capture
  table's final count at truncation), not zero when sources were captured.

---

### TC-007: Live countdown appears in the status-bar wait message

**Title:** A per-second countdown of remaining web-phase time shows in the top-right wait status.

**Preconditions:**
- ragent builds and launches.
- A terminal at least 120 columns wide so the status-bar right segment is visible.
- Default 60-second deadline.

**Steps:**
1. Launch the TUI.
2. Note the status bar layout: top-right segment shows the session status / wait message.
3. Focus the input bar and type
   `/research create webtime-test-countdown <topic>`.
4. Press `Enter`.
5. Immediately observe the top-right wait message.
6. Watch for at least 10 seconds, noting the displayed remaining time at 1-second
   intervals.
7. Continue watching until the web phase ends.

**Test data to enter:**
- Slash command: `/research create webtime-test-countdown vector database comparison`

**Expected results:**
- While the web phase is running, the wait message shows a countdown of remaining time
  (e.g. `[wait] research: webtime-test-countdown — web (⟳) — 0:47`).
- The countdown decreases by approximately 1 second per second; it never increases.
- The countdown reaches 0:00 at or near the moment the deadline truncates the phase.
- The countdown does not appear for other phases (decompose, analysis, synthesis).

---

### TC-008: Countdown clears when the web phase ends

**Title:** Countdown disappears on phase end regardless of outcome.

**Preconditions:**
- Same as TC-007.

**Steps:**
1. Launch the TUI.
2. Run `/research create webtime-test-clear <topic>`.
3. Watch the status bar through the entire web phase.
4. Note the status-bar right segment immediately after the deadline truncates the phase
   (and again after the run completes).

**Test data to enter:**
- Slash command: `/research create webtime-test-clear Rust error handling patterns`

**Expected results:**
- When the web phase ends (deadline hit, completed early, or failed), the countdown is
  removed from the wait message.
- The wait message reverts to the normal research-wait text (next phase) and, after the
  run completes, to the ready state.
- No stale countdown or `0:00` remnant remains visible at any point after the phase ends.

---

### TC-009: Custom `--web-time` value is honoured and reflected in the countdown

**Title:** Non-default deadline values bound the phase and drive the countdown.

**Preconditions:**
- ragent builds and launches.
- The `/research create` front-end accepts a web-time override.

**Steps:**
1. Launch the TUI.
2. Focus the input bar and type
   `/research create webtime-test-custom <topic> --web-time 20`.
3. Press `Enter`.
4. Watch the status bar countdown and note when the phase truncates.

**Test data to enter:**
- Slash command: `/research create webtime-test-custom Rust trait objects --web-time 20`

**Expected results:**
- The web phase truncates at approximately 20 seconds (plus in-flight fetch overshoot).
- The countdown starts from 0:20 and decreases to 0:00.
- The deadline notice reports a 20-second deadline.
- A `--web-time 0` variant (repeat with `--web-time 0`) shows no countdown and no
  deadline notice.

---

### TC-010: Countdown does not cause idle redraws when idle

**Title:** No countdown-related activity when no research run is active.

**Preconditions:**
- ragent builds and launches.
- No research run is running.

**Steps:**
1. Launch the TUI and let it sit idle at the ready prompt for 60 seconds.
2. Observe CPU usage (e.g. via `top -p $(pgrep ragent)`) over that minute.
3. Then start a research run and observe CPU during the countdown window for comparison.

**Test data to enter:**
- None (idle observation).

**Expected results:**
- Idle CPU usage is negligible and not increased relative to a build without the
  countdown feature.
- During the active countdown the loop wakes about once per second, which is visible as a
  small CPU bump but not a busy spin.
- After the run completes, idle CPU returns to the pre-run baseline.
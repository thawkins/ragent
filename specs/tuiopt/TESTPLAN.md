---
status: draft
---
# TUI Performance Optimisation Manual Test Plan

## Prerequisites

Before running these manual tests, ensure the following environment is in place:

1. A checkout of the ragent repository at the commit under test.
2. Rust toolchain 1.85+ installed and the project builds with `cargo build -p ragent-tui`.
3. A terminal with at least 120×40 characters and true-colour support preferred.
4. A local Ollama instance running at `http://127.0.0.1:11434` with at least one small model pulled (for example, `qwen2.5:0.5b` or `phi3:mini`). This provides a slow-enough provider to observe UI responsiveness.
5. An empty or disposable working directory so the generated sessions do not pollute an important project.
6. The ragent binary built and available at `target/debug/ragent`.
7. Optional: a stopwatch or the `time` command to time startup and recovery from model discovery.

## Test Cases

### TC-001 — Redraw efficiency while idle

**Title:** Verify the TUI does not consume CPU when no input or state changes occur.

**Preconditions:**
- The terminal is at least 120×40.
- No LLM call is in progress.
- The TUI is open at the main chat screen with a fresh session.

**Steps:**
1. Start the TUI from a new session:
   - In the terminal, run `cd /tmp && rm -rf tuiopt-test && mkdir tuiopt-test && cd tuiopt-test`.
   - Run `/home/thawkins/Projects/ragent/target/debug/ragent --no-tui --log-level debug 2>> /tmp/ragent-idle.log &` is **not** the correct path; instead run the TUI interactively:
     ```
     /home/thawkins/Projects/ragent/target/debug/ragent
     ```
2. Wait for the main chat screen to appear. Do not type anything.
3. Leave the TUI idle for 60 seconds.
4. In another terminal, observe CPU usage with:
   ```
   top -p $(pgrep -f "target/debug/ragent") -n 10 -d 2
   ```
5. Press `q` to exit the TUI.

**Test data to enter:**
- None.

**Expected results:**
- The `ragent` process should consume less than 5% CPU during the idle 60-second window.
- The log file should not contain a continuous stream of `render` or `poll` trace events.
- The terminal should not flicker or redraw without cause.

---

### TC-002 — Model discovery does not freeze the UI

**Title:** Verify that opening the model picker while discovery runs remains responsive.

**Preconditions:**
- The Ollama provider is enabled in `ragent.json` (or the local-first default resolves to Ollama).
- The TUI is at the main chat screen.
- A stopwatch is available.

**Steps:**
1. Start the TUI as in TC-001, step 1.
2. Press the key bound to open the model picker. The default is the `/model` slash command or the configured key binding (often `Ctrl+M`). If using the slash command:
   - Type `/model` into the input box at the bottom of the screen.
   - Press `Enter`.
3. Observe the model picker dialog.
4. If the provider list is not cached, the dialog should show a loading indicator (for example, `Scanning providers...`).
5. While the loading indicator is visible, press the `Down` arrow key three times.
6. Press `Esc` to close the picker.
7. Wait for the background discovery to complete.
8. Reopen the model picker with `/model` and press `Enter`.

**Test data to enter:**
- `/model` in the input box.
- `Down` arrow pressed three times during loading.

**Expected results:**
- The UI responds to `Down` arrow presses during loading (selection moves, no freeze).
- No `block_on` or `block_in_place` log warnings appear at `warn` level.
- After reopening, the provider/model list is populated.

---

### TC-003 — Large message history scroll performance

**Title:** Verify that scrolling through a long conversation does not stutter.

**Preconditions:**
- The TUI is at the main chat screen.
- A session with at least 500 messages exists, or a synthetic session can be imported. If no such session exists, create one by sending repeated short prompts and allowing the model to reply, or by importing a prepared SQLite session file.
- The terminal is at least 120×40.

**Steps:**
1. Start the TUI as in TC-001.
2. Resume the large session:
   - Type `/session list`.
   - Press `Enter`.
   - Select the large session with arrow keys and press `Enter`.
3. Once the chat history loads, press `Page Up` ten times.
4. Press `Page Down` ten times.
5. Press `Home` to jump to the top.
6. Press `End` to jump to the bottom.

**Test data to enter:**
- `/session list` in the input box.
- Cursor keys and `Page Up`/`Page Down`/`Home`/`End` for navigation.

**Expected results:**
- Each scroll action completes within 200 ms and the UI remains responsive.
- CPU usage spikes during the scroll sequence should return to idle levels within one second after the last keypress.
- No duplicate or missing message lines appear during scrolling.

---

### TC-004 — Active agents panel with many sub-agents

**Title:** Verify the active-agents subpanel renders smoothly with many running tasks.

**Preconditions:**
- The TUI is at the main chat screen.
- The active-agents panel is enabled in `ragent.json` (`tool_visibility.agents: true`).
- A way to spawn background sub-agents exists, such as the `/swarm` command or a prompt that calls `new_agent`.

**Steps:**
1. Start the TUI as in TC-001.
2. Open the active-agents panel if it is not already visible. The default toggle is usually `/agents` or a key binding (for example, `Ctrl+A`).
3. Spawn 20 background sub-agents:
   - Type `/swarm create a one-sentence summary of the README file`.
   - Press `Enter`.
4. Wait for the swarm tasks to appear in the active-agents panel.
5. With the panel visible, press `Tab` to cycle through UI regions, then `Down`/`Up` to scroll the agent list.
6. Let all sub-agents complete.
7. Press the active-agents toggle again to close the panel.

**Test data to enter:**
- `/swarm create a one-sentence summary of the README file` in the input box.
- `Tab`, `Down`, `Up`, and the active-agents toggle key.

**Expected results:**
- The active-agents panel updates as tasks start and finish without freezing the chat area.
- `Tab` and arrow-key navigation remain responsive while the panel is open.
- CPU usage does not remain high after all tasks are complete.

---

### TC-005 — Teams panel without per-frame disk reads

**Title:** Verify the teams panel loads from cached state and remains responsive.

**Preconditions:**
- The teams feature is enabled in `ragent.json` (`tool_visibility.teams: true`).
- A team has been created previously, or one can be created during the test.
- The TUI is at the main chat screen.

**Steps:**
1. Start the TUI as in TC-001.
2. Create a team:
   - Type `/team create perf-test-team`.
   - Press `Enter`.
3. Open the teams panel if it is not already visible. The default toggle is usually `/team list` or a key binding (for example, `Ctrl+T`).
4. Add a team task:
   - Type `/team task "Verify responsiveness"`.
   - Press `Enter`.
5. Press `Down` three times in the teams panel.
6. Press `Esc` or the teams toggle to close the panel.
7. Reopen the teams panel.

**Test data to enter:**
- `/team create perf-test-team` in the input box.
- `/team task "Verify responsiveness"` in the input box.
- `Down` arrow pressed three times in the teams panel.

**Expected results:**
- The teams panel opens and renders immediately on both first and subsequent opens.
- No visible pause or spinner appears while reading team/task data.
- Navigation with `Down` arrow is immediate.

---

### TC-006 — Markdown-heavy streaming message

**Title:** Verify markdown rendering during streaming does not jank the input box.

**Preconditions:**
- A provider is configured that returns long, markdown-formatted responses (any provider works; a cloud provider may produce longer output more easily).
- The TUI is at the main chat screen.

**Steps:**
1. Start the TUI as in TC-001.
2. Type a prompt that asks for a long markdown response:
   - Enter the prompt:
     ```
     Write a 1000-word guide to Rust error handling. Use markdown headers, bullet lists, code blocks, and bold text.
     ```
3. Press `Enter` to send the prompt.
4. While the response streams, try typing characters into the input box at the bottom.
5. Once the response finishes, press `Up`/`Down` to scroll through the rendered message.

**Test data to enter:**
- The prompt text above.
- Random characters typed into the input box while streaming.

**Expected results:**
- Characters typed while streaming appear in the input box within 100 ms.
- No visible freeze occurs when the markdown pipeline runs.
- Scrolling the completed message is smooth.

---

### TC-007 — Log panel flood recovery

**Title:** Verify the TUI recovers quickly from a flood of tracing log records.

**Preconditions:**
- The TUI is running at the main chat screen.
- The log panel is visible (toggle with the configured key, often `Ctrl+L`, or via slash command `/logs`).
- A way to trigger many log messages exists, such as starting a swarm or running a command that produces verbose provider output.

**Steps:**
1. Start the TUI as in TC-001.
2. Open the log panel.
3. Trigger a high-volume log source. For example, run a swarm with a verbose provider:
   - Type `/swarm summarize the current project in one paragraph`.
   - Press `Enter`.
4. While logs are streaming, press `Down` and `Up` in the log panel every two seconds.
5. After the operation completes, wait five seconds.
6. Press `Esc` to close the log panel.

**Test data to enter:**
- `/swarm summarize the current project in one paragraph` in the input box.
- `Down` and `Up` arrow keys every two seconds.

**Expected results:**
- Arrow-key navigation in the log panel remains responsive even while records arrive.
- After the flood stops, the UI returns to idle CPU levels within three seconds.
- The log panel does not grow unbounded beyond the configured maximum.

## Cleanup

After completing the manual tests, perform the following teardown steps:

1. Exit the TUI if it is still running by pressing `q` or using the configured quit binding.
2. Remove the disposable working directory:
   ```
   rm -rf /tmp/tuiopt-test
   ```
3. Delete any temporary session files created during the tests from the ragent data directory (default `~/.local/share/ragent/`). If you are unsure which sessions belong to this test, list them with the CLI:
   ```
   /home/thawkins/Projects/ragent/target/debug/ragent session list
   ```
   Then delete the test sessions by name or id.
4. Remove any temporary log files generated during the tests:
   ```
   rm -f /tmp/ragent-idle.log
   ```
5. If a test team was created, clean it up with the CLI or by deleting the team store under the ragent data directory.
6. Restore the original `ragent.json` if any temporary provider overrides were used.

---
status: draft
spec_id: piegap
title: "Manual Test Plan — Pie Feature Gap"
created: 2026-08-13
---
# Manual Test Plan — Pie Feature Gap

This is a **manual** test plan for verifying the pie feature gap
implementation. Each test case describes preconditions, step-by-step
instructions, test data, and expected results. No automated test code is
included.

**FR-009 (LSP Integration)** and **FR-010 (Compiled Extension Registry)** are
marked "NOT Required" in the spec. No test cases are defined for them.

## Prerequisites

### Environment

1. **Rust toolchain**: Rust 1.85+ (edition 2024) installed and on `PATH`.
2. **ragent binary**: Build the debug binary:
   ```bash
   cd ~/Projects/ragent
   cargo build
   ```

   The binary is at `target/debug/ragent`.
3. **Working directory**: A test project directory with a few source files:
   ```bash
   mkdir -p ~/Projects/piegap-test/src
   cd ~/Projects/piegap-test
   echo 'fn main() { println!("hello"); }' > src/main.rs
   ```
4. **Provider configured**: At least one LLM provider configured (e.g.,
   `export ANTHROPIC_API_KEY=sk-ant-...` or `export OPENAI_API_KEY=sk-...`).
5. **ragent config**: A `.ragent/ragent.json` in the test project with the gap
   feature being tested enabled (see each test case for the specific config).

### Feature-Specific Prerequisites

- **Web UI tests**: A modern browser (Firefox or Chromium) installed.
- **MCP notification tests**: A minimal MCP server that can push notification
  frames (can be a simple Python stdio script).
- **OpenAI Responses tests**: A local OpenAI-compatible server that implements
  the Responses API, or a real OpenAI API key.

## Test Cases

### TC-001 — Dynamic Trigger Rule Creation and Firing

**Preconditions**:

- ragent built with the dynamic triggers feature enabled
- `ragent.json` contains: `{"triggers": {"enabled": true, "poll_interval_secs": 5}}`

**Steps**:

1. Launch ragent TUI: `ragent`
2. Type the following prompt and press Enter:
   ```
   when ~/piegap-test/trigger.flag exists, print its contents
   ```
3. Wait for the agent to confirm the trigger rule was created (a `[Trigger]`
   system line should appear in the feed).
4. In a separate terminal, create the flag file:
   ```bash
   echo "trigger fired!" > ~/piegap-test/trigger.flag
   ```
5. Wait up to 15 seconds (poll interval is 5s in test config).

**Test data**: `trigger.flag` content = `trigger fired!`

**Expected results**:

- The TUI feed shows a trigger fired line (e.g., `[Trigger … fired]`).
- A sub-agent runs and prints the file contents.
- The trigger rule does NOT fire again (fire-once default).
- `/triggers` shows the rule with `fired` status.

---

### TC-002 — `/triggers` Slash Commands

**Preconditions**:

- ragent running with at least one dynamic trigger rule created (from TC-001).

**Steps**:

1. Type `/triggers` and press Enter.
2. Verify the output lists the existing rule(s) with id, condition, action,
   enabled state, and fire-once flag.
3. Type `/triggers disable <id>` (using the id from step 2) and press Enter.
4. Type `/triggers` again and verify the rule shows as disabled.
5. Type `/triggers enable <id>` and press Enter.
6. Type `/triggers remove <id>` and press Enter.
7. Type `/triggers` and verify the rule is gone.

**Expected results**:

- `/triggers` lists rules with all metadata.
- Disable/enable toggles the `enabled` field.
- Remove deletes the rule from the session and the listing.

---

### TC-003 — MCP Notification Push Event

**Preconditions**:

- ragent built with MCP notification hooks enabled
- `ragent.json` contains:
  ```json
  {
    "mcp_notifications": {"enabled": true},
    "mcp": {
      "servers": {
        "test-server": {
          "command": "python3",
          "args": ["~/piegap-test/mcp_push.py"],
          "notification_mode": "inject_summary"
        }
      }
    }
  }
  ```
- A minimal MCP server script at `~/piegap-test/mcp_push.py` that sends a
  notification frame after a short delay.

**Steps**:

1. Launch ragent TUI: `ragent`
2. Wait for the MCP server to start and push a notification frame.
3. Observe the TUI feed for the injected summary.
4. Type `/triggers` and press Enter.
5. Verify the MCP notification appears as a trigger source.

**Test data**: The MCP server pushes a notification with a short text payload
(e.g., `{"summary": "Build completed successfully"}`).

**Expected results**:

- The notification is normalized into a trigger envelope.
- With `inject_summary` mode, a bounded summary appears in the chat feed
  without a model call.
- The raw notification payload is NOT persisted as chat content.
- `/triggers` shows the MCP notification as a trigger source.
- Duplicate notifications are deduplicated (sending the same notification twice
  produces only one injected summary).

---

### TC-004 — Stateful Loop with Triage Inbox

**Preconditions**:

- ragent built with stateful loops + inbox enabled
- `ragent.json` contains: `{"loops": {"enabled": true}, "inbox": {"enabled": true}}`

**Steps**:

1. Launch ragent TUI: `ragent`
2. Type the following and press Enter:
   ```
   /cron add --stateful "*/2 * * * *" check if ~/piegap-test/trigger.flag exists and report it as a finding
   ```
3. Wait 2–3 minutes for the cron tick to fire.
4. Type `/inbox` and press Enter.
5. Verify at least one new finding is listed with a number.
6. Type `/inbox claim 1` and press Enter.
7. Verify the finding is promoted into the main chat as a real agent turn.
8. Type `/inbox dismiss 2` (if a second finding exists) and press Enter.
9. Type `/inbox all` and press Enter.
10. Verify claimed and dismissed entries are shown with their status.

**Test data**: `trigger.flag` from TC-001 should still exist.

**Expected results**:

- `/cron list` shows the job with a `[stateful]` marker.
- After the tick, `/inbox` shows at least one `new` finding.
- `/inbox claim N` promotes the finding into the main chat.
- `/inbox dismiss N` marks the finding as dismissed.
- `/inbox all` shows all entries with their lifecycle status.

---

### TC-005 — Lifecycle Hook (Command)

**Preconditions**:

- ragent built with lifecycle hooks enabled
- `ragent.json` contains: `{"hooks": {"enabled": true}}`
- A hooks config file at `~/.config/ragent/hooks.json`:
  ```json
  {
    "hooks": [
      {
        "event": "tool_end",
        "tool": "bash",
        "command": "echo \"$RAGENT_TOOL_NAME done\" >> ~/piegap-test/hooks.log",
        "timeout_ms": 3000
      }
    ]
  }
  ```

**Steps**:

1. Launch ragent TUI: `ragent`
2. Type a prompt that causes the agent to run a bash command:
   ```
   run `ls ~/piegap-test` and show me the result
   ```
3. Wait for the agent turn to complete.
4. In a separate terminal, check the log file:
   ```bash
   cat ~/piegap-test/hooks.log
   ```

**Expected results**:

- The hooks.log file contains a line like `bash done`.
- The hook does not interfere with the agent turn (no error in the TUI).
- The hook only fires for `bash` tool calls (not for read/write/etc.).

---

### TC-006 — Lifecycle Hook (Webhook)

**Preconditions**:

- ragent built with lifecycle hooks enabled
- A webhook receiver running locally (e.g., `python3 -m http.server 9876` or a
  `nc -l 9876` listener)
- Hooks config:
  ```json
  {
    "hooks": [
      {
        "event": "turn_end",
        "webhook": "http://127.0.0.1:9876/hook",
        "timeout_ms": 5000
      }
    ]
  }
  ```

**Steps**:

1. Launch ragent TUI: `ragent`
2. Type a short prompt: `hello` and press Enter.
3. Wait for the turn to complete.
4. Check the webhook receiver for an incoming POST request.

**Expected results**:

- The webhook receiver receives a `POST` with `Content-Type: application/json`.
- The JSON body contains `event`, `session_id`, `cwd`, `model_provider`,
  `model_id` fields.
- Long values are truncated/bounded in the payload.

---

### TC-007 — Bug Report Generation

**Preconditions**:

- ragent built with bug-report feature enabled
- At least one completed agent turn in the current session.

**Steps**:

1. Launch ragent TUI: `ragent` in the `~/piegap-test` directory
2. Type a prompt and wait for a response: `hello`
3. Type `/bug-report` and press Enter.
4. Note the file path reported in the TUI output.
5. In a separate terminal, read the generated file:
   ```bash
   cat <reported-path>
   ```

**Expected results**:

- The TUI reports a file path under the `log/` folder in the root project
  directory (e.g., `~/piegap-test/log/bug-report-<timestamp>.md`).
- The file contains a diagnostic section (session id, model, agent, tool count,
  cost summary).
- The file contains a transcript section with the conversation.
- Any API keys or secret-like strings are redacted (replaced with `***` or
  similar).

---

### TC-008 — Reusable Prompt Template

**Preconditions**:

- ragent built with prompt templates enabled
- A template file at `~/.config/ragent/templates/review.md`:
  ```markdown
  ---
  name: review
  description: Code review prompt
  ---
  Review the code in {{WORKING_DIR}} for bugs and suggest improvements.
  ```

**Steps**:

1. Launch ragent TUI: `ragent` in `~/Projects/piegap-test`
2. Type `/template review` and press Enter.
3. Wait for the agent to respond.
4. Type `/template` (no argument) and press Enter.

**Expected results**:

- The agent receives a prompt with `{{WORKING_DIR}}` substituted by the
  current working directory path.
- The agent's response is a code review of the project.
- `/template` without arguments lists available templates.

---

### TC-009 — `/undo` Remove Last Turn

**Preconditions**:

- ragent built with `/undo` enabled
- At least one completed user/assistant turn in the current session.

**Steps**:

1. Launch ragent TUI: `ragent`
2. Type: `what is 2+2` and press Enter. Wait for the response.
3. Type `/undo` and press Enter.
4. Type `/history` and press Enter (or check the session feed).

**Expected results**:

- The most recent user/assistant turn pair is removed from the session.
- A confirmation message appears in the TUI.
- The session feed no longer shows the removed turn.
- The removal persists (restarting the session shows the turn is gone).

---

### TC-010 — Session Naming

**Preconditions**:

- ragent built with session naming enabled.

**Steps**:

1. Launch ragent TUI: `ragent`
2. Type `/name My Test Session` and press Enter.
3. Verify the TUI status bar or banner shows the new name.
4. Exit ragent (Ctrl+C or `/quit`).
5. Relaunch ragent and resume the session:
   ```bash
   ragent session list
   ```
6. Verify the session appears with the name "My Test Session".
7. Type `/name` (empty argument) and press Enter.
8. Verify the name is cleared.

**Expected results**:

- `/name <text>` sets a display name on the session.
- The name appears in `ragent session list` output.
- `/name` with no argument clears the name.
- The name persists across session restarts.

---

### TC-011 — Goal-Based Autonomous Stop Hook

**Preconditions**:

- ragent built with goal stop hook enabled
- `ragent.json` contains: `{"goal": {"enabled": true, "max_continuations": 5}}`

**Steps**:

1. Launch ragent TUI: `ragent` in a test project
2. Type: `/goal create a file called done.txt with the content "complete"`
3. Wait for the agent to work on the goal.
4. Observe the feed for goal evaluation messages after each turn.
5. Once `done.txt` is created, wait for the next evaluation.
6. Verify the agent stops (the loop ends) with a "goal achieved" message.
7. Type `/goal` and press Enter to check goal status.
8. In a new session, type `/goal create an impossible file at /nonexistent/path`
   and press Enter.
9. Wait for `max_continuations` turns to elapse.
10. Type `/goal pause` and press Enter. Verify the goal is paused.
11. Type `/goal resume` and press Enter. Verify the goal resumes.
12. Type `/goal clear` and press Enter. Verify the goal is cleared.

**Test data**: The goal condition is "create a file called done.txt with the
content 'complete'".

**Expected results**:

- After each successful model turn, a goal evaluation runs.
- When the file is created and matches the condition, the agent stops.
- The goal state shows "achieved" in `/goal` output.
- If the goal is not achieved within `max_continuations`, the loop pauses with
  a "budget_limited" status.
- `/goal pause`, `/goal resume`, and `/goal clear` work as expected.

---

### TC-012 — Standalone Feature Independence

**Preconditions**:

- ragent source code checked out
- All gap features implemented behind feature gates.

**Steps**:

1. Edit `ragent.json` to disable ALL gap features:
   ```json
   {
     "triggers": {"enabled": false},
     "mcp_notifications": {"enabled": false},
     "loops": {"enabled": false},
     "inbox": {"enabled": false},
     "hooks": {"enabled": false},
     "goal": {"enabled": false},
     "templates": {"enabled": false},
     "web_ui": {"enabled": false},
     "session_archive": {"enabled": false},
     "bug_report": {"enabled": false},
     "undo": {"enabled": false},
     "session_naming": {"enabled": false}
   }
   ```
2. Launch ragent TUI: `ragent`
3. Type: `hello` and press Enter. Wait for a response.
4. Type: `/triggers` and press Enter (should show "feature disabled" or be
   absent from the command list).
5. Type: `/inbox` and press Enter (should show "feature disabled" or be
   absent).
6. Type: `/bug-report` and press Enter (should show "feature disabled" or be
   absent).
7. Type: `/template` and press Enter (should show "feature disabled" or be
   absent).
8. Type: `/goal test` and press Enter (should show "feature disabled" or be
   absent).
9. Exit ragent.
10. Re-enable only ONE feature (e.g., hooks) in `ragent.json`:
    ```json
    {"hooks": {"enabled": true}}
    ```
11. Launch ragent again.
12. Verify hooks work but other features remain disabled.

**Expected results**:

- With all features disabled, ragent functions normally for basic chat, file
  operations, and shell commands.
- Disabled features either show a clear "disabled" message or do not appear
  in `/help` output.
- Enabling a single feature does not require enabling any other feature.
- No compilation errors or runtime panics when features are selectively
  enabled/disabled.
- Existing ragent features (cron, compaction, permissions, MCP client, code
  index, memory, teams, research, skills, prompt optimization) continue to
  work regardless of gap feature enable/disable state.

---

### TC-013 — Portable Session Archive Export/Import

**Preconditions**:

- ragent built with session archive enabled
- An existing session with at least 2 turns and (optionally) one dynamic
  trigger rule.

**Steps**:

1. Launch ragent TUI: `ragent`
2. Complete at least 2 turns in the session.
3. Create a dynamic trigger rule (if triggers are enabled).
4. Type: `/session export ~/piegap-test/session.ragentsession` and press Enter.
5. Exit ragent.
6. In a separate terminal, inspect the archive:
   ```bash
   tar tf ~/piegap-test/session.ragentsession
   ```
7. Launch ragent in a fresh project directory:
   ```bash
   mkdir -p ~/piegap-test/import-test
   cd ~/piegap-test/import-test
   ragent session import ~/piegap-test/session.ragentsession
   ```
8. When prompted about automation activation, choose "no" (disabled by
   default).
9. Launch ragent and resume the imported session.

**Expected results**:

- The export produces a `.ragentsession` archive file.
- `tar tf` shows: `manifest.json`, `session.jsonl`, and optional sidecar files.
- The import creates a new session with the same transcript.
- Automation sidecars (triggers/cron) are imported but disabled by default.
- The manifest includes SHA-256 checksums for the session transcript.
- The manifest includes a sensitivity warning about preserved transcript.

---

### TC-014 — Browser-Based Web UI

**Preconditions**:

- ragent built with web UI enabled
- A browser installed (Firefox or Chromium)

**Steps**:

1. Start ragent in web mode:
   ```bash
   ragent web --port 9101
   ```
2. Open a browser and navigate to `http://127.0.0.1:9101`.
3. Verify the web UI loads with an input box and feed area.
4. Type a prompt in the web input box: `hello, what can you do?`
5. Click the send button (or press Enter).
6. Wait for the agent response to appear in the feed.
7. Type `/model` in the web input and verify the model picker appears.
8. Verify the abort button works (send a prompt, then click abort mid-response).
9. Close the browser tab.
10. Verify the server rejects a non-loopback bind by default:
    ```bash
    ragent web --host 0.0.0.0 --port 9101
    ```

    (Should refuse or warn about non-loopback binding.)

**Expected results**:

- The web UI renders at `http://127.0.0.1:9101`.
- Prompts can be submitted and responses stream into the feed.
- Slash commands work from the web input.
- The model picker is accessible.
- Abort cancels an in-flight turn.
- Non-loopback bind is rejected without an explicit auth token.

---

### TC-015 — OpenAI Responses API Provider (Local Model)

**Preconditions**:

- ragent built with the OpenAI Responses provider
- A local OpenAI-compatible server running that implements the Responses API
  (or a real OpenAI API key for testing)
- A model definition in `~/.config/ragent/models.json`:
  ```json
  {
    "models": [
      {
        "id": "test-responses",
        "name": "Test Responses Model",
        "api": "openai-responses",
        "provider": "local",
        "baseUrl": "http://127.0.0.1:8000/v1",
        "reasoning": true,
        "contextWindow": 100000,
        "maxTokens": 384000
      }
    ]
  }
  ```

**Steps**:

1. Launch ragent TUI: `ragent --provider local --model test-responses`
2. Type: `what is 2+2?` and press Enter.
3. Wait for the response.
4. Type: `/cost` and press Enter.
5. Run several turns, then check `/cost` again.
6. If testing with a DS4-style server, restart the server mid-session and send
   another prompt.

**Expected results**:

- The model responds via the Responses API endpoint.
- `/cost` shows cache read and cache write token counts.
- After a server restart, the turn succeeds (one transparent 409 retry) and
  cache reads resume from disk checkpoints.
- Assistant reasoning blocks are replayed in subsequent turns (no cache
  invalidation).

---

### TC-016 — Configuration Discovery

**Preconditions**:

- ragent built with at least one gap feature enabled (e.g., hooks).

**Steps**:

1. Create a project-local config at `~/Projects/piegap-test/.ragent/ragent.json`:
   ```json
   {"hooks": {"enabled": true, "opt_in_project_local": true}}
   ```
2. Create a user-global config at `~/.config/ragent/config.json`:
   ```json
   {"hooks": {"enabled": false}}
   ```
3. Launch ragent TUI: `ragent` in `~/Projects/piegap-test`
4. Verify that the project-local config takes precedence (hooks enabled).
5. Remove the project-local config:
   ```bash
   rm ~/Projects/piegap-test/.ragent/ragent.json
   ```
6. Relaunch ragent in the same directory.
7. Verify the user-global config is used (hooks disabled).
8. Test with a `.ragent/ragent.jsonc` (JSONC) file to verify JSONC parsing.

**Expected results**:

- Project-local `.ragent/ragent.json` takes precedence over
  `~/.config/ragent/config.json`.
- When no project-local config exists, the user-global config is used.
- `.ragent/ragent.jsonc` (JSON with comments) is parsed correctly.
- Configuration is discovered following the standard ragent config discovery
  pattern.

---

## Cleanup

After all manual tests are complete:

1. **Remove test project**:
   ```bash
   rm -rf ~/Projects/piegap-test
   ```
2. **Remove test config**:
   ```bash
   rm -f ~/.config/ragent/hooks.json
   rm -f ~/.config/ragent/templates/review.md
   rm -f ~/.config/ragent/models.json
   rm -f ~/.config/ragent/extensions.json
   rm -f ~/.config/ragent/config.json
   ```
3. **Remove generated artifacts**:
   ```bash
   rm -f ~/piegap-test/hooks.log
   rm -f ~/piegap-test/session.ragentsession
   rm -f ~/piegap-test/trigger.flag
   rm -f ~/piegap-test/done.txt
   rm -f ~/piegap-test/mcp_push.py
   rm -rf ~/piegap-test/log/
   ```
4. **Reset ragent config**: Restore `ragent.json` to its pre-test state or
   remove the test `ragent.json` from the test project directory.
5. **Kill any lingering servers**: Stop any local model servers, webhook
   receivers, or ragent web UI processes started during testing.
6. **Clean ragent state** (optional): If test sessions clutter the session
   list, archive or delete them via `ragent session` commands.

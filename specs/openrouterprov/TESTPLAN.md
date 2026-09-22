---
status: draft
---

# Manual Test Plan: OpenRouter Model Provider

**Spec ID:** `openrouterprov`

## Scope

This document describes the manual verification of the `openrouter` LLM
provider in ragent: model discovery against the live OpenRouter catalog,
secure token storage via `ragent auth`, chat/streaming/tool-use parity with
`ollama_cloud`, thinking/reasoning-level behaviour, configuration overrides,
and failure/safety paths (missing key, network loss, key redaction, no chat
retry).

## Prerequisites

1. Build ragent from source with the OpenRouter provider implementation:
   ```bash
   cargo build --release
   # Binary at target/release/ragent
   ```
2. Have an OpenRouter API key available:
   - Create one at <https://openrouter.ai/settings/keys>.
   - Keys start with `sk-or-v1-`.
3. Internet access is required for live tests (TC-001, TC-002, TC-003,
   TC-004, TC-005, TC-007, TC-010). Tests marked offline (TC-006, TC-008,
   TC-009, TC-011) must also pass without connectivity.
4. A disposable terminal profile: set `RAGENT_HOME` (or work in a scratch
   project directory) so the credential store and session SQLite files
   created by these tests do not pollute the operator's normal state.
5. A second, invalid key (e.g. `sk-or-v1-invalid-000...000`) reserved for
   failure-path tests (TC-009).
6. `curl` available for wire-level cross-checks in TC-012.
7. Note the model ids used below; substitute equivalents if the catalog has
   changed:
   - `anthropic/claude-sonnet-4` (vision + reasoning-capable chat model)
   - `deepseek/deepseek-r1` (reasoning model with visible reasoning deltas)
   - `google/gemini-2.0-flash-001` (fast, non-reasoning baseline)

## Test Cases

### TC-001 — Model discovery with no stored key

**Requirements:** FR-007, FR-008, FR-022, FR-026
**Title:** `ragent models openrouter` lists the public catalog without credentials

**Preconditions:**
- ragent built; no key stored; `OPENROUTER_API_KEY` unset
  (`env | grep OPENROUTER` returns nothing).
- Internet reachable.

**Steps:**
1. Run:
   ```bash
   ragent models openrouter
   ```
2. Observe the first lines of output and the total model count.
3. Confirm none of the lines contain an API key and that the remediation
   hint appears.

**Test data to enter:** none (read-only command).

**Expected results:**
- Output header `openrouter models:` followed by one line per model in the
  form `  openrouter/<id padded> <display name>`, sorted by model id.
- Extensive catalog returned (hundreds of entries, including
  `anthropic/claude-sonnet-4`).
- A hint such as `Run 'ragent auth openrouter <key>' or set
  OPENROUTER_API_KEY.` is shown because no key is configured.
- Exit code 0.

### TC-002 — Secure token storage via `ragent auth`

**Requirements:** FR-004, FR-006
**Title:** stored key wins over environment and enables chat

**Preconditions:**
- TC-001 passed.
- `OPENROUTER_API_KEY` unset in the shell.

**Steps:**
1. Store the key:
   ```bash
   ragent auth openrouter sk-or-v1-<real-key>
   ```
2. Confirm the command output does **not** echo the full key.
3. Verify the stored credential is retrievable only through ragent:
   ```bash
   ragent models openrouter
   ```
4. Grep the config/root directory for plaintext leakage:
   ```bash
   grep -r "sk-or-v1-<real-key>" ~/.local/share/ragent/ .ragent/ 2>/dev/null
   ```

**Test data to enter:** a real OpenRouter key (`sk-or-v1-…`) as the `<key>`
argument of `ragent auth`.

**Expected results:**
- `ragent auth` prints a confirmation (e.g. "Stored API key") that names the
  provider but never prints the key.
- The `models` command no longer shows the "key required" hint.
- The grep finds no plaintext match in config files (stored credential must
  live in the encrypted SQLite credential store only).

### TC-003 — Chat streaming in the TUI

**Requirements:** FR-001, FR-012, FR-013, FR-014, FR-015
**Title:** streaming chat with `openrouter/anthropic/claude-sonnet-4`

**Preconditions:**
- TC-002 completed (key stored).
- Launch the TUI from a scratch project directory.

**Steps:**
1. Start:
   ```bash
   ragent
   ```
2. Press the provider-cycle key repeatedly until the provider widget shows
   **`OpenRouter`**.
3. Press the model-picker key, type `claude-sonnet`, and select
   `openrouter/anthropic/claude-sonnet-4` from the autocomplete list.
4. Type exactly:
   ```text
   Write a haiku about streaming tokens. Keep it to 3 lines.
   ```
   and press Enter.
5. Watch the chat panel while the response streams in.
6. After completion, check the status/usage line at the bottom.

**Test data to enter:** the haiku prompt above; model selection
`openrouter/anthropic/claude-sonnet-4`.

**Expected results:**
- The provider cycle includes `OpenRouter` between the other providers; the
  widget title updates on each cycle step.
- Autocomplete offers vendor-slug ids (the `anthropic/…` part stays intact
  after `openrouter/`).
- Text arrives incrementally (token-by-token rendering, no single-blob dump).
- The session completes without errors; the usage line shows nonzero token
  counts consistent with the request.

### TC-004 — Streaming parity for a reasoning model

**Requirements:** FR-013, FR-018, FR-019, FR-020
**Title:** reasoning deltas render separately from answer text

**Preconditions:**
- TC-003 environment; key stored.

**Steps:**
1. In the running TUI, set the thinking level to **High** using the
   thinking-level selector (the same widget used for Anthropic thinking).
2. Switch the model to `openrouter/deepseek/deepseek-r1` via the
   model picker.
3. Send exactly:
   ```text
   Solve: I have 3 apples and buy 2 bags with 4 apples each. How many apples?
   ```
4. Observe: (a) any "thinking" indicator/bubble region during early stream
   chunks, (b) the visible answer text afterwards, (c) the usage line.
5. Change the thinking level to **Off** and repeat the same prompt in a new
   message.

**Test data to enter:** the arithmetic prompt above; thinking level High,
then Off.

**Expected results:**
- With High: reasoning output appears in the reasoning UI region (separate
  from the answer text), followed by the final visible answer.
- Usage line counts include reasoning tokens in the completion total.
- With Off: no reasoning region content; the answer either arrives directly
  or the model reports reasoning is disabled — no `reasoning` payload errors
  in the log window.

### TC-005 — Tool use through OpenRouter

**Requirements:** FR-012, FR-013, FR-017
**Title:** agent tool calls round-trip with OpenAI-format tool serialization

**Preconditions:**
- TC-003 environment; key stored; model set to
  `openrouter/anthropic/claude-sonnet-4` (a tool-capable model).
- Working directory is the ragent repo (file tools available).

**Steps:**
1. In the TUI, send exactly:
   ```text
   Use the read tool to show the first 10 lines of Cargo.toml, then summarize them.
   ```
2. Watch the tool-call steps appear with step numbers in the log panel.
3. After the run completes, check the message history retained the tool
   result and the final summary is grounded in the file content.

**Test data to enter:** the prompt above.

**Expected results:**
- One or more tool-call steps render (name `read`, arguments referencing
  `Cargo.toml`), each with pretty-printed JSON.
- The tool executes locally and its result routes back to the model.
- The final assistant message summarizes the actual file content.
- No "unsupported tool format" or vendor-prefix errors in the log.

### TC-006 — Missing API key at chat time (offline-safe)

**Requirements:** FR-009
**Title:** chat without a key fails fast with actionable guidance

**Preconditions:**
- A profile with **no** stored OpenRouter key and `OPENROUTER_API_KEY`
  unset.

**Steps:**
1. Run a one-shot chat:
   ```bash
   ragent run --no-tui --model openrouter/anthropic/claude-sonnet-4 "ping"
   ```
2. Read the error output.
3. Set a bogus environment variable style to double-check precedence
   Without a key (leave env unset) and confirm nothing changed.

**Test data to enter:** prompt `ping`; none otherwise.

**Expected results:**
- The run fails before any HTTP call with an error naming the provider:
  `OpenRouter requires an API key.`
- The error/motd suggests remediation (`ragent auth openrouter <key>` or
  setting `OPENROUTER_API_KEY`).
- No partial stream output, no panic/backtrace, exit code nonzero.

### TC-007 — Environment variable fallback

**Requirements:** FR-004
**Title:** `OPENROUTER_API_KEY` env var authenticates when nothing is stored

**Preconditions:**
- No stored credential for `openrouter` (fresh profile or
  `ragent auth openrouter --clear` if that subcommand exists — otherwise use
  a fresh `RAGENT_HOME`).

**Steps:**
1. Export the real key:
   ```bash
   export OPENROUTER_API_KEY=sk-or-v1-<real-key>
   ```
2. Run:
   ```bash
   ragent models openrouter
   ragent run --no-tui --model openrouter/google/gemini-2.0-flash-001 "Say OK"
   ```
3. Unset the variable (`unset OPENROUTER_API_KEY`).

**Test data to enter:** the real key in the environment; prompt `Say OK`.

**Expected results:**
- Both commands succeed with the env var only (no stored credential).
- `ragent run` returns a short reply containing `OK`.
- Precedence check (informational): when both a stored key and an env var
  exist, behavior remains stable between restarts (stored wins per FR-004).

### TC-008 — Key redaction in logs and errors (offline-safe)

**Requirements:** FR-005, FR-025
**Title:** the API key never appears in logs, errors, or diagnostics

**Preconditions:**
- Key stored (from TC-002).
- Log level raised for the test: run with `--log-level debug`.

**Steps:**
1. Run a chat that will fail at the provider (keep the real key stored but
   request a nonexistent model):
   ```bash
   ragent run --no-tui --log-level debug \
     --model openrouter/nonexistent-vendor/nonexistent-model "ping" 2>&1 | tee target/temp/or-redact.log
   ```
   (create `target/temp` if missing)
2. Grep the captured log and the terminal output for the real key:
   ```bash
   grep -c "sk-or-v1-<real-key>" target/temp/or-redact.log
   ```
3. Inspect any key-shaped strings that do appear — they must be the masked
   fingerprint form (e.g. `sk-or-…abcd`).

**Test data to enter:** the nonexistent model id; the real key stays stored
from earlier tests.

**Expected results:**
- `grep -c` returns 0 matches for the full real key.
- Any diagnostics referring to the key show only the masked form.
- The run fails with a readable model-not-found/404-style message.

### TC-009 — Invalid key failure path

**Requirements:** FR-015
**Title:** 401 from OpenRouter surfaces a readable error

**Preconditions:**
- Fresh profile with the **invalid** key stored:
  ```bash
  ragent auth openrouter sk-or-v1-invalid-0000000000000000000000000000
  ```

**Steps:**
1. Run:
   ```bash
   ragent run --no-tui --model openrouter/google/gemini-2.0-flash-001 "ping"
   ```
2. Read the error message and exit code.
3. Repeat inside the TUI: set the provider to OpenRouter, pick a model, send
   a short prompt, and observe the TUI error surface.

**Test data to enter:** the invalid key; prompt `ping`.

**Expected results:**
- CLI: an error naming the HTTP failure (401/Unauthorized or provider error
  text) is printed; no panic; nonzero exit.
- TUI: an error event appears in the chat/log panel and the input remains
  usable (the session does not wedge).
- The error text is bounded (no multi-megabyte HTML dump).

### TC-010 — Config base-URL override (local echo server)

**Requirements:** FR-002, FR-003, FR-022
**Title:** `provider.openrouter.api.base_url` redirects discovery and chat

**Preconditions:**
- A stub HTTP server that answers `GET /api/v1/models` with a minimal
  catalog fixture, e.g.:
  ```json
  {"data":[{"id":"test/test-model","name":"Test Model",
            "context_length":4096,
            "pricing":{"prompt":"0.000001","completion":"0.000002"},
            "architecture":{"input_modalities":["text","image"]},
            "supported_parameters":["reasoning","tools"]}]}
  ```
  (serve on `http://127.0.0.1:8999`; any static-file server plus a JSON
  file works).
- `.ragent/ragent.json` in the test project with:
  ```json
  { "provider": { "openrouter": { "api": { "base_url": "http://127.0.0.1:8999/" } } } }
  ```

**Steps:**
1. Start the stub server on port 8999.
2. Run:
   ```bash
   ragent models openrouter
   ```
3. Confirm the output lists exactly `openrouter/test/test-model` with the
   name `Test Model`.
4. Stop the stub server.
5. Run `ragent models openrouter` again.

**Test data to enter:** the JSON fixture above; the base_url override.

**Expected results:**
- With the override active, requests hit `127.0.0.1:8999` (stub access log
  shows the hit) and the listed catalog matches the fixture exactly —
  including the vision/reasoning capability flags inferred from the
  fixture.
- With the stub down, the command prints
  `Could not connect to OpenRouter: …` (or equivalent) and exits without
  panic, leaving no partial model cache corruption.
- The trailing slash in `base_url` does not produce a double-slash URL.

### TC-011 — Model metadata rendering (offline-safe)

**Requirements:** FR-010, FR-011, FR-026
**Title:** discovered metadata is surfaced in listings and tolerates junk

**Preconditions:**
- Same stub-server setup as TC-010, using a fixture catalog that includes:
  - a normal entry (the TC-010 model),
  - an entry missing `pricing` and `architecture`,
  - an entry with a scientific-notation price string (`"1.5e-06"`),
  - a malformed entry missing `id`.

**Steps:**
1. Start the stub with the combined fixture.
2. Run `ragent models openrouter`.
3. Compare displayed context windows, names, and the total count against the
   fixture.

**Test data to enter:** the multi-entry fixture (include the malformed
entry).

**Expected results:**
- Every well-formed entry appears once with the correct display name.
- The malformed entry (missing `id`) is skipped with a warning in debug
  logs — the other entries still list.
- Missing optional fields produce plain rendering (no `NaN`, no `0ms`
  price artifacts, no crash).

### TC-012 — Retry/billing safety and wire cross-check

**Requirements:** FR-025, FR-012
**Title:** a failed chat POST is not retried; request body matches the
OpenAI-compatible schema

**Preconditions:**
- The stub server from TC-010 extended to always answer `POST
  /api/v1/chat/completions` with `500` and to log the number of POSTs and
  the request body it received.

**Steps:**
1. Point `base_url` at the stub; store a dummy key.
2. Run:
   ```bash
   ragent run --no-tui --model openrouter/test/test-model "ping"
   ```
3. Inspect the stub's POST counter and captured body.
4. Optionally, cross-check the body shape against OpenRouter's documented
   schema (model, messages with system first, stream true, tools array when
   tools were offered, reasoning object only when a thinking config was
   active).

**Test data to enter:** prompt `ping`; dummy key; 500 stub response.

**Expected results:**
- The stub received **exactly one** POST (no automatic retries that could
  double-bill).
- The captured body has `model = "test/test-model"`, a `messages` array
  whose first entry has `role = "system"`, and `"stream": true`.
- No `Authorization` header value other than `Bearer sk-or-…` (dummy) was
  sent, and none was sent to any other host.

## Cleanup

1. Stop any stub HTTP server started for TC-010/TC-011/TC-012.
2. Remove test credentials from profiles used:
   - `ragent auth openrouter` has no dedicated remove subcommand in scope —
     delete the test `RAGENT_HOME`/profile directory or wipe the SQLite
     credential store row for provider `openrouter` manually.
3. Unset `OPENROUTER_API_KEY` in shells used for testing
   (`unset OPENROUTER_API_KEY`) or close them.
4. Delete scratch project directories and `.ragent/ragent.json` overrides
   created for the base-URL tests.
5. Remove captured artifacts: `target/temp/or-redact.log` and any stub
   server logs. `target/` contents are git-ignored and disposable.
6. Re-run `ragent models openrouter` once with the real environment to
   confirm normal (operator) state is intact and no test artifacts remain.
---
status: draft
audit:
  - { time: 1793000000, from: "none", to: "draft", actor: "system" }
---
# Manual Test Plan: Unified Tool Calling, and the Tool Rename

**Spec:** [SPEC.md](SPEC.md) - **Plan:** [PLAN.md](PLAN.md)

This is a **manual** test plan. Every case is executed by a human with a
terminal, a configured provider, a spreadsheet reader, and a scratch project.
It contains no automated test code, no `#[test]` functions, and no instruction
to run a test binary.

Each case names the requirement it exercises. Where a case drives the
interactive interface, every key press is written out in order and the exact
text to type is given as **Test data**. Keys are written as `Enter`, `Esc`,
`Tab`, `Up`, `Down`, `Ctrl+C`, and literal characters in `backticks`.

Throughout:

- **"the input field"** is the message box at the bottom of the TUI.
- **"the log panel"** is the scrolling transcript above it.
- **"a turn"** is one prompt and everything the agent does in reply to it.
- **"step"** is one model request inside a turn.
- **"the workbook"** is `tools.xlsx` in the repository root.
- **"the rename table"** is the checked-in list of 13 `(before, after)` pairs
  the change adds, in `crates/ragent-agent/src/tool/rename.rs`.

Three groups of cases run here. Cases TC-001 to TC-018 exercise the unified
tool-calling layer (FR-001 to FR-022). Cases TC-019 to TC-028 exercise the
rename (FR-023 to FR-030 and NFR-006/007). Run the first group against a build
that predates the rename, and the second group against the finished build.

TC-029 and TC-030 extend the second group past "the rename happened" to
"nothing can undo it": no configuration key, no environment variable, no CLI
flag, no dynamically bridged tool, and no completion list may reintroduce a
retired spelling (FR-031 to FR-034, NFR-008, NFR-009). Run those two against the
same finished build as TC-019 to TC-028.

---

## Prerequisites

1. **Build the binary.** Build a debug binary and confirm `ragent --version`
   prints a version string. Run every case with that binary.

2. **Keep a pre-change binary.** Before the rename lands, copy the binary built
   from the commit immediately before the change to a path outside the
   repository, for example `~/scratch/ragent-before/ragent`. TC-017 and TC-027
   need it to compare against.

3. **Configure an OpenAI-family provider.** Set `OPENAI_API_KEY` (or use a local
   Ollama endpoint with `OLLAMA_HOST`). Confirm a plain prompt returns a reply:

   ```
   ragent run "reply with the single word: ready"
   ```

   Expected: the word `ready` appears in the output and no error banner is
   printed.

4. **Configure an Anthropic-family provider.** Set `ANTHROPIC_API_KEY` and
   confirm a plain prompt returns a reply with the same command, selecting the
   Anthropic model.

   If only one family is available, run the family-specific cases for the
   family you have and mark the others as not executed. TC-001, TC-005, TC-010,
   TC-012, TC-014, TC-015 and TC-016 are family-agnostic and must run either
   way.

5. **Create a scratch project.** Create an empty directory
   `~/scratch/uniformtools-tests/` and run every session from inside it, so the
   project `.ragent/` directory belongs to the test:

   ```
   mkdir -p ~/scratch/uniformtools-tests
   cd ~/scratch/uniformtools-tests
   printf 'alpha\n' > notes.txt
   ```

6. **Know how to make the agent call two tools in one reply.** A prompt that
   asks for two independent reads works on most models:

   ```
   read notes.txt and also list the files in this directory
   ```

   If the model answers from context instead of calling tools, rephrase with
   `you must use the read tool and the list tool`.

7. **Know how to make a tool call fail.** Ask the agent to read a file that does
   not exist:

   ```
   read the file missing-file.txt
   ```

8. **Know how to produce a permission prompt.** From a fresh session ask the
   agent to write a file:

   ```
   create a file called hello.txt containing the word hello
   ```

   The permission dialog appears above the input field with an allow/deny
   choice and a countdown.

9. **Have a spreadsheet reader for the workbook.** Any tool that opens `.xlsx`
   files and shows cell values will do. Confirm you can see, in the
   `Anthropic Tools` sheet, columns headed `Tool` and `Mapping`, and in the
   `ragent Tools` sheet, columns headed `Tool`, `Anthropic equivalent` and
   `Mapping`.

10. **Record the model name and provider.** Note them for each case; several
    expected results are provider-specific.

11. **Have a checkout of the repository** with a terminal at its root. TC-019,
    TC-022, TC-024, TC-025, TC-026 and TC-030 read source and documents rather
    than the running binary.

12. **Enable the payload trace for TC-001.** Set `tools.wire_trace: true` in the
    project's `ragent.json` so the encoder's output is recorded under
    `target/temp/wire-trace/`, one file per step. TC-001 reads that file to check
    the carrier each family actually emitted; the trace is redacted, so it is
    safe to read and safe to keep.

12. **Know the guard path.** The no-alias guard ships as
    `scripts/check-tool-rename.sh` and as one automated case in the same
    change. TC-026, TC-030 and the Cleanup section refer to it by that path.

---

## Test Cases

### TC-001 - A tool call round-trips with an unchanged id (FR-001, FR-006)

**Preconditions:** Prerequisites 1, 3, 5 and 12. A fresh session in the scratch
project.

**Steps:**

1. Start the TUI: run `ragent` in `~/scratch/uniformtools-tests/`.
2. Wait for the status bar to show a model name and no error.
3. Click the input field, then type the test data and press `Enter`.
4. Watch the log panel for the tool-call block and note the call id shown beside
   it (a short identifier such as `toolu_01A...` or `call_ab12...`).
5. Observe the tool-result block that follows.
6. Repeat steps 3 to 5 on every family you configured in Prerequisite 4, then
   read the request bodies the run recorded (Prerequisite 12).

**Test data:** `read notes.txt`

**Expected results:**

- The agent calls the read tool once and reports the file content `alpha`.
- The call id displayed on the tool-call block and the id on the corresponding
  result are identical character for character.
- The model's next message reflects the file content, proving the result reached
  it.
- No error banner is printed.
- In each recorded request body the result travels in the carrier that family
  declares, and the id in it is that same identifier:
  - an OpenAI-family body carries `{"role":"tool","tool_call_id":"<id>","content":"alpha"}`;
  - an Anthropic-family body carries a `{"type":"tool_result","tool_use_id":"<id>","content":"alpha"}`
    block in the answering `user` turn;
  - a Bedrock Converse body carries a `{"toolResult":{"toolUseId":"<id>",...}}` block;
  - a Gemini body carries a `{"functionResponse":{"name":"Read","response":{"result":"alpha"}}}`
    part, naming the tool rather than the id;
  - a Responses-body request carries a `{"type":"function_call_output","call_id":"<id>","output":"alpha"}`
    item beside the `function_call` item it answers.

---

### TC-002 - The wire matrix lists every provider with no empty field (FR-002, FR-016)

**Preconditions:** Prerequisite 1.

**Steps:**

1. In a terminal, from the scratch project, run the binary with the wire-matrix
   flag by typing the test data and pressing `Enter`.
2. Read the printed table.
3. Count the rows and compare against the provider list printed by
   `ragent --help` (or the provider selector in the TUI, reached by typing
   `/provider` in the input field and pressing `Enter`).

**Test data:** `ragent --wire-matrix`

**Expected results:**

- The table has one row per registered provider.
- Every column is populated on every row: tool envelope, result carrier, id
  field, argument encoding, stream keying, parallel support, error-flag support,
  reasoning echo and synthetic ids. No cell prints `-`, `none`, or a blank where
  a value is expected.
- The program exits immediately without contacting any provider and without
  executing a tool. No network activity occurs and no session is created.
- The table is plain ASCII.
- Adding a provider would require a new row: there is no provider in
  `ragent --help` that is missing from the table.

---

### TC-003 - The same tool behaves identically on both provider families (FR-001, FR-012)

**Preconditions:** Prerequisites 1, 3, 4 and 5. Note the exact output of the
first run before starting the second.

**Steps:**

1. Start the TUI and switch to the OpenAI-family provider by typing `/provider`
   in the input field, pressing `Enter`, selecting the OpenAI entry with
   `Up`/`Down`, and pressing `Enter`.
2. Type the test data and press `Enter`.
3. Record: whether a tool block appeared, the tool name shown, whether the file
   content was reported, and whether an error appeared.
4. Type `/new` and press `Enter` to start a fresh session.
5. Switch to the Anthropic-family provider with `/provider`, `Enter`,
   `Up`/`Down`, `Enter`.
6. Type the same test data and press `Enter`.
7. Compare the recorded observations.

**Test data:** `read notes.txt`

**Expected results:**

- Both families call the same tool name (`read`).
- Both families report the same file content (`alpha`).
- Neither family prints an error.
- Only the call-id format differs, and it is not visible to the user as a
  difference in behaviour or in the model's answer.
- In both runs the permission decision, the argument validation and the result
  all happen in the same order; no family-specific message is shown.

---

### TC-004 - Two tool calls in one reply are both executed (FR-004, FR-005, FR-019)

**Preconditions:** Prerequisites 1, 3, 4 and 5.

**Steps:**

1. Start the TUI.
2. Type the test data and press `Enter`.
3. In the log panel, count the tool-call blocks belonging to the assistant
   message, before any tool result appears.
4. Read each tool block's arguments.
5. Read the tool results.

**Test data:** `you must use the read tool on notes.txt and the list tool on this directory`

**Expected results:**

- Two tool-call blocks appear in the same assistant message.
- Each block carries its own arguments: one names `notes.txt`, the other names
  the working directory (or an empty path).
- Neither block's arguments are a concatenation or truncation of the other's.
- Both tools execute and both results appear, in the order the model emitted the
  calls.
- The model's next message accounts for both results.
- On a provider that supplies no call ids of its own, the displayed ids are
  non-empty and unique, and neither id is used to work out which tool was
  called - the tool name is shown from the record, and a tool whose name
  contains an underscore still resolves correctly.
- Repeat the case on the Anthropic family and on Bedrock if available. The same
  expected results hold; in particular no call's arguments are lost when the
  two streams interleave. On Bedrock this is the case that used to lose one
  call's arguments.

---

### TC-005 - Malformed arguments produce a corrective error, not an execution (FR-007, FR-008, FR-012)

**Preconditions:** Prerequisites 1, 3 and 5. This case needs a model or client
that will emit broken JSON, so it is normally run with a small local model
(Ollama) asked to call a tool with a deliberately broken argument payload.

**Steps:**

1. Start the TUI with the local model selected (`/model`, `Enter`, `Up`/`Down`,
   `Enter`).
2. Type the test data exactly, including the malformed JSON, and press `Enter`.
3. Watch the log panel for the tool-call block and the result block.
4. Note whether any file was created or any command ran.

**Test data:** `call the read tool with the raw arguments {"path": "notes.txt"` (note the missing closing brace)

**Expected results:**

- The tool block shows the malformed argument text as received.
- A result block follows containing text that names the tool and quotes a JSON
  parse error, and that instructs the model to resend one complete JSON object.
- The read tool does **not** execute: no file content is reported.
- The result block is marked as an error (see TC-010 for how an error is
  marked).
- If the model retries with valid JSON, the retry executes normally.
- The decode happens once. Asking the model afterwards what it received shows
  one corrective message, not two.

---

### TC-006 - A missing required argument is reported, not executed (FR-009)

**Preconditions:** Prerequisites 1, 3 and 5.

**Steps:**

1. Start the TUI.
2. Type the test data and press `Enter`.
3. Read the result block that follows the tool call.

**Test data:** `call the move_file tool with only the source argument, omitting the destination`

**Expected results:**

- The tool block shows arguments that lack `destination`.
- A result block names the missing parameter and states that it is required.
- `move_file` does not execute: no file is renamed.
- The result is marked as an error.
- If the model resends with both arguments, the retry executes normally.

---

### TC-007 - A schema type mismatch is reported, not executed (FR-009)

**Preconditions:** Prerequisites 1, 3 and 5. Use the finished build, where the
shell tool is named `Bash`.

**Steps:**

1. Start the TUI.
2. Type the test data and press `Enter`.
3. Read the result block that follows.

**Test data:** `call the Bash tool with the command argument set to the number 5 instead of a string`

**Expected results:**

- A result block names the parameter `command` and states that it must be of
  type `string` while a number was supplied.
- The shell tool does not run: no shell command executes.
- The result is marked as an error.
- No permission dialog appears, because the call is rejected before the
  permission stage.

---

### TC-008 - Sequential execution is forced where the provider cannot parallelise (FR-010)

**Preconditions:** Prerequisites 1, 3, 4, 5 and 8. Confirm the setting is on
before the run.

**Steps:**

1. Open the config file used by the session (the project `.ragent/ragent.json`,
   or the file named by `RAGENT_CONFIG`) in an editor and set the experimental
   block:

   ```
   "experimental": { "parallel_tool_calls": true }
   ```

2. Save and restart the TUI so the setting takes effect.
3. Select a provider whose profile does not support parallel calls (consult the
   wire matrix from TC-002, `parallel` column).
4. Type the test data from TC-004 and press `Enter`.
5. In the log panel, observe the order of tool-call and tool-result blocks.
6. Enable debug logging in a terminal run and look for a line naming the
   provider and the reason:

   ```
   ragent run --log-level debug "you must use the read tool on notes.txt and the list tool on this directory"
   ```

**Test data:** `you must use the read tool on notes.txt and the list tool on this directory`

**Expected results:**

- The tool blocks and results strictly alternate: the first call's result
  appears before the second call completes.
- Exactly one informational log line appears naming the provider and stating
  that tool calls run sequentially because the provider does not support
  parallel tool calls.
- No error is shown to the user.
- With a provider that does support parallel calls, the same steps dispatch
  concurrently instead, and no such log line appears.
- Both calls still execute. Forcing sequential execution never drops a call.

---

### TC-009 - An unresolved approval blocks the result (FR-013)

**Preconditions:** Prerequisites 1, 3, 5 and 8. YOLO mode must be off, and
autopilot must be off.

**Steps:**

1. Start the TUI.
2. Type the test data and press `Enter`.
3. When the permission dialog appears, do not answer it. Wait and watch the log
   panel.
4. Observe whether any tool-result block appears while the dialog is open.
5. Press `Esc` or choose deny in the dialog.
6. Observe what the model receives.

**Test data:** `create a file called hello.txt containing the word hello`

**Expected results:**

- While the dialog is open, no tool-result block for this call appears in the
  log panel, and the model is not sent a result that claims success.
- The countdown in the dialog decrements; if it reaches zero the call is treated
  as denied.
- After deny, a result marked as an error (or a denial notice) is sent to the
  model, and the model does not claim the file was written.
- `hello.txt` is not created. Confirm with `ls` in the scratch directory.

---

### TC-010 - An error result is distinguishable from a success result (FR-014)

**Preconditions:** Prerequisites 1, 3, 4 and 5.

**Steps:**

1. Start the TUI.
2. Type the test data and press `Enter`.
3. Read the result block. Note whether it carries a visible error marker and
   whether the text begins with `ERROR: `.
4. Switch provider (`/provider`, `Enter`, `Up`/`Down`, `Enter`) to the other
   family, type `/new` and `Enter`, then repeat steps 2 and 3.
5. Read the model's reply in both runs.

**Test data:** `read the file missing-file.txt`

**Expected results:**

- The call fails on both families and a result block is produced.
- The result is marked as an error in the log panel on both families.
- On the family without native error support, the result text begins with the
  literal prefix `ERROR: ` followed by the failure description. Apart from that
  prefix, the content is the same text the other family receives - no other
  word is changed, added, or removed.
- The model's next message acknowledges the failure rather than reporting the
  file content.

---

### TC-011 - An unknown tool name never executes (FR-021)

**Preconditions:** Prerequisites 1, 3 and 5.

**Steps:**

1. Start the TUI.
2. Type the test data and press `Enter`.
3. Read the result block.

**Test data:** `call a tool named redd to read notes.txt`

**Expected results:**

- Nothing executes: no file is read and no shell command runs.
- A result block names the unknown tool `redd` and offers `read` as the closest
  registered name.
- The result is marked as an error.
- The model's next message either retries with `read` (which then executes) or
  explains the mistake.

---

### TC-012 - Reasoning is replayed only where the provider requires it (FR-011)

**Preconditions:** Prerequisites 1, 4 and 5. A model with extended thinking
available (an Anthropic thinking-capable model, or an OpenAI reasoning model).
Thinking must be enabled: type `/thinking high` in the input field and press
`Enter`.

**Steps:**

1. Start the TUI with the thinking-capable model selected.
2. Confirm the status bar shows the thinking level.
3. Type the test data and press `Enter`.
4. Watch the tool call and its result.
5. After the tool result, read the model's follow-up reply.
6. Run the same prompt once in a terminal with `--log-level debug`, and inspect
   the request body with the payload trace from TC-018 enabled, so the replayed
   reasoning block is visible in the messages array.
7. Repeat the whole case on a non-thinking model.

**Test data:** `read notes.txt and tell me what it says`

**Expected results:**

- With thinking enabled on a provider that requires it, the follow-up response
  succeeds: no provider error such as "thinking block missing", "signature
  required", or "invalid assistant message" appears.
- The replayed reasoning block, including its signature, sits ahead of the
  tool-use block of the same assistant turn, in that order.
- On a provider that does not require it, reasoning is not sent, and the turn
  behaves exactly as it did before the change.
- No reasoning text is shown to the user in the log panel as if it were the
  answer.

---

### TC-013 - An alias spelling resolves to the ragent tool (FR-015, FR-025)

**Preconditions:** Prerequisites 1, 3 and 5. Alias resolution enabled: add to
the project `.ragent/ragent.json`:

```
"tools": { "aliases": true, "expose_alias_names": true }
```

Note: on the finished build the alias table holds only provider spellings that
are **not** ragent names. `Bash`, `Edit`, `Write` and the other ten renamed
spellings are the registered names themselves, so they resolve on the registry
lookup and must not appear in the table (FR-026). This case therefore uses an
OpenAI spelling ragent does not adopt.

**Steps:**

1. Restart the TUI so the config is re-read.
2. Establish what the table actually contains, so the case tests a real entry
   rather than an assumed one. In a session with a model available, run
   `ragent run --log-level debug "list the tools you can call"`, and read the
   tools array in the request with the payload trace from TC-018 enabled.
3. Write down every name the tools array advertises that is not a registered
   ragent name. These are the alias spellings.
4. For each spelling written down in step 3, check it maps to the tool the
   alias source file declares. If the list is empty, record the case as not
   applicable to this build and note it - the built-in table may be empty, and an
   empty table is a valid configuration.
5. Type the first alias spelling found in step 3 into a prompt that asks for it
   by name, and press `Enter`.
6. Read the tool block and the result.
7. Disable aliases by setting `"aliases": false` in the same block, restart,
   type `/new`, `Enter`, and repeat step 5.
8. Set `"tools": { "aliases": false }` and run the Claude spelling the workbook
   made ragent adopt.

**Test data:**

- `use the web search tool to look up the current stable Rust version` (assumes
  `web_search` is an alias for `websearch`; if step 3 shows otherwise, use the
  spelling it showed)
- `use the Bash tool to print the current directory name`

**Expected results:**

- Step 2: the advertised tools array contains both ragent names and the alias
  spellings, and no alias spelling equals a ragent tool name.
- Step 5, aliases enabled: the tool block shows the canonical ragent name
  (`websearch` for the example spelling), the tool executes, and the result is
  reported. The model is not told the tool is unknown.
- Step 7, aliases disabled: the same prompt is refused as an unknown tool named
  by the provider spelling, and the suggested name is the ragent name.
- In both cases no unrelated tool runs.
- Step 8 works with aliases off, and the tool block names `Bash`: the registry
  holds that spelling, so no table lookup happened. This is the difference the
  rename makes, and it is the point of the feature.
- With `expose_alias_names` true, no renamed tool is advertised twice. `Bash`
  appears once, never as `Bash` and `bash` on the same call.

---

### TC-014 - Prior tool history survives into the next step (FR-018, FR-006)

**Preconditions:** Prerequisites 1, 3, 4 and 5. This is the case most likely to
expose a dropped-history bug, so run it on every family available, and in
particular on the OpenAI Responses API path if it is configured.

**Steps:**

1. Start the TUI.
2. Type the test data and press `Enter`.
3. After the tool result, read the model's follow-up message.
4. Type the second test line and press `Enter`.
5. Read the second reply and check whether it can still refer to the earlier
   file content without re-reading it.
6. Repeat on the Responses API path with three consecutive tool turns.

**Test data:**

- `read notes.txt`
- `without reading it again, tell me the single word that was in that file`

**Expected results:**

- The second reply states `alpha` (or quotes the file content), proving the
  earlier tool call and its result are still visible to the model.
- No hard error naming a construct, a provider, or a lost field is printed. In
  particular no message such as `cannot carry tool use for provider ...`
  appears, because the history in this case is carryable.
- On a family that genuinely cannot carry a construct, the run fails loudly with
  that error naming the construct and the provider, and **no request is sent**.
  Silent success followed by a wrong answer is a failure of this case.
- A silent drop is a failure even when the answer happens to be right: check the
  request body with the payload trace from TC-018, and confirm the prior tool
  call and its result both appear in the messages array.

---

### TC-015 - A tool-use finish with no parsed call falls back to recovery (FR-022)

**Preconditions:** Prerequisites 1, 3 and 5, plus a small local model known to
narrate tool calls as text (for example, a compact Ollama model).

**Steps:**

1. Start the TUI with the local model selected.
2. Type the test data, which asks the model to print a tool call as text rather
   than emit a native call, and press `Enter`.
3. Watch the log panel for a tool block appearing even though the model produced
   only prose.
4. Check whether the file content is reported.
5. Repeat with a prompt that produces no recognisable tool markup at all.

**Test data:** `write out the tool call as text on its own line, in the form <toolcall>{"name":"read","arguments":{"path":"notes.txt"}}</toolcall>, do not call the tool yourself`

**Expected results:**

- The turn does not end with an empty reply and does not report a truncation
  when a recoverable call is present.
- The recovered call executes through the normal pipeline: it passes the
  permission check exactly as a natively emitted call would.
- The file content `alpha` is reported.
- If the text contains no recognisable tool markup (step 5), the turn ends
  normally with the prose reply and no tool block, and no spurious recovery
  prompt is sent.

---

### TC-016 - Duplicate call ids in one reply are reconciled (FR-020)

**Preconditions:** Prerequisites 1, 3 and 5, plus the local model selected so the
raw stream can be reproduced. If the model cannot be made to emit duplicate ids,
record the case as not reproducible and note it.

**Steps:**

1. Start the TUI with the local model.
2. Type the test data, which asks for two calls carrying the same id, and press
   `Enter`.
3. Count the tool blocks and read each block's id.
4. Read both results.
5. Run the same prompt with `--log-level warn` in a terminal.

**Test data:** `emit two tool calls in one reply, both for the read tool on notes.txt, and give both calls the same id "call_dup"`

**Expected results:**

- Both calls execute; neither result overwrites the other.
- Both results appear, each attributed to its own call.
- The ids displayed are unique: the first keeps the provider's id, the later one
  is re-keyed to a generated id.
- One warning is logged per collision, naming the duplicated id, and no error
  banner is shown to the user.
- Step 5 shows exactly one such warning line, not one per subsequent step.

---

### TC-017 - The tool-calling contract is unchanged for a normal single-tool turn (FR-001, FR-003, FR-012, NFR-001, NFR-004)

**Preconditions:** Prerequisites 1, 2, 3 and 5. This is the regression case: run
it once with the pre-change binary saved in prerequisite 2, and once with the
finished binary, and compare.

**Steps:**

1. Start the TUI with the pre-change binary.
2. Type the test data and press `Enter`.
3. Note: the number of tool blocks, the tool name, the result, whether a
   permission dialog appeared, and the wall-clock time from `Enter` to the
   model's final message.
4. Type `/new`, `Enter`, and repeat steps 2 and 3 twice more.
5. Repeat steps 1 to 4 with the finished binary.
6. Compare the six runs.
7. With the finished binary, enable the payload trace (TC-018) and confirm the
   tools array in the request is byte-identical to the one the pre-change binary
   produced for the same provider and tool set. Repeat this with the
   `openai_responses` provider selected: since T-005 that family serves its tool
   array from the same cache as the other five, so its request body must equal
   the encoder's payload exactly - a hand-built array that happens to be the
   right shape is not the contract (FR-003).
8. Without any provider credentials configured, run
   `ragent run --dry-run --json "x"` and confirm it completes: the encoder and
   the profile table are exercisable offline, with no live provider and no
   network call.

**Test data:** `list the files in this directory`

**Expected results:**

- Every run produces exactly one tool block named `list` and one result
  containing `notes.txt`.
- No permission dialog appears for a read-only tool that previously required
  none.
- The time from `Enter` to the final message is not materially longer than the
  pre-change measurement; a regression of more than a fifth on the same model is
  a failure of NFR-001.
- Step 7 shows identical tools JSON, proving the encoder is one encoder and the
  cache serves a hit that equals a miss - for every provider family, including
  the Responses API, which had its own array until T-005.
- Step 8 completes without credentials, proving the pure-function encoders and
  the profile table can be checked with no provider (NFR-004).
- No new warnings or errors appear in the log panel or in the terminal.

---

### TC-018 - The payload trace is opt-in and redacted (FR-017)

**Preconditions:** Prerequisites 1, 3 and 5.

**Steps:**

1. With tracing absent from the config, start the TUI, type `list the files in
   this directory`, press `Enter`, and wait for the reply.
2. From a second terminal, check whether a trace directory exists under the
   scratch project:
   `ls -R ~/scratch/uniformtools-tests/target/temp/wire-trace 2>&1`
3. Stop the TUI with `Esc` then `Ctrl+C` in the terminal.
4. Add to the project `.ragent/ragent.json`:

   ```
   "tools": { "wire_trace": true }
   ```

5. Restart the TUI, repeat the prompt, wait for the reply, then stop it.
6. List the trace directory again and open the newest file in an editor.
7. Search the file for any credential: `grep -c 'sk-' <file>`.

**Test data:** `list the files in this directory`

**Expected results:**

- Step 2: no trace directory is created, and no files are written into the
  scratch project by the run.
- Step 6: one file per step exists, named with the session id and step number.
- The file contains the request body for that step, including the tools array
  and the messages array.
- Step 7 prints `0`: no API key, token, or password appears in cleartext.
- The file contains no non-ASCII byte: `grep -cP '[^\x00-\x7F]' <file>` prints
  `0`.
- The trace file name is ASCII (NFR-005).
- With tracing set back to `false` or removed, further runs write nothing.

---

### TC-019 - The rename table reproduces from the workbook (FR-023, FR-024, FR-025)

**Preconditions:** Prerequisites 1, 9 and 11, plus the finished build. This is
the manual cross-check the specification requires, because the code cannot open
the workbook.

The expected derivation is recorded in
[`T-029-DERIVATION.md`](T-029-DERIVATION.md), including the per-clause
accounting (215 `(row, token)` pairs split 121 / 20 / 40 / 34) that reduces the
15 candidate targets to 13 renames and 2 refusals. Compare this run against that
record as well as against the table in the source.

**Steps:**

1. Open `tools.xlsx` in the repository root.
2. In the `Anthropic Tools` sheet, list every row whose `Mapping` cell reads
   exactly `Exact`. Record its `Tool` value. These are the *candidate targets*.
3. In the `ragent Tools` sheet, list every row whose `Mapping` cell reads
   exactly `Exact` and whose `Anthropic equivalent` cell names at least one
   candidate target. Record its `Tool` value and the target it names.
4. Drop every candidate whose target is named only by ragent rows that are not
   `Exact`, or named by no ragent row at all (FR-024).
5. For each remaining target, collect every ragent tool that names it. Where
   there is more than one, keep the claimant whose name equals the target once
   each `_`-separated segment is capitalised; leave the others under their
   current names. Where no claimant matches, refuse the target and record the
   competing names (FR-025).
6. Open the rename table in
   `crates/ragent-agent/src/tool/rename.rs`.
7. Compare the two lists row by row. Then check the order: the table is
   ascending by `after`.
8. Confirm the test that ships with the table has been reported as passing by
   the reviewer of the change, and read what it asserts: every `before` is a
   registered tool, every `after` is not registered and is a valid identifier,
   and both columns are unique.

**Test data:** none - this case reads the workbook and the source.

**Expected results:**

- Step 3 yields 34 `(tool, target)` pairs over 22 ragent tools and 15 distinct
  candidate targets (15 is the target count, not the pair count).
- Steps 4 and 5 leave exactly 13 renames, matching this list: `ask_user` to
  `AskUserQuestion`, `bash` to `Bash`, `edit` to `Edit`, `plan_enter` to
  `EnterPlanMode`, `plan_exit` to `ExitPlanMode`, `glob` to `Glob`, `grep` to
  `Grep`, `skill_manage` to `Skill`, `task_create` to `TaskCreate`,
  `task_get` to `TaskGet`, `task_list` to `TaskList`, `task_update` to
  `TaskUpdate`, `write` to `Write`.
- Two targets are refused, not renamed: `WebFetch` (claimed by `mf_fetch` and
  `webfetch`) and `WebSearch` (claimed by `mf_search` and `websearch`).
  Neither claimant equals the target's spelling.
- `new_agent` and `read` are absent because their Anthropic rows are `Close`.
  All five `cron_*` tools (`cron_add`, `cron_remove`, `cron_list`, `cron_enable`,
  `cron_disable`) are absent because their own ragent rows are `Close`, even
  though `CronCreate`, `CronDelete` and `CronList` are `Exact`. `TodoWrite` is
  absent because no ragent row names it.
- `bash_reset`, `apply_patch`, `patch`, `create` and `update_file` keep their
  current names.
- Step 7: the table and the workbook agree in both directions - nothing added,
  nothing missing, order ascending by `after`.
- The invariant test does not read the workbook. Confirm by searching
  `crates/ragent-agent` for any call that opens `tools.xlsx`; there must be
  none.
- The refusal reasons recorded in step 4 and step 5 name the clause that failed
  for every excluded candidate, not only the count.

---

### TC-020 - The renamed tools advertise their Claude spelling and nothing else (FR-023, FR-026, FR-030)

**Preconditions:** Prerequisites 1, 3 and 5.

**Steps:**

1. Start the TUI in the scratch project and type `/tools list`, then press
   `Enter`. Do not send a prompt: this case needs no model reply.
2. Read the listing and search it for each of the old spellings in turn:
   `ask_user`, `bash`, `edit`, `glob`, `grep`, `plan_enter`, `plan_exit`,
   `skill_manage`, `task_create`, `task_get`, `task_list`, `task_update`,
   `write`. Scroll the window as needed.
3. Search the same listing for each of the new spellings: `AskUserQuestion`,
   `Bash`, `Edit`, `Glob`, `Grep`, `EnterPlanMode`, `ExitPlanMode`, `Skill`,
   `TaskCreate`, `TaskGet`, `TaskList`, `TaskUpdate`, `Write`.
4. Stop the TUI with `Esc` then `Ctrl+C`.
5. Run one prompt that names the old spelling explicitly.
6. Repeat step 5 through the OpenAI-compatible HTTP surface if it is enabled, by
   posting one chat-completions request whose tool name is `bash`.
7. Repeat step 5 once with a Claude-spelled skill whose `allowed-tools` names
   only `bash`.

**Test data:** `Call the tool named exactly bash with {"command": "echo renamed"}. Do not call any other tool.`

**Expected results:**

- Step 2: none of the 13 old spellings appears anywhere in the listing - not as
  a tool, not in a count, not in a parenthetical.
- Step 3: every one of the 13 new spellings appears exactly once.
- The total the listing reports is the same number it reported before the
  rename; no tool was added, removed, merged or split.
- Step 5: the run does not execute a shell command. The transcript shows an
  unknown-tool result naming `bash`, and, if the similarity threshold allows, a
  did-you-mean hint naming `Bash`. `echo renamed` is printed nowhere.
- Step 6: the HTTP response carries the same unknown-tool refusal. `bash` does
  not resolve on any inbound path - prompt, HTTP surface, ACP, or skill.
- Step 7: the skill fails the same clean way rather than silently working on one
  tool and failing on the rest.

---

### TC-021 - A tool keeps working under its new name (FR-026, FR-030)

**Preconditions:** Prerequisites 1, 2 and 5.

**Steps:**

1. In the scratch project, run
   `ragent run "Use the Bash tool to print the working directory"` and approve
   the permission prompt if one appears.
2. Repeat with `Edit`:
   `ragent run "Use the Edit tool to replace the word alpha with beta in notes.txt"`.
3. Repeat with `Write`:
   `ragent run "Use the Write tool to create note.txt containing the single line hello"`.
4. Repeat with the search tools:
   `ragent run "Use Grep to find the word beta in this directory"` and
   `ragent run "Use Glob to list every .txt file here"`.
5. Repeat once with `Skill`, `TaskCreate` and `AskUserQuestion` if the session
   exposes them (the task tools and the question tool are visible in
   `/tools list`).

**Test data:** `Use the Bash tool to print the working directory`, `Use the Edit tool to replace the word alpha with beta in notes.txt`, `Use the Write tool to create note.txt containing the single line hello`, `Use Grep to find the word beta in this directory`, `Use Glob to list every .txt file here`

**Expected results:**

- Each run executes the named tool, and the tool-call line in the output names
  the new spelling (`Bash`, `Edit`, `Write`, `Grep`, `Glob`).
- The permission prompt, where one appears, names the same new spelling.
- `note.txt` exists in the scratch project after step 3 and contains `hello`.
- `notes.txt` contains `beta` after step 2 and no longer contains `alpha`.
- No run reports an unknown tool, and no run prints a did-you-mean hint.
- The turn count and step count are what they were before the rename: the new
  spelling does not cost an extra model request.

---

### TC-022 - Tools outside the rename set keep their names (FR-024, FR-025)

**Preconditions:** Prerequisites 1, 5 and 11.

**Steps:**

1. Start the TUI in the scratch project and type `/tools list`, then press
   `Enter`.
2. Search the listing for `webfetch`, `mf_fetch`, `websearch`, `mf_search`,
   `read`, `new_agent`, `bash_reset`, `apply_patch`, `patch`, `create`,
   `update_file`, `cron_add`, `cron_remove` and `cron_list`.
3. Search the listing again for `WebFetch`, `WebSearch`, `Read`, `Agent`,
   `CronCreate`, `CronDelete`, `CronList` and `TodoWrite`.
4. In the source, confirm the two refused spellings survived as alias targets
   only, if the change chose to keep them: open
   `crates/ragent-agent/src/tool/aliases.rs` or `tool/rename.rs` and read the
   alias entries.

**Test data:** none - this case reads the listing and the source.

**Expected results:**

- Step 2: every name is still present, spelled exactly as listed.
- Step 3: none of those names is present. `WebFetch` and `WebSearch` were
  refused because two ragent tools claim each and neither is an exact spelling
  match. `Read` and `Agent` were excluded because the workbook rates those
  Anthropic tools `Close`. `CronCreate`, `CronDelete` and `CronList` were
  excluded because the ragent rows for the cron tools are `Close`, and no ragent
  tool declares a `TodoWrite` equivalent.
- A refused spelling may appear as an alias entry (so a Claude skill naming
  `WebFetch` still reaches `webfetch`), but never as a registered tool.
- `bash_reset` still runs a shell command; it was not renamed to `BashReset` and
  it did not disappear.
- `read` keeps its lower-case spelling: the workbook rates `Read` `Close`, so
  the tool is not renamed and no `Read` alias exists.

---

### TC-023 - Permission rules, hooks and stored sessions survive the rename (FR-028)

**Preconditions:** Prerequisites 1, 3 and 5, plus a session created with the
pre-change binary whose transcript already contains tool calls under the old
spellings. Resume it through the session list the TUI offers.

**Steps:**

1. Add a permission rule to the project `.ragent/ragent.json` that denies the
   permission category the shell tool uses. Note that the rule names the
   *category* (`bash`), not the tool, and that a rule with no `pattern` is
   skipped, so the pattern must be present:

   ```
   "permission": [ { "permission": "bash", "pattern": "**", "action": "deny" } ]
   ```

2. Run `ragent run "Use the Bash tool to print the working directory"`.
3. Remove that rule and add a hook whose `matcher` names only an old spelling.
   Hooks are an array of objects, each with a `trigger` and a `command`:

   ```
   "hooks": [ { "trigger": "pre_tool_use", "matcher": "bash", "command": "true" } ]
   ```

4. Start the TUI in the scratch project and read any warning shown at load.
5. Resume the pre-change session and send one further prompt.
6. Stop the TUI.
7. Repeat step 4 with a matcher naming the new spelling (`Bash`) and confirm no
   warning is printed.

**Test data:** `Use the Bash tool to print the working directory`, and for step 5 `and then list the files here`

**Expected results:**

- Step 2: the deny rule still applies. The call is refused, and the refusal
  names `Bash`, not `bash`. This proves the permission *category* is unchanged
  even though the tool name is not: a rule written against the category keeps
  working.
- Step 4: exactly one warning is printed at load, naming the matcher `bash` and
  the renamed replacement `Bash`. The hook is not silently disabled without a
  word.
- Step 5: the earlier turns replay to the provider without error, and the old
  spellings in the stored transcript are neither rewritten on disk nor rejected
  by the provider.
- Step 7: no warning, and the hook fires on `Bash`.
- The permission categories listed by the permission documentation are
  unchanged: `file:read`, `file:write`, `bash:execute`, `plan`, `task` and
  `skill:manage` all still exist and still gate the same tools.
- No run panics, and no tool is classified as destructive merely because its
  stored name is unrecognised. A stored record naming a tool that no longer
  exists is handled as non-destructive.

---

### TC-024 - Every enumeration agrees, and the rename costs nothing at run time (FR-030, NFR-002, NFR-003, NFR-006)

**Preconditions:** Prerequisites 1, 2, 5 and 11, and the pre-rename baseline
recorded in TC-027 (part one).

**Steps:**

1. Record the surface size:
   `ragent run --dry-run "x"` and read the `tools: ready` section.
2. In the TUI, type `/tools list` and note the total it reports.
3. Run one prompt that uses two different tools:
   `ragent run --log-level debug "read Cargo.toml, then use Grep to find the word name in it"`.
   Note that `read` keeps its lower-case spelling after the rename; only the 13
   adopted names change.
4. Search the debug output for any rename or translation step that consults a
   before/after table:
   `ragent run --log-level debug "read Cargo.toml, then use Grep to find the word name in it" 2>&1 | grep -i "rename\|alias\|translate\|lookup"`
5. Send `/bug-report` in the TUI and read the `Tools registered` line.
6. Ask the agent to report the registry's own total, in a session with a model
   available: `call tool_info and tell me the total number of tools it reports`.
7. Check the emitted readiness report as JSON agrees with step 1:
   `ragent run --dry-run --json "x"`, and read the `tools` section.
8. Check the dependency manifests: `git diff --stat` on `Cargo.toml` and
   `Cargo.lock`, and search the changed Rust files for `unsafe`.
9. Ask the agent for the tool catalogue it can see:
   `call commands_info and list three slash commands that mention a tool`.

**Test data:** `read Cargo.toml, then use Grep to find the word name in it`

**Expected results:**

- Steps 1, 2, 5, 6 and 7 all report the same total, and it equals the baseline
  recorded in TC-027 before the rename: 148. `T-041-BASELINE.md` holds that
  number, the 21 family counts and the name list, measured before the rename.
- The per-family counts in step 1 are unchanged except for the tool names within
  each family; no family gains or loses a tool.
- Step 4 prints nothing about renames: no dispatch step consults the rename
  table, and tool resolution compares the new spelling directly.
- The two tool calls in step 3 are dispatched with the same latency character as
  before the rename - no new per-call lookup appears in the step timing.
- Step 9 returns catalogue text drawn from the registry: no entry names a
  retired spelling.
- Step 8 shows no new entry in `Cargo.toml` or `Cargo.lock` (NFR-002) and no
  `unsafe` block in any changed file (NFR-003).
- The names stay borrowed string comparisons: the debug log for step 3 shows no
  allocation-heavy per-call name translation and no table read in the dispatch
  path (NFR-006).

---

### TC-025 - The documentation names the new spellings and maps the old ones (NFR-005, NFR-007)

**Preconditions:** Prerequisites 1 and 11.

**Steps:**

1. In the repository root, search the documentation for an old spelling:
   `grep -rn "\`bash\`" README.md SPEC.md QUICKSTART.md TUI-QUICKSTART.md docs/howtos crates/*/DOCS.md`.
2. Search for the corresponding new spelling:
   `grep -rn "\`Bash\`" README.md SPEC.md QUICKSTART.md TUI-QUICKSTART.md docs/howtos crates/*/DOCS.md`.
3. Open `CHANGELOG.md` and read the newest entry's `Renamed` table.
4. Repeat steps 1 and 2 for `write` / `Write` and `grep` / `Grep`.
5. Search the same documents for a non-ASCII byte introduced by the change:
   `grep -rnP '[^\x00-\x7F]' README.md CHANGELOG.md docs/howtos crates/*/DOCS.md`.

**Test data:** none - this case reads the repository only.

**Expected results:**

- Step 1 finds no occurrence of the old spelling used as a tool name. A
  remaining occurrence is either the shell (`bash` the program), a sentence
  about the rename itself, or inside the `CHANGELOG.md` mapping table.
- Step 2 finds the new spelling wherever the document describes calling the
  tool.
- Step 3: the `Renamed` table lists all 13 pairs, one row each, with `before`
  and `after` in that order, so a reader upgrading an existing config can find
  the mapping without opening the workbook.
- Steps 1 and 2 also hold for the crate `DOCS.md` files, the howto pages under
  `docs/`, and the bundled skill bodies.
- Step 5 adds no non-ASCII byte: the documents were ASCII before the change and
  stay ASCII after it (NFR-005).

---

### TC-026 - No stale shell spelling survives, and a Claude-spelled skill loads its tools (FR-027, FR-029)

**Preconditions:** Prerequisites 1, 3, 5 and 11.

**Steps:**

1. In the repository root, run the source guard the rename added:
   `scripts/check-tool-rename.sh`. Read anything it reports.
2. Search the non-test source for one retired spelling by hand and compare:
   `grep -rn "\"bash\"\|\`bash\`" --include=*.rs crates/*/src src/`.
3. Read the eleven tool descriptions that used to name a renamed tool, and
   confirm each now names the new spelling: `bash_reset`, `rm`,
   `append_to_file`, `create`, `multi_edit`, `list`, the four `codeindex_*`
   tools, and `edit`.
4. In the TUI, type `/prompt primary` and press `Enter`, and read the assembled
   system prompt. Search it for old spellings. This makes no model call.
5. In the scratch project, write a skill whose manifest restricts its tools,
   naming them exactly as they are registered after the rename - `Grep` and
   `Glob` in the Claude spelling the workbook made ragent adopt, and `read` in
   its unchanged ragent spelling - at `.agents/skills/rename-check/SKILL.md`:

   ```
   ---
   name: rename-check
   description: verify Claude-spelled allowed_tools resolve
   allowed-tools: read, Grep, Glob
   ---

   Read notes.txt and report the single word in it.
   ```

6. Start the TUI in the scratch project, type `/skills`, press `Enter`, then
   invoke the pack with `/skill rename-check`.
7. Send one further prompt after the skill run.
8. Edit the manifest to add a spelling that is not a registered tool
   (`allowed-tools: read, Grep, Glob, ReadFile`), restart the TUI, and invoke
   `/skill rename-check` again.
9. Stop the TUI.

**Test data:** for step 6, no free-text prompt - the `/skill rename-check`
trigger carries the instruction; for step 7, `now summarise what you just read`

**Expected results:**

- Step 1: the guard exits zero. Any hit it prints is inside the rename table,
  the `CHANGELOG.md` entry, or a line carrying an explicit commented exemption
  for a string that is not a tool name.
- Step 2: every hit is either a comparison against the new spelling in a
  comment, or lives in a test file outside the guard's scope, or is the shell
  program name in a comment. No behavioural site still keys on `bash`.
- Step 3: no shipped description tells the model to call a tool by a name that
  no longer resolves.
- Step 4: the assembled prompt uses the new spellings throughout. A stale prompt
  instructs the model to make a call that now fails, and no run may do that.
- Step 6: `/skills` lists `rename-check`, and invoking it runs with exactly
  `read`, `Grep` and `Glob` available. The run reads the file and reports `alpha`;
  no unknown-tool error appears, because the registry now holds those exact
  names and no translation was needed. Note that `Read` is deliberately absent:
  the workbook rates it `Close`, so `read` kept its name. A skill written for
  Claude Code that names `Read` is therefore reported, not silently narrowed.
- Step 7: the unrestricted tool surface returns - every other tool is available
  again, and the step behaves as an ordinary prompt.
- Step 8: one report names the offending entry `ReadFile` and the skill that
  declared it. The skill does not silently run with a smaller surface, and the
  report is printed once, not once per turn.
- No step panics, and no step writes into the repository.

---

### TC-027 - The surface baseline is recorded before the rename and met after it (FR-030)

**Preconditions:** Prerequisites 1, 2 and 5. This case needs both binaries and is
best run as a two-part check: part one before the rename lands, part two after.

Part one has already been recorded: `specs/uniformtools/T-041-BASELINE.md` holds
the pre-rename `--dry-run` total and family table, the 148-name list with its
digests, and the `/bug-report` total, all measured on the pre-rename tree. Steps
1 to 3 below are therefore a spot-check of that record on your own build, not a
new measurement, and it is the record that steps 5 and 6 compare against.

**Steps:**

1. With the pre-change binary, in the scratch project, run
   `ragent run --dry-run "x"` and copy the whole `tools` section into a file
   outside the repository, for example `~/scratch/baseline-tools.txt`.
2. Still with the pre-change binary, start the TUI and type `/tools list`, then
   copy the reported total and the full list of names into the same directory as
   `~/scratch/baseline-names.txt`.
3. Stop the TUI.
4. With the finished binary, repeat steps 1 and 2 and save the results as
   `~/scratch/after-tools.txt` and `~/scratch/after-names.txt`.
5. Compare the totals and the family counts:
   `diff ~/scratch/baseline-tools.txt ~/scratch/after-tools.txt`.
6. Compare the name lists, sorted:
   `sort ~/scratch/baseline-names.txt > ~/scratch/a.txt; sort ~/scratch/after-names.txt > ~/scratch/b.txt; diff ~/scratch/a.txt ~/scratch/b.txt`.

**Test data:** `ragent run --dry-run "x"` and `/tools list` in each build.

**Expected results:**

- Steps 1 and 2 report 148 tools and the 21 family counts recorded in
  `T-041-BASELINE.md`; check `agent` 3 and `team` 20, because a stale binary in
  `target/` also reports 148 with a different split.
- The two totals are identical. The rename changed 13 names and added,
  removed, merged or split nothing.
- Every family count is identical. The families are derived from the permission
  category, which the rename does not touch.
- Step 6 shows exactly 13 removals and 13 additions, and each removal pairs with
  an addition per the rename table. No other line differs. The sorted after-list
  hashes to `308d92d9d975666cf1f815369e18cc7b457c1c0dcbab004c45f2835100368321`
  when joined by a newline with no trailing newline, and no registered name
  carries an uppercase letter before the rename against exactly 13 after it.
- No family name changed: the family comes from the permission category, not
  from the tool name.
- The baseline is recorded in the change description, not only in these files:
  a reader of the change can see the number T-037 compares against. It is in
  `T-041-BASELINE.md` and in the `CHANGELOG.md` Unreleased entry.

---

### TC-028 - Provider spellings ragent did not adopt still resolve, and the narrowing holds (FR-015, FR-025, FR-026)

**Preconditions:** Prerequisites 1, 3 and 5. Alias resolution enabled in the
project config:

```
"tools": { "aliases": true }
```

**Steps:**

1. Restart the TUI so the config is re-read.
2. Ask for a tool by a refused Claude spelling:
   `use the WebFetch tool to download https://example.com and tell me the page title`.
3. Read the tool block and the result.
4. Ask for the ragent spelling directly:
   `use the webfetch tool to download https://example.com and tell me the page title`.
5. Disable aliases (`"aliases": false`), restart, and repeat step 2.
6. Open the alias source file the change names and read every entry.
7. Set `"tools": { "aliases": true, "expose_alias_names": true }`, restart,
   then in the TUI type `/prompt primary` and press `Enter`. Read the assembled
   system prompt and its effective tool surface for the current agent. Because
   this only prints what the agent would receive, no model call is made.

**Test data:** `use the WebFetch tool to download https://example.com and tell me the page title`, `use the webfetch tool to download https://example.com and tell me the page title`

**Expected results:**

- Step 2, aliases on: the call reaches `webfetch` and the page title is reported.
  The tool block names the canonical tool, `webfetch`.
- Step 4: identical behaviour, reached without a table lookup.
- Step 5, aliases off: the call is refused as an unknown tool named `WebFetch`,
  with `webfetch` suggested.
- Step 6: no alias entry maps a spelling onto the identical string - no no-op
  self-mapping. No entry maps `Bash`, `Edit`, `Write`, `Glob`, `Grep`, `Skill`,
  `TaskCreate`, `TaskGet`, `TaskList`, `TaskUpdate`, `AskUserQuestion`,
  `EnterPlanMode` or `ExitPlanMode`, because those are ragent names now and
  FR-026 forbids a second spelling for them.
- Step 7: the advertised tool list gains the remaining provider spellings and
  nothing else. In particular `Bash` appears once, never twice.

---

### TC-029 - No configuration restores a retired spelling, and no bridge can either (FR-031, FR-032)

**Preconditions:** Prerequisites 1, 3, 5 and 11, a finished build that includes the
rename, and a scratch project whose `.ragent/ragent.json` you may edit.

**Steps:**

1. Open the scratch project's `.ragent/ragent.json` and add an alias whose
   source is a retired spelling:

   ```
   "tools": { "aliases": { "bash": "Bash" } }
   ```

2. Start the TUI in the scratch project and read the startup output.
3. Fix the entry to a provider spelling ragent did not adopt, restart, and
   confirm the session starts:

   ```
   "tools": { "aliases": { "WebFetch": "webfetch" } }
   ```

4. Point the alias target at a name that is not registered:

   ```
   "tools": { "aliases": { "WebFetch": "bash" } }
   ```

5. Restart and read the startup output.
6. Try the self-mapping form, which asks for two spellings of one tool:

   ```
   "tools": { "aliases": { "Bash": "Bash" } }
   ```

7. Restart and read the startup output.
8. Configure an MCP server whose tool list you control, declaring a tool named
   `bash` alongside a second tool named `notes_read`, and enable it.
9. Read `/mcp` and `/tools list`, then ask for both tools by their namespaced
   names.
10. Ask for the retired spelling by the bare name `bash`.
11. Try an environment variable and a CLI flag that look like they might restore
    a retired name - for example `RAGENT_TOOL_ALIAS=bash=Bash` and a
    `--tool-alias` flag - and record that neither is accepted.
12. Stop the TUI.

**Test data:** the four `tools.aliases` blocks above; for step 9,
`use the mcp_<server>_notes_read tool to list my notes` and
`use the mcp_<server>_bash tool to echo hi`; for step 10, `use the bare tool named
bash to echo hi`; for step 11, `RAGENT_TOOL_ALIAS=bash=Bash ragent run "list the
registered tools"` and `ragent run --tool-alias bash=Bash "list the registered
tools"`

**Expected results:**

- Step 2: the run is refused at load. The error names the config file, the
  spelling `bash` and its replacement `Bash`. The session does not start with
  `bash` still resolvable.
- Step 3: the session starts, and the alias resolves `WebFetch` to `webfetch`,
  because `WebFetch` is a refused Claude spelling, not a retired ragent name.
- Step 5: the run is refused at load. The error names the file and the target
  `bash`, and points at `Bash`. Pointing an alias at a retired spelling is
  rejected exactly as naming one as a source is.
- Step 7: the self-mapping entry is refused, because a key mapped onto the
  identical string asks for two spellings of one tool. The error names the file
  and the spelling.
- Step 8: both tools bridge, because both are namespaced. The server's `bash`
  tool registers as `mcp_<server>_bash`, which is not the retired `bash`, so it
  cannot make `bash` resolvable and is not refused; `notes_read` registers as
  `mcp_<server>_notes_read`. Nothing is reported, because nothing needed to be.
- Step 9: both tools appear under their namespaced names and run. `/tools list`
  shows no second `Bash` row, and no bare `bash` row - the namespaces are what
  keep the server out of the bare ragent namespace (FR-032 clause 1). The TUI's
  stale-server scan finds them by their `mcp_<server>_` prefix, which is the same
  derivation the registry used.
- Step 10: the bare `bash` call resolves to nothing (its ordinary unknown-tool
  refusal, FR-026), because the registry never held it. The session stays usable
  and no panic occurs.
- A refusal (clause 2) fires only for a bridge that surfaces a name outside its
  namespace - the shape of the regression the backstop exists for, since the
  shipped mangler always namespaces. It is covered mechanically rather than by
  hand: `crates/ragent-agent/tests/test_fr032_bridge_containment.rs` drives it with a
  test-supplied mangler and asserts the sentence names the source, the spelling and
  the `after` replacement, that the refusal is reported once and not per turn, and
  that the server keeps its other tools.
- Step 11: the `--tool-alias` flag is rejected by the argument parser as an
  unexpected argument, and the command exits non-zero without running. The
  `RAGENT_TOOL_ALIAS` variable is not read anywhere and has no effect: the run
  completes and `bash` stays unresolvable. Record both outcomes verbatim in the
  change description - the flag failing loudly and the variable being ignored
  are different results, and a reader rebuilding the audit needs to know which
  is which.
- No step panics, and the only files written are the config edits you made.

---

### TC-030 - Nothing offers a retired spelling, and the guard holds the line (FR-033, FR-034, NFR-008, NFR-009)

**Preconditions:** Prerequisites 1, 3, 5, 11 and 12.

**Steps:**

1. In the TUI of the scratch project, type `/ba` and read the completion list;
   then `Esc` and type `/ba` again followed by `Alt+Backspace` to clear it.
2. In the input field (not the slash menu), type `ba` and read whatever the
   prompt-field completion offers, if it offers anything.
3. Ask the model to use a retired spelling directly:
   `call the bash tool with the argument echo hi`.
4. Read the tool block and the refusal text.
5. Run the guard script on a clean tree and note its exit code and runtime.
6. Create a deliberate violation of each NFR-008 predicate in turn, running the
   guard after each and reverting the edit before the next:

   - **(a)** add `"bash": "Bash"` to the alias table in source, then revert it
     and add an entry mapping `FileSearch` -> `FileSearch`;
   - **(b)** add a resolver match arm whose source is `bash`;
   - **(c)** add `bash` to a default configuration value;
   - **(d)** use `bash` as a registry key;
   - **(e)** write the literal `` `bash` `` (backticked) in a non-test source
     comment with no exemption marker;
   - **(f)** register a second tool named `Bash`;
   - **(g)** add a tool whose `execute` only calls another registered tool's
     `execute`.

7. Read each guard report. Then place three of the violations at once and
   confirm one run reports all three.
8. Confirm the guard makes no network request and does not read `tools.xlsx`:
   run it with the network interface down (or `unshare -n` if available) and
   with `tools.xlsx` temporarily renamed outside the repository. Also run it the
   second way the change ships it - as the automated case in the same change -
   and compare the report with the `scripts/` form.
9. Land the rename check: confirm in the change history that the commit which
   registers `after` is the commit that removes `before`, and that at that
   commit `ragent run --dry-run "x"` reports the FR-030 count and the workspace
   tests pass. A commit that does both, neither, or resolves both spellings is
   a failure of FR-034.
10. Restore `tools.xlsx` and revert every edit from step 6.

**Test data:** `/ba`, `ba`, `call the bash tool with the argument echo hi`

**Expected results:**

- Step 1: the slash-command completion lists only registered slash commands.
  No entry beginning `ba` offers a tool by a retired name.
- Step 2: the field offers **no** tool-name completions, and that is the recorded
  result rather than a gap. The prompt field's only completion surface is the `@`
  file picker, which completes project paths; typing `ba` opens nothing, and
  `@` lists paths only. FR-033's last clause says a field that offers no tool name
  today need not start, so `Bash` is offered by the `/` menu and the model-facing
  tool list - never by the input field.
- Step 4: the call is refused as an unknown tool named `bash`, the did-you-mean
  hint names `Bash`, and nothing executes. `echo hi` never runs.
- Step 5: the guard exits zero on a clean tree and completes in a few seconds -
  fast enough to sit in the ordinary test run without a timeout exception.
- Step 7: each of the eight deliberate violations is reported, naming the
  predicate letter, the file and line, and the exact edit required. The
  self-mapping entry (`FileSearch` -> `FileSearch`) is reported even though it
  maps no retired name, because a no-op entry is dead weight the resolver should
  not carry. All offending sites are reported in one run, not just the first -
  with several violations in place at once, the report names every one. Every
  message is ASCII.
- Step 8: the guard still exits zero on a clean tree with the network down, and
  still exits zero with `tools.xlsx` absent. A guard that needed the workbook
  would fail here. The `scripts/` form and the automated case report
  identically, because they are one implementation.
- Step 9: the history shows one commit that both registers and removes, and the
  dry-run count at that commit equals the baseline recorded in TC-027.
- Step 10: `git status --short` shows the tree restored; no test edit remains.
- No step panics, and no step writes into the repository outside the deliberate
  edits you reverted.

---

## Cleanup

1. Stop any running TUI with `Esc` then `Ctrl+C` in the terminal.
2. Remove the trace artifacts created by TC-018:
   `rm -rf ~/scratch/uniformtools-tests/target/temp/wire-trace`
3. Remove any file the agent created during the tests, in the scratch project
   only: `rm -f ~/scratch/uniformtools-tests/hello.txt ~/scratch/uniformtools-tests/note.txt`.
4. Revert the test config changes made in TC-008, TC-013, TC-018, TC-023, TC-026,
   TC-028 and TC-029 by deleting the `experimental`, `tools`, `mcp`,
   `permission` and `hooks` blocks from the scratch project's
   `.ragent/ragent.json`, or deleting that file entirely if it was created by
   these tests.
5. Delete the scratch skill used by TC-026:
   `rm -rf ~/scratch/uniformtools-tests/.agents/skills/rename-check`.
6. Delete the comparison files TC-027 wrote outside the repository:
   `rm -f ~/scratch/baseline-tools.txt ~/scratch/baseline-names.txt ~/scratch/after-tools.txt ~/scratch/after-names.txt ~/scratch/a.txt ~/scratch/b.txt`.
7. Remove the temporary MCP server TC-029 used, if you wrote one:
   `rm -rf ~/scratch/uniformtools-tests/mcp-collision`.
8. Confirm `tools.xlsx` is back in the repository root after TC-030's step 8
   renamed it aside: `ls -l tools.xlsx`. A missing workbook is not a test
   result.
9. Confirm the repository working tree is unchanged by the test run:
   `git status --short` in the repository root must show no modified or
   untracked files created by the tests. Any file the tests wrote inside the
   repository is a failure of the workspace rule that a run leaves the tree
   byte-identical and must be reported, not deleted. Every deliberate
   violation TC-030's step 6 introduced must already be reverted - re-read
   `git diff` for the guard script, the alias table, the registry and the tool
   sources and expect it empty.
10. Unset any environment variables set only for these tests
    (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `OLLAMA_HOST`, and the
    `RAGENT_TOOL_ALIAS` TC-029's step 11 used to prove it is not honoured) if
    they were added to your shell for the session.
11. Remove the pre-change binary copy from prerequisite 2 once TC-017, TC-024
    and TC-027 have been signed off:
    `rm -rf ~/scratch/ragent-before`.

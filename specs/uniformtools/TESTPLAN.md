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

**Automated counterpart (T-003):** the coverage half of FR-002 - every
registered provider id has a declared profile, and every profile a row names is
one of the six declared families - is pinned by
`crates/ragent-llm/tests/test_wire_profile_table.rs` (13 cases). It is hermetic:
it builds the provider registry offline from `create_default_registry()` (no
client is constructed, no credential is read) and reads the library's `const`
tables, so it needs no provider, no network and no scratch project. The cases a
reader of this manual case should know about are
`..._declares_every_family_once` (`FAMILIES` holds the six families, each used by
some row, labels ASCII and ascending),
`..._every_row_names_a_declared_family` (a row assigns an id to a declared
family and never composes a seventh contract) and
`..._source_declares_no_family_outside_the_list` (the module's
`pub const X: ToolWireProfile` declarations and `FAMILIES` hold the same set).
Run it with `cargo test -p ragent-llm --test all test_wire_profile_table::`.

**Automated half (T-022).** The printing half of this case - the table itself, its
cells, its ASCII-ness and its exit behaviour - is pinned by two files.
`crates/ragent-llm/tests/test_wire_matrix.rs` (8 cases, hermetic, no binary spawn)
covers the renderer: `..._prints_one_row_per_provider` (one row per `PROVIDER_WIRE`
id, steps 2 and 3's count), `..._has_no_blank_cell` (ten populated columns per row,
no cell empty, `-` or `none` - the expected-results bullet about empty fields),
`..._is_plain_ascii` (printable ASCII only and no line with trailing whitespace),
`..._shows_the_declared_contract_not_a_second_copy` (Gemini's `synthesised` id,
Responses' `by_call_id` keying, Bedrock's `toolUseId`, Anthropic's reasoning `yes`,
and the two Ollama families differing on argument encoding - so a table restating the
fields in prose, or reading them from somewhere other than the profile constants,
fails), `..._is_sorted_and_order_independent` (reversed input renders identical
bytes), `..._refuses_a_provider_with_no_declared_profile` (the error names the
provider and a mixed known/unknown list renders nothing), and
`..._headings_match_the_columns_each_row_prints`. `tests/test_wire_matrix_flag.rs`
(4 cases, root crate) covers the flag as the user meets it, in a fresh tempdir with
an isolated `HOME`/`XDG_CONFIG_HOME`: `..._prints_one_row_per_provider` (exit 0 and
step 3's comparison - every provider `ragent models` names has a row, rows sorted and
unique), `..._populates_every_column`, `..._exits_without_network_or_session_side_effects`
(finishes in under 10 s, where a connect attempt alone is seconds, and leaves no
database, session store or config root behind - the "exits immediately without
contacting any provider" bullet) and `..._is_documented_in_help`. Run them with
`cargo test -p ragent-llm --test all test_wire_matrix::` and
`cargo test -p ragent --test all test_wire_matrix_flag::`.

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

**Automated half (T-014).** "The same order in both modes" is asserted without a
provider at all, by running one scripted step twice through
`SessionProcessor::process_message` with `experimental.parallel_tool_calls` off
and on. `test_unified_dispatch::test_sequential_and_parallel_paths_produce_the_same_outcomes`
pins the same *verdicts* (one success, one schema refusal, one unknown-tool
refusal, identical sentences) and
`test_unified_dispatch::test_outcomes_keep_the_provider_order_in_both_modes` pins
the same *order*, unsorted, on a step whose first call is refused - the case that
caught the concurrent arm recording a refusal inline while its siblings were
still running. `test_unified_dispatch::test_the_permission_stage_runs_once_per_call_in_both_modes`
pins that the permission stage is a stage of that one pipeline: exactly one
`PermissionRequested` per call in both modes. The structure behind those cases is
guarded by `test_unified_dispatch::agent_loop_calls_the_one_stage_body_and_duplicates_no_refusal`
(the loop defines no stage and calls the pipeline once),
`parallel_arm_drains_one_ordered_list_of_slots` (one slot per call, drained in one
ordered pass) and `pipeline_stages_run_in_the_documented_order` (the stages are
ordered, not merely present).

**Replay half (T-026, NFR-004).** "Both families, and neither needs a live
provider" is what `test_wire_replay::` asserts: one recorded response body per
provider family, replayed by a loopback `TcpListener`, parsed by that family's own
client, then fed back through the family's encoder - `OPENAI_CHAT`,
`ANTHROPIC_MESSAGES`, `GEMINI`, `BEDROCK_CONVERSE`, `OPENAI_RESPONSES` and
`OLLAMA_NATIVE`, six families in seven cases. The two families this case names
are the first two: each replay must open exactly one call, carry its arguments,
end it once with the same id, and re-encode the assistant turn in that family's
own vocabulary (`tool_calls` with a string `arguments` for OpenAI, `tool_use`
with an object `input` preceded by the signed `thinking` block for Anthropic).
The recorded bodies are the services' documented stream shapes, not our own
output, so a decoder and an encoder that agree only with each other fail here
rather than in production. The cases need no credential, no network and no model.

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

**Automated half (T-008).** Every expectation above that is a property of the
*stream layer* rather than of a live model is pinned hermetically in
`crates/ragent-llm/tests/test_wire_decode.rs`, which needs no provider and no
network (NFR-004). Step 3's "two blocks in one message" and step 4's "each block
carries its own arguments, neither a concatenation of the other's" are
`test_accumulator_by_index_attributes_interleaved_deltas_to_their_own_block`
(four fragments alternating between blocks 1 and 2, each landing on the block its
frame names, then closed in the reverse of the open order) and
`test_a_converse_block_keys_by_content_block_index_not_by_id` (the Bedrock
Converse shape: two `toolUse` blocks keyed by `contentBlockIndex`, interleaved
1/0/1/0, each ending once). Step 3's "before any tool result appears" is
`test_accumulator_emits_exactly_one_start_and_one_end_per_call` - a provider that
repeats `function.name` on every argument frame emits no second start, and a
repeated stop frame emits no second end. The last expected bullet - "the ids are
non-empty and unique, and neither id is used to work out which tool was called" -
is `test_accumulator_synthesises_provider_counter_name_ids_for_unnamed_calls` and
`test_accumulator_by_call_id_keying_tracks_the_responses_family`, plus
`test_the_provider_modules_no_longer_keep_their_own_tool_call_maps`, a source scan
over the nine provider modules that fails if any of them goes back to keeping its
own block map. The "Repeat the case on Anthropic and on Bedrock" bullet's
"no call's arguments are lost when the two streams interleave" is the same
interleaving case on `ANTHROPIC_MESSAGES` and on `BEDROCK_CONVERSE`, which is
also where the old behaviour lost a call: the Converse path held one
`active_tool_call_id` for all open blocks. One behaviour this case must not
report as a regression: a Converse frame that omits `contentBlockIndex` is now
skipped with a warning instead of being attributed to the last-opened call, so a
*malformed* stream produces a missing argument rather than a wrong one.

**Automated half (T-009).** The Bedrock half of the last bullet - "no call's
arguments are lost when the two streams interleave" - is now pinned through the
**real Converse stream parser** rather than the accumulator alone.
`crates/ragent-llm/tests/inline/bedrock_tests.rs` gained
`test_converse_chat_stream_keeps_two_parallel_tool_use_blocks_apart`: two
`toolUse` blocks open back to back, four `toolUse.input` fragments interleave
`1, 0, 1, 0`, the stop frames arrive `1, 0`, and the case asserts two starts
(`tu_a Read`, `tu_b Grep`), that concatenating each call's fragments **by id**
yields exactly `{"path":"a.rs"}` and `{"pattern":"fn main"}` (neither absorbing
the other's input), and exactly one end per call in stop-frame order.
`test_converse_stream_drops_a_frame_that_names_no_block_index` is the
"malformed frame is a missing argument, not a wrong one" note as a case: a
`toolUse` delta with no `contentBlockIndex` between two indexed fragments reaches
no call, the named call keeps only its own fragments, and both calls still end
exactly once. Structurally,
`crates/ragent-llm/tests/test_wire_decode.rs` carries
`test_no_stream_attributes_a_frame_to_the_call_opened_last`, a source scan over
`wire/decode.rs`, `bedrock.rs`, `anthropic.rs`, `gemini.rs` and `sse.rs` that
fails on `delta_last(`, `end_last(`, `.last_mut()` and `active_tool_call_id` - the
last-opened helpers are deleted rather than left unused, so the guess is not one
method call away for the next provider that cannot find its index.
`test_a_converse_block_keys_by_content_block_index_not_by_id` covers the pure
state-machine half. **Not covered by a test:** the live-model wording of step 2
(that a real Nova/Claude model emits its stop frames in the order these cases
assume) and the TUI rendering in step 3; both need a provider and a terminal
(NFR-004 keeps them out of the suite).

**Automated half (T-010).** The last bullet's FR-019 clause - "the displayed ids
are non-empty and unique, and neither id is used to work out which tool was
called" - is now pinned end to end for the two providers that supply no ids of
their own. `crates/ragent-llm/tests/inline/ollama_cloud_tests.rs` drives the real
native Ollama Cloud stream with two id-less `tool_calls` frames and asserts the
starts are `ollama_cloud_0_read_file` and `ollama_cloud_1_read_file`, each
frame's arguments landing on its own call; the frame-local index restarts at zero
in each frame, which is the fold the stream-scoped counter exists to prevent.
`crates/ragent-llm/tests/test_gemini_tool_calls.rs` drives the real Gemini stream
with two `functionCall` parts in one frame and asserts `gemini_0_read_file` and
`gemini_1_read_file` with the two argument payloads kept apart.
`test_wire_decode.rs` gained
`test_accumulator_synthesises_provider_counter_name_ids_for_unnamed_calls` (the
`<provider>_<counter>_<name>` shape, the same id on the call's delta and end, the
name reaching the consumer whole for a tool called `read_file`, and `_unnamed`
for a frame with no name) and
`test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one`, a source scan
over ten provider modules and `wire/encode.rs` that fails on a minted
`format!("fc_`/`format!("tc_`/`_tc_{` id, on a name-shaped key
(`CallId(format!("unnamed_`) and on `tool_use_id.split`/`rsplit`/`.id.split('_')`
name recovery. `test_wire_encode_message.rs`'s
`test_a_gemini_result_never_recovers_a_tool_name_by_splitting_the_id` asserts the
*absence* of the derivation: an orphan id goes out unnamed rather than as the
`orphan` prefix of `orphan_tool_0`. **Not covered by a test:** the display of the
minted id in the log panel (step 3 is a TUI read, and the id text a human sees is
the console's, not the stream layer's).

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
- **Not every unusual payload is malformed.** A call whose argument text is
  empty or only whitespace runs a parameterless tool (the helper decodes it as
  `{}`), and a call whose argument text is a JSON *string* holding the object
  reaches the tool with that object - neither produces the corrective error above
  (FR-007; T-012). Ask the model for `think` with no arguments and for `think`
  with `"{\"thought\":\"x\"}"` to see both.

**Automated counterpart (T-013):** the whole case is pinned hermetically by
`crates/ragent-agent/tests/test_processor_more_paths.rs`, which drives a scripted
provider through `SessionProcessor::process_message` with no network and no
model: `test_non_object_arguments_are_still_refused` sends `[1, 2]` to
`tool_info` and asserts the tool never ran, the record is `ToolCallStatus::Error`,
the corrective text names the tool and what was found, it instructs a resend, and
the request the provider receives carries `is_error: true`. The sentence itself -
naming the tool, quoting the decoder verbatim, instructing a resend, saying the
tool was not executed - is
`crates/ragent-agent/tests/test_tool_input_error.rs::undecodable_refusal_names_quotes_and_instructs`,
where the quote is compared against the decoder's own `Display` output rather
than a copy of it.

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
- A result block names the missing parameter and states that it is required,
  **and** states the declared type the parameter must have.
- `move_file` does not execute: no file is renamed.
- The result is marked as an error.
- If the model resends with both arguments, the retry executes normally.

**Automated counterpart (T-013):** step 3 is
`crates/ragent-agent/tests/test_tool_input_error.rs::schema_refusal_names_the_parameter_and_its_type`,
which runs `validate_required_args` over the real shape (a required `destination`
declared `string`) and asserts the sentence names the tool, the parameter and
`string`. The whole call is
`crates/ragent-agent/tests/test_processor_more_paths.rs::test_schema_violation_names_the_parameter_and_is_marked_on_the_wire`,
which drives a `move_file` call carrying only `source` through
`SessionProcessor::process_message` and asserts the record is
`ToolCallStatus::Error`, the sentence comes from the shared constructor (the
pre-T-013 `Invalid tool arguments:` prefix must not reappear), and the request
the provider receives carries `is_error: true`.

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

**Automated half (T-015).** The procedural steps above need a provider that
declares `supports_parallel: false`, and **no registered provider does today** -
every declared family sets the field to `true`, so step 3 can only be satisfied
by editing the profile table, which is the one edit a test must not make. What
the automated cases pin instead:

- `crates/ragent-agent/tests/test_parallel_gate.rs`
  `test_a_parallelising_provider_keeps_the_concurrent_arm` - a row that declares
  `true` is not demoted, logs nothing, and still dispatches the step
  concurrently (its permission prompts all arrive before the calls close).
- `...::test_a_non_parallelising_provider_is_demoted_once_and_keeps_the_history` -
  a provider with no declared profile runs the same step sequentially: same
  recorded history as the same step with the opt-in off, the same event order
  byte for byte, and **exactly one** log line naming the provider and the reason.
  A second run asserts that line is absent when the opt-in is off.
- `...::the_gate_sits_between_the_config_read_and_the_first_call` - the gate
  reads the config, resolves the profile, forces sequential, and only then
  prepares the step's calls; exactly one `tracing::info!` sits inside it and the
  block can never set the flag back to `true`.
- `...::every_family_profile_states_supports_parallel_and_the_loop_is_its_reader`
  and `...::the_rows_used_here_declare_the_parallelism_they_are_named_for` - the
  field is declared by every family, rendered by the wire matrix, and the rows
  these cases rely on still say what they assume.

The `false` row itself is covered by no automated case: see
`T-015-PARALLEL-GATE.md` section 5, which records that limit rather than hiding
it. The router's downstream family is likewise not resolved (section 6), so a
`router` session is gated by the `router` row, not by the provider its
classification picks.

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

**Automated half (T-016).** The procedural steps need a human who deliberately
leaves a TUI dialog unanswered, which no test can be. What the automated cases
pin instead is the invariant those steps observe, in
`crates/ragent-agent/tests/test_approval_blocks_result.rs` (registered in
`tests/all.rs` only by this task - it had been an orphan file since an earlier
pass, so its cases had never run):

- `history_append_runs_only_after_the_permission_verdict_resolved` and
  `the_permission_stage_marks_then_settles_the_call` - the unresolved interval is
  a tracked state: the dispatch pipeline's permission stage marks the call
  before the prompt and settles it when the verdict resolves, and both sit after
  the call's `ToolCallStart`. The step's `chat_messages` push comes only after
  every call of the step has been awaited.
- `the_append_guard_withholds_only_the_unresolved_call` - the decision the tool
  phase reads: a call the tracker still names unresolved is not appendable; a
  sibling call of the same step, and the same call id in another session, both
  are; a settled call is appendable again (a denial and an allow both resolve
  it).
- `a_second_mark_keeps_the_call_unresolved_until_the_last_settle` - the bash
  case where one call raises a prompt per sub-command.
- `test_denied_write_writes_nothing_and_surfaces_a_denied_error` and
  `test_approved_write_executes_and_returns_success` - end to end: a denied
  write writes nothing and the next request carries an `is_error` denial
  observation; an approved write runs and its result is appended with
  `is_error: false`.

The guard's withholding arm is not reached by any automated case, and the reason
is structural rather than convenient: the decision is made inside the tool
phase's result handler, a closure over the loop's own buffers, and the
test-visible pipeline is `pub(crate)` behind a `pub(crate)`
`PendingToolCall`. Reaching it would mean publishing the dispatch surface for one
assertion. What is proven is the composition - the mark and settle, the
decision, and the handler's consultation of it - with the seam recorded in
`T-016-RESULT-BLOCK.md` section 4.

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

**Automated counterpart (T-013, record half):** the flag's *origin* - that
`is_error` is derived from the recorded status and never re-derived at a call
site - is pinned by
`crates/ragent-agent/tests/test_tool_input_error.rs::is_error_is_derived_from_the_status_type`
(the flag is `ToolCallStatus::is_error()`, and `history.rs`,
`compaction/convert.rs` and `processor.rs` write neither `is_error: !success` nor
a status-to-`Error` comparison) and by
`...::every_refusal_is_an_error_by_the_status_derivation` (both refusal causes
report `ToolCallStatus::Error`, and their `is_error()` **is** the status's own
derivation). What the provider actually receives is asserted on the captured
request in
`test_processor_more_paths.rs::test_schema_violation_names_the_parameter_and_is_marked_on_the_wire`
and `...::test_non_object_arguments_are_still_refused`.

**Automated half (T-017, transmit side):** what the *encoder* does with the flag
is pinned by `crates/ragent-llm/tests/test_tool_result_error_marking.rs` (7
cases): every format derives its marking rule from its declared message profile
(`transmits_is_error_natively`), with exactly Anthropic and Bedrock native and
the other four prefixed; a non-native format prefixes an error and never a
success; the marker is the literal six bytes `ERROR: ` (including
`error_marked("", true)`, so an empty failure is still an error result); the
named-format path a dialect uses leaves a native format's content untouched; a
definition-time name rewrite does not reach the message layer; and - end to end
- a throw-away HTTP server captures the real `OllamaProvider` request and
asserts a failed result arrives as `ERROR: No such file` while a successful one
in the same request is byte-identical. The flag's *origin* stays covered by the
cases above, and
`test_tool_input_error::every_history_converter_forwards_the_record_status_to_the_wire_flag`
adds the positive half the negative scan lacked: each history converter still
contains its `is_error` derivation. **Not covered by a test:** the prefix is
asserted per carrier and per dialect, not across a live provider round trip;
the manual steps above remain the check that a real model reads the marker as a
failure, because no case can assert what a model decides.

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

**Automated half (T-014).** `test_unified_dispatch::test_unknown_tool_is_refused_with_a_did_you_mean_hint`
scripts `rename_files` and asserts the refusal names it, offers the closest
registered name, and says nothing ran (the file is not created);
`test_unknown_tool_with_no_near_neighbour_offers_no_suggestion` asserts a name
nothing is close to is refused **without** inventing a hint;
`test_unknown_tool_stops_before_the_permission_prompt` asserts the user is never
asked to approve a call that cannot run; and
`test_unknown_tool_refusal_precedes_every_guard` pins the position of the
unknown-tool check between the decode and the schema check.
`test_refusal_outcome::an_unknown_tool_refusal_is_a_terminal_error_outcome` pins
the terminal shape: the record keeps the spelling the provider sent, its status is
`Error`, it publishes exactly one `ToolCallEnd` carrying the same sentence, and it
publishes **no** `ToolResult` - a refused call must not reach a client as a
successful call with empty output.

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

**Automated half (T-018).** The manual steps above are the end-to-end proof; the
half a machine can pin is that the block is *stored with its signature* and is
*replayed within the same turn*, which no single-layer test covers:

- `test_loop_steps_harness::test_the_echoed_reasoning_block_precedes_the_tool_call_in_the_same_turn`
  drives `process_message` against a scripted provider that streams
  `ReasoningStart` / `ReasoningDelta` / `ReasoningEnd` / `ReasoningSignature` and
  then a tool call, and asserts on the **second captured request** - the one that
  answers the call - that the assistant turn carries a `Reasoning` part *before*
  its `ToolUse` part with both the text and the signature intact. This is the case
  a test driving `encode_message` directly cannot write: the echoing request is
  built from `chat_messages`, so a reasoning part that exists only in the saved
  `MessagePart`s would first reach the provider on the following turn.
- `test_loop_steps_harness::test_the_persisted_reasoning_part_keeps_its_signature`
  asserts the saved `MessagePart::Reasoning` holds the streamed signature and that
  the saved parts begin `reasoning, tool_call`, so a resumed session replays a
  signed block rather than an unsigned paraphrase.
- `test_wire_encode_message::test_anthropic_replays_the_signed_thinking_block_before_the_call`
  and `...::test_a_profile_without_echo_reasoning_still_omits_the_block` cover the
  other half: the `thinking` block is written, byte for byte, only for a profile
  that declares `echo_reasoning`, and `...::test_an_unsigned_reasoning_block_is_replayed_without_a_signature`
  pins that a block recorded without one gains no `signature` key and no `null`.

**Not covered by a test.** Step 3's precondition - that a real thinking-capable
model accepts the replayed block without a "thinking block missing" or "signature
required" error - is provider behaviour and needs a live model. The automated
cases prove the harness sends what Anthropic's contract asks for; only the manual
run proves Anthropic accepts it. Step 7 (a non-thinking model behaves exactly as
before) is likewise a live check; the automated half is the `echo_reasoning ==
false` loop over every declared profile.

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

**Automated counterparts (T-019):** the manual steps above are pinned by two
registered test files, so a regression is caught without a model or a network -
`crates/ragent-agent/tests/test_tool_alias_table.rs` (25 cases: the table's shape
and registry agreement, the six resolver guards, and the once-only unresolved
target report) and `crates/ragent-agent/tests/test_tool_alias_resolution.rs` (4
end-to-end cases through `SessionProcessor::process_message`). They use an
explicit entry onto the offline `think` tool rather than a built-in row, because
the built-in targets (`webfetch`, `websearch`) perform network I/O. Step 5 is
`test_alias_spelling_dispatches_the_registered_tool` (the record holds the
canonical name, not the spelling the model sent); step 7 is
`test_unknown_spelling_is_refused_when_aliases_are_off` (the record holds the
spelling and `Unknown tool: '<spelling>'`); "an unregistered target is refused
when used" is `test_alias_to_an_unregistered_target_still_refuses`; and step 8's
"no table lookup happened" is `test_registered_name_wins_over_a_table_entry`
plus `test_resolver_leaves_a_registered_name_untouched`, which require a
registered name to resolve by registry lookup even when a table entry maps it
elsewhere.

**Automated counterparts (T-020) - the config the preconditions write:**
`crates/ragent-config/tests/test_tools_config.rs` (24 value cases) and
`crates/ragent-config/tests/test_tools_config_load.rs` (5 cases through the real
layered loader in a sandboxed temp tree) pin the section this case's
preconditions assume. `test_global_and_project_layers_merge_the_alias_entries` is
the precondition's own claim (a project entry plus a user-global entry compose,
and neither boolean is lost); `test_a_project_explicit_false_switches_the_global_table_off`
is FR-015's "where neither is enabled, behaviour shall be unchanged" needing to
be reachable from the file that sets it; `test_save_to_source_writes_the_section_and_it_reloads`
and `test_a_never_configured_save_writes_no_tools_key` are the round-trip and the
silence-on-disk halves; `test_a_non_boolean_switch_is_refused_naming_the_file`
covers a typo'd switch, so a preconditions block with `"expose_alias_names":
"yes"` fails loudly rather than quietly running the case with advertising off.

**Automated half (T-021) - the outbound list:** step 2 ("the advertised tools
array contains both ragent names and the alias spellings") is
`crates/ragent-agent/tests/test_tool_alias_exposure.rs`, 12 cases over the one
projection every outbound surface reads. `test_advertising_off_advertises_only_registry_names`
and `test_an_empty_table_advertises_nothing_even_when_the_switch_is_on` are
FR-015's "where neither is enabled, behaviour shall be unchanged";
`test_advertising_on_adds_one_entry_per_resolvable_alias_spelling`,
`test_an_advertised_alias_copies_its_targets_definition` (the advertised entry
carries the target's description **and** parameter schema, so a call through the
spelling is shaped like a call through the canonical name) and
`test_every_builtin_entry_is_advertisable` are step 2's two halves - the
spellings and the canonical names, against the real registry;
`test_the_advertised_list_is_sorted_by_name` is FR-030's ordering. Step 2's
"no alias spelling equals a ragent tool name" is
`test_a_registered_spelling_is_never_advertised_twice` and
`test_no_name_is_advertised_twice_for_any_builtin_entry`, which are also the last
expected result ("With `expose_alias_names` true, no renamed tool is advertised
twice. `Bash` appears once"): the registered `Bash` survives exactly once and
carries the registry's own definition, not the entry's target.
`test_an_entry_whose_target_is_unregistered_is_not_advertised` and
`test_only_the_resolvable_entries_are_advertised` are FR-031's rule that a call
which would be refused at use is never offered,
`test_no_retired_spelling_is_ever_advertised` is FR-031's outbound half for every
row of the rename table, and `test_advertising_never_renames_a_canonical_entry`
is "advertising adds entries and changes none".
`crates/ragent-agent/tests/test_tool_alias_resolution.rs` adds the end-to-end
half: `test_the_wire_and_the_prompt_advertise_the_alias_spelling` reads the tools
array of the request the processor actually sends (both the spelling and the
canonical name are there, and the prompt's tool reference names the spelling),
and `test_the_wire_list_is_unchanged_when_advertising_is_off` is step 2's
counter-case. **Not covered by a test:** the prompt branch inside
`build_turn_system_prompt`, which is `pub(crate)` with no test seam - recorded in
`T-021-EXPOSE-ALIAS-NAMES.md` section 4 rather than papered over, so a reader
knows the prompt half is inspection plus a shared projection, and the wire half
is a test.

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

**Automated half (T-007), for the refusal half of this case.** FR-018's subject
is the drop, and this case's last two expected results are the two shapes of it:
a silent drop on a carryable history (T-006's cases) and a *loud* refusal on a
history the shape genuinely cannot carry, with no request issued. The Responses
input list has two such constructs, and both are now refusals rather than
omissions. `test_a_refused_construct_refuses_the_whole_conversation` covers the
first - an image part - and `test_a_role_the_responses_input_list_has_no_item_for_is_refused`
covers the second: `ChatMessage::role` is a `String`, the input list has an item
for `user`, `assistant`, `system` and `developer` only, and an unknown role used
to emit nothing at all, which shortened the history in exactly the way this
requirement forbids. Both assert the error *names the construct and the
provider* (`WireLoss { construct: "ImageUrl" | "role:tool", provider: "openai_responses" }`)
and that the other five profiles still accept the same history. "No request is
issued" is
`test_the_responses_client_refuses_to_build_a_body_it_would_shorten`, which
drives `ResponsesApiClient::build_request_body` - the function `chat` applies `?`
to - so the refusal leaves the body builder as an error rather than as a body
with fewer entries, while a carryable history still builds. All three are in
`crates/ragent-llm/tests/test_wire_encode_message.rs`; run them with

```
cargo nextest run -p ragent-llm -E 'test(test_wire_encode_message::)'
```

**Automated half (T-006).** Steps 1-6 are covered by
`crates/ragent-llm/tests/test_wire_encode_message.rs`:
`test_the_responses_client_body_uses_the_encoder_for_its_input_list` drives
`ResponsesApiClient::build_request_body` and asserts the input list carries both
the call and its output for the canonical history, which is step 6's check that
the Responses path no longer filters tool history out;
`test_encode_messages_equals_the_per_message_calls_for_every_profile` asserts the
conversation form emits the same entries as the per-message loop for every
declared profile, so no family can drop a turn while the others keep it. The
"fails loudly, no request is sent" expectation is
`test_a_refused_construct_refuses_the_whole_conversation`: the Responses input
list returns `WireLoss { construct: "ImageUrl", provider: "openai_responses" }`
for an image and the other five profiles accept the same history, so the refusal
is the family's and not a global strip. Confirming the *body* on a live
connection (the payload trace from TC-018) stays manual.

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

**Automated half (T-024).** FR-022 splits into two claims - the reclassification
and the recovery - and both are covered without a provider or a network in
`test_loop_steps_harness`, which drives the real `SessionProcessor::process_message`
against a scripted `LlmClient`.

- `test_a_tool_use_finish_without_a_call_is_a_truncation` pins the rule itself
  (stop 1: a tool-use finish with no call at all, a call that opened but never
  ended, a complete call list, every other finish reason, and no finish reason at
  all).
- `test_a_tool_use_finish_without_a_call_recovers_the_text_call` is steps 2 to 4:
  a `Finish { ToolUse }` whose reply narrates `<toolcall>{"name":"think",...}</toolcall>`
  produces **two** requests - the recovered call really executed - and the
  recovered markup is blanked out of the saved reply, so the model does not see
  its own narration beside the tool_use part.
- `test_a_tool_use_finish_without_a_call_and_no_markup_reports_truncation` is
  step 5: with no recoverable markup there is exactly **one** request (no spurious
  recovery prompt and no continuation prompt), the turn ends with the prose reply,
  and it is recorded as `Truncation` rather than as a clean stop, so the user is
  told the reply may be incomplete instead of being handed a lost call as the
  answer.
- `test_a_started_but_unfinished_call_is_not_reclassified` is FR-022's boundary:
  a call that opened and then lost its argument stream is a parsed call, so the
  step stays in the normal tool phase (two requests, no reclassification notice)
  and its cut-off arguments are refused by the existing dispatch pipeline
  (`ContentPart::ToolResult { tool_use_id: "call_cut", is_error: true }` in the
  next request) - louder and more actionable than a truncation notice.

The end-to-end consequence is pinned where the old behaviour was asserted:
`test_processor_more_paths::test_finish_tool_use_without_calls_records_truncation`
records `Truncation` for a bare `Finish { ToolUse }` with no calls and no text (it
previously pinned `ToolUse`, which is exactly the silent-end-of-turn FR-022
forbids).

**Not covered by a test:** "the model narrates a call as text" is a property of a
particular local model, not of the code - the harness scripts the exact stream such
a model produces. That a recovered call's permission dialog looks identical to a
native call's is TC-009's manual comparison, and the "file content `alpha` is
reported" clause is the same dispatch the other tool cases already cover.

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

**Automated half (T-011).** Every clause above except "seen in the TUI" and
"seen with `--log-level warn` in a terminal" is covered without a provider or a
network. `test_wire_decode::test_accumulator_rekeys_a_duplicate_call_id_and_keeps_the_first`
drives two index-keyed calls announcing the same id and asserts the starts
`call_dup` / `openai_0_read`, each call's own fragment, and one end per call - the
"both results, each attributed to its own call" bullets.
`..._rekeys_a_duplicate_id_used_by_two_sequential_calls` does the same with the
first call *closed* before the second arrives, which is what pins "the ledger
survives `end`". `..._rekeys_a_collision_between_a_minted_id_and_a_provider_id`
seats provider ids on two consecutive counter values and then re-keys a third
call, so a replacement that is itself a duplicate fails the assertion.
`..._does_not_rekey_a_repeated_frame_of_the_same_call` asserts the opposite
direction (a repeated frame is idempotence, not a collision).
`..._warns_one_line_per_collision_and_not_per_frame` is the only case that can see
step 5: it installs a capturing `tracing::Subscriber`, drives one collision,
repeats the same two frames three more times, adds a second collision of a
different id, and asserts exactly two WARN lines, each naming its own
`duplicate_id` and saying the call was re-keyed. The same collision is exercised
end to end through the real chat-completions SSE parser
(`test_openai_provider_extra::test_openai_chat_rekeys_two_calls_that_share_one_id`)
and through the Azure Resource Anthropic branch
(`test_azure_resource_anthropic::test_azure_anthropic_stream_uses_the_shared_accumulator_for_parallel_calls`).

**Not covered by a test:** that the two blocks and two results are *rendered* in
the TUI, and that `--log-level warn` shows the line once in a real terminal run -
both are presentation-level observations of the manual steps. A model that refuses
to duplicate ids ("if the model cannot be made to emit duplicate ids, record the
case as not reproducible") is likewise a judgement only the operator can make; the
automated half does not depend on it.

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

**Automated counterpart (T-025):** the FR-001 half of this case - that the
canonical record carries decoded JSON `input` rather than a narrowing typed
field, and that this is a recorded decision rather than an open TODO - is pinned
by `crates/ragent-types/tests/test_typed_args_decision.rs` (6 cases). It is
hermetic: it reads the crate's own `src/` tree from `CARGO_MANIFEST_DIR` and the
decision record from the workspace root, so it needs no provider, no network and
no build artefact. It asserts that `ToolCallState::input` / `::output`,
`ContentPart::ToolUse::input` and `ToolDefinition::parameters` are each declared
`Value`, that no `TODO: Replace \`Value\` with a typed` marker survives in
`src/`, that each of the four fields carries a doc comment naming
`specs/uniformtools/T-025-TYPED-ARGS-DECISION.md`, and that the record still
names its three sites plus FR-001 and NFR-002. Run it with
`cargo test -p ragent-types --test all test_typed_args_decision::`.

**Automated half (T-004):** steps 6 and 7 are the two a reviewer cannot check
from the TUI, and both are hermetic in `crates/ragent-llm`. For step 7's "the
tools JSON is byte-identical", `tests/test_wire_encode.rs` carries
`test_encode_tools_bytes_match_the_pre_fold_builders` - the literal pre-fold
bytes for all five builders plus the HuggingFace-prefixed row - so a key-order,
field or comma change fails on a string rather than in a request.
`test_cached_tools_hit_equals_a_fresh_encode_for_every_format` asserts FR-003's
literal clause per key on the miss and on the hit, and
`test_every_profile_a_provider_can_serve_is_reachable_through_the_cache` checks
both directions of the id-to-envelope mapping (a provider with no cache key, and
a key no provider can reach). For NFR-001's "not materially longer", the payload
is measured once per tool set and reused: `tests/inline/tool_cache_tests.rs`
pins the key's two components separately (`..._key_separates_formats_with_the_same_tools`,
`..._key_tracks_the_tool_content`), the invalidation path
(`invalidate_tool_cache_drops_every_entry`), and that `byte_len` - what
request-size estimation and the TUI read instead of re-serialising - is the
length of the slice the caller actually assigns
(`byte_len_counts_the_payload_the_caller_assigns`). Step 8's "checkable with no
provider" is the same property: `encode_tools` is a pure function over
`&[ToolDefinition]`. Run them with

```
cargo test -p ragent-llm --test all test_wire_encode::
cargo test -p ragent-llm --lib providers::tool_cache::
```

Both are green on the current tree. The `--lib` filter is necessary rather than
stylistic: the cache cases live inline in `tool_cache.rs`, so they compile into
the lib binary, not the `--test all` one.

**Automated half (T-027), for the whole-tree form of step 7.** T-004's scan was
scoped to the six modules its fold touched; T-027 makes the claim hold over the
tree, and it is in the same file, so no extra target is needed.
`test_no_provider_module_writes_an_envelope_key` walks every `.rs` file under
`crates/ragent-llm/src/providers/` (recursively, `wire/` included), strips
comments first so module docs that *name* a shape do not fire it, and reports
each offending `file:line: key` for the eight keys a definition object can be
built from; `wire/encode.rs` - `item_json`, the one definition site - is the
single exemption, named by path.
`test_no_provider_module_writes_a_tool_body_key_literal` refuses the body-key
literals `["tools"]` / `["toolConfig"]` outside the two modules that define
them, which is why step 7's request body reads
`body[ToolFormat::<Family>.body_key()]` rather than a string literal; and
`test_every_tool_body_key_is_the_one_the_encoder_wraps` checks that the key each
format reports is the one its payload fits under (Bedrock's whole
`{"tools":[...]}` object under `toolConfig`, the other five the inner array
under `tools`), so a provider cannot put the encoded list where nothing reads
it. Run them with

```
cargo test -p ragent-llm --test all test_wire_encode::
```

**Not covered by a test:** the tool-*call* `"type": "function"` literal inside an
assistant `tool_calls` array is out of this scan's scope (it belongs to T-006's
carrier guard), and the Responses item's flat keys `description` / `parameters` /
`name` / `type` are shared with unrelated message bodies, so they are pinned only
where T-005 scoped them - to `openai_responses.rs`. A provider that built a
Responses-shaped *definition* array inside a module T-005's case does not name
would be caught by none of the three.

**Automated half (T-005), for the Responses half of step 7.** "Its request body
must equal the encoder's payload exactly" is
`test_openai_responses::test_build_request_body_tools_come_from_the_shared_encoder`:
it builds a real `ChatRequest` through `ResponsesApiClient::build_request_body`
and compares `body["tools"]` with
`cached_tools(ToolFormat::OpenAiResponses, ..).responses_tools_array()`. An inline
array of the right shape passes every shape assertion and fails that comparison,
which is the point. The old guard is pinned beside it by
`test_build_request_body_omits_tools_when_the_list_is_empty` (an empty registry
leaves `tools` absent rather than sending `[]`). On the cache side,
`responses_and_chat_are_separate_cache_entries_over_the_same_payload_slice`
asserts the two OpenAI-shaped keys are different entries with different bytes
while each `byte_len` is its own inner array, and
`test_responses_provider_asks_the_encoder_for_its_tool_array` scans
`openai_responses.rs` two ways: no `"parameters":` / `"type": "function"` literal,
**and** a `cached_tools(ToolFormat::OpenAiResponses` call - so deleting the array
without wiring the encoder fails too. Step 6's on-screen estimate is covered by
`crates/ragent-tui/tests/test_context_panel_t005.rs::test_tool_metadata_measures_the_selected_providers_own_envelope`
(a Responses session measures 51 tokens of envelope overhead against 532 for a
chat session over the same 148 definitions). Run them with

```
cargo test -p ragent-llm --lib providers::openai_responses::
cargo test -p ragent-llm --lib providers::tool_cache::
cargo test -p ragent-llm --test all test_wire_encode::
cargo test -p ragent-tui --test all test_context_panel_t005::
```

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

**Automated counterparts (T-020) - the switch itself:** this case's
preconditions depend on `tools.wire_trace` existing, defaulting off and parsing
from either layer, which is what
`crates/ragent-config/tests/test_tools_config.rs` and
`crates/ragent-config/tests/test_tools_config_load.rs` pin
(`test_serialised_default_omits_the_tools_key`,
`test_expose_alias_names_and_wire_trace_parse_independently`,
`test_merge_ors_the_opt_in_booleans`).

**Automated half (T-023) - the dump:** `crates/ragent-llm/tests/test_wire_trace.rs`
pins every step below except the two manual confirmations that need a live
provider. Step 1/2 ("no trace directory is created") is
`test_wire_trace_is_inert_by_default`: with the switch off, `trace::next_path`
returns `None` - there is no destination at all - and `record_body` writes no
file. Steps 4/5 (one file per step, named with the session id and step number)
are `test_wire_trace_writes_one_file_per_step`
(`<session>-0001.json`, `-0002`, and no third file once disabled) and
`test_wire_trace_file_name_is_sanitised` (an escaping session id is flattened
into the root; an absent id is `no-session`). "The file contains the request
body for that step, including the tools array and the messages array" is
`test_wire_trace_records_the_tools_and_messages_arrays`. Step 7
(`grep -c 'sk-' <file>` prints `0`) is
`test_wire_trace_redacts_every_credential`, which seeds a `sk-` key under a
credential-named key, a `Bearer` token under `Authorization` and a `ghp_` token
under an innocuous key, and asserts the shared placeholder is present. The
ASCII expectations (no byte >= 0x80, ASCII file name) are
`test_wire_trace_output_is_ascii`. "With tracing set back to `false` or removed,
further runs write nothing" is the tail of
`test_wire_trace_writes_one_file_per_step`. The loop really driving the switch
from the resolved config is
`test_agent_loop_sets_the_switch_from_config`, and every provider that builds a
body calling the recorder is `test_every_provider_records_its_body` (a source
guard over every module implementing `LlmClient`, with `mock_llm_client` and
`router_client` exempt and the reason recorded).

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
6. Open the rename table. The pairs are in
   `crates/ragent-types/src/tool_rename.rs` (`TOOL_RENAMES`);
   `crates/ragent-agent/src/tool/rename.rs` is a re-export shim and holds no
   pairs. (The body sits one crate lower because FR-031 makes the config loader
   read it, and `ragent-config` cannot depend on `ragent-agent`.)
7. Compare the two lists row by row. Then check the order: the table is
   ascending by `after`.
8. Confirm the test that ships with the table has been reported as passing by
   the reviewer of the change, and read what it asserts. **On the finished
   (post-rename) build** it asserts the tree the rename produced: every `before`
   is *retired* (nothing registers it), every `after` *is* registered and is a
   valid identifier, and both columns are unique. Before the rename landed the
   first two read the other way round; that polarity flipped with the tools, in
   the same change (FR-034), and neither half may be relaxed. The suite is
   `crates/ragent-agent/tests/test_tool_rename_table.rs` (9 cases, run as
   `cargo test -p ragent-agent --test all test_tool_rename_table::`); its last
   case asserts the table's test opens no workbook, which is why step 2 onward
   is a manual comparison rather than a script. The `after` names are what
   `--dry-run` must list; see TC-020 for that half.

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
5. Send `/bug-report` in the TUI. Read the `## Tool Configuration` section. On
   the tree T-041 measured there is **no** `Tools registered` line: the section
   reports `- **Tools visible**: <n>` where `n` is the number of *enabled
   tool-family visibility switches* (1 on the shipped default configuration),
   which is not a tool count. Record what the line says rather than reading a
   tool total into it; if the change adds a tool-count line, read that instead.
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

- Steps 1, 2, 6 and 7 all report the same total, and it equals the baseline
  recorded in TC-027 before the rename: 148. `T-041-BASELINE.md` holds that
  number, the 21 family counts and the name list, measured before the rename.
  Step 5 is **not** one of them: on the measured tree `/bug-report` has no tool
  count (see the step), so it cannot corroborate 148.
- The per-family counts in step 1 are unchanged except for the tool names within
  each family; no family gains or loses a tool. Check `agent` 4 and `team` 19 -
  `team_wait`'s permission category is `agent:spawn`, so it counts in `agent`.
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

**Automated half (T-040).** Steps 4 and the last bullet above are the manual
half of NFR-006 and are kept for the reason a log read is worth doing once. The
mechanical half is
`cargo test -p ragent-agent --test all test_nfr006_rename_cost::` (6 cases): a
function-granular source scan that fails on any FR-023 table read inside
`session/`, `tool/` or `backend/` outside the two once-per-turn sites (and, in
reverse, on an exemption that no longer matches), explicit `&str` /
`Option<&'static str>` bindings so an allocating name or table answer is a
compile error, and the three runtime-key cases for resolution, the definition
cache and the repeat guard. A green run here means step 4 prints nothing for a
reason; the manual step is what confirms the reason holds end-to-end through a
provider.

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

**Documentation half (T-039).** Steps 1-5 were run against the tree after the
documentation migration; the census, the per-document change list and the seven
classification reasons are in `specs/uniformtools/T-039-DOCS.md`. What a
reviewer re-runs, and what it printed:

| What to check | How, and what it must print |
|---|---|
| the migration is measured, not asserted | one whole-token regex over the NFR-007 scope (4 root documents + 15 crate `DOCS.md` + 129 howto pages): **643 lines in 59 files** name a retired spelling at HEAD, **596 in 55 files** in the tree; the migration is **74 removed lines across 25 documents** and **100 added lines carrying the adopted spelling** |
| step 1's hits are all non-tool uses | `20` remaining backticked `bash` hits, every one a config section (the `bash` block, `bash.allowlist`), the FR-028 permission vocabulary (`file:write`, `bash:execute`, the `ask_user` category), the shell program (`bash -n`, `base64 ... | bash`, a fenced shell block), or the rename prose itself |
| step 2's hits are the tool | `34` backticked `Bash` hits, including every place a page tells a caller what to invoke |
| step 3 finds one table, not two | `sed -n '/^### Renamed/,/^## \[1.0.149\]/p' CHANGELOG.md` prints a `### Renamed` section with 13 rows and the two "this is not a migration" notes; the T-031 bullet now points at it |
| the section is a sibling of `### Added` | the `### Renamed` heading sits after `### Changed`, not inside `### Added` - its first draft did, and the table was then a sub-heading of the wrong section |
| the docs agree with the registry | `ragent run --dry-run` still prints `total: 148 visible tools` over 21 families, and the rosters in `README.md` / `SPEC.md` / `docs/howtos/tools/INDEX.md` name those 148 |
| step 5 holds for the edited lines | no line this task added introduces a non-ASCII character the line it replaced did not already carry |
| the guard still agrees | `scripts/check-tool-rename.sh` exits 0 with all `39` exemptions matching - the doc migration added no source literal and broke no entry |
| the generated PDFs are knowingly stale | `pdftotext docs/howtos/tools/pdf/shell.md.pdf -` still shows `bash` in the tool table. They are release-step artifacts and are recorded as due, not silently ignored |

**Documentation half (T-028).** TC-025 covers the *spellings*; the unified tool
*path* is documented by T-028 and re-read the same way - open the document, read
the named section, and check every symbol and number it claims against the tree.

| What to check | How, and what it must print |
|---|---|
| `SPEC.md` states the contract, not just the symbols | `grep -n "Unified Tool-Calling Wire Path" -A 40 SPEC.md` prints the nine-field table with all nine `ToolWireProfile` fields, the six families, and the no-catch-all sentence naming `test_wire_profile_table.rs` |
| `SPEC.md` states the path, not just the pipeline | `grep -n "Tool-Call Wire Path and the One Dispatch Pipeline" -A 45 SPEC.md` prints the four rules: declared contract, one ordered pipeline for both modes, an unknown name never executes, a result is never indistinguishable from a success |
| the crate docs state the chain end to end | `grep -n "Unified tool path" crates/ragent-llm/DOCS.md` prints one bullet naming `encode_tools`, `encode_message`, `ToolCallAccumulator` and `decode_tool_arguments`, and the `router` bullet records that its row is nominal |
| every symbol the prose names exists | each name in the T-028 table of checked symbols resolves in the named file; a name that does not is a documentation defect, not a typo |
| `CHANGELOG.md` announces the shape change once | `sed -n '/^### Added/,/^### Renamed/p' CHANGELOG.md` opens with the T-028 entry, which names the requirement ids, the two `SPEC.md` sections, the crate `DOCS.md` addition, and the words "documentation only" |
| the claim has a guard | `cargo nextest run -p ragent-llm -E 'test(test_folded_provider_modules_no_longer_write_an_envelope_literal)'` passes, and T-027's full-tree scan is what will hold the same claim over every `providers/` file |
| NFR-005 holds for the added prose | no line T-028 added introduces a non-ASCII character; `grep -P '[^\x00-\x7F]'` over the three documents' added lines prints nothing |

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

### TC-026 automated half (T-034)

Step 1's guard has an automated half:
`crates/ragent-agent/tests/test_tool_rename_guard.rs` (4 cases, registered by
T-034 - before that it was tracked but never in `tests/all.rs`, so it reported
`running 0 tests` and proved nothing). The cases invoke the same implementation
the CI job runs (`scripts/check_tool_rename.py`, NFR-009), so the script and the
test cannot drift:

| TC-026 expectation | Automated case |
|---|---|
| step 1's "the guard exits zero" on this tree | case 1 - exit 0, output ASCII, and the success message names the `(e)` predicate letter |
| step 1's "any hit it prints ... carries an explicit commented exemption" | case 3 - `--exemptions` succeeds, names **both** structural exemptions (the FR-023 table and the `CHANGELOG.md` entry), and prints exactly one non-blank `reason:` line per per-line entry. An entry with no reason is unreviewable; an entry for a tool-name site is the defect |
| the guard actually catches a stale literal (the premise of the whole case) | case 2 - a seeded `tool == "bash"` in a scanned crate makes it exit non-zero with ASCII output naming `(e)`, the `before` spelling and the offending file |
| step 2's boundary - a test file is outside the scan | case 4 - a retired literal seeded into `crates/*/tests/` passes and is not reported. This is the half that keeps the guard from forcing FR-023's and FR-030's literal assertions to be weakened |

What stays manual: step 2's by-hand `grep` comparison, steps 3 and 4 (the tool
descriptions and the assembled prompt - prose, not literals), and steps 5 to 8
(the skill surface). The one deliberate difference from the task text is
`crates/ragent-agent/src/agent/oasf.rs`: its permission list is doc-comment prose
on lines 160-161 and the scan does not read comments, so the file needs **zero**
entries. Recording two would have added dead entries, which case 3's reverse check
fails.

### TC-026 test-tree half (T-038)

Step 2's boundary sentence - "or lives in a test file outside the guard's scope"
- is not a loophole; it is the half of the migration the guard deliberately
cannot see, and T-038 is where it was done. The census, the classification and
the red proofs are in `specs/uniformtools/T-038-TEST-MIGRATION.md`; what a
reviewer can re-run is:

| What to check | How, and what it must print |
|---|---|
| the pre-rename figure the task quotes actually reproduces | the T-029 scan on the `1.0.149` tag: `775` occurrences `/ 160` files, and the code-only half `633` `/ 142` |
| no test **fixture** feeds a retired spelling to a path that dispatches on the name | `201` code literals remain, and every one is one of the twelve classified classes: FR-028 permission vocabulary, a shell program, the `bash.allowlist` section, a language id, a GovCreate stage id, an agent capability tag, a memory tag, a scratch-dir tag, English prose, or an assertion that a spelling **is** retired |
| no assertion was relaxed | the assertion count rose (`35 980` -> `37 183`) and no test file lost one; spot-check the twelve files whose fixtures moved to match their assertion, listed in the T-038 record |
| the assertions are load-bearing | six red proofs, each restored byte-identically and `md5sum`-checked - the sharpest is reverting `crates/ragent-tools-core/src/write.rs`'s `Tool::name()` to `"write"`, which fails **both** FR-023 registry cases. A migration whose assertions cannot fail proves nothing |
| the guard's scope and this migration agree | `scripts/check-tool-rename.sh` still exits 0 with the 39 reviewed exemptions, i.e. nothing in `crates/*/src` came back; and T-034's case 4 (a literal seeded into `crates/*/tests/` passes) still holds |

Two pre-existing failure sets are **not** T-038 regressions and are recorded so a
reviewer does not attribute them to a 136-file diff: `ragent-llm --test all` keeps
its six `test_wire_*` guard failures (T-004/T-005/T-006/T-017/T-019 are still
pending) and `ragent-tui --test all` keeps its three `Alt+T` ->
`ToggleTasksPanel` failures. Both counts and both identities match the baseline
taken before this task started.

---

### TC-027 - The surface baseline is recorded before the rename and met after it (FR-030)

**Preconditions:** Prerequisites 1, 2 and 5. This case needs both binaries and is
best run as a two-part check: part one before the rename lands, part two after.

Part one has already been recorded: `specs/uniformtools/T-041-BASELINE.md` holds
the pre-rename `--dry-run` total and family table, the 148-name list with its
digests, and what `/bug-report` actually reports, all measured on the pre-rename
tree. Steps 1 to 3 below are therefore a spot-check of that record on your own
build, not a new measurement, and it is the record that steps 5 and 6 compare
against.

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
  `T-041-BASELINE.md`; check `agent` 4 and `team` 19. `team_wait` belongs to the
  `agent` family (its permission category is `agent:spawn`), so a name-prefix
  reading of the list gives 3 and 20 and is wrong. The family is the leading
  segment of `permission_category()`, never of the tool name - 50 of the 148
  names disagree with their family at the first `_` segment.
- Step 2's `/tools list` renders one or two blocks. The header is
  `Visible Tools (<visible> total, <disabled> disabled):`, the visible block has
  one row per visible definition, and the `Disabled by visibility (<n>):` block
  has one row per hidden one. Assert `<visible> + <disabled> = 148` and the total
  row count of 148, not 148 rows in the visible block: under the shipped default
  configuration the visible block holds 92 and the disabled block 56.
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
- `/bug-report` is not part of this comparison on the pre-rename tree. FR-030
  names a `Tools registered` line; there is none, and the line that resembles it
  (`- **Tools visible**: <n>`) counts enabled visibility switches, not tools.
  Either the change adds the line and step 5 of TC-024 reads it, or the absence
  is recorded. Do not read a tool total into the switch count.

### TC-027 automated half (T-037)

`crates/ragent-agent/tests/test_fr030_surface_agreement.rs` (7 cases) and
`crates/ragent-tui/tests/test_fr030_surface_agreement.rs` (2 cases) automate
steps 5 and 6 against the recorded baseline, so no one has to diff two hand-saved
files:

| TC-027 step | Automated case |
|---|---|
| steps 1 and 5 (the `--dry-run` total and family table) | agent cases 3 and 4 - the `tools` section of a real `run_dry_run`, human and JSON, compared row for row against a family table derived from the same registry, with the 21-family count, the `agent` 4 / `team` 19 canaries and the 148 sum |
| step 2 (the `/tools list` names) | TUI case 1 - the `Visible Tools (n total, m disabled):` header with `n + m = 148`, every row of both blocks, and the sorted set against `registry.list()` |
| step 6 (13 removals, 13 additions, no other line) | agent case 2 - 148 distinct names, 13 carrying an uppercase letter, those being exactly `TOOL_RENAMES`' `after` column, and the whole list hashing to `308d92d9...` |
| the other enumerations FR-030 names | agent cases 1, 5, 6 and 7 - the three registry enumerations agree element for element; `tool_info`; `commands_info` with its `/tools` row pinned to `TOOLS_SUBS`; and one real turn's outbound `ChatRequest.tools` plus its `ToolsSent` event |
| the `/bug-report` `Tools registered` line | TUI case 2 - the line and the `## Tool Configuration` section it heads |

Three of these were wrong on the live tree when the guard was first registered,
and the corrections are the point rather than an aside: the canary pair was the
name-prefix reading, the `/tools list` header was a literal no renderer writes,
and the `Tools registered` line did not exist at all (T-037 adds it, which is the
"the change adds the line" branch of the expected-results note above). The other
half of the expected results - the two totals and the family counts being
*identical* before and after - is a comparison between two builds and stays
manual; the automated cases pin the after figures against the numbers
`T-041-BASELINE.md` recorded.

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

### TC-028 automated half (T-032)

The behavioural half of FR-026 - no second registration, no forwarding wrapper,
and the refusal reaching every inbound path - is
`crates/ragent-agent/tests/test_fr026_no_alias_survives.rs` (9 cases,
`specs/uniformtools/T-032-NO-ALIAS.md`). It re-reads the same claims this case
makes by hand, at the places a hand check cannot reach:

| TC-028 expectation | Automated case |
|---|---|
| step 6's "no entry maps `Bash`, `Edit`, `Write`, ... because FR-026 forbids a second spelling" | case 4 - the alias table drops a retired source, a retired target, and a retired source pointed at an unrelated tool, for all 13 pairs; case 9 repeats it for `tools.aliases: true` |
| step 7's "`Bash` appears once, never twice" | case 2 - `definitions()` holds no retired spelling and each adopted one exactly once; case 1 - no duplicate key and no retired key |
| "a second registration or wrapper could hide" | case 3 - no pair resolves both spellings and no key disagrees with the name its tool reports; case 8 seeds both defects and proves the comparison sees them |
| "the refusal happens on every inbound path" | case 6 - one real `process_message` turn records the refused call as `Error`; case 7 - all four surfaces call `process_message` and none resolves a tool name itself |
| the case fold is permitted, not a second spelling | cases 1 and 5 assert `bash` -> `Bash` folds and `skill_manage` does not, which is FR-026's own note |

What stays manual: steps 2, 4 and 5 (a live provider call and the TUI block it
renders), and step 5's aliases-off refusal, which is the config loader's
behaviour rather than the resolver's.

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
- **Automated half (T-044) - all three files were dead until T-044 declared them.**
  Steps 8-10 and the clause-2 bullet are proved by 16 cases in three files, and
  until T-044 landed **none of them had ever compiled**: `ragent-types/src/lib.rs`
  had no `pub mod tool_bridge;` and none of the three `tests/all.rs` files carried a
  `#[path]` line for its `test_fr032_*` file. The registrations are part of the
  deliverable, so a reader re-running this step should confirm the declarations
  first (`grep 'mod tool_bridge' crates/ragent-types/src/lib.rs`, and `fr032` in the
  three `tests/all.rs` files) and only then the behaviour: 9 cases in
  `crates/ragent-types/tests/test_fr032_dynamic_bridge.rs` (namespacing for both
  manglings, the exhaustive "no bridged form of any of the 13 pairs equals a bare
  spelling", the prefix-agnostic contract, the refusal sentence, and the
  once-per-source-and-name ledger), 5 in
  `crates/ragent-agent/tests/test_fr032_bridge_containment.rs` (all `#[serial]`,
  because the ledger is process-wide and the consolidated `--test all` binary
  shares one process; `bridge_server_tools_named` supplies the mangler a real
  registration never supplies, so containment can be observed at all), and 2 in
  `crates/ragent-plugins/tests/test_fr032_plugin_bridge.rs` (through the real
  `PluginManager::register_plugin_tools` path). Red proofs are recorded in
  `specs/uniformtools/T-044-DYNAMIC-BRIDGE-REFUSAL.md`: disabling the prefix
  clause, pinning the retired-spelling lookup, dropping the ledger's `continue`,
  dropping the bridge's `continue`, and un-namespacing `plugin_tool_name` each fail
  a named case.
- Step 11: the `--tool-alias` flag is rejected by the argument parser as an
  unexpected argument, and the command exits non-zero without running. The
  `RAGENT_TOOL_ALIAS` variable is not read anywhere and has no effect: the run
  completes and `bash` stays unresolvable. Record both outcomes verbatim in the
  change description - the flag failing loudly and the variable being ignored
  are different results, and a reader rebuilding the audit needs to know which
  is which.
- No step panics, and the only files written are the config edits you made.
- **Automated half (T-042) - steps 1-7 and 12, through the real loader.**
  Steps 2, 5 and 7 are proved by
  `crates/ragent-config/tests/test_retired_alias_spelling.rs`, which writes each
  block into a temp file, points `RAGENT_CONFIG` at it and calls `Config::load()`
  - the uncached path, so no cwd change and no project file is involved - then
  asserts the refusal names the file, the spelling and its `after` replacement:
  `test_a_retired_source_key_is_refused_at_load_naming_file_spelling_and_replacement`
  (step 1-2), `..._a_retired_alias_target_is_refused_...` (step 4-5),
  `test_the_self_mapping_form_of_an_adopted_spelling_is_refused` and
  `..._of_a_retired_spelling_...` (step 6-7), and
  `test_every_offending_entry_is_reported_in_one_error` (all three at once, one
  message - the manual steps only ever try one entry). Step 3 is
  `test_a_provider_spelling_onto_a_registered_tool_loads_cleanly`; the
  "not a rename problem" half is
  `test_an_unregistered_alias_target_is_not_a_load_error` and
  `test_the_self_mapping_of_an_untouched_spelling_is_accepted`; the inline route
  is `test_the_inline_environment_route_is_refused_and_names_itself`; and
  `test_every_pair_in_the_rename_table_is_refused_as_key_and_as_target` walks
  the whole FR-023 table so the check cannot pass by pinning `bash` alone.
  `ragent run --dry-run` parses the same files through its own path, so
  `crates/ragent-agent/tests/test_dry_run_extra.rs` carries the readiness
  report's agreement with the loader:
  `test_project_config_with_a_retired_alias_spelling_is_blocked` (the config
  section goes BLOCKED, exit 1, the message names the spelling and `Bash`) and
  `test_project_config_with_an_unregistered_alias_target_still_loads`. The
  resolver half - that no retired entry is ever consulted, and that an
  unresolved target is reported once and refused when used - is
  `crates/ragent-agent/tests/test_tool_alias_table.rs`.

**Automated counterparts (T-043) - the surfaces no manual step can enumerate:**
steps 1-7 and 11 are hand-checkable one entry at a time; the question FR-031
really asks is whether *any* mechanism in the tree can restore a spelling, and
that is what `crates/ragent-agent/tests/test_escape_hatch_audit.rs` answers (8
cases: one per row of `T-043-ESCAPE-HATCH-AUDIT.md`'s seven-row table, plus one
pinning the single reviewed exemption). Step 11's two
outcomes are recorded verbatim in the audit document: `--tool-alias` is refused
by the argument parser with exit **2**, while `RAGENT_TOOL_ALIAS` is read
nowhere (`--dry-run --json` output is byte-identical with and without it set,
SHA-256 compared with `--model` pinned). Each scanning case asserts a floor on
how many names it inspected, so deleting the thing being searched cannot make a
case pass vacuously.

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

### TC-030 automated half (T-045)

`crates/ragent-agent/tests/test_fr033_completion_sources.rs` (6 cases) and
`crates/ragent-tui/tests/test_fr033_completion_surfaces.rs` (4 cases) automate the
steps above that can be asserted without a human reading a menu:

| TC-030 step | Automated case |
|---|---|
| step 1 (`/ba` completion) | case 9 - every menu trigger and its suggestions, over the whole slash surface, against `TOOL_RENAMES` |
| step 2 (no field completion) | case 10 - `ba` opens nothing and `@zeta` rows are project paths, never registered tool names |
| step 4 (refusal + hint) | cases 1 and 2 - `suggest_tool_name` driven with a **foreign** registry, and no answer or candidate is a `before` spelling |
| the tool lists the steps read | cases 3-6 - `tool_info`, the `--dry-run` tools section, `commands_info` and the prompt tool reference, each handed a registry it has never seen |
| the `/tools` menu | cases 7 and 8 - the menu and its hint are exactly `TOOLS_SUBS`, and every offered form is handled while `/tools frobnicate` is refused |

Two notes for a reader of that output. First, case 10 is a *negative*: it fails if the
`@` picker ever offers a registered tool name, which is the "adds no new completion
surface" clause stated as a test rather than as prose. Second, case 9's exemption is
load-bearing - `bash` is a retired tool spelling and also the live `/bash` command, so
the case computes the set of such spellings from `TOOL_RENAMES` and `SLASH_COMMANDS`
and asserts it is exactly `{"bash"}`; a second retired spelling promoted to a command
fails there.

**Automated counterpart (T-046):** step 9's tree half - "exactly one spelling of
each FR-023 pair resolves, and it is the `after` one" - is pinned by
`crates/ragent-agent/tests/test_tool_rename_atomic_landing.rs` (7 cases, all
green). It reads `TOOL_RENAMES` and asserts the **live registry**:

| Case | What it proves |
|---|---|
| 1 | every pair resolves one spelling across `list()`, `all_tools()` and `definitions()`, and it is `after` |
| 2 | `get(before)` is `None` for all thirteen, and no registry key is a `before` |
| 3 | the three enumerations are element-for-element identical |
| 4 | the tool stored under an `after` key reports `after` from `Tool::name()`, and no stored tool reports a `before` |
| 5 | 21 permission families, `agent` 4 / `team` 19 |
| 6 | 148 names and 148 definitions |
| 7 | no enumeration repeats a name |

Case 4 is the half-renamed detector and is the reason the file exists: a tree that
moves the key but not `Tool::name()` (or the reverse) compiles and passes a
key-only check. All three states - both spellings, neither, and a third column -
were proven red by temporarily breaking `Tool::name()` and the alias registration;
`specs/uniformtools/T-046-ATOMIC-LANDING.md` section 7 records the exact messages.

The **commit graph** half of step 9 stays manual and stays this case's
responsibility: a test that shelled out to `git log` would fail in a source
tarball and would duplicate the review. What the automated case does add is the
measurement the landing compares against - 148 names, 13 uppercase, digest
`308d92d9...`, a 26-name symmetric difference that is exactly the table - so the
reviewer's check at the landing commit is a diff against a recorded number
rather than a reading of the history.

### TC-030 automated half, part 2 (T-048, NFR-009)

Steps 5, 7, 8 and the "one implementation" expected result are automated by
`crates/ragent-agent/tests/test_tool_rename_guard.rs` (16 cases), which runs the
one guard implementation `scripts/check_tool_rename.py` as a subprocess - the
same program `scripts/check-tool-rename.sh` and the CI job run.

| TC-030 step | Automated case |
|---|---|
| step 5 (a clean run, exit 0, a few seconds) | `test_the_guard_finishes_in_a_few_seconds` - a full-tree run inside a 10 s budget (measured 2.2 s warm), and `test_the_script_form_and_the_test_form_report_the_same_thing` for the exit code |
| step 6 (one violation per predicate) | the eight `test_nfr008_predicate_*` cases, each seeding its fixture into a **private copy** of the tree under `target/temp/` and asserting the predicate letter, the file, the line and the `fix:` line |
| step 7 (three at once) | `test_one_run_reports_every_predicate_at_once` - four independent violations, one run, every letter reported |
| step 8 (`tools.xlsx` absent, network down) | `test_the_guard_reads_only_the_repository_and_never_a_workbook` - the module imports nothing that can open a socket or a workbook, `--root` confines every reported path, and a copy with no `tools.xlsx` judges identically |
| the "one implementation" result | `test_the_script_form_and_the_test_form_report_the_same_thing` - `--check` (what the script prints) and `--json` (what the case reads) agree on the verdict, on every hit, on the fix text and on the success wording |
| a deliberate violation of a predicate the seeded cases cannot reach | `test_a_failure_names_the_predicate_letter_and_the_exact_edit` and `test_a_root_without_the_rename_table_fails_closed` |

The manual half of step 8 stays manual for the part a test cannot observe - a run
with the interface actually down - but the workbook half is now a case: the
scratch copy the case scans carries no `tools.xlsx`, so a guard that needed the
workbook would fail there. Step 6's eight edits are also still worth doing by
hand once, because the fixture a case seeds is one line where the manual edit is
real code; what the cases add is that the *detection* is asserted on every run
rather than once.

**Not covered by a test:** step 8's `unshare -n` half, and the human reading of a
report produced from a real (not seeded) edit. Both need a person at a terminal.

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

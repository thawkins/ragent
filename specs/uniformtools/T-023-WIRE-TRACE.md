# T-023 - `tools.wire_trace`: the redacted payload dump

Spec `uniformtools`, task T-023. Requirement FR-017, validated by TESTPLAN case
TC-018. Config key defined by T-020 (`crates/ragent-config/src/tools.rs`).

## 1. What FR-017 asks for

> Where `tools.wire_trace` is enabled in config, the system **shall** write the
> exact per-step request body for the active provider to
> `target/temp/wire-trace/<session>-<step>.json` with every credential redacted
> through the existing sanitizer, and **shall** be inert when the setting is
> absent or `false`.

Four obligations, one of which is easy to miss:

1. **The exact per-step body** - the object the provider sends, including the
   `tools` array and the `messages` array, not a summary of it. A trace that
   described the request would not be diffable against a provider's documented
   shape.
2. **`<session>-<step>` naming** - one file per step of a turn, so a retried
   send (a second request on the wire) is visible as its own file.
3. **Every credential redacted, through the existing sanitizer** - the shared
   `ragent_types::sanitize` primitives, not a new redactor that could drift.
4. **Inert when absent or `false`** - which TC-018 step 2 states strongly: *no
   trace directory is created, and no files are written into the scratch project
   by the run*. "Inert" therefore means "returns before touching the
   filesystem", not "creates the directory and writes nothing".

TC-018 also adds reader-facing obligations: the file names are ASCII (NFR-005),
the file holds no non-ASCII byte, and `grep -c 'sk-' <file>` prints `0`.

## 2. State found

`crates/ragent-llm/src/providers/wire/trace.rs` already existed (282 lines,
dated before this task) but nothing used it:

- `grep -rn 'record_body' crates/ragent-llm/src/` matched **only the
  definition** - no provider called it, so the trace never produced a file.
- `grep -rn 'wire_trace' crates/ragent-agent/src/` matched **nothing** - the
  agent loop never set the switch, so the facility was permanently inert even
  with `"tools": { "wire_trace": true }` in the config.
- `crates/ragent-llm/tests/test_wire_trace.rs` existed but its
  `test_agent_loop_sets_the_switch_from_config` and
  `test_every_provider_records_its_body` cases were red (the failures the T-003,
  T-004, T-008, T-012, T-022 records all list as "T-023's"), and its
  redaction case had been reduced to placeholder-vs-placeholder comparisons
  (`body["api_key"] = "[REDACTED]"` compared against `"[REDACTED]"`), which
  passes no matter what the redactor does.

So the task was: finish the wiring, then make the tests actually test.

## 3. What was built

### `crates/ragent-llm/src/providers/wire/trace.rs` (295 lines)

- `ENABLED: AtomicBool` (default `false`), `set_enabled`, `is_enabled` - the
  process-wide switch. The body only exists inside each provider's
  `build_request_body`, which is a *method on the concrete client*, so the
  recording has to happen there. Threading a per-request flag to fifteen clients
  would mean either a new `ChatRequest` field (touching ~100 construction sites
  across the workspace) or a magic key in `ChatRequest::options` that a provider
  could accidentally serialise onto the wire. A process-wide switch costs one
  relaxed load per send and cannot leak into a request body. The trade-off is
  stated in the module docs rather than hidden: two concurrent sessions with
  different settings share the last value the loop wrote, and the only
  observable effect is whether an extra file appears under `target/temp/`.
- `next_path(&ChatRequest) -> Option<PathBuf>` (new) - `None` while disabled, so
  "inert" is observable without a filesystem and without creating a directory
  to look inside.
- `path_for` (private) - the only place a step number is consumed, so asking
  `next_path` a question can never skip a step.
- `record_body(&ChatRequest, &Value)` - the one write site. Returns immediately
  while `is_enabled()` is `false`, so a disabled trace creates no directory and
  writes no file. When enabled it names the file `<session>-<step:04>.json`,
  redacts, escapes non-ASCII, and writes atomically (temporary file in the same
  directory, then rename) so a crash cannot leave a truncated trace that reads
  like a truncated request.
- `set_root` (`#[doc(hidden)]`) / `root()` - the test hook that points the tree
  at `target/temp/`, and the default `target/temp/wire-trace` production ships.
- `session_component` - flattens a session id to ASCII alphanumerics, `-` and
  `_`, and renders an absent id as `no-session`. A session id arrives from
  storage and from callers, so a path separator must not create a nested
  directory or escape the trace root, and an empty id must not produce a file
  named `-0001.json`.
- `render` - serialise pretty, then `redact`, then `escape_non_ascii`.
- `redact` - two passes, in this order:
  1. `redact_named_keys` - any credential-named field, via
     `is_secret_json_key`. **String values only**: a tool's JSON *schema*
     legitimately declares a property called `token`, and masking that object
     would destroy the very document the trace exists to show.
  2. `redact_matched_values` (new) - runs
     `ragent_types::sanitize::redact_secrets` over every string and replaces
     only the matched span.
- `escape_non_ascii` - rewrites every non-ASCII `char` as one or more `\uXXXX`
  escapes. A non-ASCII character can only appear inside a JSON string literal,
  where the escape is equivalent, so the file stays valid JSON while every byte
  is ASCII (NFR-005).
- `next_step` - a per-session counter behind a poison-tolerant `Mutex`.

### Why pass 2 is `redact_secrets`, not `redact_json_value`

`ragent_types::sanitize::redact_json_value` walks a value and replaces a whole
string *value* that matches. That is right for a log field and wrong for a
request body: a provider's `system` prompt or a tool description legitimately
contains a line like `curl -H 'Authorization: Bearer ...'` or `api_key=...`
while explaining one, and blanking the entire string would destroy the
document. Calling the shared *matcher* on the string instead keeps the shared
pattern set and the shared placeholder while replacing only the credential
span, so the trace redacts exactly what the sanitizer redacts and nothing else.

The first draft used `redact_json_value`, and the rewritten test caught it
immediately: a 12-character `Bearer abc123` and `ghp_abcdefghijklmnop` were not
matched at all. The shared patterns have length floors (`sk-` needs 20+
characters, `Bearer` 20+), so the test now seeds real-shaped credentials - a
short placeholder would have passed whether or not redaction ran.

### Provider call sites

`record_body` is called once, with the built body, in thirteen sends over twelve
files: `anthropic`, `openai`, `openai_responses`, `azure_foundry`,
`azure_resource` (the Anthropic-flavoured Azure client), `gemini`,
`huggingface`, `ollama`, `ollama_cloud`, `openrouter`, `copilot`, and both
`bedrock` clients (Messages and Converse). Each call sits immediately after
`build_request_body`, before serialisation, so the recorded object is the object
the provider serialises. Two modules are deliberately not covered, and the
reason is recorded at the guard rather than left to inference:

- `mock_llm_client` has no wire at all - it returns a scripted event list.
- `router_client` delegates to a downstream client, whose own `chat` records.
  Recording the router's wrapper as well would write two files for one send.

### `crates/ragent-agent/src/session/loop_steps.rs`

`prepare_client` sets the switch once per turn from the turn's resolved config
(`trace::set_enabled(session_config.tools.wire_trace)`), after
`apply_session_backend_override` so an automation's per-session config is what is
read. Setting it per turn means a `wire_trace` change takes effect on the next
turn without a restart, and a default config leaves every provider writing
nothing.

## 4. Tests

`crates/ragent-llm/tests/test_wire_trace.rs` - 8 cases, all green. The switch
and the root are process-wide, so every case takes `crate::global_env_lock()`
(the consolidated binary's one lock), points the root at
`target/temp/wire-trace-tests/`, and gives its session a unique id so a run
leaves the repository byte-identical.

| Case | What it pins |
|---|---|
| `test_wire_trace_is_inert_by_default` | default off, `next_path` is `None`, and a disabled `record_body` writes **nothing** |
| `test_wire_trace_writes_one_file_per_step` | `<session>-0001.json`, `-0002`, and no third file once disabled |
| `test_wire_trace_records_the_tools_and_messages_arrays` | the body, verbatim, as valid JSON |
| `test_wire_trace_redacts_every_credential` | real-shaped `sk-`, `Bearer` and `ghp_` credentials gone; the shared placeholder present |
| `test_wire_trace_output_is_ascii` | no byte `>= 0x80`; the escape is present; the file name is ASCII |
| `test_wire_trace_file_name_is_sanitised` | a `../` id is flattened into the root; an absent id becomes `no-session` |
| `test_every_provider_records_its_body` | every module implementing `LlmClient` calls `trace::record_body`, minus the two named exemptions, and at least ten clients were inspected |
| `test_agent_loop_sets_the_switch_from_config` | the loop drives the switch from `tools.wire_trace` |

The last two are source guards on purpose. A provider that quietly skipped the
call, or a switch that was never set from config, would leave the trace
incomplete or permanently inert with **no error anywhere** - there is nothing for
a runtime test to observe.

`test_wire_trace_file_name_is_sanitised` is written against the names this call
*added* and clears them first, because the step counter is per-session and
process-wide, and a session id that escapes the root writes outside the
directory the helper scans.

## 5. Red proofs

Each was run and reverted, restored byte-identically (`cp -p` + `md5sum -c`
against `target/temp/t023/before.md5`) and `touch`ed so cargo could not reuse the
stale object:

1. Deleting the `record_body` call from `gemini.rs` red
   `test_every_provider_records_its_body`, naming `gemini.rs`.
2. Deleting the `redact_matched_values` call from `redact` red
   `test_wire_trace_redacts_every_credential` on the `Bearer` value under
   `Authorization`.
3. Deleting the string arm of `redact_matched_values` red it again - the value
   pass cannot be stubbed while keeping the call.
4. Deleting `escape_non_ascii` red `test_wire_trace_output_is_ascii` on the raw
   bytes (`[204, 129, 195, 175, 240, 159, 152, 128]`).
5. Deleting the `/`-flattening in `session_component` red
   `test_wire_trace_file_name_is_sanitised` with an empty match set: the file
   escaped the trace root.
6. Deleting the loop's `set_enabled` line red
   `test_agent_loop_sets_the_switch_from_config`.

## 6. Verification

- `cargo fmt --all -- --check` - clean.
- `cargo clippy -p ragent-llm -p ragent-agent --all-targets` - no finding in a
  touched file (19 pre-existing `unnecessary structure name repetition` in
  `tool_cache.rs`, and the pre-existing `pub(crate) ... inside private module`
  set elsewhere).
- `cargo nextest run -p ragent-llm --no-fail-fast` - 619 passed, 2 failed,
  2 skipped. Both failures are the pre-existing FR-006 / FR-019 source guards
  T-006 and T-010 own: `bedrock.rs:473` still writes the hand-built
  `"tool_result"` literal (present before this task - it is the Bespoke-Messages
  encoder T-006 deletes), and `gemini.rs` still recovers a tool name by
  splitting the id.
- `cargo nextest run -p ragent-agent --no-fail-fast` - 2101 passed, 1 skipped.
- `scripts/check-*.sh` - eleven PASS; `check-bench-regression` exits 2 because
  `target/criterion` does not exist (a missing bench baseline, not a finding).

No dependency (NFR-002), no `unsafe` (NFR-003), no provider contacted, no tool
renamed, and nothing written on the default path.

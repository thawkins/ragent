# T-023 - `tools.wire_trace`: the redacted payload dump

Spec `uniformtools`, task T-023. Requirement FR-017, validated by TESTPLAN case
TC-018.

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
2. **`<session>-<step>` naming** - one file per step of a turn, so a retried send
   (a second request on the wire) is visible as its own file.
3. **Every credential redacted, through the existing sanitizer** - the shared
   `ragent_types::sanitize` primitives, not a new redactor that could drift.
4. **Inert when absent or `false`** - which TC-018 step 2 states strongly: *no
   trace directory is created, and no files are written into the scratch project
   by the run*. "Inert" therefore means "returns before touching the filesystem",
   not "creates the directory and writes nothing".

TC-018 also adds reader-facing obligations: the file names are ASCII (NFR-005),
the file holds no non-ASCII byte, and `grep -c 'sk-' <file>` prints `0`.

## 2. What was built

### `crates/ragent-llm/src/providers/wire/trace.rs` (new)

- `ENABLED: AtomicBool` (default `false`), `set_enabled`, `is_enabled` - the
  process-wide switch. The body only exists inside each provider's
  `build_request_body`, which is a *method on the concrete client*, so the recording
  has to happen there. Threading a per-request flag to fifteen clients would mean
  either a new `ChatRequest` field (touching ~100 construction sites across the
  workspace) or a magic key in `ChatRequest::options` that a provider could
  accidentally serialise onto the wire. A process-wide switch costs one relaxed
  load per send and cannot leak into a request body. The trade-off is stated in the
  module docs rather than hidden: two concurrent sessions with different settings
  share the last value the loop wrote, and the only observable effect is whether an
  extra file appears under `target/temp/`.
- `record_body(&ChatRequest, &Value)` - the one write site. Returns immediately
  while `is_enabled()` is `false`, so a disabled trace creates no directory and
  writes no file. When enabled it names the file
  `<session>-<step:04>.json`, redacts, escapes non-ASCII, and writes atomically
  (temporary file in the same directory, then rename) so a crash cannot leave a
  truncated trace that reads like a truncated request.
- `set_root` (`#[doc(hidden)]`) / `root()` - the test hook that points the tree at
  `target/temp/`, and the default `target/temp/wire-trace` production code ships.
- `session_component` - flattens a session id to ASCII alphanumerics, `-` and `_`,
  and renders an absent id as `no-session`. A session id arrives from storage and
  from callers, so a path separator must not create a nested directory or escape the
  trace root, and an empty id must not produce a file named `-0001.json`.
- `render` - serialise pretty, then `redact`, then `escape_non_ascii`.
- `redact` - two layers, in this order: `redact_named_keys` (any credential-named
  field, via `is_secret_json_key`), then
  `ragent_types::sanitize::redact_json_value` (credential-shaped *values* under
  innocuous keys, e.g. an MCP `Authorization` header or an `env` var). The key-name
  pass replaces **string values only**: a tool's JSON *schema* legitimately declares
  a property called `token`, and masking that object would destroy the very
  document the trace exists to show.
- `escape_non_ascii` - rewrites every non-ASCII `char` as one or more `\uXXXX`
  escapes. A non-ASCII character can only appear inside a JSON string literal,
  where the escape is equivalent, so the file stays valid JSON while every byte is
  ASCII (NFR-005).
- `next_step` - a per-session counter behind a poison-tolerant `Mutex`.

### Provider call sites

`record_body` is called once, with the built body, in every client that speaks a
wire: `anthropic`, `openai`, `openai_responses`, `azure_foundry`, `azure_resource`
(the Anthropic-flavoured Azure client), `gemini`, `huggingface`, `ollama`,
`ollama_cloud`, `openrouter`, `copilot`, and both `bedrock` clients (Messages and
Converse). Two modules are deliberately not covered, and the reason is recorded at
the call site's guard rather than left to inference:

- `mock_llm_client` has no wire at all - it returns a scripted event list.
- `router_client` delegates to a downstream client, whose own `chat` records.
  Recording the router's wrapper as well would write two files for one send.

### `crates/ragent-agent/src/session/loop_steps.rs`

`prepare_client` sets the switch once per turn from the turn's resolved config
(`session_config.tools.wire_trace`), after
`apply_session_backend_override` so an automation's per-session config is what is
read. Setting it per turn means a `wire_trace` change takes effect on the next turn
without a restart, and a default config leaves every provider writing nothing.

## 3. Tests

`crates/ragent-llm/tests/test_wire_trace.rs` - 8 cases. The switch and the root are
process-wide, so every case takes `crate::global_env_lock()` (the consolidated
binary's one lock) and gives its session a unique id.

| Case | What it pins |
|---|---|
| `test_wire_trace_is_inert_by_default` | default off, and a disabled `record_body` writes **nothing** |
| `test_wire_trace_writes_one_file_per_step` | `<session>-0001.json`, `-0002`, and no third file once disabled |
| `test_wire_trace_records_the_tools_and_messages_arrays` | the body, verbatim, as valid JSON |
| `test_wire_trace_redacts_every_credential` | `sk-`, `ghp_` and `Bearer` shapes gone; the shared placeholder present |
| `test_wire_trace_output_is_ascii` | no byte `>= 0x80`; the escape is present; the file name is ASCII |
| `test_wire_trace_file_name_is_sanitised` | a `../` id is flattened into the root; an absent id becomes `no-session` |
| `test_every_provider_records_its_body` | every module implementing `LlmClient` calls `record_body`, minus the two named exemptions |
| `test_agent_loop_sets_the_switch_from_config` | the loop drives the switch from `tools.wire_trace` |

The last two are source guards on purpose. A provider that quietly skipped the call,
or a switch that was never set from config, would leave the trace incomplete or
permanently inert with **no error anywhere** - there is nothing for a runtime test to
observe. The provider guard also asserts it inspected at least ten clients, so it
cannot pass by scanning an empty directory.

The `test_wire_trace_file_name_is_sanitised` case is written against the set of names
this call *added*, not against the directory contents, because the step counter is
per-session and process-wide and the file names therefore differ between runs.

## 4. Red proofs

Each was run and reverted, with the restored file `touch`ed so cargo could not reuse
the broken object file:

1. Deleting the `redact_json_value` pass (leaving only the key-name pass) turned
   `test_wire_trace_redacts_every_credential` red on the `ghp_` value under the
   innocuous `x-note` key.
2. Deleting the `escape_non_ascii` call turned `test_wire_trace_output_is_ascii` red.
3. Deleting the `record_body` call from `gemini.rs` turned
   `test_every_provider_records_its_body` red, naming `gemini.rs`.

## 5. Verification

- `cargo fmt --check` - clean.
- `cargo clippy -p ragent-llm --all-targets` - clean.
- `cargo test -p ragent-llm --test all` - 253 passed, 2 ignored.
- `cargo check --workspace --all-targets` - clean.
- The eight project guard scripts pass (the `check-file-tool-containment` `grep`
  warning about a moved source file is pre-existing and unrelated).

No dependency, no `unsafe` block, no provider call, and nothing written on the
default path.

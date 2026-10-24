# T-010 - Synthetic call ids carry the name; nothing recovers it from the id

Spec `uniformtools`, task T-010. Requirement FR-019:

> If a provider returns a tool call without an id, the system **shall** generate
> one of the form `<provider>_<monotonic counter>_<tool name>`, **shall** record
> the tool name separately in the canonical record rather than recovering it by
> parsing the id, and **shall** log one `tracing::debug!` naming the provider.

The requirement has two halves and both are unwanted-behaviour guards: mint an id
of a stated shape, and never read a tool name back out of one.

## 1. What the tree did before

SPEC's divergence 3 named it: *"Gemini supplies none, so `providers/gemini.rs`
invents `fc_<name>_<n>` and later recovers the tool name by splitting that
synthetic id on `_`. A tool whose name contains an underscore breaks the
recovery."*

Four independent spellings existed, none of them the required one:

| Site | Spelling | Where the name came from later |
|---|---|---|
| `gemini.rs` | `fc_<name>_<counter>` | `wire/encode.rs`, `tool_use_id.split('_').next()` |
| `ollama_cloud.rs` (delta frame) | `ollama_cloud_tc_<index>` | the id-to-name map, so correct but not the FR-019 shape |
| `ollama_cloud.rs` (done frame) | `ollama_cloud_done_tc_<index>` | same |
| `wire/decode.rs` | `tc_<index>` (index-keyed fallback) | nothing - but not the FR-019 shape |

The Gemini recovery was the one FR-019 forbids, and the failure is concrete: a
call to a tool named `read_file` produced the id `fc_read_file_0`, which the
encoder split into `read`. The request then named a tool the model never called -
or, once the rename lands, a *different* registered tool (`Read`), which is worse.

## 2. The change

### `wire/decode.rs` - the accumulator owns the id

- `ToolCallAccumulator::new(profile, provider)` takes the provider id. It is an
  input because FR-019's id starts with it and because the requirement's debug
  line names it; there is no `Default` and no empty-string fallback, so a caller
  states who it is.
- `synthetic_id(&mut self, name) -> String` builds
  `<provider>_<counter>_<name>` from a per-stream `counter` and logs one
  `tracing::debug!` naming the provider, the tool and the id. A call with no name
  at all ends `_unnamed`, so the id is still well formed.
- `open()` mints and *stores* the id on the call before emitting the start, so
  every later frame of that call carries the same string and
  `end` cannot disagree with `start`.
- `defer(name, args_json)` no longer takes an id: it mints one, so Gemini's
  `DeferredCall` id is the accumulator's spelling and no provider invents one.
- `fallback_id()` is deleted. `OpenCall::display_id()` now returns
  `Option<String>` (pinned start id, else the stored id) because an unstarted
  index-keyed call genuinely has none until `open` mints it; a delta that reaches
  such a call is dropped with a debug line rather than sent under a made-up id.
- `entry()` became `position()` (it returns an index, since `open` needs to hold
  no borrow while it mints an id) with identical semantics.

### `wire/encode.rs` - no name recovery

The `GeminiFunctionResponse` carrier names the tool and has no id slot, so the
name comes from `tool_names_by_id(messages)` - built from the canonical
`MessagePart::ToolCall { tool, call_id }` records. The `tool_use_id.split('_')`
fallback is **deleted**; a result whose call the history no longer carries now
goes out with an empty name and one `tracing::debug!`, which is a visibly missing
name rather than a plausible wrong one. (Turning that gap into a hard refusal is
FR-018, T-007.)

### Providers

- `gemini.rs`: `accumulator.defer(name, args)`; the `next_tool_call_id` counter
  and its `fc_` format are gone.
- `ollama_cloud.rs`: the provider's `id` when present, otherwise `idx` names the
  call and the accumulator mints the id. `idx` was already per-frame, so an
  id-less call needed a stream-scoped counter (`unnamed_calls`) - without it two
  unnamed calls in different frames would both key `Index(0)` and fold onto the
  first.
- `sse.rs`: the shared OpenAI-compatible parser builds its accumulator from the
  spec's `provider_label` folded to lower case (`"openai"`, `"openrouter"`), which
  is the provider id rather than the capitalised display name, because a call id is
  written into history and replayed in the next request. The other seven streaming
  modules pass their own id literal at the construction site; a display label that
  is not the provider id would therefore need an explicit field, and none of the
  fifteen registered ids needs one today.

## 3. What did not change

- `StreamEvent` - still `ToolCallStart { id, name }`: the name was already on the
  record, which is why no new field was needed to satisfy "carry the name on the
  record".
- `ContentPart::ToolUse { id, name, input }` and
  `MessagePart::ToolCall { tool, call_id, state }` - already carry both.
- The event id of a provider-supplied call is still the provider's own string,
  byte for byte (FR-006).
- `IdField::Synthesised` / `synthetic_ids` on `GEMINI` and `OLLAMA_NATIVE` - the
  declaration was already right; only the implementation moved.
- No dependency (NFR-002), no `unsafe` (NFR-003), no tool renamed.

## 4. Tests

`crates/ragent-llm/tests/test_wire_decode.rs`:

- `test_accumulator_synthesises_provider_counter_name_ids_for_unnamed_calls`
  (rewrite of the old `..._tc_fallback`): `openai_0_read` for an index-keyed
  unnamed call, the same id on its delta and end, a second unnamed call at the
  same index getting `openai_1_read`, the name `read_file` reaching the consumer
  *whole*, and `openai_0_unnamed` for a frame carrying no name.
- `test_accumulator_pins_the_start_id_so_a_later_id_frame_cannot_split_the_call`:
  the pinned id is now `openai_0_read`; the clause it proves is unchanged.
- `test_accumulator_holds_a_whole_deferred_call_until_it_is_flushed`: `defer` takes
  no id and the flushed starts are `gemini_0_get_weather` / `gemini_1_read_file`.
- **New** `test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one`: a
  source scan over ten modules failing on `format!("fc_`, `format!("tc_`, `_tc_{`
  (a provider spelling its own id) and on `tool_use_id.split` /
  `tool_use_id.rsplit` / `.id.split('_')` (name recovery).

`crates/ragent-llm/tests/test_wire_encode_message.rs`:

- `test_a_gemini_result_never_recovers_a_tool_name_by_splitting_the_id` (replaces
  `..._for_an_unknown_id_still_names_a_tool`, which asserted the derivation this
  task deletes): an orphan id now yields an empty name, and the same carrier with
  the calling turn present names `read_file` whole.

`crates/ragent-llm/tests/inline/ollama_cloud_tests.rs`:

- **New** `test_cloud_chat_synthesises_an_id_for_a_tool_call_that_has_none`: two
  id-less calls in two frames get `ollama_cloud_0_read_file` and
  `ollama_cloud_1_read_file`, each delta attributed to its own call.

## 5. Red proofs

Each applied, observed to fail the named case, and restored byte-identically
(verified by `md5sum`; the restored file is `touch`ed so cargo cannot reuse the
broken object file):

| # | Injected defect | Case that failed |
|---|---|---|
| 1 | `encode.rs`: the `tool_use_id.split('_')` fallback put back | `test_a_gemini_result_never_recovers_a_tool_name_by_splitting_the_id` (`"orphan"` vs `""`) |
| 2 | `decode.rs`: `synthetic_id` reverted to `format!("tc_{counter}")` | `test_accumulator_synthesises_provider_counter_name_ids_for_unnamed_calls` (`tc_0` vs `openai_0_read`) |
| 3 | `gemini.rs`: a `format!("fc_")` id minted again beside `defer` | `test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one` |

## 6. Verification

- `cargo fmt --all -- --check` - clean.
- `cargo clippy -p ragent-llm --all-targets` - no warnings.
- `cargo check --workspace --all-targets` - clean.
- `cargo test -p ragent-llm --test all` - **280 passed**, 2 ignored.
- `cargo test -p ragent-llm --lib` - 327 passed; `--doc` - 30 passed.
- `cargo test -p ragent-agent --test all` - 1883 passed, 1 ignored.
- Guard scripts: `check-inline-tests`, `check-silent-errors`, `check-poison-locks`,
  `check-security-unwraps`, `check-shared-guards`, `check-test-env-isolation`,
  `check-dead-code-reasons`, `check-file-tool-containment` - all PASS.

## 7. What is left to FR-020

Two calls arriving under the *same* provider id are untouched here: `open` on a
key that is already open returns `None`, so the second call's start never reaches
the consumer. Re-keying the later ones is T-011.

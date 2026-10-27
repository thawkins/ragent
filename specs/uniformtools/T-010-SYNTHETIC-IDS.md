# T-010 - Synthetic call ids carry the name; nothing recovers it from the id

Spec `uniformtools`, task T-010. Requirement FR-019:

> If a provider returns a tool call without an id, the system **shall** generate
> one of the form `<provider>_<monotonic counter>_<tool name>`, **shall** record
> the tool name separately in the canonical record rather than recovering it by
> parsing the id, and **shall** log one `tracing::debug!` naming the provider.

Two halves, both unwanted-behaviour guards: mint an id of a stated shape, and
never read a tool name back out of one.

## 1. What the tree did before

SPEC's divergence 3 named it: *"Gemini supplies none, so `providers/gemini.rs`
invents `fc_<name>_<n>` and later recovers the tool name by splitting that
synthetic id on `_`. A tool whose name contains an underscore breaks the
recovery."*

| Site | Spelling | Where the name came from later |
|---|---|---|
| `gemini.rs` | `fc_<name>_<counter>` | `wire/encode.rs`, `tool_use_id.split('_').next()` |
| `ollama_cloud.rs` (stream frame) | `ollama_cloud_tc_<index>` | the id-to-name map (correct, but not FR-019's shape) |
| `ollama_cloud.rs` (done frame) | `ollama_cloud_done_tc_<index>` | same |
| `sse.rs` (shared OpenAI parser) | `tc_<index>` | nothing - but not FR-019's shape |

The Gemini recovery was the forbidden half, and the failure is concrete: a call
to a tool named `read_file` produced `fc_read_file_0`, which the encoder split to
`read` - after the rename, a *different registered tool*.

## 2. State found at the start of this pass

Most of the task had already shipped with `d6abcd4a` (T-008's accumulator
landing): `ToolCallAccumulator::new(profile, provider)`, `synthetic_id`'s
`<provider>_<counter>_<name>` format with its `tracing::debug!`, `defer(name,
args)`, the deletion of `fallback_id`, and the deletion of the `split('_')`
name-recovery in `wire/encode.rs`. `gemini.rs` minted nothing. `cargo nextest run
-p ragent-llm` at HEAD was **631 passed / 1 failed**, the failure being this
task's own guard case
`test_wire_decode::test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one`,
which named exactly one defect:

```
crates/ragent-llm/src/providers/ollama_cloud.rs still mints its own tool-call id
with `_tc_{`; the synthetic id FR-019 asks for is `<provider>_<counter>_<name>`
```

So the delta owed was `ollama_cloud.rs` plus the two cases the record named but
the tree did not have (the Gemini stream case and the two-unnamed-calls Ollama
case), and one design hole the cases exposed.

## 3. The change

### `wire/decode.rs` - a key is not an id

The tree had an ambiguity the new cases found. `StreamKey::CallId` is the
provider's id for the native shapes, and a `position()`-seeded key that only
makes an id-less call addressable otherwise - so "the call has an id" could not
answer "did a provider supply it", and an id-less Ollama call whose key was a
placeholder reached the start under the key rather than under FR-019's id.

`OpenCall` now records the two facts separately:

- `provider_id: Option<String>` - the id a *frame* (or `set_id`) supplied, byte
  for byte (FR-006);
- `key_is_id: bool` - whether that id came from a frame rather than from the key
  being seeded into the field so the call is addressable;

and `started_id: Option<String>` stays the id the consumer was given.

`position()` still seeds a `CallId` key into `provider_id` (so an id-less call
is addressable and later frames reach it), and `store_id` - one write path for
`set_id` and `open`'s `id` argument - sets `key_is_id`. `open` mints FR-019's id
for exactly the calls no frame named:

```rust
let id = match self.open[position].provider_id.clone() {
    Some(id) if self.open[position].key_is_id => self.start_id(id, &name), // FR-020
    Some(_) | None => { /* mint, record in the emitted ledger, pin */ }
};
self.open[position].started_id = Some(id.clone());
```

`display_id` is **deleted** with `fallback_id`. `delta` forwards only under
`started_id` - the id the consumer's pending call was created with - and drops a
delta for a call whose start was never emitted with a `tracing::debug!`, so an
index a provider opened but never started is not sent under an invented id. The
module docs record the rule as `key_is_id` does: a `CallId` key is a handle for
an id-less call and reads as `call@<counter>`, a spelling no provider uses.

### `ollama_cloud.rs` - the stream-scoped counter

Both tool-call arms (the streaming frames and the `done:true` frame) now take the
provider's id when the frame has one - the id stays the key, so a later frame
that repeats no id reaches the same call - and otherwise key the call by a
*stream-scoped* counter (`call@<n>`) while passing **no** id to `open`, so the
accumulator mints `<provider>_<counter>_<name>`. The old per-frame index is the
bug this fixes: `idx` restarts at zero in every NDJSON frame, so two id-less
calls in two frames both keyed `0` and the second folded onto the first.

### `sse.rs` - the provider id is not the display label

`OpenAiSseSpec` gained `provider_id: String` beside `provider_label`, and the
accumulator is built from it, because a call id is recorded in history and
replayed in the next request: `openai_0_read`, not `OpenAI_0_read`.

### `wire/encode.rs` - nothing to delete

The Gemini `functionResponse` name already came from `tool_names_by_id` only,
and an orphan id already went out unnamed and logged. Left as it was; section 4 pins
that absence.

### Providers that needed nothing

`gemini.rs`, `anthropic.rs`, `bedrock.rs` (both clients), `azure_resource.rs`,
`copilot.rs`, `huggingface.rs`, `ollama.rs`, `openai_responses.rs` - each either
supplies real ids or already let the accumulator mint.

## 4. Tests

`crates/ragent-llm/tests/test_wire_decode.rs`:

- `test_accumulator_synthesises_provider_counter_name_ids_for_unnamed_calls`
  (already present): `openai_0_read`, the same id on the call's delta and end,
  `openai_1_read` for a second unnamed call at the same index, `read_file` reaching
  the consumer whole, `openai_0_unnamed` for a frame with no name.
- `test_accumulator_does_not_rekey_a_repeated_frame_of_the_same_call` gained the
  FR-019 boundary, because it is the one place both readings are visible: a frame
  that names only the key supplies no id and gets `openai_responses_0_read`, while
  a frame that carries the id (`Some("call_y")` on key `CallId("call_y")`) is the
  provider's, byte for byte.
- `test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one` kept its
  ten-module scan (`gemini.rs`, `ollama_cloud.rs`, `ollama.rs`, `sse.rs`,
  `copilot.rs`, `huggingface.rs`, `anthropic.rs`, `bedrock.rs`,
  `azure_resource.rs`, `openai_responses.rs`, plus `wire/encode.rs`) over
  `format!("fc_`, `format!("tc_`, `_tc_{` and
  `tool_use_id.split`/`tool_use_id.rsplit`/`.id.split('_')`, and gained
  `CallId(format!("unnamed_` - a name-shaped *key* reaches the consumer as the
  call id whenever the accumulator has to mint over it, so it is the same defect.

`crates/ragent-llm/tests/inline/ollama_cloud_tests.rs` (new):

- `test_cloud_chat_synthesises_an_id_for_a_tool_call_that_has_none` drives the
  real native `/api/chat` stream with two id-less `tool_calls` frames and asserts
  the starts are `ollama_cloud_0_read_file` / `ollama_cloud_1_read_file`, each
  frame's arguments landing on its own call and two ends. Both frames are
  *streaming* frames: the accumulator's counter is per stream, so a call carried on
  the `done:true` frame always follows the streaming ones and would mint `_1_`
  even if the streaming arm folded - the case would pass by luck.

`crates/ragent-llm/tests/test_gemini_tool_calls.rs` (new):

- `test_gemini_unnamed_calls_get_distinct_provider_counter_name_ids` drives the
  real Gemini SSE stream with two `functionCall` parts in one frame and asserts
  `gemini_0_read_file` / `gemini_1_read_file` with `a.rs` and `b.rs` each present
  once.

`crates/ragent-llm/tests/test_wire_encode_message.rs` (unchanged, re-read):

- `test_a_gemini_result_never_recovers_a_tool_name_by_splitting_the_id` asserts
  the absence of the derivation: an orphan id yields an empty name, and the same
  carrier with the calling turn present names `read_file` whole.

## 5. Red proofs

Each applied, observed to fail the named case, restored byte-identically from
`target/temp/t010/` (verified with `md5sum -c after.md5`; the restored file is
`touch`ed so cargo cannot reuse the broken object):

| # | Injected defect | Case that failed |
|---|---|---|
| 1 | `encode.rs`: the `tool_use_id.split('_').next()` fallback put back | `test_a_gemini_result_never_recovers_a_tool_name_by_splitting_the_id` (`"orphan"` vs `""`) |
| 2 | `decode.rs`: `synthetic_id` reverted to `format!("{provider}_{counter}")` | `test_accumulator_synthesises_provider_counter_name_ids_for_unnamed_calls` (`("openai_0", "read")` vs `("openai_0_read", "read")`) |
| 3 | `gemini.rs`: a `format!("fc_{}_{}", name, 0usize)` minted beside `defer` | `test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one` (names `gemini.rs`) |

## 6. Verification

- `cargo fmt --all -- --check` - clean.
- `cargo check --workspace --all-targets` - clean (1m 16s).
- `cargo clippy -p ragent-llm --all-targets` - no finding in a touched file; the
  19 `unnecessary structure name repetition` warnings in `tool_cache.rs` and the
  one `let` binding in `ragent-storage/src/storage.rs:1709` are pre-existing.
- `cargo nextest run -p ragent-llm -p ragent-agent --no-fail-fast` - **2751
  passed, 3 skipped, 0 failed** (`ragent-llm` 634, the file having been 632 at
  HEAD; the two new stream cases plus the T-009 cases are included).
- `cargo test -p ragent-llm --doc` - 30 passed.
- `cargo test --workspace --doc` - 55 passed, 5 failed, all five pre-existing
  compile failures in `ragent-agent` doc examples (`Arc` not imported in
  `read_git_status`, `export_session_archive`/`import_session_archive` argument
  drift, `ToolContext` missing `allowed_roots`/`canonical_cache`, and
  `team::plan_gate_denial`), none of which this task touches.
- Guard scripts: twelve of twelve PASS. `check-bench-regression` was not run (it
  exits 2 when `target/criterion` is absent, which is a missing bench baseline,
  not a code finding).

No dependency added (NFR-002), no `unsafe` (NFR-003), no provider or network
contacted by any test (NFR-004), no tool renamed.

## 7. Deliberately not changed

- `StreamEvent`, `ContentPart::ToolUse` and `MessagePart::ToolCall` - the name was
  already on every record, which is why "carry the name on the record" needed no
  new field.
- `IdField::Synthesised` / `synthetic_ids` on `GEMINI` and `OLLAMA_NATIVE` - the
  declaration was already right; only the implementation moved.
- The provider's own id for a provider-supplied call, byte for byte (FR-006).

## 8. Hand-off

- **FR-020 (T-011) is the neighbour.** The `emitted_ids` ledger already re-keys a
  duplicate id to a fresh minted one and warns once; what T-011 adds is the case
  where two calls in one response genuinely arrive under the same provider id.
- `test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one` is the guard
  that will fail first if any provider goes back to inventing an id or a
  name-shaped key, and it names the file and the retired spelling.

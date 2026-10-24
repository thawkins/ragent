# T-008 - One `ToolCallAccumulator` keyed by the profile's `StreamKeying`

Spec `uniformtools`, task T-008. Requirement FR-004 (index-keyed and id-keyed stream
accumulation), with FR-005 (parallel calls stay independent) as the reason the keying
exists.

## 1. What was there

Four providers each kept their own answer to "which open tool call does this frame
belong to", and the four answers disagreed:

| Module | Structure | Failure it caused |
|---|---|---|
| `anthropic.rs` | `BTreeMap<u64, String>` of open blocks, keyed by SSE content-block index | correct after F5; the only module that got parallel blocks right |
| `sse.rs` (OpenAI / OpenRouter / Azure Foundry) | `HashMap<u64, String>` of ids **plus** a `HashSet<u64>` of started indices | two structures for one question, plus a per-provider `dedup_tool_call_start` switch so OpenRouter could *opt out* of the de-duplication FR-004 requires |
| `bedrock.rs` (Anthropic-shaped path) | `HashMap<String, String>` keyed by id | `iter_mut().last()` and `iter().last()` attributed every `input_json_delta` and every `content_block_stop` to an arbitrary open block |
| `bedrock.rs` (Converse path) | `active_tool_call_id: String` | one slot for N calls: a second parallel `toolUse` block overwrote the first (FR-005's named failure) |
| `gemini.rs` | `Vec<(id, name, args)>` of whole calls flushed at the finish frame | correct for its one-frame shape, but a fifth representation |
| `copilot.rs`, `huggingface.rs`, `ollama.rs`, `ollama_cloud.rs` | a copy of the `sse.rs` `HashMap` + drain-at-finish loop, or a `HashMap<String, String>` | five copies of the same loop, each drifting separately |

Every one of these emitted the same three `StreamEvent`s. The divergence was in the
bookkeeping, not the contract, which is exactly what a declared profile should own.

## 2. What replaced it

New `crates/ragent-llm/src/providers/wire/decode.rs`:

- `StreamKey` - `Index(u64)` or `CallId(String)`, the key a frame names;
  `StreamKey::from_frame(index, id)` is the one place a provider decides which of the
  two its frame supplies (an index wins, then a non-empty id, else the frame names no
  call).
- `ToolCallAccumulator::new(profile)` - built from a `ToolWireProfile`, takes the
  family's `stream_keying` and keeps open calls in a `Vec<OpenCall>` in the order the
  stream opened them. A response carries a handful of calls, so a linear scan is
  cheaper than a map and preserves the ordering FR-005 asks for without a second
  structure.
- `open(key, id, name) -> Option<StreamEvent>` - emits `ToolCallStart` at most once per
  key. **The id is pinned at that moment** (`started_id`), so a frame repeating the
  name cannot emit a second start and a frame carrying an id *after* the start cannot
  split one call into two the consumer cannot pair.
- `delta(&key, args_json) -> Option<StreamEvent>` - attributes a fragment to the call
  its key names, or drops it with a `tracing::debug!` when the key names no open call
  (inventing one would dispatch a tool the model never called).
- `delta_last` / `end_last` - the Converse path's fragment-without-a-key case, which
  until T-009 can only mean the call the stream opened last. Two small methods rather
  than a `None`-keyed hack in the main API.
- `end(&key)` / `end_all()` - one `ToolCallEnd` per *started* call, `ByIndex` families
  sorted by index so a parallel response ends in the model's own order, `ByCallId`
  families in open order. A call whose start was never emitted is never ended, because
  the consumer pairs an end with the pending call its start created.
- `defer` / `deferred_count` / `flush_deferred` - the one whole-argument case (Gemini's
  `functionCall`, which must not be emitted before the `finishReason` sharing its
  frame has been read). This is the only place the accumulator holds argument bytes,
  and it holds a string the caller had already allocated.

The accumulator deliberately does **not** buffer streamed argument fragments: the agent
loop already concatenates them into its own bounded buffer
(`MAX_TOOL_CALL_ARGS_BYTES`), and a second copy would double the memory a large call
takes for no benefit (NFR-001). Fragments are forwarded verbatim, which is what FR-007
requires anyway.

## 3. Call sites converted

Nine modules, all of which now hold one `ToolCallAccumulator` and no block map of their
own:

| Module | Family used | What went away |
|---|---|---|
| `anthropic.rs` | `ANTHROPIC_MESSAGES` | `BTreeMap<u64, String> tool_call_blocks`; `use std::collections::BTreeMap` |
| `sse.rs` | `OPENAI_CHAT` | `HashMap<u64, String> tool_call_ids`, `HashSet<u64> started_tool_call_indices`, **and the `dedup_tool_call_start` switch** |
| `bedrock.rs` (Anthropic path) | `ANTHROPIC_MESSAGES` | `HashMap<String, String> tool_call_args` and both `iter().last()` lookups; the path now keys by index when the frame has one and falls back to `delta_last`/`end_last` when it does not |
| `bedrock.rs` (Converse path) | `BEDROCK_CONVERSE` | `active_tool_call_id: String` |
| `gemini.rs` | `GEMINI` | `Vec<(String, String, String)> pending_tool_calls` and two hand-written emit loops |
| `copilot.rs` | `OPENAI_CHAT` | `HashMap<u64, String> tool_call_ids` + drain loop |
| `huggingface.rs` | `OPENAI_CHAT` | the same map; the `t_` strip now happens to the name passed to `open`, not to a per-site variable |
| `ollama.rs` | `OPENAI_CHAT` | `BTreeMap<u64, String> tool_call_ids` + drain loop |
| `ollama_cloud.rs` | `OLLAMA_NATIVE` | `HashMap<String, String> open_tool_calls` on both the normal frame and the `done` frame |

The `dedup_tool_call_start` field is deleted rather than defaulted to `true`:
FR-004 says exactly one start per call id, so a provider switch that could turn the
guarantee off is a contract with an opt-out. OpenRouter's old "emit duplicate starts"
behaviour is not a behaviour any caller wanted; it was a drift bug.

`ToolFormat` for the tool-list cache is untouched - this task changes the *decode*
side only.

## 4. What deliberately did not change

- **`StreamEvent`** - no field added. The accumulator emits the same three events the
  providers always emitted, so the agent loop, the TUI and every existing test compile
  unchanged.
- **`tc_<index>`** - still the fallback id for an index-keyed call whose frame carries
  no id, because FR-019's `<provider>_<counter>_<name>` form is T-010 and re-keying
  duplicate ids (FR-020) is T-011. Changing the fallback here would have made two
  tasks' worth of test churn land in one diff.
- **Gemini's `fc_<name>_<counter>`** - the id is still computed in `gemini.rs` and
  handed to `defer`, because the id *shape* is FR-019's subject, not FR-004's.
- **The Converse path's last-opened attribution** - preserved exactly, because
  converting it to per-block keys is T-009 (FR-005) and doing it here would duplicate
  that task's proof.
- **`sse.rs`'s FUNC-032 index requirement** - a `tool_call` delta with no `index` is
  still skipped with a warning rather than folded onto stream 0.

## 5. Tests

`crates/ragent-llm/tests/test_wire_decode.rs`, 11 cases:

1. `by_index_attributes_interleaved_deltas_to_their_own_block` - two blocks open,
   fragments alternate, each lands on the block its frame names; closed in reverse
   order by index.
2. `emits_exactly_one_start_and_one_end_per_call` - a repeated name frame emits no
   second start (still yielding its delta), a repeated stop emits no second end.
3. `by_call_id_keying_tracks_the_responses_family` - two `CallId` calls stay separate
   and `end_all` keeps open order.
4. `end_all_closes_by_index_families_in_index_order` - opened 3, 1, 2; ended 1, 2, 3;
   a second `end_all` is empty.
5. `drops_a_delta_that_names_no_open_call` - no invented call, no invented end.
6. `gives_an_index_without_a_provider_id_the_tc_fallback` - `tc_2`, documented as
   T-010's to replace.
7. `pins_the_start_id_so_a_later_id_frame_cannot_split_the_call` - start under `tc_0`,
   then `set_id("call_late")`; the delta and the end stay `tc_0`.
8. `holds_a_whole_deferred_call_until_it_is_flushed` - two deferred calls, not counted
   as open, six events on flush, nothing on a second flush.
9. `stream_key_from_frame_prefers_the_index_and_falls_back_to_the_id`.
10. `every_profile_keying_is_served_by_the_accumulator` - walks all 15 `PROVIDER_WIRE`
    rows and asserts the accumulator takes each family's declared keying.
11. `the_provider_modules_no_longer_keep_their_own_tool_call_maps` - source scan over
    the four modules the task names plus the four OpenAI-family siblings, failing on
    `tool_call_blocks`, `tool_call_ids`, `tool_call_args`, `active_tool_call_id`,
    `pending_tool_calls`, `started_tool_call_indices`, `open_tool_calls`.

## 6. Red proofs

Each break was applied, the suite run, and the file restored byte-identically
(`cp` from `target/temp/`):

| Break | Result |
|---|---|
| `delta` attributes to the last open call instead of the keyed one | cases 1 and 3 fail - the interleaving case and the `ByCallId` case |
| `open` drops its `started_id.is_some()` early return | case 2 fails - a repeated name frame emits a second start |
| `sse.rs` passes `""` instead of the tool name to `open` | `test_openrouter_provider::test_openrouter_chat_stream_maps_sse_events` and `..._emits_reasoning_events_with_transitions` fail - the existing provider tests catch a lost tool name |

## 7. Measurements

| Item | Before | After |
|---|---|---|
| `wire/decode.rs` | absent | 404 lines |
| distinct open-call representations in `providers/` | 5 (`BTreeMap<u64,_>`, `HashMap<u64,_>` + `HashSet<u64>`, `HashMap<String,_>`, `active_tool_call_id: String`, `Vec<(id,name,args)>`) plus 5 copies of the OpenAI loop | 1 (`ToolCallAccumulator`) |
| per-provider de-duplication switch | 1 (`dedup_tool_call_start`, opt-out) | 0 (always one start per call) |
| `ToolCallAccumulator` constructions | 0 | 9 (one per parsed stream: `anthropic`, `bedrock` x2, `gemini`, `sse`, `copilot`, `huggingface`, `ollama`, `ollama_cloud`) |

Verification (all green, one run each):

- `cargo fmt --all -- --check`
- `cargo clippy -p ragent-llm --all-targets` - no warnings
- `cargo test -p ragent-llm --test all` - 237 passed, 2 ignored
- `cargo test -p ragent-llm --lib` - 321 passed
- `cargo test -p ragent-llm --doc` - 28 passed
- `cargo check --workspace --all-targets` - clean
- guard scripts `check-inline-tests`, `check-test-env-isolation`,
  `check-silent-errors`, `check-security-unwraps`, `check-poison-locks`,
  `check-shared-guards`, `check-dead-code-reasons` - all pass
- no dependency added (NFR-002), no `unsafe` block (NFR-003)

## 8. What this hands to the next tasks

- **T-009** replaces `delta_last` / `end_last` on the Converse path with per-block
  keys; the accumulator already accepts `StreamKey::Index`, so that task deletes the
  two helpers' only caller and then the helpers.
- **T-010** replaces `fallback_id`'s `tc_<index>` with `<provider>_<counter>_<name>` and
  moves Gemini's `fc_<name>_<n>` id into the accumulator.
- **T-011** adds duplicate-id reconciliation inside `open`, which is where an id first
  becomes known.
- **T-026** can replay recorded frames through `ToolCallAccumulator` directly, because
  it is a pure state machine over `StreamKey`s (NFR-004).

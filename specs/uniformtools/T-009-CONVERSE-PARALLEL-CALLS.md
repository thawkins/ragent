# T-009 - Bedrock Converse keys a tool call by `contentBlockIndex`

Spec `uniformtools`, task T-009. Requirement FR-005 (parallel tool calls are
distinguished). This is a live bug fix, not a refactor.

## 1. The bug

FR-005 names the failure precisely: *"This applies to the Bedrock Converse path,
which today tracks a single active call id."* Before T-008 the Converse stream
parser held

```rust
let mut active_tool_call_id: String = String::new();
```

and every frame that did not name a call was attributed to whatever that slot
last held. T-008 replaced the slot with the shared `ToolCallAccumulator` but kept
the same attribution rule in two helpers, `delta_last` / `end_last` - "the call
the stream opened most recently" - with the note that per-block keys were T-009's
work.

The Converse wire format does not require that guess. Every event of a block
carries `contentBlockIndex`:

| Event | `contentBlockIndex` | other members |
|---|---|---|
| `ContentBlockStartEvent` | **required**, integer >= 0 | `start` (union: `toolUse` / `text` / `image` / ...) |
| `ContentBlockDeltaEvent` | **required**, integer >= 0 | `delta` (union: `text` / `toolUse` / `reasoningContent` / ...) |
| `ContentBlockStopEvent` | **required**, integer >= 0 | - |

(Verified against the AWS API reference for `runtime_ContentBlockStartEvent`,
`runtime_ContentBlockDeltaEvent` and `runtime_ContentBlockStopEvent`; all three
document the field as `Required: Yes`.)

So the id is only ever on the start frame (`toolUse.toolUseId`), while the index
is on every frame. Keying by id - which is what T-008's Converse path did, because
a fragment frame carries no id - left the fragment unattributable, and the
last-opened guess then merged two blocks:

- two `toolUse` blocks open in one response;
- both blocks' `toolUse.input` fragments are appended to the **second** call,
  because it is the one opened last, so the first call's arguments are lost and
  the second's are a concatenation of two different tool calls;
- the first `contentBlockStop` closes the second call, and the second
  `contentBlockStop` closes nothing (or, with the pre-T-008 slot, closes it twice),
  so the consumer is left with one pending call and an end it cannot pair.

## 2. The fix

`crates/ragent-llm/src/providers/bedrock.rs`, Converse path:

- `contentBlockStart` -> `StreamKey::from_frame(parsed["contentBlockIndex"].as_u64(), Some(id))`,
  so the call is opened at its own index. `from_frame` already prefers an index
  over an id, so the id is still pinned on the start (and is what every emitted
  event carries).
- `contentBlockDelta` -> `accumulator.delta(&StreamKey::Index(index), json_str)`.
  A frame whose `contentBlockIndex` is absent is **skipped with a warning**
  (`FUNC-032`, the rule `sse.rs` already applies to a `tool_calls[]` entry with no
  `index`) rather than folded onto an open call.
- `contentBlockStop` -> `accumulator.end(&StreamKey::Index(index))`, with the same
  absent-index warning. Blocks therefore close in the order their stop frames
  name, which is the model's own order.

The Anthropic-Messages path behind the same provider had the same two fallbacks
(`index` is optional there, so `input_json_delta` and `content_block_stop` fell
back to `delta_last`/`end_last` when it was missing). It now applies the same rule:
no index, no attribution, warn and skip. That is one rule for both Bedrock paths
instead of "index when present, last-opened when not".

`delta_last` and `end_last` are **deleted** from
`crates/ragent-llm/src/providers/wire/decode.rs` - their only caller was this
path, and leaving a last-opened helper in the shared accumulator is leaving the
FR-005 failure one method call away. The module docs now say so, and explain why
the guess is forbidden rather than merely unused.

## 3. Why this is not a behaviour regression for well-formed streams

AWS documents `contentBlockIndex` as required on all three events, so the skipped
path is a malformed-frame path only. Before this change the same frame was
silently attributed to an arbitrary open call - which, for a two-block response,
is a wrong answer rather than a missing one. Skipping is the FR-004 rule already
applied elsewhere: *a delta is attributed to the call its key names; a delta that
names no open call is dropped rather than invented*.

## 4. What was found, not written

T-008 (`d6abcd4a` plus the T-008 landing) had already done most of the
structural half of this task, and honestly recorded why it stopped both Bedrock
paths already key by the index their frame carries and skip-with-a-warning a
frame that carries none. What it did *not* do is delete the two last-opened
helpers; that deletion is this task's (section 5, item 3). Concretely, at the
start of this task:

- `bedrock.rs` Converse `contentBlockDelta` -> `parsed["contentBlockIndex"]`, with
  `tracing::warn!` + `continue` when absent;
- `bedrock.rs` Converse `contentBlockStop` -> the same index, same warning;
- `bedrock.rs` Anthropic-Messages `input_json_delta` and `content_block_stop` ->
  `parsed["index"]`, same warning (one rule for both Bedrock paths);
- `active_tool_call_id` appears in `bedrock.rs` nowhere (the only occurrences in
  the tree are the T-008/T-009 documents and the source scan's retired list);
- `test_wire_decode::test_no_stream_attributes_a_frame_to_the_call_opened_last`
  and `test_a_converse_block_keys_by_content_block_index_not_by_id` are present
  and green.

So the delta this task actually owed was the **proof at the parser**, not the
parser. Nothing about the stream arms needed a code change to pass the FR-005
case - which is exactly why the case had to be written: the property was
unpinned, so the next edit could have taken it away silently.

## 5. What this task added

1. `crates/ragent-llm/tests/inline/bedrock_tests.rs` - two new Converse cases
   driven through the real event-stream parser (`spawn_bedrock_stream` +
   `BedrockConverseClient::chat`), not through the accumulator:
   - `test_converse_chat_stream_keeps_two_parallel_tool_use_blocks_apart` - the
     FR-005 case. Two `toolUse` starts (`tu_a` Read, `tu_b` Grep), four
     `toolUse.input` fragments interleaved `1, 0, 1, 0`, two stop frames in the
     order `1, 0`. Asserts two starts in open order, that the fragments
     concatenated **by id** are exactly `{"path":"a.rs"}` and
     `{"pattern":"fn main"}`, and that there is exactly one end per call, in
     stop-frame order (`tu_b`, then `tu_a`).
   - `test_converse_stream_drops_a_frame_that_names_no_block_index` - the
     FUNC-032 half. An unindexed `toolUse` delta sits between two indexed
     fragments; the named call keeps only its own bytes, no emitted event
     carries a `LEAKED` marker from the dropped frame, and both calls still end
     exactly once.
2. `test_wire_decode::test_no_stream_attributes_a_frame_to_the_call_opened_last`
   - the source scan's retired list is now `delta_last(`, `end_last(`,
   `.last_mut()` and `active_tool_call_id`. It previously looked for the bare
   tokens `delta_last`/`end_last`, which meant the *deletion note* that records
   why they were removed (`decode.rs` module docs, section 6 below) tripped the
   scan that documents it. The function-call form and the id name are what the
   scan is actually about, so the list is narrower and the note can exist.
3. `crates/ragent-llm/src/providers/wire/decode.rs` - the deletion the task
   asks for is finished here: the module docs now carry the note the scan names,
   recording that the two helpers which once offered the last-opened guess are
   **deleted**, not merely left unused, with the reason (such a helper is the
   FR-005 failure one method call away for the next provider that cannot find its
   index). The helper names do not appear as bare tokens in the source, so the
   scan and the prose cannot trip each other. The historical sentence that
   spelled the retired field name says "a single-slot `String`" instead, so the
   prose and the source scan cannot contradict each other. No code was left to
   remove: neither method existed in the tree when this task started.
4. `crates/ragent-llm/src/providers/bedrock.rs` - the Converse
   `contentBlockStart` comment now states the two-arm rule: the index is the
   key, and the `from_frame` fallback covers only a start that supplies an id and
   no index - such a block takes no fragment, because the delta and stop arms
   require the index. Comment only, no behaviour change.

## 6. Red proofs

Each break was applied, the suite run, and the file restored byte-identically from
`target/temp/` (with `touch`, so cargo could not reuse the broken object file):

| Break | Result |
|---|---|
Three breaks, run individually, each restored byte-identically from
`target/temp/t009/` (`cp -p` then `touch`, and `md5sum -c` against
`target/temp/t009/after.md5`) before the next one:

| Break | Result (verbatim from the run) |
|---|---|
| Converse start, delta and stop all keyed `StreamKey::Index(1)` (the single-slot behaviour) | **both** new cases fail. Interleaving case: `panicked at bedrock_tests.rs:604: assertion left == right failed: each toolUse block opens its own call` - one start instead of two. Unindexed-frame case: same file, `left: ...` on the argument rejoin. |
| The `toolUse` delta arm ignores `contentBlockIndex` (`let Some(index) = Some(0u64)`) | `test_converse_stream_drops_a_frame_that_names_no_block_index` fails at the rejoin: `left: "{\"path\":\"LEAKED\"}\"a.rs\"}"`, `right: "{\"path\":\"a.rs\"}"` - the dropped frame leaked into the named call. The interleaving case also reds (`left: "{\"pattern\":{\"path\":\"fn main\"}\"a.rs\"}"`), because both blocks collapse onto block 0. |
| `end_last` re-added to `ToolCallAccumulator` | `test_no_stream_attributes_a_frame_to_the_call_opened_last` fails: `wire/decode.rs still carries the last-opened attribution 'end_last'` (later, `'delta_last'` and `'active_tool_call_id'` as the retired list was narrowed) |

The first break is the one the old architecture scored: it is what
`active_tool_call_id` *was*, and it now fails at the parser rather than only at
the accumulator.

## 7. What deliberately did not change

- **`StreamEvent`** - no field added; the emitted events are the same three.
- **`tc_<index>`** - still the id an index-keyed call gets when the provider
  supplies none, because FR-019's `<provider>_<counter>_<name>` form is T-010.
- **The id every event carries** - still the provider's `toolUseId`, pinned at the
  start, so a start and its end always name the same call.
- **`end_all`** - untouched; it remains the finish-frame drain the OpenAI-family
  providers use, `ByIndex` families closing in index order.

## 8. Verification (all green)

- `cargo fmt --all -- --check` - clean
- `cargo check --workspace --all-targets` - clean (1m 14s)
- `cargo clippy -p ragent-llm --all-targets` - only the pre-existing 19
  `unnecessary structure name repetition` in `tool_cache.rs` and one
  `returning the result of a let binding` in `ragent-storage/src/storage.rs:1709`;
  nothing in a file this task touched
- `cargo nextest run -p ragent-llm --no-fail-fast` - **632 run, 631 passed, 1
  failed, 2 skipped**. The one failure is T-010's
  `test_wire_decode::test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one`
  (`ollama_cloud.rs` still mints `_tc_`), red at HEAD and identical without this
  change.
- `cargo nextest run -p ragent-llm --lib` - 328 passed
- `cargo test -p ragent-llm --doc` - 30 passed
- guard scripts - 12 of 13 pass; `check-bench-regression` exits 2 on
  "no criterion results directory found at target/criterion" (no bench run, not a
  code finding)
- no dependency added (NFR-002), no `unsafe` block (NFR-003), no tool renamed

## 9. What this hands to the next tasks

- **T-010** can emit `<provider>_<counter>_<name>` from the accumulator's
  fallback id without touching either Bedrock path: both now pass a real index.
  Its outstanding red is the one named above.
- **T-011** reconciles duplicate call ids inside `open`, which is where an id first
  becomes known.
- **T-026** can replay a Converse two-block frame set through `ToolCallAccumulator`
  directly; the new inline case is that fixture in miniature.

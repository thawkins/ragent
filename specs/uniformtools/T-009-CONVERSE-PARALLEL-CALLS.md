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

## 4. Tests

`crates/ragent-llm/tests/inline/bedrock_tests.rs` (the file the source's
`#[cfg(test)] #[path]` hook includes), three Converse cases:

1. `test_converse_chat_stream_text_tool_and_finish` - updated, not weakened: the
   frames now carry the `contentBlockIndex` the wire requires (the text delta is
   block 1, the tool call block 0). Every original assertion is unchanged.
2. `test_converse_chat_stream_keeps_two_parallel_tool_use_blocks_apart` - **new**,
   the FR-005 case. Two `toolUse` blocks open back to back, four argument
   fragments interleave `1, 0, 1, 0`, and the two stop frames arrive `1, 0`. It
   asserts the starts are `[tu_a Read, tu_b Grep]`, that reassembling each call's
   fragments by id yields exactly `{"path":"a.rs"}` and `{"pattern":"fn main"}`
   (neither absorbing the other's input), and that there is exactly one end per
   call in stop-frame order.
3. `test_converse_stream_drops_a_frame_that_names_no_block_index` - **new**, the
   `FUNC-032` half. A `toolUse` delta with no `contentBlockIndex` between two
   indexed fragments is not attributed to either open call; the call keeps only
   the fragments that named it and still ends exactly once.

`crates/ragent-llm/tests/test_wire_decode.rs`, two cases added:

4. `test_no_stream_attributes_a_frame_to_the_call_opened_last` - a source scan over
   `wire/decode.rs`, `bedrock.rs`, `anthropic.rs`, `gemini.rs` and `sse.rs`,
   failing on `delta_last`, `end_last` and `open.last_mut()`. Structural, because
   the failure it prevents is a *caller* reaching for the fallback.
5. `test_a_converse_block_keys_by_content_block_index_not_by_id` - the pure
   state-machine half: `from_frame(Some(0|1), Some("tu_a"|"tu_b"))` resolves to
   `Index`, two interleaved calls keep their own fragments, and each ends once.

## 5. Red proofs

Each break was applied, the suite run, and the file restored byte-identically from
`target/temp/` (with `touch`, so cargo could not reuse the broken object file):

| Break | Result |
|---|---|
| Converse keys every block by index `1` (the pre-T-008 single-slot behaviour) | both new Converse cases fail: one start instead of two, one end instead of two |
| Converse ignores `contentBlockIndex` on a delta and uses a fixed block | `test_converse_stream_drops_a_frame_that_names_no_block_index` fails on the reassembled arguments (`""` instead of `{"path":"a.rs"}`) |
| `end_last` re-added to the accumulator | `test_no_stream_attributes_a_frame_to_the_call_opened_last` fails, naming `wire/decode.rs` and `end_last` |

## 6. What deliberately did not change

- **`StreamEvent`** - no field added; the emitted events are the same three.
- **`tc_<index>`** - still the id an index-keyed call gets when the provider
  supplies none, because FR-019's `<provider>_<counter>_<name>` form is T-010.
- **The id every event carries** - still the provider's `toolUseId`, pinned at the
  start, so a start and its end always name the same call.
- **`end_all`** - untouched; it remains the finish-frame drain the OpenAI-family
  providers use, `ByIndex` families closing in index order.

## 7. Verification (all green)

- `cargo fmt --all -- --check`
- `cargo clippy -p ragent-llm --all-targets` - no warnings
- `cargo test -p ragent-llm --test all` - 279 passed, 2 ignored
- `cargo test -p ragent-llm --lib` - 326 passed
- `cargo test -p ragent-llm --doc` - 30 passed
- `cargo check --workspace --all-targets` - clean
- guard scripts `check-inline-tests`, `check-test-env-isolation`,
  `check-silent-errors`, `check-security-unwraps`, `check-poison-locks`,
  `check-shared-guards`, `check-dead-code-reasons` - all pass
- no dependency added (NFR-002), no `unsafe` block (NFR-003), no tool renamed

## 8. What this hands to the next tasks

- **T-010** can emit `<provider>_<counter>_<name>` from the accumulator's
  `fallback_id` without touching either Bedrock path: both now pass a real index.
- **T-011** reconciles duplicate call ids inside `open`, which is where an id first
  becomes known.
- **T-026** can replay a Converse two-block frame set through `ToolCallAccumulator`
  directly; the new inline case is that fixture in miniature.

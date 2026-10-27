# T-018 - Echo the signed reasoning block where the profile requires it

Spec `uniformtools`, task T-018. Requirement FR-011 (reasoning echo for profiles
that require it, state-driven).

Status: **completed**. Dependencies: T-001 (the canonical `ContentPart::Reasoning`
part and its `signature`), T-006 (the one message encoder).

## 1. What the task asks for

> **FR-011** (state-driven) While the active profile declares
> `echo_reasoning == true`, the history encoder **shall** replay the stored
> reasoning block, including its signature, ahead of the tool-use block of the
> same assistant turn. While the profile declares `echo_reasoning == false`,
> reasoning **shall** continue to be omitted.

Three things have to be true for that sentence:

1. the block is **stored** with its signature (a signature lost at the record is
   an unverifiable replay, not a replay);
2. the block reaches the **encoder** on the assistant turn that answers the call
   - within that same turn, not one request later;
3. the encoder **replays** it, ahead of the call, for a profile that declares
   `echo_reasoning`, and omits it everywhere else.

## 2. State found

| # | State | Where |
|---|---|---|
| 1 | `ContentPart::Reasoning { text, signature }` and `MessagePart::Reasoning { text, signature }` already existed with the omission rule on the signature key (T-001). | `ragent-types` |
| 2 | `StreamEvent::ReasoningSignature` already existed and the three Anthropic-speaking parsers already emitted it from `signature_delta` (Anthropic, Azure Resource, Bedrock). | `providers/{anthropic,azure_resource,bedrock}.rs` |
| 3 | **The loop collected the signature but nothing recorded it.** `LlmStepResult` had `reasoning_buffer` and no signature field; the saved `MessagePart::Reasoning` was built as `{ text }`, so every persisted block was unsigned. A red proof of this is section 5, proof 2. | `session/{loop_steps,processor}.rs` |
| 4 | **Both history converters dropped the part entirely**: `MessagePart::Reasoning { .. } => { /* not forwarded */ }`. No reasoning block could reach any provider from history. | `session/history.rs` (two converters), `compaction/convert.rs` |
| 5 | **Within the turn, no reasoning part was written to `chat_messages` at all.** The tool-step assistant message carried `Text` and `ToolUse` only. This is the failure mode that makes a converter-only fix look sufficient and behave incorrectly: the echoing request is built from `chat_messages`, so a block that exists only in the saved `MessagePart`s arrives on the *following* turn's rebuilt history. | `session/processor.rs` tool phase |
| 6 | The encoder had no `thinking` arm. `anthropic_message` wrote `text` / `tool_use` / `tool_result` / `image`; every family dropped `Reasoning` via `_ => None`. | `providers/wire/encode.rs` (HEAD) |
| 7 | The profile already declared the decision: `ANTHROPIC_MESSAGES.echo_reasoning == true`, the other five `false`, with the FR-016 matrix printing it. | `providers/wire/profile_table.rs` |

Rows 3, 4 and 5 are the reason this was not an encoder-only task: the encoder
was reachable only from a history that had already lost both halves, and the
in-turn path had no reasoning part to encode.

A note on why row 5 is not merely a redundancy of row 4. A turn that calls a
tool issues a *second* request for the same turn (the one that carries the
results). Its history is `chat_messages`, which is built by appending the step's
assistant message and then the tool results. The saved `MessagePart`s only reach
a request at the *next* turn's `history_to_chat_messages` rebuild. A provider
that requires the signed block on the turn answering the call would therefore
have received nothing it needed, while a test driving `encode_message` directly
would have passed.

## 3. What landed

### 3.1 The loop records the signature (rows 3)

- `LlmStepResult` gained `reasoning_signature: Option<String>`, documented as the
  provider's signature over `reasoning_buffer`.
- `call_llm_step` holds `let mut reasoning_signature: Option<String> = None` and
  sets it on `StreamEvent::ReasoningSignature`.
- The retry reset now clears it beside the text it signed. Clearing the text and
  keeping the signature would pair a fresh block with the previous attempt's
  signature, and a provider verifies the two together - a mismatch is rejected,
  not merely unverified.
- The saved record is `MessagePart::Reasoning { text, signature }`, so a resumed
  session replays a signed block rather than an unsigned paraphrase.

### 3.2 The converters forward it (row 4)

`parts_to_chat_content_sync`, `parts_to_chat_content` and
`chat_messages_to_messages` push `ContentPart::Reasoning { text, signature }`
instead of dropping the part. Neither converter consults `echo_reasoning`: the
omission for a profile that does not echo is a property of the wire contract, so
it stays in the encoder. `crates/ragent-agent/tests/test_tool_input_error.rs`
asserts that `history.rs` never names `echo_reasoning`, which is what stops a
converter from re-implementing the decision.

### 3.3 The turn echoes within itself (row 5)

The tool phase pushes the reasoning part into `assistant_content_parts`
**before** the text and before any `ToolUse`:

```rust
if !llm_result.reasoning_buffer.is_empty() {
    let text = std::mem::take(&mut llm_result.reasoning_buffer);
    let signature = llm_result.reasoning_signature.take();
    assistant_content_parts.push(ContentPart::Reasoning { text, signature });
}
```

The order is the requirement: FR-011 says "ahead of the tool-use block of the
same assistant turn". The push sits above the text push rather than beside the
`ToolUse` push precisely so it cannot drift behind the call if the phase is
reordered later. It is unconditional - no profile lookup on this path.

### 3.4 The encoder replays it (row 6)

`anthropic_message` gained a `Reasoning` arm delegating to a new
`reasoning_block(profile, part)`:

- returns `None` when `!profile.echo_reasoning` (the declared omission);
- writes `{"type":"thinking","thinking":<text>}`;
- adds `"signature"` only when the record has one - an unsigned block gains no
  key, and in particular no `null`, matching T-001's omission rule (fabricating
  an empty signature would claim the provider signed a block it did not);
- is a total match on the part, so a changed caller cannot produce a `thinking`
  block for anything but a reasoning part.

Every other family's `Reasoning` arm is unchanged and documented as the declared
omission rather than a loss, which distinguishes it from the Responses image part
that FR-018 refuses.

## 4. Deliberately unchanged

- **No other family gained a reasoning arm.** `echo_reasoning` is `false` for
  `openai_chat`, `gemini`, `bedrock_converse`, `openai_responses` and
  `ollama_native`; those arms keep their comments and their behaviour. The
  Bedrock *Anthropic* client (`BedrockAnthropicClient`) already passes
  `ANTHROPIC_MESSAGES`, so it echoes through the same arm as the direct client -
  which is why the profile, not the provider module, is the thing that decides.
- **The profile table was not edited.** `echo_reasoning` was declared in T-002/T-003
  and read only now; the red proof in section 5 flips one row to show the tests
  read it.
- **No config key, no CLI flag.** FR-011 is state-driven on the profile.
- **`should_retry_stream_error`, the compaction estimator and the byte estimator
  were untouched.** The estimator already counted `Reasoning { text, .. }` by
  `text.len()`; a signature is a few dozen bytes and is not a text estimate.

## 5. Red proofs

Each mutation was applied, the named test run, and the file restored
byte-identically (`cp -p` from `target/temp/t018/`, `touch`, `md5sum -c`).

| # | Mutation | Case that reddened | Panic text |
|---|---|---|---|
| 1 | the within-turn `assistant_content_parts.push(ContentPart::Reasoning { .. })` deleted | `test_loop_steps_harness::test_the_echoed_reasoning_block_precedes_the_tool_call_in_the_same_turn` | `the echoing turn must carry the reasoning block (FR-011)` |
| 2 | `signature` replaced with `None` in that push | same case | `assertion left == right failed`: `left: None`, `right: Some("EqQBCkYIBxgCKkA")` |
| 3 | `ANTHROPIC_MESSAGES.echo_reasoning` flipped `true -> false` | `test_wire_encode_message::test_anthropic_replays_the_signed_thinking_block_before_the_call` | `assertion left == right failed: the signed block is replayed first, byte for byte`, `left` = the `tool_use` block, `right` = the expected `thinking` block |

Proof 1 is the one that matters: it is the red that a converter-only fix would
not produce. Proof 2 is the red that a text-only record would not produce.
Proof 3 shows the encoder arm is gated by the declaration and not by the shape.

## 6. Tests

`crates/ragent-agent/tests/test_loop_steps_harness.rs`, 7 -> 9 cases. The harness
gained a `ScriptedReply::SignedReasoningThenToolCall` variant that streams
`ReasoningStart`, `ReasoningDelta`, `ReasoningEnd`, `ReasoningSignature`, then a
call and a `ToolUse` finish - the frame order the Anthropic parser produces.

- `test_the_echoed_reasoning_block_precedes_the_tool_call_in_the_same_turn` -
  drives `process_message` for a tool step plus a closing text step and asserts,
  on the **second captured request** (the one that answers the call), that the
  assistant turn carries a `Reasoning` part *before* its `ToolUse` part, with both
  the text and the signature intact.
- `test_the_persisted_reasoning_part_keeps_its_signature` - asserts the saved
  `MessagePart::Reasoning` holds the streamed signature and that the saved parts
  begin `reasoning, tool_call`.

Pre-existing cases that this task made green or that already covered it:
`test_wire_encode_message::test_anthropic_replays_the_signed_thinking_block_before_the_call`,
`...::test_a_profile_without_echo_reasoning_still_omits_the_block`,
`...::test_an_unsigned_reasoning_block_is_replayed_without_a_signature`,
`...::test_the_conversation_encoder_replays_the_signed_block`,
`test_tool_input_error::the_reasoning_signature_is_recorded_and_replayed_from_history`,
`test_canonical_tool_parts` (the serde omission rule).

## 7. Verification

- `cargo fmt --all -- --check` - clean.
- `cargo nextest run -p ragent-llm -p ragent-agent --no-fail-fast` - nextest
  reports **2810 tests run: 2805 passed, 5 failed, 3 skipped**. All five failures are the pre-existing
  `test_tool_rename_guard::` group (the T-047 predicate-(g) hit at
  `crates/ragent-agent/src/tool/aliases.rs:118`, plus predicate (e) on a guard
  fixture), unrelated to this task and unchanged by it.
- `cargo clippy -p ragent-agent -p ragent-llm --all-targets` - no warning
  originates in `processor.rs`, `loop_steps.rs`, `wire/encode.rs`,
  `profile_table.rs` or `test_loop_steps_harness.rs` (the surviving groups are the
  pre-existing `unnecessary structure name repetition` in `tool_cache.rs`, the
  `pub(crate) item inside private module` group and one `let`-binding in
  `ragent-storage`).
- Guard scripts: 11 of 13 pass. `check-bench-regression` exits 2 for a missing
  `target/criterion`; `check-tool-rename` exits 1 on the same pre-existing
  T-047 hit.

## 8. Notes for the downstream tasks

- **A seventh wire family** that can carry signed reasoning needs two edits: a
  `thinking`-equivalent arm in its message encoder, and `echo_reasoning: true` on
  its profile. The encoder arm alone changes nothing, because the arm is gated by
  the declaration.
- **The echo is two-sided and both sides must exist.** A test that only drives
  `encode_message` cannot catch a missing within-turn part, and a test that only
  reads the saved `MessagePart`s cannot catch a missing encoder arm. The two new
  harness cases and the existing encoder cases split that boundary deliberately.
- **T-026** (the remaining pending task) does not touch this path.

# T-026 - Recorded-frame replay fixtures, one per provider family (NFR-004)

Task: add recorded-frame replay fixtures (one per provider family) and
per-profile encoder/decoder tests. Requirement: NFR-004. Dependencies:
T-006, T-008.

## 1. The requirement this task lands

NFR-004, verbatim:

> Each requirement **shall** be exercisable without a live provider: a loopback
> SSE server can replay recorded frames per profile, and the encoders are pure
> functions over `&[ToolDefinition]` and `&ChatMessage`.

Two clauses, and the task is one test file each:

- the **replay** clause - a recorded body per family, served by a loopback
  `TcpListener` and parsed by that family's own stream parser;
- the **purity** clause - the encoders are functions of their arguments.

## 2. State found

The tree already had a great deal of per-family stream coverage, and none of it
was this. Every case below replayed a hand-built frame against one provider, in
a file that did not know the other families existed.

| File | What it already covered | What it could not show |
|---|---|---|
| `tests/inline/anthropic_tests.rs` | one `tool_use` block, indexed deltas, `stop_reason` | that the call the decoder produced survives the encoder written back |
| `tests/test_ollama_tool_handling.rs` | chat-completions-shaped frames | the *native* `/api/chat` family at all |
| `tests/test_gemini_tool_calls.rs` | `functionCall` in the final candidate | the minted id the accumulator gives it |
| `tests/test_m5_6_provider_consistency.rs` | one Responses frame, one `[DONE]` | any tool construct |
| `tests/test_wire_decode.rs` | the accumulator directly, no HTTP | a provider's own parser reaching the accumulator |
| `tests/test_wire_encode_message.rs` | the encoder directly, no HTTP | the decoder the history it re-encodes came from |

No test anywhere drove one family's **decode** and then fed its output into that
family's **encode**. That gap is what a fixture closes: a decoder and an encoder
that agree only with themselves - the same wrong key on both sides - pass every
test in the table above.

Two encoder defects were found while writing the fixtures, both in
`ollama_native_message` and both invisible without a replay:

1. a message holding both text and a tool construct was written as **one** entry
   in a two-way race: text *or* the construct, never both. An assistant turn that
   said something and called a tool lost the call; a user turn carrying the
   narrative text and the result lost the result (`src/providers/wire/encode.rs`).
2. the text of such a message was flattened with `texts.join("\n")`, so a single
   image-only or construct-only message became `"content": ""` where the
   OpenAI-family rule (the block form) writes an empty array.

## 3. What was built

`crates/ragent-llm/tests/test_wire_replay.rs`, registered in `tests/all.rs` as
`test_wire_replay`, seven cases:

| Case | Family | Recorded body | Asserts |
|---|---|---|---|
| `test_openai_chat_replay_round_trips_through_the_encoder` | `OPENAI_CHAT` | `chat.completion.chunk` SSE, index-keyed `tool_calls`, two argument fragments | one call `call_REDACTED1`/`Read`, arguments concatenated, the recorded `[ToolUse, Stop]` finish pair, the OpenAI envelope in the captured request, and the assistant turn re-encoded as `function.arguments` as a **string** |
| `test_anthropic_replay_round_trips_through_the_encoder` | `ANTHROPIC_MESSAGES` | `content_block_*` SSE: thinking + text + `tool_use` | one call `toolu_REDACTED1`/`Read`, the `signature_delta` reaching `ReasoningSignature`, `input_schema` in the captured request, and the re-encoded turn carrying `thinking` (with its signature) **before** `tool_use` with object arguments |
| `test_gemini_replay_round_trips_through_the_encoder` | `GEMINI` | NDJSON candidates with `functionCall` and `finishReason: TOOL_USE` | the minted `gemini_0_Read` id (FR-019), `functionDeclarations` in the captured request, `functionCall` out and `functionResponse` naming the tool back |
| `test_bedrock_converse_replay_round_trips_through_the_encoder` | `BEDROCK_CONVERSE` | `contentBlock*` events for **two** parallel blocks | two calls, two ends in index order, each block's own arguments (FR-005), `toolConfig.tools[].toolSpec` in the captured request, `toolUse` out and `toolResult` with the native `status: error` back |
| `test_openai_responses_replay_round_trips_through_the_encoder` | `OPENAI_RESPONSES` | `response.output_item.added` + id-keyed `function_call_arguments` events | one call, the flattened envelope (no `function` object) in the captured request, and the input list carrying `function_call` + `function_call_output` rather than only text (FR-018) |
| `test_ollama_native_replay_round_trips_through_the_encoder` | `OLLAMA_NATIVE` | NDJSON `/api/chat` frames, an **id-less** `tool_calls` entry | the minted `ollama_cloud_0_Read` id (FR-019), the OpenAI envelope in the captured request, and the re-encoded turn keeping its text *and* its call, its text *and* its result |
| `test_every_family_encoder_is_a_pure_function_of_its_inputs` | all six | none | the purity clause: `encode_messages` twice over the same history is byte-identical, for every `FAMILIES` row |

The shared half is [`assert_one_call`]: one start, deltas attributed to it,
exactly one end carrying the same id, at least one finish. A family that opens
two calls (a repeated frame), none (a dropped call) or ends a different id fails
on the same line as its siblings.

### Fixtures are recordings, in the shape each family's parser reads

Each fixture is written from the service's documented stream format, not from
our own output, and every one keeps `REDACTED` in its ids so a real credential or
account id can never be read as a fixture value. Three shapes had to be corrected
against the parsers during the work, and each correction is the finding:

| Family | Shape the parser reads | What was wrong at first |
|---|---|---|
| `GEMINI` | newline-delimited **plain JSON** (`take_sse_line` on each line, no `data:` prefix) | replayed as `data:` SSE, which the parser skips: the replay was empty |
| `OPENAI_RESPONSES` | `data:` lines with **no** `event:` line; dispatch is on the JSON `type` | replayed with `event:` lines, which the parser skips: the replay was empty |
| `BEDROCK_CONVERSE` | `event:` line + `data:` line pairs | JSON written without the `data:` prefix, which the parser skips: the replay was empty |

Each produced an empty event list rather than a wrong one, which is why the
first assertion is "exactly one start" rather than "the arguments match": an
absent replay must fail loudly on the first line, not on a downstream field.

## 4. The one production change

`ollama_native_message` (`crates/ragent-llm/src/providers/wire/encode.rs`) now
writes construct-bearing messages as the ordinary `{role, content}` entry
followed by one entry per construct:

- a turn with no tool construct is unchanged (one entry, `images` beside it);
- a turn with text **and** calls writes the text entry then the assistant
  `tool_calls` entry, because the recorded request shape accepts both and the old
  two-way race dropped one;
- a turn with text **and** results writes the text entry then one `role:"tool"`
  entry per result, which is the shape that previously lost the result of any
  tool call the model narrated before answering;
- the assistant text entry is emitted only when the message has text: the native
  endpoint rejects an assistant turn with empty content and no calls, so an
  assistant turn that did nothing but call a tool still writes just the call.

`joined_text` was added beside `openai_content` for the same rule the block form
follows: only a text part becomes a string, because collapsing anything else
writes `"content": null` and loses the part.

No other production file changed. No provider module, no profile, no type.

## 5. Red proofs

Each proof was applied, the file recompiled, the failing assertion read, and the
source restored from `target/temp/t026/proofs/` with `cp -p`, `touch` and an
`md5sum -c` verification (`encode.rs` `2a7f1f78...`, `test_wire_replay.rs`
`306b50b7...`).

| Proof | Edit | Result |
|---|---|---|
| 1 | `if has_text` -> `if false && has_text` in `ollama_native_message` | `test_ollama_native_...` reds `left: 1, right: 2` - the text entry the shape requires is gone |
| 2 | `if tool_calls.is_empty() && results.is_empty()` -> `if tool_calls.is_empty()` | `test_ollama_native_...` reds at the answer half `left: 1, right: 2` - the narrative text entry is gone, i.e. the old race is back |
| 3 | `reasoning_block` stops writing `signature` | `test_anthropic_...` reds `left: Null, right: "EqQBCkYIBxgCKkA"` - an unsigned replay is caught by the fixture, which is the FR-011 property the recorded frame exists for |

Proof 2 is the one that proves the fixture is worth having: it restores exactly
the behaviour the tree had before the task, and only a replay of a recorded
*request* shape notices.

## 6. Recorded limits

- **One family has no second client.** `bedrock` also serves Anthropic models
  through `BedrockAnthropicClient`, and `azure_resource` through
  `AzureAnthropicClient`. Both speak `ANTHROPIC_MESSAGES`, which the Anthropic
  case replays, so neither has a fixture of its own; a case per *client* would be
  a case per transport, not per family.
- **The Responses fixture does not cover `image` refusal.** `function_call` and
  `function_call_output` are asserted; the `WireLoss` refusal for an image part
  and for an unknown role belongs to T-006's `test_wire_encode_message.rs`, where
  it already is.
- **A family may settle its reason twice.** The chat-completions parser maps
  `finish_reason` and then the family's end-of-stream frame settles `Stop`.
  The helper returns the **last** reason the consumer sees and the chat case pins
  the recorded pair `[ToolUse, Stop]` explicitly, so the double settle is
  asserted rather than smoothed away.
- **`AWS_*` is process-global.** The Bedrock case is a `#[test]` that owns a
  current-thread runtime, takes `crate::global_env_lock()` and never holds it
  across an `await` in the test's own frame, which is what
  `clippy::await_holding_lock` requires. It is not `#[tokio::test]` for exactly
  that reason.

## 7. Verification

- `cargo fmt --all --check` clean.
- `cargo clippy -p ragent-llm --all-targets` names no line this task added.
- `cargo nextest run -p ragent-llm`: **661 run, 661 passed, 2 skipped**, of
  which 7 are `test_wire_replay::`.
- `cargo nextest run -p ragent-agent` (the crate that consumes the encoder):
  2156 run, 2151 passed, 5 failed - all five the pre-existing
  `test_tool_rename_guard::` group (T-047), zero non-guard failures.
- The file is listed in `tests/all.rs`; a replay file that is not listed never
  runs, which is the orphan-module defect this spec has hit six times.

## 8. Files

| File | Change |
|---|---|
| `crates/ragent-llm/tests/test_wire_replay.rs` | new, 7 cases, 1052 lines |
| `crates/ragent-llm/tests/all.rs` | one `#[path]` / `mod` pair |
| `crates/ragent-llm/src/providers/wire/encode.rs` | `ollama_native_message` writes text + constructs; `joined_text` |
| `specs/uniformtools/PLAN.md` | this task's row -> completed |
| `specs/uniformtools/TESTPLAN.md` | TC-003's automated half recorded |
| `CHANGELOG.md` | one `### Added` bullet |

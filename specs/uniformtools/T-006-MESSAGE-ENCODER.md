# T-006 - One profile-driven `encode_message`, four result carriers, seven loops gone

Spec `uniformtools`, task T-006. Requirement FR-006 (the active provider's encoder
emits the carrier its profile declares, with the id byte-identical to the one the
model returned).

## 1. What was found, and what was not

The task read as "write the encoder". The encoder was already there - `commit
d6abcd4a` landed `wire/encode.rs` with `MessageFamily`, `MessageContext`,
`tool_names_by_id` and `encode_message` - but only **one** of the eight arms had
been converted. `anthropic.rs` called the shared encoder; the other seven copies
of the message loop were still in the tree, byte for byte:

| File | Arms | What the copy still wrote |
|---|---|---|
| `anthropic.rs` | 1 | converted - `encode_message(ANTHROPIC_MESSAGES, ...)` |
| `bedrock.rs` (`BedrockAnthropicClient`) | 1 | the Anthropic blocks, a second copy: `"tool_result"` |
| `bedrock.rs` (`BedrockConverseClient`) | 1 | `"toolUse"` / `"toolResult"`, plus its own role filter |
| `gemini.rs` | 1 | `"functionCall"` / `"functionResponse"`, name from `split('_')` |
| `openai_compat.rs` | 1 | `"tool_calls"`, `role:"tool"` keyed by `"tool_call_id":` |
| `huggingface.rs` | 1 | the OpenAI copy with `t_` names and no `tool_name` |
| `ollama_cloud.rs` | 1 | object arguments, `role:"tool"` keyed by `name` |
| `openai_responses.rs` | 1 | its own `build_input_items`, `"function_call_output"` |

The regression proof was already in the tree and **red at the start of this
pass**: `test_wire_encode_message::test_no_provider_module_writes_a_result_carrier_literal`
failed with

```
bedrock.rs still writes "tool_result", which is the Anthropic result carrier;
every request-side carrier belongs to `wire::encode::encode_message` (FR-006)
```

This is the second time in this spec that a task's behaviour test shipped before
its implementation (T-008 was the first). The measured work was therefore the
*deletion* plus the two holes the deletion exposed, not the encoder.

## 2. What this task did

### 2.1 Eight arm bodies deleted, eight call sites left

Every module now names a profile and nothing else:

```rust
let ctx = MessageContext::default();
let messages = match encode_messages(ANTHROPIC_MESSAGES, &request.messages, &ctx) {
    Ok(messages) => messages,
    Err(loss) => return json!({ "error": loss.to_string() }),
};
```

| File | Change | Added / removed (`git diff --numstat`) |
|---|---|---|
| `anthropic.rs` | loop -> `encode_messages(ANTHROPIC_MESSAGES, ..)` | +37 / -33 |
| `bedrock.rs` (both clients) | both loops -> `encode_messages(..)`, dead helper moved out | +149 / -186 |
| `gemini.rs` | loop -> `tool_names_by_id` + `encode_messages(GEMINI, ..)` | +42 / -116 |
| `huggingface.rs` | loop -> `MessageContext { name_prefix: Some(HF_TOOL_PREFIX) }` | +47 / -123 |
| `ollama_cloud.rs` | loop -> per-message `encode_message(OLLAMA_NATIVE, ..)` | +45 / -150 |
| `openai_compat.rs` | loop -> per-message `encode_message(OPENAI_CHAT, ..)`, flag deleted | +20 / -126 |
| `openai_responses.rs` | `build_input_items` -> `encode_messages_fallible(OPENAI_RESPONSES, ..)` | +22 / -95 |
| `wire/encode.rs` | the three new entry points and the Responses refusal | +90 / -16 |

Counted across the eight files: **845 removed against 452 added**, the additions
including the `#[cfg(test)]` imports the modules' inline tests need, the new
failure arms, and the encoder's three entry points. The `"tool_result"`,
`"toolResult"`, `"functionResponse"`, `"function_call_output"` and
`"tool_call_id":` literals now occur zero times in `providers/*.rs` (the test that
scans for them is the proof).

The three call sites that need a context fact build it themselves rather than
re-deriving a shape:

- `huggingface.rs` passes `name_prefix: Some(HF_TOOL_PREFIX)`. The constant moved
  to `wire::encode` beside the encoder that applies it, so the request and the
  system-prompt rewrite cannot disagree on the rewrite.
- `gemini.rs` builds `tool_names_by_id(&request.messages)` and passes it. The old
  copy named the tool `tool_use_id.split('_').next().unwrap_or("tool")`, which is
  what FR-019 forbids and T-010 replaces - the shared encoder reads the map.
- `ollama_cloud.rs` passes `allow_images: last_user_idx == Some(idx)` per message,
  the most-recent-user-turn rule the native endpoint requires.

`openai_compat.rs` keeps `build_openai_messages`' signature and its system-prompt
packing (the dialect Copilot and Ollama share) and delegates the per-message work;
`tool_result_name` now feeds `mirror_tool_name` and the id-to-name map.

### 2.2 The two dialect switches, dead with the loops

- **`OpenAiCompatSpec::strict_single_text_collapse`.** It existed because the
  Copilot copy collapsed a single-element parts array *unconditionally*, writing
  `"content": null` when that element was an image, and losing the image. The
  shared encoder collapses a single **text** part and nothing else, which is the
  strict rule Copilot asked for, so the flag had no second value left. Copilot's
  spec keeps `openai_envelope` and `stream_options_in_base`, and
  `test_copilot_and_openai_differ_only_in_stream_options` pins that.
- **`bedrock::mime_to_converse_format`.** Moved to `wire::encode`, beside its only
  caller (`converse_message`), and now `pub`. `tests/inline/bedrock_tests.rs`
  imports it from its new home rather than pinning a private copy.

### 2.3 The refusal the Responses family owns

`build_input_items` returned `Err(RagentError::WireLoss { construct: "ImageUrl",
provider: "openai_responses" })` for an image part (FR-018). The shared encoder had
that arm as `{}` - it dropped the part - with a comment deferring the refusal to
T-007. Deleting the client's loop without carrying the arm would have turned a
refusal into a silent strip, which is the worse of the two behaviours.

Resolution: the **family that has no slot for a construct is the one that refuses
it**. `responses_items` returns `Result<_, RagentError>` and fails on an image,
because that is the only place that knows the input list's slot set. The public
surface gained:

- `encode_message(profile, msg, ctx) -> Vec<Value>` - the infallible form the six
  families that carry everything use; it is the fallible form with the report
  dropped.
- `encode_message_fallible`, `encode_messages`, `encode_messages_fallible` - the
  conversation-level form. `encode_messages` still returns `Result` so the three
  callers that must not issue a shortened request read as if they refuse.

The three body builders that cannot report an error (`build_request_body -> Value`)
write the refusal into the body as `{"error": "<loss>"}` rather than dropping a
message; the transport reports it, no history is silently shortened.

## 3. Side effect that was T-007's

The Responses path now reaches `function_call` / `function_call_output`, so the
tool history FR-018 names stops being filtered out of the input list. What FR-018
wanted was the refusal, and it is now in the encoder (section 2.3). Reasoning
replay for an `echo_reasoning` profile is still dropped silently by every family,
and that remains T-018.

## 4. Tests

`crates/ragent-llm/tests/test_wire_encode_message.rs`, **23 -> 36 cases**, in four
groups:

1. **One case per carrier** (unchanged) - the `role:"tool"` message, the
   `tool_result` block, the Converse `toolResult` block, the Gemini
   `functionResponse` part, the Responses `function_call_output` item, and the
   call side for all four.
2. **The id survives every carrier** (unchanged) - the walk over `PROVIDER_WIRE`.
3. **The absorbed dialects** (unchanged) plus two source scans in opposite
   directions.
4. **New: the conversation form and the dead switches** -
   `test_encode_messages_equals_the_per_message_calls_for_every_profile` (the
   conversation form must equal the loop it replaced, for every declared profile),
   `test_a_refused_construct_refuses_the_whole_conversation` (Responses refuses an
   image; the five other profiles accept the same history),
   `test_a_single_text_part_collapses_and_an_image_never_does` (the rule the
   Copilot flag used to choose), `test_copilot_and_openai_differ_only_in_stream_options`,
   `test_a_local_ollama_result_carries_both_the_id_and_the_name` (drives
   `build_openai_messages` with both specs, and checks the system prompt is still
   the builder's), and `test_the_encoder_refuses_a_profile_its_family_table_does_not_cover`
   (an invented envelope/carrier pair writes nothing rather than guessing).

Two cases were **rewritten**, not added, because they were the reason a red proof
went green:

- `test_tool_role_message_carries_the_id_in_the_profile_id_field` now reads the
  id through `OPENAI_CHAT.id_field` instead of indexing `["tool_call_id"]`. A test
  that names the literal tests the literal; the scan is what proves no provider
  module writes its own, and it must be able to fail.
- `test_tool_role_message_mirrors_the_tool_name_only_when_asked` lost its literal
  index for the same reason.

`test_no_provider_module_writes_a_result_carrier_literal` also gained a
`path.is_file()` check over its module list (a renamed module would have passed
the scan silently) and reports the offending **line** rather than only the file.

## 5. Red proofs

Seven, each applied, observed to fail the named case, and restored byte-identically
(`md5sum -c target/temp/t006/after.md5`; `cp -p` keeps the old mtime so every
restored file is `touch`ed - a restored file that is not touched is not rebuilt and
the *broken* object file is reused).

| # | Change | Failed |
|---|---|---|
| 1 | `wire/encode.rs`: Responses item `"function_call_output"` -> `"function_call_output_item"` | 5 cases - the two Responses carrier cases, the tool-history case, the Responses client body case, and the encoder-direction scan |
| 2 | `huggingface.rs`: `const RED_PROOF: &str = "\"tool_call_id\":";` | `test_no_provider_module_writes_a_result_carrier_literal` |
| 3 | `wire/encode.rs`: Gemini result name from `tool_use_id.split('_')` again | 4 cases - `test_gemini_function_response_names_the_tool_it_answers`, `..._prefixes_an_error...`, `test_a_gemini_result_never_recovers_a_tool_name_by_splitting_the_id`, `test_every_declared_profile_writes_its_carrier_and_keeps_the_id` |
| 4 | `openai_responses.rs`: the historic text-only filter restored | `test_the_responses_client_body_uses_the_encoder_for_its_input_list` |
| 5 | `wire/encode.rs`: Converse role filter relaxed to `msg.role.as_str()` | `test_converse_drops_a_role_it_has_no_slot_for` |
| 6 | `wire/encode.rs`: `args_json` ignoring the declared encoding | 3 cases - native Ollama, the assistant turn, the two-OpenAI-families comparison |
| 7 | `openai_compat.rs`: `mirror_tool_name` not passed | `test_a_local_ollama_result_carries_both_the_id_and_the_name` |

**Proof 2 went green twice before it could work, and it is worth recording why.**
First the injected constant was written *inside* the module but the scan reported
`still writes "tool_call_id":` for `bedrock.rs` - a stale build, because a
`cp -p` restore had left the old mtime and cargo reused the previous object file;
`touch`ing the file fixed it. Then, with the build current, the scan still passed:
cases 2's own needles were `result[0]["tool_call_id"]` literals in the *test* file,
so `grep`-style scans cannot see a constant's contents unless the scan itself is
correct - and the scan was correct. The actual defect was in the *test*: its
needle literals made it a scan of what the test asserts rather than of what the
provider writes, and once those literals were replaced by a profile-driven read
(section 4) proof 2 failed exactly as designed.

## 6. Verification

- `cargo fmt --all -- --check` - clean.
- `cargo check --workspace --all-targets` - clean.
- `cargo clippy -p ragent-llm --all-targets` - no finding in any touched file.
  The remaining warnings are pre-existing: 19 `unnecessary structure name
  repetition` in `providers/tool_cache.rs` (untouched by this task) and one
  `returning the result of a let binding` in
  `crates/ragent-storage/src/storage.rs:1709` (untouched).
- `cargo nextest run -p ragent-llm -E 'test(test_wire_encode_message::)'` -
  **36 passed** (23 before).
- `cargo nextest run -p ragent-llm -p ragent-agent` - **2732 passed, 1 failed, 3
  skipped**. The single failure is
  `test_wire_decode::test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one`
  ("`ollama_cloud.rs` still mints its own tool-call id with `_tc_{`"), which is
  T-010's and fails identically without this change.
- `cargo test -p ragent-llm --lib` - 326 passed.
- `cargo test -p ragent-llm --doc` - 30 passed (the `encode_message` example).

No dependency added (NFR-002), no `unsafe` (NFR-003), no tool renamed, tool
surface unchanged.

## 7. Gotchas recorded for the next task

- **A landed behaviour test that is red at the start is the task, not a
  blocker.** `test_no_provider_module_writes_a_result_carrier_literal` failed on
  the first run and named the file to convert.
- **`cargo fix` removes imports an inline test needs.** Five modules had
  `ChatContent` / `ContentPart` imports that only `tests/inline/*.rs` used (via
  `#[path]` inclusion into the module tree). `cargo fix --lib` deleted them and
  every inline test stopped compiling. They are now `#[cfg(test)] use` lines in
  those modules: a module whose *tests* use a type keeps the import, under `cfg`.
- **`#[expect(dead_code)]` on a test-only helper is a lie when the test file is
  not compiled in the current configuration** - it was tried for the restored
  `ContentPart` and removed, because `#[cfg(test)]` expresses the same thing
  without depending on which test targets are built.
- **`cp -p` restore + no `touch` = stale build.** Three quarters of an hour was
  spent on a proof that "did not fail" while cargo was reusing the previous
  object file.

# T-006 - One profile-driven `encode_message`, four result carriers, seven loops gone

Spec `uniformtools`, task T-006. Requirement FR-006 (the active provider's encoder
emits the carrier its profile declares, with the id byte-identical to the one the
model returned).

## 1. What was there

Eight copies of the same loop, in seven files:

| File | Arms | What the copy wrote |
|---|---|---|
| `anthropic.rs` | 1 | `role`/`content` blocks: `tool_use`, `tool_result` |
| `bedrock.rs` (`BedrockAnthropicClient`) | 1 | the **same** Anthropic blocks, second copy |
| `bedrock.rs` (`BedrockConverseClient`) | 1 | `toolUse`, `toolResult`, plus its own role filter |
| `gemini.rs` | 1 | `contents[].parts[]`: `functionCall`, `functionResponse` |
| `openai_compat.rs` | 1 | `tool_calls`, `role:"tool"` keyed by `tool_call_id` |
| `huggingface.rs` | 1 | the OpenAI copy with `t_` names and no `tool_name` |
| `ollama_cloud.rs` | 1 | object arguments, `role:"tool"` keyed by `name` |
| `openai_responses.rs` | 1 | a text-only filter that dropped the whole tool exchange |

The four carriers FR-006 names therefore existed in eight places, and they did not
agree: only three of the eight read the profile anything was supposed to be
dispatching on, and two of them (`bedrock`'s Messages path, `huggingface`) were
byte-for-byte copies of a neighbour that had already been edited once. The
Responses copy is FR-018's subject and is the reason the requirement is phrased as
"emit the carrier its profile declares" rather than "carry the result".

## 2. What replaced it

`crates/ragent-llm/src/providers/wire/encode.rs` gained the one message encoder
beside the one tool encoder:

```rust
pub fn encode_message(
    profile: ToolWireProfile,
    msg: &ChatMessage,
    ctx: &MessageContext<'_>,
) -> Vec<serde_json::Value>
```

- **The carrier comes from the profile.** `ResultCarrier`, `IdField` and
  `ArgsEncoding` are read, never restated. The id is written into exactly the key
  `id_field` names, and `ArgsEncoding::JsonString` escapes the arguments into a
  string while `JsonObject` embeds them - one arm, not two copies.
- **`MessageFamily::of(profile)` derives the vocabulary.** A `match` on
  `(tool_envelope, result_carrier, args_encoding)` resolves to one of six
  vocabularies, so a caller cannot name a message shape that disagrees with its own
  declared contract; a profile combination no family implements resolves to `None`
  and the encoder logs an error rather than guessing. Two of the six exist because
  the *same* carrier name is written two ways: `UserToolResultBlock` is Anthropic's
  `tool_result` and Converse's `toolResult`, and `ToolRoleMessage` is OpenAI's
  id-keyed message and native Ollama's name-keyed one. Telling them apart is the
  profile's job, so it is one derivation here rather than a sixth `ResultCarrier`
  variant the requirement does not name.
- **`MessageContext` carries only what a profile cannot.** Four facts:
  `tool_names` (the id-to-name map, for the two carriers that name a tool),
  `name_prefix` (HuggingFace's `t_`, the same rewrite `encode_tools` takes),
  `allow_images` (native Ollama's most-recent-user-turn-only rule), and
  `mirror_tool_name` (local Ollama's request for both `tool_call_id` and
  `tool_name`). Everything else is read from the profile, so a caller passing every
  field is still not making a wire decision.
- **`tool_names_by_id(messages)` is the one scan.** Two carriers identify a result
  by *naming* the tool, and neither provider returns the name on the result frame,
  so the name has to be recovered from the assistant turn that opened the call.
  That scan existed nowhere before because both call sites derived the name from
  the id instead (`tool_use_id.split('_').next()` in `gemini.rs`) - which is
  precisely what FR-019 forbids and T-010 replaces.

Every provider module now calls it:

```rust
let ctx = MessageContext::default();
let messages: Vec<Value> = request
    .messages
    .iter()
    .flat_map(|msg| encode_message(ANTHROPIC_MESSAGES, msg, &ctx))
    .collect();
```

`openai_compat::build_openai_messages` keeps its signature and its system-prompt
packing - that is a *dialect* (Copilot, Ollama) rather than a wire contract - and
delegates the per-message work.

## 3. Two dialect switches that died with the loops

- **`OpenAiCompatSpec::strict_single_text_collapse`** existed because the Copilot
  copy collapsed a single-element parts array unconditionally, which writes
  `"content": null` when that element is an image and silently loses it. The shared
  encoder collapses a single *text* part and nothing else, so the strict rule is the
  only rule and the flag has no second value to take. Copilot's spec keeps only
  `openai_envelope`, `stream_options_in_base` and `tool_result_name`.
- **`bedrock::mime_to_converse_format`** moved to `wire::encode`, beside its only
  caller, and is now `pub` so the case that pinned it can follow it rather than
  pinning a private copy (`tests/inline/bedrock_tests.rs` imports it from its new
  home).

## 4. Side effect that is T-007's, not this task's

The Responses path now reaches the `function_call` / `function_call_output`
carriers, so the tool history FR-018 names stops being filtered out of the input
list. What FR-018 still wants is the *refusal*: a construct this profile cannot
carry (an image part; a reasoning part for a profile that echoes it) is still
dropped silently, and turning that drop into `Err(WireLoss { construct, provider })`
is T-007. The tests here assert presence, not policy, and say so.

## 5. Tests

`crates/ragent-llm/tests/test_wire_encode_message.rs`, 23 cases in three groups:

1. **One case per carrier** - the `role:"tool"` message (and the mirrored
   `tool_name` only when asked), the `tool_result` block, the Converse
   `toolResult` block, the Gemini `functionResponse` part, the Responses
   `function_call_output` item, and the call side for all four.
2. **The id survives every carrier** - `test_every_declared_profile_writes_its_carrier_and_keeps_the_id`
   walks `PROVIDER_WIRE`, deduplicates by profile, and asserts for each declared
   profile that (a) a tool result reaches the wire at all, (b) the carrier is the
   one the profile names, and (c) the id is present exactly once under the key
   `id_field` names - or absent for a `Synthesised` profile, which is the one case
   FR-006 exempts by naming the tool instead.
   `test_the_id_on_a_carrier_is_byte_identical_to_the_id_on_the_call` states the
   same property as an equality with an awkward id rather than a shape assertion.
3. **The absorbed dialects** - the Converse role policy, the `t_` prefix applying to
   the name and nothing else, native Ollama's object arguments and name-keyed
   result, the data-URI strip plus the image rule, and the Responses input list
   carrying both halves of the tool exchange. Plus a case that drives
   `ResponsesApiClient::build_request_body` itself, so the client is proven to reach
   the encoder rather than only the encoder being proven to exist.

Two source scans in opposite directions close the structural half:

- `test_no_provider_module_writes_a_result_carrier_literal` - no provider module may
  hold `"tool_result"`, `"toolResult"`, `"functionResponse"`,
  `"function_call_output"` or `"tool_call_id":`.
- `test_the_encoder_is_the_only_place_the_carriers_are_written` - the encoder must
  contain all of them, so the first scan cannot be satisfied by deletion.

## 6. Red proofs

Six, each applied, observed to fail the named case, and restored byte-identically
(`md5sum -c target/temp/t006/before.md5`; `cp -p` leaves the old mtime, so the
restored file is `touch`ed or cargo reuses the broken build's object file):

| Proof | Change | Failed |
|---|---|---|
| 1 | `wire/encode.rs`: `"tool_result"` -> `"tool_result_block"` | `test_user_tool_result_block_wraps_the_result_in_a_user_turn` (the other 22 stayed green, so the source scan needed the second direction added) |
| 2 | `huggingface.rs`: `let _red_proof = r#""tool_call_id":"#;` | `test_no_provider_module_writes_a_result_carrier_literal` |
| 3 | `wire/encode.rs`: Gemini result name derived from the id again, ignoring the map | `test_gemini_function_response_names_the_tool_it_answers`, `test_every_declared_profile_writes_its_carrier_and_keeps_the_id` |
| 4 | `openai_responses.rs`: the historic text-only filter restored | `test_the_responses_client_body_uses_the_encoder_for_its_input_list` |
| 5 | `wire/encode.rs`: `Converse` role filter relaxed to `msg.role.as_str()` | `test_converse_drops_a_role_it_has_no_slot_for` |
| 6 | `wire/encode.rs`: `args_json` ignoring the declared encoding | three cases - `test_native_ollama_...`, `test_the_assistant_turn_...`, `test_the_two_openai_families_differ_...` |

Proof 1 is the one worth recording: it was **green at first**, because the failing
case only appeared once the source scans were added, and the string it broke was the
one the doctest and the scan both named. That is why the two scans are in opposite
directions.

## 7. Verification

- `cargo fmt -p ragent-llm -- --check` - clean.
- `cargo check --workspace --all-targets` - clean.
- `cargo clippy -p ragent-llm --all-targets` - no new warnings (two in the new test
  file were fixed: `needless_collect` + `assert_is_empty`).
- `cargo test -p ragent-llm --test all` - 277 passed, 2 ignored (254 before; +23).
- `cargo test -p ragent-llm --lib` - 324 passed.
- `cargo test -p ragent-llm --doc` - 30 passed (the new `encode_message` example).
- `cargo test --test all` (root crate) - 48 passed.
- The eight guard scripts pass, `check-inline-tests` included.

No dependency added (NFR-002), no `unsafe` (NFR-003), no tool renamed, tool surface
unchanged at 148 visible tools / 21 families.

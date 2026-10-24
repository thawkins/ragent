# T-005 - The Responses API joins the one encoder

Spec `uniformtools`, task T-005. Requirement FR-003 (one tool-definition encoder, a
cache hit byte-identical to a miss, and no provider constructing its own tool array).
Status: completed. Dependency: T-004 (the fold that created `encode_tools`).

## 1. What was there

`crates/ragent-llm/src/providers/openai_responses.rs`, inside
`ResponsesApiClient::build_request_body`:

```rust
// Build tools array
let tools: Vec<Value> = request
    .tools
    .iter()
    .map(|tool| {
        json!({
            "type": "function",
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters,
        })
    })
    .collect();

if !tools.is_empty() {
    body["tools"] = Value::Array(tools);
}
```

FR-003's second test clause is "no provider module contains a hand-built
`"parameters"` / `"input_schema"` / `"toolSpec"` JSON literal outside the encoder", and
this was that literal. It also re-derived the flattening rule - the Responses item has
`type`, `name`, `description` and `parameters` side by side, with no nested `function`
object - which is a *different* item shape from the OpenAI chat envelope that
`openai_compat.rs` sends.

The shape itself was not wrong. What was wrong is that it had two definitions: one in
`openai_responses.rs` (used at run time) and one in `wire/encode.rs`'s
`ResponsesFlatFunction` arm (used only by tests, because no `ToolFormat` resolved to
`OPENAI_RESPONSES`). T-004 pinned the encoder's arm *before* this task precisely so the
encoder's shape was a recorded contract rather than a description of the module this
task deletes, and it named the missing cache key as the one documented exception in
`test_every_profile_a_provider_can_serve_is_reachable_through_the_cache`.

## 2. What changed

### 2.1 `ToolFormat::OpenAiResponses` (the cache key)

`crates/ragent-llm/src/providers/tool_cache.rs`:

| Item | Why |
|---|---|
| `ToolFormat::OpenAiResponses` | a sixth cache key; `format_profile` resolves it to `OPENAI_RESPONSES` with no name prefix |
| `ToolFormat::ALL: [Self; 6]` | an exhaustive array, so a caller that must consider every key cannot forget one: adding a variant without extending it is a compile error |
| `ToolFormat::tool_envelope(self)` | reads the envelope out of the *same* `format_profile` the encoder uses, so a test can ask what shape a key produces without encoding anything and cannot drift from it |

The variant is not a second encoding rule. It is a cache key, exactly like the other
five: it says *which already-serialised buffer* a caller wants, and the encoding comes
from `OPENAI_RESPONSES.tool_envelope` through `encode_tools`. The two OpenAI-shaped
keys (`OpenAi` and `OpenAiResponses`) therefore differ only in which profile they name -
and that is a real difference, because the item shapes differ (`{"type","function":{…}}`
versus a flattened `{"type","name",…}`) while the payload *slice* the caller assigns is
the same inner array.

### 2.2 `CachedTools::responses_tools_array`

`crates/ragent-llm/src/providers/wire/encode.rs`. It returns the same slice as
`openai_tools_array` because both families assign the inner array to `body["tools"]`,
and it exists as a *named* accessor because the caller asks for a shape, not for an
offset: the encoder decides which bytes a named shape occupies. Without it the
Responses call site would have read `openai_tools_array()` and the next reader would
have to re-derive that the item shapes still differ.

### 2.3 The call site

```rust
if !request.tools.is_empty() {
    // FR-003: the tool array comes from the one encoder, through the same
    // cache every other family uses. ...
    let cached = cached_tools(ToolFormat::OpenAiResponses, &request.tools);
    body["tools"] = cached.responses_tools_array();
}
```

Same guard as before (an empty list stays *absent* rather than becoming `"tools":[]`,
which some Responses deployments reject), same body position, same cache discipline as
the five families that already used it.

### 2.4 The TUI's envelope accounting

`crates/ragent-tui/src/app/session_ops.rs`, `tool_format_for_provider` gained:

```rust
"openai_responses" => ToolFormat::OpenAiResponses,
```

This is not cosmetic. `tool_metadata_token_count` measures the *selected provider's*
per-tool envelope overhead as `cached_tools(format, &defs).byte_len` minus the raw
definition bytes, so a provider id that resolves to the wrong key reports an overhead
the session never pays. Before the arm existed a Responses session was measured with the
chat envelope: 532 tokens against 532 - identical, because both sides of the
subtraction had been taken from the chat buffer.

## 3. What deliberately did not change

- **The wire bytes.** The Responses item shape is `OPENAI_RESPONSES.tool_envelope`'s
  arm, which T-004 pinned as `{"tools":[{"description":"d","name":"Read","parameters":{},"type":"function"}]}`
  in `test_encode_tools_bytes_match_the_pre_fold_builders`. That case still passes, so
  the payload this task moves is byte-identical to the one the inline array produced.
- **`cached_tools`, `invalidate_tool_cache`, the fingerprint, the locking discipline.**
  Untouched; the new key only widens the key space.
- **Every other provider.** `openai_responses.rs` was the last module under
  `providers/` that built its own array. `openai_compat.rs` and `huggingface.rs` still
  contain a `"type": "function"` literal, but it is the *tool-call* item in an assistant
  message (`{"id","type","function":{"name","arguments"}}`), not a definition
  envelope - FR-006's territory, which is T-006.
- **`ToolFormat` as an enum.** Deleting it in favour of `ToolWireProfile` would have
  made the process-global cache's key a nine-field struct and changed the TUI's
  argument. T-004 already recorded why it survives.

## 4. Proof

### 4.1 New and extended cases

| File | Case | Asserts |
|---|---|---|
| `tests/test_wire_encode.rs` | `test_responses_provider_asks_the_encoder_for_its_tool_array` (new) | `openai_responses.rs` contains neither `"parameters":` nor `"type": "function"`, **and** does call `cached_tools(ToolFormat::OpenAiResponses` - the scan is two-sided so deleting the array without wiring the encoder also fails |
| | `test_folded_provider_modules_no_longer_write_an_envelope_literal` (extended) | `openai_responses.rs` joins the six-module scan for `"input_schema"` / `"toolSpec"` / `"functionDeclarations"` |
| | `test_cached_tools_hit_equals_a_fresh_encode_for_every_format` (extended) | all six keys, cold and warm, equal a fresh `encode_tools` byte for byte |
| | `test_every_profile_a_provider_can_serve_is_reachable_through_the_cache` (rewritten) | now **total and two-directional**: every `PROVIDER_WIRE` envelope has a cache key, and every cache key's envelope is one `PROVIDER_WIRE` declares. Driven by `ToolFormat::ALL` + `tool_envelope`, so the case cannot drift from the mapping it checks |
| | `test_encode_tools_empty_list_is_an_empty_valid_payload` (extended) | the Responses accessor on the empty list, and the OpenAI arm split from the Responses arm so the two shapes are named separately |
| `tests/inline/tool_cache_tests.rs` | `responses_and_chat_are_separate_cache_entries_over_the_same_payload_slice` (new) | distinct `Arc`s, distinct bytes, each `byte_len` equal to its own buffer minus the 10-byte wrapper prefix - and the Responses item carries `name` with no nested `function` |
| | `every_format_maps_to_a_distinct_profile_and_prefix_pair` (extended) | `OpenAiResponses -> (OPENAI_RESPONSES, None)` |
| | `cached_tools_hit_is_byte_identical_to_a_fresh_encode`, `empty_tool_list_is_a_valid_empty_payload` (extended) | iterate `ToolFormat::ALL`, so a seventh key is covered the moment it joins the array |
| `tests/inline/openai_responses_tests.rs` | `test_build_request_body_tools_come_from_the_shared_encoder` (new) | the body's `tools` value **equals** the encoder's payload - not merely "is an array of the right length" - plus the flattened item's `type`/`name` and the absence of `function` |
| | `test_build_request_body_omits_tools_when_the_list_is_empty` (new) | pinning the guard the old code had, so "route it through the encoder" did not quietly add `"tools":[]` |
| `crates/ragent-tui/tests/test_context_panel_t005.rs` | `test_tool_metadata_measures_the_selected_providers_own_envelope` (new) | a Responses session's metadata count is strictly smaller than a chat session's over the same 148 definitions |

The equality in `test_build_request_body_tools_come_from_the_shared_encoder` is the
case that matters. "Agrees today" is not "derived": an inline array that produced the
right shape would satisfy every shape assertion and still be the state FR-003 forbids.
Comparing against `cached_tools(ToolFormat::OpenAiResponses, …)` makes the request body
and the encoder the same value, so only the derivation can pass.

### 4.2 Red proofs

Four, each applied, observed, and reverted byte-identically - `md5sum -c` against
`target/temp/t005/before.md5` (three files, the exact bytes recorded before the first
break) after every restore.

| Break | Failing cases |
|---|---|
| Restore the inline `Vec<Value>` of `json!` items in `openai_responses.rs` | `test_responses_provider_asks_the_encoder_for_its_tool_array` - `openai_responses.rs writes the flattened function item literal "parameters":` |
| Delete the TUI's `"openai_responses" => ToolFormat::OpenAiResponses` arm | `test_tool_metadata_measures_the_selected_providers_own_envelope` - `chat=532, responses=532` |
| Point `ToolFormat::OpenAiResponses` at `OPENAI_CHAT` | `every_format_maps_to_a_distinct_profile_and_prefix_pair`, `test_cached_tools_hit_equals_a_fresh_encode_for_every_format`, `test_every_profile_a_provider_can_serve_is_reachable_through_the_cache` |
| Keep the encoder call but re-wrap its payload into a nested `function` object before assignment | `test_build_request_body_tools_come_from_the_shared_encoder` |

The fourth is the interesting one. The source scan passes (there is no `json!` literal),
the encoder is genuinely called, and the cache is genuinely used - and the wire is
still wrong, because the Responses item is flattened. It is the assertion that ties the
*request body* to the *encoder output*, not to a shape description, that catches it.

Restoring from backup needed `touch`, exactly as the memory of T-022 recorded: `cp -p`
leaves the restored file's older mtime in place and cargo then reuses the broken build's
object file, so the "restored" tree still fails and the proof looks unrevertable.

## 5. Measurements

| Measure | Before | After |
|---|---|---|
| provider modules that build a tool-definition array | 2 (`tool_cache.rs`, `openai_responses.rs`) | 0 (both now call `wire::encode::encode_tools`) |
| `ToolFormat` variants | 5 | 6 |
| cache keys with no caller | 0 | 0 (asserted both ways) |
| `openai_responses.rs` lines | 657 | 649 |
| `tool_cache.rs` lines | 165 (post-T-004) | 205 |
| `PROVIDER_WIRE` rows whose envelope no cache key serves | 1 (`openai_responses`, documented exception) | 0 |

Verification, all green on this tree:

```
cargo fmt --all -- --check                        clean
cargo check --workspace --all-targets             clean
cargo clippy -p ragent-llm -p ragent-tui --all-targets
                                                  no new warning in a touched file
cargo test -p ragent-llm --lib                    324 passed
cargo test -p ragent-llm --test all               254 passed, 2 ignored
cargo test -p ragent-llm --doc                    29 passed
cargo test -p ragent-tui --test all               3705 passed
ragent run --dry-run "x"                          148 visible tools, 20 families
scripts/check-inline-tests.sh                     PASS
scripts/check-test-env-isolation.sh               PASS
scripts/check-silent-errors.sh                    PASS
scripts/check-security-unwraps.sh                 PASS
scripts/check-poison-locks.sh                     PASS
scripts/check-shared-guards.sh                    PASS
scripts/check-dead-code-reasons.sh                PASS
scripts/check-file-tool-containment.sh            PASS
```

No dependency added (NFR-002). No `unsafe` block (NFR-003). No tool renamed and the
tool surface is unchanged at 148 across 21 families (FR-030).

## 6. What is left to the next task

- **T-027** owns the full-tree version of the envelope-literal scan: every module under
  `providers/`, every envelope key. This task's scan is two-sided but still scoped to
  the six modules it and T-004 touched, and it scans for *definition* keys only, so the
  tool-*call* `"type": "function"` literal in `openai_compat.rs` and `huggingface.rs` is
  deliberately outside it.
- **T-006** takes the same encoder to the tool *result* carriers. `CachedTools` is
  definitions-only and stays that way; the `"type": "function"` literals inside assistant
  `tool_calls` arrays are that task's subject.
- **T-007** replaces the Responses path's text-only part filter (divergence 2 in
  `SPEC.md`) with a real tool-item encoding path; this task touched only the *definition*
  half of `build_request_body`.

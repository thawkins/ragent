# T-004 - One profile-driven `encode_tools`, one unchanged cache

Spec `uniformtools`, task T-004. Requirements FR-003 (one tool-definition encoder,
cache hit byte-identical to a miss) and NFR-001 (no per-step re-serialisation).

## 1. What was there

At `1.0.149` (`d172b658`), `crates/ragent-llm/src/providers/tool_cache.rs` was 373
lines: a byte cache plus, inside it, five near-identical builders.

```
build_openai        -> {"type":"function","function":{name,description,parameters}}
build_anthropic     -> {"name","description","input_schema"}
build_gemini        -> {"functionDeclarations":[{name,description,parameters}]}
build_bedrock       -> {"toolSpec":{name,description,inputSchema:{json:...}}}
build_huggingface   -> the OpenAI envelope with every name prefixed "t_"
```

Each repeated the same nine lines of framing: reserve, write `{"tools":[`, loop with
`push_tool`, write `]}`. Each re-derived its own `payload_start` / `payload_end` pair,
and three of them recomputed the same `buf.len() - 1` while a fourth hard-coded `9`.
Bedrock alone framed differently - no payload offsets, the whole buffer assigned - which
is the one real difference between the families and was expressed four different ways.

The duplication mattered because FR-003 is not "one function exists" but "a provider
shall not construct its own tool array": five builders is five places a new envelope can
be added, and four of them never mention the profile that is supposed to define the
shape. `ToolFormat::format_profile` did not exist, so the builders and the profile table
were two independent statements of the same mapping.

## 2. What replaced it

New `crates/ragent-llm/src/providers/wire/encode.rs` (1072 lines including the message
encoders T-006 adds), holding the one definition encoder and the one `CachedTools` type:

- `encode_tools(profile, name_prefix, tools) -> CachedTools` - frames the list once,
  then builds each item from `profile.tool_envelope`.
- `item_json(envelope, tool, name)` - the **only** definition of the five item shapes in
  the workspace, one `match` arm each.
- `push_tool` - the comma separator and the log-and-skip on a serialisation failure,
  kept verbatim from the old shared helper.
- `CachedTools` and its accessors - moved without a change to their bodies or their
  contract.

`tool_cache.rs` is now 198 lines: `ToolFormat`, the key, the two public functions and
`tool_arguments_json`. It re-exports `CachedTools` (`pub use crate::provider::wire::
encode::CachedTools`), so the historic path still names the type and no caller changed.

The one real *difference* between the families is which slice of the buffer the caller
keeps:

| Envelope | Caller assigns | Payload |
|---|---|---|
| `BedrockToolSpec` | `body["toolConfig"]` | the whole `{"tools":[...]}` object |
| the other four | `body["tools"]` | the inner `[...]` array |

That is one arm of a `match` on `profile.tool_envelope` in `encode_tools`, not a
per-provider builder:

```rust
let (payload_start, payload_end) = match profile.tool_envelope {
    ToolEnvelope::BedrockToolSpec => (0, buf.len()),
    ToolEnvelope::OpenAiFunction
    | ToolEnvelope::AnthropicTool
    | ToolEnvelope::GeminiFunctionDeclaration
    | ToolEnvelope::ResponsesFlatFunction => (array_start, buf.len() - 1),
};
```

It is also why `payload_start` / `payload_end` stay private fields rather than a public
flag: the caller asks for a shape through an accessor named for the shape, and the
encoder decides the offsets. `byte_len` (which request-size estimation and the TUI read)
is `payload_end - payload_start`, so the one expression serves both jobs.

## 3. What deliberately did not change

FR-003 names the cache, not just the encoder, so the fold stops at the encoder's edge.

- **`cached_tools(format, tools) -> Arc<CachedTools>`** - same signature, same body.
  All eight call sites in `crates/*/src` compile unchanged: `anthropic.rs:316`,
  `bedrock.rs:520` and `:919`, `gemini.rs:531`, `huggingface.rs:455`,
  `openai_compat.rs:284`, `openai_responses.rs:385` (which T-005 routes) and the TUI's
  `app/session_ops.rs:421`.
- **`invalidate_tool_cache()`** - same body: lock, `clear`. The invalidation path from
  `SessionProcessor::invalidate_tool_cache` is untouched.
- **`CachedTools`** - re-exported from `tool_cache`, so the existing import path works.
  The four accessors FR-003's call sites use keep their names and semantics
  (`openai_tools_array`, `anthropic_tools_array`, `gemini_tools_array`,
  `bedrock_tool_config_object`); `responses_tools_array` joined them in T-005.
- **The key** - `(ToolFormat, fingerprint)` where the fingerprint hashes name,
  description and parameters in that order. Not "equivalent to" the old one: the same
  function, so a cache built before the fold and one built after agree.
- **The locking discipline** - lookup under the lock, encode *outside* it, then re-check
  and insert under a second acquisition. Concurrent misses still do not contend and the
  first insert still wins.
- **`tool_arguments_json`** - moved, not changed. It is a *decode-side* helper (FR-007)
  and T-012's test asserts its two properties by reading this file, so moving it would
  have broken T-012 for no benefit.

`ToolFormat` also survives, and the reason is worth recording because the obvious
reading of "fold the builders into `encode_tools`" is that the enum disappears:

- it is the **cache key**, and the cache is process-global and shared across providers,
  so the key must distinguish six buffers over the *same* definitions;
- it is what the TUI passes (`tool_metadata_token_count`), because the TUI knows a
  provider id, not a profile;
- it carries no encoding rule. It resolves to `(profile, name_prefix)` through
  `format_profile` and `name_prefix`, and the envelope is read from the profile via
  `tool_envelope`. So the enum is a lookup argument, not a second source of truth: a new
  envelope is added to `ToolEnvelope`, to `format_profile` and to nothing else.

The HuggingFace prefix is the one place the fold had to make a judgement call. It is not
a different envelope - it is the OpenAI envelope with a name rewrite - so making it a
sixth `ToolEnvelope` constant would have split one family's contract in two (FR-002 says
a family declares *one* profile). It is therefore a `name_prefix` argument:
`Option<&str>`, applied to the name alone, `Cow::Borrowed` when there is no prefix so the
common path does not allocate. `HF_TOOL_PREFIX` moved into `encode.rs` with it - the
prefix is a wire-name rule - and `huggingface.rs` imports it back because the *strip* on
the reply path is that module's.

## 4. Proof

### 4.1 `crates/ragent-llm/tests/test_wire_encode.rs` - 13 cases

Twelve belong to this task, one was added by T-005 and is listed for completeness.

| Case | Asserts |
|---|---|
| `test_encode_tools_openai_function_envelope` | `type`/`function` nesting, name reached the wire, schema reached the wire, no flattened `name` |
| `test_encode_tools_anthropic_tool_envelope` | `name`/`description`/`input_schema`, and that there is no `type` or `function` wrapper |
| `test_encode_tools_gemini_function_declaration_envelope` | one `functionDeclarations` wrapper per tool, one declaration inside it, schema under `parameters` |
| `test_encode_tools_bedrock_tool_spec_envelope` | `toolSpec` items under `tools`, schema under `inputSchema.json` |
| `test_encode_tools_responses_flat_function_envelope` | flattened item: `type` and `name` side by side, `parameters` at the top level, no `function` object |
| `test_encode_tools_name_prefix_applies_to_the_name_only` | the prefix reaches the name and **not** the description or the schema |
| `test_encode_tools_is_deterministic_across_calls` | same bytes on a second call, after encoding an unrelated profile in between |
| `test_encode_tools_empty_list_is_an_empty_valid_payload` | every profile produces a parseable empty list, not `Value::Null` |
| `test_cached_tools_hit_equals_a_fresh_encode_for_every_format` | FR-003's literal clause: for every key, `cached_tools(f, t).bytes == encode_tools(profile, prefix, t).bytes` on the miss **and** on the hit |
| `test_every_profile_a_provider_can_serve_is_reachable_through_the_cache` | every `PROVIDER_WIRE` row resolves to an envelope some `ToolFormat` can ask for, and no key resolves to an envelope no provider declares - both directions |
| `test_folded_provider_modules_no_longer_write_an_envelope_literal` | none of the five folded modules contains `"input_schema"`, `"toolSpec"` or `"functionDeclarations"` |
| `test_encode_tools_bytes_match_the_pre_fold_builders` | the exact pre-fold byte strings, all five builders plus the HuggingFace-prefixed sixth row, captured from the originals |
| `test_responses_provider_asks_the_encoder_for_its_tool_array` | T-005: `openai_responses.rs` holds neither flattened-item literal and does call `cached_tools(ToolFormat::OpenAiResponses` |

The byte case is the behaviour-preservation proof: a refactor that changes key order,
adds a field, or drops the comma separator fails on a literal rather than in a provider
request at run time. It is safe to pin because `serde_json`'s default map is sorted (no
`preserve_order` feature anywhere in the workspace), so the bytes do not depend on how a
`json!` literal is written:

```
openai      {"tools":[{"function":{"description":"d","name":"Read","parameters":{}},"type":"function"}]}
anthropic   {"tools":[{"description":"d","input_schema":{},"name":"Read"}]}
gemini      {"tools":[{"functionDeclarations":[{"description":"d","name":"Read","parameters":{}}]}]}
bedrock     {"tools":[{"toolSpec":{"description":"d","inputSchema":{"json":{}},"name":"Read"}}]}
responses   {"tools":[{"description":"d","name":"Read","parameters":{},"type":"function"}]}
huggingface {"tools":[{"function":{"description":"d","name":"t_Read","parameters":{}},"type":"function"}]}
```

The Responses envelope was pinned here *before* T-005 routed `openai_responses.rs`
through the encoder, so the shape is recorded as a contract rather than inferred from the
module that task deletes.

### 4.2 `crates/ragent-llm/tests/inline/tool_cache_tests.rs` - 8 cases

The cache's own behaviour. The file at `HEAD` had nine cases, each of which re-asserted
one envelope's *shape* against the encoder - work `test_wire_encode.rs` already does with
more precision - and **none** of which tested the cache's key, its invalidation, or the
slice it hands a caller. Reworked so the cases test what the fold actually created a new
seam in, and so the shape checks that stay collapse into one loop over `ToolFormat::ALL`:

- `every_format_maps_to_a_distinct_profile_and_prefix_pair` - the whole mapping in one
  assertion (profile **and** prefix per key, pair uniqueness, `ALL` total), so
  `OpenAi`/`HuggingFace` sharing a profile but differing by a prefix is visible rather
  than implied; then the same loop checks each key's emitted item really is its declared
  envelope and that the name rewrite landed on the name only.
- `cached_tools_reuses_same_buffer` - the second call over the same tools returns the
  same `Arc`, not a re-serialised equal one.
- `cached_tools_key_separates_formats_with_the_same_tools` - six keys over one slice:
  every pair gets a distinct `Arc`, and a pair with a different envelope also gets
  different bytes.
- `cached_tools_key_tracks_the_tool_content` - name, description and parameters each
  changed **separately** (a fingerprint over the name alone would pass a test that
  changed all three at once), plus the unchanged definitions still hitting the original.
- `invalidate_tool_cache_drops_every_entry` - after the call every entry rebuilds as a
  new `Arc` with identical bytes and `byte_len`, and a second invalidation is harmless.
- `empty_tool_list_is_a_valid_empty_payload` - an empty registry serialises to an empty
  list on every key, never to `Null`.
- `byte_len_counts_the_payload_the_caller_assigns` - `byte_len` equals the length of the
  serialised payload the caller assigns, so it is strictly smaller than `bytes` for the
  four array envelopes and equal to it for Bedrock.
- `cached_tools_hit_is_byte_identical_to_a_fresh_encode` - FR-003's literal clause on
  the miss and on the hit for every key, against `encode_tools` with the pair the key
  resolves to, plus `byte_len`.

All eight take one file-local `cache_lock()`, and that was measured rather than assumed.
With the eight lock acquisitions deleted and the suite run eight times, four runs failed
with 2-3 of the eight red:

```
FAILED. 6 passed; 2 failed    FAILED. 6 passed; 2 failed
FAILED. 5 passed; 3 failed    FAILED. 6 passed; 2 failed
FAILED. 6 passed; 2 failed    FAILED. 5 passed; 3 failed
FAILED. 5 passed; 3 failed    FAILED. 5 passed; 3 failed
```

With the lock restored, five consecutive `providers::tool_cache` runs and a full
`--lib` run at `--test-threads=32` are all green. One binary per crate means the cache is
shared by every test in the binary, so they must contend on one mutex. A file-local one
suffices because these are the only tests that assert on cache *identity or emptiness*;
`providers::bedrock::tests::test_converse_tool_config` also reads the map, but only for
shape, so an interleaved clear merely makes it a miss - and by FR-003 a miss is
byte-identical to the hit it would have received. That reasoning is written into the
file's module docs so the lock is not "simplified" away.

### 4.3 Red proofs

Six, each applied, observed, and reverted byte-identically. `md5sum -c
target/temp/t004/before.md5` verified the restore of every touched file, and each restore
was `touch`ed (`cp -p` preserves the old mtime, so cargo would otherwise reuse the broken
object).

| Break | Failing cases |
|---|---|
| `encode.rs` Anthropic arm: `input_schema` -> `inputSchema` | `test_encode_tools_anthropic_tool_envelope`, `test_encode_tools_bytes_match_the_pre_fold_builders` (**test binary**) and, because the shape loop reads the emitted item, `every_format_maps_to_a_distinct_profile_and_prefix_pair` (**lib binary**) |
| `gemini.rs`: `body["tools"] = cached.gemini_tools_array()` -> a hand-built `json!([{"functionDeclarations": []}])` | `test_folded_provider_modules_no_longer_write_an_envelope_literal` |
| `tool_cache.rs`: key becomes `(ToolFormat::OpenAi, fingerprint)` - the format dropped | `cached_tools_key_separates_formats_with_the_same_tools` (`OpenAi and Anthropic share one cache entry, so the format is not in the key`), `every_format_maps_to_a_distinct_profile_and_prefix_pair`, `empty_tool_list_is_a_valid_empty_payload`, `byte_len_counts_the_payload_the_caller_assigns` |
| `tool_cache.rs`: `t.description.hash(&mut hasher)` deleted from the fingerprint | `cached_tools_key_tracks_the_tool_content` (`changing the description did not change the cache key`) |
| `tool_cache.rs`: `invalidate_tool_cache()` body deleted | `invalidate_tool_cache_drops_every_entry` (`OpenAi: the entry survived invalidate_tool_cache`) |
| `tool_cache.rs`: `ToolFormat::HuggingFace => Some(HF_TOOL_PREFIX)` -> `None` | `every_format_maps_to_a_distinct_profile_and_prefix_pair` (`HuggingFace resolves to the wrong name prefix`) |

The second is the one that shows the source scan is a guard rather than decoration: it
fails on a *caller* re-deriving the envelope, which is exactly the state FR-003 forbids.
The third is the one the file needed most: the pre-existing suite could not fail on a
dropped format component, because it never called two formats over one tool slice.

## 5. Measurements

| Measure | Before (`1.0.149`) | After |
|---|---|---|
| `tool_cache.rs` lines | 373 | 198 |
| tool-envelope builders | 5 | 0 |
| modules under `providers/` that build a tool envelope | 2 (`tool_cache.rs`, `openai_responses.rs`) | 0 (T-005 routed the second) |
| `ToolFormat` variants | 5 | 6 (`OpenAiResponses`, T-005) |
| `cached_tools` call sites in `crates/*/src` | 6 (`1.0.149`) / 8 (now, incl. Responses and the TUI) | 8, unchanged by this task |
| public API items | `ToolFormat`, `CachedTools` + 4 accessors, `cached_tools`, `invalidate_tool_cache` | identical set (+ `responses_tools_array`, T-005) |
| `tool_cache_tests.rs` cases | 9 | 8 (shape re-assertions folded into one loop; the cache's key/invalidation/slice now covered) |

Verification on this tree:

```
cargo fmt --all -- --check                                clean
cargo clippy -p ragent-llm --all-targets                  no finding in a file this task touched
                                                          (19 pre-existing `unnecessary structure name
                                                          repetition` warnings in other modules)
cargo test -p ragent-llm --lib                            323 passed
cargo test -p ragent-llm --lib providers::tool_cache      8 passed (5 consecutive runs, and at /32 threads)
cargo test -p ragent-llm --doc                            30 passed
cargo test -p ragent-llm --test all                       293 passed, 2 ignored, 5 pre-existing failures
cargo test -p ragent-llm --test all test_wire_encode::    13 passed
all twelve scripts/check-*.sh                            PASS
```

The five `--test all` failures are the FR-004/FR-006/FR-017/FR-019/FR-020 guards
belonging to T-006/T-008/T-023, which are not done: `test_wire_decode::
test_the_provider_modules_no_longer_keep_their_own_tool_call_maps`,
`test_wire_decode::test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one`,
`test_wire_encode_message::test_no_provider_module_writes_a_result_carrier_literal`,
`test_wire_trace::test_agent_loop_sets_the_switch_from_config`,
`test_wire_trace::test_every_provider_records_its_body`. None is in a file this task
touched.

No dependency added (NFR-002). No `unsafe` block (NFR-003). No tool renamed by this
task; the Pass B rename is untouched.

## 6. What is left to the next task

- **T-006** reuses this encoder for the tool *result* carriers; `CachedTools` is
  definitions-only and stays that way.
- **T-027** owns the full-tree version of the envelope-literal scan: every module under
  `providers/`, every envelope key. This task's scan is scoped to the six modules the
  fold and T-005 cover, so the guard lands with the change rather than after it.
- T-005 has already landed the `OpenAiResponses` key, `ToolFormat::ALL`,
  `ToolFormat::tool_envelope` and the TUI's `openai_responses` arm, so
  `test_every_profile_a_provider_can_serve_is_reachable_through_the_cache` is now
  total in both directions and carries no exception.

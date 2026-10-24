# T-004 - One profile-driven `encode_tools`, one unchanged cache

Spec `uniformtools`, task T-004. Requirements FR-003 (one tool-definition encoder,
cache hit byte-identical to a miss) and NFR-001 (no per-step re-serialisation).

## 1. What was there

`crates/ragent-llm/src/providers/tool_cache.rs` held a byte cache and, inside it,
five near-identical builders:

```
build_openai        -> {"type":"function","function":{name,description,parameters}}
build_anthropic     -> {"name","description","input_schema"}
build_gemini        -> {"functionDeclarations":[{name,description,parameters}]}
build_bedrock       -> {"toolSpec":{name,description,inputSchema:{json:...}}}
build_huggingface   -> the OpenAI envelope with every name prefixed "t_"
```

Each one repeated the same nine lines of framing: reserve, write
`{"tools":[`, loop with `push_tool`, write `]}`. Each one also re-derived its own
`payload_start` / `payload_end` pair, and three of them recomputed the same
`buf.len() - 1` expression with a different comment. 259 lines of the module were
these five functions plus their shared `push_tool`.

The duplication mattered because FR-003 is not "one function exists" but "a provider
shall not construct its own tool array": five builders is five places a new envelope
can be added, and four of them never mention the profile that is supposed to define
the shape.

## 2. What replaced it

New `crates/ragent-llm/src/providers/wire/encode.rs`, holding the one encoder and the
one `CachedTools` type:

- `encode_tools(profile, name_prefix, tools) -> CachedTools` - frames the list once,
  then builds each item from `profile.tool_envelope`.
- `item_json(envelope, tool, name)` - the **only** definition of the five item shapes
  in the workspace, one `match` arm each.
- `push_tool` - the comma separator and the log-and-skip on a serialisation failure,
  kept verbatim from the old shared helper.
- `CachedTools` and its four accessors (`openai_tools_array`, `anthropic_tools_array`,
  `gemini_tools_array`, `bedrock_tool_config_object`) - moved without a change to
  their bodies or their contract.

The one real *difference* between the families is which slice of the buffer the caller
keeps:

| Envelope | Caller assigns | Payload |
|---|---|---|
| `BedrockToolSpec` | `body["toolConfig"]` | the whole `{"tools":[...]}` object |
| the other four | `body["tools"]` | the inner `[...]` array |

That is one arm of a `match` on `tool_envelope` in `encode_tools`, not a per-provider
builder. It is the reason `payload_start` / `payload_end` stay private fields rather
than becoming a public flag: the caller asks for a shape through an accessor named for
the shape, and the encoder decides the offsets.

## 3. What deliberately did not change

FR-003 names the cache, not just the encoder, so the fold stops at the encoder's edge.

- **`cached_tools(format, tools) -> Arc<CachedTools>`** - same signature. Every caller
  (`anthropic.rs`, `bedrock.rs` twice, `gemini.rs`, `openai_compat.rs`,
  `huggingface.rs`, plus the TUI's `tool_metadata_token_count`) compiles unchanged.
- **`invalidate_tool_cache()`** - same body: lock, `clear`. The invalidation path from
  `SessionProcessor::invalidate_tool_cache` is untouched.
- **`CachedTools`** - re-exported from `tool_cache` so the historic path still names it.
- **The key** - `(ToolFormat, fingerprint)` where the fingerprint hashes name,
  description and parameters in that order. Not "equivalent to" the old one: the same
  function, so a cache built before the fold and one built after would agree.
- **The locking discipline** - lookup under the lock, encode *outside* it, then
  re-check and insert under a second acquisition. Concurrent misses still do not
  contend and the first insert still wins.
- **`tool_arguments_json`** - moved, not changed. It is a *decode-side* helper (FR-007)
  and T-012's test asserts its two properties by reading this file, so moving it
  would have broken T-012 for no benefit.

`ToolFormat` also survives, and the reason is worth recording because the obvious
reading of "fold the builders into `encode_tools`" is that the enum disappears:

- it is the **cache key**, and the cache is process-global and shared across
  providers, so the key must distinguish five buffers over the *same* definitions;
- it is what the TUI passes (`tool_metadata_token_count`), because the TUI knows a
  provider id, not a profile;
- it carries no encoding rule. It resolves to `(profile, name_prefix)` through
  `format_profile`, and the envelope is read from the profile.

So the enum is a lookup argument, not a second source of truth. A new envelope is
added to `ToolEnvelope` and to `format_profile` and to nothing else.

The HuggingFace prefix is the one place the fold had to make a judgement call. It is
not a different envelope - it is the OpenAI envelope with a name rewrite - so making it
a sixth `ToolEnvelope` constant would have split one family's contract in two (FR-002
says a family declares *one* profile). It is therefore a `name_prefix` argument:
`Option<&str>`, applied to the name alone, `Cow::Borrowed` when there is no prefix so
the common path does not allocate. `HF_TOOL_PREFIX` moved into `encode.rs` with it -
the prefix is a wire-name rule - and `huggingface.rs` imports it back because the
*strip* on the reply path is that module's.

## 4. Proof

### 4.1 `crates/ragent-llm/tests/test_wire_encode.rs` - 12 cases

| Case | Asserts |
|---|---|
| `..._openai_function_envelope` | `type`/`function` nesting, name reached the wire, schema reached the wire, no flattened `name` |
| `..._anthropic_tool_envelope` | `name`/`description`/`input_schema`, and that there is no `type` or `function` wrapper |
| `..._gemini_function_declaration_envelope` | one `functionDeclarations` wrapper per tool, one declaration inside it, schema under `parameters` |
| `..._bedrock_tool_spec_envelope` | `toolSpec` items under `tools`, schema under `inputSchema.json` |
| `..._responses_flat_function_envelope` | flattened item: `type` and `name` side by side, `parameters` at the top level, no `function` object |
| `..._name_prefix_applies_to_the_name_only` | the prefix reaches the name and **not** the description or the schema |
| `..._is_deterministic_across_calls` | same bytes on a second call, after encoding an unrelated profile in between |
| `..._empty_list_is_an_empty_valid_payload` | five profiles produce a parseable empty list, not `Value::Null` |
| `test_cached_tools_hit_equals_a_fresh_encode_for_every_format` | FR-003's literal clause: for all five formats, `cached_tools(f, t).bytes == encode_tools(p, prefix, t).bytes` on the miss **and** on the hit |
| `test_every_profile_a_provider_can_serve_is_reachable_through_the_cache` | every `PROVIDER_WIRE` row resolves to an envelope a `ToolFormat` can ask for, with the one documented exception (below) |
| `test_folded_provider_modules_no_longer_write_an_envelope_literal` | none of the five folded modules contains `"input_schema"`, `"toolSpec"` or `"functionDeclarations"` |
| `test_encode_tools_bytes_match_the_pre_fold_builders` | the exact pre-fold byte strings, all five builders, captured from the originals |

The last one is the behaviour-preservation proof: a refactor that changes key order,
adds a field, or drops the comma separator fails on a literal rather than in a
provider request at run time. It is safe to pin because `serde_json`'s default map is
sorted, so the bytes do not depend on how a `json!` literal is written.

The Responses envelope is pinned here *before* T-005 routes `openai_responses.rs`
through the encoder, so the shape is recorded as a contract rather than inferred from
the module that task deletes. `test_every_profile_...` names it as the one exception
that no cache key resolves to yet, so the assertion that T-005's routing landed is the
same assertion that fails if it lands without a cache key.

### 4.2 `crates/ragent-llm/tests/inline/tool_cache_tests.rs` - 8 cases

The cache's own behaviour, unchanged in intent from before the fold and extended where
the fold created a new seam:

- `every_format_maps_to_a_distinct_profile_and_prefix_pair` - the whole mapping in one
  assertion, so `OpenAi`/`HuggingFace` sharing a profile but differing by prefix is
  visible rather than implied.
- `cached_tools_reuses_same_buffer`, `cached_tools_key_separates_formats_with_the_same_tools`,
  `cached_tools_key_tracks_the_tool_content` (name, description and schema each
  separately), `invalidate_tool_cache_drops_every_entry` (the map is empty, the bytes
  are identical, the buffer is new), `empty_tool_list_is_a_valid_empty_payload`,
  `byte_len_counts_the_payload_the_caller_assigns`, and
  `cached_tools_hit_is_byte_identical_to_a_fresh_encode`.

All eight take one file-local `cache_lock()`. That was not defensive: on a full
`--lib` run, `cached_tools_key_separates_formats_with_the_same_tools` cleared the
process-global map between `cached_tools_hit_is_byte_identical_to_a_fresh_encode`'s two
calls and the reuse assertion failed. One binary per crate means the cache is shared
by every test in the binary, so they must contend on one mutex. A file-local one
suffices because these are the only tests in the `ragent-llm` lib binary that touch it;
a crate sharing the cache with other modules would take `crate::global_env_lock()`.

### 4.3 Red proofs

Both were run, observed red, and reverted byte-identically:

| Break | Failing cases |
|---|---|
| Anthropic arm: `input_schema` -> `inputSchema` | `..._anthropic_tool_envelope`, `..._bytes_match_the_pre_fold_builders` |
| `gemini.rs`: `body["tools"] = cached.gemini_tools_array()` -> a hand-built `json!([{"functionDeclarations": []}])` | `..._folded_provider_modules_no_longer_write_an_envelope_literal` |

The second is the one that shows the source scan is a guard rather than decoration: it
fails on a *caller* re-deriving the envelope, which is exactly the state FR-003
forbids.

## 5. Measurements

| Measure | Before | After |
|---|---|---|
| `tool_cache.rs` lines | 373 | 165 |
| tool-envelope builders | 5 | 0 |
| modules under `providers/` that build a tool envelope | 2 (`tool_cache.rs`, `openai_responses.rs`) | 1 (`openai_responses.rs`, T-005's) |
| `cached_tools` call sites | 6 | 6 (unchanged) |
| public API items | `ToolFormat`, `CachedTools` + 4 accessors, `cached_tools`, `invalidate_tool_cache` | identical |

Verification, all green on this tree:

```
cargo fmt --all -- --check                                clean
cargo clippy -p ragent-llm --all-targets                 0 warnings
cargo test -p ragent-llm --test all                      226 passed, 2 ignored
cargo test -p ragent-llm --lib                           321 passed
cargo test -p ragent-llm --doc                           27 passed
cargo check --workspace --all-targets                    clean
scripts/check-inline-tests.sh                            PASS
scripts/check-test-env-isolation.sh                      PASS
scripts/check-silent-errors.sh                           PASS
scripts/check-security-unwraps.sh                        PASS
scripts/check-poison-locks.sh                            PASS
scripts/check-shared-guards.sh                           PASS
```

No dependency added (NFR-002). No `unsafe` block (NFR-003). No tool renamed by this
task; the Pass B rename is untouched.

## 6. What is left to the next task

- **T-005** deletes the inline array in `openai_responses.rs` and adds the
  `ToolFormat` variant that keys the Responses envelope. Until then that module is the
  one provider still building its own array, and the encoder's
  `ResponsesFlatFunction` arm is exercised only by tests. The exception is named
  explicitly in `test_every_profile_a_provider_can_serve_is_reachable_through_the_cache`
  so it cannot be forgotten.
- **T-027** owns the full-tree version of the envelope-literal scan: every module under
  `providers/`, every envelope key. This task's scan is deliberately scoped to the five
  modules it folded, so the guard lands with the change rather than after it.
- **T-006** reuses this encoder for the tool *result* carriers; `CachedTools` is
  definitions-only and stays that way.

# T-027 - No provider builds its own tool JSON, proven over the whole tree

Spec `uniformtools`, task T-027. Requirement FR-003: one tool-definition encoder,
and no provider module that constructs its own tool array.

## 1. What the task asks for

> Prove no provider builds its own tool JSON: add a source guard scanning
> `crates/ragent-llm/src/providers/` for hand-built envelope literals.

T-004 folded the five builders into `wire::encode::encode_tools` and added a scan,
but that scan was scoped to the six modules the fold touched and to three keys. A
scoped scan proves the change; it does not keep the tree true afterwards. This
task owns the full-tree version: every `.rs` file under `providers/` (including
`wire/`), every key an item shape is built from.

## 2. State found

| Site | State at the start of this task |
|---|---|
| `crates/ragent-llm/src/providers/wire/encode.rs` | `item_json` was, and is, the one definition of the five item shapes. No provider module built a definition object. |
| `tests/test_wire_encode.rs` | `test_folded_provider_modules_no_longer_write_an_envelope_literal` scanned six named files for three keys (`"input_schema"`, `"toolSpec"`, `"functionDeclarations"`). It did not scan `azure_foundry.rs`, `azure_resource.rs`, `copilot.rs`, `ollama*.rs`, `openai.rs`, `openrouter.rs`, `router*.rs`, `xai.rs`, `sse.rs`, `thinking.rs` or `wire/`; it did not know the Bedrock `inputSchema`, the snake-case `function_declarations` spelling, or the two OpenAI `function` spellings. |
| the six body assignments | Each provider opened the body key as a string literal: `body["tools"] = ...` in five modules and `body["toolConfig"] = ...` in `bedrock.rs`. Correct today, and unguarded: the literal is exactly what a hand-built list would also need, so a scan for the key could only be written by exempting the one place that writes it. |

So the claim in `SPEC.md` ("no provider module builds tool JSON of its own") was
true by inspection and held by nothing that walks the tree.

## 3. What this task changed

### 3.1 The body key is derived, not typed

`crates/ragent-llm/src/providers/tool_cache.rs` gained two constants and one
accessor:

```rust
pub const TOOLS_KEY: &str = "tools";
pub const BEDROCK_TOOLS_KEY: &str = "toolConfig";

impl ToolFormat {
    pub const fn body_key(self) -> &'static str;
}
```

`body_key` reads `tool_envelope()` and answers `toolConfig` for
`BedrockToolSpec` and `tools` for the other four, so the key a provider assigns
to and the payload the encoder produces cannot drift apart - which is the one
thing a hand-written key could still get wrong silently. The six call sites now
read `body[ToolFormat::Anthropic.body_key()] = ...` (or `Gemini`, `HuggingFace`,
`OpenAi`, `Bedrock`, `OpenAiResponses`), with no behaviour change: the same two
strings reach the same six bodies.

This is what makes the key scannable: with the literals gone, the scan can refuse
`["tools"]` and `["toolConfig"]` everywhere outside the constants, so the only
remaining way to name the key is to reintroduce the literal the guard rejects.

### 3.2 Three cases in `crates/ragent-llm/tests/test_wire_encode.rs`

Both scans are hermetic - they read the crate's own `src/` tree through
`CARGO_MANIFEST_DIR`, contact nothing and build no session (NFR-004).

- `test_no_provider_module_writes_an_envelope_key` walks every `.rs` file under
  `providers/` recursively, strips comments (line, block and nested block, with
  string and character literals honoured so a `//` inside a URL is not a
  comment), and fails naming every offending `file:line: key`. `item_json`'s own
  module is the single exemption and is named by path, so a second module under
  `wire/` cannot inherit it. Eight keys: `"input_schema"`, `"inputSchema"`,
  `"toolSpec"`, `"functionDeclarations"`, `"function_declarations"`,
  `"tool_call"`, and both OpenAI function spellings (`\"type\":\"function\"` in
  a JSON string, `"type": "function"` in a `json!` literal).
- `test_no_provider_module_writes_a_tool_body_key_literal` refuses `["tools"]`
  and `["toolConfig"]` in every provider module except the two places that
  define them (`tool_cache.rs`'s constants) and `wire/encode.rs` (the wrapper
  itself).
- `test_every_tool_body_key_is_the_one_the_encoder_wraps` is the case that makes
  the other two worth something: a format that *reported* the wrong key would
  pass both scans and still put the payload where nothing reads it. It asserts
  `body_key` per format against `tool_envelope`, and that Bedrock's payload
  really starts `{"tools":[` while the other five assign the inner array.

### 3.3 What is deliberately not scanned

- **Prose.** Comments are stripped before the scan. `encode.rs`, `wire/mod.rs`
  and `profile_table.rs` name `input_schema` and `toolSpec` when explaining why
  the shapes live where they do; a scan that fired on module docs would be
  narrowed until it stopped seeing the defect.
- **`"tool_use"`.** It is the Anthropic item's tag *and* the Anthropic stream's
  content-block type *and* its `stop_reason`, so in this tree the literal only
  appears where it means the other two (`anthropic.rs`, `azure_resource.rs`,
  `bedrock.rs`). Including it produced eight false hits and no true one. The
  Anthropic item is caught by `"input_schema"`, which no message shape uses.
  This was measured, not assumed: see the red proofs.
- **The Responses item's flat keys** (`description`, `parameters`, `name`,
  `type`). They are shared with unrelated message bodies; T-005's own case
  (`test_responses_provider_asks_the_encoder_for_its_tool_array`) scopes them to
  the one module that used to build a Responses array.
- **The tool-*call* `"type": "function"` literal** inside assistant `tool_calls`
  arrays. Those spellings are in this key list because they are the same text;
  the call-carrier subject belongs to T-006, whose own guard
  (`test_no_provider_module_writes_a_result_carrier_literal`) owns it.

## 4. Red proofs

Three, each restored byte-identically from `target/temp/t027/`, verified with
`md5sum -c` and `touch`ed. Measured with `-E 'test(test_wire_encode::)'` only.

1. **A seeded hand-built array in a module neither scan had ever looked at.**
   `xai.rs` gained
   `fn seeded_hand_built_tools() { serde_json::json!({ "tools": [{ "input_schema": {} }] }) }`.
   `..._writes_an_envelope_key` fails with
   `providers/xai.rs:299: "input_schema"`. Before this task that file was not in
   any list, so the scoped scan stayed green.
2. **A hand-written body key.** `gemini.rs` reverted to `body["tools"] = ...`.
   `..._writes_a_tool_body_key_literal` fails with
   `providers/gemini.rs:473: ["tools"]`, while the envelope scan stays green -
   which is the point of having both.
3. **A format reporting the wrong key.** `body_key`'s non-Bedrock arm was changed
   to `"toolConfig"` *in the test's own expectation* (a source arm change is
   rejected by the compiler, so this is the only form the break can take). The
   case fails with `left: "tools"` / `right: "toolConfig"` naming `OpenAi`.

Two further attempts proved nothing and are recorded rather than claimed:

- The first run of `..._writes_an_envelope_key` with `"tool_use"` in the key list
  produced eight hits, all of them the Anthropic stream's block type or stop
  reason (`anthropic.rs:495`, `:551`, `azure_resource.rs:338`, `:538`, `:592`,
  `bedrock.rs:671`, `:760`, `:1099`). The key was removed and the reason written
  into the constant's doc. A scan that cannot say "this literal means a
  definition here" is not a guard for definitions.
- Editing `body_key`'s Bedrock arm in the source (rather than the test) failed to
  compile: the match on `tool_envelope()` is exhaustive over `ToolEnvelope`, so a
  wrong arm is a type error before it is a test failure. Recorded because it
  means case 3's guard is one-directional by construction - it catches a key
  expectation that disagrees with the envelope, not a compiler-rejected arm.

## 5. Verification

- `cargo fmt --all -- --check` clean.
- `cargo check --workspace --all-targets` clean (1m 18s).
- `cargo clippy -p ragent-llm --all-targets` reports only the pre-existing 19
  `unnecessary structure name repetition` in `tool_cache.rs` (none in a line this
  task added - the new constants and `body_key` are clean) and one pre-existing
  `let`-binding warning in `ragent-storage`; the one trail- ing-comma warning the
  new test introduced was fixed.
- `cargo nextest run -p ragent-llm` 639 run / 639 passed / 2 skipped.
- `cargo test -p ragent-llm --doc` 30 passed.
- `cargo nextest run -p ragent-agent --no-fail-fast` 2151 run / 2146 passed / 5
  failed / 1 skipped. All five are `test_tool_rename_guard::` cases belonging to
  T-047 (the NFR-008 guard's predicate (g) reports a real wholesale forwarder at
  `crates/ragent-agent/src/tool/aliases.rs:118`); none is in a file this task
  touched.
- 13 guard scripts: 12 pass. `check-tool-rename.sh` exits 1 on the same T-047
  hit, and `check-bench-regression.sh` exits 2 for want of `target/criterion`.
  Both are pre-existing and unrelated.
- No dependency added (NFR-002), no `unsafe` (NFR-003), no tool renamed.

## 6. What is left to the next task

- **T-026** consolidates the per-family recorded frames into replay fixtures;
  this task added no fixture and changed no encoder byte.
- **T-006** owns the tool-*call* carrier literals (`"type": "function"` inside
  assistant `tool_calls`), which this scan sees only as text and does not claim.
- The full-tree scan now covers `wire/`, so T-006's carrier work and T-013's
  dispatch work add modules the envelope case sees and the body-key case
  exempts only for `tool_cache.rs` and `wire/encode.rs` - a new module that
  names a key fails without an edit to the exemption list.

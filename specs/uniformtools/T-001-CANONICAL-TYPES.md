# T-001 - The two canonical type additions

Spec `uniformtools`, task T-001. Requirements FR-014 (an error result is never
indistinguishable from a success) and FR-011 (reasoning is replayed where the
profile requires it).

Status: **completed**. Dependency: none (root of Pass A).

## 1. What the task asks for

D1 of the spec states the scope exactly:

> `ContentPart` is already the provider-neutral vocabulary every encoder matches
> on. The fix is to complete it, not to replace it:
>
> - add `is_error: bool` to `ToolResult`;
> - add an optional `reasoning` echo channel (`ContentPart::Reasoning { text,
>   signature: Option<String> }`) that is populated from persisted history and
>   read only by providers whose profile declares `echo_reasoning`.

So T-001 is a **type-representation** task, not a behaviour task. Transmitting
the flag is FR-014's encoding half (T-017); replaying the signed block is FR-011's
encoder half (T-018).

## 2. What is in the tree

Both additions are present in `crates/ragent-types/src/llm.rs` (landed with the
rename commit, `d6abcd4a`), and this task's work was to verify they say what the
requirement needs and to pin them with tests, because until now nothing in the
crate tested the `Reasoning` variant at all.

| Addition | Site | Shape |
|---|---|---|
| `is_error` | `ContentPart::ToolResult` | `bool`, `#[serde(default)]` |
| `Reasoning` | `ContentPart` (fifth variant) | `{ text: String, signature: Option<String> }`, `signature` is `#[serde(default, skip_serializing_if = "Option::is_none")]` |

Three properties are deliberate and are what the tests pin:

1. **`is_error` defaults rather than rejects.** A session or fixture persisted
   before the flag existed deserialises with `false` - "a result with no recorded
   error" keeps its prior meaning. The alternative (`deny_unknown_fields` on the
   variant, or a required field) would have failed to load old history.
2. **`is_error` is *always* written**, including `false`. The encoders read the
   field (`wire::encode::mark_error` takes it as the value to branch on), so an
   omitted `false` would make "absent" and "success" two spellings of one state,
   and a strict reader could not tell a success from a pre-flag record. This is
   the one place the two additions are deliberately asymmetric.
3. **`signature` is omitted when `None`.** A signature-less block then has the
   same wire shape it would have had without the field, and `null` still
   deserialises to `None` for a payload that writes one explicitly.

The tag strings (`text`, `tool_use`, `tool_result`, `image_url`, `reasoning`) are
the exact keys every provider encoder and the history converters match on, so the
set of five is part of the contract rather than an implementation detail.

## 3. What this task did not touch

The flag's *derivation* already exists at the record sites and belongs to FR-014's
other half: `crates/ragent-agent/src/session/history.rs:245`,
`session/processor.rs:2932` and `compaction/convert.rs:189` all set
`is_error: state.status.is_error()`, and `ToolCallStatus::is_error` is the one
derivation (never `content` parsing, never `!success`). That is T-017's subject and
it is unchanged here.

`parts_to_chat_content` still skips `MessagePart::Reasoning`, and
`MessagePart::Reasoning` still carries no signature - that is T-018's subject
(persist the signature, replay it for `echo_reasoning` profiles). T-001 only
provides the destination those two tasks write into.

## 4. Proof

New `crates/ragent-types/tests/test_canonical_tool_parts.rs`, six cases, registered
in `crates/ragent-types/tests/all.rs`:

| Case | Asserts |
|---|---|
| `..._error_flag_reaches_the_wire_in_both_states` | an error result serialises `is_error: true`; a success serialises an explicit `false` and not an omission; the content is byte-identical between the two |
| `..._error_flag_round_trips_both_states` | both states survive serialise/deserialise, so the encoder cannot see a different flag than history recorded |
| `..._reasoning_part_carries_text_and_signature` | the `reasoning` tag, the text, and the signature all reach the wire and come back |
| `..._reasoning_part_without_a_signature_omits_the_key` | the key is absent on `None`; an absent key and an explicit `null` both decode to `None` |
| `..._reasoning_is_not_the_text_part` | a reasoning block with the same words as a text block is still a different tag - otherwise every provider would be sent reasoning as assistant output |
| `..._canonical_part_vocabulary_is_five_tags` | the whole vocabulary and its wire tags in one assertion, so a sixth variant or a renamed tag fails here rather than in a provider request |

`every_content_part_variant_round_trips` in `test_llm_serde.rs` also gained the
`Reasoning` variant (and a `reasoning` tag assertion). That test claimed to cover
"every" variant and had been silently covering four of five since the variant was
added - the gap this task closes is that the fifth one was untested, not that it
was broken.

### Red proofs

Both were run, observed red, and reverted byte-identically (`git diff` against
`HEAD` empty afterwards):

| Break | Failing case |
|---|---|
| `is_error` given `skip_serializing_if = "std::ops::Not::not"` (i.e. omit `false`) | `..._error_flag_reaches_the_wire_in_both_states` - "no entry found for key" on the success payload |
| `signature` loses `skip_serializing_if` | `..._reasoning_part_without_a_signature_omits_the_key` - the payload reads `"signature": Null` |

The second break is the sharper one: it would not have failed any existing test,
which is why the omission rule is worth pinning as its own case.

## 5. Verification

All green on the tree as it stands:

- `cargo fmt --check`
- `cargo clippy -p ragent-types --all-targets` - no warning, no error
- `cargo test -p ragent-types --test all` - **401 passed**, 0 failed
- `cargo test -p ragent-types --lib` - 2 passed
- `cargo test -p ragent-types --doc` - 38 passed, 2 ignored

## 6. Note for the downstream tasks

`is_error` being written even when `false` is the property T-017's `mark_error`
already depends on (it reads the field, not its presence), and `signature`'s
omission rule is what T-018's replay has to preserve: an echoed Anthropic
thinking block carries `signature` as a sibling of `thinking`, and a block with
no signature must not gain a `null` it never had.

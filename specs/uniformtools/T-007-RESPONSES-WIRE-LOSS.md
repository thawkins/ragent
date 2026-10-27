# T-007 - The Responses input list refuses what it cannot carry

Spec `uniformtools`, task T-007. Requirement FR-018 (a canonical construct the
profile cannot carry is refused with an error naming the construct and the
provider, and the request is not issued), guarding the historic Responses-API
history drop.

Dependency: T-006 (the profile-driven `encode_message`).

## 1. What was found, and what was not

The task read as "delete the text-only part filter and raise `WireLoss`". The
filter was already gone, and so was the first refusal:

- `commit d6abcd4a` rewrote `ResponsesApiClient::build_input_items` and the
  filter it replaced (`ContentPart::Text` matched, everything else `_ => None`)
  became `function_call` / `function_call_output` items.
- The same commit raised `WireLoss` for the one construct the input list has no
  item shape for (an image part).
- T-006 deleted the client-side loop entirely, so the whole construction lives
  in `wire::encode::responses_items`, and it moved the image refusal there with
  it: the family with no slot for a construct is the one that refuses it, and a
  loop deletion would otherwise have turned a refusal into a silent strip.

So two of the three cases this task's subject needs were **green at HEAD**:

| Case | State at the start of this pass |
|---|---|
| `test_responses_input_carries_tool_history_instead_of_only_text` | green |
| `test_the_responses_client_body_uses_the_encoder_for_its_input_list` | green |
| `test_a_refused_construct_refuses_the_whole_conversation` (image) | green |
| a construct that is silently *omitted* rather than refused | **open** |

That last row is the measured work, and section 2.1 names the construct.

## 2. What changed

### 2.1 The second construct with no item shape: the message role

`ChatMessage::role` is a `String`, not an enum, so a role the input list does
not accept reaches the encoder. `responses_role` accepts `user`, `assistant`,
`system` and `developer`, and `responses_items` answered every other role with

```rust
let Some(role) = responses_role(&msg.role) else {
    return Ok(Vec::new());
};
```

which is a turn silently missing from the request - precisely the failure FR-018
exists to forbid ("**shall not** silently drop it"). It now returns

```rust
return Err(RagentError::WireLoss {
    construct: format!("role:{}", msg.role),
    provider: RESPONSES_PROVIDER_ID.to_string(),
});
```

The error names the role it could not carry (`role:tool`, not a bare `role`), so
the failure says *what* was lost as well as who could not carry it.

### 2.2 One provider id instead of two literals

Both refusals - the image part and the role - lived with a `"openai_responses"`
string literal in the encoder. They now read one private constant:

```rust
const RESPONSES_PROVIDER_ID: &str = "openai_responses";
```

Four lines of code, and the point is not brevity: an error that names a
*different* provider from the one that failed is worse than no name, and two
literals are two chances to drift.

### 2.3 Deliberately unchanged

- **The other five profiles keep their own role policies.** A `system` turn on
  Converse, or a `tool` turn on Gemini, is still a dropped turn; the encoder is
  shared but each family still owns its slot list, and widening this refusal to
  six families would be a behaviour change no requirement asks for. The new case
  asserts exactly that (`only the Responses input list refuses an unknown
  role`), so a later "consistency" edit has to argue with a test.
- **A reasoning part stays omitted (FR-011).** The Responses API has no
  signed-reasoning input item, and reasoning replay is T-018; the refusal there
  would be wrong today because the canonical history does not yet carry a
  reasoning part at all.
- **`build_request_body` keeps returning `Result`.** `chat` already applies `?`
  to it, so the "shall not issue the request" clause is a property of the
  existing call graph, and the new case pins it at that boundary rather than at
  the encoder's.

### 2.4 Diff size

| File | Change |
|---|---|
| `wire/encode.rs` | `responses_items` refuses an unknown role; `RESPONSES_PROVIDER_ID`; two doc paragraphs |
| `tests/test_wire_encode_message.rs` | two cases added (37 -> 38), one module comment and one case comment re-pointed |

No wire byte changed for any history that already encoded: every message the
input list accepted before still produces the same items.

## 3. The cases

`crates/ragent-llm/tests/test_wire_encode_message.rs`:

1. `test_a_role_the_responses_input_list_has_no_item_for_is_refused` - the
   conversation plus a `role: "tool"` turn; asserts
   `WireLoss { construct: "role:tool", provider: "openai_responses" }`, that all
   four accepted roles still carry, and that the other five profiles accept the
   same history.
2. `test_the_responses_client_refuses_to_build_a_body_it_would_shorten` - the
   same history through `ResponsesApiClient::build_request_body`, which is the
   function `chat` applies `?` to, plus the carryable history that still builds.
   This is the case that distinguishes "the encoder refuses" from "no request is
   issued".

T-007 is why `test_responses_input_carries_tool_history_instead_of_only_text` no
longer says the refusal policy is "T-007's change": the policy is here, and the
case now points at the two cases that assert it.

## 4. Red proofs

Each break was reverted byte-identically and verified with
`md5sum -c target/temp/t007/after.md5` plus `touch` (so the rebuild is not
mtime-cached).

| # | Break | Red |
|---|---|---|
| 1 | `responses_items` back to `return Ok(Vec::new())` for an unknown role | both new cases: `a role ... must be refused rather than dropped (FR-018): Ok([...4 items, no tool turn...])` |
| 2 | the image refusal's `provider` emptied | T-006's `test_a_refused_construct_refuses_the_whole_conversation` |
| 3 | `construct: format!("role:{}")` -> `construct: "role"` | both new cases: `left: "role"` / `right: "role:tool"` |

Proof 1's payload is the interesting one: it prints the exact history a user
would have had sent, with the `role: "tool"` turn simply absent.

## 5. Verification

- `cargo fmt --all -- --check` clean.
- `cargo check --workspace --all-targets` clean (1m16s).
- `cargo clippy -p ragent-llm --all-targets`: only the pre-existing 19
  `unnecessary structure name repetition` in `tool_cache.rs` plus one
  `ragent-storage` let-binding; nothing in a touched file.
- `cargo nextest run -p ragent-llm --no-fail-fast`: 641 passed, 2 skipped,
  0 failed.
- `cargo nextest run -p ragent-llm -E 'test(test_wire_encode_message::)'`:
  38 passed (was 36).

## 6. Hand-off

- T-018 (reasoning replay) owns the third construct: when the Responses input
  list gains a reasoning item, the `ContentPart::Reasoning` arm stops being a
  no-op, and if it does not gain one the arm becomes a refusal under this task's
  rule.
- The role refusal is recorded in TC-014's automated half; the
  `role:<role>` construct name is the spelling a reader of a log will see, so a
  later change to it is a change to a user-visible string.

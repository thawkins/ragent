# T-017 - Mark a failed tool result on every family

Spec `uniformtools`, task T-017. Requirement FR-014 (error results are marked).

Status: **completed**. Dependencies: T-006 (the one message encoder and its four
result carriers), T-013 (the refusal wiring, which set the flag's origin).

## 1. What the task asks for

> **FR-014** (ubiquitous) Every tool result **shall** carry an `is_error` flag
> derived from its record status. Where the profile declares
> `supports_is_error == true` the flag **shall** be transmitted natively; where
> it declares `false` the encoder **shall** prefix the content with the literal
> marker `ERROR: ` and no other content change **shall** be made.

Two halves, and they were delivered at different times. The **flag's origin** -
that it comes from `ToolCallState.status` (via `ToolCallStatus::is_error`) and
never from the content text - landed with T-013 and is pinned by
`test_tool_input_error::is_error_is_derived_from_the_status_type` and
`...::every_refusal_is_an_error_by_the_status_derivation`. The **transmit**
half is this task, and the honest thing to record is that most of it already
existed: T-006's encoder applied `mark_error` at five of its carrier arms and
`tests/test_wire_encode_message.rs` already had one case per carrier. What was
missing was narrower and worse than a missing test.

## 2. State found

| # | State | Where |
|---|---|---|
| 1 | `mark_error(profile, content, is_error)` already produced the `ERROR: ` prefix wherever `supports_is_error` is `false`, and returned a flag saying whether the caller should also write a native one. Five carrier arms called it: OpenAI `role:"tool"`, native Ollama's name-keyed result, Gemini `functionResponse`, Responses `function_call_output`, and the fall-through arms. Anthropic and Converse wrote their native flags (`is_error`, `status`). | `wire/encode.rs` |
| 2 | `MessageContext` had no way to say "mark this result by a rule other than the carrier's profile", so the one caller that does not use the carrier had none. | `wire/encode.rs` |
| 3 | **Local Ollama marks nothing.** `build_openai_messages` cannot call the OpenAI carrier for Ollama because the dialect adds a `tool_name` key (`OpenAiCompatSpec::tool_result_name`), so the messages went out with `is_error` **dropped entirely** - no flag, no prefix. A failed `Read` reached the model looking exactly like a successful one, on one provider, with no test covering it. | `openai_compat.rs` |
| 4 | `OpenAiCompatSpec::ollama_cloud` speaks the native `/api/chat` shape and never reaches the encoder's OpenAI carrier, so its marking was already correct at the `OLLAMA_NATIVE` arm. | `ollama_cloud.rs` |
| 5 | Nothing stated *which* profile's `supports_is_error` a format's messages obey. `format_profile()` answers the question for **definitions**; for five of six formats it is also the right answer for messages, and for HuggingFace it is right by coincidence (both formats it could concern are non-native). | `tool_cache.rs` |
| 6 | `test_tool_input_error::is_error_is_derived_from_the_status_type` was a **negative** scan (two wrong spellings absent). A converter that wrote `is_error: false` everywhere would have passed it while making every failure look successful. | `ragent-agent/tests` |

Row 3 is the reason this task was not a test-only task.

## 3. What landed

### 3.1 The format states the marking rule, not the caller

```rust
impl ToolFormat {
    const fn message_profile(self) -> ToolWireProfile;      // definitions vs messages
    const fn transmits_is_error_natively(self) -> bool;     // message_profile().supports_is_error
    fn result_content(self, content: &str, is_error: bool) -> String; // error_marked(..)
    const fn message_name_prefix(self) -> Option<&'static str>;       // always None
}
```

`result_content` is `error_marked(content, is_error && !transmits_is_error_natively())`:
the marking is the exported `error_marked`, so the dialect path and the encoder
path produce the same six bytes, and what `ToolFormat` adds is only the *choice*
of whether they are needed.

`message_profile` is deliberately a **second** name rather than a reuse of
`format_profile`. The two answer different questions - which envelope encodes the
tool list, and which contract encodes the history - and they differ for
`OpenAiResponses` in fact (`IdField::Key` versus OpenAI chat's
`synthetic_ids: true`, `function_call_output` versus `role:"tool"`). Sharing one
name would make the next reader assume the definition envelope dictates the
message shape, which is exactly the bug row 3 was.

`message_name_prefix` is `None` for all six formats. HuggingFace prefixes every
*tool name in its definitions* (`t_`); its messages are OpenAI's, where a tool
call carries no name and a result is keyed by the id the model already returned.
Carrying the prefix into the history would edit bytes for no gain and could only
break the id/name pairing. It exists as a named method so the next person who
wonders "should the prefix apply here too?" reads a documented no.

### 3.2 The encoder takes a named format

`MessageContext` gained:

```rust
pub error_marking: Option<crate::provider::tool_cache::ToolFormat>,
```

`Default` is `None`, which is correct for the four carriers (they already mark
themselves). The OpenAI `role:"tool"` arm reads it first:

```rust
let (content, _) = match ctx.error_marking {
    Some(format) => (format.result_content(content, *is_error), false),
    None => mark_error(profile, content, *is_error),
};
```

Naming the *format* rather than a `bool` is what keeps one derivation. A boolean
would let a caller assert "this family needs no marker" and be wrong with no test
able to see it; a format is a fact the profile table already declares.

### 3.3 The dialect uses it

`build_openai_messages` now sets two context fields:

```rust
let ctx = MessageContext {
    tool_names: tool_names.as_ref(),
    mirror_tool_name: spec.tool_result_name,
    error_marking: spec.tool_result_name.then_some(OpenAiCompatSpec::MESSAGE_FORMAT),
    name_prefix: OpenAiCompatSpec::MESSAGE_FORMAT.message_name_prefix(),
    ..MessageContext::default()
};
```

`OpenAiCompatSpec::MESSAGE_FORMAT` is `ToolFormat::OpenAi` with a doc paragraph
saying why: every provider that builds a body through this module speaks the
OpenAI chat-completions *message* vocabulary, and what varies is dialect
switches, never a second message shape.

`name_prefix` is new on this call site and is `None` today (row 3.1). It is set
so the wiring exists: a dialect that later needs a rewrite asks the format rather
than restating the prefix, which is what stops the request's tool names and the
names in the calls it is answered with from drifting apart.

## 4. Tests

`crates/ragent-llm/tests/test_tool_result_error_marking.rs` (registered in
`tests/all.rs`), seven cases:

1. `test_every_format_takes_the_marking_rule_from_its_declared_profile` - per
   format, `transmits_is_error_natively == message_profile().supports_is_error`;
   and exactly two formats (Anthropic, Bedrock) are native.
2. `test_a_format_without_a_native_flag_prefixes_exactly_an_error` - every
   format: an error is prefixed iff it is not native, and a success is never
   edited.
3. `test_the_marker_is_the_literal_fr014_names` - `ERROR_PREFIX == "ERROR: "`,
   and `error_marked("", true) == "ERROR: "` (an empty failure is an error
   result, not a blank success).
4. `test_a_named_format_marks_the_result_the_encoder_writes` - the OpenAI carrier
   marked by a named `ToolFormat::OpenAi`, by its own profile, by a *native*
   named format (content untouched, which is what makes the field a statement of
   the rule rather than an override), native Ollama's own carrier, and a success
   on each.
5. `test_a_definition_name_rewrite_does_not_reach_the_messages` - the
   HuggingFace definition envelope does carry `t_Read`, and
   `message_name_prefix` is `None` for every format.
6. `test_a_caller_that_names_a_tool_still_gets_the_rewrite_applied` - a caller
   that *does* pass `name_prefix` gets it in an assistant tool call, and the
   marking is unaffected.
7. `test_local_ollama_marks_a_failed_result_on_the_wire` - the end-to-end case:
   a throw-away HTTP server captures the real `OllamaProvider` request, which
   carries one successful and one failed result; the failed one is
   `ERROR: No such file` and the successful one is still `alpha`.

`crates/ragent-agent/tests/test_tool_input_error.rs` gained the **positive half**
the existing negative scan lacked:
`every_history_converter_forwards_the_record_status_to_the_wire_flag` asserts
each converter contains its derivation
(`state.status.is_error()` in `history.rs` and `convert.rs`, `status.is_error()`
in `processor.rs`, which holds the outcome's status rather than the state).

## 5. Red proofs

Each was applied, run, and restored byte-identically from `target/temp/t017/`
with `md5sum -c after.md5` and a `touch` (a `cp -p` restore keeps the old mtime,
so cargo would reuse the broken object):

1. **The marking removed from the OpenAI carrier** (`let (content, _) =
   mark_error(profile, content, *is_error);` in place of the `match`) -
   `test_a_named_format_marks_the_result_the_encoder_writes` reds with
   `left: String("ERROR: Permission denied")` / `right: "Permission denied"` on
   the native-format arm, and `test_a_format_without_a_native_flag_prefixes_exactly_an_error`
   reds with `Anthropic sends the flag itself, so the text must not be edited`.
2. **`result_content` ignoring the profile** (`error_marked(content, is_error)`)
   - the same two cases red with the same two messages, which is what shows the
     named format is doing real work rather than agreeing by luck.
3. **The named format left off the Ollama dialect's context**
   (`error_marking: None`) - the end-to-end case reds; this is the one that
   proves row 3 of section 2 was a real defect, because the other six cases all
   stay green.
4. **`message_name_prefix` returning `self.name_prefix()`** -
   `test_a_definition_name_rewrite_does_not_reach_the_messages` reds with
   `left: Some("t_")` / `right: None`.

A fifth candidate - deleting `name_prefix` from the dialect context - was
rejected as proving nothing: it is `None` today, so no case can distinguish it,
and the case that guards the decision is (4), not the deletion.

**A red proof was run against a *stale* build once and must not be repeated.**
After (1) was restored with a plain `cp` (no `touch`), the subsequent run
reported all seven green because cargo had reused the object it had already
compiled for the restored mtime-compatible file. The `touch` in the restore
recipe exists for exactly this; a "still green" result after a restore is a
signal to check `md5sum -c` and the mtime, not evidence.

## 6. Deliberately unchanged

- **No wire byte changed for the six carriers' non-dialect paths.** The
  `error_marking` default is `None` and every existing marking case passes
  unchanged.
- **`mark_error` was not deleted or renamed.** It is the carrier-level function
  and `error_marked` is its content-level twin; `result_content` composes the
  latter with the profile's declaration, so three names remain for three
  distinct jobs.
- **`OpenAiCompatSpec::ollama_cloud` still passes no `error_marking`.** It does
  not use this encoder's OpenAI carrier at all; its results are marked by the
  `OLLAMA_NATIVE` carrier, which is already correct, and setting the field would
  be a second rule for the same bytes.
- **The `router` row is untouched.** `RouterClient` forwards the canonical
  request to the downstream provider, whose own client encodes it with the
  downstream profile, so the marking is already the effective downstream family's.
  The gap T-003 records for other fields applies for the same reason and is
  recorded there, not fixed here.
- **No `ERROR: ` prefix is added beside a native flag.** FR-014 says the marker
  is for profiles without native support, and a Converse `toolResult` carrying
  both `status: "error"` and an `ERROR: `-prefixed content would change content
  FR-014 says must not change.

## 7. Verification

- `cargo fmt --all -- --check` clean.
- `cargo check --workspace --all-targets` clean (1m 17s).
- `cargo nextest run -p ragent-llm --no-fail-fast`: **648 run, 648 passed,
  2 skipped**; the touched files' own cases 7/7 and `test_wire_encode_message`
  38/38.
- `cargo nextest run -p ragent-agent --no-fail-fast`: **2152 run, 2147 passed,
  5 failed, 1 skipped**. All five failures are the pre-existing
  `test_tool_rename_guard::` group (the T-047 predicate-(g) hit at
  `crates/ragent-agent/src/tool/aliases.rs:118`, unrelated to this task); the
  `test_tool_input_error::` module is 9/9.
- `cargo clippy -p ragent-llm --all-targets`: only the pre-existing 25
  `unnecessary structure name repetition` warnings on `ToolFormat::` paths in
  `tool_cache.rs` (the same lint the file already carried before this task) and
  the pre-existing `ragent-storage` let-binding. No warning originates in a line
  this task added except members of that existing group.
- Guard scripts: 11 of 13 pass. `check-bench-regression` exits 2 for a missing
  `target/criterion`, and `check-tool-rename` exits 1 on the same T-047 hit -
  both pre-existing.

## 8. Notes for the downstream tasks

- **T-018** (reasoning echo) does not touch this path: `ContentPart::Reasoning`
  is not a tool result and carries no `is_error`.
- **A seventh format** now has four questions to answer rather than two:
  `format_profile`, `message_profile`, `message_name_prefix` and `body_key` are
  all exhaustive `match`es over the enum, so the compiler names each one. That is
  intentional - a new wire format is a contract change.
- **Any new dialect that packs its own `messages` array** must set
  `MessageContext::error_marking`. The alternative - restating the prefix in the
  dialect - is the defect this task fixed.

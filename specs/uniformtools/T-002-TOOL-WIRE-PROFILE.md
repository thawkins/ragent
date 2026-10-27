# T-002 - The declared wire contract type

Spec `uniformtools`, task T-002. Requirement FR-002 (every LLM client declares a
`ToolWireProfile`; a client constructed without one **shall** fail to compile; a
test **shall** assert the set covers every registered provider id).

Status: **completed**. Dependency: T-001 (the two canonical type additions).

## 1. What the task asks for

FR-002 names nine decisions and one compile-time property:

> Every LLM client **shall** declare a `ToolWireProfile` covering tool envelope,
> result carrier, id field, argument encoding, stream keying, parallel support,
> `is_error` support, reasoning echo, and synthetic-id need. A client constructed
> without a profile **shall** fail to compile. A test **shall** assert that the set
> of profiles covers every registered provider id.

D2 of the spec states the same nine fields as a table. T-002 is the **type** half
of the requirement; the per-family values and the id-to-family table are T-003,
and each encoder that reads a field is a later task.

## 2. What is in the tree

`crates/ragent-llm/src/providers/wire/mod.rs` declares the contract:

| Item | Shape | Purpose |
|---|---|---|
| `ToolEnvelope` | 5 variants | the wrapper a tool *definition* goes in (FR-003) |
| `ResultCarrier` | 4 variants | where a tool *result* is emitted (FR-006) |
| `IdField` | `Key(&'static str)` / `Synthesised` | the JSON key the call id occupies (FR-006, FR-019) |
| `ArgsEncoding` | `JsonString` / `JsonObject` | the argument-JSON shape (FR-007) |
| `StreamKeying` | `ByIndex` / `ByCallId` | the key streamed deltas are attributed by (FR-004) |
| `ToolWireProfile` | 9 `pub` fields, `Debug + Clone + Copy + PartialEq + Eq + Hash` | the contract itself |

The nine fields, in declaration order, are exactly the nine FR-002 names:
`tool_envelope`, `result_carrier`, `id_field`, `args_encoding`, `stream_keying`,
`supports_parallel`, `supports_is_error`, `echo_reasoning`, `synthetic_ids`.

Four properties of the declaration are deliberate, and each is now pinned by a
test:

1. **Nothing is optional and nothing is defaulted.** There is no `Default` impl
   for `ToolWireProfile`, no `Option<...>` field and no optional constructor. That
   is what makes FR-002's compile-time clause true rather than aspirational: a
   client that does not state a profile has nothing to inherit, and a provider
   registered without a row in
   [`PROVIDER_WIRE`](../../crates/ragent-llm/src/providers/wire/profile_table.rs)
   fails T-003's coverage assertion instead of encoding with a plausible guess.
2. **It is not configuration.** No field carries a serde attribute and the type is
   not deserialised: a profile is a statement of what a provider's wire format
   *is*, not a knob. A defaulted or configured profile would let a build claim a
   contract its client does not speak.
3. **Every value type has an ASCII label.** `as_str()` on each enum returns a
   fixed lower-case ASCII word, which is what the wire matrix (FR-016) prints
   verbatim; NFR-005 forbids a non-ASCII or blank cell.
4. **Labels are never blank, even for the id-less case.** `IdField::Synthesised`
   prints `synthesised` rather than `""`, so the matrix cannot show an empty cell
   for the one family that supplies no id.

Three of the nine fields name a consumer that is a later task
(`supports_parallel` - T-015, `supports_is_error`'s encoder - T-017,
`echo_reasoning` - T-018; `synthetic_ids` is already read by T-008's accumulator).
All nine are declared here anyway, because a profile is the whole statement of a
provider's terms: a field added per task would mean a client encoded requests it
never described. The doc comment records each field's consumer and task, so the
declaration doubles as the map from decision to feature.

## 3. What this task did not touch

- **The values.** Six family `const`s and the 15-row `PROVIDER_WIRE` table already
  exist in `profile_table.rs` (they landed with `d6abcd4a`) and T-003 owns their
  assertion. This task did not change a single value.
- **The consumers.** `encode_tools`, `encode_message`, `mark_error`,
  `ToolCallAccumulator::new` and the TUI/agent call sites all read fields already;
  none was edited.
- **`Default`-adjacent escape hatches elsewhere.** `MessageContext` (in
  `wire/encode.rs`) does implement `Default`, which is correct: it carries the four
  *call-site* facts a profile cannot (tool names, name prefix, image permission,
  name mirroring), not a wire decision.

## 4. Proof

`crates/ragent-llm/tests/test_wire_profile.rs` grew from 9 to 12 cases (255 added
lines). The three new cases pin the properties a value-level test cannot reach:

| Case | Asserts |
|---|---|
| `..._declares_exactly_the_nine_named_decisions` | the declaration's `pub` fields are exactly the nine FR-002 names, in order; no field is `Option<...>`; no serde attribute appears on the type; every requirement (FR-003/004/006/010/011/014/019) is named by some field's documentation |
| `..._has_no_default_impl_anywhere_in_the_crate` | the derive list above the declaration does not contain `Default`, and no file under `crates/ragent-llm/src` contains `impl Default for ToolWireProfile` or `ToolWireProfile::default()` - so the escape cannot be added quietly in a later task while T-003's doc comment claims it does not exist |
| `..._is_send_sync_static_and_fully_derived` | `ToolWireProfile: Send + Sync + 'static` and `Copy + Eq + Hash + Debug` hold as bounds (`LlmClient: Send + Sync`, and a client holds a profile as a field); the six derives are present in the declaration; two equal profiles hash equally and two different contracts do not collide |

It also gained a fourth case that closes a real gap in the pre-existing file:

| Case | Asserts |
|---|---|
| `..._representatives_match_the_declared_table_constants` | each of the file's five representative profiles equals the family `const` T-003 declares for it, and `synthetic_ids` agrees with `id_field` on **every** `PROVIDER_WIRE` row |

That case matters because every existing case in the file asserts against a
**local copy** of a family profile rather than the declared constant (deliberately:
a `const` lets the compiler fold the assertion). Freedom to copy is only sound while
the copy still equals the constant - a drifted copy would make eight passing cases
true of a contract nothing speaks.

`crates/ragent-llm/tests/test_wire_profile_table.rs` had its module docs corrected:
it described the no-`Default` guarantee and disclaimed asserting it, and pointed at
"T-003 declares the contract" when the contract is T-002's. The docs now name T-002
for the type and T-003 for the table, and point at the case that does assert the
guarantee.

### Red proofs

All four were run, observed red, and reverted byte-identically (`md5sum` before and
after each):

| Break | Failing case | Message observed |
|---|---|---|
| `impl Default for ToolWireProfile` appended to `wire/mod.rs` | `..._has_no_default_impl_anywhere_in_the_crate` | "wire/mod.rs introduces `impl Default for ToolWireProfile`; a client without a profile must fail to compile rather than inherit one (FR-002)" |
| field order swapped (`echo_reasoning` moved after `synthetic_ids`) | `..._declares_exactly_the_nine_named_decisions` | `left == right` with the two fields transposed |
| `(FR-010)` deleted from `supports_parallel`'s doc | `..._declares_exactly_the_nine_named_decisions` | "no field documents FR-010; every decision names the requirement it serves" |
| `GEMINI.synthetic_ids` flipped to `false` | `..._representatives_match_the_declared_table_constants` | "the `gemini` representative in this file has drifted from the declared family constant" |
| `OLLAMA_NATIVE.id_field` changed to `Key("tool_call_id")` | `..._representatives_match_the_declared_table_constants` | "`ollama_cloud`: `synthetic_ids` and `id_field` must agree ... (FR-019)" |

A fifth break was tried and rejected as a bad test rather than a good one:
`Default` added to the `#[derive(...)]` list makes
`test_wire_profile_is_send_sync_static_and_fully_derived`'s `Hash`-of-two-contracts
assertion compile-fail first, so it proves nothing the `impl Default` break does not
already prove more directly.

The derive check itself needed a fix found by that first run: the type's own
documentation says "no `Default` impl", so scanning the raw text above the
declaration gave a false positive. The scan now walks the contiguous `#[...]`
attribute run immediately above `pub struct`, skipping doc comments.

## 5. Verification

Green on the tree as it stands:

- `cargo fmt --all -- --check` - clean
- `cargo test -p ragent-llm --test all test_wire_profile` - **23 passed**, 0 failed
  (12 in this file, 11 in `test_wire_profile_table`)
- `cargo clippy -p ragent-llm --test all` - no new warning (19 pre-existing
  `use_self` warnings in `tool_cache.rs`, untouched here)
- 7 guard scripts PASS: `check-dead-code-reasons`, `check-inline-tests`,
  `check-test-env-isolation`, `check-silent-errors`, `check-security-unwraps`,
  `check-poison-locks`, `check-shared-guards`

### Pre-existing, not caused by this task

Recorded because a reader will see them and should not attribute them here. Both
were reproduced with this task's diff stashed and the tree otherwise unchanged:

1. `cargo test -p ragent-llm --test all` fails 5 cases, all of them source-scan
   guards for **unimplemented** tasks: `test_wire_decode::*` (2, T-008's
   accumulator not yet adopted by `anthropic.rs`/`gemini.rs`'s `format!("fc_`),
   `test_wire_encode_message::test_no_provider_module_writes_a_result_carrier_literal`
   (T-006), and `test_wire_trace::*` (2, T-023's payload trace). 291 passed.
2. `cargo check -p ragent-llm --all-targets` fails to compile the **lib test**
   target on `tests/inline/anthropic_tests.rs:50` and `:267` (`ChatContent` not in
   scope). That file is included from inside `anthropic.rs`, whose `use` list does
   not import `ChatContent`; it is unrelated to the wire module. The integration
   target (`--test all`) compiles and runs normally.

## 6. Note for the downstream tasks

- **T-003** should not re-add the nine-field assertion: it is
  `test_wire_profile_declares_exactly_the_nine_named_decisions`. T-003's job is the
  *values* (one `const` per family) plus the coverage assertion, and the
  representative-matches-constant case is the bridge between this file's copies and
  its constants - extend it rather than duplicating it when a seventh family
  appears.
- **T-015 / T-017 / T-018** now have a documented destination: each of
  `supports_parallel`, `supports_is_error` and `echo_reasoning` names its consumer
  in the type's docs, so a change to what a field *means* has one place to say so.
- **If a tenth decision is ever needed**, three things fail together and that is
  intended: the field-set assertion here, `HEADINGS` in `wire/matrix.rs` (which has
  one entry per printed column and is `[&str; 10]` for provider + nine fields), and
  the row arrays in `matrix::render_rows`. Adding a decision therefore cannot
  silently leave the printed table describing the old contract.

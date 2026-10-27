# T-003 - One profile per provider family, and the coverage test (FR-002)

Spec `uniformtools`, task T-003. Requirement FR-002, validated by TESTPLAN case
TC-002.

Status: **completed**. Dependency: T-002 (the contract type).

## 1. What the task asks for

> Define one profile per provider family plus a coverage test asserting every
> registered provider id has a profile.

FR-002's own sentence is the standard the task is measured against:

> Every LLM client **shall** declare a `ToolWireProfile` ... A test **shall**
> assert that the set of profiles covers every registered provider id.

T-002 built the *type*; this task is the *values* plus the assertion. The values -
six family `const`s and the 15-row `PROVIDER_WIRE` table - landed with `d6abcd4a`
and are not re-derived here. What this task found is that the values as delivered
could only assert **half** of FR-002, and that the missing half was structural
rather than a missing `assert!`.

## 2. What was actually missing

`PROVIDER_WIRE` is a many-to-one map: eight provider ids name `OPENAI_CHAT`. The
task's phrase "one profile per provider family" is a statement about the **left**
side of that map - there is one contract object per family, and rows point at it -
but the table alone cannot express it. Three things were true as delivered:

1. **The family set existed only as a repeated literal.** Four test cases each
   wrote the same six-element array of family constants by hand
   (`[OPENAI_CHAT, ANTHROPIC_MESSAGES, GEMINI, BEDROCK_CONVERSE,
   OPENAI_RESPONSES, OLLAMA_NATIVE]`). Nothing in the *library* named the set, so
   "one profile per family" was a convention the tests maintained by copying.
2. **A row could compose its own contract.** A row is
   `(id, ToolWireProfile)`; nothing stopped a row from writing a struct literal
   instead of naming a family. Such a provider would declare a seventh profile,
   pass every value-level case (it is a valid `ToolWireProfile`), and make the
   task's own sentence false.
3. **A seventh `const` could be added and never listed.** A new
   `pub const MAVERICK: ToolWireProfile` compiles, and every assertion about the
   table still passes, because the table only speaks about rows.

So the deliverable is `FAMILIES`, and three cases that pin it in both directions.

## 3. What landed

### 3.1 `FAMILIES` (library, `profile_table.rs`)

```rust
pub const FAMILIES: &[(&str, ToolWireProfile)] = &[
    ("anthropic_messages", ANTHROPIC_MESSAGES),
    ("bedrock_converse", BEDROCK_CONVERSE),
    ("gemini", GEMINI),
    ("ollama_native", OLLAMA_NATIVE),
    ("openai_chat", OPENAI_CHAT),
    ("openai_responses", OPENAI_RESPONSES),
];
```

Six entries, ascending, each label the family's `const` name in lower case. A slice
pair table rather than `[ToolWireProfile; 6]` for the same reason
`PROVIDER_WIRE` is a slice: the *label* is what a reader and a failure message
need, and an array of bare profiles would make a red assertion say
`profile 3` instead of `bedrock_converse`.

`FAMILIES` is read by tests only. Production paths read the family `const`s
directly (`anthropic.rs`, `tool_cache.rs`, `wire/decode.rs`'s doc) or go through
`profile_for`, so the list adds no indirection to a request path and no cost to a
turn. That is deliberate: a list on the hot path would be a fourth place the
mapping could drift, and FR-002 asks for a declaration, not a runtime lookup table
(there is already exactly one of those, `profile_for`).

### 3.2 The module docs now say what the list is for

`profile_table.rs`'s "Six families, fifteen providers" section now names
`FAMILIES` and states the reason the table needs it: the table is many-to-one, so
on its own it cannot tell a shared declared contract from fifteen literals that
happen to be equal today. `wire/mod.rs`'s crate-header link list gained the same
sentence, and `crates/ragent-llm/DOCS.md` names the list in the `wire` bullet.

## 4. The cases - 13 in `tests/test_wire_profile_table.rs`

Nine cases were already there and stayed (the two registry-coverage directions, the
count, uniqueness and ordering, lookup agreement, the per-provider spot checks, the
divergence axes, the fully-declared check, `Copy`/`const`). One was rewritten and
three are new.

| # | Case | What it pins |
|---|---|---|
| 1 | `..._covers_every_registered_provider` | FR-002: every id `create_default_registry()` registers has a row |
| 2 | `..._has_no_row_for_an_unregistered_provider` | the safe direction: no dead row claims a contract nothing speaks |
| 3 | `..._and_registry_have_the_same_size` | the counts, so a failure reports the numbers |
| 4 | `..._ids_are_unique_and_ascending` | the table is a table |
| 5 | `..._lookup_agrees_with_the_slice` | `profile_for` and the slice cannot disagree; no catch-all |
| 6 | **`..._declares_every_family_once`** (new) | `FAMILIES` holds 6 entries, each one named by some row, each pair distinct, each label non-blank / ASCII / lower-case-underscore / ascending |
| 7 | **`..._every_row_names_a_declared_family`** (new) | every `PROVIDER_WIRE` row's profile equals a `FAMILIES` entry - a row is an assignment, never a contract composed at the row |
| 8 | **`..._source_declares_no_family_outside_the_list`** (new) | scans `profile_table.rs`: the number of `pub const X: ToolWireProfile` declarations equals `FAMILIES.len()`, and each one is listed by its lower-cased name |
| 9 | `..._assigns_each_provider_its_declared_family` | the assignments a reader is most likely to question (`ollama` vs `ollama_cloud`, `huggingface`, `bedrock`, `azure_resource`, `router`) |
| 10 | `..._families_differ_where_the_tree_diverges` | argument encoding, stream keying, `synthetic_ids`/`id_field`, `supports_is_error` and `echo_reasoning` per family |
| 11 | `..._every_row_is_fully_declared` | no blank or non-ASCII cell on any row (NFR-005, TC-002) |
| 12 | `..._envelopes_and_carriers_agree_with_the_family` (**rewritten**) | the envelope/carrier pair is one a declared family uses - now read from `FAMILIES` instead of a repeated literal |
| 13 | `..._profiles_are_copy_and_const` | a row can be read twice; `Some(profile) == profile_for(id)` |

Cases 6, 7 and 8 are three different failures, not three spellings of one:

- 6 fails when the **list** drifts (a family constant dropped, a label spoiled, a
  duplicate added).
- 7 fails when a **row** invents a contract the list does not contain.
- 8 fails when the **module** gains a family the list never learned about.

Case 12's rewrite matters more than it looks: while it held its own copy of the
six constants, a seventh family could be declared, listed and used by a row, and
case 12 would still have reported that the pair was undeclared. Reading `FAMILIES`
makes the three cases share one source of truth.

## 5. Red proofs

Each break was applied to a backed-up file, observed red, and restored
byte-identically (`md5sum` before and after; the restored file was `touch`ed so
cargo could not reuse the broken build's object file).

| Break | Case that failed | Message observed |
|---|---|---|
| `pub const MAVERICK: ToolWireProfile { ... }` added to `profile_table.rs` | `..._source_declares_no_family_outside_the_list` | "`MAVERICK` is declared in profile_table.rs but not listed in FAMILIES, so the wire matrix would never print it and no row could name it (FR-002)" |
| `("xai", ToolWireProfile { supports_parallel: false, ..OPENAI_CHAT })` | `..._every_row_names_a_declared_family` | `"xai" declares a profile no family declares: OpenAiFunction / ToolRoleMessage / Key("tool_call_id")` |
| the `("gemini", GEMINI)` row deleted | 4 cases together: `..._declares_every_family_once`, `..._every_row_names_a_declared_family`, `..._envelopes_and_carriers_agree_with_the_family`, `..._source_declares_no_family_outside_the_list` | the first reports `"gemini" (...) is not named by any PROVIDER_WIRE row` |
| the `("router", OPENAI_CHAT)` row deleted | `..._covers_every_registered_provider` | `FR-002: provider "router" is registered but declares no ToolWireProfile; add a row to PROVIDER_WIRE in ...` |
| a `FAMILIES` label changed to `"Gemini"` | `..._declares_every_family_once` | `the family label "Gemini" is not the const name in lower case` |

Two of these are worth reading closely.

**The `xai` break proves case 7 is not redundant with case 12.** The composed row
was a valid profile whose envelope/carrier pair `OPENAI_CHAT` declares, so
`..._envelopes_and_carriers_agree_with_the_family` stayed **green**; only the
new family-membership case caught it. That is the exact shape of the hole the
task's wording ("one profile per provider family") describes.

**The `gemini`-row deletion failed case 8 too**, which is the honest result rather
than a surprise: removing the *only* row that names `GEMINI` leaves a family
constant that nothing uses, and case 8 checks the count in both directions. The
list is not merely a duplicate of the rows.

A sixth break was run and **rejected as proving nothing about `FAMILIES`**:
renaming a row's id (the `("gemini", GEMINI)` row to `("gemini_pro", GEMINI)` in
`PROVIDER_WIRE`) leaves the family list untouched, so it fails
`..._covers_every_registered_provider` - `FR-002: provider "gemini" is registered
but declares no ToolWireProfile` - which is case 1 doing its own job. It was
observed and is not counted here.

## 6. Verification

Green on this tree:

- `cargo fmt --all -- --check` - clean
- `cargo test -p ragent-llm --test all test_wire_profile` - **25 passed**, 0 failed
  (12 in `test_wire_profile.rs`, 13 here)
- `cargo test -p ragent-llm --doc` - **30 passed**, 0 failed
- `cargo clippy -p ragent-llm --lib` / `--test all` - no new warning (the 19
  `unnecessary structure name repetition` warnings in `tool_cache.rs` are
  pre-existing and untouched); `--all-targets` still fails to compile the **lib
  test** target on `tests/inline/anthropic_tests.rs:50`/`:267`
  (`ChatContent` not in scope), pre-existing and unrelated
- 7 guard scripts PASS: `check-dead-code-reasons`, `check-inline-tests`,
  `check-test-env-isolation`, `check-silent-errors`, `check-security-unwraps`,
  `check-poison-locks`, `check-shared-guards`

### Pre-existing, not caused by this task

`cargo test -p ragent-llm --test all` reports **5 failed / 293 passed / 2 ignored**.
All five are source-scan guards for work this task does not own, reproduced with
this task's diff stashed:

| Failing case | Owner |
|---|---|
| `test_wire_decode::test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one` | T-010 / T-011 |
| `test_wire_decode::test_the_provider_modules_no_longer_keep_their_own_tool_call_maps` | T-008 |
| `test_wire_encode_message::test_no_provider_module_writes_a_result_carrier_literal` | T-006 |
| `test_wire_trace::test_agent_loop_sets_the_switch_from_config` | T-023 |
| `test_wire_trace::test_every_provider_records_its_body` | T-023 |

## 7. Notes for the downstream tasks

- **A seventh family is now a three-part change, and each part fails loudly if
  missed:** the `const`, the `FAMILIES` entry, and at least one `PROVIDER_WIRE`
  row. Cases 6/7/8 fail on the missing part by name.
- **`FAMILIES.len() == 6` is asserted**, deliberately. A new wire format is a
  contract change that should make a test read the reason, not a silent seventh
  entry: raise the number in case 6 in the same commit that adds the family, and
  the diff says why.
- **T-022's `--wire-matrix` is unaffected.** It iterates the *registered provider
  ids* and looks each up with `profile_for`, so it neither reads `FAMILIES` nor
  needs to: the matrix is per provider, and this list is per family.
- **T-015/T-017 must resolve the `router` row** before acting on a field for a
  router session - the row is nominal (`OPENAI_CHAT`) because `RouterClient`
  forwards the canonical request to a downstream provider that encodes with its
  own profile. `FAMILIES` does not change that; it is the table's own documented
  caveat, restated in section 2 of the module docs.

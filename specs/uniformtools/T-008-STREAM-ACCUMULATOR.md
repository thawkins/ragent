# T-008 - One `ToolCallAccumulator` keyed by the profile's `StreamKeying`

Spec `uniformtools`, task T-008. Requirement FR-004 (one keyed stream accumulator),
with FR-005 (parallel calls stay independent) as the reason keying exists at all.

## 1. What the tree already held, and what this task actually changed

The accumulator itself is **not** this task's delta. `crates/ragent-llm/src/providers/wire/decode.rs`
(559 lines) landed with `d6abcd4a` and already carries the whole delivered API:

- `StreamKey::{Index, CallId}` and `StreamKey::from_frame(index, id)` - the one place a
  provider decides which of the two its frame supplies (an index wins, then a non-empty
  id, else the frame names no call);
- `ToolCallAccumulator::new(profile, provider)`, `keying`, `open_count`, `deferred_count`,
  `set_id`, `open`, `delta`, `end`, `end_all`, `defer`, `flush_deferred`;
- strict-keying semantics: a delta whose key names no open call is **dropped** with a
  `tracing::debug!`, never folded onto "the call I opened last".

`git diff` against `HEAD` for `wire/decode.rs` is **0 lines** after this task, and
`grep delta_last|end_last` over the file returns nothing - the two last-opened helpers the
task description names do not exist in this tree, because the strict-keying rule was
already in place. That is worth stating plainly rather than writing a record that
describes helpers nobody can find.

What this task therefore did is the **conversion half**: nine provider stream parsers, all
of which still held their own answer to "which open tool call does this frame belong to",
now delegate to the accumulator.

### One deliberate deviation from the task description

The description says "the Converse path keeps last-opened attribution via
`delta_last`/`end_last` because per-block keys are T-009 (FR-005)". This tree cannot do
that, because T-009's *test* had already landed even though its source work had not:
`crates/ragent-llm/tests/test_wire_decode.rs:400`
(`test_no_stream_attributes_a_frame_to_the_call_opened_last`) scans `wire/decode.rs`,
`bedrock.rs`, `anthropic.rs`, `gemini.rs` and `sse.rs` and **fails on the strings
`delta_last`, `end_last` and `open.last_mut()`**. Adding either helper here would have made
that case red, and the only way to have both is to weaken an existing assertion - which is
not an option.

So this task does what T-009's rule requires of the two Bedrock paths: key by the index the
frame carries, and **skip with a warning** a frame that carries none (section 6). Nothing in
the tree calls a last-opened helper, `wire/decode.rs` gains none, and
`test_no_stream_attributes_a_frame_to_the_call_opened_last` passes because the thing it
forbids does not exist. T-009 is left with its behaviour-proof half (the two parallel-block
cases in `bedrock_tests.rs`), not with a deletion it would find already done. Ten construction sites (`bedrock.rs` has two clients),
five distinct open-call representations deleted, and one per-provider contract opt-out
removed.

At `HEAD`, `crates/ragent-llm/tests/test_wire_decode.rs:340`
(`test_the_provider_modules_no_longer_keep_their_own_tool_call_maps`) was **red**, failing
on `anthropic.rs does not use the shared accumulator`, because the test was written with
the conversion and the conversion was not applied. It is green now. That is this task's
own red-to-green.

## 2. The five representations that were there

| Module | Structure | Failure it caused |
|---|---|---|
| `anthropic.rs` | `BTreeMap<u64, String>` of open blocks | correct after F5; the only module that got parallel blocks right |
| `sse.rs` (OpenAI / OpenRouter / Azure Foundry) | `HashMap<u64, String>` **plus** `HashSet<u64>` of started indices | two structures for one question, and a per-provider `dedup_tool_call_start` switch so OpenRouter could *opt out* of the de-duplication FR-004 requires |
| `bedrock.rs` (Anthropic path) | `HashMap<String, String>` keyed by id | `iter_mut().last()` and `iter().last()` attributed every `input_json_delta` and every `content_block_stop` to an arbitrary open block |
| `bedrock.rs` (Converse path) | `active_tool_call_id: String` | one slot for N calls: a second parallel `toolUse` block overwrote the first |
| `gemini.rs` | `Vec<(id, name, args)>` + `next_tool_call_id` | a fifth representation, with its own `fc_<name>_<n>` id minting |
| `copilot.rs`, `huggingface.rs`, `ollama.rs`, `ollama_cloud.rs` | a copy of the `sse.rs` map + drain-at-finish loop, or a `HashMap<String, String>` | five copies of the same loop, each drifting separately |

Every one of them emitted the same three `StreamEvent`s. The divergence was in the
bookkeeping, not the contract - which is exactly what a declared profile should own.

## 3. The ten sites, and what each lost

| Module | Profile | What went away |
|---|---|---|
| `anthropic.rs` | `ANTHROPIC_MESSAGES` | `BTreeMap<u64, String> tool_call_blocks`; `BTreeMap` import |
| `azure_resource.rs` | `ANTHROPIC_MESSAGES` | the same `BTreeMap` block map; `BTreeMap` import |
| `bedrock.rs` (Anthropic path) | `ANTHROPIC_MESSAGES` | `HashMap<String, String> tool_call_args` and both `iter().last()` lookups |
| `bedrock.rs` (Converse path) | `BEDROCK_CONVERSE` | `active_tool_call_id: String` |
| `sse.rs` | `OPENAI_CHAT` | `HashMap<u64, String>`, `HashSet<u64> started_tool_call_indices`, **and `dedup_tool_call_start`** |
| `copilot.rs` | `OPENAI_CHAT` | `HashMap<u64, String>` + drain-and-sort loop |
| `huggingface.rs` | `OPENAI_CHAT` | the same map; the `t_` strip now happens to the name passed to `open` |
| `ollama.rs` | `OPENAI_CHAT` | `BTreeMap<u64, String>` + drain loop |
| `gemini.rs` | `GEMINI` | `Vec<(String, String, String)> pending_tool_calls`, `next_tool_call_id`, and two hand-written emit loops |
| `ollama_cloud.rs` | `OLLAMA_NATIVE` | `HashMap<String, String> open_tool_calls` on both the frame path and the `done` frame, and its `drain()` end loop |

Two details are load-bearing rather than cosmetic:

- **`dedup_tool_call_start` is deleted, not defaulted to `true`.** FR-004 says exactly one
  start per call; a provider switch that could turn that guarantee off is a contract with
  an opt-out. OpenRouter's old "emit duplicate starts" behaviour was not something any
  caller wanted - it was drift.
- **`sse.rs` `provider_label` is now also the provider id** a synthesised call id is built
  from, so the label is an input rather than a diagnostic string (FR-019).

## 4. What deliberately did not change

- **`StreamEvent`** - no field added, no variant added. The accumulator emits the same
  three events the providers always emitted, so the agent loop, the TUI and every existing
  provider test compile unchanged.
- **`tc_<index>` fallback** and **Gemini's `fc_<name>_<n>` / Ollama Cloud's
  `<x>_tc_<idx>`** - the fallback *spellings* are FR-019's subject (T-010), so they stay
  where they are. `sse.rs` now passes `None` and lets the accumulator mint, because its
  `format!("tc_{index}")` would otherwise have been a second copy of a shape the
  accumulator owns; the other three pass an explicit id through `open`/`defer` and are
  re-keyed by T-010, not here.
- **Id-less Ollama Cloud calls** keep their frame-local `_tc_<idx>` spelling, so behaviour
  is unchanged; the frame-local index restarting at zero is a real defect but it is
  FR-019's, and fixing it here would land two tasks' test churn in one diff.
- **`sse.rs`'s FUNC-032 index requirement** - a `tool_call` delta with no `index` is still
  skipped with a warning rather than folded onto stream 0.
- **`wire/decode.rs`** - byte-identical before and after (`md5sum` unchanged).

## 5. Tests

`crates/ragent-llm/tests/test_wire_decode.rs` holds 19 cases; the eleven below are this
task's (the rest are T-009 to T-011's, whose requirements are separate):

1. `by_index_attributes_interleaved_deltas_to_their_own_block` - two blocks open,
   fragments alternate, each lands on the block its frame names; closed in reverse order
   by key. The FR-005 centre of the file.
2. `emits_exactly_one_start_and_one_end_per_call` - a repeated name frame emits no second
   start (and still yields its delta); a repeated stop emits no second end.
3. `by_call_id_keying_tracks_the_responses_family` - two `CallId` calls stay separate and
   `end_all` keeps open order.
4. `end_all_closes_by_index_families_in_index_order` - opened 3, 1, 2; ended 1, 2, 3; a
   second `end_all` is empty.
5. `drops_a_delta_that_names_no_open_call` - no invented call, no invented end.
6. `synthesises_provider_counter_name_ids_for_unnamed_calls` - an index-only frame still
   gets an id, two unnamed calls at one index do not share it, an underscore in the tool
   name is not a separator, and a nameless frame still gets a well-formed id.
7. `pins_the_start_id_so_a_later_id_frame_cannot_split_the_call`.
8. `holds_a_whole_deferred_call_until_it_is_flushed` - two deferred calls, not counted as
   open, six events on flush, nothing on a second flush.
9. `stream_key_from_frame_prefers_the_index_and_falls_back_to_the_id`.
10. `every_profile_keying_is_served_by_the_accumulator` - walks every `PROVIDER_WIRE` row
    and asserts the accumulator takes the keying that row declares.
11. `the_provider_modules_no_longer_keep_their_own_tool_call_maps` - the mechanical half:
    reads the nine converted modules and fails on `tool_call_blocks`, `tool_call_ids`,
    `tool_call_args`, `active_tool_call_id`, `pending_tool_calls`,
    `started_tool_call_indices`, `open_tool_calls`. This is the case that was red at
    `HEAD` and is the task's own guard.

`crates/ragent-llm/tests/inline/bedrock_tests.rs`'s Converse case was updated rather than
weakened: its frames now carry the `contentBlockIndex` that a frame naming no block cannot
have (see section 6), and every original assertion is unchanged.

## 6. The rule a frame with no block identity now follows

Both Bedrock paths key by the index the frame carries and **skip** a frame that carries
none, with a `tracing::warn!` (FUNC-032) - the same rule `sse.rs` already applied to a
`tool_calls[]` entry with no `index`. AWS documents `contentBlockIndex` as required on all
three Converse events, so the skipped path is a malformed-frame path only, and skipping is
strictly better than the old behaviour: a missing fragment is a missing argument, while an
attributed-to-the-wrong-block fragment is a *wrong* argument for one call and a corrupted
one for the other.

The consequence is a **behaviour change**, so it is stated rather than hidden: before this
task a Converse delta with no `contentBlockIndex` was attributed to the last-opened call;
now it is dropped and logged. The alternative - keeping a last-opened helper in the shared
accumulator for one caller - is the FR-005 failure one method call away, which is why
`wire/decode.rs` has no such method at all.

## 7. Red proofs

Three breaks, each applied, the suite run, and the file restored byte-identically
(`cp -p` from `target/temp/t008/pre/`, `md5sum -c` verified, `touch`ed so cargo could not
reuse the broken object):

| Break | Result |
|---|---|
| `delta` finds `self.open.iter_mut().last()` instead of the call its key names | 5 cases red: `by_index_attributes_interleaved_deltas_to_their_own_block`, `by_call_id_keying_tracks_the_responses_family`, `a_converse_block_keys_by_content_block_index_not_by_id`, `rekeys_a_duplicate_call_id_and_keeps_the_first`, and the already-red T-010 scan |
| `open` drops its `started_id.is_some()` early return | 3 cases red: `emits_exactly_one_start_and_one_end_per_call`, `does_not_rekey_a_repeated_frame_of_the_same_call`, `warns_one_line_per_collision_and_not_per_frame` |
| `sse.rs` passes `""` instead of the tool name to `open` | `test_openrouter_provider::test_openrouter_chat_stream_maps_sse_events` and `..._emits_reasoning_events_with_transitions` red - `left: [("call_1", "")], right: [("call_1", "read")]` |

All nine provider files and `wire/decode.rs` verified byte-identical to their pre-proof
state afterwards (`md5sum` against `target/temp/t008/pre/`).

## 8. Measurements

| Item | At `HEAD` | Now |
|---|---|---|
| `wire/decode.rs` | 559 lines | 559 lines (0-line diff) |
| distinct open-call representations under `providers/` | 5, plus 5 copies of the OpenAI loop | 1 (`ToolCallAccumulator`) |
| `ToolCallAccumulator::new` sites | 0 | 10 over 9 modules |
| per-provider de-duplication switch | 1 (`dedup_tool_call_start`, opt-out) | 0 |
| `dedup_tool_call_start` occurrences in `src/` | 4 | 0 |
| retained bookkeeping names in the nine modules | 52 | 0 |
| `test_wire_decode::test_the_provider_modules_...maps` | **red** | green |

Per-file line deltas (`HEAD` -> now, added/removed): `anthropic.rs` 25/27,
`azure_resource.rs` 22/24, `bedrock.rs` 128/43 (the badly indented match arms were
re-indented while being edited), `copilot.rs` 18/23, `gemini.rs` 24/38, `huggingface.rs`
24/24, `ollama.rs` 18/24, `ollama_cloud.rs` 30/25, `sse.rs` 31/43.
`wire/mod.rs`, `wire/profile_table.rs`, `mock_llm_client.rs`, `test_wire_profile.rs`,
`test_wire_profile_table.rs` and `tool_cache_tests.rs` also differ from `HEAD` but belong
to T-002/T-003/T-004 and were not touched by this task.

Verification:

- `cargo fmt --all -- --check` - clean.
- `cargo check --workspace --all-targets` - clean, no new warning.
- `cargo clippy -p ragent-llm --all-targets` - the 19 findings are all pre-existing
  `unnecessary structure name repetition` in `providers/tool_cache.rs`; no finding in any
  file this task touched.
- `cargo test -p ragent-llm --lib` - 323 passed.
- `cargo test -p ragent-llm --doc` - 30 passed.
- `cargo test -p ragent-llm --test all` - **294 passed, 4 failed, 2 ignored**. The count
  went 293 -> 294 passed and 5 -> 4 failed against the T-004 baseline, and the case that
  moved is this task's own guard. The four remaining failures are all owned by later,
  still-`pending` tasks and none is in a file this task touched:
  `test_wire_decode::test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one`
  (T-010: `gemini.rs` still splits `tool_use_id` and `ollama_cloud.rs` still says
  `_tc_{`), `test_wire_encode_message::test_no_provider_module_writes_a_result_carrier_literal`
  (T-006: the `"tool_result"` literal at `bedrock.rs:472`, untouched here),
  `test_wire_trace::test_agent_loop_sets_the_switch_from_config` and
  `..._every_provider_records_its_body` (T-023).
- all twelve `scripts/check-*.sh` guards pass.
- no dependency added (NFR-002), no `unsafe` block (NFR-003).

## 9. What this hands to the next tasks

- **T-009** (FR-005) makes the index mandatory on the frames that still may omit it - the
  last structural guess left is `StreamKey::from_frame`'s id fallback on a start frame, and
  `bedrock.rs`'s two warn-and-skip arms are already the shape that task wants.
- **T-010** (FR-019) moves every remaining id spelling into the accumulator:
  `gemini.rs`'s `fc_<name>_<n>`, `ollama_cloud.rs`'s `_tc_<idx>` (whose frame-local index
  restarts at zero and folds unnamed calls together), and `sse.rs`'s `provider_label` as
  the id prefix. It also deletes `gemini.rs`'s `tool_use_id.split('_')` name recovery,
  which is the reason the T-010 source scan is still red.
- **T-011** (FR-020) reconciles duplicate ids inside `open`, which is where an id first
  becomes known.
- **T-026** (NFR-004) can replay recorded frames through `ToolCallAccumulator` directly: it
  is a pure state machine over `StreamKey`s with no I/O and no clock.

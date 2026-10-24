# T-011 - A call id the provider reuses inside one response is reconciled

Spec `uniformtools`, task T-011. Requirement FR-020:

> If two tool calls in one response carry the same id, the system **shall** keep
> the first, **shall** re-key the later ones to unique synthetic ids, and
> **shall** log one `tracing::warn!` per collision.

## 1. The failure this closes

`ToolCallAccumulator::open` emitted a start at most once per *key*. Two different
keys carrying the same provider id were two separate calls to the accumulator, so
both starts were emitted - under the same id. Everything downstream keys on that
id:

- `SessionProcessor`'s stream loop pushes `PendingToolCall { id, name, args_json }`
  on a start and finds the call to append a fragment to with
  `tool_calls.iter_mut().find(|t| t.id == id)` - the second start therefore
  **overwrites** the first call's record rather than adding a second;
- the history writes one `MessagePart::ToolCall { tool, call_id, state }` per
  pending call, and the result encoder resolves the tool a result answers by
  scanning that history for the id.

So with a duplicated id one call's arguments were replaced by the other's and only
one result could ever be returned. The user-visible shape is TC-016: two tool
blocks asked for, one executed, and its result reported against both - or, in the
worst case, a result written to the wrong tool.

The provider cannot be relied on to avoid it. TC-016's test data asks a local model
to "give both calls the same id `call_dup`", which is exactly what a small local
model does; a server replaying a cached frame produces the same shape without being
asked.

## 2. The change

All of it is inside `crates/ragent-llm/src/providers/wire/decode.rs`:

- **A per-response ledger.** `ToolCallAccumulator` gained
  `emitted_ids: Vec<String>` - every id it has emitted a start under in this
  stream. A `Vec` rather than a `HashSet`: a response carries a handful of calls of
  which at most one or two collide, so a linear `contains` is cheaper than hashing
  and keeps the accumulator's allocation count the same.
- **The first claim keeps the id; the rest are re-keyed.** `open` now computes the
  requested id (the provider's own, or the one FR-019 mints when the provider named
  none) and hands it to a new `start_id(requested, name)`. If the ledger has not
  seen it, it is recorded and returned unchanged. If it has, a fresh
  `<provider>_<counter>_<name>` id is minted and recorded instead, and one
  `tracing::warn!` is logged - naming the provider, the duplicated id, the
  replacement and the tool.
- **One warning per collision, not per frame.** The warning is inside `start_id`,
  which is reached only on the branch that emits a start. A provider that repeats
  the whole start frame on every step takes the `started_id.is_some()` early return
  in `open` and never reaches `start_id` at all, so it cannot multiply the line.
  TC-016 step 5 ("exactly one such warning line, not one per subsequent step") is
  the requirement this satisfies.
- **A repeated frame is still idempotence, not a collision.** For a
  `StreamKeying::ByCallId` family the id *is* the key, so a repeated frame resolves
  to the same `OpenCall` and returns `None`; re-keying there would invent a call
  out of a repeated frame. This is asserted directly.
- **`synthetic_id` walks past used ids.** It now loops while the candidate is in
  `emitted_ids` before returning, because a provider is free to spell its own id
  exactly like ours (`openai_0_read`). Without that, the mint intended to resolve a
  collision would create one.
- **Logging split out.** `synthetic_id` only formats now; the FR-019 debug line
  moved to a new `mint_for_unnamed`, and the FR-020 warning lives in `start_id`.
  The two events tell different stories ("the provider named no id" vs "the
  provider reused an id") and a shared logger could only log one of them.

### The Azure Resource Anthropic branch (a tenth accumulator site)

`providers/azure_resource.rs`'s `AzureAnthropicClient` parsed the Anthropic Messages
shape with its own `BTreeMap<u64, String> tool_call_blocks`, emitting
`StreamEvent::ToolCallStart` **directly** - outside the accumulator entirely. Its
`content_block_stop` arm even skipped a block whose id was empty
(`if let Some(id) = ... && !id.is_empty()`), so a block the server left unnamed got a
start and no end. Every FR-004/FR-005/FR-019/FR-020 guarantee was therefore absent
on that path, and no FR-020 reconciliation could reach it.

The branch now builds `ToolCallAccumulator::new(ANTHROPIC_MESSAGES,
"azure_resource")` and routes its three arms through `open` / `delta` / `end`, exactly
as `anthropic.rs` does - the duplication T-008 and T-011 exist to delete. The
`BTreeMap` import went with it.

## 3. What did not change

- `StreamEvent` - still `ToolCallStart { id, name }`; no field added. The re-keyed
  id simply *is* the id on the record.
- The provider's own id is kept byte for byte on the first claim (FR-006).
- `defer`/`flush_deferred` - the ids are minted from the same monotonic counter, so
  they cannot collide with each other; they are recorded in the ledger so a
  provider id spelled like a synthetic one still cannot collide with them.
- `end` / `end_all` - the ledger deliberately survives a close, so two *sequential*
  calls in one response collide exactly as two parallel ones do. Dropping the entry
  on `end` would have made the second result unattributable.
- No dependency (NFR-002), no `unsafe` (NFR-003), no tool renamed.

## 4. Tests

`crates/ragent-llm/tests/test_wire_decode.rs` (19 cases, 5 new):

- `test_accumulator_rekeys_a_duplicate_call_id_and_keeps_the_first` - two index-keyed
  calls both announcing `call_dup`: the first start is `call_dup`, the second is
  `openai_0_read`; each keeps its own fragments; both ends are emitted.
- `test_accumulator_rekeys_a_duplicate_id_used_by_two_sequential_calls` - the first
  call is *closed* before the second arrives, and the id is still spent
  (`anthropic_0_Grep`), which is what keeps a sequential duplicate attributable.
- `test_accumulator_rekeys_a_collision_between_a_minted_id_and_a_provider_id` - a
  provider id spelled `openai_0_read`, then an unnamed call whose first candidate
  would be that same string: the mint advances to `openai_1_read`.
- `test_accumulator_does_not_rekey_a_repeated_frame_of_the_same_call` - a repeated
  `CallId` frame and a repeated `Index` frame each produce no event and no collision.
- `test_accumulator_warns_one_line_per_collision_and_not_per_frame` - the only case
  that can see the third clause. It installs a capturing `tracing::Subscriber`
  (rebuilt interest cache, `WARN` and above), drives one collision, repeats the same
  two frames three more times, adds a second collision of a different id, and
  asserts exactly **two** warnings, each naming its own `duplicate_id`, with a
  message that says the call was re-keyed.

`crates/ragent-llm/tests/test_openai_provider_extra.rs` (1 new) -
`test_openai_chat_rekeys_two_calls_that_share_one_id`: the same collision through the
real SSE parser and a local one-shot server, asserting two starts
(`call_dup`, `openai_0_read`), two deltas each carrying its own `path`, and two ends.

`crates/ragent-llm/tests/test_azure_resource_anthropic.rs` (1 new) -
`test_azure_anthropic_stream_uses_the_shared_accumulator_for_parallel_calls`: two
parallel `tool_use` blocks with the same id through `AzureAnthropicClient` against a
local server; starts are `toolu_dup` / `azure_resource_0_Grep`, and the interleaved
`input_json_delta` fragments reassemble to `{"path":"a.rs"}` and
`{"pattern":"fn main"}` under their own ids.

The two source scans in `test_wire_decode.rs` were extended to `azure_resource.rs`:
it joins the "no private tool-call map" list and the "no self-minted id / no name
recovery" list.

## 5. Red proofs

Each defect applied, the named case observed to fail, then the file restored from
`target/temp` and verified byte-identical by `md5sum` (and `touch`ed so cargo cannot
reuse the broken object file):

| # | Injected defect | Case that failed |
|---|---|---|
| 1 | `start_id`'s ledger check short-circuited to `if true` (every id keeps itself - the pre-T-011 behaviour) | `..._rekeys_a_duplicate_call_id_and_keeps_the_first`, `..._rekeys_a_duplicate_id_used_by_two_sequential_calls`, `..._warns_one_line_per_collision_and_not_per_frame` (`[]` warnings), and the end-to-end `test_openai_chat_rekeys_two_calls_that_share_one_id` |
| 2 | the `tracing::warn!` moved into `open`'s per-frame path so every repeated frame logs | `..._warns_one_line_per_collision_and_not_per_frame` (10 warnings, listing `call_dup` three times) |
| 3 | `synthetic_id`'s skip-used-ids loop deleted | `..._rekeys_a_collision_between_a_minted_id_and_a_provider_id` (`openai_0_read` where `openai_2_read` is required - the replacement was itself the duplicate) |
| 4 | `azure_resource.rs`'s `content_block_start` arm restored to the raw `StreamEvent::ToolCallStart` emit | `test_azure_anthropic_stream_uses_the_shared_accumulator_for_parallel_calls` (`toolu_dup` twice) |

Proof 3 needed its case rewritten before it proved anything. The first version of the
test drove one provider id (`openai_0_read`) and one *unnamed* call, which passes with
or without the walk: the unnamed call mints `openai_0_read` internally, `start_id`
then sees the collision and mints again from counter 1, so `openai_1_read` comes out
either way and the walk never iterates. The case now seats provider ids on both
consecutive counters first (`openai_0_read`, `openai_1_read`), then re-keys a third call
that repeats the first - so without the walk the replacement is a *third*
`openai_0_read` and the assertion fails, which is the proof recorded above. A red
proof that stays green is not a proof, and a test whose comment claims to exercise a
branch it never reaches is worse than no test.

## 6. Verification

- `cargo fmt --all -- --check` - clean.
- `cargo clippy -p ragent-llm --all-targets` - no warnings.
- `cargo check --workspace --all-targets` - clean.
- `cargo test -p ragent-llm --test all` - **287 passed**, 2 ignored (was 280).
- `cargo test -p ragent-llm --lib` - 327 passed; `--doc` - 30 passed.
- Guard scripts `check-inline-tests`, `check-silent-errors`, `check-poison-locks`,
  `check-security-unwraps`, `check-shared-guards`, `check-test-env-isolation`,
  `check-dead-code-reasons`, `check-file-tool-containment` - all PASS.

## 7. What is left in this area

- T-024 (FR-022) treats a tool-use finish with zero complete calls as truncation;
  the re-keyed call counts as complete, which is the point of emitting the start at
  all.
- T-026 can replay recorded frames through the accumulator directly (NFR-004); the
  duplicate case above is already a hand-written version of that replay.

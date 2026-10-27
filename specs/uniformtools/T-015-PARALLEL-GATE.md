# T-015 - Gate parallel dispatch on the profile

Spec `uniformtools`, task T-015. Requirement FR-010 (sequential execution when
the provider cannot parallelise).

Status: **completed**. Dependencies: T-014 (the one ordered dispatch pipeline) and
T-003 (the family profiles that declare `supports_parallel`).

## 1. What the task asks for

> **FR-010** (state-driven) While the active provider profile declares
> `supports_parallel == false`, the system **shall** execute the step's tool
> calls sequentially even when `experimental.parallel_tool_calls` is `true`, and
> **shall** log one `tracing::info!` naming the provider and the reason.

Two things are easy to get wrong here and the plan's risk note names the first:
the gate is a *demotion*. It never turns concurrency on, and it never removes
concurrency from a provider that declares `true`. The user's
`experimental.parallel_tool_calls` remains the only switch that can enable it.

The second is where the gate sits. T-014 left the loop with one ordered pipeline
reached through a slot list: `parallel_tool_calls` decides whether a call is
awaited inline (sequential arm) or pushed as a slot and drained afterwards
(concurrent arm). Both arms call the same pipeline with the same order, so
"forced sequential" is exactly "the flag is false before the first call of the
step is prepared" - a single assignment in the per-step derivation, not a second
dispatch path.

## 2. State found

| # | State | Where |
|---|---|---|
| 1 | `supports_parallel` was declared (T-002), set on all six family `const`s (T-003) and printed by the wire matrix (T-022), but **no consumer read it**. `profile_for` had exactly one caller in the whole tree: the matrix renderer. | `ragent-llm/src/providers/wire/profile_table.rs` |
| 2 | The per-step flag was `let parallel_tool_calls = turn.session_config.experimental.parallel_tool_calls;` - the config's value and nothing else. | `ragent-agent/src/session/processor.rs` |
| 3 | A source-shaped guard already existed at `crates/ragent-agent/tests/test_parallel_gate.rs` (added in `d6abcd4a`), asserting the strings the gate would contain. It was **not included** in `tests/all.rs`, so it never ran: the file was dead. Its assertion `source.contains("parallel_tool_calls = false")` was also satisfied by no such line while the "ordered" assertion around it happened to compare positions in a way the missing line could not fail, and its `tracing::info!` check counted the loop's unrelated alias-resolution log. | `crates/ragent-agent/tests/test_parallel_gate.rs` |
| 4 | Every declared **family** sets `supports_parallel: true`, and `PROVIDER_WIRE` is many-to-one onto those families, so no registered id currently resolves to a `false` profile. The gate's `false` row is therefore a declared contract with no provider behind it yet. | `ragent-llm/src/providers/wire/profile_table.rs` |

## 3. What landed

### 3.1 The gate

One block in the per-step derivation of `processor.rs`, between the config read
and the slot list:

```rust
let mut parallel_tool_calls = parallel_tool_calls;
if parallel_tool_calls
    && crate::provider::wire::profile_table::profile_for(&turn.model_ref.provider_id)
        .is_none_or(|profile| !profile.supports_parallel)
{
    parallel_tool_calls = false;
    tracing::info!(
        provider = %turn.model_ref.provider_id,
        reason = "the provider's wire profile declares supports_parallel == false, \
                  so the step's tool calls run sequentially (FR-010)",
        "parallel tool dispatch demoted to sequential"
    );
}
```

Three deliberate choices:

- **`is_none_or`, not `is_some_and`.** Both arms of "the provider cannot carry a
  parallel step" are covered by one expression: a profile that says `false`, and
  a provider id with **no** declared profile. The second is not a fallback - FR-002
  states there is no catch-all and no `Default`, so an id the table does not name
  declares nothing, and a harness must not treat "declares nothing" as "permits
  everything". This is also what makes the demotion reachable today (section 5).
- **Per step, not per turn.** The lookup is a linear scan of fifteen rows; the
  turn's provider cannot change between the derivation and the drain.
- **`if parallel_tool_calls &&`.** The gate runs only when the user asked for
  concurrency, which is what makes "log once per demotion" true rather than
  "log once per step": a step that was never parallel has nothing to demote and
  logs nothing.

The comment above the block records the router gap (section 6) at the call site,
not only in this document.

### 3.2 The field's contract is now stated as a demotion

`ToolWireProfile::supports_parallel` documented only "`false` forces sequential
execution". It now says what `true` means: concurrency is **permitted**, never
switched on - only the user's own opt-in does that. A reader who took `true` as
"the loop will parallelise" would reintroduce exactly the behaviour FR-010's risk
note forbids.

The `profile_table` module docs no longer attribute effective-contract resolution
to this task; they record what T-015 actually does (the row is read like any
other) and that the router's downstream family is not resolved.

## 4. The cases

`crates/ragent-agent/tests/test_parallel_gate.rs` was rewritten (it existed as a
dead, never-included source guard - see 2.3) and added to `tests/all.rs`. Five
cases:

| Case | Pins |
|---|---|
| `test_a_parallelising_provider_keeps_the_concurrent_arm` | a provider whose row declares `true` is not demoted, logs nothing, and still dispatches concurrently (the step's prompts all arrive before its ends) |
| `test_a_non_parallelising_provider_is_demoted_once_and_keeps_the_history` | the same step under a provider with no declared profile produces (a) the same recorded history as the same step with the opt-in off, (b) the sequential arm's event order, byte-identical to that reference run, and (c) **exactly one** demotion notice naming the provider and the reason. A second run asserts the notice is absent when the user never opted in |
| `the_gate_sits_between_the_config_read_and_the_first_call` | the structural half: config read, then `profile_for`, then the forced `false`, then the slot list - in that order; exactly one `tracing::info!` in the demotion block; the block never contains `parallel_tool_calls = true` |
| `every_family_profile_states_supports_parallel_and_the_loop_is_its_reader` | every family `const` sets the field, the matrix renders it, and the field's doc states the demotion contract |
| `the_rows_used_here_declare_the_parallelism_they_are_named_for` | the two rows the behavioural cases depend on still say what the cases assume, so a flipped row fails here by name |

### How the two arms are told apart

Both arms call the one pipeline, so a call's *recorded outcome* is identical
either way - comparing outcomes proves nothing about concurrency. The observable
is the bus: the permission stage publishes one `PermissionRequested` per call and
each arm closes the call by a different route.

- sequential: `prompt call_0, end call_0, prompt call_1, end call_1`;
- concurrent: `prompt call_0, prompt call_1, end call_0, end call_1`.

The step is two `Bash` calls running `sleep 0.05`. Fifty milliseconds is not
decoration: with a sub-millisecond call both arms produce the same event order by
accident and the case passes for the wrong reason (T-014 hit exactly that with
`bash_reset`). The command is off the safe-command whitelist (so the permission
layer holds it and prompts), harmless when it runs, and slow enough that a
concurrently dispatched pair provably overlaps.

The harness registers its own provider over a real provider id, stores that
provider's credential in the case's in-memory database (**not** an environment
variable, so no process-global state is touched and no env-isolation rule
applies), sets `auto_approve: false` (an auto-approving harness answers the
permission stage for it and the prompts never appear), and files one `Ask` rule
for the `bash` family.

## 5. Proof

### Red proofs

Each break was applied, the named cases observed red, then the file restored from
`target/temp/t015/processor.rs.bak` and verified byte-identical with `md5sum -c`,
with a `touch` first so cargo could not reuse the broken object.

| Break | Failing case(s) | Message observed |
|---|---|---|
| the gate's condition forced to `false` (the flag never demoted) | `test_a_non_parallelising_provider_is_demoted_once_and_keeps_the_history` | `left: [Prompt, Prompt, EndError("call_0"), EndError("call_1")]` - the concurrent arm's order where the sequential arm's was required |
| the `tracing::info!` deleted, condition intact | the same case, plus `the_gate_sits_between_the_config_read_and_the_first_call` | "the demotion is logged exactly once per demoted step"; "the demotion must log exactly one tracing::info! (FR-010)" |
| `if parallel_tool_calls &&` replaced by `if false &&` only | the same case | `left: [Prompt, Prompt, ...]` - identical to the first break, as expected: the condition was already satisfied |

### A limit recorded rather than hidden

No registered provider currently resolves to a `supports_parallel: false` row -
every family sets `true` (state 2.4). The behavioural cases therefore reach the
gate's demoting arm through its *other* input: a provider id with **no** declared
profile, which the same `is_none_or` branch covers. That is not a weaker test of
FR-010 - it is the arm FR-002 exists to force - but it does mean the `false` row
itself is exercised by no behavioural case, only by the field's declaration and
the matrix. Adding a `false` row to a family would change the contract of every
provider in it; the alternative, a test editing the profile table to suit itself,
would make the case assert on its own edit. Recorded, not papered over.

### Verification (green)

- `cargo fmt --all -- --check` - clean
- `cargo check --workspace --all-targets` - clean (1m 16s)
- `cargo clippy -p ragent-agent --all-targets` - no diagnostic names a file this
  task touched. The probe's turn future is boxed (`run_probe` ->
  `run_probe_inner` plus `Box::pin` at the call site) rather than carrying four
  `large_futures` warnings; the remaining diagnostics are the pre-existing
  `redundant_pub_crate` group in the library
- `cargo nextest run -p ragent-agent` - **2126 passed**, 0 failed, 1 skipped
  (2121 before this task; the five new cases are
  `test_parallel_gate::*`)
- the thirteen `scripts/check-*.sh` guards - twelve pass;
  `check-bench-regression` exits 2 because `target/criterion` does not exist on
  this tree, which is not a code finding

## 6. Deliberately not done

- **The router's downstream family is not resolved.** `RouterClient` picks a
  downstream provider per request, inside `chat`, after the agent loop has
  already chosen sequential or concurrent for the step. Honouring a downstream
  `supports_parallel: false` would need the loop to learn the selection before it
  dispatches, which is a different design (the router would have to expose the
  chosen entry ahead of the request). The gate reads the `router` row like any
  other - `true` today - and both the call site and `profile_table`'s docs say
  so. The field's doc in `wire/mod.rs` names T-015 as its consumer, which is now
  true for every id except this one.
- **No per-call gate.** A step is one provider's response, so the decision cannot
  change between calls of a step; gating per call would add a branch to the hot
  path and change nothing.
- **No config switch to override the gate.** FR-010 is a statement about what the
  provider can carry. A user who wants concurrency from a provider that cannot
  carry it is asking for a torn request, not a preference.
- **No change to `Cargo.toml`'s real dependencies.** `tracing-subscriber` was
  added as a *dev*-dependency only, so the scoped-subscriber capture in the test
  does not reach production code.

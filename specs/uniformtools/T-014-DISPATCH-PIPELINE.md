# T-014 - One ordered dispatch pipeline for both execution modes

Spec `uniformtools`, task T-014. Requirements FR-012 (one dispatch pipeline for
every execution mode) and FR-021 (an unknown tool name never executes, and the
refusal offers the closest registered name).

Status: **completed**. Dependency: T-013 (the one pre-execution refusal and its
`is_error` ownership).

## 1. What the task asks for

> **FR-012** The system **shall** run every tool call through one ordered
> pipeline - decode arguments, validate schema, unknown-tool check, repeat
> guard, pre-tool hooks, permission verdict, backend dispatch, result
> normalisation - and the sequential and parallel paths **shall** both call that
> pipeline rather than duplicating any stage.
>
> **FR-021** If a resolved tool name is not registered, the system **shall not**
> execute anything, and **shall** return a corrective tool result naming the
> unknown tool and the closest registered name when one is within the existing
> similarity threshold.

FR-012 is a statement about *equality of shape* between two call paths. A test
that observes one mode can never prove it, so the task has two halves: the
behavioural half (both modes reach the same verdicts, in the same order) and the
structural half (the stages exist once, are named once, and neither arm can run
a subset of them).

## 2. State found

Most of the pipeline was already in the tree at `d6abcd4a`, and T-013 had just
wired the refusal constructors. What was missing or wrong:

| # | Defect | Where |
|---|---|---|
| 1 | `ToolDispatchContext` borrowed the turn (`&'a str`, `&'a ToolContext`, `&'a [HookConfig]`, ...), so the context could not be moved into the spawned tool task. The loop therefore built a *second*, owned copy of each borrow per call and the two shapes diverged on lifetime bookkeeping. | `session/dispatch.rs` |
| 2 | `ResolvedToolCall.tool` was `Result<Arc<dyn Tool>, Box<ToolInputError>>`. The box existed only to keep the enum small, and it made every constructor call read `Err(Box::new(...))` - three spellings of "a refusal" where the value is one type. | `session/dispatch.rs` |
| 3 | `suggest_tool_name` was a second implementation of the did-you-mean ranking, alongside `processor::unknown_tool_error`'s copy. Two policies that could (and did) drift; FR-021 names one threshold. | `session/dispatch.rs`, `session/processor.rs` |
| 4 | `canonical_dispatch_name` resolved the name through `registry.resolve` and then asked the tool for its `name()`, so a registry whose lookup normalises differently from its registration could rewrite a name onto itself. | `session/dispatch.rs` |
| 5 | **The join-failure outcome had no constructor.** The sequential arm built the nine-field tuple inline; the concurrent arm built it inline in a closure. Two copies of "a task that never produced a result", whose text and status could be tuned apart. | `session/processor.rs` |
| 6 | **The two arms applied outcomes differently.** The sequential arm recorded a refused call inline, in the prepare loop; the concurrent arm collected futures with `join_all` and recorded them as the batch completed. A step whose first call was refused therefore produced two different histories depending on `experimental.parallel_tool_calls` - the precise drift FR-012 forbids. | `session/processor.rs` |

Defects 5 and 6 are the ones a reader of the code would not find by inspection:
5 is a duplicated literal, and 6 is a *timing* difference that only shows when
one call of the step is refused.

## 3. What landed

### 3.1 The context is owned, not borrowed

`ToolDispatchContext` now owns its fields (`String`, `Arc<...>`, `ToolContext`,
`Vec<HookConfig>`, `Option<Arc<LoopSpec>>`, ...). The loop builds it once per
call and moves it into the spawned task. The pipeline body reads through it
(`ctx.loop_spec.as_deref()`, `&ctx.hook_configs`, `&ctx.permission_checker`), so
the sequential arm is `pipeline.run(&ctx, ..).await` inlined into a task and the
concurrent arm is the identical task pushed into a list. Nothing about the stages
changes with the lifetime.

### 3.2 One refusal type

`ResolvedToolCall.tool` is `Result<Arc<dyn Tool>, ToolInputError>`.
[`ToolDispatchPipeline::refusal`] takes `&ToolInputError` and produces the
outcome; the sentence, the record status and the `is_error` flag all come from
that one value (T-013), and the loop states none of the three.

### 3.3 One did-you-mean ranking

`suggest_tool_name` in `dispatch.rs` is now a one-line re-export of
`session::tool_suggestion::suggest_tool_name`, which owns the ranking, the
thresholds and the edit-distance helper. `processor::unknown_tool_error`'s copy
is gone. FR-021's "existing similarity threshold" is therefore one number, in one
function, reached by both the pipeline and any caller that wants a hint.

`canonical_dispatch_name` no longer asks the resolved tool for its name: it reads
`registry.list()` and rewrites only on a **unique** ASCII-case-insensitive match,
so two registered names that differ only by case cannot make the rewrite depend
on iteration order.

### 3.4 A third terminal-outcome constructor

`ToolDispatchPipeline::synthesised(tc, message, duration_ms, bus, session)` is
the answer to a task that panicked, failed to join, or was aborted by the
watchdog. Both arms call it - the sequential arm directly, the concurrent arm
through a closure built before the future is moved - so the two cannot word the
same failure differently. It publishes the `ToolCallEnd` itself, exactly as
`denial` and `refusal` do, and returns the identical nine-field tuple.

### 3.5 The concurrent arm applies outcomes in the provider's order

This is the behavioural core of FR-012, and it is the change with a real
before/after. A step is now a list of **slots** in the provider's order:

```rust
type StepOutcomeSlot = Pin<Box<dyn Future<Output = ToolStepResult> + Send>>;
type PendingStepCall  = Result<StepOutcomeSlot, Box<ToolStepResult>>;
```

- a call that runs contributes `Ok(future)`;
- a call the prepare loop refused contributes `Err(boxed outcome)` - settled,
  already carrying its position;
- nothing is recorded during preparation. The drain walks the whole list once,
  after every call of the step has been prepared, and applies each outcome in
  turn.

`ToolStepResult` carries the call's position in the step (`usize`), which is
what makes "in the provider's order" checkable rather than incidental, and what
the sort-free list construction relies on.

Before this change, a three-call step whose first call was refused produced:

| mode | recorded order |
|---|---|
| sequential | `move_file (error)`, `think`, `calculator` |
| concurrent | `move_file (error)`, `think`, `calculator` *if the refusal beat both futures, otherwise* `think`, `calculator`, `move_file (error)` |

and the refusal's record could also land *ahead of* the calls the provider
listed before it. After the change both modes produce the first row.

### 3.6 The ordering contract is stated at the call site

`processor.rs` records, next to `resolve_tool_call`, that stages 1-3 are the
pipeline's and that the loop reaches them once; and next to the drain, that the
list is the provider's order and that the order is not negotiable. The doc
comment on `ToolDispatchPipeline::run` and on the module states the stage list
and *why* each pair of stages is ordered as it is.

## 4. The cases

`crates/ragent-agent/tests/test_unified_dispatch.rs` - **11 existing cases, +3
new**. The new ones:

| Case | Pins |
|---|---|
| `test_outcomes_keep_the_provider_order_in_both_modes` (new) | the behavioural half of 3.5: a three-call step whose **first** call is refused by its schema, followed by two slow `bash_reset` calls, is recorded in that order in *both* modes. The step is built so that "appended where it was decided" and "appended as it completed" both differ from the expected order |
| `parallel_arm_drains_one_ordered_list_of_slots` (new) | the structural half: the concurrent arm keeps one slot per call, a refused call pushes a settled slot rather than calling the record handler inline, and the drain walks the list in one ordered pass. With three fast calls the two shapes happen to agree, so only the source shows the slot really occupies its position |
| `test_the_permission_stage_runs_once_per_call_in_both_modes` (new) | the permission stage is a *stage* of the one pipeline: exactly one `PermissionRequested` per call in both modes. `test_sequential_and_parallel_paths_produce_the_same_outcomes` compares recorded outcomes, which leaves a mode free to reach them by a different route |
| `agent_loop_calls_the_one_stage_body_and_duplicates_no_refusal` (rewritten) | the loop defines no `resolve_call`, calls `resolve_tool_call` exactly once and `ToolDispatchPipeline::run` exactly once, and carries none of `denied_tool_call`, `unknown_tool_error`, `synthetic_error_result` |
| `unknown_tool_refusal_precedes_every_guard` (extended) | the repeat guard is no longer called from the loop body; it is the pipeline's stage 9 |

`crates/ragent-agent/tests/test_refusal_outcome.rs` - **new file, 3 cases**:

| Case | Pins |
|---|---|
| `an_unknown_tool_refusal_is_a_terminal_error_outcome` | FR-021 + FR-014: the record names the tool the provider sent, status `Error`, text behind one `Error: ` marker, exactly one `ToolCallEnd` with the same text, and **no** `ToolResult` event (a refused call must not look like a completed one with empty output) |
| `a_schema_refusal_is_a_terminal_error_outcome` | the same shape from the schema stage |
| `every_stopped_call_has_the_same_record_shape` | a guard denial and a pre-dispatch refusal differ in sentence but not in shape: both `Error`, both carry a reason, both close the call once |

`crates/ragent-agent/tests/test_tool_input_error.rs` - the T-013 case
`both_gates_build_their_refusal_with_the_shared_constructor` now reads
`session/dispatch.rs` as well as `processor.rs`: stages 1-3 moved out of the loop
body, and a scan of one file would have reported the gate as missing merely
because the code moved.

## 5. Proof

### Red proofs

Each break was applied, the named case observed red, then the file restored from
`target/temp/t014-processor.final` and verified byte-identical with `md5sum -c`,
with a `touch` first so cargo could not reuse the broken object.

| Break | Failing case | Message observed |
|---|---|---|
| a refused call recorded inline in the prepare loop (defect 6 restored) | `parallel_arm_drains_one_ordered_list_of_slots` | "a refused call must occupy its position in the step's slot list rather than append its record where it was decided (FR-012)" |
| the slot list drained as each future finished rather than in one ordered pass | the same case | "the concurrent arm must drain the step's slots in one ordered pass (FR-012)" |
| the loop answering a refusal with its own inline tuple instead of the pipeline's constructor | `agent_loop_calls_the_one_stage_body_and_duplicates_no_refusal` | "the agent loop must not carry its own `unknown_tool_error`" |

### A proof that was attempted and rejected

The behavioural case was first written with two fast tools after a first-call
refusal, and it passed **without** the fix. Investigating (a temporary
`eprintln!` in the drain, removed again) showed why: both futures are ready by
the time `join_all` polls them, and the sequential arm's `tokio::spawn` also
completes after the prepare loop, so on a fast step the two arms happen to agree.
The case was rebuilt so the orderings genuinely differ - two `bash_reset` calls
that wait on the session's shell - and the source guard above was added for the
half that timing cannot pin. This is recorded because "the case passed before the
fix" is exactly the failure mode a red proof exists to catch, and it was caught.

### Verification (green)

- `cargo fmt --all -- --check` - clean
- `cargo check --workspace --all-targets` - no error, no new warning
- `cargo clippy -p ragent-agent --all-targets` - no diagnostic names a file this
  task touched (the only groups are the pre-existing
  `pub(crate) ... inside private module` lints and provider
  `unnecessary structure name repetition`)
- `cargo nextest run -p ragent-agent` - **2101 passed**, 0 failed, 1 skipped
  (was 2098 passed / 1 failed before the last two fixes; the two failures were
  `test_sandbox_secrets` cases colliding on one container name, see below)
- the twelve `scripts/check-*.sh` guards - all pass

### A fix outside the task's own files

`test_sandbox_secrets` and `test_container_backend` each start a real container
from a name derived from the workspace path (`ragent-sandbox-<hash>`). Under
nextest's one-process-per-test model two of them run at once and the loser fails
with *"the container name ... is already in use"*. `.config/nextest.toml` gained
a `test-group-sandbox` group (`max-threads = 1`) covering both modules, in
`profile.default` and `profile.ci`. This is the cross-process exclusion AGENTS.md
requires be declared there rather than in a file-local lock; it is not a
weakening of any assertion.

## 6. Notes for the downstream tasks

- **T-015** gates parallel dispatch on the profile. The slot list is the place to
  do it: force `parallel_tool_calls = false` before the prepare loop and every
  call runs through the sequential arm, which is now the same pipeline with the
  same order.
- **T-016** blocks a result append while a permission verdict is unresolved. The
  only append site is the drain's `handle_tool_execution_result`; the refused-call
  slot is the one outcome that never waits for a verdict.
- **T-017** finishes FR-014's transmit side; `is_error` is already correct from
  the record through to `ContentPart::ToolResult`, and `refusal()` already
  prefixes the model-visible text with `Error: `.
- `ToolDispatchPipeline::refusal` now renders `Error: <sentence>` rather than the
  bare sentence, so a refusal and a failed run reach the model with the same
  marker. `ToolCallEnd`'s `error` field still carries the bare sentence, which is
  what the TUI shows; `test_refusal_outcome.rs` pins both.

# T-016 - Block the result while a verdict is unresolved

Spec `uniformtools`, task T-016. Requirement FR-013 (approval blocks the result,
not just the call).

Status: **completed**. Dependency: T-014 (the one ordered dispatch pipeline),
whose permission stage is where the unresolved interval begins.

## 1. What the task asks for

> **FR-013** (state-driven) While a tool call's permission verdict is
> unresolved, the system **shall not** append a tool result for it to history, so
> no turn can continue on an unapproved call's output.

The requirement is deliberately *state-driven*: it does not describe a function
that runs when the user clicks deny, it describes a fact that must hold for the
lifetime of a pending approval. A guard that only checked "was the verdict a
denial?" would satisfy a call-by-call reading and miss the state entirely - and
the state is exactly where a wedged wait, an aborted task or a dropped future
lives.

## 2. State found

| # | State | Where |
|---|---|---|
| 1 | The permission layer already *awaits* before dispatching: `dispatch_tool_with_permissions` calls `check_permission_with_prompt` and only a `PermissionAction::Allow` reaches `dispatch_tool_leaf`, so on the normal path no output exists while a prompt is open. A timeout is a `Deny`, and a surviving `Ask` is mapped to an error. | `ragent-agent/src/session/processor.rs` |
| 2 | Nothing *tracked* the interval. There was no state anywhere saying "this call is at a prompt", so no code outside the permission layer could answer "is this call's verdict unresolved?" - the pipeline's own stages and the tool phase's append path had no shared fact to consult. | - |
| 3 | The tool phase appended a result for **every** outcome it was handed (`handle_tool_execution_result`), unconditionally: `assistant_parts.push(MessagePart::ToolCall { .. })` and `tool_result_parts.push(ContentPart::ToolResult { .. })`. | `processor.rs`, tool phase |
| 4 | `crates/ragent-agent/tests/test_approval_blocks_result.rs` existed (from an earlier pass at FR-013) but was **not listed** in `tests/all.rs`, so with `autotests = false` it never compiled and never ran - the orphan-file symptom this spec has hit several times. Two of its four cases were source guards whose needles had gone stale against the current comments. | `tests/test_approval_blocks_result.rs`, `tests/all.rs` |

## 3. What landed

### 3.1 The tracker

A new module, `crates/ragent-agent/src/session/pending.rs`, holding
`PendingVerdicts`: per session, the set of call ids currently at a prompt.

```rust
pub struct PendingVerdicts { inner: Arc<parking_lot::Mutex<HashMap<String, HashSet<String>>>> }

impl PendingVerdicts {
    pub fn begin(&self, session_id: &str, call_id: &str);   // immediately before the prompt
    pub fn settle(&self, session_id: &str, call_id: &str);  // when the verdict resolves
    pub fn is_unresolved(&self, session_id: &str, call_id: &str) -> bool;
}
```

Three design points, each of which a simpler shape would have got wrong:

* **Keyed by session *and* call.** Request and call ids are not globally
  unique (a provider restarts its `call_1` in every response, and two sessions
  run concurrently), so a tracker keyed on the call id alone lets one session's
  verdict unblock another's call.
* **Begin before the prompt, settle on every exit.** The mark is taken by the
  *stage*, not inside the prompt helper, so the interval covers the whole
  permission stage - including a future that is dropped or joined after the wait
  it owned has been abandoned. The settle sits immediately after the await, so an
  allow, a deny, a policy refusal and a timeout all clear it: a timed-out prompt
  is a denial, and none of those leaves the call awaiting an answer.
* **A second `begin` does not need a matching `settle`.** The bash path prompts
  once per sub-command under one call id. The set is idempotent, and one settle
  resolves the call, because the *verdict* is one verdict - the last prompt it
  raised having an answer.

### 3.2 The mark and the settle

In the pipeline's stage 12
(`crates/ragent-agent/src/session/dispatch.rs`):

```rust
ctx.pending_verdicts.begin(session_id, &tc.id);
let result = super::processor::dispatch_tool_with_permissions(..).await;
ctx.pending_verdicts.settle(session_id, &tc.id);
```

`ToolDispatchContext` gained the field, `SessionProcessor` gained the field
(one shared tracker per processor, so the pipeline and the append path observe
the same state rather than two copies), and every construction site in the tree
initialises it - `src/main.rs`, the TUI and server and agent test harnesses.

### 3.3 The guard

The tool phase's one append path now reads the tracker through a named free
function rather than an inline `if`, because the result handler already borrows
three step-local buffers mutably and a handler that *owned* the tracker would have
to take those buffers with it:

```rust
fn result_is_appendable(pending: &PendingVerdicts, session_id: &str, call_id: &str) -> bool {
    !pending.is_unresolved(session_id, call_id)
}
```

On an unresolved verdict the handler:

* still writes the `MessagePart::ToolCall` record - with status `Pending`, no
  output and no error - so the conversation carries the `tool_use` the provider
  listed and no model call is left orphaned;
* does **not** push the `ContentPart::ToolResult`, so the model never receives
  the unapproved call's outcome;
* logs one `tracing::warn!` naming the session, the call and the tool, because a
  silently dropped result would hide a wedged verdict.

The two comments the guards read
(`// FR-013 (T-016): ... history append can run while a permission verdict ...`
in the handler and `// FR-013 (T-016): the turn's history is appended only here`
at the step's `chat_messages` push) record the ordering the requirement depends
on: the turn's history can only be appended after every call of the step has been
awaited and settled.

## 4. Tests

`crates/ragent-agent/tests/test_approval_blocks_result.rs` is now registered in
`tests/all.rs` and holds nine cases:

| Case | What it pins |
|---|---|
| `permission_check_maps_unresolved_verdict_to_a_denied_error` | the `Ask` branch is an error, tagged FR-013, with a message the model reads |
| `history_append_runs_only_after_the_permission_verdict_resolved` | both ordering comments exist and the append still goes through `handle_tool_execution_result` |
| `the_permission_stage_marks_then_settles_the_call` | the mark precedes the settle, both live in the permission stage, and both sit after the call's `ToolCallStart` |
| `the_result_append_reads_the_tracker` | `result_is_appendable` exists, is consulted for the call being appended, and gates the `ToolResult` push |
| `the_tracker_starts_empty_and_is_per_session` | a fresh tracker is inert; a mark is visible only to its own session and its own call |
| `a_second_mark_keeps_the_call_unresolved_until_the_last_settle` | the bash sub-command case: two prompts, one verdict |
| `the_append_guard_withholds_only_the_unresolved_call` | the decision itself: unresolved -> withheld, sibling call unaffected, other session unaffected, settled -> appended |
| `test_denied_write_writes_nothing_and_surfaces_a_denied_error` | end to end: a denied `Write` writes nothing and the next request carries an `is_error` denial observation |
| `test_approved_write_executes_and_returns_success` | the converse: an approved call runs and its result is appended with `is_error: false` |

### Red proofs

Three, each applied, run, and restored byte-identically from
`target/temp/t016/` with `md5sum -c` and a `touch`:

1. `result_is_appendable` forced to `true` (the guard never withholds) -
   `the_append_guard_withholds_only_the_unresolved_call` fails with
   *"a call whose permission verdict has not resolved must not have its result
   appended (FR-013)"*.
2. The `begin` deleted from the permission stage -
   `the_permission_stage_marks_then_settles_the_call` fails naming the missing
   mark.
3. The handler's call site replaced by `let verdict_resolved = true;` (the guard
   exists but is not consulted) - `the_result_append_reads_the_tracker` fails.

### What no automated case covers

The guard's `false` arm cannot be reached from an integration test. The decision
is made inside the tool phase's result handler, which is a closure over the loop's
own buffers and is not callable from outside the crate, and the test-visible
pipeline (`ToolDispatchPipeline`) is `pub(crate)` behind a `pub(crate)`
`PendingToolCall`. Reaching it behaviourally would mean publishing the whole
dispatch surface for one assertion, which this spec has refused elsewhere. What is
proven instead is the composition: the permission stage marks and settles (case
3), the decision withholds exactly the unresolved call (case 7), and the handler
consults that decision (case 5). A future `tests/` harness that can build a
`SessionProcessor` and drive one pipeline call would close the last seam; the task
records this rather than claiming the arm is covered.

## 5. Deliberately unchanged

* **Where the verdict is decided.** The tracker observes the permission stage; it
  does not re-implement it. `check_permission_with_prompt` keeps its own timeout,
  YOLO, auto-approve, hook-approval and analyzer rules.
* **The denial path.** A denial is a *resolved* verdict, so its result - the
  error observation the existing behaviour already produced - is appended
  normally. FR-013 forbids appending an unresolved call's output, not a denial.
* **The TUI.** A pending call already renders as a pending part
  (`ToolCallStatus::Running | Pending`), and the withheld record uses exactly that
  status, so no renderer changed.
* **`experimental.parallel_tool_calls`.** The mark/settle pair is per call, and
  the tracker is keyed per call, so both arms of a parallel step track their own
  calls independently.

## 6. Verification

* `cargo fmt --all -- --check` - clean.
* `cargo check --workspace --all-targets` - clean (1m 14s).
* `cargo clippy -p ragent-agent --all-targets` - no warning names a file this task
  touched (`pending.rs`, `session/dispatch.rs`, `session/processor.rs`,
  `tests/test_approval_blocks_result.rs`); the pre-existing
  `redundant_pub_crate` group elsewhere is unchanged.
* `cargo nextest run -p ragent-agent` - 2135 passed, 1 skipped, 0 failed.
* Guard scripts: 12 of 13 pass; `check-bench-regression.sh` exits 2 on a missing
  `target/criterion` directory, which is not code-caused.

## 7. Hand-off

* T-017 (FR-014) reads the same recorded status to set `is_error`; the withheld
  record is `Pending`, whose `ToolCallStatus::is_error()` is `false`, so the two
  tasks do not interact.
* T-026/T-027 (the gate) should treat the FR-013 guard as part of the one
  pipeline's contract: a stage that refuses must settle, and the guard is what
  makes "settle" observable.

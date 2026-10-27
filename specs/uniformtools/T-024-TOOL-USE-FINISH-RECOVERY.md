# T-024 - A tool-use finish with zero complete calls is a truncation

| Field | Value |
|---|---|
| Spec | `uniformtools` |
| Requirement | FR-022 |
| Dependencies | T-008 (the accumulator), T-009, T-010, T-011 |
| Status | completed |

## 1. The requirement

> If a provider reports a tool-use finish reason but the accumulator produced
> zero complete tool calls, the system **shall** treat the step as an incomplete
> response and **shall** enter the existing text-recovery path before deciding
> the turn is finished.

The unwanted behaviour is precise: a turn that ends silently with a lost tool
call, presented to the user as a clean reply.

## 2. What was already there, and what was missing

Three paths already existed around the gap this task fills.

- **The silent-cut detector.** `call_llm_step` records `Truncation` when the
  provider emitted no `Finish` at all and produced no tool calls. A provider that
  *does* emit a tool-use finish fails that test (`saw_finish_reason` is `Some`),
  so it slipped through.
- **The truncation-continuation kick.** A sub-agent whose reply was truncated and
  which has text to continue from asks the model to continue, once.
- **The text-recovery scan (F9).** `text_toolcalls::extract_text_tool_calls_with_spans`
  scans the text buffer, then the reasoning buffer, for the recognised
  tool-call dialects and, on a hit, replaces the empty call list with the
  recovered calls and blanks the matched spans.

What was missing is the join. A provider that reports `FinishReason::ToolUse`
while the accumulator holds nothing left the step looking like a completed
tool-use turn: `loop_state.last_finish_reason` stayed `ToolUse`, the recovery
scan ran (it is unconditional on an empty call list) but nothing had prepared the
step to be treated as incomplete, and with no recoverable markup the processor
took its "no tool calls" branch and ended the run - recording `ToolUse` and
telling nobody.

## 3. What this task added

`crates/ragent-agent/src/session/loop_steps.rs`:

1. **A per-attempt start flag.** `tool_calls_ever_started` is set when a
   `StreamEvent::ToolCallStart` arrives and cleared on a plain retry alongside
   `saw_completed_tool_call`. FR-022 keys on "zero complete calls" *and* on
   nothing having started, because those are different failures: a call whose
   arguments were cut off is a dispatch problem, which the normal pipeline
   reports as an invalid-argument tool result the model can correct, while a
   call that never started is a call nobody can act on.
2. **One predicate.** `SessionProcessor::finish_was_tool_use_without_a_call(finish_reason, saw_tool_call_start, tool_call_count) -> bool`
   is the rule, public so the integration test can pin every arm of it without a
   provider. It is the only place the three inputs are combined.
3. **The reclassification, placed before the continuation kick and before the
   recovery scan.** When the predicate holds, the step logs one `tracing::warn!`
   naming the provider, publishes one `AgentNotice` telling the user the
   provider reported a tool call that could not be parsed and that the reply text
   is being checked, and sets `loop_state.last_finish_reason = Some(Truncation)`.
   The ordering is the requirement: the truncation-continuation kick tests
   `Truncation` (and is inert here, which is the "no spurious recovery prompt"
   expectation), and the recovery scan below is the "enter the existing
   text-recovery path" clause, now guaranteed to run on a step the rest of the
   loop knows is incomplete.

Nothing else changed. The recovery scan, the blanking, the dispatch pipeline and
the terminal-notice path are the existing ones; this task only makes a lost call
reach them, and makes the run honest when it cannot be recovered.

## 4. Red proofs

Each was applied, the named test run, and the file restored byte-identically from
`target/temp/t024/` and checked with `md5sum -c` plus a `touch`.

| # | Injected defect | What went red |
|---|---|---|
| 1 | The call site replaced by the pre-change shape (`tool_calls.is_empty() && !tool_calls_ever_started && matches!(last_finish_reason, Some(FinishReason::ToolUse))`), i.e. no reclassification | `test_a_tool_use_finish_without_a_call_and_no_markup_reports_truncation` - the run ended recorded as a clean stop |
| 2 | `finish_was_tool_use_without_a_call` made to return `false` unconditionally | `test_a_tool_use_finish_without_a_call_is_a_truncation` (panics on its first assertion) |
| 3 | The call site rewritten as `!tool_calls_ever_started && matches!(last_finish_reason, Some(FinishReason::ToolUse))` - the *complete* list ignored | `test_a_started_but_unfinished_call_is_not_reclassified` - `a started call must not be reclassified as a lost call: ["The provider reported a tool call but none could be parsed - checking the reply text for a recoverable call."]` |

Two honest negative results are recorded rather than hidden, because both cost
time and both say something about what the tests can see:

- **Inverting the start flag alone stays green.** With `tool_calls_ever_started`
  never set (so a started call is invisible to the rule), proof 3's test still
  passed, twice. The reason is that in the real pipeline a started call is *not*
  reclassified end to end there: `call_llm_step`'s recovery scan leaves the call
  list non-empty, and the truncation-continuation kick requires
  `tool_calls.is_empty()`, so the step still reaches the tool phase. The guard is
  therefore provable only at the call site (proof 3), not by falsifying its
  input; the flag's own effect is a one-line difference in a condition that the
  observable behaviour normally masks.
- **The first version of the boundary test asserted the wrong observable.** It
  counted requests, and a reclassified step loops through recovery on every step
  until the step budget (1024 requests), so the assertion was about a runaway,
  not about the rule. It now asserts the exact request count (2: one dispatch,
  then the closing text step), the absent reclassification notice, and the
  refused-arguments tool result carrying the provider's own call id.

## 5. Deliberately unchanged

- **A complete call list on a tool-use finish is the ordinary tool phase.** The
  reclassification requires zero starts, so the normal path never sees it.
- **`FinishReason::Length` and the silent-cut path keep their own handling.** FR-022
  names the tool-use finish; broadening the rule would reclassify replies that
  already have a correct, more specific label.
- **The continuation kick stays sub-agent-only.** A tool-use finish is not a
  silent cut - the provider said what it was doing, it just did not deliver - so
  asking the model to "continue from where it stopped" would re-ask for the call
  rather than salvage it.
- **`test_processor_more_paths`'s pinned reason changed, and that is the point.**
  `test_finish_tool_use_without_calls_records_tool_use` asserted exactly the
  behaviour FR-022 forbids; it is now
  `..._records_truncation` and pins `Truncation`.

## 6. Tests

`crates/ragent-agent/tests/test_loop_steps_harness.rs` (the existing fake-provider
harness, extended with two scripted replies):

- `test_a_tool_use_finish_without_a_call_is_a_truncation` - every arm of the rule.
- `test_a_tool_use_finish_without_a_call_recovers_the_text_call` - the narrated
  call is recovered and dispatched (two requests), the recovered markup is
  blanked from the saved reply, the recovery notice names one call, and the run
  ends with a clean stop.
- `test_a_tool_use_finish_without_a_call_and_no_markup_reports_truncation` - one
  request, no spurious recovery or continuation prompt, and the run recorded
  `Truncation`.
- `test_a_started_but_unfinished_call_is_not_reclassified` - two requests, no
  reclassification notice, and the next request carries
  `ToolResult { tool_use_id: "call_cut", is_error: true }` for the cut-off call.

`crates/ragent-agent/tests/test_processor_more_paths.rs`:
`test_finish_tool_use_without_calls_records_truncation`.

## 7. Verification

- `cargo fmt --all -- --check` clean.
- `cargo nextest run -p ragent-agent --no-fail-fast`: 2121 tests run, 2121
  passed, 1 skipped.
- `cargo nextest run -p ragent-server -p ragent-llm -p ragent-config -p ragent-tui --no-fail-fast`:
  5058 passed, 2 failed - `test_clipboard::test_clipboard_repeated_writes` (the
  clipboard lock is not held across the read in this sandbox; it passes in
  isolation) and `test_slash_arms_deep::tools_visibility_save_failure_is_reported`
  (pre-existing, recorded in earlier tasks). Neither touches this change.
- `cargo clippy -p ragent-agent --all-targets`: no finding names a file this task
  edited.
- Guard scripts: unchanged by this task (no source-scan list or count was edited).

## 8. Hand-off

FR-022 is now true end to end for the loop in `loop_steps.rs`. The remaining
provider-side half - a provider that *itself* swallows a tool-call event and
still reports a tool-use finish - is covered by the same rule without provider
work, because the rule keys on what the accumulator produced rather than on what
the provider claimed.

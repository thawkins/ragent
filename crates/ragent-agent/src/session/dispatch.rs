//! The one ordered tool-dispatch pipeline (spec `uniformtools` FR-012, FR-021).
//!
//! Every tool call the agent loop executes - whether the step's calls run
//! sequentially or concurrently - goes through [`ToolDispatchPipeline::run`].
//! The stages are fixed and run in this order, once each:
//!
//! 1. **arguments decoded** - [`ragent_types::tool_args::decode_tool_arguments`],
//!    the one decode of this call's accumulated text (FR-007);
//! 2. **tool resolved** - the registry lookup; an unregistered name is refused
//!    here with [`ToolInputError::unknown_tool`] and its did-you-mean hint, and
//!    nothing else runs (FR-021);
//! 3. **schema validated** - [`ragent_tools_core::schema::validate_required_args`]
//!    against the tool's declared `parameters_schema()` (FR-009);
//! 4. **loop restriction** - the active [`LoopSpec`]'s tool-set / scope /
//!    read-only / destructive-action rules, checked against the decoded input so
//!    an undecodable payload fails closed;
//! 5. **plan gate** - a teammate whose plan awaits the lead's verdict cannot
//!    mutate work (T-307);
//! 6. **interactive-tool block** - a sub-agent run has no attached user, so an
//!    interactive tool is refused rather than left waiting for ever;
//! 7. **pre-tool hooks** - `PreToolUse`, which may allow, deny, block, or replace
//!    the input, so the guard below sees the arguments the tool would run with;
//! 8. **repeat guard** - five consecutive identical calls pass, the sixth
//!    prompts or is denied (FR-044);
//! 9. **concurrency permit** - acquired only after every guard, so a refused call
//!    never holds one;
//! 10. **security analyzer** - the LLM verdict, published before the action is
//!     permitted or refused (spec `openhands` FR-006, FR-017);
//! 11. **permission verdict and backend dispatch** - the 7-layer bash validation,
//!     the path guards and the backend confinement, with the call marked
//!     unresolved for exactly the interval its verdict is outstanding (FR-013);
//! 12. **result normalised** - the tool's output or error becomes the record, the
//!     LLM-visible result text, the `PostToolUse` guidance, the bus events and
//!     the memory-extraction observation.
//!
//! ## Why one pipeline rather than two paths
//!
//! Before this module the stages lived inside the body of the tool phase of
//! [`SessionProcessor::process_message`](crate::session::processor::SessionProcessor),
//! and that body was spawned into a task per tool call. The only thing that
//! distinguished the sequential path from the concurrent one was whether the
//! spawned task was awaited immediately or collected into a `join_all`, so the
//! two shared the stages by sharing a closure - but the closure was reached
//! through two different call shapes, each with its own watchdog, panic and
//! skip handling. A change to the order of two stages could therefore reach one
//! shape and not the other.
//!
//! [`ToolDispatchPipeline::run`] is now the only place the stages exist. The loop
//! decides *when* to await a call (`await` or `join_all`); it cannot skip,
//! reorder, or repeat a stage. [`ToolDispatchPipeline::denial`],
//! [`ToolDispatchPipeline::refusal`] and [`ToolDispatchPipeline::synthesised`]
//! build the same nine-field outcome from the three ways a call stops without a
//! tool result (a guard's denial, a pre-dispatch refusal, a join or watchdog
//! failure), so every terminal path returns the identical tuple.
//!
//! ## Ordering rationale
//!
//! * Decode before resolve, because the decode is what proves the call is
//!   executable at all, and the refusal for a bad payload must name the tool the
//!   model asked for even when that tool does not exist.
//! * Resolve before schema, because a schema check needs the tool's declared
//!   schema and there is none for an unregistered name.
//! * Both before the loop restriction, hooks and permissions, because those
//!   stages read the decoded *and* validated arguments; running a hook on
//!   arguments the tool cannot accept wastes a subprocess and reports a
//!   misleading reason.
//! * The hooks before the repeat guard, because the guard must key on the
//!   canonical input a `PreToolUse` hook may have replaced - and an allowed call
//!   must reset the consecutive counter, which only works before dispatch.
//! * The permission layer last, because everything before it is a refusal the
//!   user should never be asked to approve.

use std::sync::Arc;

use serde_json::Value;

use ragent_types::message::ToolCallStatus;

use crate::event::Event;
use crate::hooks::HookConfig;
use crate::session::history::PendingToolCall;
use crate::session::loop_state::LoopSpec;
use crate::session::pending::PendingVerdicts;
use crate::session::permissions::RepeatTracker;
use crate::session::tool_input_error::ToolInputError;
use crate::tool::{Tool, ToolContext};

/// The outcome one dispatched tool call produces, consumed by
/// [`SessionProcessor`](crate::session::processor::SessionProcessor)'s tool
/// phase to append history, publish the batch event and record the activity.
///
/// The fields are exactly the ones the loop needs, so a refusal, a denial and a
/// completed call are indistinguishable in shape - which is why a stage cannot
/// "return early" in a different format from any other.
/// Field order: the call; the decoded input the call ran with (`Value::Null` for
/// a refusal); the record status whose
/// [`is_error`](ToolCallStatus::is_error) is the result flag (FR-014); the tool's
/// metadata (present on success); the failure text (present on a refusal or a
/// failed run); the wall-clock duration; the content the model sees; the
/// metadata attached to the LLM-visible result; and the `PostToolUse` hook
/// `additionalContext` blocks to fold in.
pub(crate) type ToolOutcome = (
    PendingToolCall,
    Value,
    ToolCallStatus,
    Option<Value>,
    Option<String>,
    u64,
    String,
    Option<Value>,
    Vec<String>,
);

/// Everything one dispatched call needs from the session that owns it.
///
/// Bundled into one struct so [`ToolDispatchPipeline::run`] keeps a readable
/// signature: the fields are the loop's own borrows, cloned once per step.
pub(crate) struct ToolDispatchContext {
    /// The session being served.
    pub session_id: String,
    /// The session's permission checker, consulted last (stage 9).
    pub permission_checker: Arc<parking_lot::RwLock<crate::permission::PermissionChecker>>,
    /// The bus every stage publishes on.
    pub event_bus: Arc<crate::event::EventBus>,
    /// The execution context handed to the tool.
    pub tool_ctx: ToolContext,
    /// The active loop's spec, when the session runs one.
    pub loop_spec: Option<Arc<LoopSpec>>,
    /// Configured and plugin-contributed hooks, already parsed for this turn.
    pub hook_configs: Vec<HookConfig>,
    /// The working directory the hooks run in.
    pub hook_working_dir: std::path::PathBuf,
    /// Per-session repeat trackers (FR-044).
    pub tool_repeat_guard:
        Arc<parking_lot::Mutex<std::collections::HashMap<String, RepeatTracker>>>,
    /// Telemetry sink for the per-call recorder.
    pub telemetry: Arc<crate::telemetry::TelemetrySubsystem>,
    /// The memory-extraction engine, when enabled.
    pub extraction_engine: std::sync::OnceLock<Arc<crate::memory::ExtractionEngine>>,
    /// Storage handle the extraction engine records through.
    pub storage: Arc<crate::storage::Storage>,
    /// Cache of resolved team directories, cleared after a `team_*` call.
    pub team_context_cache: Arc<
        parking_lot::RwLock<
            std::collections::HashMap<String, (crate::tool::TeamContext, std::time::Instant)>,
        >,
    >,
    /// The security analyzer client, model and timeout for this turn, when the
    /// analyzer is enabled (spec `openhands` FR-006, FR-017).
    pub security_analyzer: Option<(Arc<dyn crate::llm::LlmClient>, String, u64)>,
    /// Whether this run is a sub-agent (no attached user).
    pub agent_is_subagent: bool,
    /// The names of the tools this run can call that mutate the workspace -
    /// the automatic sub-agent YOLO scope's membership test.
    ///
    /// Resolved once per turn from the run's own exposed surface
    /// ([`crate::session::permissions::write_capable_tool_names`]) so every
    /// call in the turn asks the same question. Empty for a read-only run,
    /// which is how an `explore` sub-agent stays under the ordinary policy.
    pub write_capable_tools: Arc<std::collections::HashSet<String>>,
    /// `None` forces a destructive-action checkpoint; `Some(bool)` is the normal
    /// auto-approve setting.
    pub auto_approve: Option<bool>,
    /// Whether the active loop has destructive-action checkpoints enabled.
    pub loop_checkpoint: bool,
    /// Timeout for a checkpoint or repeat-guard prompt.
    pub checkpoint_timeout_secs: u64,
    /// The calls currently at a permission prompt (FR-013).
    ///
    /// The permission stage marks its call unresolved immediately before it
    /// publishes a prompt and settles it when the verdict resolves, so the tool
    /// phase can refuse to append a result for a call whose verdict never
    /// resolved. See [`crate::session::pending`].
    pub pending_verdicts: PendingVerdicts,
}

/// The ordered, mode-independent tool-dispatch pipeline.
///
/// A unit struct: the pipeline holds no state, so both the sequential and the
/// concurrent call shape construct it identically and there is nowhere for the
/// two to diverge.
pub(crate) struct ToolDispatchPipeline;

impl ToolDispatchPipeline {
    /// Runs one tool call through every stage, in the fixed order documented on
    /// this module.
    ///
    /// Returns [`ToolOutcome`] on every path - the stages that refuse build
    /// their refusal with [`ToolDispatchPipeline::refusal`], which is the same
    /// shape a completed call produces. A `return` from a stage is therefore
    /// never an early exit from "the pipeline" but a normal, expected outcome.
    ///
    /// The function never panics on data-driven input: an unregistered name, an
    /// undecodable payload, a schema violation, a denied permission and a failed
    /// tool call are all data.
    #[allow(clippy::too_many_lines)]
    pub(crate) async fn run(
        ctx: &ToolDispatchContext,
        tc: &PendingToolCall,
        resolved: ResolvedToolCall,
        profiler: &Arc<crate::session::profiler::AgentLoopProfiler>,
    ) -> ToolOutcome {
        let event_bus = &ctx.event_bus;
        let session_id = ctx.session_id.as_str();
        let _tool_total_scope = profiler.scope_with(|| format!("tool.total:{}", tc.name));
        event_bus.publish(Event::ToolCallStart {
            session_id: session_id.to_string(),
            call_id: tc.id.clone(),
            tool: tc.name.clone(),
        });
        event_bus.increment_tool_calls(session_id);

        // Stage 1-3: decode, resolve, validate. `resolved` carries the outcome
        // of the pipeline's own first three stages, computed once by
        // [`resolve_tool_call`] before any guard runs, so no stage can observe a
        // different decode than another (FR-007, FR-008, FR-009, FR-021).
        let ResolvedToolCall { input, tool } = resolved;
        let tool = match tool {
            Ok(tool) => tool,
            Err(refusal) => {
                // The decoded `input` is kept for a refused call so the TUI can
                // still render what the model sent; it is `Value::Null` only
                // when the payload would not decode at all.
                return Self::refusal(tc, &refusal, input, event_bus, session_id);
            }
        };

        // Stage 4: destructive-action checkpoint flag for this call (loop
        // FR-015). Plain write tools keep the normal permission path.
        let checkpoint_forced = ctx.loop_checkpoint
            && crate::session::permissions::is_destructive_tool(&tc.name, &input);

        // Stage 5: loop tool-set / scope / read-only restriction (FR-008,
        // FR-009, FR-021, FR-022). Unconditional: it applies in auto-approve /
        // YOLO mode too, because the permission layer is always in the path.
        if let Some(spec) = ctx.loop_spec.as_deref() {
            if let Some(reason) = spec.deny_reason(&tc.name, &input) {
                tracing::info!(
                    tool = %tc.name,
                    reason = %reason,
                    "loop restriction denied tool execution"
                );
                return Self::denial(tc, &reason, input, event_bus, session_id);
            }
        }

        // Stage 6: plan-approval gate. A teammate without an approved plan must
        // not start mutating work; read-only tools and the team coordination
        // tools stay available (that is how it learns the verdict).
        //
        // `is_plan_pending` is the narrow "a plan awaits the verdict" question
        // that T-307 asked; `is_plan_gate_active` is the wider FR-014 question
        // ("`require_plan_approval` is set and this member's plan is not
        // `Approved`"), so a teammate that never submitted a plan is stopped too
        // (F-10). The two are ORed rather than replaced: the narrow query keeps
        // its meaning for a setting disabled mid-run.
        if let Some(tc_team) = ctx.tool_ctx.team_context.as_ref() {
            let plan_gate = ctx.tool_ctx.team_manager.as_ref().is_some_and(|tm| {
                tm.is_plan_pending(&tc_team.agent_id) || tm.is_plan_gate_active(&tc_team.agent_id)
            });
            if let Some(reason) =
                crate::team::plan_gate_denial(&tc.name, tc_team.is_lead, plan_gate)
            {
                tracing::info!(
                    tool = %tc.name,
                    agent_id = %tc_team.agent_id,
                    "plan-pending gate denied tool execution (T-307, FR-014)"
                );
                return Self::denial(tc, &reason, input, event_bus, session_id);
            }
        }

        // Stage 7: interactive-tool block for sub-agent runs. A sub-agent has no
        // attached user (background, cron, server, forked skill), so an
        // interactive tool would block the run for ever.
        if ctx.agent_is_subagent && crate::session::permissions::is_interactive_tool(&tc.name) {
            let reason = format!(
                "interactive tool '{}' is not available in subagent runs - no user \
                 is attached; decide autonomously from the task context and \
                 continue",
                tc.name
            );
            tracing::warn!(tool = %tc.name, "interactive tool denied in subagent run");
            return Self::denial(tc, &reason, input, event_bus, session_id);
        }

        // Stage 8: pre-tool hooks, handed the decode this pipeline already
        // performed so a guard and the Claude event payload do not parse the
        // arguments again (FR-007).
        let pre_hook_result = crate::hooks::run_pre_tool_use_hooks_decoded(
            &ctx.hook_configs,
            &ctx.hook_working_dir,
            &tc.name,
            &tc.args_json,
            Some(&input),
            session_id,
            Some(event_bus),
        );
        // SEC-ragent-agent-001 (SECTASKS T-007): `hook_approved` distinguishes
        // "a hook explicitly approved this call" from "no hook decided". It is
        // threaded into the permission stage, where it can satisfy an `Ask` but
        // never an explicit `Deny`.
        let mut hook_approved = false;
        // spec `openhands` T-015 (FR-006, FR-017): set when the security analyzer
        // returned `allow` for this call; it satisfies a bare `Ask` and never an
        // explicit `Deny` or a forced checkpoint.
        let mut security_approved = false;
        let tool_input: Value = match pre_hook_result {
            crate::hooks::PreToolUseResult::Allow {
                hook_approved: approved,
            } => {
                hook_approved = approved;
                input
            }
            crate::hooks::PreToolUseResult::Deny { reason } => {
                tracing::warn!(
                    tool = %tc.name,
                    reason = %reason,
                    "PreToolUse hook denied tool execution"
                );
                let err_msg = format!("Permission denied by hook: {reason}");
                return Self::denial(tc, &err_msg, input, event_bus, session_id);
            }
            crate::hooks::PreToolUseResult::Blocked { reason } => {
                tracing::warn!(
                    tool = %tc.name,
                    reason = %reason,
                    "PreToolUse hook blocked tool execution"
                );
                let err_msg = format!("Blocked by hook: {reason}");
                return Self::denial(tc, &err_msg, input, event_bus, session_id);
            }
            crate::hooks::PreToolUseResult::ModifiedInput { input } => input,
            crate::hooks::PreToolUseResult::NoDecision => input,
        };

        // Stage 9: repeat guard, on the canonical input (a hook may have
        // replaced it). Five identical consecutive calls pass; the sixth prompts
        // the user or is denied with a corrective observation.
        let repeat_guard_input = tool_input.clone();
        if let Some(guard_reason) = crate::session::permissions::check_tool_repeat_guard(
            &ctx.tool_repeat_guard,
            event_bus,
            session_id,
            &tc.name,
            &repeat_guard_input,
            ctx.agent_is_subagent,
            ctx.auto_approve == Some(true),
            ctx.checkpoint_timeout_secs,
        )
        .await
        {
            return Self::denial(tc, &guard_reason, repeat_guard_input, event_bus, session_id);
        }

        // Stage 10: concurrency permit. Acquiring it after every guard means a
        // refused call never holds one.
        let _permit = match crate::resource::acquire_tool_permit().await {
            Ok(permit) => permit,
            Err(e) => {
                let err_msg = format!("tool permit acquisition failed: {e}");
                // C3: close out the UI tool call - no `ToolCallEnd` is published
                // on this path, which used to leave the TUI spinner stuck on the
                // in-flight call.
                return Self::denial(tc, &err_msg, tool_input, event_bus, session_id);
            }
        };

        let start = std::time::Instant::now();
        // One serialisation of the resolved input, reused by the security
        // analyzer and by the hooks' `RAGENT_TOOL_INPUT` environment variable.
        let tool_input_for_post_hook =
            serde_json::to_string(&tool_input).unwrap_or_else(|_| tc.args_json.clone());

        // Stage 11: the LLM security analyzer (spec `openhands` FR-006, FR-017),
        // after the pre-tool hooks and before execution. The verdict is
        // published before the action is permitted or refused.
        if let Some((analyzer_client, analyzer_model, analyzer_timeout)) =
            ctx.security_analyzer.as_ref()
        {
            let verdict = crate::security::analyze_tool_action(
                analyzer_client,
                analyzer_model,
                &tc.name,
                &tool_input_for_post_hook,
                *analyzer_timeout,
            )
            .await;
            event_bus.publish(Event::SecurityVerdict {
                session_id: session_id.to_string(),
                tool: tc.name.clone(),
                verdict: verdict.label().to_string(),
                rationale: verdict.rationale.clone(),
            });
            match verdict.decision {
                crate::permission::PermissionAction::Deny => {
                    let reason =
                        format!("Action denied by security analyzer: {}", verdict.rationale);
                    return Self::denial(tc, &reason, tool_input.clone(), event_bus, session_id);
                }
                crate::permission::PermissionAction::Allow => security_approved = true,
                crate::permission::PermissionAction::Ask => {
                    // The analyzer escalated to a prompt: surface its rationale
                    // so the user sees why the call was flagged, then let the
                    // standard interactive flow decide.
                    event_bus.publish(Event::AgentNotice {
                        session_id: session_id.to_string(),
                        message: format!(
                            "security analyzer: {} - {}",
                            verdict.label(),
                            verdict.rationale
                        ),
                    });
                }
            }
        }

        // Stage 12: the permission checks and the backend dispatch. The 7-layer
        // bash validation, the path guards and the container/remote confinement
        // all live behind this call; the earlier stages have already decided
        // that the call is well-formed and wanted.
        let post_hook_decoded = tool_input.clone();
        // The decoded input is needed again after the dispatch move - by the
        // PostToolUse hooks and the memory-extraction engine - so the permission
        // layer receives a clone and this binding stays live for the
        // normalisation stage below.
        let dispatch_input = tool_input.clone();
        // FR-013: the call is marked unresolved for exactly the interval its
        // permission verdict is outstanding - from immediately before the layer
        // runs (it is the layer that publishes the prompt) until the verdict
        // resolves. A settled tracker is what permits the tool phase to append
        // this call's result; an unsettled one makes it refuse. The mark is taken
        // here rather than inside the permission layer so the interval covers the
        // whole stage, including a join failure after the wait was dropped.
        ctx.pending_verdicts.begin(session_id, &tc.id);
        let result = super::processor::dispatch_tool_with_permissions(
            &tool,
            dispatch_input,
            tc,
            &ctx.tool_ctx,
            &ctx.permission_checker,
            event_bus,
            session_id,
            ctx.auto_approve,
            checkpoint_forced,
            ctx.checkpoint_timeout_secs,
            hook_approved,
            security_approved,
            // The automatic sub-agent YOLO scope: a write-capable tool in an
            // unattended run. Resolved here so the permission layer receives
            // one boolean rather than a sub-agent question it would have to
            // answer twice.
            crate::session::permissions::subagent_yolo_bypasses_prompt(
                ctx.agent_is_subagent,
                &ctx.write_capable_tools,
                &tc.name,
            ),
        )
        .await;
        // Every exit from the permission stage is a resolved verdict: an allow,
        // a deny, a policy refusal and a timed-out prompt (which counts as a
        // denial) all reach this line, and none of them leaves the call awaiting
        // an answer.
        ctx.pending_verdicts.settle(session_id, &tc.id);
        let duration_ms = start.elapsed().as_millis() as u64;

        // Stage 13: result normalisation.
        let tool_recorder = crate::telemetry::ToolRecorder::from_subsystem(&ctx.telemetry);
        tool_recorder.record_invocation(&tc.name);
        tool_recorder.record_duration(&tc.name, duration_ms as f64);
        if tc.name.starts_with("team_") {
            ctx.team_context_cache.write().clear();
        }
        let output_content = result
            .as_ref()
            .map(|o| o.content.clone())
            .unwrap_or_default();
        let output_json = result
            .as_ref()
            .ok()
            .and_then(|o| o.metadata.clone())
            .unwrap_or_else(|| serde_json::json!({ "content": output_content }));
        let success = result.is_ok();
        let mut additional_contexts: Vec<String> = Vec::new();
        let post_hook_result = crate::hooks::run_post_tool_use_hooks_decoded(
            &ctx.hook_configs,
            &ctx.hook_working_dir,
            &tc.name,
            &tool_input_for_post_hook,
            Some(&post_hook_decoded),
            &output_json.to_string(),
            success,
            session_id,
            Some(event_bus),
        )
        .await;
        let modified_output = match post_hook_result {
            crate::hooks::PostToolUseResult::Ok {
                modified_output,
                additional_context,
            } => {
                additional_contexts.extend(additional_context);
                modified_output
            }
            crate::hooks::PostToolUseResult::Flagged { reason } => {
                tracing::info!(
                    tool = %tc.name,
                    reason = %reason,
                    "PostToolUse hook flagged tool result as policy-violated"
                );
                None
            }
            crate::hooks::PostToolUseResult::Warn { message } => {
                // SEC: hook-supplied text can echo user/tool data; redact before
                // logging.
                tracing::info!(
                    tool = %tc.name,
                    message = %ragent_types::sanitize::redact_secrets(&message),
                    "PostToolUse hook emitted warning"
                );
                None
            }
        };
        let result = match modified_output {
            Some(modified) => match modified.get("content").and_then(Value::as_str) {
                Some(modified_content) => Ok(crate::tool::ToolOutput {
                    content: modified_content.to_string(),
                    metadata: Some(modified),
                }),
                None => result,
            },
            None => result,
        };
        let (output_value, error) = match &result {
            Ok(output) => {
                let val = match &output.metadata {
                    Some(meta) if meta.is_object() => {
                        let mut obj = meta.clone();
                        if let Some(map) = obj.as_object_mut() {
                            map.insert("content".to_string(), serde_json::json!(output.content));
                        }
                        obj
                    }
                    _ => serde_json::json!({ "content": output.content }),
                };
                (Some(val), None)
            }
            Err(e) => (None, Some(format!("{e:#}"))),
        };
        if let Some(err_msg) = &error
            && err_msg.contains("permission denied")
        {
            crate::hooks::fire_hooks(
                &ctx.hook_configs,
                crate::hooks::HookTrigger::OnPermissionDenied,
                &ctx.hook_working_dir,
                &[("RAGENT_ERROR", err_msg.as_str())],
            );
        }
        let status = if result.is_ok() {
            ToolCallStatus::Completed
        } else {
            ToolCallStatus::Error
        };
        // Recovery for a message-window display race: when a tool call fails with
        // a permission error, the TUI drains the event queue in the same wake as
        // the `PermissionReplied` handler and re-renders the chat, removing the
        // in-flight ToolCall part that `ToolCallStart` created. The queued
        // `ToolCallArgs` then finds no part and is buffered in
        // `pending_tool_args`, while the re-created part renders with an empty
        // input. Republish the args now, after the part has been re-created.
        if result.is_err()
            && error
                .as_deref()
                .is_some_and(|e| e.contains("Permission denied") || e.contains("Blocked by hook"))
        {
            event_bus.publish(Event::ToolCallArgs {
                session_id: session_id.to_string(),
                call_id: tc.id.clone(),
                tool: tc.name.clone(),
                args: tc.args_json.clone(),
            });
        }
        let success = status == ToolCallStatus::Completed;
        event_bus.publish(Event::ToolCallEnd {
            session_id: session_id.to_string(),
            call_id: tc.id.clone(),
            tool: tc.name.clone(),
            error: error.clone(),
            duration_ms,
        });
        let result_content = match &result {
            Ok(output) => output.content.clone(),
            Err(e) => format!("Error: {e}"),
        };
        let content_line_count = result
            .as_ref()
            .ok()
            .and_then(|o| o.metadata.as_ref())
            .and_then(|m| m.get("lines"))
            .and_then(Value::as_u64)
            .map_or_else(|| result_content.lines().count(), |n| n as usize);
        // M-007: publish only a short display preview on the event; the LLM gets
        // the full content through `tool_result_content_for_llm`.
        const TOOL_RESULT_EVENT_PREVIEW_CHARS: usize = 2000;
        let result_preview =
            super::processor::truncate_preview(&result_content, TOOL_RESULT_EVENT_PREVIEW_CHARS);
        let tool_metadata = result.as_ref().ok().and_then(|o| o.metadata.clone());
        event_bus.publish(Event::ToolResult {
            session_id: session_id.to_string(),
            call_id: tc.id.clone(),
            tool: tc.name.clone(),
            content: result_preview,
            content_line_count,
            metadata: tool_metadata.clone(),
            success,
        });
        if let Some(engine) = ctx.extraction_engine.get() {
            engine.on_tool_result(
                &tc.name,
                &tool_input,
                &result_content,
                success,
                session_id,
                &ctx.storage,
                event_bus,
                &ctx.hook_working_dir,
            );
        }
        (
            tc.clone(),
            tool_input,
            status,
            output_value,
            error,
            duration_ms,
            result_content,
            tool_metadata,
            additional_contexts,
        )
    }

    /// Builds the outcome for a call a stage refused, and closes the call out on
    /// the UI first.
    ///
    /// Every refusal in the pipeline goes through here, so a refusal always
    /// publishes `ToolCallEnd` (leaving no spinner spinning) and always returns
    /// the same shape with [`ToolCallStatus::Error`] - whose
    /// [`is_error`](ToolCallStatus::is_error) is the result flag (FR-014).
    #[must_use]
    pub(crate) fn denial(
        tc: &PendingToolCall,
        reason: &str,
        input: Value,
        event_bus: &Arc<crate::event::EventBus>,
        session_id: &str,
    ) -> ToolOutcome {
        event_bus.publish(Event::ToolCallEnd {
            session_id: session_id.to_string(),
            call_id: tc.id.clone(),
            tool: tc.name.clone(),
            error: Some(reason.to_string()),
            duration_ms: 0,
        });
        (
            tc.clone(),
            input,
            ToolCallStatus::Error,
            None,
            Some(reason.to_string()),
            0u64,
            String::new(),
            None,
            Vec::new(),
        )
    }

    /// Builds the outcome for a call refused *before* a guard ran: an
    /// unregistered name, an undecodable payload, or a schema violation
    /// (FR-008, FR-009, FR-021).
    ///
    /// The sentence, the status and the `is_error` flag all come from the
    /// [`ToolInputError`] value ([`crate::session::tool_input_error`]), so this
    /// adapter states the content of the refusal nowhere - it cannot drift from
    /// the constructor.
    ///
    /// The tuple's seventh field is the text the model reads. A refusal is
    /// reported through `Error: <sentence>` exactly like a failed run: before
    /// this adapter used the bare, unprefixed sentence, so a refused call
    /// reached the model as a plain statement where a provider or a client keyed
    /// on the `Error: ` marker saw it as a success report - the same status, two
    /// spellings.
    #[must_use]
    pub(crate) fn refusal(
        tc: &PendingToolCall,
        refusal: &ToolInputError,
        input: Value,
        event_bus: &Arc<crate::event::EventBus>,
        session_id: &str,
    ) -> ToolOutcome {
        let message = refusal.to_string();
        event_bus.publish(Event::ToolCallEnd {
            session_id: session_id.to_string(),
            call_id: tc.id.clone(),
            tool: tc.name.clone(),
            error: Some(message.clone()),
            duration_ms: 0,
        });
        (
            tc.clone(),
            input,
            refusal.status(),
            None,
            Some(message.clone()),
            0u64,
            format!("Error: {message}"),
            None,
            Vec::new(),
        )
    }

    /// Builds the outcome for a call whose task never produced a result: it
    /// panicked, failed to join, or was aborted by the watchdog.
    ///
    /// A join failure is not a tool result, but the `tool_use` record still has
    /// to be answered - an orphaned record leaves the conversation with a call
    /// whose outcome the model never sees. This constructor is the one place
    /// that answers it, so the sequential and the concurrent shape cannot
    /// disagree about the text, the status or the flag (FR-012).
    #[must_use]
    pub(crate) fn synthesised(
        tc: &PendingToolCall,
        message: &str,
        duration_ms: u64,
        event_bus: &Arc<crate::event::EventBus>,
        session_id: &str,
    ) -> ToolOutcome {
        event_bus.publish(Event::ToolCallEnd {
            session_id: session_id.to_string(),
            call_id: tc.id.clone(),
            tool: tc.name.clone(),
            error: Some(message.to_string()),
            duration_ms,
        });
        (
            tc.clone(),
            Value::Null,
            ToolCallStatus::Error,
            None,
            Some(message.to_string()),
            duration_ms,
            format!("Error: {message}"),
            None,
            Vec::new(),
        )
    }
}

/// The result of the pipeline's first three stages for one call.
///
/// `tool` is the resolved tool or the refusal that stopped the call. It is
/// computed by [`resolve_tool_call`] and then consumed by
/// [`ToolDispatchPipeline::run`], so the decode and the resolve happen exactly
/// once per call and every later stage sees the same value.
pub struct ResolvedToolCall {
    /// The decoded arguments.
    ///
    /// `Value::Null` only when the payload would not decode at all, which is
    /// also the fail-closed value the loop restriction and the hooks receive.
    pub input: Value,
    /// The registered tool, or the refusal that stopped the call.
    pub tool: std::result::Result<Arc<dyn Tool>, ToolInputError>,
}

/// Runs stages 1-3 for one call: decode the arguments, resolve the tool, and
/// validate the arguments against the tool's declared schema.
///
/// The order matters (FR-012, FR-021):
///
/// 1. decode - so the refusal for a bad payload can quote the parser, and so a
///    tool name that does not exist is *also* reported with its own refusal
///    rather than a parse error;
/// 2. resolve - the unknown-tool refusal with its did-you-mean hint. Nothing
///    executes, and the registry is the only source of the suggestion;
/// 3. validate - the schema violation, reported against the tool that will not
///    run.
///
/// Both refusals are [`ToolInputError`] values, so their sentence, record status
/// and `is_error` flag are one derivation.
#[must_use]
pub(crate) fn resolve_tool_call(
    registry: &crate::tool::ToolRegistry,
    tc: &PendingToolCall,
) -> ResolvedToolCall {
    resolve_call(registry, &tc.name, &tc.args_json)
}

/// Runs stages 1-3 for one call named by `name` with accumulated `args_json`.
///
/// This is the body [`resolve_tool_call`] wraps: taking the name and the raw
/// argument text keeps the pipeline's first three stages callable from an
/// integration test and from any other caller that has a name and a payload but
/// no loop record.
#[must_use]
pub fn resolve_call(
    registry: &crate::tool::ToolRegistry,
    name: &str,
    args_json: &str,
) -> ResolvedToolCall {
    let decoded = match ragent_types::tool_args::decode_tool_arguments(args_json) {
        Ok(value) => value,
        Err(e) => {
            tracing::warn!(
                error = %e,
                // SEC: arguments can carry credentials; redact before logging.
                args = %ragent_types::sanitize::redact_secrets(args_json),
                "Failed to parse tool call arguments"
            );
            // A payload that is not a JSON object is also, as far as the tool's
            // schema is concerned, missing every required parameter. Both
            // reasons go into the one corrective sentence so the model can fix
            // the call in a single retry (FR-008). The lookup tries the exact
            // spelling, then a unique letter-case fold, so a payload sent for a
            // name spelled in another case is still measured against the schema
            // of the tool the model meant.
            let refusal = match registry.get(name) {
                Some(tool) => match ragent_tools_core::schema::validate_required_args(
                    &tool.parameters_schema(),
                    &Value::Null,
                ) {
                    Err(reason) => ToolInputError::undecodable_with_reason(name, &e, &reason),
                    Ok(()) => ToolInputError::undecodable(name, &e),
                },
                None => ToolInputError::undecodable(name, &e),
            };
            return ResolvedToolCall {
                input: Value::Null,
                tool: Err(refusal),
            };
        }
    };

    // FR-021: an unregistered name never executes, and the refusal names the
    // closest registered name when one is within the similarity threshold.
    //
    // The lookup is `ToolRegistry::resolve`, which tries the exact spelling
    // first and then a unique ASCII-case-insensitive match, so a provider that
    // spells `Bash` as `bash` still reaches the tool. The loop canonicalises the
    // name before this point, so in the agent loop the exact lookup already
    // succeeds; the fallback is what keeps a direct caller (`resolve_call` is
    // public, and an ACP or OpenAI-surface caller may not have canonicalised)
    // from refusing a call that names a real tool.
    let Some(tool) = registry.get(name) else {
        let suggestion = suggest_tool_name(registry, name);
        tracing::warn!(
            tool = %name,
            suggestion = suggestion.as_deref().unwrap_or("<none>"),
            "unknown tool name; the call was refused"
        );
        return ResolvedToolCall {
            input: decoded,
            tool: Err(ToolInputError::unknown_tool(name, suggestion.as_deref())),
        };
    };

    // Argument keys that differ from a declared property only by letter case
    // (`Path` for `path`, `EXPRESSION` for `expression`) are renamed onto the
    // declared spelling before the schema check and before the tool body reads
    // `input["<declared>"]`. Without this a payload whose value is present under
    // a differently-cased key was refused as "missing required parameter".
    let schema = tool.parameters_schema();
    let decoded = ragent_tools_core::schema::fold_argument_keys(&schema, decoded);

    if let Err(reason) = ragent_tools_core::schema::validate_required_args(&schema, &decoded) {
        return ResolvedToolCall {
            input: decoded,
            tool: Err(ToolInputError::schema_violation(name, &reason)),
        };
    }

    ResolvedToolCall {
        input: decoded,
        tool: Ok(tool),
    }
}

/// The registered spelling of a tool name a provider sent, when it differs from
/// `name` only by ASCII letter case.
///
/// Returns `Some(canonical)` for `bash` -> `Bash`, `READ` -> `read` and
/// `WRITE` -> `Write`, and `None` when `name` is already registered exactly or
/// names no tool at all. The loop rewrites the pending call's name with this, so
/// every later stage - the loop restriction, `LOOP_WRITE_TOOLS`,
/// `is_destructive_tool`, the permission layer, the hooks, the repeat guard, the
/// telemetry key and the history record - sees one canonical spelling.
///
/// That rewrite is the reason this is not just a lookup fallback: a call that
/// stayed named `bash` would reach the permission layer as an unrecognised name
/// and skip the destructive-action checkpoint the `Bash` arm applies.
#[must_use]
pub fn canonical_dispatch_name(registry: &crate::tool::ToolRegistry, name: &str) -> Option<String> {
    if registry.get(name).is_some() {
        return None;
    }
    let registered = registry.list();
    let mut folded = registered
        .iter()
        .filter(|candidate| candidate.eq_ignore_ascii_case(name));
    let canonical = folded.next()?;
    // Only a *unique* case-insensitive match is a rewrite: two registered names
    // that differ only by case must not make the choice depend on iteration
    // order.
    if folded.next().is_some() {
        return None;
    }
    Some(canonical.clone())
}

/// The closest registered tool name to `name`, or `None` when nothing is within
/// the similarity threshold (FR-021's did-you-mean hint).
///
/// This is a re-export of the one pool
/// ([`crate::session::tool_suggestion::suggest_tool_name`]) rather than a second
/// implementation: the ranking, the thresholds and the edit-distance helper used
/// to live here *and* in `processor.rs`, so the pipeline's hint and the loop's
/// hint were two policies that could be tuned apart (FR-033).
#[must_use]
pub fn suggest_tool_name(registry: &crate::tool::ToolRegistry, name: &str) -> Option<String> {
    crate::session::tool_suggestion::suggest_tool_name(registry, name)
}

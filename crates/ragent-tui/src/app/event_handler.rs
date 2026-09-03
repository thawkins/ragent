//! Event bus event handling for the TUI.
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ragent_agent::{
    agent::ModelRef,
    event::{Event, FinishReason},
    message::{Message, MessagePart, Role},
    permission::PermissionRequest,
    provider::ModelInfo,
};
use ragent_team::team::{
    self, Mailbox, MailboxMessage, MemberStatus, MessageType, TeamMember, TeamStore,
};
use ragent_telemetry::counters as telemetry_counters;

// Prompt optimization templates

// State types from app/state.rs
use crate::app::state::{
    App, BgTaskView, LlmRequestStat, LogLevel, ModelDownloadState, ModelLoadingState,
    OutputViewLineCache, OutputViewState, OutputViewTarget, PlanApprovalState, ProviderSetupStep,
    QuestionRequest, ResearchViewState,
};

// Helpers
use crate::app::helpers::{short_session_id, summarise_error};
use crate::app::session_ops::recover_poisoned;
use crate::widgets::message_widget::truncate_str;

// Re-export status types from theme

impl App {
    /// Poll the pending swarm decomposition result and, if ready, render the
    /// team summary (or surface an error) into the chat log.
    pub(crate) fn poll_pending_swarm(&mut self) {
        let outcome = {
            let mut guard = recover_poisoned(self.swarm_result.lock(), "swarm_result");
            guard.take()
        };
        let Some(outcome) = outcome else { return };
        match outcome {
            Ok(raw_json) => {
                let default_agent_type = self
                    .swarm_state
                    .as_ref()
                    .and_then(|s| s.default_agent_type.as_deref());
                match team::parse_decomposition_with_default(&raw_json, default_agent_type) {
                    Ok(decomposition) => {
                        self.execute_swarm_decomposition(decomposition);
                    }
                    Err(msg) => {
                        self.status = "[warn] swarm: decomposition parse error".to_string();
                        self.append_assistant_text(&format!(
                            "From: /swarm\n## [err] Decomposition Failed\n\n{}\n",
                            msg
                        ));
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            format!("Swarm parse error: {}", msg),
                        );
                    }
                }
            }
            Err(msg) => {
                self.status = format!("[warn] swarm failed: {}", msg);
                self.append_assistant_text(&format!(
                    "From: /swarm\n## [err] Swarm Error\n\n{}\n",
                    msg
                ));
                self.push_log_no_agent(LogLevel::Warn, format!("Swarm error: {}", msg));
            }
        }
    }

    pub(crate) fn execute_plan_delegation(
        &mut self,
        session_id: &str,
        task: String,
        context: String,
    ) {
        // Push current agent to stack so plan_exit can restore it
        self.agent_stack.push(self.agent_info.clone());

        // Find and switch to the plan agent
        let plan_agent = self
            .cycleable_agents
            .iter()
            .find(|a| a.name == "plan")
            .cloned();

        if let Some(mut plan) = plan_agent {
            let prev_name = self.agent_name.clone();

            // Apply current model override to plan agent
            if let Some(ref model_str) = self.selected_model {
                if let Some((provider, model)) = model_str.split_once('/') {
                    plan.model = Some(ModelRef {
                        provider_id: provider.to_string(),
                        model_id: model.to_string(),
                    });
                }
            }

            self.agent_info = plan.clone();
            self.agent_name = "plan".to_string();
            self.status = format!("agent: plan (delegated from {})", prev_name);
            self.push_log_no_agent(
                LogLevel::Info,
                format!("plan delegation: {} → plan", prev_name),
            );

            // Publish the switch event
            self.event_bus.publish(Event::AgentSwitched {
                session_id: session_id.to_string(),
                from: prev_name,
                to: "plan".to_string(),
            });

            // Build the task message
            let full_task = if context.is_empty() {
                task
            } else {
                format!("{}\n\nContext:\n{}", task, context)
            };

            // Add user message to UI
            let sid = session_id.to_string();
            let msg = Message::user_text(&sid, &full_task);
            self.messages.push(msg);
            self.trim_messages_if_needed();

            // Spawn async processing
            let processor = self.session_processor.clone();
            let agent = self.agent_info.clone();
            let task_text = full_task;
            tokio::spawn(async move {
                if let Err(e) = processor
                    .process_message(&sid, &task_text, &agent, Arc::new(AtomicBool::new(false)))
                    .await
                {
                    tracing::debug!(error = %e, "Plan agent failed");
                }
            });
        } else {
            self.push_log_no_agent(LogLevel::Error, "plan agent not found".to_string());
            // Pop the agent we just pushed since we can't delegate
            self.agent_stack.pop();
        }
    }

    /// Returns `true` for tool-lifecycle events that the safety-net drain in
    /// [`Self::handle_event`] must not run after, because a handler earlier in
    /// the same pass is responsible for applying `pending_tool_args`.
    fn event_handles_pending_args(event: &Event) -> bool {
        matches!(
            event,
            Event::ToolCallArgs { .. } | Event::ToolCallStart { .. } | Event::ToolCallBatch { .. }
        )
    }

    /// Dispatch a single [`Event`] from the event bus to the appropriate UI
    /// handler. Marks the UI dirty on every event so the next render reflects
    /// the state change.
    pub fn handle_event(&mut self, event: Event) {
        // After the previous event, re-apply any buffered tool args to parts
        // that may have been created out-of-order since (safety net for the
        // rare pending_args-leak path; see `drain_pending_tool_args`).
        if Self::event_handles_pending_args(&event) {
            self.drain_pending_tool_args();
        }
        // Mark UI dirty for any event handling
        self.needs_redraw = true;
        match event {
            Event::SessionCreated { ref session_id } if self.session_id.is_none() => {
                self.session_id = Some(session_id.clone());
                // Map the primary session's short_sid to the current agent name
                let short_sid = short_session_id(session_id);
                self.sid_to_display_name
                    .insert(short_sid, self.agent_name.clone());
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!(
                        "session created: {}",
                        &session_id[..8.min(session_id.len())]
                    ),
                );
                telemetry_counters::increment_sessions_total(1);
                telemetry_counters::add_sessions_active(1);
            }
            Event::TextDelta {
                ref session_id,
                ref text,
            } if self.is_current_session(session_id) => {
                self.stream_in_bytes += text.len() as u64;
                self.append_assistant_text(text);
            }
            Event::OpenResearchView {
                ref name,
                ref path,
                ref markdown,
            } => {
                let md = markdown.clone();
                let p = path.clone();
                let n = name.clone();
                self.status = format!("research: viewing {n}");
                self.open_research_view(n, p, md);
                self.needs_redraw = true;
            }
            Event::ReasoningDelta {
                ref session_id,
                ref text,
            } if self.is_current_session(session_id) => {
                self.stream_in_bytes += text.len() as u64;
                self.append_reasoning_text(text);
            }
            Event::CompressionStarted { ref session_id, .. }
                if self.is_current_session(session_id) =>
            {
                self.compress_in_progress = true;
                self.status = "compressing context...".to_string();
                self.needs_redraw = true;
                self.push_log_no_agent(LogLevel::Info, "Context compression started".to_string());
            }
            Event::CompressionFinished {
                ref session_id,
                original_tokens,
                compressed_tokens,
                compression_ratio,
                did_compress,
                ..
            } if self.is_current_session(session_id) => {
                self.compress_in_progress = false;
                self.last_input_tokens = compressed_tokens as u64;
                self.needs_redraw = true;
                telemetry_counters::increment_context_compressions(1);
                if did_compress {
                    let saved = original_tokens.saturating_sub(compressed_tokens);
                    self.status = format!(
                        "compress: saved {} tokens ({:.0}%)",
                        saved,
                        (1.0 - 1.0 / compression_ratio) * 100.0
                    );
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!(
                            "Context compression finished: {} → {} tokens ({:.2}x ratio, saved {})",
                            original_tokens, compressed_tokens, compression_ratio, saved
                        ),
                    );
                } else {
                    self.status = format!(
                        "compress: no change ({} tokens, {:.0}% threshold)",
                        original_tokens, 80.0
                    );
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!(
                            "Context compression finished: no change ({} tokens)",
                            original_tokens
                        ),
                    );
                }
            }
            Event::RequestStarted {
                ref session_id,
                outbound_bytes,
            } if self.is_current_session(session_id) => {
                self.stream_in_bytes = 0;
                self.stream_out_bytes = outbound_bytes;
                telemetry_counters::increment_llm_requests(1);
            }
            Event::ToolCallStart {
                ref session_id,
                ref call_id,
                ref tool,
            } if self.is_current_session(session_id)
                || self.is_tracked_agent_session(session_id) =>
            {
                let is_primary = self.is_current_session(session_id);
                if is_primary {
                    self.stream_in_bytes += (call_id.len() + tool.len()) as u64;
                    telemetry_counters::increment_tool_invocations(1);
                }
                // Get the current step count from the event bus (single source of truth)
                let step = self.event_bus.current_step(session_id) as u32;
                let short_sid = short_session_id(session_id);
                // Check if step changed - if so, reset substep counter to 0
                let last_step = self
                    .last_step_per_session
                    .get(session_id)
                    .copied()
                    .unwrap_or(0);
                if step != last_step {
                    self.substep_counter_per_session
                        .insert(session_id.clone(), 0);
                    self.last_step_per_session.insert(session_id.clone(), step);
                }
                // Increment sub-step counter for this session
                let substep = self
                    .substep_counter_per_session
                    .entry(session_id.clone())
                    .or_insert(0);
                *substep += 1;
                let current_substep = *substep;
                self.tool_step_map
                    .insert(call_id.clone(), (short_sid.clone(), step, current_substep));
                if is_primary {
                    self.add_tool_call_part(tool, call_id);

                    // If args were received before the start event, apply them now.
                    if let Some(args_json) = self.pending_tool_args.remove(call_id) {
                        let _ = self.update_tool_call_input(call_id, &args_json);
                    }
                    self.status = format!("running: {}", tool);
                }
                // Log the step for the primary session AND for every tracked
                // sub-agent/teammate session so the Agents/Teams panels' step
                // count (one per ToolCallStart) is visible in the log.
                let (agent_tag, agent_prefix, log_session_id) =
                    self.tracked_log_context(session_id, is_primary);
                self.push_log_for(
                    LogLevel::Tool,
                    format!("{agent_prefix}[{step}.{current_substep}] tool call: {tool}"),
                    agent_tag,
                    log_session_id,
                );
                self.needs_redraw = true;
            }
            Event::ToolCallEnd {
                ref session_id,
                ref call_id,
                ref tool,
                ref error,
                duration_ms,
            } if self.is_current_session(session_id)
                || self.is_tracked_agent_session(session_id) =>
            {
                let is_primary = self.is_current_session(session_id);
                if is_primary {
                    telemetry_counters::set_tool_duration_last(duration_ms as f64);
                    self.update_tool_call_status(
                        call_id,
                        error.is_none(),
                        error.as_deref(),
                        duration_ms,
                    );
                    self.set_status_working("processing");
                }
                let step_tag = self
                    .tool_step_map
                    .get(call_id)
                    .map(|(_sid, step, substep)| format!("[{step}.{substep}] "))
                    .unwrap_or_default();
                let (agent_tag, agent_prefix, log_session_id) =
                    self.tracked_log_context(session_id, is_primary);
                if let Some(err) = error {
                    self.push_log_for(
                        LogLevel::Error,
                        format!(
                            "{agent_prefix}{step_tag}tool {tool} failed: {err} ({duration_ms}ms)"
                        ),
                        agent_tag,
                        log_session_id,
                    );
                } else {
                    let message =
                        format!("{agent_prefix}{step_tag}tool {tool} completed ({duration_ms}ms)");
                    self.push_log_for(LogLevel::Tool, message, agent_tag, log_session_id);
                }
                if is_primary {
                    // T-010/FR-013: the tool-call state changed, so refresh the
                    // Context panel snapshot off the UI thread (no-op when the
                    // panel is closed or a refresh is already in flight).
                    self.schedule_context_snapshot_refresh();
                }
                self.needs_redraw = true;
            }
            Event::ToolCallBatch {
                ref session_id,
                ref calls,
                ..
            } if self.is_current_session(session_id)
                || self.is_tracked_agent_session(session_id) =>
            {
                let is_primary = self.is_current_session(session_id);
                // Atomic fallback: if per-call ToolCallStart/End events were
                // dropped by the broadcast bridge during a burst, the batch
                // still carries the final status/duration for every call.
                for entry in calls {
                    // If the ToolCallStart event was dropped we never tagged
                    // the call with a step/substep, so the log would show one
                    // fewer "tool call" line than the Agents/Teams panels'
                    // step count (which counts every ToolCallStart). Rebuild
                    // the tag and log the missing line now.
                    if !self.tool_step_map.contains_key(&entry.call_id) {
                        let step = self.event_bus.current_step(session_id) as u32;
                        let short_sid = short_session_id(session_id);
                        let last_step = self
                            .last_step_per_session
                            .get(session_id)
                            .copied()
                            .unwrap_or(0);
                        if step != last_step {
                            self.substep_counter_per_session
                                .insert(session_id.clone(), 0);
                            self.last_step_per_session.insert(session_id.clone(), step);
                        }
                        let substep = self
                            .substep_counter_per_session
                            .entry(session_id.clone())
                            .or_insert(0);
                        *substep += 1;
                        let current_substep = *substep;
                        self.tool_step_map.insert(
                            entry.call_id.clone(),
                            (short_sid.clone(), step, current_substep),
                        );
                        let (agent_tag, agent_prefix, log_session_id) =
                            self.tracked_log_context(session_id, is_primary);
                        let message = format!(
                            "{agent_prefix}[{step}.{current_substep}] tool call: {}",
                            entry.tool
                        );
                        self.push_log_for(LogLevel::Tool, message, agent_tag, log_session_id);
                    }
                    if is_primary {
                        if !self.find_tool_call_part(&entry.call_id) {
                            self.add_tool_call_part(&entry.tool, &entry.call_id);
                        }
                        // If the start event was dropped we never consumed the
                        // pending ToolCallArgs, so the newly-created part would
                        // render with a missing input summary. Apply any pending
                        // args now so the header shows the path/command/etc.
                        if let Some(args_json) = self.pending_tool_args.remove(&entry.call_id) {
                            let _ = self.update_tool_call_input(&entry.call_id, &args_json);
                        }
                        // The batch entry now carries the call's raw JSON args, so
                        // apply them as a fallback when the per-call ToolCallArgs
                        // event never arrived (e.g. the broadcast→mpsc bridge task
                        // aborted after a Lagged error racing a permission prompt).
                        // Pending args already applied above take precedence.
                        let _ = self.update_tool_call_input(&entry.call_id, &entry.args);
                        self.update_tool_call_status(
                            &entry.call_id,
                            entry.success,
                            entry.error.as_deref(),
                            entry.duration_ms,
                        );
                    }
                }
                if is_primary {
                    // T-010/FR-013: atomic batch finalized multiple tool calls,
                    // so refresh the Context panel snapshot off the UI thread.
                    self.schedule_context_snapshot_refresh();
                }
                self.needs_redraw = true;
            }
            Event::MessageStart {
                ref session_id,
                ref message_id,
            } if self.is_current_session(session_id) => {
                self.is_processing = true;
                self.last_task_completed_at = None;
                self.agent_halted = false;
                self.set_status_working("processing");
                telemetry_counters::increment_messages_user(1);
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!(
                        "response started ({})",
                        &message_id[..8.min(message_id.len())]
                    ),
                );
            }
            Event::MessageEnd {
                ref session_id,
                ref message_id,
                ref reason,
            } if self.is_current_session(session_id) => {
                // The "init" message_id is used exclusively by the AGENTS.md
                // acknowledgment exchange that runs before the main agent loop.
                // It must NOT reset processing state — the main loop hasn't
                // started yet.  Only set force_new_message so the real response
                // starts in a fresh message block.
                if message_id == "init" {
                    self.force_new_message = true;
                    return;
                }
                telemetry_counters::add_sessions_active(-1);
                self.is_processing = false;
                self.cancel_flag = None;
                if *reason == FinishReason::Cancelled {
                    self.agent_halted = true;
                    self.status = "halted — /resume to continue".to_string();
                    self.push_log_no_agent(LogLevel::Warn, "Agent halted by user".to_string());
                } else {
                    self.agent_halted = false;
                    self.status = "ready".to_string();
                    self.status_set_at = None;
                }
                self.force_new_message = true;
                self.push_log_no_agent(LogLevel::Info, format!("response finished ({reason:?})"));
                // T-010/FR-013: the conversation changed, so refresh the
                // Context panel snapshot off the UI thread (no-op when the
                // panel is closed or a refresh is already in flight).
                self.schedule_context_snapshot_refresh();

                // Compaction no longer runs through the agent loop
                // (SessionProcessor::compact_session is a direct runner call),
                // so no MessageEnd arrives for it. Compaction completion and
                // queued-send dispatch are handled by poll_compaction_result.
                self.compact_in_progress = false;
                // Session title generation has been removed along with the
                // internal LLM subsystem; sessions default to an empty
                // title and rely on the user to rename them.
                let _ = (session_id, reason);
                // Handle pending plan delegation: switch agent and auto-send task
                if let Some((task, context)) = self.pending_plan_task.take() {
                    self.execute_plan_delegation(session_id, task, context);
                }

                // FR-012: /reverse --create <name> chaining — after the LLM
                // finishes generating the synthetic prompt, invoke
                // `/spec create <name> <generated-prompt>` using the last
                // assistant message as the prompt text.
                if let Some(spec_name) = self.pending_reverse_create.take() {
                    if *reason != FinishReason::Cancelled {
                        let prompt = self
                            .messages
                            .iter()
                            .rev()
                            .find(|m| m.role == Role::Assistant)
                            .map(|m| m.text_content())
                            .unwrap_or_default();
                        if !prompt.is_empty() {
                            let cmd = format!("/spec create {spec_name} {prompt}");
                            self.execute_slash_command(&cmd);
                        }
                    }
                }

                // /spec impl sequential driver: after each agent turn ends,
                // check the just-run task's status and dispatch the next task
                // (or finish the run). Skip when the turn was cancelled so the
                // user can resume manually with `/spec impl`.
                if self.spec_impl_state.is_some() && *reason != FinishReason::Cancelled {
                    self.advance_spec_impl();
                }

                // Autopilot auto-continue: after agent completes a turn without calling
                // agent_complete, automatically send a continuation prompt so the agent
                // keeps working towards its goal.
                // If TaskCompleted was consumed before us, autopilot_enabled will already
                // be false and this block is unreachable — this is just a defensive fallback.
                if self.autopilot_enabled
                    && *reason != FinishReason::Cancelled
                    && self.last_task_completed_at.is_none()
                {
                    // Check time limit
                    let time_exceeded = self
                        .autopilot_time_limit_secs
                        .and_then(|limit| {
                            self.autopilot_started_at
                                .map(|s| s.elapsed().as_secs() >= limit)
                        })
                        .unwrap_or(false);
                    if time_exceeded {
                        self.autopilot_enabled = false;
                        self.autopilot_started_at = None;
                        self.autopilot_pending_continue = None;
                        self.status = "autopilot: time limit reached".to_string();
                        self.append_assistant_text(
                            "⚡ **Autopilot stopped** — time limit reached.",
                        );
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            "autopilot stopped: time limit".to_string(),
                        );
                    } else {
                        // Schedule a continuation on the next render tick
                        self.autopilot_pending_continue = Some(
                                "Continue working on the task. When fully done, call agent_complete with a summary.".to_string()
                            );
                        self.status = "⚡ autopilot".to_string();
                    }
                }

                // Auto-compaction no longer runs through the agent loop, so a
                // MessageEnd can only be the init/prepare_client-failure echo
                // of a compaction attempt and must not dispatch the queued
                // send. poll_compaction_result handles all compaction
                // completion paths. Consume the flag so the stale value cannot
                // leak into a later turn-end.
                self.auto_compact_in_progress = false;
            }
            Event::PermissionRequested {
                ref session_id,
                ref request_id,
                ref permission,
                ref description,
                ref options,
            } => {
                if self.is_current_session(session_id) {
                    // Deduplicate: skip if this request_id is already queued.
                    if self.permission_queue.iter().any(|r| r.id == *request_id) {
                        tracing::warn!(
                            request_id = %request_id,
                            "Duplicate PermissionRequested ignored"
                        );
                    } else {
                        tracing::info!(
                            session_id = %session_id,
                            request_id = %request_id,
                            permission = %permission,
                            "TUI received PermissionRequested, showing dialog"
                        );
                        let created_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        self.permission_queue.push_back(PermissionRequest {
                            id: request_id.clone(),
                            session_id: session_id.clone(),
                            permission: permission.clone(),
                            patterns: vec![description.clone()],
                            metadata: serde_json::json!({
                                "created_at": created_at,
                                "timeout_secs": 120u64,
                                "options": options,
                            }),
                            tool_call_id: None,
                        });
                        self.question_selected_index = 0;
                        self.status = "awaiting permission".to_string();
                        self.push_log_no_agent(
                            LogLevel::Warn,
                            format!("permission requested: {} — {}", permission, description),
                        );
                    }
                } else {
                    tracing::warn!(
                        expected_session = %self.session_id.as_deref().unwrap_or("none"),
                        received_session = %session_id,
                        permission = %permission,
                        "Ignoring PermissionRequested for different session"
                    );
                }
            }
            Event::QuestionRequested {
                ref session_id,
                ref request_id,
                ref question,
                ref options,
            } => {
                // Allow questions from any session (including sub-agents) to be displayed
                // since they require user interaction to proceed.
                if self.question_queue.iter().any(|r| r.id == *request_id) {
                    tracing::warn!(
                        request_id = %request_id,
                        "Duplicate QuestionRequested ignored"
                    );
                } else {
                    tracing::info!(
                        session_id = %session_id,
                        request_id = %request_id,
                        "TUI received QuestionRequested, showing dialog"
                    );
                    self.question_queue.push_back(QuestionRequest {
                        id: request_id.clone(),
                        session_id: session_id.clone(),
                        question: question.clone(),
                        options: options.clone(),
                    });
                    self.pending_question_input.clear();
                    self.question_selected_index = 0;
                    self.status = "awaiting question".to_string();
                    self.needs_redraw = true;
                    self.push_log_no_agent(
                        LogLevel::Warn,
                        format!("question requested: {}", question),
                    );
                }
            }
            Event::PermissionReplied {
                ref session_id,
                ref request_id,
                allowed,
                ..
            } if self.is_current_session(session_id) => {
                // Remove the specific answered request from the queue.
                self.permission_queue.retain(|r| r.id != *request_id);
                self.pending_question_input.clear();
                self.question_selected_index = 0;
                if self.permission_queue.is_empty() {
                    self.set_status_working("processing");
                }
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("permission {}", if allowed { "granted" } else { "denied" }),
                );
            }
            Event::QuestionAnswered {
                ref session_id,
                ref request_id,
                ..
            } if self.is_current_session(session_id) => {
                self.question_queue.retain(|r| r.id != *request_id);
                self.pending_question_input.clear();
                self.question_selected_index = 0;
                if self.question_queue.is_empty() {
                    self.set_status_working("processing");
                }
            }
            Event::AgentSwitched {
                ref session_id,
                ref from,
                ref to,
            } if self.is_current_session(session_id) => {
                self.agent_name = to.clone();
                // Update the display name mapping for the current session
                if let Some(ref sid) = self.session_id {
                    let short_sid = short_session_id(sid);
                    self.sid_to_display_name.insert(short_sid, to.clone());
                }
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("agent switched: {} → {}", from, to),
                );
            }
            Event::AgentSwitchRequested {
                ref session_id,
                ref to,
                ref task,
                ref context,
            } if self.is_current_session(session_id) => {
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("agent switch requested → {} ({})", to, task),
                );
                self.pending_plan_task = Some((task.clone(), context.clone()));
            }
            Event::AgentRestoreRequested {
                ref session_id,
                ref summary,
            } if self.is_current_session(session_id) => {
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("agent restore requested ({} chars)", summary.len()),
                );
                // Show plan approval dialog instead of immediately restoring.
                // The user presses Approve/Reject (Enter/r) to proceed.
                self.plan_approval_pending = Some(PlanApprovalState {
                    plan_text: summary.clone(),
                    cursor_approve: true,
                });
            }
            Event::TaskCompleted {
                ref session_id,
                ref summary,
            } if self.is_current_session(session_id) => {
                self.push_log_no_agent(LogLevel::Info, "agent_complete signalled".to_string());
                self.last_task_completed_at = Some(std::time::Instant::now());
                // Exit autopilot mode on task completion
                if self.autopilot_enabled {
                    self.autopilot_enabled = false;
                    self.autopilot_started_at = None;
                    self.autopilot_pending_continue = None;
                    self.status = "task complete".to_string();
                    self.push_log_no_agent(
                        LogLevel::Info,
                        "autopilot stopped: task complete".to_string(),
                    );
                }
                // Render the summary as markdown so headers, bullet points,
                // and bold text create visual structure instead of a wall of
                // plain text.  Normal LLM streaming text bypasses markdown
                // rendering (it arrives in fragments), but the agent_complete
                // summary is a complete, self-contained message.
                let rendered = self.render_markdown_unconditionally(&format!(
                    "[ok] **Task Complete**\n\n{summary}"
                ));
                self.force_new_message = true;
                self.append_assistant_text(&rendered);
            }
            Event::AgentNotice {
                ref session_id,
                ref message,
            } if self.is_current_session(session_id) => {
                // Research progress events are sentinel-prefixed; route them
                // to the dedicated progress log list instead of the generic
                // agent-notice chat bubble.
                if let Some(decoded) = crate::research_progress::decode_progress_event(message) {
                    let level = if decoded.status == crate::research_progress::StepStatus::Error {
                        LogLevel::Warn
                    } else {
                        LogLevel::Info
                    };
                    self.push_log_no_agent(
                        level,
                        format!(
                            "research: {} — {}",
                            decoded.phase.as_str(),
                            crate::app::helpers::sanitize_for_display(&decoded.detail)
                        ),
                    ); // Keep the status bar in sync with the running phase.
                    // The `[wait]` prefix marks this as async-in-progress so
                    // [`App::arm_status_expiry`] will not auto-clear it to
                    // "ready" while the background research is still running.
                    // Errors outside the web phase surface as a warning
                    // status instead.
                    if decoded.status == crate::research_progress::StepStatus::Error
                        && decoded.phase != crate::research_progress::SessionPhase::Web
                    {
                        self.status = format!("[warn] research: {}", decoded.detail);
                    } else if decoded.total_sources.is_none() {
                        self.status = format!(
                            "[wait] research: {} — {} ({}) — running",
                            decoded.name,
                            decoded.phase.as_str(),
                            decoded.status.icon(),
                        );
                    }
                    let progress = if let Some(existing) = self
                        .research_progress
                        .iter_mut()
                        .find(|p| p.name == decoded.name)
                    {
                        existing
                    } else {
                        // First event for this run arrived before the
                        // `/research create` handler seeded the tracker (or
                        // the run was started through a non-TUI path). Create
                        // it on demand so progress is never lost.
                        self.research_progress.push(
                            crate::research_progress::ResearchProgress::new(
                                decoded.name.clone(),
                                decoded.topic.clone(),
                            ),
                        );
                        let Some(last) = self.research_progress.last_mut() else {
                            tracing::error!("research progress list empty immediately after push");
                            return;
                        };
                        last
                    };
                    progress.apply_with_capture(
                        decoded.phase,
                        decoded.status,
                        decoded.detail.clone(),
                        decoded.capture.clone(),
                    );
                    if let Some(total) = decoded.total_sources {
                        progress.finish(
                            total,
                            decoded.pdf_count,
                            decoded.youtube_count,
                            decoded.excluded_count,
                        ); // The final progress event marks the run complete.
                        // Drop the `[wait]` in-progress status for a terminal
                        // message and arm the auto-expiry timer so it
                        // transitions to "ready" after the grace period.
                        let mut status =
                            format!("research: {} complete — {total} sources", decoded.name);
                        if decoded.pdf_count > 0 {
                            status.push_str(&format!(
                                ", {} PDF{}",
                                decoded.pdf_count,
                                if decoded.pdf_count == 1 { "" } else { "s" }
                            ));
                        }
                        if decoded.youtube_count > 0 {
                            status.push_str(&format!(
                                ", {} YouTube video{}",
                                decoded.youtube_count,
                                if decoded.youtube_count == 1 { "" } else { "s" }
                            ));
                        }
                        if decoded.excluded_count > 0 {
                            status.push_str(&format!(", {} excluded", decoded.excluded_count));
                        }
                        self.status = status;
                        self.arm_status_expiry();
                    } else if decoded.phase == crate::research_progress::SessionPhase::Finalize
                        && decoded.status == crate::research_progress::StepStatus::Done
                    {
                        // `/research cluster` does not carry a source count;
                        // its final progress event is a finalize/done step.
                        self.status = format!("research: {} extraction complete", decoded.name);
                        self.arm_status_expiry();
                    }
                    let name_for_refresh = decoded.name.clone();
                    self.refresh_research_progress_message(&name_for_refresh);
                    return;
                }
                self.push_log_no_agent(LogLevel::Info, format!("agent notice: {}", message));
                // Agent notices are displayed in the message window only.
                // They are intentionally not mirrored to the status bar so
                // multi-line summaries do not overflow or duplicate there.
                self.append_assistant_text(&format!("📋 Agent Notice\n{}", message));
            }
            Event::AgentError {
                ref session_id,
                ref error,
            } if self.is_current_session(session_id) => {
                self.is_processing = false;
                self.cancel_flag = None;
                self.agent_halted = false;
                if self.auto_compact_in_progress {
                    self.auto_compact_in_progress = false;
                    self.auto_compact_failed = true;
                    self.pending_send_after_compact = None;
                    self.push_log_no_agent(
                        LogLevel::Warn,
                        "Auto-compaction failed; send blocked for this turn".to_string(),
                    );
                }
                self.compact_in_progress = false;
                self.needs_redraw = true;
                // Full details go to the log panel only
                self.push_log_no_agent(LogLevel::Error, format!("agent error: {}", error)); // Clean summary for the status bar and chat panel
                let summary = summarise_error(error);
                self.status = format!("error: {}", summary);
                self.append_assistant_text(&format!("[warn] {}", summary));
            }
            Event::TokenUsage {
                ref session_id,
                input_tokens,
                output_tokens,
            } if self.is_current_session(session_id) => {
                self.last_input_tokens = input_tokens;
                self.token_usage.0 += input_tokens;
                self.token_usage.1 += output_tokens;
                telemetry_counters::increment_tokens_input(input_tokens);
                telemetry_counters::increment_tokens_output(output_tokens);
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!(
                        "tokens: +{}in +{}out (total {}in {}out)",
                        input_tokens, output_tokens, self.token_usage.0, self.token_usage.1
                    ),
                );
                // T-010/FR-013: the reported input tokens are the actual
                // context size sent to the model this turn ("Sent to model"
                // row), so refresh the Context panel snapshot immediately.
                self.schedule_context_snapshot_refresh();
                self.needs_redraw = true;
            }
            Event::RunCostSummary {
                ref session_id,
                ref model_id,
                input_tokens,
                output_tokens,
                total_cost_usd,
                duration_ms,
            } if self.is_current_session(session_id) => {
                telemetry_counters::set_cost_session_last(total_cost_usd);
                let duration_secs = (duration_ms as f64) / 1000.0;
                // Compact one-line banner shown transiently (dismissed on keypress).
                let banner = format!(
                    "⟡ run complete · {in}+{out} tokens · ${cost:.4} · {dur:.1}s",
                    in = input_tokens,
                    out = output_tokens,
                    cost = total_cost_usd,
                    dur = duration_secs,
                );
                self.run_cost_banner = Some(banner);
                self.run_cost_banner_at = Some(std::time::Instant::now());
                self.needs_redraw = true;
                // Full details always go to the log panel (model + ms precision).
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!(
                        "⟡ run complete · {in}in / {out}out tokens · model {model} · ${cost:.6} · {ms}ms ({dur:.2}s)",
                        in = input_tokens,
                        out = output_tokens,
                        model = model_id,
                        cost = total_cost_usd,
                        ms = duration_ms,
                        dur = duration_secs,
                    ),
                );
            }
            Event::HookWarning {
                ref session_id,
                ref hook_command,
                ref tool,
                ref stderr,
            } if self.is_current_session(session_id) => {
                // Show a short transient toast in the status bar.  We deliberately
                // omit the "[warn]" prefix so that `arm_status_expiry()` will
                // auto-clear the toast after the standard grace period.
                let short_reason = if stderr.len() > 80 {
                    let mut end = 80;
                    while end > 0 && !stderr.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}…", &stderr[..end])
                } else {
                    stderr.clone()
                };
                self.status = format!("hook warning: {} — {}", tool, short_reason);
                self.arm_status_expiry();
                self.push_log_no_agent(
                    LogLevel::Warn,
                    format!("hook warning on {} ({}): {}", tool, hook_command, stderr),
                );
            }
            Event::ToolResultFlagged {
                ref session_id,
                ref tool,
                ref hook_command,
                ref reason,
            } if self.is_current_session(session_id) => {
                self.status = format!("[warn] {} flagged", tool);
                self.push_log_no_agent(
                    LogLevel::Error,
                    format!(
                        "[flag] tool {} flagged by {}: {}",
                        tool, hook_command, reason
                    ),
                );
            }
            Event::QuotaUpdate {
                ref session_id,
                percent,
            } if self.is_current_session(session_id) => {
                self.quota_percent = Some(percent);
                self.push_log_no_agent(LogLevel::Info, format!("quota: {:.1}% used", percent));
            }
            Event::ToolsSent {
                ref session_id,
                ref tools,
            } if self.is_current_session(session_id)
                && self.event_bus.current_step(session_id) <= 1 =>
            {
                // Only log the list of tools during system initialisation (first step).
                // The SessionProcessor increments the EventBus step at the start of
                // each loop iteration; the first LLM request corresponds to step 1.
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("tools sent: [{}]", tools.join(", ")),
                );
            }
            Event::ModelResponse {
                ref session_id,
                ref text,
                elapsed_ms,
                input_tokens,
                output_tokens,
            } if self.is_current_session(session_id) => {
                telemetry_counters::set_llm_duration_last(elapsed_ms as f64);
                if let Some(model_ref) = self.active_model_ref_string() {
                    self.llm_request_stats.push(LlmRequestStat {
                        model_ref,
                        elapsed_ms,
                        input_tokens,
                        output_tokens,
                    });
                    // R-22: Cap the stats vec to a rolling window so it does
                    // not grow without bound over a long session.
                    const MAX_LLM_REQUEST_STATS: usize = 1000;
                    if self.llm_request_stats.len() > MAX_LLM_REQUEST_STATS {
                        let drop_count = self.llm_request_stats.len() - MAX_LLM_REQUEST_STATS;
                        self.llm_request_stats.drain(0..drop_count);
                    }
                }
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("model response ({elapsed_ms}ms): {text}"),
                );
            }
            Event::ToolCallArgs {
                ref session_id,
                ref call_id,
                ref tool,
                ref args,
            } if self.is_current_session(session_id) => {
                self.stream_in_bytes += (call_id.len() + tool.len() + args.len()) as u64;
                // Try to apply args to an existing ToolCall part; if not found,
                // store them pending until the ToolCallStart event arrives.
                let applied = self.update_tool_call_input(call_id, args);
                if !applied {
                    self.pending_tool_args.insert(call_id.clone(), args.clone());
                }
                let step_tag = self
                    .tool_step_map
                    .get(call_id)
                    .map(|(_sid, step, substep)| format!("[{step}.{substep}] "))
                    .unwrap_or_default();
                // Pretty-print JSON args as a single truncated log entry
                // (H-009) rather than one entry per line, which both avoided
                // dozens of `format!` allocations for a large payload and
                // (previously) bumped the global log version per line. Compact
                // serialization is used because newlines render poorly in the
                // single-line log panel and only the first 200 chars survive
                // the preview cap anyway.
                let pretty = serde_json::from_str::<serde_json::Value>(args)
                    .ok()
                    .and_then(|v| serde_json::to_string(&v).ok())
                    .unwrap_or_else(|| args.to_string());
                // Cap at ~200 chars so a single tool call cannot flood the log.
                let preview = crate::app::helpers::truncate_chars(pretty.trim(), 200);
                self.push_log_no_agent(
                    LogLevel::Tool,
                    format!("{}→ {}({})", step_tag, tool, preview),
                );
                self.needs_redraw = true;
            }
            Event::ToolResult {
                ref session_id,
                ref call_id,
                ref tool,
                ref content,
                content_line_count,
                ref metadata,
                success,
                ..
            } if self.is_current_session(session_id) => {
                self.update_tool_call_output(call_id, content_line_count, metadata.as_ref());
                if *tool == "team_create"
                    && success
                    && let Some(meta) = metadata
                    && let Some(team_name) = meta.get("team_name").and_then(|v| v.as_str())
                {
                    let working_dir = std::env::current_dir().unwrap_or_default();
                    if let Ok(store) = TeamStore::load_by_name(team_name, &working_dir) {
                        let name = store.config.name.clone();
                        let team_dir = store.dir.clone();
                        self.team_members = store.config.members.clone();
                        self.active_team = Some(store.config);
                        self.show_teams = true;
                        // Reconcile is needed here: team was created via LLM tool path,
                        // so the TeamManager didn't exist during blueprint seeding and
                        // members may have been queued in Spawning state.
                        self.ensure_team_manager_for_team_inner(&name, Some(team_dir), true);
                    }
                }
                let step_tag = self
                    .tool_step_map
                    .get(call_id)
                    .map(|(_sid, step, substep)| format!("[{step}.{substep}] "))
                    .unwrap_or_default();
                let icon = if success { "✓" } else { "✗" };
                let display_content = truncate_str(content, 2516);
                let log_line = if display_content.is_empty() {
                    format!("{}← {} {}", step_tag, tool, icon)
                } else {
                    format!("{}← {} {} {}", step_tag, tool, icon, display_content)
                };
                self.push_log_no_agent(LogLevel::Tool, log_line);
                // Mark the Tasks side-panel cache stale when task data
                // mutates, so the next panel render reflects the change.
                if matches!(tool.as_str(), "task_create" | "task_update") {
                    self.tasks_cache_dirty = true;
                }
                // T-010/FR-013: the tool result changed the conversation
                // history/tool-call payload, so refresh the Context panel.
                self.schedule_context_snapshot_refresh();
                self.needs_redraw = true;
            }
            Event::SubagentStart {
                ref session_id,
                ref task_id,
                ref child_session_id,
                ref agent,
                ref task,
                background,
                ..
            } if self.is_current_or_descendant_session(session_id) => {
                telemetry_counters::increment_subagent_spawns(1);
                telemetry_counters::add_agents_active(1);
                // Map the child session's short_sid to the task id for display.
                // The task id already encodes the agent type plus a unique suffix
                // (e.g. "explore-a1b2c3d4") so tools and panels can distinguish
                // multiple subagents of the same type.
                let short_sid = short_session_id(child_session_id);
                self.sid_to_display_name.insert(short_sid, task_id.clone());

                // Add to active_tasks so the agent panel shows it immediately.
                let entry = ragent_agent::task::TaskEntry {
                    id: task_id.clone(),
                    parent_session_id: session_id.clone(),
                    child_session_id: child_session_id.clone(),
                    agent_name: agent.clone(),
                    task_prompt: task.clone(),
                    background,
                    status: ragent_agent::task::TaskStatus::Running,
                    result: None,
                    error: None,
                    created_at: chrono::Utc::now(),
                    completed_at: None,
                    reported: false,
                    waiter_count: 0,
                    output_file: None,
                    report_status: ragent_agent::task::ReportStatus::default(),
                };
                self.active_tasks.push(entry);
                let (icon, kind) = if background {
                    ("[bg]", "Background")
                } else {
                    ("[fg]", "Foreground")
                };
                self.push_log_for(
                    LogLevel::Info,
                    format!("{} {} task started: {} ({})", icon, kind, task_id, agent),
                    None,
                    Some(child_session_id.clone()),
                );
            }
            Event::SubagentComplete {
                ref session_id,
                ref task_id,
                ref child_session_id,
                ref summary,
                ref finish_reason,
                success,
                ..
            } if self.is_current_or_descendant_session(session_id) => {
                telemetry_counters::add_agents_active(-1);
                telemetry_counters::increment_agents_completed(1);
                if let Some(idx) = self.active_tasks.iter().position(|t| t.id == *task_id) {
                    if let Some(t) = self.active_tasks.get_mut(idx) {
                        // Propagate the finish signature so the Agents popup
                        // shows truncation / continuation on the completed
                        // row instead of leaving it at the spawn default.
                        t.report_status = match finish_reason.as_str() {
                            "continued" => ragent_agent::task::ReportStatus::Continued,
                            "truncated" => ragent_agent::task::ReportStatus::Truncated,
                            _ => ragent_agent::task::ReportStatus::Complete,
                        };
                    }
                    self.active_tasks.remove(idx);
                }
                let icon = if success { "[ok]" } else { "[err]" };
                let suffix = match finish_reason.as_str() {
                    "continued" => " (truncated; continuation retry recovered the tail)",
                    "truncated" => " (TRUNCATED by provider; report is incomplete)",
                    _ => "",
                };
                self.push_log_for(
                    LogLevel::Info,
                    format!(
                        "{} Task completed{} ({}): {}",
                        icon,
                        suffix,
                        &task_id[..8.min(task_id.len())],
                        summary
                    ),
                    None,
                    Some(child_session_id.clone()),
                );
            }
            Event::SubagentCancelled {
                ref session_id,
                ref task_id,
            } if self.is_current_or_descendant_session(session_id) => {
                if let Some(idx) = self.active_tasks.iter().position(|t| t.id == *task_id) {
                    let child_session_id = self.active_tasks[idx].child_session_id.clone();
                    self.active_tasks.remove(idx);
                    self.push_log_for(
                        LogLevel::Info,
                        format!(
                            "[cancel] Task cancelled ({})",
                            &task_id[..8.min(task_id.len())]
                        ),
                        None,
                        Some(child_session_id),
                    );
                } else {
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!(
                            "[cancel] Task cancelled ({})",
                            &task_id[..8.min(task_id.len())]
                        ),
                    );
                }
            }
            Event::SubagentSuspended {
                ref session_id,
                ref task_id,
                ref child_session_id,
            } if self.is_current_or_descendant_session(session_id) => {
                if let Some(task) = self.active_tasks.iter_mut().find(|t| t.id == *task_id) {
                    task.status = ragent_agent::task::TaskStatus::Suspended;
                }
                self.push_log_for(
                    LogLevel::Info,
                    format!(
                        "[pause] Task suspended ({})",
                        &task_id[..8.min(task_id.len())]
                    ),
                    None,
                    Some(child_session_id.clone()),
                );
            }
            Event::SubagentResumed {
                ref session_id,
                ref task_id,
                ref child_session_id,
            } if self.is_current_or_descendant_session(session_id) => {
                if let Some(task) = self.active_tasks.iter_mut().find(|t| t.id == *task_id) {
                    task.status = ragent_agent::task::TaskStatus::Running;
                }
                self.push_log_for(
                    LogLevel::Info,
                    format!("▷ Task resumed ({})", &task_id[..8.min(task_id.len())]),
                    None,
                    Some(child_session_id.clone()),
                );
            }
            Event::SubagentKilled {
                ref session_id,
                ref task_id,
                ref child_session_id,
                force,
            } if self.is_current_or_descendant_session(session_id) => {
                if let Some(idx) = self.active_tasks.iter().position(|t| t.id == *task_id) {
                    self.active_tasks.remove(idx);
                }
                let label = if force { "Force-killed" } else { "Killed" };
                self.push_log_for(
                    LogLevel::Info,
                    format!(
                        "[kill] {} task ({})",
                        label,
                        &task_id[..8.min(task_id.len())]
                    ),
                    None,
                    Some(child_session_id.clone()),
                );
            }
            Event::BackgroundTaskSpawned {
                ref session_id,
                ref task_id,
                ref command,
            } if self.is_current_session(session_id) => {
                self.bg_tasks.push(BgTaskView {
                    id: task_id.clone(),
                    session_id: session_id.clone(),
                    command: command.clone(),
                    status: "running".to_string(),
                    created_at: chrono::Utc::now(),
                    completed_at: None,
                });
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!(
                        "[bg]  Background task started: {} ({})",
                        &task_id[..8.min(task_id.len())],
                        command
                    ),
                );
            }
            Event::BackgroundTaskUpdated {
                ref session_id,
                ref task_id,
                ref status,
                ..
            } if self.is_current_session(session_id) => {
                if let Some(task) = self.bg_tasks.iter_mut().find(|t| t.id == *task_id) {
                    task.status = status.clone();
                    if matches!(status.as_str(), "completed" | "failed" | "cancelled") {
                        task.completed_at = Some(chrono::Utc::now());
                    }
                }
            }
            Event::BackgroundTaskCompleted {
                ref session_id,
                ref task_id,
                ref status,
                ..
            } if self.is_current_session(session_id) => {
                if let Some(task) = self.bg_tasks.iter_mut().find(|t| t.id == *task_id) {
                    task.status = status.clone();
                    task.completed_at = Some(chrono::Utc::now());
                }
                let icon = if status == "completed" {
                    "[ok]"
                } else {
                    "[err]"
                };
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!(
                        "{} Background task completed ({}): {}",
                        icon,
                        &task_id[..8.min(task_id.len())],
                        status
                    ),
                );
                // Remove the background task from the Agents panel once it
                // reaches a terminal state.
                self.bg_tasks.retain(|t| t.id != *task_id);
            }
            Event::TeammateSpawned {
                ref session_id,
                ref team_name,
                ref teammate_name,
                ref agent_id,
            } if self.is_current_session(session_id) => {
                telemetry_counters::set_team_members(self.team_members.len() as i64 + 1);
                // Add new member to team_members if not already present.
                if !self.team_members.iter().any(|m| m.agent_id == *agent_id) {
                    let member =
                        TeamMember::new(teammate_name.as_str(), agent_id.as_str(), "teammate");
                    self.team_members.push(member);
                    self.team_message_counts
                        .entry(agent_id.clone())
                        .or_insert((0, 0));
                }
                // Always refresh the stored values (session id, status, current task)
                // from disk so races between UI hydration and spawn events don't
                // leave the UI showing an outdated state.
                let teammate_session_id = if let Some(m) = self
                    .team_members
                    .iter_mut()
                    .find(|m| m.agent_id == *agent_id)
                {
                    let working_dir = std::env::current_dir().unwrap_or_default();
                    if let Ok(store) = TeamStore::load_by_name(team_name, &working_dir)
                        && let Some(stored) = store
                            .config
                            .members
                            .iter()
                            .find(|x| x.agent_id == *agent_id)
                    {
                        m.session_id = stored.session_id.clone();
                        m.status = stored.status.clone();
                        m.current_task_id = stored.current_task_id.clone();
                        // Map this teammate's session short_sid → a unique display
                        // label (name + agent id) so tool step tags and panels can
                        // distinguish teammates with the same name.
                        if let Some(ref sid) = stored.session_id {
                            let short_sid = short_session_id(sid);
                            let display_name = format!("{}-{}", teammate_name, agent_id);
                            self.sid_to_display_name.insert(short_sid, display_name);
                        }
                    }
                    m.session_id.clone()
                } else {
                    None
                };
                self.show_teams = true;
                self.push_log_for(
                    LogLevel::Info,
                    format!("[team] [{team_name}] Spawned teammate '{teammate_name}' ({agent_id})"),
                    None,
                    teammate_session_id,
                );
            }
            Event::TeammateMessage {
                ref session_id,
                ref team_name,
                ref from,
                ref to,
                ref message_type,
                ref preview,
            } if self.is_current_session(session_id) => {
                if from.as_str() != "lead" {
                    let counts = self
                        .team_message_counts
                        .entry(from.clone())
                        .or_insert((0, 0));
                    counts.0 = counts.0.saturating_add(1);
                }
                if to.as_str() != "lead" {
                    let counts = self.team_message_counts.entry(to.clone()).or_insert((0, 0));
                    counts.1 = counts.1.saturating_add(1);
                }
                self.push_log_for(
                    LogLevel::Info,
                    format!("📨 [{team_name}] {from} → {to} ({message_type}): {preview}"),
                    None,
                    self.team_member_session_id_by_agent_id(from),
                );
            }
            Event::TeammateP2PMessage {
                ref session_id,
                ref team_name,
                ref from,
                ref to,
                ref message_type,
                ref preview,
            } if self.is_current_session(session_id) => {
                // Track sent count for sender.
                let from_counts = self
                    .team_message_counts
                    .entry(from.clone())
                    .or_insert((0, 0));
                from_counts.0 = from_counts.0.saturating_add(1);
                // Track received count for recipient.
                let to_counts = self.team_message_counts.entry(to.clone()).or_insert((0, 0));
                to_counts.1 = to_counts.1.saturating_add(1);
                self.push_log_for(
                    LogLevel::Info,
                    format!("🔀 [{team_name}] P2P {from} → {to} ({message_type}): {preview}"),
                    None,
                    self.team_member_session_id_by_agent_id(from),
                );
            }
            Event::TeammateIdle {
                ref session_id,
                ref team_name,
                ref agent_id,
            } if self.is_current_session(session_id) => {
                if let Some(m) = self
                    .team_members
                    .iter_mut()
                    .find(|m| m.agent_id == *agent_id)
                {
                    m.status = MemberStatus::Idle;
                }
                // M6-T1: record progress so the watchdog does not flag this
                // teammate as hung.
                if let Some(tm) = self.session_processor.team_manager.get() {
                    tm.record_progress(agent_id);
                }
                self.push_log_for(
                    LogLevel::Info,
                    format!("💤 [{team_name}] Teammate {agent_id} is idle"),
                    None,
                    self.team_member_session_id_by_agent_id(agent_id),
                );
            }
            Event::TeammateFailed {
                ref session_id,
                ref team_name,
                ref agent_id,
                ref error,
            } if self.is_current_session(session_id) => {
                if let Some(m) = self
                    .team_members
                    .iter_mut()
                    .find(|m| m.agent_id == *agent_id)
                {
                    m.status = MemberStatus::Failed;
                    m.last_spawn_error = Some(error.clone());
                }
                // M6-T1: record progress so the watchdog stops tracking
                // this agent.
                if let Some(tm) = self.session_processor.team_manager.get() {
                    tm.record_progress(agent_id);
                }
                let short_err = if error.len() > 200 {
                    let mut end = 200;
                    while end > 0 && !error.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}…", &error[..end])
                } else {
                    error.to_string()
                };
                self.push_log_for(
                    LogLevel::Error,
                    format!("[err] [{team_name}] Teammate {agent_id} failed: {short_err}"),
                    None,
                    self.team_member_session_id_by_agent_id(agent_id),
                );
            }
            Event::TeamTaskClaimed {
                ref session_id,
                ref team_name,
                ref agent_id,
                ref task_id,
            } if self.is_current_session(session_id) => {
                if let Some(m) = self
                    .team_members
                    .iter_mut()
                    .find(|m| m.agent_id == *agent_id)
                {
                    m.status = MemberStatus::Working;
                    m.current_task_id = Some(task_id.clone());
                }
                // M6-T1: record progress.
                if let Some(tm) = self.session_processor.team_manager.get() {
                    tm.record_progress(agent_id);
                }
                self.push_log_for(
                    LogLevel::Info,
                    format!("📋 [{team_name}] {agent_id} claimed task {task_id}"),
                    None,
                    self.team_member_session_id_by_agent_id(agent_id),
                );
            }
            Event::TeamTaskCompleted {
                ref session_id,
                ref team_name,
                ref agent_id,
                ref task_id,
            } if self.is_current_session(session_id) => {
                if let Some(m) = self
                    .team_members
                    .iter_mut()
                    .find(|m| m.agent_id == *agent_id)
                {
                    m.current_task_id = None;
                }
                // M6-T1: record progress.
                if let Some(tm) = self.session_processor.team_manager.get() {
                    tm.record_progress(agent_id);
                }
                self.push_log_for(
                    LogLevel::Info,
                    format!("[ok] [{team_name}] {agent_id} completed task {task_id}"),
                    None,
                    self.team_member_session_id_by_agent_id(agent_id),
                );
            }
            Event::TeamCleanedUp {
                ref session_id,
                ref team_name,
            } if self.is_current_session(session_id) => {
                self.active_team = None;
                self.team_members.clear();
                self.team_message_counts.clear();
                self.show_teams = false;
                self.focused_teammate = None;
                if self
                    .swarm_state
                    .as_ref()
                    .is_some_and(|s| &s.team_name == team_name)
                {
                    self.swarm_state = None;
                }
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("[clr] Team '{team_name}' cleaned up"),
                );
            }
            Event::ShellCwdChanged {
                ref session_id,
                ref cwd,
            } if self.is_current_session(session_id) => {
                self.shell_cwd = Some(cwd.clone());
            }
            Event::UserInput { ref session_id, .. } if self.is_current_session(session_id) => {
                self.set_status_working("processing");
            }
            // ── Structured-memory cache invalidation ───────────────────────
            // Memory tools modify the SQLite store directly, so the TUI panel
            // cache must be marked stale when these events arrive.  The next
            // `render_memory_panel` call will then refresh from disk.
            Event::MemoryStored { ref session_id, .. }
            | Event::MemoryRecalled { ref session_id, .. }
            | Event::MemoryForgotten { ref session_id, .. }
            | Event::MemorySearched { ref session_id, .. }
                if self.is_current_session(session_id) =>
            {
                self.memory_cache_dirty = true;
            }
            // ── Provider model-list loading (spinner popup) ──────────────────
            Event::ProviderLoadingStarted {
                ref provider_id,
                ref provider_name,
            } => {
                self.model_loading_state = Some(ModelLoadingState {
                    provider_id: provider_id.clone(),
                    provider_name: provider_name.clone(),
                    started_at: std::time::Instant::now(),
                });
            }
            Event::ProviderLoadingFinished {
                ref provider_id,
                ref provider_name,
                ref models,
                ref error,
            } => {
                self.model_loading_state = None;
                let parsed_models: Vec<ModelInfo> = models
                    .iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect();

                // Only persist successful non-empty discovery results; an empty
                // result from a failed discovery should not wipe a previously
                // useful cache or prevent fallback to static defaults.
                if !parsed_models.is_empty() {
                    self.cache_discovered_models(provider_id, &parsed_models);
                }

                if let Some(err) = error {
                    self.push_log_no_agent(
                        LogLevel::Error,
                        format!("{} model discovery failed: {}", provider_name, err),
                    );
                    self.status = format!("{}: discovery failed", provider_name);
                } else {
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!(
                            "{} model discovery finished ({} models)",
                            provider_name,
                            parsed_models.len()
                        ),
                    );
                }

                // Advance the provider-setup dialog if it is waiting on this provider.
                if let Some(ProviderSetupStep::LoadingModels {
                    provider_id: ref pid,
                    ..
                }) = self.provider_setup
                {
                    if pid == provider_id {
                        let entries = if parsed_models.is_empty() {
                            // Fall back to static defaults so the user still sees
                            // models even when dynamic discovery fails.
                            self.picker_entries_from_models(
                                self.provider_registry
                                    .get(provider_id)
                                    .map(|p| p.default_models())
                                    .unwrap_or_default(),
                            )
                        } else {
                            self.picker_entries_from_models(parsed_models)
                        };
                        self.provider_setup = Some(ProviderSetupStep::SelectModel {
                            provider_id: provider_id.clone(),
                            provider_name: provider_name.clone(),
                            models: entries,
                            selected: 0,
                        });
                    }
                }
            }
            Event::RouterClassification {
                ref session_id,
                ref tier,
                ref requested_tier,
                ref model,
                composite_score,
                ref prompt,
                ref dimensions,
            } if self.is_current_session(session_id) => {
                self.router_current_tier = Some(tier.clone());
                self.router_current_model = Some(model.clone());
                let dims = dimensions
                    .iter()
                    .map(|(name, score)| format!("{}={:.2}", name, score))
                    .collect::<Vec<_>>()
                    .join(", ");
                let prompt_display = if prompt.chars().count() > 80 {
                    let mut end = 80;
                    while end > 0 && !prompt.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}…", &prompt[..end])
                } else {
                    prompt.clone()
                };
                let fallback_note = requested_tier
                    .as_ref()
                    .filter(|rt| rt.as_str() != tier.as_str())
                    .map(|rt| format!(" (requested {})", rt))
                    .unwrap_or_default();
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!(
                        "Router: bucket={} model={} composite={:.4} prompt=\"{}\" dimensions=[{}]{}",
                        tier, model, composite_score, prompt_display, dims, fallback_note
                    ),
                );
            } // ── Model download progress (progress bar popup) ───────────────
            Event::ModelDownloadStarted {
                ref provider_id,
                ref model_id,
                ..
            } => {
                let _provider_name = self
                    .provider_registry
                    .get(provider_id)
                    .map(|p| p.name().to_string())
                    .unwrap_or_else(|| provider_id.clone());
                self.model_download_state = Some(ModelDownloadState {
                    provider_id: provider_id.clone(),
                    model_id: model_id.clone(),
                    percent: 0.0,
                    started_at: std::time::Instant::now(),
                });
            }
            Event::ModelDownloadProgress {
                ref provider_id,
                ref model_id,
                percent,
                ..
            } => {
                if let Some(ref mut state) = self.model_download_state {
                    if state.provider_id == *provider_id && state.model_id == *model_id {
                        state.percent = percent;
                        // Progress proves the latch is alive: restart the
                        // staleness clock so slow-but-moving downloads are
                        // not reaped by the watchdog purely on start time.
                        state.started_at = std::time::Instant::now();
                    }
                }
            }
            Event::ModelDownloadFinished {
                ref provider_id,
                ref model_id,
                ref session_id,
                ref error,
            } => {
                self.model_download_state = None;
                if let Some(err) = error {
                    let display_name = self
                        .provider_registry
                        .get(provider_id)
                        .map(|p| p.name().to_string())
                        .unwrap_or_else(|| provider_id.clone());
                    self.push_log_no_agent(
                        LogLevel::Error,
                        format!("Download failed for {}/{}: {}", provider_id, model_id, err),
                    );
                    if self.is_current_session(session_id) {
                        self.status = format!("download failed: {}", model_id);
                        self.append_assistant_text(&format!(
                                        "[warn] **Model download failed**\n\nProvider: `{}`\nModel: `{}`\nError: {}",
                                        display_name, model_id, err
                                    ));
                    }
                } else {
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("Download finished for {}/{}", provider_id, model_id),
                    );
                }
            } // ── Service start errors (local runtime failed to start) ──
            Event::ServiceStartError {
                ref session_id,
                ref service,
                ref command_path,
                ref stdout,
                ref stderr,
                ref error,
            } => {
                let summary = format!(
                    "[warn] **{} failed to start**\n\nCommand: `{}`\nError: {}\n\nstdout:\n```\n{}\n```\n\nstderr:\n```\n{}\n```",
                    service, command_path, error, stdout, stderr
                );
                self.push_log_no_agent(LogLevel::Error, summary.clone());
                if self.is_current_session(session_id) {
                    self.append_assistant_text(&summary);
                    self.status = format!("{} failed to start", service);
                }
            }
            _ => {}
        }

        // Handle device flow completion outside the match to avoid
        // borrow issues (we need &mut self for storage + UI updates).
        if let Event::CopilotDeviceFlowComplete {
            ref token,
            ref api_base,
        } = event
        {
            let _ = self.storage.set_provider_auth("copilot", token);

            let _ = self.storage.set_setting("copilot_api_base", api_base);
            let _ = self.storage.delete_setting("provider_copilot_disabled");
            self.push_log_no_agent(
                LogLevel::Info,
                format!("Copilot authorised (api: {api_base})"),
            );
            self.refresh_provider();
            let models = self.models_for_provider("copilot");
            self.provider_setup = Some(ProviderSetupStep::SelectModel {
                provider_id: "copilot".to_string(),
                provider_name: "GitHub Copilot".to_string(),
                models,
                selected: 0,
            });
        }

        // ── GitHub device-flow complete ──────────────────────────────────
        if let Event::GithubDeviceFlowComplete { success, ref error } = event {
            self.provider_setup = None;
            if success {
                self.push_log_no_agent(
                    LogLevel::Info,
                    "GitHub authentication successful".to_string(),
                );
                self.append_assistant_text(
                    "From: /github login\n[ok] GitHub authentication successful! Token saved to ~/.ragent/github_token.",
                );
                self.status = "GitHub authenticated".to_string();
            } else {
                let msg = error
                    .clone()
                    .unwrap_or_else(|| "GitHub login failed.".to_string());
                self.push_log_no_agent(LogLevel::Warn, format!("GitHub login failed: {msg}"));
                self.append_assistant_text(&format!("From: /github login\n[err] {msg}"));
                self.status = "GitHub login failed".to_string();
            }
        }

        // ── GitLab setup complete ────────────────────────────────────────
        if let Event::GitLabSetupComplete { success, ref error } = event {
            if success {
                self.provider_setup = None;
                self.push_log_no_agent(
                    LogLevel::Info,
                    "GitLab configured successfully".to_string(),
                );
            } else {
                // Revert to form with error
                let (url, tok) = if let Some(ProviderSetupStep::GitLabValidating {
                    ref instance_url,
                    ref token,
                }) = self.provider_setup
                {
                    (instance_url.clone(), token.clone())
                } else {
                    (String::new(), String::new())
                };
                self.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input: url,
                    url_cursor: 0,
                    token_input: tok,
                    token_cursor: 0,
                    active_field: 0,
                    error: error
                        .clone()
                        .or_else(|| Some("Validation failed.".to_string())),
                });
            }
        }
    }

    pub(crate) fn cancel_bg_task(&mut self, task_id: &str) {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let bg = self.session_processor.bg_service.get().cloned();
            let id = task_id.to_string();
            handle.spawn(async move {
                if let Some(bg) = bg {
                    let _ = bg.cancel(&id).await;
                }
            });
        }
    }

    pub(crate) fn suspend_agent_task(&mut self, task_id: &str) {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let tm = self.session_processor.agent_manager.get().cloned();
            let id = task_id.to_string();
            handle.spawn(async move {
                if let Some(tm) = tm {
                    let _ = tm.suspend_task(&id).await;
                }
            });
        }
    }

    pub(crate) fn resume_agent_task(&mut self, task_id: &str) {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let tm = self.session_processor.agent_manager.get().cloned();
            let id = task_id.to_string();
            handle.spawn(async move {
                if let Some(tm) = tm {
                    let _ = tm.resume_task(&id).await;
                }
            });
        }
    }

    pub(crate) fn kill_agent_task(&mut self, task_id: &str) {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let tm = self.session_processor.agent_manager.get().cloned();
            let id = task_id.to_string();
            handle.spawn(async move {
                if let Some(tm) = tm {
                    let _ = tm.kill_task(&id).await;
                }
            });
        }
    }

    pub(crate) fn open_output_view_team_member(
        &mut self,
        team_name: String,
        agent_id: String,
        teammate_name: String,
        session_id: Option<String>,
    ) {
        self.selected_agent_session_id = session_id.clone();
        self.output_view = Some(OutputViewState {
            target: OutputViewTarget::TeamMember {
                team_name,
                agent_id,
                teammate_name,
                session_id,
            },
            scroll_offset: 0,
            max_scroll: 0,
            line_cache: OutputViewLineCache {
                lines: Vec::new(),
                wrapped_lines: Vec::new(),
                content_lines: Vec::new(),
                wrapped_count: 0,
                cache_width: 0,
                source_generation: 0,
            },
        });
    }

    pub(crate) fn scroll_output_view_by(&mut self, delta: i16) {
        if let Some(ref mut view) = self.output_view {
            view.scroll_by(delta);
        }
    }

    pub(crate) fn jump_output_view_start(&mut self) {
        if let Some(ref mut view) = self.output_view {
            view.jump_start();
        }
    }

    pub(crate) fn jump_output_view_end(&mut self) {
        if let Some(ref mut view) = self.output_view {
            view.jump_end();
        }
    }

    pub(crate) fn jump_research_view_start(&mut self) {
        if let Some(ref mut view) = self.research_view {
            view.jump_start();
        }
    }

    pub(crate) fn jump_research_view_end(&mut self) {
        if let Some(ref mut view) = self.research_view {
            view.jump_end();
        }
    }

    pub(crate) fn scroll_research_view_by(&mut self, delta: i16) {
        if let Some(ref mut view) = self.research_view {
            view.scroll_by(delta);
        }
    }

    pub(crate) fn scroll_memory_view_by(&mut self, delta: i16) {
        if let Some(ref mut view) = self.memory_view {
            view.scroll_by(delta);
        }
    }

    pub(crate) fn jump_memory_view_start(&mut self) {
        if let Some(ref mut view) = self.memory_view {
            view.jump_start();
        }
    }

    pub(crate) fn jump_memory_view_end(&mut self) {
        if let Some(ref mut view) = self.memory_view {
            view.jump_end();
        }
    }

    pub(crate) fn open_research_view(
        &mut self,
        name: String,
        path: std::path::PathBuf,
        markdown: String,
    ) {
        let base_dir = path
            .parent()
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        self.research_view = Some(ResearchViewState {
            name,
            path,
            base_dir,
            markdown,
            scroll_offset: 0,
            max_scroll: 0,
            line_cache: crate::app::OutputViewLineCache {
                lines: Vec::new(),
                wrapped_lines: Vec::new(),
                content_lines: Vec::new(),
                wrapped_count: 0,
                cache_width: 0,
                source_generation: 0,
            },
        });
    }

    pub(crate) fn cycle_focused_teammate(&mut self, forward: bool) {
        if self.team_members.is_empty() {
            self.focused_teammate = None;
            return;
        }
        let ids: Vec<String> = self
            .team_members
            .iter()
            .map(|m| m.agent_id.clone())
            .collect();
        let current_idx = self
            .focused_teammate
            .as_ref()
            .and_then(|f| ids.iter().position(|id| id == f));
        let next = match (current_idx, forward) {
            (None, true) => Some(0),
            (None, false) => Some(ids.len() - 1),
            (Some(i), true) => {
                if i + 1 >= ids.len() {
                    None
                } else {
                    Some(i + 1)
                }
            }
            (Some(i), false) => {
                if i == 0 {
                    None
                } else {
                    Some(i - 1)
                }
            }
        };
        match next {
            Some(idx) => {
                let agent_id = ids[idx].clone();
                self.focus_teammate_by_id(&agent_id);
            }
            None => {
                self.focused_teammate = None;
                self.output_view = None;
                self.status = "team: focus cleared".to_string();
            }
        }
    }

    pub(crate) fn focus_teammate_by_id(&mut self, agent_id: &str) {
        let member = self
            .team_members
            .iter()
            .find(|m| m.agent_id == agent_id)
            .cloned();
        if let Some(m) = member {
            self.focused_teammate = Some(m.agent_id.clone());
            let team_name = self
                .active_team
                .as_ref()
                .map(|t| t.name.clone())
                .unwrap_or_default();
            self.open_output_view_team_member(
                team_name,
                m.agent_id.clone(),
                m.name.clone(),
                m.session_id.clone(),
            );
            self.status = format!("team: focused → {}", m.name);
        }
    }

    pub(crate) fn focus_teammate_by_name(&mut self, name: &str) -> Result<(), String> {
        let lower = name.to_lowercase();
        let matches: Vec<_> = self
            .team_members
            .iter()
            .filter(|m| {
                m.name.to_lowercase().contains(&lower) || m.agent_id.to_lowercase().contains(&lower)
            })
            .cloned()
            .collect();
        match matches.len() {
            0 => Err(format!("No teammate matching '{name}'")),
            1 => {
                let agent_id = matches[0].agent_id.clone();
                self.focus_teammate_by_id(&agent_id);
                Ok(())
            }
            _ => {
                let names: Vec<_> = matches.iter().map(|m| m.name.as_str()).collect();
                Err(format!("Ambiguous: matches {}", names.join(", ")))
            }
        }
    }

    pub(crate) fn send_teammate_message(
        &mut self,
        team_name: &str,
        teammate_name: &str,
        text: &str,
    ) {
        let member = self
            .team_members
            .iter()
            .find(|m| m.name == teammate_name)
            .cloned();
        let working_dir = std::env::current_dir().unwrap_or_default();
        match (self.active_team.as_ref(), member) {
            (Some(_team), Some(member)) => match TeamStore::load_by_name(team_name, &working_dir) {
                Ok(store) => match Mailbox::open(&store.dir, &member.agent_id) {
                    Ok(mb) => {
                        let msg = MailboxMessage::new(
                            "lead",
                            &member.agent_id,
                            MessageType::Message,
                            text,
                        );
                        match mb.push(msg) {
                            Ok(_) => {
                                self.push_log_no_agent(
                                    LogLevel::Info,
                                    format!("📨 lead → {teammate_name}: {text}"),
                                );
                                self.status = format!("message sent to {teammate_name}");
                            }
                            Err(e) => {
                                self.status = format!("Failed to send message: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        self.status = format!("Failed to open mailbox: {e}");
                    }
                },
                Err(e) => {
                    self.status = format!("Failed to load team: {e}");
                }
            },
            (None, _) => {
                self.status = "No active team".to_string();
            }
            (Some(_), None) => {
                self.status = format!("Teammate '{teammate_name}' not found");
            }
        }
    }

    pub(crate) fn is_current_session(&self, session_id: &str) -> bool {
        self.session_id.as_deref() == Some(session_id)
    }

    /// Returns the session id for a team member by agent id, if known.
    /// Used to attribute team-related log entries to the teammate's own
    /// output-view overlay.
    pub(crate) fn team_member_session_id_by_agent_id(&self, agent_id: &str) -> Option<String> {
        self.team_members
            .iter()
            .find(|m| m.agent_id == agent_id)
            .and_then(|m| m.session_id.clone())
    }

    /// Returns `true` when the session belongs to a tracked sub-agent task or
    /// team member rendered in the Agents / Teams panels.
    ///
    /// Used to mirror tool-call activity for those sessions into the shared
    /// log so the panels' step count (one per `ToolCallStart`) is auditable.
    pub(crate) fn is_tracked_agent_session(&self, session_id: &str) -> bool {
        self.active_tasks
            .iter()
            .any(|t| t.child_session_id == session_id)
            || self
                .team_members
                .iter()
                .any(|m| m.session_id.as_deref() == Some(session_id))
    }

    /// Returns the display tag used to attribute log lines to a tracked
    /// sub-agent task or team member, if any.
    ///
    /// Team members take precedence over sub-agent tasks when a session is
    /// registered in both (teammates also appear as sub-agent tasks).
    pub(crate) fn agent_log_tag(&self, session_id: &str) -> Option<String> {
        if let Some(member) = self
            .team_members
            .iter()
            .find(|m| m.session_id.as_deref() == Some(session_id))
        {
            return Some(member.name.clone());
        }
        self.active_tasks
            .iter()
            .find(|t| t.child_session_id == session_id)
            .map(|t| t.id.clone())
    }

    /// Build the `(agent_tag, agent_prefix, log_session_id)` triple used by
    /// every per-session log line that needs to attribute activity to a
    /// tracked sub-agent/teammate. Primary-session events get `agent_tag =
    /// None` and an empty prefix; tracked events get a `[name] ` prefix
    /// and the child session id so the per-agent output view picks them up.
    fn tracked_log_context(
        &self,
        session_id: &str,
        is_primary: bool,
    ) -> (Option<String>, String, Option<String>) {
        let agent_tag = if is_primary {
            None
        } else {
            self.agent_log_tag(session_id)
        };
        let agent_prefix = agent_tag
            .as_ref()
            .map(|tag| format!("[{tag}] "))
            .unwrap_or_default();
        let log_session_id = if is_primary {
            None
        } else {
            Some(session_id.to_string())
        };
        (agent_tag, agent_prefix, log_session_id)
    }

    /// Returns `true` for the primary session or any session that is a
    /// known descendant (sub-agent spawned by this TUI's agent tree).
    /// This lets nested sub-agents show up in the Agents panel even when
    /// the intermediate parent has already completed and been removed from
    /// `active_tasks`.
    pub(crate) fn is_current_or_descendant_session(&self, session_id: &str) -> bool {
        if self.session_id.as_deref() == Some(session_id) {
            return true;
        }
        self.active_tasks
            .iter()
            .any(|t| t.child_session_id == session_id || t.parent_session_id == session_id)
    }

    pub(crate) fn refresh_research_progress_message(&mut self, name: &str) {
        let Some(progress) = self
            .research_progress
            .iter()
            .find(|p| p.name == name)
            .cloned()
        else {
            return;
        };
        let rendered = self.render_markdown_to_ascii(&progress.render());
        // Must match the first line emitted by `ResearchProgress::render`
        // ("[research] Research Progress — `{name}`") so the existing
        // message is replaced in place instead of duplicating per event.
        const HEADER: &str = "[research] Research Progress";
        // Each research run gets its own message in the window, tagged with
        // the run name so older runs stay visible alongside the latest one.
        // We look for an existing assistant message whose first text part
        // starts with the header AND carries this run's name on its first
        // line, replacing it in place; otherwise we insert a new message.
        let header_line = format!("{HEADER} — `{}`", name);

        for msg in self.messages.iter_mut() {
            if msg.role != Role::Assistant {
                continue;
            }
            if let Some(MessagePart::Text { text }) = msg.parts.first_mut()
                && text.starts_with(HEADER)
                && text
                    .lines()
                    .next()
                    .map(|l| l == header_line)
                    .unwrap_or(false)
            {
                *text = rendered;
                msg.touch();
                return;
            }
        }

        if let Some(ref sid) = self.session_id {
            self.force_new_message = false;
            self.messages.push(Message::new(
                sid.clone(),
                Role::Assistant,
                vec![MessagePart::Text { text: rendered }],
            ));
            self.trim_messages_if_needed();
        }
    }

    /// Append assistant streaming text to the chat log, reusing the last
    /// assistant message when possible (and starting a new Text part after a
    /// tool call so ordering is preserved).
    pub(crate) fn append_assistant_text(&mut self, text: &str) {
        let rendered = self.render_markdown_to_ascii(text);
        if !self.force_new_message {
            if let Some(last) = self.messages.last_mut()
                && last.role == Role::Assistant
            {
                // Only append to the last part if it is a Text part;
                // otherwise start a new Text part so text after tool calls
                // appears in the correct position.
                if let Some(MessagePart::Text { text: t }) = last.parts.last_mut() {
                    t.push_str(&rendered);
                } else {
                    last.parts.push(MessagePart::Text {
                        text: rendered.clone(),
                    });
                }
                last.touch();
                return;
            }
        }
        self.force_new_message = false;
        // Create new assistant message
        if let Some(ref sid) = self.session_id {
            let msg = Message::new(
                sid,
                Role::Assistant,
                vec![MessagePart::Text { text: rendered }],
            );
            self.messages.push(msg);
            // T-010/FR-013: message count changed; refresh the Context panel.
            self.schedule_context_snapshot_refresh();
        }
    }

    pub(crate) fn append_reasoning_text(&mut self, text: &str) {
        if let Some(last) = self.messages.last_mut()
            && last.role == Role::Assistant
        {
            if let Some(MessagePart::Reasoning { text: t }) = last.parts.last_mut() {
                t.push_str(text);
            } else {
                last.parts.push(MessagePart::Reasoning {
                    text: text.to_string(),
                });
            }
            last.touch();
            return;
        }
        if let Some(ref sid) = self.session_id {
            let msg = Message::new(
                sid,
                Role::Assistant,
                vec![MessagePart::Reasoning {
                    text: text.to_string(),
                }],
            );
            self.messages.push(msg);
            self.trim_messages_if_needed();
            // T-010/FR-013: message count changed; refresh the Context panel.
            self.schedule_context_snapshot_refresh();
        }
    }

    pub(crate) fn add_tool_call_part(&mut self, tool: &str, call_id: &str) {
        use ragent_agent::message::{ToolCallState, ToolCallStatus};

        if let Some(last) = self.messages.last_mut()
            && last.role == Role::Assistant
        {
            last.parts.push(MessagePart::ToolCall {
                tool: tool.to_string(),
                call_id: call_id.to_string(),
                state: ToolCallState {
                    status: ToolCallStatus::Running,
                    input: serde_json::Value::Null,
                    output: None,
                    error: None,
                    duration_ms: None,
                },
            });
            last.touch();
            return;
        }
        if let Some(ref sid) = self.session_id {
            let msg = Message::new(
                sid,
                Role::Assistant,
                vec![MessagePart::ToolCall {
                    tool: tool.to_string(),
                    call_id: call_id.to_string(),
                    state: ToolCallState {
                        status: ToolCallStatus::Running,
                        input: serde_json::Value::Null,
                        output: None,
                        error: None,
                        duration_ms: None,
                    },
                }],
            );
            self.messages.push(msg);
            self.trim_messages_if_needed();
            // T-010/FR-013: message count changed; refresh the Context panel.
            self.schedule_context_snapshot_refresh();
        }
    }

    pub(crate) fn update_tool_call_status(
        &mut self,
        call_id: &str,
        success: bool,
        error: Option<&str>,
        duration_ms: u64,
    ) {
        use ragent_agent::message::ToolCallStatus;

        for msg in self.messages.iter_mut().rev() {
            for part in msg.parts.iter_mut() {
                if let MessagePart::ToolCall {
                    call_id: cid,
                    state,
                    ..
                } = part
                    && cid == call_id
                {
                    state.status = if success {
                        ToolCallStatus::Completed
                    } else {
                        ToolCallStatus::Error
                    };
                    if let Some(err) = error {
                        state.error = Some(err.to_string());
                    }
                    state.duration_ms = Some(duration_ms);
                    msg.touch();
                    return;
                }
            }
        }
    }

    pub(crate) fn update_tool_call_input(&mut self, call_id: &str, args_json: &str) -> bool {
        if let Ok(input) = serde_json::from_str::<serde_json::Value>(args_json) {
            for msg in self.messages.iter_mut().rev() {
                for part in msg.parts.iter_mut() {
                    if let MessagePart::ToolCall {
                        call_id: cid,
                        state,
                        ..
                    } = part
                        && cid == call_id
                    {
                        // Never overwrite an input that was already populated
                        // (e.g. by the per-call ToolCallArgs event). A later
                        // ToolCallBatch fallback carrying the same args would
                        // otherwise clobber it — and batch entries built by
                        // older code paths may carry a placeholder `{}`.
                        if state.input.is_null() {
                            state.input = input;
                            msg.touch();
                        }
                        return true;
                    }
                }
            }
        }
        false
    }

    pub(crate) fn find_tool_call_part(&self, call_id: &str) -> bool {
        for msg in self.messages.iter().rev() {
            for part in &msg.parts {
                if let MessagePart::ToolCall { call_id: cid, .. } = part
                    && cid == call_id
                {
                    return true;
                }
            }
        }
        false
    }

    /// Applies tool-call args held in `pending_tool_args` to an existing
    /// ToolCall part whose `state.input` has not been populated yet.
    ///
    /// This is the safety net for the case where the Args event was processed
    /// *before* the Start event but neither the Start-end pending application
    /// nor the `ToolCallBatch` fallback fired (e.g. `handle_event` calls with
    /// the event variant extracted, or the batch itself was lost). Without
    /// this drain, the part would render with an empty input summary even
    /// though the args were received and logged correctly.
    pub(crate) fn drain_pending_tool_args(&mut self) {
        if self.pending_tool_args.is_empty() {
            return;
        }
        let call_ids: Vec<String> = self.pending_tool_args.keys().cloned().collect();
        for call_id in call_ids {
            let mut applied = false;
            for msg in self.messages.iter_mut().rev() {
                for part in msg.parts.iter_mut() {
                    if let MessagePart::ToolCall {
                        call_id: cid,
                        state,
                        ..
                    } = part
                        && cid == &call_id
                    {
                        if state.input.is_null() {
                            let Some(args_json) = self.pending_tool_args.get(&call_id).cloned()
                            else {
                                break;
                            };
                            if let Ok(input) = serde_json::from_str::<serde_json::Value>(&args_json)
                            {
                                state.input = input;
                                msg.touch();
                            }
                        }
                        self.pending_tool_args.remove(&call_id);
                        applied = true;
                        break;
                    }
                }
                if applied {
                    break;
                }
            }
        }
    }

    pub(crate) fn update_tool_call_output(
        &mut self,
        call_id: &str,
        content_line_count: usize,
        metadata: Option<&serde_json::Value>,
    ) {
        let mut value = serde_json::json!({ "line_count": content_line_count });
        // Merge tool metadata fields into the output for the TUI
        if let Some(meta) = metadata {
            if let Some(obj) = meta.as_object() {
                for (k, v) in obj {
                    value[k] = v.clone();
                }
            }
        }
        for msg in self.messages.iter_mut().rev() {
            for part in msg.parts.iter_mut() {
                if let MessagePart::ToolCall {
                    call_id: cid,
                    state,
                    ..
                } = part
                    && cid == call_id
                {
                    state.output = Some(value);
                    msg.touch();
                    return;
                }
            }
        }
    }

    /// If autopilot is enabled and a continue message is pending, dispatch it
    /// as the next user turn. Clears the pending continue when the agent is
    /// busy, when autopilot is disabled, or when the task was already marked
    /// as completed by the agent (TaskCompleted consumed before or after us).
    pub(crate) fn poll_autopilot_continue(&mut self) {
        if self.autopilot_continued_this_wake {
            return;
        }
        if self.last_task_completed_at.is_some() {
            self.autopilot_pending_continue = None;
            self.autopilot_enabled = false;
            self.autopilot_started_at = None;
            self.status = "task complete".to_string();
            self.push_log_no_agent(
                LogLevel::Info,
                "autopilot stopped: task complete (suppressed continuation)".to_string(),
            );
            return;
        }
        if !self.autopilot_enabled || self.is_processing {
            self.autopilot_pending_continue = None;
            return;
        }
        if let Some(text) = self.autopilot_pending_continue.take() {
            self.autopilot_continued_this_wake = true;
            self.dispatch_user_message(text, vec![]);
        }
    }

    pub(crate) fn dispatch_spec_impl_task(
        &mut self,
        prompt: String,
        spec_id: &str,
        rank: usize,
        total: usize,
    ) {
        let session_id = self.session_id.clone().unwrap_or_default();

        // Use the coder agent for implementation.
        let mut agent = self.agent_info.clone();
        self.apply_selected_model_and_thinking(&mut agent);

        let msg = Message::user_text(&session_id, &prompt);
        self.messages.push(msg);
        self.trim_messages_if_needed();

        let processor = self.session_processor.clone();
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(flag.clone());
        self.is_processing = true;
        self.status = format!("spec: implementing {spec_id} — task {rank}/{total}");

        let event_bus = self.event_bus.clone();
        let sid = session_id;
        tokio::spawn(async move {
            if let Err(e) = processor.process_message(&sid, &prompt, &agent, flag).await {
                tracing::warn!(error = %e, "spec: implementation failed");
                event_bus.publish(ragent_agent::event::Event::AgentError {
                    session_id: sid,
                    error: format!("spec implementation failed: {e}"),
                });
            }
        });
    }

    pub(crate) fn advance_spec_impl(&mut self) {
        use ragent_specs::{SpecManager, spec::SpecStatus};

        let Some(state) = self.spec_impl_state.clone() else {
            return;
        };
        let rt = tokio::runtime::Handle::current();
        let mgr = SpecManager::new(&state.specs_root);
        let sid = match ragent_specs::spec::SpecId::new(&state.spec_id) {
            Some(id) => id,
            None => {
                self.spec_impl_state = None;
                self.append_assistant_text(&format!(
                    "From: /spec impl\n\n[warn] Invalid spec ID `{}` — run stopped.",
                    state.spec_id,
                ));
                return;
            }
        };

        // Read the spec to check the just-run task's status.
        let spec = tokio::task::block_in_place(|| rt.block_on(async { mgr.read_spec(&sid).await }));
        let spec = match spec {
            Ok(s) => s,
            Err(e) => {
                self.spec_impl_state = None;
                self.append_assistant_text(&format!(
                    "From: /spec impl\n\n[warn] Failed to read spec `{}` after task {}: {}",
                    state.spec_id, state.current_rank, e,
                ));
                return;
            }
        };

        let current_task_id = state
            .task_ids
            .get(state.current_rank.saturating_sub(1))
            .cloned()
            .unwrap_or_default();
        let task_status = spec
            .tasks
            .iter()
            .find(|t| t.id == current_task_id)
            .map(|t| t.status)
            .unwrap_or(ragent_specs::spec::TaskStatus::Pending);

        // Only `Blocked` stops the run — the agent explicitly signalled it
        // cannot proceed. For `Pending` or `InProgress` (the agent finished
        // its turn but forgot to call `spec_task_update`), auto-mark the task
        // as `Completed` and continue to the next task. This is the common
        // case: agents implement the task, end their turn (often via
        // `agent_complete`), but don't update the spec task status.
        if task_status == ragent_specs::spec::TaskStatus::Blocked {
            self.spec_impl_state = None;
            self.append_assistant_text(&format!(
                "From: /spec impl\n\n[stop] Task **{}** ({}/{}) is **blocked** — run stopped.\n\n\
                 Re-run `/spec impl {}` to resume from this task.",
                current_task_id, state.current_rank, state.total, state.spec_id,
            ));
            self.push_log_no_agent(
                LogLevel::Warn,
                format!("spec impl: task {} blocked — run stopped", current_task_id),
            );
            return;
        }

        // If the agent didn't mark the task as completed itself, do it now
        // so the PLAN.md reflects reality and the run can continue.
        let mut spec = spec;
        if task_status != ragent_specs::spec::TaskStatus::Completed {
            if let Err(e) = tokio::task::block_in_place(|| {
                rt.block_on(async {
                    mgr.update_task_status(
                        &mut spec,
                        &current_task_id,
                        ragent_specs::spec::TaskStatus::Completed,
                    )
                    .await
                })
            }) {
                self.spec_impl_state = None;
                self.append_assistant_text(&format!(
                    "From: /spec impl\n\n[warn] Failed to auto-complete task **{}**: {e}",
                    current_task_id,
                ));
                return;
            }
            self.push_log_no_agent(
                LogLevel::Info,
                format!(
                    "spec impl: auto-marked task {} as completed (agent did not call spec_task_update)",
                    current_task_id,
                ),
            );
        }

        // Task completed — advance to the next task or finish the run.
        let next_rank = state.current_rank + 1;
        if next_rank > state.total {
            // All tasks done — transition the spec to `implemented`.
            self.spec_impl_state = None;
            if spec.status == SpecStatus::InProgress {
                if let Err(e) = tokio::task::block_in_place(|| {
                    rt.block_on(async {
                        mgr.transition(&mut spec, SpecStatus::Implemented, "spec-impl")
                            .await
                    })
                }) {
                    self.push_log_no_agent(
                        LogLevel::Warn,
                        format!("spec impl: failed to transition to implemented: {e}"),
                    );
                }
            }
            self.append_assistant_text(&format!(
                "From: /spec impl\n\n🎉 All {} tasks completed for spec **{}**. \
                 Spec status set to `implemented`.",
                state.total, state.spec_id,
            ));
            self.status = format!("spec: {} implemented", state.spec_id);
            return;
        }

        // Dispatch the next task's prompt using the original runner so the
        // rank/total reflect the original execution plan, even as tasks are
        // completed and filtered out of a rebuilt runner.
        let next_task_id = state
            .task_ids
            .get(next_rank.saturating_sub(1))
            .cloned()
            .unwrap_or_default();
        let prompt = match state.runner.task_prompt(next_rank) {
            Some(p) => p,
            None => {
                self.spec_impl_state = None;
                self.append_assistant_text(&format!(
                    "From: /spec impl\n\n[warn] No task at rank {} — run stopped.",
                    next_rank,
                ));
                return;
            }
        };

        // Update the state's current_rank before dispatching.
        if let Some(s) = self.spec_impl_state.as_mut() {
            s.current_rank = next_rank;
        }
        self.append_assistant_text(&format!(
            "From: /spec impl\n\n[ok] Task **{}** completed ({}/{}). Next: **{}**.",
            current_task_id, state.current_rank, state.total, next_task_id,
        ));
        self.dispatch_spec_impl_task(prompt, &state.spec_id, next_rank, state.total);
    }
}

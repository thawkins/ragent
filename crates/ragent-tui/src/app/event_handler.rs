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
    App, LlmRequestStat, LogLevel, ModelDownloadState, ModelLoadingState, OutputViewState,
    OutputViewTarget, PlanApprovalState, ProviderSetupStep, QuestionRequest, ResearchViewState,
};

// Helpers
use crate::app::helpers::{is_discovery_notice, short_session_id, summarise_error};

// Re-export status types from theme

impl App {
    /// Poll the pending swarm decomposition result and, if ready, render the
    /// team summary (or surface an error) into the chat log.
    pub fn poll_pending_swarm(&mut self) {
        let outcome = {
            let mut guard = match self.swarm_result.lock() {
                Ok(g) => g,
                Err(poisoned) => {
                    tracing::error!("swarm_result mutex poisoned, recovering");
                    poisoned.into_inner()
                }
            };
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
                        self.status = "⚠ swarm: decomposition parse error".to_string();
                        self.append_assistant_text(&format!(
                            "From: /swarm\n## ❌ Decomposition Failed\n\n{}\n",
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
                self.status = format!("⚠ swarm failed: {}", msg);
                self.append_assistant_text(&format!(
                    "From: /swarm\n## ❌ Swarm Error\n\n{}\n",
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

    /// Dispatch a single [`Event`] from the event bus to the appropriate UI
    /// handler. Marks the UI dirty on every event so the next render reflects
    /// the state change.
    pub fn handle_event(&mut self, event: Event) {
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
            } if self.is_current_session(session_id) => {
                self.stream_in_bytes += (call_id.len() + tool.len()) as u64;
                telemetry_counters::increment_tool_invocations(1);
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
                self.add_tool_call_part(tool, call_id);

                // If args were received before the start event, apply them now.
                if let Some(args_json) = self.pending_tool_args.remove(call_id) {
                    let _ = self.update_tool_call_input(call_id, &args_json);
                }

                self.status = format!("running: {}", tool);
                let display_name = self
                    .sid_to_display_name
                    .get(&short_sid)
                    .cloned()
                    .unwrap_or(short_sid);
                self.push_log_no_agent(
                    LogLevel::Tool,
                    format!(
                        "[{display_name}:{step}.{current_substep}] tool call: {}",
                        tool
                    ),
                );
            }
            Event::ToolCallEnd {
                ref session_id,
                ref call_id,
                ref tool,
                ref error,
                duration_ms,
            } if self.is_current_session(session_id) => {
                telemetry_counters::set_tool_duration_last(duration_ms as f64);
                self.update_tool_call_status(
                    call_id,
                    error.is_none(),
                    error.as_deref(),
                    duration_ms,
                );
                self.set_status_working("processing");
                let step_tag = self
                    .tool_step_map
                    .get(call_id)
                    .map(|(sid, step, substep)| {
                        let name = self
                            .sid_to_display_name
                            .get(sid)
                            .cloned()
                            .unwrap_or_else(|| sid.clone());
                        format!("[{name}:{step}.{substep}] ")
                    })
                    .unwrap_or_default();
                if let Some(err) = error {
                    self.push_log_no_agent(
                        LogLevel::Error,
                        format!(
                            "{}tool {} failed: {} ({}ms)",
                            step_tag, tool, err, duration_ms
                        ),
                    );
                } else {
                    self.push_log_no_agent(
                        LogLevel::Tool,
                        format!("{}tool {} completed ({}ms)", step_tag, tool, duration_ms),
                    );
                }
            }
            Event::MessageStart {
                ref session_id,
                ref message_id,
            } if self.is_current_session(session_id) => {
                self.is_processing = true;
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
                let was_auto_compaction = self.auto_compact_in_progress;
                self.is_processing = false;
                self.cancel_flag = None;
                if *reason == FinishReason::Cancelled {
                    self.agent_halted = true;
                    self.status = "halted — /resume to continue".to_string();
                    self.push_log_no_agent(LogLevel::Warn, "Agent halted by user".to_string());
                } else {
                    self.agent_halted = false;
                    self.status = "ready".to_string();
                }
                self.force_new_message = true;
                self.push_log_no_agent(LogLevel::Info, format!("response finished ({reason:?})"));

                // After compaction: replace session message history with just the summary.
                // The summary is the last assistant message in self.messages.
                if self.compact_in_progress && *reason != FinishReason::Cancelled {
                    self.compact_in_progress = false;
                    self.needs_redraw = true;
                    let summary_text = self
                        .messages
                        .iter()
                        .rev()
                        .find(|m| m.role == Role::Assistant)
                        .map(|m| m.text_content());
                    if let Some(summary) = summary_text {
                        self.apply_compaction_summary(session_id, &summary);
                    }
                } else {
                    self.compact_in_progress = false;
                    self.needs_redraw = true;
                }
                // Session title generation has been removed along with the
                // internal LLM subsystem; sessions default to an empty
                // title and rely on the user to rename them.
                let _ = (session_id, reason);
                // Handle pending plan delegation: switch agent and auto-send task
                if let Some((task, context)) = self.pending_plan_task.take() {
                    self.execute_plan_delegation(session_id, task, context);
                }

                // /spec impl sequential driver: after each agent turn ends,
                // check the just-run task's status and dispatch the next task
                // (or finish the run). Skip when the turn was cancelled so the
                // user can resume manually with `/spec impl`.
                if self.spec_impl_state.is_some() && *reason != FinishReason::Cancelled {
                    self.advance_spec_impl();
                }

                // Autopilot auto-continue: after agent completes a turn without calling
                // task_complete, automatically send a continuation prompt so the agent
                // keeps working towards its goal.
                if self.autopilot_enabled && *reason != FinishReason::Cancelled {
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
                                "Continue working on the task. When fully done, call task_complete with a summary.".to_string()
                            );
                        self.status = "⚡ autopilot".to_string();
                    }
                }

                if was_auto_compaction {
                    self.auto_compact_in_progress = false;
                    self.push_log_no_agent(LogLevel::Info, "Auto-compaction completed".to_string());
                    if let Some((queued_text, queued_images)) =
                        self.pending_send_after_compact.take()
                    {
                        self.dispatch_user_message(queued_text, queued_images);
                    }
                }
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
                self.push_log_no_agent(LogLevel::Info, "task_complete signalled".to_string());
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
                self.append_assistant_text(&format!("✅ **Task Complete**\n\n{}", summary));
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
                    );
                    // Keep the status bar in sync with the running phase.
                    // The `⏳` prefix marks this as async-in-progress so
                    // [`App::arm_status_expiry`] will not auto-clear it to
                    // "ready" while the background research is still running.
                    // Errors outside the web phase surface as a warning
                    // status instead.
                    if decoded.status == crate::research_progress::StepStatus::Error
                        && decoded.phase != crate::research_progress::SessionPhase::Web
                    {
                        self.status = format!("⚠ research: {}", decoded.detail);
                    } else if decoded.total_sources.is_none() {
                        self.status = format!(
                            "⏳ research: {} — {} ({}) — running",
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
                        self.research_progress.last_mut().expect("just pushed")
                    };
                    progress.apply(decoded.phase, decoded.status, decoded.detail);
                    if let Some(total) = decoded.total_sources {
                        progress.finish(total);
                        // The final progress event marks the run complete.
                        // Drop the `⏳` in-progress status for a terminal
                        // message and arm the auto-expiry timer so it
                        // transitions to "ready" after the grace period.
                        self.status =
                            format!("research: {} complete — {total} sources", decoded.name);
                        self.arm_status_expiry();
                    }
                    let name_for_refresh = decoded.name.clone();
                    self.refresh_research_progress_message(&name_for_refresh);
                    return;
                }
                self.push_log_no_agent(LogLevel::Info, format!("agent notice: {}", message));
                // The instruction-file-discovery summary is multi-line and is
                // also rendered into the message window. Suppressing it in
                // the status bar avoids a duplicated, truncated/overflowing
                // copy on the right of status line 1.
                if !is_discovery_notice(message) {
                    self.status = summarise_error(message);
                }
                // Also display in the message window for visibility
                self.append_assistant_text(&format!("📋 **Agent Notice**\n{}", message));
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
                self.append_assistant_text(&format!("⚠ {}", summary));
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
                    .map(|(sid, step, substep)| {
                        let display = self
                            .sid_to_display_name
                            .get(sid)
                            .cloned()
                            .unwrap_or_else(|| sid.clone());
                        format!("[{display}:{step}.{substep}] ")
                    })
                    .unwrap_or_default();
                // Pretty-print JSON args across multiple log lines
                let pretty = serde_json::from_str::<serde_json::Value>(args)
                    .ok()
                    .and_then(|v| serde_json::to_string_pretty(&v).ok());
                if let Some(formatted) = pretty {
                    let mut first = true;
                    for line in formatted.lines() {
                        if first {
                            self.push_log_no_agent(
                                LogLevel::Tool,
                                format!("{}→ {} {}", step_tag, tool, line),
                            );
                            first = false;
                        } else {
                            self.push_log_no_agent(LogLevel::Tool, format!("  {}", line));
                        }
                    }
                } else {
                    self.push_log_no_agent(
                        LogLevel::Tool,
                        format!("{}→ {}({})", step_tag, tool, args),
                    );
                }
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
                    .map(|(sid, step, substep)| {
                        let display = self
                            .sid_to_display_name
                            .get(sid)
                            .cloned()
                            .unwrap_or_else(|| sid.clone());
                        format!("[{display}:{step}.{substep}] ")
                    })
                    .unwrap_or_default();
                let icon = if success { "✓" } else { "✗" };
                self.push_log_no_agent(
                    LogLevel::Tool,
                    format!("{}← {} {} {}", step_tag, tool, icon, content),
                );
            }
            Event::SubagentStart {
                ref session_id,
                ref task_id,
                ref child_session_id,
                ref agent,
                ref task,
                background,
                ..
            } if self.is_current_session(session_id) => {
                telemetry_counters::increment_subagent_spawns(1);
                telemetry_counters::add_agents_active(1);
                // Map the child session's short_sid to the agent name for display
                let short_sid = short_session_id(child_session_id);
                self.sid_to_display_name.insert(short_sid, agent.clone());

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
                };
                self.active_tasks.push(entry);
                let (icon, kind) = if background {
                    ("⚙️", "Background")
                } else {
                    ("🔄", "Foreground")
                };
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!(
                        "{} {} task started: {} ({})",
                        icon,
                        kind,
                        &task_id[..8.min(task_id.len())],
                        agent
                    ),
                );
            }
            Event::SubagentComplete {
                ref session_id,
                ref task_id,
                ref summary,
                success,
                ..
            } if self.is_current_session(session_id) => {
                telemetry_counters::add_agents_active(-1);
                telemetry_counters::increment_agents_completed(1);
                if let Some(idx) = self.active_tasks.iter().position(|t| t.id == *task_id) {
                    self.active_tasks.remove(idx);
                }
                let icon = if success { "✅" } else { "❌" };
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!(
                        "{} Task completed ({}): {}",
                        icon,
                        &task_id[..8.min(task_id.len())],
                        summary
                    ),
                );
            }
            Event::SubagentCancelled {
                ref session_id,
                ref task_id,
            } if self.is_current_session(session_id) => {
                if let Some(idx) = self.active_tasks.iter().position(|t| t.id == *task_id) {
                    self.active_tasks.remove(idx);
                }
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("🚫 Task cancelled ({})", &task_id[..8.min(task_id.len())]),
                );
            }
            Event::SubagentSuspended {
                ref session_id,
                ref task_id,
                child_session_id: _,
            } if self.is_current_session(session_id) => {
                if let Some(task) = self.active_tasks.iter_mut().find(|t| t.id == *task_id) {
                    task.status = ragent_agent::task::TaskStatus::Suspended;
                }
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("⏸ Task suspended ({})", &task_id[..8.min(task_id.len())]),
                );
            }
            Event::SubagentResumed {
                ref session_id,
                ref task_id,
                child_session_id: _,
            } if self.is_current_session(session_id) => {
                if let Some(task) = self.active_tasks.iter_mut().find(|t| t.id == *task_id) {
                    task.status = ragent_agent::task::TaskStatus::Running;
                }
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("▷ Task resumed ({})", &task_id[..8.min(task_id.len())]),
                );
            }
            Event::SubagentKilled {
                ref session_id,
                ref task_id,
                force,
                child_session_id: _,
            } if self.is_current_session(session_id) => {
                if let Some(idx) = self.active_tasks.iter().position(|t| t.id == *task_id) {
                    self.active_tasks.remove(idx);
                }
                let label = if force { "Force-killed" } else { "Killed" };
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("💀 {} task ({})", label, &task_id[..8.min(task_id.len())]),
                );
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
                if let Some(m) = self
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
                        // Map this teammate's session short_sid → name for log display
                        if let Some(ref sid) = stored.session_id {
                            let short_sid = short_session_id(sid);
                            self.sid_to_display_name
                                .insert(short_sid, teammate_name.clone());
                        }
                    }
                }
                self.show_teams = true;
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("🤝 [{team_name}] Spawned teammate '{teammate_name}' ({agent_id})"),
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
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("📨 [{team_name}] {from} → {to} ({message_type}): {preview}"),
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
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("🔀 [{team_name}] P2P {from} → {to} ({message_type}): {preview}"),
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
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("💤 [{team_name}] Teammate {agent_id} is idle"),
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
                self.push_log_no_agent(
                    LogLevel::Error,
                    format!("❌ [{team_name}] Teammate {agent_id} failed: {short_err}"),
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
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("📋 [{team_name}] {agent_id} claimed task {task_id}"),
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
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("✅ [{team_name}] {agent_id} completed task {task_id}"),
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
                    format!("🗑️  Team '{team_name}' cleaned up"),
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
                                        "⚠️ **Model download failed**\n\nProvider: `{}`\nModel: `{}`\nError: {}",
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
                    "⚠️ **{} failed to start**\n\nCommand: `{}`\nError: {}\n\nstdout:\n```\n{}\n```\n\nstderr:\n```\n{}\n```",
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

    pub(crate) fn suspend_agent_task(&mut self, task_id: &str) {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let tm = self.session_processor.task_manager.get().cloned();
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
            let tm = self.session_processor.task_manager.get().cloned();
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
            let tm = self.session_processor.task_manager.get().cloned();
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
        });
    }

    pub(crate) fn scroll_output_view_by(&mut self, delta: i16) {
        if let Some(ref mut view) = self.output_view {
            if delta >= 0 {
                view.scroll_offset = (view.scroll_offset + delta as u16).min(view.max_scroll);
            } else {
                view.scroll_offset = view.scroll_offset.saturating_sub((-delta) as u16);
            }
        }
    }

    pub(crate) fn jump_output_view_start(&mut self) {
        if let Some(ref mut view) = self.output_view {
            view.scroll_offset = 0;
        }
    }

    pub(crate) fn jump_output_view_end(&mut self) {
        if let Some(ref mut view) = self.output_view {
            view.scroll_offset = view.max_scroll;
        }
    }

    pub(crate) fn jump_research_view_start(&mut self) {
        if let Some(ref mut view) = self.research_view {
            view.scroll_offset = 0;
        }
    }

    pub(crate) fn jump_research_view_end(&mut self) {
        if let Some(ref mut view) = self.research_view {
            view.scroll_offset = view.max_scroll;
        }
    }

    pub(crate) fn scroll_research_view_by(&mut self, delta: i16) {
        if let Some(ref mut view) = self.research_view {
            if delta >= 0 {
                view.scroll_offset = (view.scroll_offset + delta as u16).min(view.max_scroll);
            } else {
                view.scroll_offset = view.scroll_offset.saturating_sub((-delta) as u16);
            }
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
        const HEADER: &str = "🔬 Research Progress";
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
        }
    }

    /// Append assistant streaming text to the chat log, reusing the last
    /// assistant message when possible (and starting a new Text part after a
    /// tool call so ordering is preserved).
    pub fn append_assistant_text(&mut self, text: &str) {
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
                        state.input = input;
                        return true;
                    }
                }
            }
        }
        false
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
                    return;
                }
            }
        }
    }

    /// If autopilot is enabled and a continue message is pending, dispatch it
    /// as the next user turn. Clears the pending continue when the agent is
    /// busy or autopilot is disabled.
    pub fn poll_autopilot_continue(&mut self) {
        if !self.autopilot_enabled || self.is_processing {
            self.autopilot_pending_continue = None;
            return;
        }
        if let Some(text) = self.autopilot_pending_continue.take() {
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
                    "From: /spec impl\n\n⚠️ Invalid spec ID `{}` — run stopped.",
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
                    "From: /spec impl\n\n⚠️ Failed to read spec `{}` after task {}: {}",
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

        if task_status != ragent_specs::spec::TaskStatus::Completed {
            // Task did not complete — stop the run so the user can resume.
            self.spec_impl_state = None;
            self.append_assistant_text(&format!(
                "From: /spec impl\n\n🚫 Task **{}** ({}/{}) is **{}** — run stopped.\n\n\
                 Re-run `/spec impl {}` to resume from this task.",
                current_task_id,
                state.current_rank,
                state.total,
                task_status.as_str(),
                state.spec_id,
            ));
            self.push_log_no_agent(
                LogLevel::Warn,
                format!(
                    "spec impl: task {} not completed (status={})",
                    current_task_id,
                    task_status.as_str(),
                ),
            );
            return;
        }

        // Task completed — advance to the next task or finish the run.
        let next_rank = state.current_rank + 1;
        if next_rank > state.total {
            // All tasks done — transition the spec to `implemented`.
            self.spec_impl_state = None;
            let mut spec = spec;
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

        // Dispatch the next task's prompt.
        let next_task_id = state
            .task_ids
            .get(next_rank.saturating_sub(1))
            .cloned()
            .unwrap_or_default();
        // Build the next task's prompt via a fresh runner so we don't need to
        // carry the `SpecImplRunner` across turns (it owns the parsed tasks).
        let opts = ragent_specs::ImplOptions::new();
        let runner = match tokio::task::block_in_place(|| {
            rt.block_on(async {
                ragent_specs::SpecImplRunner::new(&state.spec_id, state.specs_root.clone(), opts)
                    .await
            })
        }) {
            Ok(r) => r,
            Err(e) => {
                self.spec_impl_state = None;
                self.append_assistant_text(&format!(
                    "From: /spec impl\n\n⚠️ Failed to rebuild runner for task {}: {}",
                    next_rank, e,
                ));
                return;
            }
        };
        let prompt = match runner.task_prompt(next_rank) {
            Some(p) => p,
            None => {
                self.spec_impl_state = None;
                self.append_assistant_text(&format!(
                    "From: /spec impl\n\n⚠️ No task at rank {} — run stopped.",
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
            "From: /spec impl\n\n✅ Task **{}** completed ({}/{}). Next: **{}**.",
            current_task_id, state.current_rank, state.total, next_task_id,
        ));
        self.dispatch_spec_impl_task(prompt, &state.spec_id, next_rank, state.total);
    }
}

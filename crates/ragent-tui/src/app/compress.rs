//! Compaction command handling for the TUI.
//!
//! `/compact` performs a one-shot LLM summarisation of the current session
//! and replaces the in-memory message list with the resulting compaction
//! message (FR-009). The legacy `/compress` slash command is a deprecated
//! alias that forwards to the same path.
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ragent_agent::{
    agent::ModelRef,
    event::Event,
    message::{Message, MessagePart, Role},
};

// Prompt optimization templates

// State types from app/state.rs
use crate::app::state::{App, LogLevel};

// Helpers

// Re-export status types from theme

impl App {
    /// Poll the pending prompt-optimization result and, if ready, push it
    /// onto the input buffer (or surface an error in the status line).
    pub fn poll_pending_opt(&mut self) {
        let outcome = {
            let mut guard = match self.opt_result.lock() {
                Ok(g) => g,
                Err(poisoned) => {
                    tracing::error!("opt_result mutex poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            guard.take()
        };
        if let Some(outcome) = outcome {
            match outcome {
                Ok(text) => {
                    let lines = text.lines().count();
                    self.append_assistant_text(&text);
                    self.status = "opt: done".to_string();
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("Finished /opt — {} lines output", lines),
                    );
                    // Arm the status auto-expiry timer so "opt: done" transitions
                    // to "ready" after the grace period.
                    self.arm_status_expiry();
                }
                Err(msg) => {
                    self.status = format!("⚠ opt failed: {}", msg);
                    self.push_log_no_agent(LogLevel::Warn, format!("opt error: {}", msg));
                }
            }
        }
    }

    pub(crate) fn start_provider_compaction_for_session(
        &mut self,
        session_id: &str,
        auto_triggered: bool,
    ) -> bool {
        let compaction_agent =
            ragent_agent::agent::resolve_agent("compaction", &Default::default())
                .unwrap_or_else(|_| self.agent_info.clone());

        let mut agent = compaction_agent;
        let resolved_model = self
            .selected_model
            .as_deref()
            .and_then(|s| s.split_once('/'))
            .map(|(p, m)| ModelRef {
                provider_id: p.to_string(),
                model_id: m.to_string(),
            })
            .or_else(|| self.agent_info.model.clone());
        if let Some(model_ref) = resolved_model {
            agent.model = Some(model_ref);
        }
        self.apply_selected_model_and_thinking(&mut agent);

        let summary_prompt =
            "Summarise the conversation so far into a concise representation that \
             preserves all important context, decisions, code changes, file paths, \
             and outstanding tasks. Output only the summary — no preamble."
                .to_string();

        self.auto_compact_in_progress = auto_triggered;
        self.compact_in_progress = true;
        self.needs_redraw = true;
        if auto_triggered {
            self.auto_compact_failed = false;
            self.status = "compacting before send…".to_string();
            self.push_log_no_agent(
                LogLevel::Warn,
                "Auto-compaction triggered (provider fallback)".to_string(),
            );
        } else {
            self.status = "compacting…".to_string();
            self.push_log_no_agent(
                LogLevel::Info,
                "Compaction started with provider fallback".to_string(),
            );
        }

        let processor = self.session_processor.clone();
        let event_bus = self.event_bus.clone();
        let sid = session_id.to_string();
        tokio::spawn(async move {
            match processor
                .process_message(
                    &sid,
                    &summary_prompt,
                    &agent,
                    Arc::new(AtomicBool::new(false)),
                )
                .await
            {
                Ok(_) => {
                    tracing::info!(session_id = %sid, "Compaction LLM call completed");
                }
                Err(e) => {
                    tracing::error!(error = %e, "Compaction failed");
                    event_bus.publish(Event::AgentError {
                        session_id: sid,
                        error: format!("Compaction failed: {e}"),
                    });
                }
            }
        });
        true
    }

    pub(crate) fn apply_compaction_summary(&mut self, session_id: &str, summary: &str) -> bool {
        if summary.trim().is_empty() {
            return false;
        }
        let summary_msg = Message::new(
            session_id,
            Role::Assistant,
            vec![MessagePart::Text {
                text: format!("[Conversation compacted]\n\n{}", summary.trim()),
            }],
        );
        if let Err(error) = self.storage.delete_messages(session_id) {
            self.push_log_no_agent(
                LogLevel::Warn,
                format!("Compaction: failed to clear messages: {error}"),
            );
            return false;
        }
        if let Err(error) = self.storage.create_message(&summary_msg) {
            self.push_log_no_agent(
                LogLevel::Warn,
                format!("Compaction: failed to save summary: {error}"),
            );
            return false;
        }
        self.messages = vec![summary_msg];
        self.push_log_no_agent(
            LogLevel::Info,
            "Compaction: session history replaced with summary".to_string(),
        );
        true
    }
}

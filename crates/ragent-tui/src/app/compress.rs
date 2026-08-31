//! Compaction command handling for the TUI.
//!
//! `/compact` runs the dedicated compaction runner
//! ([`ragent_agent::session::processor::SessionProcessor::compact_session`])
//! — a single no-tools LLM summarisation call — and replaces the in-memory
//! message list with the returned `[compaction, ...recent]` history (FR-009).
//! The legacy `/compress` slash command is a deprecated alias that forwards to
//! the same path.
//!
//! The spawned task delivers its outcome through the `compact_result` mutex;
//! [`App::poll_compaction_result`] drains it on the UI thread and applies the
//! state transitions (history replacement, status, queued-send dispatch). The
//! old implementation spawned a full `process_message` agent turn instead,
//! which re-sent the entire history plus all tool definitions, ran the
//! AGENTS.md init acknowledgement, and could trigger double summarisation.
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ragent_agent::{
    agent::ModelRef,
    compaction::CompactionOutcome,
    message::{Message, Role},
};

// State types from app/state.rs
use crate::app::state::{App, LogLevel};

// Helpers
use crate::app::session_ops::recover_poisoned;

impl App {
    /// Poll the pending prompt-optimization result and, if ready, push it
    /// onto the input buffer (or surface an error in the status line).
    pub fn poll_pending_opt(&mut self) {
        let outcome = {
            let mut guard = recover_poisoned(self.opt_result.lock(), "opt_result");
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
                    self.status = format!("[warn] opt failed: {}", msg);
                    self.push_log_no_agent(LogLevel::Warn, format!("opt error: {}", msg));
                }
            }
            self.needs_redraw = true;
        }
    }

    /// Poll the pending compaction result and, if ready, apply the compacted
    /// history to the in-memory session (or surface the failure), then dispatch
    /// any user message queued behind an auto-compaction-before-send.
    pub fn poll_compaction_result(&mut self) {
        let outcome = {
            let mut guard = recover_poisoned(self.compact_result.lock(), "compact_result");
            guard.take()
        };
        let Some(outcome) = outcome else {
            return;
        };
        self.compact_in_progress = false;
        self.needs_redraw = true;
        let was_auto_compaction = self.auto_compact_in_progress;
        match outcome {
            Ok(new_messages) => {
                self.apply_compaction_messages(new_messages);
                // T-010/FR-013: schedule the Context panel snapshot refresh
                // AFTER the in-memory history has been replaced, so the
                // captured conversation token count reflects the compacted
                // history rather than the old, larger one.
                self.schedule_context_snapshot_refresh();
                if was_auto_compaction {
                    self.auto_compact_in_progress = false;
                    self.push_log_no_agent(LogLevel::Info, "Auto-compaction completed".to_string());
                }
                self.status = "ready".to_string();
                self.status_set_at = None;
            }
            Err(err) => {
                if was_auto_compaction {
                    self.auto_compact_in_progress = false;
                    self.auto_compact_failed = true;
                    self.pending_send_after_compact = None;
                    self.push_log_no_agent(
                        LogLevel::Warn,
                        "Auto-compaction failed; send blocked for this turn".to_string(),
                    );
                }
                self.status = format!("[warn] compact failed: {err}");
                self.push_log_no_agent(LogLevel::Error, format!("compaction error: {err}"));
            }
        }

        // FR-012-style chaining: once compaction fully completed, send the
        // message the user typed before auto-compaction was triggered.
        if let Some((queued_text, queued_images)) = self.pending_send_after_compact.take() {
            self.dispatch_user_message(queued_text, queued_images);
        }
    }

    pub(crate) fn start_provider_compaction_for_session(
        &mut self,
        session_id: &str,
        auto_triggered: bool,
    ) -> bool {
        let resolved_model = self
            .selected_model
            .as_deref()
            .and_then(|s| s.split_once('/'))
            .map(|(p, m)| ModelRef {
                provider_id: p.to_string(),
                model_id: m.to_string(),
            })
            .or_else(|| self.agent_info.model.clone());
        let Some(model_ref) = resolved_model else {
            self.status = "[warn] No model selected — use /model to choose".to_string();
            return false;
        };

        self.auto_compact_in_progress = auto_triggered;
        self.compact_in_progress = true;
        self.needs_redraw = true;
        if auto_triggered {
            self.auto_compact_failed = false;
            self.status = "compacting before send…".to_string();
            self.push_log_no_agent(LogLevel::Warn, "Auto-compaction triggered".to_string());
        } else {
            self.status = "compacting…".to_string();
            self.push_log_no_agent(LogLevel::Info, "Compaction started".to_string());
        }

        let processor = self.session_processor.clone();
        let compact_result = Arc::clone(&self.compact_result);
        let sid = session_id.to_string();
        let reason = if auto_triggered { "auto" } else { "manual" };
        let cancel = Arc::new(AtomicBool::new(false));
        tokio::spawn(async move {
            let outcome = processor
                .compact_session(&sid, &model_ref, reason, cancel.as_ref())
                .await
                .map(|o: CompactionOutcome| o.new_messages)
                .map_err(|e| e.to_string());
            if let Ok(mut guard) = compact_result.lock() {
                *guard = Some(outcome);
            } else {
                tracing::error!("compact_result mutex poisoned, result dropped");
            }
        });
        true
    }

    /// Replace the in-memory session history with the compacted form
    /// (`[compaction_message, ...recent]`). Storage was already replaced by
    /// [`SessionProcessor::compact_session`]; this only mirrors it into the UI.
    pub(crate) fn apply_compaction_messages(&mut self, new_messages: Vec<Message>) {
        if new_messages.is_empty() {
            return;
        }
        let summary_present = new_messages
            .iter()
            .any(|m| m.role == Role::Compaction || m.role == Role::Assistant);
        self.messages = new_messages;
        // Structural change: the cache must be rebuilt from scratch because
        // the whole timeline was replaced by the summary message.
        self.message_line_cache.clear();
        if summary_present {
            self.push_log_no_agent(
                LogLevel::Info,
                "Compaction: session history replaced with summary".to_string(),
            );
        }
    }
}

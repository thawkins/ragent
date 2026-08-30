//! Session compaction operations (`/compact` and auto pre-send compaction).
//!
//! This module provides [`SessionProcessor::compact_session`], the single
//! entry point for running the OpenCode-derived compaction runner against a
//! session's persisted history. It deliberately bypasses the full agent loop
//! (`process_message`): the legacy TUI fallback spawned an entire agent turn
//! for the summarisation request, which re-sent the whole history plus ~169
//! tool definitions, ran the AGENTS.md init acknowledgement exchange (an extra
//! LLM call), applied the current model's thinking configuration, and allowed
//! the in-loop pre-send compaction trigger to fire again — producing up to
//! three summarisation LLM calls for one `/compact`.
//!
//! The runner path makes exactly one no-tools LLM call with a lean,
//! no-thinking request (see [`crate::compaction::build_summary_request`]) and
//! publishes `CompressionStarted` / `CompressionFinished` events itself.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, bail};
use tracing::{info, warn};

use crate::agent::{AgentInfo, AgentMode, ModelRef};
use crate::compaction::CompactionOutcome;
use crate::message::Role;
use crate::session::processor::SessionProcessor;

impl SessionProcessor {
    /// Compact a session's history via the dedicated compaction runner.
    ///
    /// Loads the persisted history from storage, runs a single no-tools LLM
    /// summarisation call through [`crate::compaction::compact`], and — on
    /// success — replaces the persisted history with
    /// `[compaction_message, ...recent]` so the next turn loads history from
    /// the compaction point forward (FR-005 / FR-007).
    ///
    /// Unlike [`SessionProcessor::process_message`], this never enters the
    /// agent loop: no tool definitions are sent, no system prompt is built,
    /// the AGENTS.md init acknowledgement exchange is skipped, and the
    /// current model's thinking configuration is not applied.
    ///
    /// # Arguments
    ///
    /// * `session_id` — the session to compact.
    /// * `model_ref` — provider/model used for the summarisation call.
    /// * `reason` — label carried on compaction lifecycle events (e.g.
    ///   `"manual"` for `/compact`, `"auto"` for pre-send compaction).
    /// * `cancel` — cooperative cancellation flag checked before the LLM call.
    ///
    /// # Errors
    ///
    /// Returns an error when compaction is cancelled, the provider/model
    /// cannot be resolved, the API key is missing, the model is known but the
    /// summarisation prompt would overflow its context window, the LLM call
    /// fails, or the summary comes back empty. Single-message sessions bail
    /// with "nothing to summarise" before any LLM call.
    ///
    /// On success the returned [`CompactionOutcome`] mirrors the new persisted
    /// history (`new_messages` = `[compaction_message, ...recent]`).
    pub async fn compact_session(
        &self,
        session_id: &str,
        model_ref: &ModelRef,
        reason: &str,
        cancel: &AtomicBool,
    ) -> Result<CompactionOutcome> {
        if cancel.load(Ordering::Relaxed) {
            bail!("compaction cancelled");
        }

        // Resolve the LLM client exactly as an agent turn would — reusing the
        // warm per-(provider, model) client cache — but with a synthetic
        // subagent agent that carries no prompt, no thinking config, and a
        // one-step bound. `prepare_client` publishes `AgentError` +
        // `MessageEnd` itself on failure, so TUI/server error surfacing keeps
        // working.
        let profiler = crate::session::profiler::agent_loop_profiler();
        let mut compaction_agent = AgentInfo::new(
            "compaction",
            "One-shot context compaction summariser (not user-selectable)",
        );
        compaction_agent.mode = AgentMode::Subagent;
        compaction_agent.max_steps = Some(1);
        compaction_agent.temperature = Some(0.2);
        compaction_agent.model = Some(model_ref.clone());
        let turn = self
            .prepare_client(session_id, "compaction", &compaction_agent, &profiler)
            .await?;

        if cancel.load(Ordering::Relaxed) {
            bail!("compaction cancelled");
        }

        // Load the persisted history the compaction runner operates on.
        let messages: Vec<crate::message::Message> = {
            let sid = session_id.to_string();
            self.storage_op(move |s| s.get_messages(&sid)).await?
        };

        // Feed any previous compaction summary as prior context (mirrors the
        // in-loop pre-send path).
        let previous_summary = messages
            .iter()
            .rev()
            .find(|m| m.role == Role::Compaction)
            .map(|m| m.text_content());

        // Resolve the context window the same way `build_turn_chat_messages`
        // does (128k fallback for virtual/unknown windows).
        let context_window = self
            .provider_registry
            .get(&model_ref.provider_id)
            .and_then(|p| {
                p.default_models()
                    .into_iter()
                    .find(|m| m.id == model_ref.model_id)
            })
            .map(|m| m.context_window)
            .filter(|w| *w > 0)
            .unwrap_or(128_000);

        let cfg = self.load_config_cached();
        let outcome = crate::compaction::compact(
            session_id,
            messages,
            &model_ref.model_id,
            context_window,
            0,
            &cfg.compaction,
            previous_summary.as_deref(),
            &turn.client,
            &self.event_bus,
            reason,
            &self.stream_config,
        )
        .await?;

        // Replace the persisted history with the compacted form so the next
        // turn loads `[compaction_message, ...recent]` and not the full
        // pre-compaction history again.
        //
        // `get_messages` orders rows by `created_at ASC`, but the synthetic
        // compaction message carries `Utc::now()` — newer than every retained
        // message — which would sort it to the END of the persisted history
        // and send the summary as the last context entry. Backdate it to one
        // millisecond before the oldest retained message so the chronological
        // order matches the FR-005 conceptual order.
        let mut new_messages = outcome.new_messages;
        if let Some(oldest_recent) = new_messages.get(1).map(|m| m.created_at) {
            let seeded = oldest_recent - chrono::Duration::milliseconds(1);
            if let Some(first) = new_messages.first_mut() {
                first.created_at = seeded;
                first.updated_at = seeded;
            }
        }
        let compaction_message = new_messages
            .first()
            .cloned()
            .unwrap_or_else(|| outcome.compaction_message.clone());
        let persist_messages = new_messages.clone();
        let persist_sid = session_id.to_string();
        let persist_err = self
            .storage_op(move |s| -> Result<()> {
                s.delete_messages(&persist_sid)?;
                for msg in &persist_messages {
                    s.create_message(msg)?;
                }
                Ok(())
            })
            .await
            .err();
        if let Some(e) = persist_err {
            warn!(session_id, error = %e, "failed to persist compacted history");
        }

        info!(
            session_id,
            reason,
            original_tokens = outcome.original_tokens,
            compressed_tokens = outcome.compressed_tokens,
            kept_messages = outcome.kept_message_count,
            "session compaction complete"
        );
        Ok(CompactionOutcome {
            new_messages,
            compaction_message,
            ..outcome
        })
    }
}

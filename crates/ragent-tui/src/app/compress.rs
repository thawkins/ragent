//! Context compression command handling for the TUI.
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

    pub(crate) fn handle_compress_command(&mut self, args: &str) {
        let subcmd = args.trim().to_lowercase();
        let is_empty = subcmd.is_empty();

        // /compress help — show subcommand help and current config
        if subcmd == "help" {
            let config = ragent_agent::Config::load().unwrap_or_default();
            let help = ragent_agent::compression::compress_help(&config.compression);
            self.append_assistant_text(&help);
            self.status = "compress help".to_string();
            return;
        }

        // /compress stats — show compression statistics for the current session
        if subcmd == "stats" {
            let msg_count = self.messages.len();
            let total_chars: usize = self
                .messages
                .iter()
                .map(|m| {
                    m.parts
                        .iter()
                        .map(|p| match p {
                            MessagePart::Text { text } => text.len(),
                            MessagePart::ToolCall { tool, state, .. } => {
                                tool.len()
                                    + state
                                        .output
                                        .as_ref()
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.len())
                                        .unwrap_or(0)
                                    + state.error.as_ref().map(|s| s.len()).unwrap_or(0)
                            }
                            MessagePart::Image { .. } => 1000,
                            MessagePart::Reasoning { text } => text.len(),
                        })
                        .sum::<usize>()
                })
                .sum();
            let est_tokens = ragent_agent::compression::count_tokens(&self.messages);
            let config = ragent_agent::Config::load().unwrap_or_default();
            let compression_status = if config.compression.enabled {
                "enabled"
            } else {
                "disabled"
            };

            let mut out = String::from("From: /compress stats\n\n");
            out.push_str(&format!("Messages: {}\n", msg_count));
            out.push_str(&format!("Total characters: {}\n", total_chars));
            out.push_str(&format!("Estimated tokens: {}\n", est_tokens));
            out.push_str(&format!("Compression: {}\n", compression_status));
            out.push_str(&format!(
                "Auto threshold: {:.0}%\n",
                config.compression.auto_threshold * 100.0
            ));
            out.push_str(&format!(
                "Compressors: json={}, diff={}, log={}, search={}, code={}, prose={}\n",
                config.compression.compressors.json,
                config.compression.compressors.diff,
                config.compression.compressors.log,
                config.compression.compressors.search,
                config.compression.compressors.code,
                config.compression.compressors.prose,
            ));
            out.push_str(&format!(
                "Relevance: {} (scorer={}, keep_top_k={})\n",
                if config.compression.relevance.enabled {
                    "on"
                } else {
                    "off"
                },
                config.compression.relevance.scorer,
                config.compression.relevance.keep_top_k,
            ));
            self.append_assistant_text(&out);
            self.status = "compress stats".to_string();
            return;
        }

        // /compress, /compress default, /compress aggressive, /compress conservative
        // Run the compression pipeline with the specified mode.
        if self.session_id.is_none() {
            self.status = "⚠ No active session to compress".to_string();
            return;
        }
        if self.messages.is_empty() {
            self.status = "⚠ No messages to compress".to_string();
            return;
        }

        let mode_str = if is_empty { "default" } else { &subcmd };

        {
            use ragent_agent::compression::CompressionMode;
            let mode = match mode_str.parse::<CompressionMode>() {
                Ok(m) => m,
                Err(e) => {
                    self.append_assistant_text(&format!(
                        "From: /compress\n⚠ Invalid mode '{}'. {}\n\n\
                                               Available modes: default, aggressive, conservative\n\
                                               Use /compress help for details.",
                        mode_str, e,
                    ));
                    self.status = "compress: invalid mode".to_string();
                    return;
                }
            };

            let config = ragent_agent::Config::load().unwrap_or_default();
            if !config.compression.enabled {
                self.append_assistant_text(
                              "From: /compress\n\
                                           ⚠ Compression is disabled in the configuration.\n\n\
                                           To enable, set `compression.enabled = true` in ragent.json, \
                                           or use /compact for LLM-based summarisation.",
                          );
                self.status = "compress: disabled".to_string();
                return;
            }

            // Mark compression as in-progress for the status bar indicator.
            self.compress_in_progress = true;
            self.needs_redraw = true;

            // Determine context window from the selected model.
            let context_window = self
                .selected_model
                .as_deref()
                .and_then(|s| s.split_once('/'))
                .and_then(|(p, m)| {
                    self.provider_registry
                        .resolve_model(p, m)
                        .map(|m| m.context_window)
                })
                .unwrap_or(128_000);

            let _original_tokens = ragent_agent::compression::count_tokens(&self.messages);

            let result = ragent_agent::compression::compress_history_with_mode(
                &self.messages,
                context_window,
                8192,
                &config.compression,
                mode,
            );

            let stats = &result.stats;
            let ratio = if stats.compressed_tokens > 0 {
                format!("{:.2}x", stats.compression_ratio)
            } else {
                "N/A".to_string()
            };

            let mut out = format!(
                "From: /compress {}\n\n\
                             Compression completed.\n\n\
                             | Metric | Value |\n\
                             |---|---|\n\
                             | Mode | {} |\n\
                             | Original tokens | {} |\n\
                             | Compressed tokens | {} |\n\
                             | Compression ratio | {} |\n\
                             | Messages compressed | {} |\n\
                             | CCR entries stashed | {} |\n",
                mode,
                mode,
                stats.original_tokens,
                stats.compressed_tokens,
                ratio,
                stats.messages_compressed,
                stats.ccr_entries_stashed,
            );

            if stats.compressed_tokens < stats.original_tokens {
                let saved = stats
                    .original_tokens
                    .saturating_sub(stats.compressed_tokens);
                out.push_str(&format!(
                    "\nSaved {} tokens ({:.1}% reduction).\n",
                    saved,
                    (saved as f64 / stats.original_tokens as f64) * 100.0
                ));
            } else {
                out.push_str(
                    "\nNo token reduction achieved. The context may already be within limits.\n",
                );
            }

            if result.messages.len() != self.messages.len() {
                out.push_str(&format!(
                    "\nMessages: {} → {}\n",
                    self.messages.len(),
                    result.messages.len()
                ));
            }

            // Replace messages with compressed result.
            self.messages = result.messages;
            self.compress_in_progress = false;
            self.append_assistant_text(&out);
            self.status = format!("compress {}", mode);
        }
    }
}

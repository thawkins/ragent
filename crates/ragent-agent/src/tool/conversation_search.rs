//! `conversation_search` — Search within the current session transcript.
//!
//! Provides keyword search, turn-range retrieval, and conversation statistics
//! for the active session.  The tool is read-only and uses the SQLite-backed
//! `messages_fts` index (and optional embedding cache) managed by
//! `ragent_storage`.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::event::Event;

/// Tool for searching the current session's message history.
///
/// Supports three modes:
/// - `keyword` (default): full-text search over the current session.
/// - `turn_range`: retrieve messages by turn index (1-based).
/// - `stats`: return message counts and compaction status.
pub struct ConversationSearchTool;

#[async_trait::async_trait]
impl Tool for ConversationSearchTool {
    fn name(&self) -> &'static str {
        "conversation_search"
    }

    fn description(&self) -> &'static str {
        "Search the current session conversation history. Modes: keyword (default), \
         turn_range, stats. REQUIRED parameter: none for stats mode; 'query' (string) \
         for keyword mode; 'start_turn' and 'end_turn' (integers, 1-based inclusive) \
         for turn_range mode. Optional: 'limit' (keyword result count, default 10), \
         'context_turns' (surrounding turns per keyword match, default 0). Returns \
         ranked keyword matches, a slice by turn number, or message-count statistics. \
         Common gotcha: keyword mode fails if 'query' is missing; turn_range requires \
         both start_turn and end_turn."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query for keyword mode (required when mode=keyword)"
                },
                "mode": {
                    "type": "string",
                    "enum": ["keyword", "turn_range", "stats"],
                    "description": "Search mode (default: keyword)"
                },
                "start_turn": {
                    "type": "integer",
                    "description": "First turn to include in turn_range mode (1-based, inclusive, required when mode=turn_range)"
                },
                "end_turn": {
                    "type": "integer",
                    "description": "Last turn to include in turn_range mode (1-based, inclusive, required when mode=turn_range)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results for keyword mode (default: 10)"
                },
                "context_turns": {
                    "type": "integer",
                    "description": "Number of surrounding turns to include around each keyword match (default: 0)"
                }
            },
            "required": [],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let storage = ctx
            .storage
            .as_deref()
            .context("conversation_search requires storage (SQLite) but none is available")?;

        let session_id = ctx.session_id.as_str();
        let mode = input["mode"].as_str().unwrap_or("keyword");

        match mode {
            "stats" => self.stats_mode(storage, session_id, ctx).await,
            "turn_range" => {
                let start = input["start_turn"].as_u64().unwrap_or(1).max(1) as usize;
                let end = input["end_turn"].as_u64().unwrap_or(start as u64) as usize;
                self.turn_range_mode(storage, session_id, start, end, ctx)
                    .await
            }
            _ => {
                let query = input["query"]
                    .as_str()
                    .context("Missing required 'query' parameter for keyword mode")?;
                let limit = input["limit"].as_u64().unwrap_or(10) as usize;
                let context_turns = input["context_turns"].as_u64().unwrap_or(0) as usize;
                self.keyword_mode(storage, session_id, query, limit, context_turns, ctx)
                    .await
            }
        }
    }
}

impl ConversationSearchTool {
    /// Keyword search within the current session.
    async fn keyword_mode(
        &self,
        storage: &crate::storage::Storage,
        session_id: &str,
        query: &str,
        limit: usize,
        context_turns: usize,
        ctx: &ToolContext,
    ) -> Result<ToolOutput> {
        let results = storage.search_conversation(session_id, query, limit)?;

        let mut output = String::new();
        if results.is_empty() {
            output.push_str(&format!(
                "No messages in the current session matched '{query}'."
            ));
        } else {
            output.push_str(&format!(
                "Found {} message(s) in the current session matching '{query}':\n\n",
                results.len()
            ));

            let all_messages = if context_turns > 0 {
                Some(storage.get_messages(session_id)?)
            } else {
                None
            };

            for (i, result) in results.iter().enumerate() {
                output.push_str(&format_message_result(i + 1, result));
                if context_turns > 0 {
                    if let Some(ref msgs) = all_messages {
                        append_context(&mut output, msgs, &result.message_id, context_turns);
                    }
                }
                output.push('\n');
            }
        }

        let () = ctx.event_bus.publish(Event::ConversationSearched {
            session_id: session_id.to_string(),
            query: query.to_string(),
            mode: "keyword".to_string(),
            result_count: results.len(),
        });

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "query": query,
                "mode": "keyword",
                "session_id": session_id,
                "result_count": results.len()
            })),
        })
    }

    /// Retrieve a range of conversation turns by index.
    async fn turn_range_mode(
        &self,
        storage: &crate::storage::Storage,
        session_id: &str,
        start_turn: usize,
        end_turn: usize,
        ctx: &ToolContext,
    ) -> Result<ToolOutput> {
        let messages = storage.get_messages(session_id)?;
        if messages.is_empty() {
            return Ok(ToolOutput {
                content: "The current session has no messages yet.".to_string(),
                metadata: Some(json!({
                    "mode": "turn_range",
                    "session_id": session_id,
                    "result_count": 0
                })),
            });
        }

        let start = (start_turn.saturating_sub(1)).min(messages.len() - 1);
        let end = (end_turn.saturating_sub(1)).min(messages.len() - 1);
        if start > end {
            return Ok(ToolOutput {
                content: format!(
                    "Invalid turn range: start_turn ({start_turn}) is greater than end_turn ({end_turn})."
                ),
                metadata: Some(json!({
                    "mode": "turn_range",
                    "session_id": session_id,
                    "result_count": 0
                })),
            });
        }

        let mut output = String::new();
        output.push_str(&format!(
            "Turns {}–{} of {} in the current session:\n\n",
            start + 1,
            end + 1,
            messages.len()
        ));

        for (idx, msg) in messages[start..=end].iter().enumerate() {
            let turn = start + idx + 1;
            let text = msg.text_content();
            let preview = truncate(&text, 300);
            output.push_str(&format!(
                "Turn {turn} [{}] {}: {preview}{suffix}\n",
                msg.created_at.to_rfc3339(),
                msg.role,
                suffix = if text.len() > 300 { "…" } else { "" }
            ));
        }

        let () = ctx.event_bus.publish(Event::ConversationSearched {
            session_id: session_id.to_string(),
            query: format!("turn_range:{start_turn}-{end_turn}"),
            mode: "turn_range".to_string(),
            result_count: end - start + 1,
        });

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "mode": "turn_range",
                "session_id": session_id,
                "start_turn": start + 1,
                "end_turn": end + 1,
                "result_count": end - start + 1
            })),
        })
    }

    /// Return conversation statistics.
    async fn stats_mode(
        &self,
        storage: &crate::storage::Storage,
        session_id: &str,
        ctx: &ToolContext,
    ) -> Result<ToolOutput> {
        let stats = storage.conversation_stats(session_id)?;

        let output = format!(
            "Conversation statistics for the current session:\n\n\
             Total messages: {}\n\
             User messages: {}\n\
             Assistant messages: {}\n\
             Compaction messages: {}\n\
             Has compaction summary: {}\n",
            stats.total,
            stats.user_count,
            stats.assistant_count,
            stats.compaction_count,
            if stats.has_compaction { "yes" } else { "no" }
        );

        let () = ctx.event_bus.publish(Event::ConversationSearched {
            session_id: session_id.to_string(),
            query: "stats".to_string(),
            mode: "stats".to_string(),
            result_count: stats.total as usize,
        });

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "mode": "stats",
                "session_id": session_id,
                "total": stats.total,
                "user_count": stats.user_count,
                "assistant_count": stats.assistant_count,
                "compaction_count": stats.compaction_count,
                "has_compaction": stats.has_compaction
            })),
        })
    }
}

/// Format a single keyword search result for display.
fn format_message_result(rank: usize, result: &ragent_storage::MessageSearchResult) -> String {
    let preview = truncate(&result.content, 300);
    let suffix = if result.content.len() > 300 {
        "…"
    } else {
        ""
    };
    format!(
        "{}. [{}] {}: {preview}{suffix}\n   (rank: {:.4})",
        rank, result.created_at, result.role, result.rank
    )
}

/// Append surrounding turns for a keyword match.
fn append_context(
    output: &mut String,
    messages: &[ragent_types::message::Message],
    target_id: &str,
    context_turns: usize,
) {
    let Some(pos) = messages.iter().position(|m| m.id == target_id) else {
        return;
    };
    let start = pos.saturating_sub(context_turns);
    let end = (pos + context_turns).min(messages.len() - 1);
    if start == end {
        return;
    }
    output.push_str("   Context:\n");
    for (offset, msg) in messages[start..=end].iter().enumerate() {
        let idx = start + offset;
        let marker = if idx == pos { "▶" } else { " " };
        let text = msg.text_content();
        let preview = truncate(&text, 200);
        let suffix = if text.len() > 200 { "…" } else { "" };
        output.push_str(&format!(
            "   {marker} turn {} [{}]: {preview}{suffix}\n",
            idx + 1,
            msg.role
        ));
    }
}

/// Truncate a string to `max` bytes without breaking UTF-8 boundaries.
fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

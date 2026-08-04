//! `session_search` — Search across all stored sessions.
//!
//! Performs ranked full-text search over every persisted session transcript,
//! with filters for date range, working directory, role, per-session limits,
//! and optional surrounding context for each match.  Semantic search is
//! prepared at the storage layer but falls back to FTS5 when no embedding
//! provider is available.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::event::Event;

/// Tool for searching messages across all sessions.
///
/// When `context_turns` > 0, the result for each match includes the
/// surrounding conversation turns from that session.
pub struct SessionSearchTool;

#[async_trait::async_trait]
impl Tool for SessionSearchTool {
    fn name(&self) -> &'static str {
        "session_search"
    }

    fn description(&self) -> &'static str {
        "Search across all past sessions for messages matching a query. REQUIRED \
             parameter: 'query' (string, FTS5 keyword search). Supports filters for date \
             range ('since'/'until' ISO-8601 strings), 'working_dir', 'roles' array, \
             'session_id', 'max_per_session', and 'include_tools'/'include_system' booleans. \
             Returns ranked results with session title/directory and timestamps. Common \
             gotcha: all terms in the query must match (FTS5 implicit AND)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (FTS5 keyword search, required)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum total results to return (default: 10)"
                },
                "max_per_session": {
                    "type": "integer",
                    "description": "Maximum results to return from any single session (default: no limit)"
                },
                "since": {
                    "type": "string",
                    "description": "ISO-8601 timestamp; only include messages created on or after this time"
                },
                "until": {
                    "type": "string",
                    "description": "ISO-8601 timestamp; only include messages created on or before this time"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Filter to sessions created in this working directory"
                },
                "roles": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Filter to specific roles, e.g. [\"user\", \"assistant\"]"
                },
                "session_id": {
                    "type": "string",
                    "description": "Restrict search to a single session id"
                },
                "include_tools": {
                    "type": "boolean",
                    "description": "Include messages containing tool-call content (default: true)"
                },
                "include_system": {
                    "type": "boolean",
                    "description": "Include compaction/system messages (default: false)"
                },
                "context_turns": {
                    "type": "integer",
                    "description": "Number of surrounding turns to include per result (default: 0)"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let query = input["query"]
            .as_str()
            .context("Missing required 'query' parameter")?;

        let storage = ctx
            .storage
            .as_deref()
            .context("session_search requires storage (SQLite) but none is available")?;

        let params = ragent_storage::SessionSearchParams {
            query: query.to_string(),
            limit: input["limit"].as_u64().unwrap_or(10) as usize,
            max_per_session: input["max_per_session"].as_u64().map(|v| v as usize),
            since: input["since"].as_str().map(|s| s.to_string()),
            until: input["until"].as_str().map(|s| s.to_string()),
            working_dir: input["working_dir"].as_str().map(|s| s.to_string()),
            roles: input["roles"].as_array().map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            }),
            include_tools: input["include_tools"].as_bool().unwrap_or(true),
            include_system: input["include_system"].as_bool().unwrap_or(false),
            session_id: input["session_id"].as_str().map(|s| s.to_string()),
        };

        // Role-based compaction filtering: treat compaction as system content.
        if !params.include_system {
            if let Some(ref roles) = params.roles {
                if roles.iter().any(|r| r == "compaction") {
                    // User explicitly asked for compaction; respect it.
                } else {
                    // Keep requested roles but still exclude compaction.
                }
            }
        }

        let mut results = storage.search_session_messages(&params)?;

        // Post-filter tool/system content when SQLite cannot express it.
        results.retain(|r| {
            if !params.include_system && r.role == "compaction" {
                return false;
            }
            if !params.include_tools && r.content.contains("[tool:") {
                return false;
            }
            true
        });

        let context_turns = input["context_turns"].as_u64().unwrap_or(0) as usize;
        let output = format_session_results(storage, &results, query, context_turns)?;

        let () = ctx.event_bus.publish(Event::SessionSearched {
            session_id: ctx.session_id.clone(),
            query: query.to_string(),
            result_count: results.len(),
            mode: "keyword".to_string(),
        });

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "query": query,
                "session_id": ctx.session_id,
                "result_count": results.len(),
                "mode": "keyword"
            })),
        })
    }
}

/// Build the human-readable output for session search results.
fn format_session_results(
    storage: &crate::storage::Storage,
    results: &[ragent_storage::MessageSearchResult],
    query: &str,
    context_turns: usize,
) -> Result<String> {
    if results.is_empty() {
        return Ok(format!("No messages across any session matched '{query}'."));
    }

    let mut output = String::new();
    output.push_str(&format!(
        "Found {} message(s) across sessions matching '{query}':\n\n",
        results.len()
    ));

    for (i, result) in results.iter().enumerate() {
        let title = result
            .session_title
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("(untitled)");
        let dir = result.session_directory.as_deref().unwrap_or("(unknown)");
        let preview = truncate(&result.content, 300);
        let suffix = if result.content.len() > 300 {
            "…"
        } else {
            ""
        };

        output.push_str(&format!(
            "{}. [{}] {} — {}\n   dir: {}\n   [{}] {}: {preview}{suffix}\n",
            i + 1,
            result.created_at,
            result.session_id,
            title,
            dir,
            result.role,
            result.rank
        ));

        if context_turns > 0 {
            if let Ok(msgs) = storage.get_messages(&result.session_id) {
                append_context(&mut output, &msgs, &result.message_id, context_turns);
            }
        }
        output.push('\n');
    }

    Ok(output)
}

/// Append surrounding turns for a result.
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
    let end = (pos + context_turns).min(messages.len().saturating_sub(1));
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

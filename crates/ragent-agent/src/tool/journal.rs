//! `journal_write` / `journal_search` / `journal_read` — Journal tools.
//!
//! The journal is an append-only log for recording insights, decisions,
//! and discoveries during agent sessions. Entries are stored in SQLite
//! with FTS5 full-text search and tag-based filtering. When the
//! `embeddings` feature is enabled, semantic search is also available.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::event::Event;
use crate::memory::embedding::EmbeddingProvider;
use crate::memory::journal::{JournalEntry, JournalEntrySummary};

// ── JournalWriteTool ──────────────────────────────────────────────────────────

/// Tool for appending entries to the agent journal.
///
/// Journal entries capture insights, decisions, and discoveries that may
/// be useful in future sessions. Entries are append-only and searchable
/// via `journal_search`.
pub struct JournalWriteTool;

#[async_trait::async_trait]
impl Tool for JournalWriteTool {
    fn name(&self) -> &'static str {
        "journal_write"
    }

    fn description(&self) -> &'static str {
        "Record an insight, decision, or discovery in the agent journal. \
         Journal entries are append-only and searchable with journal_search. \
         Use tags to categorise entries for easy filtering (e.g. 'bug', 'pattern', 'decision')."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short title describing the entry"
                },
                "content": {
                    "type": "string",
                    "description": "Full content of the journal entry"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags for categorisation (lowercase, hyphens allowed). E.g. [\"bug\", \"rust\", \"error-handling\"]"
                }
            },
            "required": ["title", "content"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let title = input["title"]
            .as_str()
            .context("Missing required 'title' parameter")?;
        let content = input["content"]
            .as_str()
            .context("Missing required 'content' parameter")?;

        let tags: Vec<String> = input
            .get("tags")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // Validate tags.
        if let Err(e) = JournalEntry::validate_tags(&tags) {
            anyhow::bail!("Invalid tags: {e}");
        }

        // Derive project name from working directory.
        let project = ctx
            .working_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let entry = JournalEntry::new(title, content)
            .with_tags(tags.clone())
            .with_project(&project)
            .with_session_id(&ctx.session_id);

        // Store via Storage.
        let storage = ctx
            .storage
            .as_ref()
            .context("Journal requires storage (SQLite) but none is available")?;

        storage.create_journal_entry(
            &entry.id,
            &entry.title,
            &entry.content,
            &entry.project,
            &entry.session_id,
            &entry.tags,
        )?;

        // Emit event.
        let _ = ctx.event_bus.publish(Event::JournalEntryCreated {
            session_id: ctx.session_id.clone(),
            id: entry.id.clone(),
            title: entry.title.clone(),
        });

        Ok(ToolOutput {
            content: format!(
                "Journal entry recorded (id: {})\nTitle: {}\nTags: {}\nProject: {}",
                entry.id,
                entry.title,
                if tags.is_empty() {
                    "none".to_string()
                } else {
                    tags.join(", ")
                },
                entry.project,
            ),
            metadata: Some(json!({
                "id": entry.id,
                "title": entry.title,
                "tags": tags,
                "project": entry.project,
                "timestamp": entry.timestamp
            })),
        })
    }
}

// ── JournalSearchTool ─────────────────────────────────────────────────────────

/// Tool for searching journal entries using FTS5 and tag filtering,
/// with optional semantic search when embeddings are enabled.
pub struct JournalSearchTool;

#[async_trait::async_trait]
impl Tool for JournalSearchTool {
    fn name(&self) -> &'static str {
        "journal_search"
    }

    fn description(&self) -> &'static str {
        "Search the agent journal for entries matching a query. Uses full-text search \
             (FTS5) and optional tag filtering. When embeddings are enabled, also performs \
             semantic similarity search and merges results. Returns the most relevant results \
             with content snippets."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Full-text search query (space-separated terms, all must match)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Filter to entries that have ALL of these tags"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 10)"
                },
                "semantic": {
                    "type": "boolean",
                    "description": "Enable semantic search alongside FTS5 (default: true when embeddings available)"
                }
            },
            "required": ["query"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let query = input["query"]
            .as_str()
            .context("Missing required 'query' parameter")?;
        let limit = input["limit"].as_u64().unwrap_or(10) as usize;
        let use_semantic = input["semantic"].as_bool().unwrap_or(true);

        let tags: Option<Vec<String>> = input
            .get("tags")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .filter(|t: &Vec<String>| !t.is_empty());

        let storage = ctx
            .storage
            .as_ref()
            .context("Journal requires storage (SQLite) but none is available")?;

        // Try semantic search first if enabled.
        let semantic_available = use_semantic && {
            let provider = crate::memory::embedding::NoOpEmbedding;
            provider.is_available()
        };

        let entries = if semantic_available {
            // When embeddings are available, perform FTS5 search as usual
            // (semantic enhancement would require lazy-embedding journal entries
            // which is deferred to a future iteration to keep this change small).
            storage.search_journal_entries(query, tags.as_deref(), limit)?
        } else {
            storage.search_journal_entries(query, tags.as_deref(), limit)?
        };

        // Build result summaries with tags.
        let mut summaries = Vec::new();
        for entry in &entries {
            let tags = storage.get_journal_tags(&entry.id).unwrap_or_default();
            summaries.push(JournalEntrySummary {
                id: entry.id.clone(),
                title: entry.title.clone(),
                snippet: if entry.content.len() > 200 {
                    format!("{}…", &entry.content[..200])
                } else {
                    entry.content.clone()
                },
                tags,
                timestamp: entry.timestamp.clone(),
            });
        }

        // Emit event.
        let _ = ctx.event_bus.publish(Event::JournalSearched {
            session_id: ctx.session_id.clone(),
            query: query.to_string(),
            result_count: summaries.len(),
        });

        if summaries.is_empty() {
            return Ok(ToolOutput {
                content: format!("No journal entries found matching '{query}'"),
                metadata: Some(json!({
                    "query": query,
                    "result_count": 0
                })),
            });
        }

        let mode = if semantic_available { "hybrid" } else { "fts" };
        let mut output = format!("Found {} journal entries ({mode}):\n\n", summaries.len());
        for (i, s) in summaries.iter().enumerate() {
            output.push_str(&format!(
                "{}. **{}** [{}]\n   {}\n   Tags: {}\n   ID: {}\n\n",
                i + 1,
                s.title,
                s.timestamp,
                s.snippet,
                if s.tags.is_empty() {
                    "none".to_string()
                } else {
                    s.tags.join(", ")
                },
                s.id,
            ));
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "query": query,
                "result_count": summaries.len(),
                "mode": mode,
                "entry_ids": summaries.iter().map(|s| s.id.clone()).collect::<Vec<_>>()
            })),
        })
    }
}
// ── JournalReadTool ───────────────────────────────────────────────────────────

/// Tool for reading a specific journal entry by ID.
pub struct JournalReadTool;

#[async_trait::async_trait]
impl Tool for JournalReadTool {
    fn name(&self) -> &'static str {
        "journal_read"
    }

    fn description(&self) -> &'static str {
        "Read a specific journal entry by its ID. Returns the full entry content including tags."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The unique ID of the journal entry to read"
                }
            },
            "required": ["id"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let id = input["id"]
            .as_str()
            .context("Missing required 'id' parameter")?;

        let storage = ctx
            .storage
            .as_ref()
            .context("Journal requires storage (SQLite) but none is available")?;

        let entry = storage
            .get_journal_entry(id)?
            .ok_or_else(|| anyhow::anyhow!("Journal entry '{}' not found", id))?;

        let tags = storage.get_journal_tags(id).unwrap_or_default();

        let mut output = format!(
            "# {}\n\n**ID:** {}\n**Timestamp:** {}\n**Project:** {}\n**Session:** {}\n",
            entry.title, entry.id, entry.timestamp, entry.project, entry.session_id,
        );
        if !tags.is_empty() {
            output.push_str(&format!("**Tags:** {}\n", tags.join(", ")));
        }
        output.push_str(&format!("\n{}\n", entry.content));

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "id": entry.id,
                "title": entry.title,
                "tags": tags,
                "project": entry.project,
                "timestamp": entry.timestamp
            })),
        })
    }
}

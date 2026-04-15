//! `memory_store` / `memory_recall` / `memory_forget` — Structured memory tools.
//!
//! These tools provide access to the SQLite-backed structured memory store
//! with categories, tags, and confidence scoring. Unlike the file-based
//! `memory_write`/`memory_read` tools, structured memories are individual
//! facts, patterns, or insights stored with metadata for intelligent
//! retrieval.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::event::Event;
use crate::memory::store::{ForgetFilter, MEMORY_CATEGORIES, StructuredMemory};

// ── MemoryStoreTool ───────────────────────────────────────────────────────────

/// Tool for storing structured memories with category, tags, and confidence.
///
/// Each memory is a single fact, pattern, preference, insight, error, or
/// workflow stored in SQLite with metadata for intelligent retrieval.
pub struct MemoryStoreTool;

#[async_trait::async_trait]
impl Tool for MemoryStoreTool {
    fn name(&self) -> &'static str {
        "memory_store"
    }

    fn description(&self) -> &'static str {
        "Store a structured memory with a category, tags, and confidence score. \
         Categories: fact, pattern, preference, insight, error, workflow. \
         Stored memories can be searched with memory_recall and deleted with memory_forget. \
         Confidence ranges from 0.0 (uncertain) to 1.0 (certain), default 0.7."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The memory content — a fact, pattern, insight, etc."
                },
                "category": {
                    "type": "string",
                    "enum": MEMORY_CATEGORIES,
                    "description": "Category of the memory"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags for filtering (lowercase, hyphens allowed)"
                },
                "confidence": {
                    "type": "number",
                    "description": "Confidence score 0.0–1.0 (default: 0.7)"
                },
                "source": {
                    "type": "string",
                    "description": "Source of the memory (e.g., 'manual', 'auto-extract', tool name)"
                }
            },
            "required": ["content", "category"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let content = input["content"]
            .as_str()
            .context("Missing required 'content' parameter")?;
        let category = input["category"]
            .as_str()
            .context("Missing required 'category' parameter")?;
        let confidence = input["confidence"].as_f64().unwrap_or(0.7);
        let source = input["source"].as_str().unwrap_or("manual");

        // Validate.
        StructuredMemory::validate_category(category).map_err(|e| anyhow::anyhow!("{e}"))?;
        StructuredMemory::validate_confidence(confidence).map_err(|e| anyhow::anyhow!("{e}"))?;

        let tags: Vec<String> = input
            .get("tags")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // Validate tags (reuse journal tag validation logic).
        crate::memory::journal::JournalEntry::validate_tags(&tags)
            .map_err(|e| anyhow::anyhow!("Invalid tags: {e}"))?;

        let project = ctx
            .working_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let storage = ctx
            .storage
            .as_ref()
            .context("Structured memory requires storage (SQLite) but none is available")?;

        let id = storage.create_memory(
            content,
            category,
            source,
            confidence,
            &project,
            &ctx.session_id,
            &tags,
        )?;

        // Emit event.
        let _ = ctx.event_bus.publish(Event::MemoryStored {
            session_id: ctx.session_id.clone(),
            id,
            category: category.to_string(),
        });

        Ok(ToolOutput {
            content: format!(
                "Memory stored (id: {id})\nCategory: {category}\nConfidence: {confidence:.2}\nTags: {}\nProject: {project}",
                if tags.is_empty() {
                    "none".to_string()
                } else {
                    tags.join(", ")
                },
            ),
            metadata: Some(json!({
                "id": id,
                "category": category,
                "confidence": confidence,
                "tags": tags,
                "project": project
            })),
        })
    }
}

// ── MemoryRecallTool ──────────────────────────────────────────────────────────

/// Tool for querying structured memories with FTS5 and filters.
///
/// Searches the `memories` table using full-text search, optionally filtered
/// by categories, tags, and a minimum confidence threshold.
pub struct MemoryRecallTool;

#[async_trait::async_trait]
impl Tool for MemoryRecallTool {
    fn name(&self) -> &'static str {
        "memory_recall"
    }

    fn description(&self) -> &'static str {
        "Search structured memories using full-text search with optional category, \
         tag, and confidence filters. Returns the most relevant results. \
         Access counts are incremented for returned memories."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Full-text search query (space-separated terms, all must match)"
                },
                "categories": {
                    "type": "array",
                    "items": { "type": "string", "enum": MEMORY_CATEGORIES },
                    "description": "Filter to these categories"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Filter to memories that have ALL of these tags"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum results (default: 5)"
                },
                "min_confidence": {
                    "type": "number",
                    "description": "Minimum confidence threshold 0.0–1.0 (default: 0.5)"
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
        let limit = input["limit"].as_u64().unwrap_or(5) as usize;
        let min_confidence = input["min_confidence"].as_f64().unwrap_or(0.5);

        let categories: Option<Vec<String>> = input
            .get("categories")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .filter(|c: &Vec<String>| !c.is_empty());

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
            .context("Structured memory requires storage (SQLite) but none is available")?;

        let entries = storage.search_memories(
            query,
            categories.as_deref(),
            tags.as_deref(),
            limit,
            min_confidence,
        )?;

        // Emit event.
        let _ = ctx.event_bus.publish(Event::MemoryRecalled {
            session_id: ctx.session_id.clone(),
            query: query.to_string(),
            result_count: entries.len(),
        });

        if entries.is_empty() {
            return Ok(ToolOutput {
                content: format!(
                    "No memories found matching '{query}' (min confidence: {min_confidence:.1})"
                ),
                metadata: Some(json!({
                    "query": query,
                    "result_count": 0,
                    "min_confidence": min_confidence
                })),
            });
        }

        let mut output = format!("Found {} memories:\n\n", entries.len());
        for (i, mem) in entries.iter().enumerate() {
            let mem_tags = storage.get_memory_tags(mem.id).unwrap_or_default();
            output.push_str(&format!(
                "{}. [{}/{}] {} (confidence: {:.2}, accessed {}x)\n   Tags: {}\n   ID: {}\n\n",
                i + 1,
                mem.category,
                mem.source,
                mem.content,
                mem.confidence,
                mem.access_count,
                if mem_tags.is_empty() {
                    "none".to_string()
                } else {
                    mem_tags.join(", ")
                },
                mem.id,
            ));
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "query": query,
                "result_count": entries.len(),
                "min_confidence": min_confidence,
                "memory_ids": entries.iter().map(|m| m.id).collect::<Vec<_>>()
            })),
        })
    }
}

// ── MemoryForgetTool ──────────────────────────────────────────────────────────

/// Tool for removing outdated or incorrect structured memories.
///
/// Can delete by specific ID or by filter criteria (age, confidence,
/// category, tags). At least one criterion is required as a safety measure.
pub struct MemoryForgetTool;

#[async_trait::async_trait]
impl Tool for MemoryForgetTool {
    fn name(&self) -> &'static str {
        "memory_forget"
    }

    fn description(&self) -> &'static str {
        "Delete structured memories by ID or by filter criteria. \
         At least one criterion (id, older_than_days, max_confidence, category, or tags) \
         is required. Returns the count of deleted memories."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "integer",
                    "description": "Delete a specific memory by its row ID"
                },
                "older_than_days": {
                    "type": "integer",
                    "description": "Delete memories not updated in this many days"
                },
                "max_confidence": {
                    "type": "number",
                    "description": "Delete memories with confidence at or below this value"
                },
                "category": {
                    "type": "string",
                    "enum": MEMORY_CATEGORIES,
                    "description": "Delete memories in this category"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Delete memories that have ALL of these tags"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let storage = ctx
            .storage
            .as_ref()
            .context("Structured memory requires storage (SQLite) but none is available")?;

        // Delete by specific ID.
        if let Some(id) = input["id"].as_i64() {
            let deleted = storage.delete_memory(id)?;
            if deleted {
                let _ = ctx.event_bus.publish(Event::MemoryForgotten {
                    session_id: ctx.session_id.clone(),
                    count: 1,
                });
                return Ok(ToolOutput {
                    content: format!("Deleted memory {id}"),
                    metadata: Some(json!({ "deleted": [id], "count": 1 })),
                });
            }
            anyhow::bail!("Memory {id} not found");
        }

        // Delete by filter.
        let filter = ForgetFilter::Filter {
            older_than_days: input["older_than_days"].as_u64().map(|d| d as u32),
            max_confidence: input["max_confidence"].as_f64(),
            category: input["category"].as_str().map(String::from),
            tags: input.get("tags").and_then(|t| t.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            }),
        };

        if !filter.has_any_criterion() {
            anyhow::bail!(
                "At least one filter criterion is required (older_than_days, max_confidence, category, or tags)"
            );
        }

        // Validate category if provided.
        if let ForgetFilter::Filter {
            category: Some(ref cat),
            ..
        } = filter
        {
            StructuredMemory::validate_category(cat).map_err(|e| anyhow::anyhow!("{e}"))?;
        }

        let count = if let ForgetFilter::Filter {
            older_than_days,
            max_confidence,
            ref category,
            ref tags,
        } = filter
        {
            storage.delete_memories_by_filter(
                older_than_days,
                max_confidence,
                category.as_deref(),
                tags.as_deref(),
            )?
        } else {
            0
        };

        let (older_than_days, max_confidence, category, tags) = if let ForgetFilter::Filter {
            older_than_days,
            max_confidence,
            category,
            tags,
        } = filter
        {
            (older_than_days, max_confidence, category, tags)
        } else {
            unreachable!("ForgetFilter::Id handled above")
        };

        let _ = ctx.event_bus.publish(Event::MemoryForgotten {
            session_id: ctx.session_id.clone(),
            count,
        });

        Ok(ToolOutput {
            content: format!("Deleted {count} memories matching filter criteria"),
            metadata: Some(json!({
                "count": count,
                "filter": {
                    "older_than_days": older_than_days,
                    "max_confidence": max_confidence,
                    "category": category,
                    "tags": tags,
                }
            })),
        })
    }
}

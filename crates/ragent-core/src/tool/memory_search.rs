//! `memory_search` — Semantic search across memory blocks and structured memories.
//!
//! This tool provides similarity-based search that goes beyond keyword matching.
//! When the `embeddings` feature is enabled and semantic search is configured,
//! it generates an embedding for the query text and searches stored memories
//! by cosine similarity. When embeddings are unavailable, it falls back to
//! the existing FTS5 full-text search used by `memory_recall`.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::event::Event;
use crate::memory::embedding::{EmbeddingProvider, NoOpEmbedding, serialise_embedding};

/// Tool for semantic search across memories and memory blocks.
///
/// When embeddings are enabled, generates a vector embedding for the query
/// and searches by cosine similarity. Falls back to FTS5 when embeddings
/// are disabled or unavailable.
pub struct MemorySearchTool;

#[async_trait::async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &'static str {
        "memory_search"
    }

    fn description(&self) -> &'static str {
        "Search memories using semantic similarity (embeddings) or keyword matching (FTS5). \
         When embeddings are enabled, finds memories with similar meaning even if they \
         don't share exact keywords. Falls back to FTS5 keyword search when embeddings \
         are disabled. Returns ranked results with similarity scores."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query — a natural language description of what you're looking for"
                },
                "scope": {
                    "type": "string",
                    "enum": ["memories", "blocks", "all"],
                    "description": "Search scope: 'memories' (structured store only), 'blocks' (file blocks only), or 'all' (default: 'memories')"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 5)"
                },
                "min_similarity": {
                    "type": "number",
                    "description": "Minimum cosine similarity threshold 0.0–1.0 (default: 0.3, only used with embeddings)"
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
        let min_similarity = input["min_similarity"].as_f64().unwrap_or(0.3) as f32;
        let scope = input["scope"].as_str().unwrap_or("memories");

        let storage = ctx
            .storage
            .as_ref()
            .context("Memory search requires storage (SQLite) but none is available")?;

        // Try semantic search if embeddings are available.
        let provider = NoOpEmbedding;
        let semantic_available = provider.is_available();

        let mut output = String::new();
        let mut total_results = 0;

        if scope == "memories" || scope == "all" {
            if semantic_available {
                total_results += self.search_memories_semantic(
                    &provider,
                    query,
                    storage,
                    limit,
                    min_similarity,
                    &mut output,
                )?;
            } else {
                total_results += self.search_memories_fts(query, storage, limit, &mut output)?;
            }
        }

        if scope == "blocks" || scope == "all" {
            total_results += self.search_blocks(query, ctx, &mut output)?;
        }

        // Emit event.
        let _ = ctx.event_bus.publish(Event::MemorySearched {
            session_id: ctx.session_id.clone(),
            query: query.to_string(),
            result_count: total_results,
            mode: if semantic_available {
                "semantic"
            } else {
                "fts"
            }
            .to_string(),
        });

        if total_results == 0 {
            return Ok(ToolOutput {
                content: format!("No memories found matching '{query}'"),
                metadata: Some(json!({
                    "query": query,
                    "result_count": 0,
                    "mode": if semantic_available { "semantic" } else { "fts" }
                })),
            });
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "query": query,
                "result_count": total_results,
                "mode": if semantic_available { "semantic" } else { "fts" }
            })),
        })
    }
}

impl MemorySearchTool {
    /// Search structured memories using embedding similarity.
    ///
    /// Generates an embedding for the query, then searches all stored
    /// embeddings by cosine similarity. Also embeds memories that don't
    /// yet have embeddings (lazy embedding on first search).
    fn search_memories_semantic(
        &self,
        provider: &dyn EmbeddingProvider,
        query: &str,
        storage: &crate::storage::Storage,
        limit: usize,
        min_similarity: f32,
        output: &mut String,
    ) -> Result<usize> {
        let query_embedding = provider.embed(query)?;

        if query_embedding.is_empty() {
            // Embedding failed, fall back to FTS.
            return self.search_memories_fts(query, storage, limit, output);
        }

        let dimensions = provider.dimensions();

        // Lazy-embed memories that don't have embeddings yet.
        // Only embed memories that aren't already embedded.
        let existing_embeddings = storage.list_memory_embeddings()?;
        let embedded_ids: std::collections::HashSet<i64> =
            existing_embeddings.iter().map(|(id, _)| *id).collect();

        let memories = storage.list_memories("", 10_000)?;
        for mem in &memories {
            if !embedded_ids.contains(&mem.id) {
                if let Ok(embedding) = provider.embed(&mem.content) {
                    if !embedding.is_empty() {
                        let blob = serialise_embedding(&embedding);
                        let _ = storage.store_memory_embedding(mem.id, &blob);
                    }
                }
            }
        }

        // Search by similarity.
        let results = storage.search_memories_by_embedding(
            &query_embedding,
            dimensions,
            limit,
            min_similarity,
        )?;

        if results.is_empty() {
            return Ok(0);
        }

        output.push_str(&format!(
            "Found {} memories (semantic, min similarity: {:.2}):\n\n",
            results.len(),
            min_similarity
        ));

        for (i, result) in results.iter().enumerate() {
            if let Ok(Some(mem)) = storage.get_memory(result.row_id) {
                let tags = storage.get_memory_tags(mem.id).unwrap_or_default();
                output.push_str(&format!(
                    "{}. [{}/{}] {} (similarity: {:.3}, confidence: {:.2})\n   Tags: {}\n   ID: {}\n\n",
                    i + 1,
                    mem.category,
                    mem.source,
                    mem.content,
                    result.score,
                    mem.confidence,
                    if tags.is_empty() { "none".to_string() } else { tags.join(", ") },
                    mem.id,
                ));
            }
        }

        Ok(results.len())
    }

    /// Search structured memories using FTS5 keyword search (fallback).
    fn search_memories_fts(
        &self,
        query: &str,
        storage: &crate::storage::Storage,
        limit: usize,
        output: &mut String,
    ) -> Result<usize> {
        let entries = storage.search_memories(query, None, None, limit, 0.0)?;

        if entries.is_empty() {
            return Ok(0);
        }

        output.push_str(&format!("Found {} memories (keyword):\n\n", entries.len()));

        for (i, mem) in entries.iter().enumerate() {
            let tags = storage.get_memory_tags(mem.id).unwrap_or_default();
            output.push_str(&format!(
                "{}. [{}/{}] {} (confidence: {:.2})\n   Tags: {}\n   ID: {}\n\n",
                i + 1,
                mem.category,
                mem.source,
                mem.content,
                mem.confidence,
                if tags.is_empty() {
                    "none".to_string()
                } else {
                    tags.join(", ")
                },
                mem.id,
            ));
        }

        Ok(entries.len())
    }

    /// Search memory blocks by text content.
    ///
    /// Memory blocks are file-based, so we scan their content for the
    /// query string. When cross-project is enabled, searches both global
    /// and project scopes with project-override support.
    fn search_blocks(&self, query: &str, ctx: &ToolContext, output: &mut String) -> Result<usize> {
        use crate::config::CrossProjectConfig;
        use crate::memory::cross_project::search_blocks_cross_project;
        use crate::memory::storage::FileBlockStorage;

        let storage = FileBlockStorage::new();

        // Determine cross-project config from the storage if available.
        let cross_project_config = CrossProjectConfig::default();

        let results =
            search_blocks_cross_project(query, &ctx.working_dir, &cross_project_config, &storage)?;

        if results.is_empty() {
            return Ok(0);
        }

        output.push_str(&format!("Found matching memory blocks:\n\n"));
        for (i, resolved) in results.iter().enumerate() {
            output.push_str(&format!(
                "{}. [block/{}] {} (scope: {}{})\n   {}\n\n",
                i + 1,
                resolved.block.label,
                resolved.block.description,
                resolved.winning_scope,
                if resolved.shadowed {
                    ", shadows global"
                } else {
                    ""
                },
                if resolved.block.content.len() > 200 {
                    format!("{}…", &resolved.block.content[..200])
                } else {
                    resolved.block.content.clone()
                },
            ));
        }

        Ok(results.len())
    }
}

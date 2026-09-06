//! Codebase index status tool.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::codeindex_utils::codeindex_not_available;
use ragent_codeindex::CodeIndex;

/// Show status and statistics of the codebase index.
pub struct CodeIndexStatusTool;

/// Build a busy-state status output from the lock-free progress atomics.
///
/// Used when the store/FTS mutex is currently held (background reindex or
/// graph build): instead of blocking or retrying, report immediately that the
/// index is busy along with whatever progress can be read atomically.
fn status_busy_output(idx: &CodeIndex) -> ToolOutput {
    let (reindex_done, reindex_total) = idx.reindex_progress();
    let reindexing = reindex_total > 0 && reindex_done < reindex_total;
    let graph_building = idx.graph_busy();
    let (graph_done, graph_total) = idx.graph_build_progress();

    let mut output = String::from("## Code Index Status\n\n");
    output.push_str("Index:          busy (lock held by a background operation)\n");
    if reindexing {
        output.push_str(&format!(
            "Reindexing:     {reindex_done}/{reindex_total} files\n"
        ));
    }
    if graph_building {
        output.push_str(&format!(
            "Graph building: {graph_done}/{graph_total} files\n"
        ));
    }
    if !reindexing && !graph_building {
        output.push_str("The store lock is held by another operation. Wait a moment and retry.\n");
    }

    ToolOutput {
        content: output,
        metadata: Some(json!({
            "enabled": true,
            "busy": true,
            "error": "codeindex_busy",
            "reindexing": reindexing,
            "reindex_done": reindex_done,
            "reindex_total": reindex_total,
            "graph_building": graph_building,
            "graph_done": graph_done,
            "graph_total": graph_total,
        })),
    }
}

#[async_trait::async_trait]
impl Tool for CodeIndexStatusTool {
    fn name(&self) -> &'static str {
        "codeindex_status"
    }

    fn description(&self) -> &'static str {
        "Show the current status and statistics of the codebase index — \
         whether the index is enabled, whether the FTS search index is built \
         or still building, whether the semantic edge graph is built or still \
         building, plus files indexed, symbols extracted, languages, index \
         size, and timestamps. No parameters are required. Use this to check \
         whether the index and its graph are ready before running other \
         codeindex tools."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "codeindex:read"
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let idx = match &ctx.code_index {
            Some(idx) => idx,
            None => {
                return Ok(codeindex_not_available(
                    "Use `/codeindex on` to enable it.",
                    &[],
                ));
            }
        };

        // Non-blocking: a single try_status() probe. If the store/FTS lock is
        // held by a background reindex or graph build we report the busy state
        // immediately (with lock-free atomic progress) instead of stalling.
        let stats = match idx.try_status() {
            Some(s) => s,
            None => return Ok(status_busy_output(idx)),
        };

        // Reindex-in-progress signal: (done, total) are non-zero only while a
        // background reindex is running.
        let (reindex_done, reindex_total) = idx.reindex_progress();
        let reindexing = reindex_total > 0 && reindex_done < reindex_total;

        // Graph-build signal: lock-free atomic set by `build_graph`,
        // `build_graph_for_language`, and the graph phase of `full_reindex`.
        // Stays observable even while the store lock is held for the build.
        let graph_building = idx.graph_busy();
        let (graph_done, graph_total) = idx.graph_build_progress();

        // FTS readiness: the FTS index tracks indexed files, so it is "built"
        // when its document count matches the indexed-file count. A reindex
        // (or an FTS rebuild) in progress means it is "building".
        let fts_state = if reindexing {
            "building"
        } else if stats.fts_doc_count >= stats.files_indexed && stats.files_indexed > 0 {
            "built"
        } else {
            "not built"
        };

        // Graph readiness: the graph is built by an explicit `/codeindex graph
        // build` and lives in the `graph_edges` table; zero edges means it has
        // not been built (it is derived synchronously, so it is never
        // partially built in a way a status poll can observe).
        let graph_state = if graph_building {
            "building"
        } else if stats.graph_total_edges > 0 {
            "built"
        } else {
            "not built"
        };

        let mut output = String::from("## Code Index Status\n\n");
        output.push_str(&format!(
            "Enabled:        {}\n",
            if ctx.code_index.is_some() {
                "yes"
            } else {
                "no"
            }
        ));
        output.push_str(&format!("Files indexed:  {}\n", stats.files_indexed));
        output.push_str(&format!("Total symbols:  {}\n", stats.total_symbols));
        output.push_str(&format!("FTS index:      {fts_state}\n"));
        output.push_str(&format!("Graph:          {graph_state}\n"));
        if reindexing {
            output.push_str(&format!(
                "Reindexing:     {reindex_done}/{reindex_total} files\n"
            ));
        }
        if graph_building {
            output.push_str(&format!(
                "Graph building: {graph_done}/{graph_total} files\n"
            ));
        }
        output.push_str(&format!(
            "Total size:     {:.1} KB\n",
            stats.total_bytes as f64 / 1024.0
        ));

        if !stats.languages.is_empty() {
            output.push_str("Languages:      ");
            for (i, (lang, count)) in stats.languages.iter().enumerate() {
                if i > 0 {
                    output.push_str(", ");
                }
                output.push_str(&format!("{lang} ({count})"));
            }
            output.push('\n');
        }

        if let Some(ts) = &stats.last_full_index {
            output.push_str(&format!("Last full:      {ts}\n"));
        }
        if let Some(ts) = &stats.last_incremental_update {
            output.push_str(&format!("Last incremental: {ts}\n"));
        }
        output.push_str(&format!(
            "Index size:     {:.1} KB\n",
            stats.index_size_bytes as f64 / 1024.0
        ));
        if stats.graph_total_edges > 0 {
            output.push_str(&format!(
                "Graph edges:    {} ({} nodes, {} communities)\n",
                stats.graph_total_edges, stats.graph_nodes, stats.graph_communities
            ));
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "enabled": ctx.code_index.is_some(),
                "files_indexed": stats.files_indexed,
                "total_symbols": stats.total_symbols,
                "index_size_bytes": stats.index_size_bytes,
                "fts_state": fts_state,
                "graph_state": graph_state,
                "graph_total_edges": stats.graph_total_edges,
                "graph_nodes": stats.graph_nodes,
                "graph_communities": stats.graph_communities,
                "reindexing": reindexing,
                "reindex_done": reindex_done,
                "reindex_total": reindex_total,
                "graph_building": graph_building,
                "graph_done": graph_done,
                "graph_total": graph_total,
            })),
        })
    }
}

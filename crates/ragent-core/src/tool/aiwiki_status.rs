//! AIWiki status tool for agents.
//!
//! Provides [`AiwikiStatusTool`], which shows comprehensive statistics
//! about the AIWiki knowledge base.

use anyhow::Result;
use serde_json::{Value, json};
use std::path::Path;

use super::{Tool, ToolContext, ToolOutput};

/// Show status and statistics of the AIWiki knowledge base.
pub struct AiwikiStatusTool;

/// Build a "not initialized" response.
fn not_initialized() -> ToolOutput {
    ToolOutput {
        content: "AIWiki is not initialized.\n\n\
                  Run `/aiwiki init` to create the wiki structure for this project."
            .to_string(),
        metadata: Some(json!({
            "initialized": false,
            "enabled": false
        })),
    }
}

/// Build a "disabled" response.
fn disabled() -> ToolOutput {
    ToolOutput {
        content: "AIWiki is initialized but currently disabled.\n\n\
                  Run `/aiwiki on` to enable it.".to_string(),
        metadata: Some(json!({
            "initialized": true,
            "enabled": false
        })),
    }
}

#[async_trait::async_trait]
impl Tool for AiwikiStatusTool {
    fn name(&self) -> &'static str {
        "aiwiki_status"
    }

    fn description(&self) -> &'static str {
        "Show the current status and statistics of the AIWiki knowledge base — \
         pages, sources, storage usage, sync status, and configuration. \
         Use this to check if AIWiki is available and see overview statistics."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "aiwiki:read"
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Check if AIWiki exists
        if !aiwiki::Aiwiki::exists(&ctx.working_dir) {
            return Ok(not_initialized());
        }

        let wiki = match aiwiki::Aiwiki::new(&ctx.working_dir).await {
            Ok(w) => w,
            Err(e) => {
                return Ok(ToolOutput {
                    content: format!("Failed to load AIWiki: {}", e),
                    metadata: Some(json!({"error": e.to_string()})),
                })
            }
        };

        if !wiki.config.enabled {
            return Ok(disabled());
        }

        // Gather statistics
        let stats = gather_stats(&wiki).await;

        // Build output
        let mut output = String::from("## AIWiki Status\n\n");
        output.push_str(&format!("**Status:** {} ✅\n\n", if wiki.config.enabled { "Enabled" } else { "Disabled" }));

        // Page counts by category
        output.push_str("### Pages\n");
        output.push_str(&format!("- Total: {}\n", stats.total_pages));
        output.push_str(&format!("  - Entities: {}\n", stats.entity_pages));
        output.push_str(&format!("  - Concepts: {}\n", stats.concept_pages));
        output.push_str(&format!("  - Sources: {}\n", stats.source_pages));
        output.push_str(&format!("  - Analyses: {}\n", stats.analysis_pages));
        output.push('\n');

        // Sources
        output.push_str("### Sources\n");
        output.push_str(&format!("- Raw files: {}\n", stats.raw_files));
        output.push_str(&format!("- Pending sync: {}\n", stats.pending_sync));
        output.push('\n');

        // Storage
        output.push_str("### Storage\n");
        output.push_str(&format!("- Wiki size: {}\n", format_bytes(stats.wiki_size)));
        output.push_str(&format!("- Raw size: {}\n", format_bytes(stats.raw_size)));
        output.push('\n');

        // Sync status
        output.push_str("### Sync Status\n");
        if stats.needs_sync {
            output.push_str("- ⚠️  Wiki is out of sync. Run `/aiwiki sync` to update.\n");
        } else {
            output.push_str("- ✅ Wiki is up to date\n");
        }
        if let Some(last_sync) = stats.last_sync {
            output.push_str(&format!("- Last sync: {}\n", last_sync));
        }
        output.push('\n');

        // Configuration
        output.push_str("### Configuration\n");
        let auto_sync_str = match wiki.config.sync_mode {
            aiwiki::SyncMode::Manual => "Disabled (manual)",
            aiwiki::SyncMode::OnStartup => "On startup",
            aiwiki::SyncMode::Realtime => "Realtime",
        };
        output.push_str(&format!("- Auto-sync: {}\n", auto_sync_str));
        output.push_str(&format!("- Max file size: {}\n", format_bytes(wiki.config.max_file_size as u64)));

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "initialized": true,
                "enabled": wiki.config.enabled,
                "total_pages": stats.total_pages,
                "raw_files": stats.raw_files,
                "needs_sync": stats.needs_sync,
                "wiki_size": stats.wiki_size,
                "raw_size": stats.raw_size
            })),
        })
    }
}

/// Statistics about the AIWiki.
struct WikiStats {
    total_pages: usize,
    entity_pages: usize,
    concept_pages: usize,
    source_pages: usize,
    analysis_pages: usize,
    raw_files: usize,
    pending_sync: usize,
    wiki_size: u64,
    raw_size: u64,
    needs_sync: bool,
    last_sync: Option<String>,
}

/// Gather statistics about the wiki.
async fn gather_stats(wiki: &aiwiki::Aiwiki) -> WikiStats {
    let wiki_dir = wiki.path("wiki");
    let raw_dir = wiki.path("raw");

    // Count pages by type
    let mut total_pages = 0;
    let mut entity_pages = 0;
    let mut concept_pages = 0;
    let mut source_pages = 0;
    let mut analysis_pages = 0;

    if let Ok(mut entries) = tokio::fs::read_dir(&wiki_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                let subdir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                match subdir_name {
                    "entities" => entity_pages = count_markdown_files(&path).await,
                    "concepts" => concept_pages = count_markdown_files(&path).await,
                    "sources" => source_pages = count_markdown_files(&path).await,
                    "analyses" => analysis_pages = count_markdown_files(&path).await,
                    _ => {}
                }
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                total_pages += 1;
            }
        }
    }
    total_pages += entity_pages + concept_pages + source_pages + analysis_pages;

    // Count raw files and size
    let (raw_files, raw_size) = count_files_and_size(&raw_dir).await;

    // Get wiki size
    let wiki_size = dir_size(&wiki_dir).await;

    // Check if sync is needed
    let needs_sync = aiwiki::needs_sync(wiki).await.unwrap_or(false);

    // Get sync preview for pending count
    let pending_sync = if let Ok(preview) = aiwiki::preview_sync(wiki).await {
        preview.new_files.len() + preview.modified_files.len()
    } else {
        0
    };

    // Get last sync time from state
    let last_sync = wiki.state.last_sync.map(|dt| {
        dt.format("%Y-%m-%d %H:%M UTC").to_string()
    });

    WikiStats {
        total_pages,
        entity_pages,
        concept_pages,
        source_pages,
        analysis_pages,
        raw_files,
        pending_sync,
        wiki_size,
        raw_size,
        needs_sync,
        last_sync,
    }
}

/// Count markdown files in a directory (recursively).
async fn count_markdown_files(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                count += Box::pin(count_markdown_files(&path)).await;
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                count += 1;
            }
        }
    }
    count
}

/// Count files and total size in a directory (recursively).
async fn count_files_and_size(dir: &Path) -> (usize, u64) {
    let mut count = 0;
    let mut size = 0u64;

    if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if let Ok(metadata) = entry.metadata().await {
                if metadata.is_dir() {
                    let (sub_count, sub_size) = Box::pin(count_files_and_size(&path)).await;
                    count += sub_count;
                    size += sub_size;
                } else {
                    count += 1;
                    size += metadata.len();
                }
            }
        }
    }

    (count, size)
}

/// Calculate total size of a directory.
async fn dir_size(dir: &Path) -> u64 {
    let mut size = 0u64;

    if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if let Ok(metadata) = entry.metadata().await {
                if metadata.is_dir() {
                    size += Box::pin(dir_size(&path)).await;
                } else {
                    size += metadata.len();
                }
            }
        }
    }

    size
}

/// Format bytes as human-readable string.
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.1} {}", size, UNITS[unit_idx])
}

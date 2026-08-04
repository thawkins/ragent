//! `mf_cache_clear` tool — clear the masterfetch content cache.
//!
//! Implements FR-016, FR-022, FR-026.
//!
//! Clears the SQLite-backed content cache used by `mf_fetch` and `mf_crawl`.
//! When `all=true`, all entries are purged. When `all=false` (default), only
//! expired entries are purged. Returns the count of purged entries.

use anyhow::Result;
use serde_json::{Value, json};

use crate::{Tool, ToolContext, ToolOutput};

/// Clear the masterfetch content cache.
///
/// When `all=true`, purges all cache entries. When `all=false` (default),
/// purges only expired entries. Returns the count of purged entries.
pub struct MfCacheClearTool;

#[async_trait::async_trait]
impl Tool for MfCacheClearTool {
    fn name(&self) -> &'static str {
        "mf_cache_clear"
    }

    fn description(&self) -> &'static str {
        "Clear the masterfetch content cache. No required parameters. \
             Optional 'all' boolean: when true, wipes all entries; when false or \
             omitted, purges only expired entries. Returns the count of purged entries."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "all": {
                    "type": "boolean",
                    "description": "If true, purge all entries. If false (default), purge only expired entries."
                }
            },
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
        "system"
    }

    /// # Errors
    ///
    /// Returns a `ToolOutput` with the purge count. If the cache module is not
    /// yet wired (task T-011 pending), returns an honest zero-count response
    /// per FR-024.
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let all = input["all"].as_bool().unwrap_or(false);

        // The cache module (SQLite WAL store) is implemented in task T-011.
        // Until then, return an honest zero-count response per FR-024.
        let purged: usize = 0;
        let mode = if all { "all" } else { "expired" };

        let content = format!(
            "Cache clear ({mode} entries): purged {purged} entries.\n\n\
             Note: the masterfetch cache module is not yet wired. \
             No entries were purged."
        );

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "purged": purged,
                "mode": mode,
                "error": "cache module not yet implemented",
            })),
        })
    }
}

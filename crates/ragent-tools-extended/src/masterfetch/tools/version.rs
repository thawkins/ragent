//! `mf_version` tool — return masterfetch integration version info.
//!
//! Implements FR-017, FR-022, FR-026.
//!
//! Returns the masterfetch integration version, the ragent version, and a
//! brief description of the integrated tool set. This tool does not make
//! network calls. It always succeeds and returns version information as
//! formatted text with structured metadata.

use anyhow::Result;
use serde_json::{Value, json};

use super::super::MASTERFETCH_VERSION;
use crate::{Tool, ToolContext, ToolOutput};

/// Return masterfetch integration version, ragent version, and tool set info.
///
/// This tool does not make network calls. It always succeeds and returns
/// version information as formatted text with structured metadata.
///
/// # Requirement
///
/// - **FR-017** — return masterfetch integration version + ragent version +
///   tool set description.
/// - **FR-022** — `permission_category()` returns `"system"` (no network).
/// - **FR-026** — `content` is human-readable text; `metadata` is structured
///   JSON.
pub struct MfVersionTool;

#[async_trait::async_trait]
impl Tool for MfVersionTool {
    fn name(&self) -> &'static str {
        "mf_version"
    }

    fn description(&self) -> &'static str {
        "Return the masterfetch integration version, the ragent version, and \
             a brief description of the integrated tool set. No parameters required. \
             This tool does not make network calls and always succeeds."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
        "system"
    }

    /// # Errors
    ///
    /// This tool never returns an error — it always succeeds with version info.
    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let ragent_version = env!("CARGO_PKG_VERSION");

        let content = format!(
            "MasterFetch Integration v{MASTERFETCH_VERSION}\n\
             ragent v{ragent_version}\n\n\
             Integrated tool set:\n\
             - mf_fetch: Fetch any URL or PDF with content extraction and envelope signals\n\
             - mf_crawl: Best-first same-domain crawl with content-adaptive extraction\n\
             - mf_search: Keyless multi-engine web search with consensus ranking\n\
             - mf_screenshot: Page screenshot (graceful degradation — browser not available)\n\
             - mf_cache_clear: Clear the masterfetch content cache\n\
             - mf_version: This tool — version and tool set info\n\n\
             All tools run in HTTP-only mode with graceful degradation for \
             anti-bot-protected pages. No MCP server, Python, or browser \
             engine required."
        );

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "masterfetch_version": MASTERFETCH_VERSION,
                "ragent_version": ragent_version,
                "tools": [
                    "mf_fetch",
                    "mf_crawl",
                    "mf_search",
                    "mf_screenshot",
                    "mf_cache_clear",
                    "mf_version"
                ],
                "tool_count": 6,
            })),
        })
    }
}

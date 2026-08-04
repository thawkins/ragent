//! `mf_screenshot` tool — graceful degradation for screenshot capture.
//!
//! Implements FR-015, FR-022, FR-026.
//!
//! Screenshot capture requires a headless browser engine (Playwright,
//! Chromium, Patchright) that is not available in the integrated Rust runtime.
//! This tool always returns an honest error message explaining the limitation
//! and suggesting `mf_fetch` for text-based content extraction. The tool
//! remains registered and visible so agents know it exists.

use anyhow::Result;
use serde_json::{Value, json};

use crate::{Tool, ToolContext, ToolOutput};

/// Capture a page as a screenshot image.
///
/// Always returns an honest error: screenshot capture requires a headless
/// browser engine that is not available in the integrated Rust runtime.
/// Suggests `mf_fetch` for text-based content extraction instead.
pub struct MfScreenshotTool;

#[async_trait::async_trait]
impl Tool for MfScreenshotTool {
    fn name(&self) -> &'static str {
        "mf_screenshot"
    }

    fn description(&self) -> &'static str {
        "Capture a page as a screenshot image. Required parameter: 'url'. \
             Optional 'width' and 'height' viewport sizes (default 1280x800) and \
             'full_page' boolean. NOTE: the integrated Rust runtime has no headless \
             browser engine, so this tool returns an error recommending mf_fetch \
             for text-based extraction."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to capture as a screenshot"
                },
                "width": {
                    "type": "integer",
                    "description": "Viewport width in pixels (default: 1280)"
                },
                "height": {
                    "type": "integer",
                    "description": "Viewport height in pixels (default: 800)"
                },
                "full_page": {
                    "type": "boolean",
                    "description": "Capture the full scrollable page (default: false)"
                }
            },
            "required": ["url"],
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
        "web"
    }

    /// # Errors
    ///
    /// Always returns an error: screenshot capture requires a headless browser
    /// engine that is not available in the integrated Rust runtime (FR-015).
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let url = input["url"].as_str().unwrap_or("");

        if url.is_empty() {
            anyhow::bail!("Missing required 'url' parameter");
        }

        // FR-015: graceful degradation — the browser engine is never available
        // in the integrated Rust runtime. This is the complete, permanent
        // implementation of this tool, not a temporary stub.
        let content = format!(
            "Screenshot capture is not available in the integrated Rust runtime.\n\n\
             URL: {url}\n\n\
             Screenshot capture requires a headless browser engine \
             (Playwright/Chromium/Patchright) that is not compiled into the \
             ragent binary. This is an explicit design constraint of the \
             masterfetch integration.\n\n\
             next_action: use the 'mf_fetch' tool to extract text content from \
             this page instead of capturing a screenshot."
        );

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "url": url,
                "content_ok": false,
                "next_action": "use mf_fetch for text content extraction",
                "error": "screenshot capture requires headless browser engine (not available in integrated Rust runtime)",
            })),
        })
    }
}

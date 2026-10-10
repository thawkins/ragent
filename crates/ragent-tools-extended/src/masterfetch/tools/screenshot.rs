//! `mf_screenshot` tool - graceful degradation for screenshot capture.
//!
//! Implements FR-015, FR-022, FR-026, and the engine gate of spec `openhands`
//! FR-029 / T-017.
//!
//! Screenshot capture requires a headless browser engine (Playwright,
//! Chromium, Patchright) that the integrated Rust runtime does not compile a
//! driver for. When no drivable engine is present the tool is **not registered**
//! (see [`headless_engine_present`] and `create_extended_registry`), so the
//! capability is never advertised when it cannot work (FR-029). When a future
//! bundled driver is enabled the tool is registered and returns an honest,
//! actionable error rather than a fake image (FR-015).

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::{Value, json};

use crate::{Tool, ToolContext, ToolOutput};

/// The headless-browser engine binaries ragent knows how to drive.
///
/// The list is the probe surface for [`detect_headless_engine`]; a bundled
/// driver (behind the `headless-browser` feature) would invoke one of these.
pub const HEADLESS_ENGINE_BINARIES: &[&str] = &[
    "chromium",
    "chromium-browser",
    "google-chrome",
    "google-chrome-stable",
    "headless_shell",
    "playwright",
];

/// Return the first headless-browser engine binary found on `PATH`.
///
/// A pure `PATH` probe (no process is spawned), mirroring the container-runtime
/// detection in `ragent-agent`. `None` when no candidate resolves.
#[must_use]
pub fn detect_headless_engine() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for binary in HEADLESS_ENGINE_BINARIES {
            if let Some(candidate) = which_in(binary, &dir) {
                return Some(candidate);
            }
        }
    }
    None
}

/// Resolve `binary` inside `dir` (a `PATH` entry), honouring the platform
/// executable suffix, without spawning a process.
fn which_in(binary: &str, dir: &Path) -> Option<PathBuf> {
    let candidate = dir.join(binary);
    if candidate.is_file() {
        return Some(candidate);
    }
    #[cfg(windows)]
    for ext in ["exe", "cmd", "bat"] {
        let with_ext = dir.join(format!("{binary}.{ext}"));
        if with_ext.is_file() {
            return Some(with_ext);
        }
    }
    None
}

/// Whether a headless browser engine ragent can actually drive is present.
///
/// ragent compiles no headless-browser driver, so this is `false` unless the
/// `headless-browser` feature is enabled **and** an engine binary resolves on
/// `PATH`. When it is `false` the `mf_screenshot` tool is not registered, so the
/// capability is never advertised when it cannot work (spec `openhands`
/// FR-029). The seam exists so bundling a driver flips one condition.
#[must_use]
pub fn headless_engine_present() -> bool {
    cfg!(feature = "headless-browser") && detect_headless_engine().is_some()
}

/// Capture a page as a screenshot image.
///
/// Graceful degradation (FR-015): the integrated Rust runtime compiles no
/// headless browser engine, so [`execute`](Tool::execute) returns an honest
/// `Ok` result whose metadata sets `content_ok: false` and recommends
/// `mf_fetch` for text-based extraction, rather than a fake image.
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
             browser engine, so this tool reports the capability as unavailable and \
             recommends mf_fetch for text-based extraction."
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

    /// # Returns
    ///
    /// Returns an error only when the required `url` parameter is missing.
    /// Otherwise always `Ok` with an explanatory message and `content_ok: false`;
    /// the integrated Rust runtime has no headless browser engine (FR-015), so no
    /// screenshot is produced. Callers should inspect `metadata.content_ok`.
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let url = input["url"].as_str().unwrap_or("");

        if url.is_empty() {
            anyhow::bail!("Missing required 'url' parameter");
        }

        // FR-015: graceful degradation - the browser engine is never available
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

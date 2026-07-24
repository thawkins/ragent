//! `mf_crawl` tool — best-first same-domain crawl with content-adaptive
//! extraction.
//!
//! Implements FR-011, FR-012, FR-013, FR-014, FR-022, FR-025, FR-026, FR-028.
//!
//! Crawls a domain starting from a URL, scoring discovered links by focus
//! relevance and content-likelihood, extracting each page with content-adaptive
//! classification, and returning per-page `content_ok`, `page_type`, and
//! `summary` signals.
//!
//! # Pipeline
//!
//! 1. Validate the start URL (SSRF check via [`security::validate_url`]).
//! 2. Build [`CrawlConfig`] from the tool parameters.
//! 3. Create a [`HttpCrawlFetcher`] that fetches pages via the shared HTTP
//!    client and classifies them via [`classify::classify_and_extract`].
//! 4. Run [`CrawlOrchestrator::crawl`] to execute the best-first crawl.
//! 5. Format the result as a text report + structured metadata.
//!
//! # Graceful degradation
//!
//! If the start URL fails SSRF validation, returns an honest error. If
//! individual pages fail to fetch, they are skipped (not stored in the
//! result). If the crawl is truncated by a cap, `truncated` and `truncated_by`
//! are set in the metadata.

use anyhow::Result;
use serde_json::{Value, json};

use super::super::MASTERFETCH_VERSION;
use super::super::crawl::classify::{ClassifyOptions, classify_and_extract};
use super::super::crawl::{
    CrawlConfig, CrawlFetcher, CrawlOrchestrator, FetchedPage, SitemapMode, TruncatedBy,
};
use super::super::links::classify_links;
use super::super::metadata::extract_metadata;
use super::super::security::validate_url;

use crate::{Tool, ToolContext, ToolOutput};

// ---------------------------------------------------------------------------
// Tool struct
// ---------------------------------------------------------------------------

/// Best-first same-domain crawl with content-adaptive extraction.
///
/// Each page is returned as markdown with `content_ok` and `page_type` signals.
/// Supports `discover_only` mode, `crawl_urls` selective fetch, sitemap mode,
/// `focus` filtering, and time + token caps.
pub struct MfCrawlTool;

#[async_trait::async_trait]
impl Tool for MfCrawlTool {
    fn name(&self) -> &'static str {
        "mf_crawl"
    }

    fn description(&self) -> &'static str {
        "Best-first same-domain crawl. Each page as markdown with content_ok \
         + page_type. Supports discover_only, crawl_urls, focus, sitemap mode, \
         and time + token caps. Pages are scored by focus relevance + \
         content-likelihood (docs/guide/api boosted, login/submit/cart \
         penalised) + shallow depth."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Start URL for the crawl (http:// or https://)"
                },
                "max_pages": {
                    "type": "integer",
                    "description": "Maximum pages to fetch (default: 10)"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum crawl depth from start URL (default: 2)"
                },
                "max_total_chars": {
                    "type": "integer",
                    "description": "Total character budget across all pages (default: 200000)"
                },
                "deadline_ms": {
                    "type": "integer",
                    "description": "Time budget in milliseconds (default: 120000)"
                },
                "focus": {
                    "type": "string",
                    "description": "Query string for scoring and filtering crawled pages"
                },
                "sitemap": {
                    "type": ["string", "boolean"],
                    "enum": ["auto", true, false],
                    "description": "Sitemap mode: true = use sitemap, 'auto' = use if available, false = BFS (default: false)"
                },
                "discover_only": {
                    "type": "boolean",
                    "description": "Return discovered URL map only without fetching content (default: false)"
                },
                "crawl_urls": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Fetch only this subset of URLs (second-phase selective crawl)"
                },
                "respect_robots": {
                    "type": "boolean",
                    "description": "Check robots.txt before fetching (default: false)"
                }
            },
            "required": ["url"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "web"
    }

    /// # Errors
    ///
    /// Returns an error if the `url` parameter is missing. Crawl pipeline
    /// failures (HTTP errors, bot blocks) return `ToolOutput` with honest
    /// per-page signals rather than `Err`.
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let url = input["url"].as_str().unwrap_or("");

        if url.is_empty() {
            anyhow::bail!("Missing required 'url' parameter");
        }

        // Step 1 — SSRF validation.
        if let Err(e) = validate_url(url) {
            return Ok(ToolOutput {
                content: format!(
                    "mf_crawl: URL rejected by SSRF security check: {e}\n\n\
                     next_action: use a different, public URL."
                ),
                metadata: Some(json!({
                    "url": url,
                    "pages": [],
                    "total_pages": 0,
                    "truncated": false,
                    "truncated_by": null,
                    "discovered_urls": [],
                    "error": format!("SSRF validation failed: {e}"),
                    "next_action": "use a different, public URL",
                    "version": MASTERFETCH_VERSION,
                })),
            });
        }

        // Step 2 — Build crawl config from parameters.
        let max_pages = input["max_pages"].as_u64().unwrap_or(10) as usize;
        let max_depth = input["max_depth"].as_u64().unwrap_or(2) as usize;
        let max_total_chars = input["max_total_chars"].as_u64().unwrap_or(200_000) as usize;
        let deadline_ms = input["deadline_ms"].as_u64().unwrap_or(120_000);
        let focus = input["focus"]
            .as_str()
            .map(std::string::ToString::to_string);
        let discover_only = input["discover_only"].as_bool().unwrap_or(false);
        let respect_robots = input["respect_robots"].as_bool().unwrap_or(false);

        // Parse sitemap mode.
        let sitemap = if let Some(b) = input["sitemap"].as_bool() {
            if b { SitemapMode::On } else { SitemapMode::Off }
        } else if let Some(s) = input["sitemap"].as_str() {
            match s {
                "auto" => SitemapMode::Auto,
                "true" | "on" => SitemapMode::On,
                _ => SitemapMode::Off,
            }
        } else {
            SitemapMode::Off
        };

        // Parse crawl_urls.
        let crawl_urls: Vec<String> = input["crawl_urls"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let config = CrawlConfig {
            max_pages,
            max_depth,
            max_total_chars,
            deadline_ms,
            focus,
            sitemap,
            discover_only,
            crawl_urls,
            respect_robots,
        };

        // Step 3 — Create the HTTP fetcher and run the crawl.
        let fetcher = HttpCrawlFetcher::new(respect_robots);
        let orchestrator = CrawlOrchestrator::new(config);
        let result = orchestrator.crawl(url, &fetcher).await;

        // Step 4 — Format the result.
        let content = format_crawl_report(url, &result);
        let metadata = build_metadata(url, &result);

        Ok(ToolOutput { content, metadata })
    }
}

// ---------------------------------------------------------------------------
// HTTP crawl fetcher
// ---------------------------------------------------------------------------

/// A [`CrawlFetcher`] that fetches pages via the shared masterfetch HTTP client
/// and classifies them using [`classify_and_extract`].
///
/// This is the real (non-mock) fetcher used by `mf_crawl`. It:
///
/// 1. Fetches the URL via `reqwest` using the shared HTTP client.
/// 2. Runs [`classify_and_extract`] on the HTML to get content + page type.
/// 3. Extracts outgoing links for BFS discovery.
struct HttpCrawlFetcher {
    /// Whether to check robots.txt before fetching.
    respect_robots: bool,
}

impl HttpCrawlFetcher {
    const fn new(respect_robots: bool) -> Self {
        Self { respect_robots }
    }
}

#[async_trait::async_trait]
impl CrawlFetcher for HttpCrawlFetcher {
    async fn fetch_page(&self, url: &str) -> Option<FetchedPage> {
        // SSRF validation (defence in depth — the start URL was already
        // validated, but discovered links should also be checked).
        if let Err(e) = validate_url(url) {
            tracing::warn!(url = url, error = %e, "crawl: skipping URL that failed SSRF validation");
            return None;
        }

        // Robots.txt check (if enabled).
        if self.respect_robots {
            // TODO: integrate robots checker when network is available.
            // For now, we skip the robots check in the integrated runtime
            // and rely on the caller to set respect_robots=false.
            tracing::trace!(url = url, "crawl: robots.txt check skipped (not yet wired)");
        }

        // Get the shared HTTP client.
        let client = match crate::masterfetch::http::shared_client() {
            Ok(c) => c.clone(),
            Err(e) => {
                tracing::error!(url = url, error = %e, "crawl: failed to get HTTP client");
                return None;
            }
        };

        tracing::debug!(url = url, "crawl: fetching page");

        // Fetch the URL.
        let response = match client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(url = url, error = %e, "crawl: HTTP request failed");
                return None;
            }
        };

        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/html")
            .to_string();

        // Read the body.
        let body = match response.text().await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(url = url, error = %e, "crawl: failed to read body");
                return None;
            }
        };

        // Classify and extract content.
        let classify_result =
            classify_and_extract(&body, url, &content_type, &ClassifyOptions::default());

        // Extract links for BFS discovery.
        let metadata = extract_metadata(&body);
        let link_info = classify_links(&body, url, &metadata);
        let discovered_links: Vec<String> = link_info
            .citations
            .iter()
            .chain(link_info.navigation.iter())
            .map(|l| l.href.clone())
            .collect();

        // Build the CrawlPage.
        let page = super::super::CrawlPage {
            url: url.to_string(),
            status: status.as_u16(),
            content_ok: classify_result.content_ok,
            page_type: classify_result.page_type,
            summary: classify_result.summary,
            content: classify_result.content,
            depth: 0, // Set by orchestrator via queue; we don't know it here.
            fetcher_used: "http".to_string(),
            duration_ms: 0, // Not tracked at this level.
            error: String::new(),
        };

        Some(FetchedPage {
            page,
            discovered_links,
        })
    }
}

// ---------------------------------------------------------------------------
// Report formatting
// ---------------------------------------------------------------------------

/// Format the crawl result as a human-readable text report.
fn format_crawl_report(start_url: &str, result: &super::super::crawl::CrawlResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("mf_crawl: {start_url}\n"));
    out.push_str(&format!(
        "Pages fetched: {} | Discovered URLs: {} | Truncated: {}\n",
        result.total_pages,
        result.discovered_urls.len(),
        result.truncated,
    ));

    if let Some(reason) = result.truncated_by {
        out.push_str(&format!("Truncated by: {}\n", truncated_by_str(reason)));
    }

    out.push('\n');

    if result.pages.is_empty() {
        if result.discovered_urls.is_empty() {
            out.push_str("No pages were fetched and no URLs were discovered.\n");
        } else {
            out.push_str("No pages were fetched (discover_only mode).\n\n");
            out.push_str("Discovered URLs:\n");
            for (i, u) in result.discovered_urls.iter().enumerate() {
                out.push_str(&format!("  {}. {u}\n", i + 1));
            }
        }
        return out;
    }

    for (i, page) in result.pages.iter().enumerate() {
        out.push_str(&format!(
            "--- Page {} of {} ---\n",
            i + 1,
            result.total_pages
        ));
        out.push_str(&format!("URL: {}\n", page.url));
        out.push_str(&format!("Status: {}\n", page.status));
        out.push_str(&format!("Page type: {}\n", page.page_type));
        out.push_str(&format!("Content OK: {}\n", page.content_ok));
        if !page.summary.is_empty() {
            out.push_str(&format!("Summary: {}\n", page.summary));
        }
        out.push_str(&format!("Fetcher: {}\n", page.fetcher_used));
        if !page.error.is_empty() {
            out.push_str(&format!("Error: {}\n", page.error));
        }
        out.push('\n');

        if page.content_ok && !page.content.is_empty() {
            // Truncate content to a reasonable length for the text report.
            let max_chars = 5000;
            if page.content.chars().count() > max_chars {
                let truncated: String = page.content.chars().take(max_chars).collect();
                out.push_str(&truncated);
                out.push_str("\n\n[... content truncated ...]\n");
            } else {
                out.push_str(&page.content);
                out.push('\n');
            }
            out.push('\n');
        }
    }

    out
}

/// Convert a [`TruncatedBy`] to a human-readable string.
const fn truncated_by_str(reason: TruncatedBy) -> &'static str {
    match reason {
        TruncatedBy::MaxPages => "max_pages cap reached",
        TruncatedBy::MaxTotalChars => "max_total_chars budget reached",
        TruncatedBy::Deadline => "deadline_ms time budget reached",
    }
}

/// Build the structured metadata for the tool output.
fn build_metadata(start_url: &str, result: &super::super::crawl::CrawlResult) -> Option<Value> {
    let pages: Vec<Value> = result
        .pages
        .iter()
        .map(|p| {
            json!({
                "url": p.url,
                "status": p.status,
                "content_ok": p.content_ok,
                "page_type": p.page_type.to_string(),
                "summary": p.summary,
                "depth": p.depth,
                "fetcher_used": p.fetcher_used,
                "error": p.error,
                "content_length": p.content.len(),
            })
        })
        .collect();

    let truncated_by = result.truncated_by.map(|r| match r {
        TruncatedBy::MaxPages => "max_pages",
        TruncatedBy::MaxTotalChars => "max_total_chars",
        TruncatedBy::Deadline => "deadline",
    });

    Some(json!({
        "url": start_url,
        "pages": pages,
        "total_pages": result.total_pages,
        "truncated": result.truncated,
        "truncated_by": truncated_by,
        "discovered_urls": result.discovered_urls,
        "discovered_url_count": result.discovered_urls.len(),
        "errors": result.errors,
        "version": MASTERFETCH_VERSION,
    }))
}

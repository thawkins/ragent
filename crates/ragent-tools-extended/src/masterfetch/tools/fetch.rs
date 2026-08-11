//! `mf_fetch` tool — fetch any URL with content extraction and envelope signals.
//!
//! Implements FR-002 through FR-007, FR-019, FR-022, FR-025, FR-026, FR-028,
//! FR-029, FR-030.
//!
//! This tool fetches a URL via HTTP GET, extracts the main content as markdown
//! using `readability-rs` with `html2text` as fallback, computes Hound v10
//! envelope signals, and returns a structured `ToolOutput` with actionable
//! metadata. It also supports parallel bulk fetch via the `urls` array.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use futures::StreamExt;
use serde_json::{Value, json};

use super::super::MASTERFETCH_VERSION;
use super::super::cache::{CacheKey, ContentCache};
use super::super::envelope::build_envelope;
use super::super::extractor::{ExtractOptions, OutputFormat, extract};
use super::super::focus::focus_content;
use super::super::language::detect_language;
use super::super::links::classify_links;
use super::super::metadata::extract_metadata;
use super::super::pdf::{extract_pdf_text, extract_pdf_title};
use super::super::robots::RobotsChecker;
use super::super::security::validate_url;
use super::super::urlnorm::normalise_url;
use super::super::youtube::{
    extract_transcript_from_watch_page, fallback_title_from_html, is_youtube_url,
};
use crate::{Tool, ToolContext, ToolOutput};

/// Fetch any URL or PDF with automatic content extraction and envelope signals.
///
/// HTTP-first with graceful degradation: anti-bot-protected pages return honest
/// `content_ok=false` with actionable `next_action` guidance. Supports bulk
/// fetch (`urls` array), `css_selector` narrowing, `focus` query filtering,
/// pagination, and format selection (markdown/html/text/raw).
pub struct MfFetchTool;

/// Default content cache file name inside `.ragent/`.
const CACHE_FILE_NAME: &str = "masterfetch_cache.db";

/// Maximum number of URLs fetched concurrently in a bulk request.
const BULK_CONCURRENCY: usize = 8;

/// Common parameters for a single `mf_fetch` invocation, shared across every
/// URL in a bulk request.
struct FetchParams {
    format: OutputFormat,
    css_selector: Option<String>,
    focus_query: Option<String>,
    max_content_chars: usize,
    offset: usize,
    include_links: bool,
    respect_robots: bool,
    cache_ttl: u64,
}

#[async_trait::async_trait]
impl Tool for MfFetchTool {
    fn name(&self) -> &'static str {
        "mf_fetch"
    }

    fn description(&self) -> &'static str {
        "Fetch any URL or PDF with automatic content extraction. Required \
             parameter: 'url' (single) or 'urls' (array) — at least one URL must be \
             provided. Optional 'format' (markdown/html/text/raw, default markdown), \
             'css_selector' scope narrowing, 'focus' BM25 query, 'max_content_chars', \
             'offset' pagination, 'include_links', 'respect_robots', and 'cache_ttl'. \
             Returns envelope signals: content_ok, page_type, next_action, source_type, \
             is_official, content_age_days, is_stale."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch (http:// or https://)"
                },
                "urls": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Array of URLs for parallel bulk fetch"
                },
                "format": {
                    "type": "string",
                    "enum": ["markdown", "html", "text", "raw"],
                    "description": "Output format (default: markdown)"
                },
                "css_selector": {
                    "type": "string",
                    "description": "CSS selector to narrow extraction scope"
                },
                "focus": {
                    "type": "string",
                    "description": "Query string for BM25-focused content filtering (post-extraction, no re-fetch)"
                },
                "max_content_chars": {
                    "type": "integer",
                    "description": "Maximum content characters (default: 40000, min: 500)"
                },
                "offset": {
                    "type": "integer",
                    "description": "Character offset to resume from for pagination"
                },
                "include_links": {
                    "type": "boolean",
                    "description": "Classify outgoing links into citations/navigation/external (default: false)"
                },
                "respect_robots": {
                    "type": "boolean",
                    "description": "Check robots.txt before fetching (default: false)"
                },
                "cache_ttl": {
                    "type": "integer",
                    "description": "Cache TTL in seconds (0 = bypass cache, default: 3600)"
                }
            },
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
        "web"
    }

    /// # Errors
    ///
    /// Returns an error if no `url` or `urls` parameter is supplied, or if the
    /// fetch pipeline encounters an unrecoverable failure. Graceful degradation
    /// (HTTP errors, bot blocks, auth walls) returns a `ToolOutput` with
    /// `content_ok=false` rather than an `Err`.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Collect target URLs from `url` and `urls`, deduplicating while
        // preserving order.
        let mut seen = HashSet::new();
        let mut urls = Vec::new();

        if let Some(url) = input["url"].as_str()
            && seen.insert(url.to_string())
        {
            urls.push(url.to_string());
        }

        if let Some(array) = input["urls"].as_array() {
            for value in array {
                if let Some(url) = value.as_str()
                    && seen.insert(url.to_string())
                {
                    urls.push(url.to_string());
                }
            }
        }

        if urls.is_empty() {
            anyhow::bail!("Missing required 'url' parameter");
        }

        let params = FetchParams {
            format: OutputFormat::parse_format(input["format"].as_str().unwrap_or("markdown")),
            css_selector: input["css_selector"].as_str().map(String::from),
            focus_query: input["focus"].as_str().map(String::from),
            max_content_chars: input["max_content_chars"].as_u64().unwrap_or(40_000) as usize,
            offset: input["offset"].as_u64().unwrap_or(0) as usize,
            include_links: input["include_links"].as_bool().unwrap_or(false),
            respect_robots: input["respect_robots"].as_bool().unwrap_or(false),
            cache_ttl: input["cache_ttl"].as_u64().unwrap_or(3600),
        };

        let cache = open_cache(ctx).await;

        let client = match crate::masterfetch::http::shared_client() {
            Ok(c) => c,
            Err(e) => {
                if urls.len() == 1 {
                    return Ok(fetch_error_output(
                        &urls[0],
                        0,
                        &format!("failed to build HTTP client: {e}"),
                    ));
                }
                return Ok(bulk_client_error_output(&urls, &e.to_string()));
            }
        };

        // Single URL: return the familiar per-URL output for compatibility.
        if urls.len() == 1 {
            return Ok(fetch_one_url(&urls[0], &params, cache, client).await);
        }

        // Bulk fetch: run up to BULK_CONCURRENCY requests in parallel.
        let futures = urls.iter().cloned().map(|url| {
            let params_ref = &params;
            let cache_clone = cache.clone();
            async move {
                (
                    url.clone(),
                    fetch_one_url(&url, params_ref, cache_clone, client).await,
                )
            }
        });

        let results: Vec<(String, ToolOutput)> = futures::stream::iter(futures)
            .buffer_unordered(BULK_CONCURRENCY)
            .collect()
            .await;

        Ok(combine_outputs(results))
    }
}

/// Fetch and extract a single URL.
///
/// This helper contains the full fetch pipeline for one target URL: SSRF
/// validation, optional robots.txt check, cache lookup, HTTP fetch, content
/// extraction, focus filtering, pagination, link classification, envelope
/// generation, and cache write-back.
async fn fetch_one_url(
    url: &str,
    params: &FetchParams,
    cache: ContentCache,
    client: &reqwest::Client,
) -> ToolOutput {
    // FR-019: SSRF validation.
    if let Err(e) = validate_url(url) {
        return security_blocked_output(url, &e.to_string());
    }

    // FR-028: robots.txt check (when enabled).
    if params.respect_robots {
        let checker = RobotsChecker::new();
        match checker.is_allowed(url, "*").await {
            Ok(false) => {
                return robots_blocked_output(url);
            }
            Ok(true) => {}
            Err(e) => {
                tracing::warn!(url = url, error = %e, "mf_fetch: robots.txt check failed; allowing by default");
            }
        }
    }

    let start = Instant::now();
    let normalised_url = normalise_url(url).unwrap_or_else(|_| url.to_string());

    // FR-018: content cache lookup.
    let cache_key = CacheKey::new(&normalised_url)
        .with_extraction_type(params.format.to_string())
        .with_css_selector(params.css_selector.clone());

    if params.cache_ttl != 0
        && let Ok(Some(cached)) = check_cache(&cache, &cache_key).await
    {
        return cached_output(
            &cached,
            &normalised_url,
            start.elapsed().as_millis() as u64,
            params.include_links,
        );
    }

    // FR-025: fetch via shared HTTP client.
    let response = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            return fetch_error_output(url, 0, &e.to_string());
        }
    };

    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/html")
        .to_string();

    let is_pdf = content_type
        .to_ascii_lowercase()
        .contains("application/pdf")
        || url.to_ascii_lowercase().ends_with(".pdf");

    if is_pdf {
        let bytes = match response.bytes().await {
            Ok(b) => b,
            Err(e) => {
                return fetch_error_output(url, status, &e.to_string());
            }
        };
        return pdf_tool_output(url, status, &content_type, &bytes, params).await;
    }

    let is_youtube = is_youtube_url(url);

    let body = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            return fetch_error_output(url, status, &e.to_string());
        }
    };

    if is_youtube {
        return youtube_tool_output(url, status, &content_type, &body).await;
    }

    let total_size_bytes = body.len();

    // Build extraction options.
    // To support `offset`, extract enough content so we can slice from the
    // offset without re-fetching. We clamp to a sensible upper bound.
    let extract_max = (params.offset + params.max_content_chars).clamp(500, 2_000_000);
    let extract_opts = ExtractOptions {
        format: params.format,
        css_selector: params.css_selector.clone(),
        max_content_chars: extract_max,
    };

    let extract_result = match extract(&body, url, &content_type, &extract_opts) {
        Ok(r) => r,
        Err(e) => {
            return selector_error_output(url, &e.to_string());
        }
    };

    // FR-004: optional focus filtering (post-extraction).
    let focused = if let Some(ref query) = params.focus_query {
        focus_content(&extract_result.content, query)
    } else {
        extract_result.content.clone()
    };

    // Apply offset + max_content_chars for pagination.
    let total_extracted_chars = focused.chars().count();
    let display_content: String = focused
        .chars()
        .skip(params.offset)
        .take(params.max_content_chars)
        .collect();
    let is_truncated = total_extracted_chars > params.offset + params.max_content_chars;
    let next_offset = if is_truncated {
        params.offset + params.max_content_chars
    } else {
        0
    };

    // FR-006: structured metadata.
    let mut metadata = extract_metadata(&body);
    // Detect the human language of the extracted text so downstream consumers
    // (e.g. the research References Index) can report it. Best-effort: empty
    // or non-linguistic content leaves the field as `None`.
    metadata.detected_language = detect_language(&display_content);

    // FR-007: outgoing link classification.
    let links = if params.include_links {
        classify_links(&body, url, &metadata)
    } else {
        Default::default()
    };

    // FR-003 / FR-029 / FR-030: envelope signals.
    let content_ok = !display_content.trim().is_empty();
    let mut envelope = build_envelope(&body, url, &metadata, content_ok, total_extracted_chars);
    if envelope.summary.is_empty() {
        envelope.summary = extract_result
            .title
            .clone()
            .or_else(|| metadata.title.clone())
            .unwrap_or_default();
    }

    let duration_ms = start.elapsed().as_millis() as u64;
    let fetcher_used = "http";
    let extraction_method = extract_result.method.to_string();

    // FR-018: cache successful content.
    let content_type_for_cache = content_type.clone();
    let cache_ttl = params.cache_ttl;
    if content_ok && cache_ttl != 0 {
        let cache_body = display_content.clone();
        let extraction_method_for_cache = extraction_method.clone();
        let key = cache_key.clone();
        let cache_clone = cache.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let method = Some(extraction_method_for_cache.as_str());
            if let Err(e) = cache_clone.set_cached_with_method(
                &key,
                &cache_body,
                true,
                status,
                &content_type_for_cache,
                cache_ttl,
                method,
            ) {
                tracing::warn!(error = %e, "mf_fetch: failed to cache content");
            }
        })
        .await;
    }

    let content = format!(
        "mf_fetch: {url}\nStatus: {status}\nContent type: {content_type}\nPage type: {page_type}\nContent OK: {content_ok}\nFetcher: {fetcher_used}\n\n{display_content}",
        page_type = envelope.page_type,
    );

    let mut metadata_json = json!({
        "url": url,
        "final_url": normalised_url,
        "status": status,
        "content_ok": content_ok,
        "page_type": envelope.page_type.to_string(),
        "source_type": envelope.source_type.to_string(),
        "is_official": envelope.is_official,
        "content_age_days": envelope.content_age_days,
        "is_stale": envelope.is_stale,
        "next_action": envelope.next_action,
        "summary": envelope.summary,
        "fetcher_used": fetcher_used,
        "extraction_method": extraction_method,
        "is_truncated": is_truncated,
        "next_offset": next_offset,
        "total_size_bytes": total_size_bytes,
        "total_extracted_chars": total_extracted_chars,
        "duration_ms": duration_ms,
        "cached": false,
        "detected_language": metadata.detected_language,
        "metadata": metadata,
        "version": MASTERFETCH_VERSION,
    });

    if let Some(obj) = metadata_json.as_object_mut()
        && params.include_links
    {
        obj.insert(
            "links".to_string(),
            json!({
                "citations": links.citations.iter().map(|l| json!({"href": l.href, "text": l.text, "rel": l.rel})).collect::<Vec<_>>(),
                "navigation": links.navigation.iter().map(|l| json!({"href": l.href, "text": l.text, "rel": l.rel})).collect::<Vec<_>>(),
                "external": links.external.iter().map(|l| json!({"href": l.href, "text": l.text, "rel": l.rel})).collect::<Vec<_>>(),
                "primary_source": links.primary_source,
            }),
        );
    }

    ToolOutput {
        content,
        metadata: Some(metadata_json),
    }
}

/// Build a `ToolOutput` for a successfully fetched PDF.
///
/// Extracts text from the PDF bytes and returns it as the page body, with
/// metadata signals that identify the source as a PDF document.
async fn pdf_tool_output(
    url: &str,
    status: u16,
    content_type: &str,
    bytes: &[u8],
    params: &FetchParams,
) -> ToolOutput {
    let total_size_bytes = bytes.len();
    let start = Instant::now();
    let bytes_owned = bytes.to_vec();
    let extracted = match tokio::task::spawn_blocking(move || extract_pdf_text(&bytes_owned)).await
    {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => {
            return pdf_error_output(url, status, &e.to_string());
        }
        Err(e) => {
            return pdf_error_output(url, status, &format!("PDF extraction task panicked: {e}"));
        }
    };
    let duration_ms = start.elapsed().as_millis() as u64;

    let title = extract_pdf_title(bytes);
    let total_extracted_chars = extracted.chars().count();
    let display_content: String = extracted
        .chars()
        .skip(params.offset)
        .take(params.max_content_chars)
        .collect();
    let is_truncated = total_extracted_chars > params.offset + params.max_content_chars;
    let next_offset = if is_truncated {
        params.offset + params.max_content_chars
    } else {
        0
    };
    let content_ok = !display_content.trim().is_empty();

    // Detect the human language of the extracted PDF text (best-effort).
    let detected_language = detect_language(&display_content);

    let content = format!(
        "mf_fetch: {url}\nStatus: {status}\nContent type: {content_type}\nPage type: pdf\nContent OK: {content_ok}\nFetcher: http\n\n{display_content}"
    );

    let metadata_json = json!({
        "url": url,
        "status": status,
        "content_ok": content_ok,
        "content_type": content_type,
                "content_type": "application/pdf",
                "page_type": "pdf",        "source_type": "unknown",
        "is_official": false,
        "content_age_days": -1,
        "is_stale": false,
        "next_action": if content_ok { "pdf text extracted" } else { "pdf extraction produced no text" },
        "summary": title.clone().unwrap_or_else(|| "PDF document".into()),
        "fetcher_used": "http",
        "is_truncated": is_truncated,
        "next_offset": next_offset,
        "total_size_bytes": total_size_bytes,
        "total_extracted_chars": total_extracted_chars,
        "duration_ms": duration_ms,
        "cached": false,
        "detected_language": detected_language,
        "version": MASTERFETCH_VERSION,
    });

    ToolOutput {
        content,
        metadata: Some(metadata_json),
    }
}

/// Build a `ToolOutput` when PDF text extraction fails.
fn pdf_error_output(url: &str, status: u16, error: &str) -> ToolOutput {
    ToolOutput {
        content: format!(
            "mf_fetch: {url}\nStatus: {status}\nContent type: application/pdf\nPage type: pdf\nContent OK: false\nFetcher: http\n\n[PDF text extraction failed: {error}]"
        ),
        metadata: Some(json!({
            "url": url,
            "status": status,
            "content_ok": false,
            "content_type": "application/pdf",
            "page_type": "pdf",
            "source_type": "unknown",
            "is_official": false,
            "content_age_days": -1,
            "is_stale": false,
            "next_action": "try a different PDF URL or use a dedicated PDF reader",
            "summary": "PDF text extraction failed",
            "fetcher_used": "http",
            "is_truncated": false,
            "next_offset": 0,
            "total_size_bytes": 0,
            "total_extracted_chars": 0,
            "duration_ms": 0,
            "cached": false,
            "error": error,
            "version": MASTERFETCH_VERSION,
        })),
    }
}

/// Build a `ToolOutput` for a successfully transcribed YouTube video.
///
/// Parses the watch-page HTML for `ytInitialPlayerResponse`, fetches captions,
/// and returns the transcript as the page body.
async fn youtube_tool_output(url: &str, status: u16, content_type: &str, html: &str) -> ToolOutput {
    let total_size_bytes = html.len();
    let start = Instant::now();
    let (title, transcript) = match extract_transcript_from_watch_page(html).await {
        Ok(t) => t,
        Err(e) => {
            let fallback_title =
                fallback_title_from_html(html).unwrap_or_else(|| "YouTube video".to_string());
            return youtube_error_output(url, status, &fallback_title, &e.to_string());
        }
    };
    let duration_ms = start.elapsed().as_millis() as u64;

    let total_extracted_chars = transcript.chars().count();
    let content_ok = !transcript.trim().is_empty();

    // Detect the human language of the transcript (best-effort).
    let detected_language = detect_language(&transcript);

    let content = format!(
        "mf_fetch: {url}\nStatus: {status}\nContent type: {content_type}\nPage type: youtube\nContent OK: {content_ok}\nFetcher: http\n\nTitle: {title}\n\n{transcript}"
    );

    let metadata_json = json!({
        "url": url,
        "status": status,
        "content_ok": content_ok,
        "page_type": "youtube",
        "source_type": "unknown",
        "is_official": false,
        "content_age_days": -1,
        "is_stale": false,
        "next_action": if content_ok { "youtube transcript extracted" } else { "no captions available" },
        "summary": title,
        "fetcher_used": "http",
        "is_truncated": false,
        "next_offset": 0,
        "total_size_bytes": total_size_bytes,
        "total_extracted_chars": total_extracted_chars,
        "duration_ms": duration_ms,
        "cached": false,
        "detected_language": detected_language,
        "version": MASTERFETCH_VERSION,
    });

    ToolOutput {
        content,
        metadata: Some(metadata_json),
    }
}

/// Build a `ToolOutput` when YouTube transcript extraction fails.
fn youtube_error_output(url: &str, status: u16, title: &str, error: &str) -> ToolOutput {
    ToolOutput {
        content: format!(
            "mf_fetch: {url}\nStatus: {status}\nContent type: text/html\nPage type: youtube\nContent OK: false\nFetcher: http\n\nTitle: {title}\n\n[YouTube transcript extraction failed: {error}]"
        ),
        metadata: Some(json!({
            "url": url,
            "status": status,
            "content_ok": false,
            "page_type": "youtube",
            "source_type": "unknown",
            "is_official": false,
            "content_age_days": -1,
            "is_stale": false,
            "next_action": "this video may not have captions; try a different source",
            "summary": title,
            "fetcher_used": "http",
            "is_truncated": false,
            "next_offset": 0,
            "total_size_bytes": 0,
            "total_extracted_chars": 0,
            "duration_ms": 0,
            "cached": false,
            "error": error,
            "version": MASTERFETCH_VERSION,
        })),
    }
}

/// Combine per-URL `ToolOutput`s into a single bulk response.
fn combine_outputs(results: Vec<(String, ToolOutput)>) -> ToolOutput {
    let count = results.len();
    let successful = results
        .iter()
        .filter(|(_, output)| {
            output
                .metadata
                .as_ref()
                .and_then(|m| m.get("content_ok"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
        .count();
    let content_ok = successful > 0;

    let mut parts = Vec::with_capacity(results.len() + 1);
    parts.push(format!(
        "mf_fetch: bulk fetch completed\nTotal URLs: {count}\nSuccessful: {successful}\nContent OK: {content_ok}\n"
    ));

    let mut result_metadata = Vec::with_capacity(results.len());
    for (_, output) in &results {
        parts.push(output.content.clone());
        result_metadata.push(output.metadata.clone().unwrap_or(Value::Null));
    }

    let urls: Vec<String> = results.iter().map(|(url, _)| url.clone()).collect();

    let content = parts.join("\n\n---\n\n");
    let metadata = json!({
        "bulk": true,
        "urls": urls,
        "count": count,
        "successful": successful,
        "content_ok": content_ok,
        "version": MASTERFETCH_VERSION,
        "results": result_metadata,
    });

    ToolOutput {
        content,
        metadata: Some(metadata),
    }
}

/// Build a `ToolOutput` when the shared HTTP client cannot be constructed for
/// a bulk request.
fn bulk_client_error_output(urls: &[String], error: &str) -> ToolOutput {
    ToolOutput {
        content: format!(
            "mf_fetch: bulk fetch failed to initialise HTTP client.\nURLs: {urls}\nError: {error}\n\nnext_action: retry or check the build environment.",
            urls = urls.join(", "),
        ),
        metadata: Some(json!({
            "bulk": true,
            "urls": urls,
            "count": urls.len(),
            "successful": 0,
            "content_ok": false,
            "next_action": "retry or check the build environment",
            "fetcher_used": "none",
            "error": error,
            "version": MASTERFETCH_VERSION,
            "results": [],
        })),
    }
}

/// Open the content cache for this tool invocation.
///
/// Tries to use a persistent `SQLite` cache under `.ragent/` in the working
/// directory. If that fails (e.g. in tests or ephemeral contexts), falls back
/// to an in-memory cache.
async fn open_cache(ctx: &ToolContext) -> ContentCache {
    let working_dir = &ctx.working_dir;
    let cache_path: PathBuf = working_dir.join(".ragent").join(CACHE_FILE_NAME);

    if let Err(e) =
        tokio::fs::create_dir_all(cache_path.parent().expect("cache path has parent")).await
    {
        tracing::warn!(error = %e, "mf_fetch: failed to create cache directory; using in-memory cache");
        return ContentCache::open_in_memory().expect("in-memory content cache should always open");
    }

    match ContentCache::open(&cache_path) {
        Ok(cache) => cache,
        Err(e) => {
            tracing::warn!(error = %e, path = %cache_path.display(), "mf_fetch: failed to open persistent cache; using in-memory cache");
            ContentCache::open_in_memory().expect("in-memory content cache should always open")
        }
    }
}

/// Look up a cached entry in a blocking-safe way.
async fn check_cache(
    cache: &ContentCache,
    key: &CacheKey,
) -> anyhow::Result<Option<crate::masterfetch::cache::CachedEntry>> {
    let cache = cache.clone();
    let key = key.clone();
    tokio::task::spawn_blocking(move || cache.get_cached(&key))
        .await
        .map_err(|e| anyhow::anyhow!("cache lookup task panicked: {e}"))?
}

/// Build a `ToolOutput` for a cache hit.
fn cached_output(
    entry: &crate::masterfetch::cache::CachedEntry,
    url: &str,
    duration_ms: u64,
    include_links: bool,
) -> ToolOutput {
    let mut metadata = json!({
        "url": url,
        "status": entry.status_code,
        "content_ok": entry.content_ok,
        "page_type": "unknown",
        "source_type": "unknown",
        "is_official": false,
        "content_age_days": -1,
        "is_stale": false,
        "next_action": "served from cache",
        "summary": "served from masterfetch content cache",
        "fetcher_used": "cache",
        // The extraction method is recorded at insert time; entries cached
        // before this signal existed report `"readability"` so downstream
        // consumers (the research web-gather phase) can still distinguish
        // readability-extracted pages from fallback extractions.
        "extraction_method": entry.extraction_method.as_deref().unwrap_or("readability"),
        "is_truncated": false,
        "next_offset": 0,
        "total_size_bytes": entry.content.len(),
        "total_extracted_chars": entry.content.chars().count(),
        "duration_ms": duration_ms,
        "cached": true,
        "version": MASTERFETCH_VERSION,
    });

    if include_links && let Some(obj) = metadata.as_object_mut() {
        obj.insert("links".to_string(), json!({}));
    }

    ToolOutput {
        content: format!(
            "mf_fetch: {url}\nStatus: {status}\nContent type: {content_type}\nFetcher: cache\n\n{content}",
            status = entry.status_code,
            content_type = entry.content_type,
            content = entry.content,
        ),
        metadata: Some(metadata),
    }
}
/// Build a `ToolOutput` when SSRF validation blocks the URL.
fn security_blocked_output(url: &str, error: &str) -> ToolOutput {
    ToolOutput {
        content: format!(
            "mf_fetch: URL rejected by SSRF security check.\nURL: {url}\nError: {error}\n\nnext_action: use a different, public URL."
        ),
        metadata: Some(json!({
            "url": url,
            "status": 0,
            "content_ok": false,
            "page_type": "unknown",
            "next_action": "use a different, public URL",
            "fetcher_used": "none",
            "is_truncated": false,
            "next_offset": 0,
            "error": error,
            "version": MASTERFETCH_VERSION,
        })),
    }
}

/// Build a `ToolOutput` when robots.txt disallows the URL.
fn robots_blocked_output(url: &str) -> ToolOutput {
    ToolOutput {
        content: format!(
            "mf_fetch: robots.txt disallows fetching this URL.\nURL: {url}\n\nnext_action: set respect_robots=false or choose a different URL."
        ),
        metadata: Some(json!({
            "url": url,
            "status": 0,
            "content_ok": false,
            "page_type": "unknown",
            "next_action": "set respect_robots=false or choose a different URL",
            "fetcher_used": "none",
            "is_truncated": false,
            "next_offset": 0,
            "error": "robots.txt disallowed",
            "version": MASTERFETCH_VERSION,
        })),
    }
}

/// Build a `ToolOutput` for a CSS selector parse error.
fn selector_error_output(url: &str, error: &str) -> ToolOutput {
    ToolOutput {
        content: format!(
            "mf_fetch: invalid CSS selector.\nURL: {url}\nError: {error}\n\nnext_action: correct or remove the css_selector parameter."
        ),
        metadata: Some(json!({
            "url": url,
            "status": 0,
            "content_ok": false,
            "page_type": "unknown",
            "next_action": "correct or remove the css_selector parameter",
            "fetcher_used": "none",
            "is_truncated": false,
            "next_offset": 0,
            "error": error,
            "version": MASTERFETCH_VERSION,
        })),
    }
}

/// Build a `ToolOutput` for an HTTP or body-read failure.
fn fetch_error_output(url: &str, status: u16, error: &str) -> ToolOutput {
    ToolOutput {
        content: format!(
            "mf_fetch: request failed.\nURL: {url}\nStatus: {status}\nError: {error}\n\nnext_action: retry, check connectivity, or try a different URL."
        ),
        metadata: Some(json!({
            "url": url,
            "status": status,
            "content_ok": false,
            "page_type": "unknown",
            "next_action": "retry, check connectivity, or try a different URL",
            "fetcher_used": "none",
            "is_truncated": false,
            "next_offset": 0,
            "error": error,
            "version": MASTERFETCH_VERSION,
        })),
    }
}

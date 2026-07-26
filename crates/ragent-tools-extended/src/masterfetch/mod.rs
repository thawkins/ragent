//! `MasterFetch` — integrated web-access tools for ragent.
//!
//! This module re-implements Hound's six web-access tools
//! ([`mf_fetch`][super], `mf_crawl`, `mf_search`, `mf_screenshot`,
//! `mf_cache_clear`, `mf_version`) as native Rust tools inside the
//! `ragent-tools-extended` crate. It provides shared types that mirror
//! Hound's `ResponseModel`, `SearchResult`, and `CrawlPage` structures so
//! that every tool response carries actionable, structured envelope signals.
//!
//! The types defined here are the foundation for the entire masterfetch
//! toolset. Subsequent tasks populate the sibling modules
//! (`security`, `http`, `urlnorm`, `extractor`, `metadata`, `links`,
//! `envelope`, `focus`, `robots`, `cache`, `search`, `crawl`, `tools`).
//!
//! # Requirements
//!
//! - **FR-001** — native Rust re-implementation of all six Hound tools.
//! - **NFR-004** — every public item carries a `///` doc comment; the module
//!   carries a `//!` doc comment.
//!
//! # Design
//!
//! All shared types derive `Serialize`, `Deserialize`, `Clone`, and `Debug`
//! so they can be embedded directly into [`ToolOutput::metadata`](super::ToolOutput::metadata)
//! as structured JSON. Enums implement [`std::fmt::Display`] and
//! [`serde::Serialize`] using their kebab-case string form so they serialize
//! to the exact wire format Hound uses (e.g. `"article"`, `"js_shell"`).

use serde::{Deserialize, Serialize};

pub mod cache;
pub mod crawl;
pub mod envelope;
pub mod extractor;
pub mod focus;
pub mod http;
pub mod links;
pub mod metadata;
pub mod pdf;
pub mod robots;
pub mod search;
pub mod security;
pub mod tools;
pub mod urlnorm;
pub mod youtube;

// ---------------------------------------------------------------------------
// Integration version (FR-017)
// ---------------------------------------------------------------------------

/// Masterfetch integration version — embedded in the `fetcher_used` signal
/// and returned by the `mf_version` tool.
///
/// This is the version of the masterfetch *integration* (the native Rust
/// re-implementation of Hound's tools), independent of the ragent product
/// version. It follows its own semver track starting at `0.1.0`.
pub const MASTERFETCH_VERSION: &str = "0.1.0";

// ---------------------------------------------------------------------------
// Page-type classification (FR-029)
// ---------------------------------------------------------------------------

/// Classification of a fetched page based on its HTML structure.
///
/// Drives the `next_action` suggestion: `list` pages suggest fetching the
/// linked URLs, `auth_wall` pages suggest switching sources, `js_shell`
/// pages report `content_ok = false` honestly.
///
/// Serializes to a lowercase snake-case string (e.g. `"js_shell"`,
/// `"auth_wall"`), matching Hound's wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PageType {
    /// Long-form article (blog post, news story, essay).
    Article,
    /// Documentation page (reference, guide, API docs).
    Docs,
    /// List / index page (directory, search results, tag page).
    List,
    /// Forum thread page.
    Forum,
    /// Q&A page (Stack Overflow, Quora question).
    Qa,
    /// JavaScript-rendered shell with little or no static content.
    JsShell,
    /// Login / authentication wall.
    AuthWall,
    /// Paywall-protected page.
    Paywall,
    /// Redirect / interstitial page.
    Redirect,
    /// Standalone image page.
    Image,
    /// JSON API endpoint (raw JSON response).
    Json,
    /// Unclassified page.
    #[default]
    Unknown,
}

impl std::fmt::Display for PageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Article => "article",
            Self::Docs => "docs",
            Self::List => "list",
            Self::Forum => "forum",
            Self::Qa => "qa",
            Self::JsShell => "js_shell",
            Self::AuthWall => "auth_wall",
            Self::Paywall => "paywall",
            Self::Redirect => "redirect",
            Self::Image => "image",
            Self::Json => "json",
            Self::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

// ---------------------------------------------------------------------------
// Source-authority classification (FR-030)
// ---------------------------------------------------------------------------

/// Classification of a page's source authority from its URL domain.
///
/// `is_official` is `true` only on strong signals: government, education,
/// GitHub, or vendor documentation sites.
///
/// Serializes to a lowercase snake-case string (e.g. `"vendor_docs"`,
/// `"docs_site"`), matching Hound's wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    /// Vendor documentation site (e.g. `docs.microsoft.com`).
    VendorDocs,
    /// Official product documentation (e.g. `developer.android.com`).
    OfficialDocs,
    /// News organisation site.
    News,
    /// Blog platform (e.g. Medium, Substack).
    Blog,
    /// Forum site (e.g. Reddit, Discourse).
    Forum,
    /// Q&A site (e.g. Stack Overflow, Stack Exchange).
    Qa,
    /// Government site (`.gov`).
    Gov,
    /// Educational institution (`.edu`).
    Edu,
    /// GitHub repository or profile.
    Github,
    /// Generic documentation site (`docs.*`, `developer.*`).
    DocsSite,
    /// E-commerce site.
    Ecommerce,
    /// Unclassified source.
    #[default]
    Unknown,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::VendorDocs => "vendor_docs",
            Self::OfficialDocs => "official_docs",
            Self::News => "news",
            Self::Blog => "blog",
            Self::Forum => "forum",
            Self::Qa => "qa",
            Self::Gov => "gov",
            Self::Edu => "edu",
            Self::Github => "github",
            Self::DocsSite => "docs_site",
            Self::Ecommerce => "ecommerce",
            Self::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

// ---------------------------------------------------------------------------
// Page metadata (FR-006)
// ---------------------------------------------------------------------------

/// Structured metadata extracted from a page's `OpenGraph` tags, JSON-LD
/// blocks, canonical link, and `<title>` tag.
///
/// All fields are optional strings because any given page may be missing
/// some or all metadata. Empty strings are normalised to [`Option::None`]
/// during extraction so downstream consumers can treat absent and blank
/// values identically.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageMetadata {
    /// Page title (from `OpenGraph` `og:title` or `<title>`).
    pub title: Option<String>,
    /// Page description (from `og:description` or `<meta name="description">`).
    pub description: Option<String>,
    /// Site name (from `og:site_name`).
    pub site_name: Option<String>,
    /// `OpenGraph` object type (from `og:type`).
    #[serde(rename = "type")]
    pub og_type: Option<String>,
    /// Preview image URL (from `og:image`).
    pub image: Option<String>,
    /// Canonical URL (from `<link rel="canonical">`).
    pub canonical: Option<String>,
    /// Page language (from `<html lang="...">`).
    pub lang: Option<String>,
    /// Publication timestamp (ISO 8601 string from `article:published_time`
    /// or JSON-LD `datePublished`).
    pub published_time: Option<String>,
    /// Last-modified timestamp (ISO 8601 string from
    /// `article:modified_time` or JSON-LD `dateModified`).
    pub modified_time: Option<String>,
    /// Author name (from `article:author` or JSON-LD `author`).
    pub author: Option<String>,
}

// ---------------------------------------------------------------------------
// Envelope signals (FR-003, FR-029, FR-030)
// ---------------------------------------------------------------------------

/// Hound v10 envelope signals attached to every fetch and crawl response.
///
/// These fields make every response actionable: agents branch on the
/// structured fields rather than parsing error text. Hard-blocks (404, bot
/// detection, auth walls) populate `content_ok = false` with a
/// `next_action` hint rather than returning fake content.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvelopeSignals {
    /// Detected page type.
    pub page_type: PageType,
    /// Detected source authority.
    pub source_type: SourceType,
    /// `true` only on strong authority signals (gov, edu, github, vendor docs).
    pub is_official: bool,
    /// Age of the content in days, computed from the modified (preferred) or
    /// published date. `-1` when no date is recoverable.
    pub content_age_days: i64,
    /// `true` when `content_age_days > 365`.
    pub is_stale: bool,
    /// `true` when usable content was successfully extracted.
    pub content_ok: bool,
    /// Suggested next action for the agent (e.g. "fetch linked URLs",
    /// "switch source").
    pub next_action: String,
    /// One-line summary of the page content.
    pub summary: String,
}

// ---------------------------------------------------------------------------
// Fetch result (FR-002, FR-003, FR-005)
// ---------------------------------------------------------------------------

/// Result of a single URL fetch through the masterfetch HTTP path.
///
/// Produced by `mf_fetch` (and internally by `mf_crawl` for each page).
/// The `content` field is the extracted text; the `envelope` field carries
/// the actionable signals; `metadata` carries the structured page metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FetchResult {
    /// Final URL after redirects.
    pub url: String,
    /// HTTP status code (0 if the request failed before receiving a response).
    pub status: u16,
    /// Extracted text content (markdown, html, text, or raw per the `format`
    /// parameter).
    pub content: String,
    /// HTTP `Content-Type` header value.
    pub content_type: String,
    /// Total response body size in bytes.
    pub total_size_bytes: usize,
    /// Number of characters in the extracted content.
    pub total_extracted_chars: usize,
    /// `true` when the content was truncated to fit `max_content_chars`.
    pub is_truncated: bool,
    /// Character offset to resume from on the next paginated call.
    pub next_offset: usize,
    /// Which fetcher produced the result: `"http"` or `"cache"`.
    pub fetcher_used: String,
    /// `true` when the result was served from the cache.
    pub cached: bool,
    /// Request duration in milliseconds.
    pub duration_ms: u64,
    /// Structured page metadata.
    pub metadata: PageMetadata,
    /// Envelope signals (page type, freshness, source authority, etc.).
    pub envelope: EnvelopeSignals,
    /// Error message if the fetch failed; empty string on success.
    pub error: String,
}

// ---------------------------------------------------------------------------
// Search result (FR-008, FR-009)
// ---------------------------------------------------------------------------

/// A single ranked result from `mf_search`.
///
/// Results are merged from multiple keyless search-engine backends,
/// deduplicated by normalised URL, and ranked by relevance. The
/// `engines_consensus` field indicates how many distinct backends returned
/// the same URL, providing a cross-engine trust signal.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchResult {
    /// Result title.
    pub title: String,
    /// Result URL (normalised).
    pub url: String,
    /// Short snippet / abstract from the search engine.
    pub snippet: String,
    /// Source domain or engine label.
    pub source: String,
    /// 1-based position in the final ranked list.
    pub position: usize,
    /// Relevance score in the range `0.0..=1.0`.
    pub relevance_score: f64,
    /// Coarse relevance tier: `"high"`, `"med"`, or `"low"`.
    pub fetch_relevance: String,
    /// Cross-engine consensus label (e.g. `"2/3"` meaning 2 of 3 engines
    /// returned this URL).
    pub engines_consensus: String,
}

// ---------------------------------------------------------------------------
// Crawl page (FR-011, FR-012)
// ---------------------------------------------------------------------------

/// A single page within a `mf_crawl` result set.
///
/// Each crawled page carries its own `content_ok` flag, `page_type`
/// classification, and one-line `summary` so the agent can decide which
/// pages to read in full. The `content` field holds the extracted markdown
/// (or a structured link list for list/index pages).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CrawlPage {
    /// Page URL (normalised, final after redirects).
    pub url: String,
    /// HTTP status code.
    pub status: u16,
    /// `true` when usable content was extracted.
    pub content_ok: bool,
    /// Detected page type.
    pub page_type: PageType,
    /// One-line summary of the page.
    pub summary: String,
    /// Extracted content (markdown for articles/docs; link list for
    /// list/index pages).
    pub content: String,
    /// Crawl depth from the start URL (0 = start page).
    pub depth: usize,
    /// Which fetcher produced the page: `"http"` or `"cache"`.
    pub fetcher_used: String,
    /// Per-page fetch duration in milliseconds.
    pub duration_ms: u64,
    /// Error message if the page fetch failed; empty string on success.
    pub error: String,
}

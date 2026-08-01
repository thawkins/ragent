//! Web-gathering phase for the research system (FR-006, FR-007).
//!
//! This module implements the orchestration logic that turns a research
//! topic into a list of [`Source::Web`] entries. The actual HTTP calls
//! are made through the [`WebSearchTool`] and [`WebFetchTool`] trait
//! abstractions so the gatherer can be unit-tested without network access
//! and reused from any integration context (TUI agent loop, CLI, HTTP
//! endpoint, tests).
//!
//! ## Flow
//!
//! 1. [`WebGatherer::gather`] issues a [`WebSearchTool::search`] for the
//!    topic and collects up to `max_results` candidate URLs.
//! 2. For each candidate URL it calls [`WebFetchTool::fetch`] to obtain
//!    the page body and title.
//! 3. Each captured page becomes a [`Source::Web`] entry with a synthetic
//!    supporting-file path of the form `sources/web-NN.md` (zero-padded,
//!    starting at 01) — the actual supporting-file write is done by the IO
//!    layer (T-015) once we have an item directory on disk; this module
//!    only returns the captured metadata.
//! 4. If the search or fetch tools return zero results the gatherer
//!    returns an empty `Vec` (FR-006: graceful degradation).
//!
//! ## Reuse, not reimplementation
//!
//! Per the spec constraints, the gatherer does **not** reimplement search
//! or fetch — it delegates entirely to the provided `WebSearchTool` /
//! `WebFetchTool` implementations. In production these wrap the existing
//! `websearch` and `webfetch` tools in `crates/ragent-tools-extended`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};
use ragent_llm::provider::ProviderRegistry;
use serde::Deserialize;

use crate::document::fence_source_body;
use crate::source::Source;

/// Maximum number of focused sub-queries the research decomposer will
/// produce for a single topic. Increasing this raises the web-search
/// parallelism and usually increases the number of distinct sources found,
/// while staying within typical LLM output budgets for a JSON array.
pub(crate) const MAX_DECOMPOSED_QUERIES: usize = 10;

/// Default maximum number of web sources to capture per research item. The
/// earlier 15-source cap was too restrictive for broad topics; a larger
/// default lets the decomposer's parallel queries surface a much wider
/// set of candidate URLs before the synthesis phase.
/// Default cap on the number of web sources captured when the caller does not
/// supply an explicit `max_web_results` (FR-011).
pub const DEFAULT_MAX_WEB_RESULTS: usize = 250;

/// Default upper bound on the number of concurrent page fetches issued during
/// the capture phase of [`WebGatherer::gather_with_observer`]. 10 is a safe
/// middle ground: fast enough to keep wall-clock latency low when a search
/// returns many candidate URLs, while staying well clear of OS file-descriptor
/// limits and typical search-provider rate ceilings.  Override with the
/// `--fetch-concurrently N` CLI flag or [`WebGatherer::with_fetch_concurrency`].
pub const DEFAULT_FETCH_CONCURRENCY: usize = 10;

/// Cap a captured web body at the same byte budget used by the supporting
/// file renderer so the body stored on the `Source` matches what ends up on
/// disk. Keeps runaway pages from blowing up the synthesis prompt.
fn fence_captured_body(body: &str) -> String {
    fence_source_body(body)
}

/// Maximum length enforced for a stored web-source title. Longer titles are
/// truncated at a word boundary with an ellipsis so the References Index and
/// the per-finding `**Sources:**` bullets stay readable. Captured titles come
/// from the page's readability-extracted `<title>`/heading or the search-hit
/// title and frequently contain nav chrome ("Skip to main content") or consent
/// banners ("We use essential cookies to make our site work..."); see
/// [`clean_web_source_title`].
const MAX_WEB_SOURCE_TITLE_CHARS: usize = 120;

/// Leading phrases that mark a captured title as page chrome rather than
/// article content. When the cleaned title starts with one of these it is
/// stripped; when the *entire* cleaned title is one of these (after markdown
/// link syntax is removed) the title is discarded in favour of the fallback.
const TITLE_NOISE_PHRASES: &[&str] = &[
    "skip to main content",
    "skip to content",
    "skip navigation",
    "skip to nav",
    "jump to content",
    "we use essential cookies",
    "we use cookies",
    "this site uses cookies",
    "agree & join",
    "agree and join",
    "sign in",
    "sign up",
    "log in",
    "join/login",
    "join sign in",
];

/// Clean a page title captured from a fetch or search hit before it is stored
/// on a [`Source::Web`], so the title shown in the References Index and the
/// per-finding `**Sources:**` bullets is short and meaningful rather than nav
/// chrome or a consent banner. This is a pure code transform — no LLM.
///
/// Steps:
/// 1. Strip markdown reference-link (`[text][n]`) and inline-link
///    (`[text](url)`) syntax, keeping the link text.
/// 2. Drop a leading nav/cookie/consent phrase from [`TITLE_NOISE_PHRASES`].
/// 3. Collapse internal whitespace and trim.
/// 4. Truncate to [`MAX_WEB_SOURCE_TITLE_CHARS`] at a word boundary with an
///    ellipsis.
/// 5. When the cleaned primary is empty (or was pure noise), repeat on
///    `fallback` (typically the search-hit title or the URL). When both are
///    empty/noise, return the raw fallback so the title is never blank.
#[must_use]
fn clean_web_source_title(primary: &str, fallback: &str) -> String {
    let cleaned = clean_title_text(primary);
    if !cleaned.is_empty() {
        return cleaned;
    }
    let cleaned_fallback = clean_title_text(fallback);
    if !cleaned_fallback.is_empty() {
        return cleaned_fallback;
    }
    // Both reduced to nothing — surface a non-empty raw value so the
    // References Index never shows a blank title cell.
    fallback.trim().to_string()
}

/// Strip markdown link syntax, leading nav/consent noise, collapse whitespace,
/// and truncate to [`MAX_WEB_SOURCE_TITLE_CHARS`] at a word boundary.
fn clean_title_text(s: &str) -> String {
    let stripped = strip_markdown_link_text(s);
    let stripped = strip_leading_noise(&stripped);
    let collapsed = collapse_title_ws(&stripped);
    truncate_title_words(&collapsed, MAX_WEB_SOURCE_TITLE_CHARS)
}

/// Replace markdown reference links (`[text][n]`, `[text][]`) and inline links
/// (`[text](url)`) with just the link `text`, leaving non-link content intact.
fn strip_markdown_link_text(s: &str) -> String {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        // Match `[text]` immediately followed by either `[...]` or `(...)`.
        regex::Regex::new(r"\[([^\]]*)\](?:\[[^\]]*\]|\([^)]*\))").expect("title link regex")
    });
    RE.replace_all(s, "$1").into_owned()
}

/// Remove a single leading nav/cookie/consent phrase (case-insensitive) from
/// `s`, including any trailing separator punctuation. Returns `s` unchanged
/// when no noise phrase matches the start.
fn strip_leading_noise(s: &str) -> String {
    let trimmed = s.trim_start();
    let lower = trimmed.to_lowercase();
    for phrase in TITLE_NOISE_PHRASES {
        if lower.starts_with(phrase) {
            // Map the matched prefix length back to the original slice so we
            // keep the original casing of the remainder.
            let kept = &trimmed[phrase.len()..];
            let after = kept.trim_start_matches([' ', ',', ':', '|', '-', '—', '·']);
            return after.trim().to_string();
        }
    }
    trimmed.trim().to_string()
}

/// Collapse runs of whitespace into single spaces and trim the ends.
fn collapse_title_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Truncate `s` to at most `max_chars` Unicode scalar values, cutting at the
/// last whitespace boundary at or before the limit so words are not split. An
/// ellipsis is appended when truncation occurs.
fn truncate_title_words(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    // Reserve two chars for the " …" suffix when possible.
    let budget = max_chars.saturating_sub(2);
    let mut end_byte = 0usize;
    let mut last_space_byte = 0usize;
    for (i, (byte_idx, ch)) in s.char_indices().enumerate() {
        if i >= budget {
            break;
        }
        end_byte = byte_idx + ch.len_utf8();
        if ch.is_whitespace() {
            last_space_byte = byte_idx;
        }
    }
    // Prefer to cut at the last whitespace so we don't split a word.
    let cut_byte = if last_space_byte > 0 {
        last_space_byte
    } else {
        end_byte
    };
    // Walk back to a UTF-8 char boundary (last_space_byte is already on a
    // boundary; end_byte is a char-end boundary by construction).
    let mut out = s[..cut_byte].trim_end().to_string();
    if !out.is_empty() {
        out.push('…');
    }
    out
}

/// Trait abstracting the decomposition of a research topic into focused
/// sub-queries.  A decomposer may be heuristic (cheap, no LLM) or LLM-backed
/// (higher quality, costs one call).  When no decomposer is configured the
/// gatherer falls back to searching the raw topic as a single query.
#[async_trait]
pub trait QueryDecomposer: Send + Sync {
    /// Break `topic` into a list of search queries.  The gatherer runs each
    /// query in parallel, deduplicates results by URL, and then fetches up
    /// to the caller's `max_results` unique pages.
    async fn decompose(&self, topic: &str) -> anyhow::Result<Vec<String>>;
}

/// Simple heuristic decomposer that splits a topic on conjunctions and
/// commas, then also includes the original topic as a catch-all query.
///
/// Cheap and deterministic; requires no network calls.  Kept as a fallback
/// for the LLM-backed decomposer and for callers that intentionally want
/// heuristic splitting.
#[derive(Debug, Default, Clone, Copy)]
pub struct HeuristicQueryDecomposer;

#[async_trait]
impl QueryDecomposer for HeuristicQueryDecomposer {
    async fn decompose(&self, topic: &str) -> anyhow::Result<Vec<String>> {
        let trimmed = topic.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        // 1. Split on sentence boundaries first. Long prose topics often
        //    contain commas inside a single sentence; splitting on those commas
        //    first produces nonsensical fragments.
        let mut queries: Vec<String> = Vec::new();
        for sentence in split_into_sentence_chunks(trimmed) {
            // 2. Within each sentence, split on explicit conjunctions of short
            //    noun phrases (e.g. "Rust async and Tokio runtime"). Only split
            //    when every resulting chunk is short enough to be a focused query.
            let mut sentence_queries = split_short_conjunctions(&sentence);
            queries.append(&mut sentence_queries);
        }

        // 3. If the whole topic is a short comma-separated list (no sentence
        //    punctuation), treat the comma-separated items as distinct queries.
        if queries.len() == 1
            && let Some(list_queries) = split_comma_list(trimmed)
        {
            queries = list_queries;
        }

        // Deduplicate preserving order; keep the full topic last so it acts
        // as a catch-all when earlier sub-queries returned nothing.
        let mut seen = HashSet::new();
        let mut deduped: Vec<String> = Vec::new();
        for q in queries {
            let normalized = collapse_whitespace(&q);
            if normalized.is_empty() {
                continue;
            }
            let lower = normalized.to_lowercase();
            if seen.insert(lower) {
                deduped.push(normalized);
            }
        }
        let full_lower = trimmed.to_lowercase();
        if seen.insert(full_lower) {
            deduped.push(trimmed.to_string());
        }

        // Cap the number of sub-queries to avoid hammering the search
        // provider while still giving broad topics enough coverage.
        deduped.truncate(MAX_DECOMPOSED_QUERIES);
        Ok(deduped)
    }
}

/// Split a topic on sentence boundaries, keeping parenthesised text intact.
fn split_into_sentence_chunks(topic: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0usize;
    let chars: Vec<char> = topic.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        current.push(c);
        match c {
            '(' | '[' | '{' => paren_depth += 1,
            ')' | ']' | '}' if paren_depth > 0 => paren_depth -= 1,
            _ => {}
        }
        if paren_depth == 0 && matches!(c, '.' | '?' | '!') {
            // End of a sentence only if followed by whitespace or end of text.
            if i + 1 == chars.len() || chars[i + 1].is_whitespace() {
                let chunk = current.trim().to_string();
                if !chunk.is_empty() {
                    out.push(chunk);
                }
                current.clear();
            }
        }
        i += 1;
    }
    let remainder = current.trim().to_string();
    if !remainder.is_empty() {
        out.push(remainder);
    }
    if out.is_empty() {
        out.push(topic.to_string());
    }
    out
}

/// Split a sentence on " and ", " & ", " + " and "; " only when every
/// resulting chunk is short enough to be a useful focused query. This keeps
/// long prose sentences intact while expanding short conjunctions like
/// "Rust async and Tokio runtime".
fn split_short_conjunctions(sentence: &str) -> Vec<String> {
    const MAX_CHUNK_WORDS: usize = 8;
    let separators = [" and ", " & ", " + ", "; "];

    // First try splitting on each separator.
    let mut best: Option<Vec<String>> = None;
    for sep in &separators {
        let parts: Vec<String> = sentence
            .split(sep)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(collapse_whitespace)
            .collect();
        if parts.len() > 1
            && parts
                .iter()
                .all(|p| p.split_whitespace().count() <= MAX_CHUNK_WORDS)
            && best.as_ref().is_none_or(|b| parts.len() > b.len())
        {
            best = Some(parts);
        }
    }

    if let Some(parts) = best {
        return parts;
    }
    vec![collapse_whitespace(sentence)]
}

/// If `topic` looks like a short comma-separated list of distinct phrases,
/// return those phrases. Returns `None` for long prose or single-sentence
/// topics so they are not over-split.
fn split_comma_list(topic: &str) -> Option<Vec<String>> {
    let comma_chunks: Vec<&str> = topic
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if comma_chunks.len() < 2 || comma_chunks.len() > 5 {
        return None;
    }
    if topic.contains('.') || topic.contains('?') || topic.contains('!') || topic.contains(';') {
        return None;
    }
    let total_words: usize = comma_chunks
        .iter()
        .map(|s| s.split_whitespace().count())
        .sum();
    if total_words > 25 {
        return None;
    }
    Some(comma_chunks.into_iter().map(collapse_whitespace).collect())
}

/// Collapse runs of whitespace into a single space and trim.
fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// LLM-backed query decomposer.
///
/// Sends the topic to the configured provider/model and asks it to return a
/// JSON array of 1-10 focused web-search queries. The first query should be the
/// most specific; the last query can be a broader catch-all. If the model
/// response cannot be parsed, or the provider is unavailable, the decomposer
/// falls back to the heuristic splitter so research always makes progress.
#[derive(Clone)]
pub struct LlmQueryDecomposer {
    provider_registry: Arc<ProviderRegistry>,
    provider_id: String,
    model_id: String,
    api_key: Option<String>,
    base_url: Option<String>,
    fallback: HeuristicQueryDecomposer,
}

impl std::fmt::Debug for LlmQueryDecomposer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmQueryDecomposer")
            .field("provider_id", &self.provider_id)
            .field("model_id", &self.model_id)
            .field("has_api_key", &self.api_key.is_some())
            .finish_non_exhaustive()
    }
}

impl LlmQueryDecomposer {
    /// Build a new LLM decomposer.
    pub fn new(
        provider_registry: Arc<ProviderRegistry>,
        provider_id: impl Into<String>,
        model_id: impl Into<String>,
    ) -> Self {
        Self {
            provider_registry,
            provider_id: provider_id.into(),
            model_id: model_id.into(),
            api_key: None,
            base_url: None,
            fallback: HeuristicQueryDecomposer,
        }
    }

    /// Provide an API key for the provider.
    #[must_use]
    pub fn with_api_key(mut self, api_key: Option<String>) -> Self {
        self.api_key = api_key;
        self
    }

    /// Override the API base URL.
    #[must_use]
    pub fn with_base_url(mut self, base_url: Option<String>) -> Self {
        self.base_url = base_url;
        self
    }

    async fn decompose_with_llm(&self, topic: &str) -> anyhow::Result<Vec<String>> {
        let provider = self
            .provider_registry
            .get(&self.provider_id)
            .ok_or_else(|| anyhow::anyhow!("unknown provider '{}'", self.provider_id))?;

        let api_key = self.api_key.clone().unwrap_or_default();
        let client = provider
            .create_client(
                &api_key,
                self.base_url.as_deref(),
                &std::collections::HashMap::new(),
            )
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "failed to create LLM client for {}/{}: {e}",
                    self.provider_id,
                    self.model_id
                )
            })?;

        let prompt = format!(
            "You are decomposing a research topic into focused web-search queries.\n\nTopic: {topic}\n\nReturn a JSON object with exactly one key, \"queries\", whose value is an array of 1 to {MAX_DECOMPOSED_QUERIES} short search-engine queries that together cover the topic. Put the most specific query first and a broader catch-all query last. Each query must be a plain string with no markdown or explanation.\n\nExample response:\n{{\"queries\":[\"Rust async runtime internals\", \"Tokio runtime scheduling\", \"Rust async and Tokio runtime\"]}}\n\nNow produce only the JSON object:"
        );

        let request = ChatRequest {
            model: self.model_id.clone(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text(prompt),
            }]),
            tools: Arc::new(vec![]),
            temperature: Some(0.2),
            top_p: Some(1.0),
            max_tokens: Some(512),
            system: Some(std::sync::Arc::from(
                "You are a precise research assistant that returns only valid JSON.",
            )),
            options: std::collections::HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: Some(120),
            thinking: None,
        };

        let mut stream = client.chat(request).await?;
        let mut text = String::new();
        while let Some(event) = stream.next().await {
            match event {
                StreamEvent::TextDelta { text: delta } => text.push_str(&delta),
                StreamEvent::Error { message } => anyhow::bail!("provider error: {message}"),
                StreamEvent::Finish { .. } => break,
                _ => {}
            }
        }

        parse_query_decomposition(&text)
    }
}

#[async_trait]
impl QueryDecomposer for LlmQueryDecomposer {
    async fn decompose(&self, topic: &str) -> anyhow::Result<Vec<String>> {
        match self.decompose_with_llm(topic).await {
            Ok(qs) if !qs.is_empty() => Ok(qs),
            Ok(_) => {
                tracing::warn!(
                    topic,
                    "research: LLM decomposer returned empty queries; falling back to heuristic"
                );
                self.fallback.decompose(topic).await
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    topic,
                    "research: LLM query decomposition failed; falling back to heuristic"
                );
                self.fallback.decompose(topic).await
            }
        }
    }
}

/// Parse the model's JSON response into a list of queries.
///
/// Accepts `{ "queries": [...] }`, markdown-fenced JSON, and strips trailing
/// commas before delegating to `serde_json`.
fn parse_query_decomposition(raw: &str) -> anyhow::Result<Vec<String>> {
    let trimmed = raw.trim();
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };

    let cleaned = remove_trailing_commas(json_str);

    #[derive(Deserialize)]
    struct DecompResponse {
        queries: Vec<String>,
    }

    let parsed: DecompResponse = serde_json::from_str(&cleaned).map_err(|e| {
        anyhow::anyhow!("failed to parse decomposition JSON: {e}\n\nRaw response:\n{raw}")
    })?;

    let queries: Vec<String> = parsed
        .queries
        .into_iter()
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty())
        .collect();

    if queries.is_empty() {
        anyhow::bail!("LLM decomposer returned no usable queries");
    }

    // Enforce the same cap used elsewhere.
    Ok(queries.into_iter().take(MAX_DECOMPOSED_QUERIES).collect())
}

/// Remove trailing commas before `}` or `]` in JSON.
fn remove_trailing_commas(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for i in 0..len {
        if chars[i] == ',' {
            let mut j = i + 1;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            if j < len && (chars[j] == '}' || chars[j] == ']') {
                continue;
            }
        }
        result.push(chars[i]);
    }
    result
}

/// Result of a decomposed web-gathering pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatherResult {
    /// Sub-queries that were actually issued to the search tool.
    pub queries: Vec<String>,
    /// Captured web sources, already deduplicated by URL and limited to the
    /// caller's `max_results` budget.
    pub sources: Vec<Source>,
    /// Count of captured PDF documents.
    pub pdf_count: usize,
    /// Count of captured YouTube video URLs.
    pub youtube_count: usize,
    /// Number of candidate web sources that were fetched but excluded because
    /// their relevance score was too low.
    pub excluded_count: usize,
}

impl GatherResult {
    /// Empty result with no queries and no sources.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            queries: Vec::new(),
            sources: Vec::new(),
            pdf_count: 0,
            youtube_count: 0,
            excluded_count: 0,
        }
    }
}

/// Search-result row returned by a [`WebSearchTool`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearchHit {
    /// Page URL.
    pub url: String,
    /// Page title as reported by the search provider (may be empty).
    pub title: String,
    /// One- or two-line snippet (may be empty).
    pub snippet: String,
    /// The actual sub-query string that returned this hit. Used by the
    /// gatherer to compute a deterministic relevance note and to annotate the
    /// source in the RESEARCH.md References Index.
    pub matched_query: String,
    /// Name of the agent tool that issued the search (e.g. `"mf_search"` or
    /// `"websearch"`). This lets the research output show *which* search tool
    /// produced the source.
    pub search_tool: String,
    /// Name(s) of the backend search engine(s) that returned this hit. For
    /// `mf_search` this is a comma-separated list like `"duckduckgo, brave"`;
    /// for `websearch` it is `"tavily"`.
    pub search_engine: String,
}
/// Page body returned by a [`WebFetchTool`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebFetchedPage {
    /// Page URL — must match the URL passed in.
    pub url: String,
    /// Resolved page title (may be empty if the page lacked a title).
    pub title: String,
    /// Rendered text body of the page, in UTF-8. HTML tags should already
    /// have been stripped by the implementation.
    pub body: String,
    /// Publication date parsed from the page's embedded metadata, when the
    /// fetcher was able to determine one. `None` when the page did not expose
    /// a parseable publication date.
    pub published_at: Option<DateTime<Utc>>,
    /// HTTP `Content-Type` reported by the fetcher, when available. Used by
    /// the research layer to classify PDFs and other media types.
    pub content_type: Option<String>,
    /// Page-type classification reported by the fetcher (e.g. `article`,
    /// `docs`). Currently informational; `content_type` drives media
    /// classification.
    pub page_type: Option<String>,
    /// Detected human language of the page body, when the fetcher reported
    /// one. `None` when language detection was unavailable.
    pub language: Option<String>,
}

/// Classified kind of a captured web source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSourceKind {
    /// A normal web page (article, blog post, documentation, etc.).
    Page,
    /// A PDF document detected by `Content-Type` or URL extension.
    Pdf,
    /// A YouTube video URL. When the fetch layer extracts a transcript the
    /// captured body contains the caption text; otherwise the body contains the
    /// watch-page chrome and description.
    YouTube,
}

/// Classify a web URL by its `Content-Type` and host.
///
/// PDFs are recognised by an `application/pdf` content type or by a `.pdf`
/// path extension. YouTube URLs are recognised by host (`youtube.com` or
/// `youtu.be`). Everything else is treated as a generic page.
#[must_use]
pub fn classify_web_source(url: &str, content_type: Option<&str>) -> WebSourceKind {
    if content_type.is_some_and(|ct| ct.to_ascii_lowercase().contains("application/pdf"))
        || url.to_ascii_lowercase().ends_with(".pdf")
    {
        return WebSourceKind::Pdf;
    }
    if let Ok(parsed) = url::Url::parse(url) {
        let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
        if host.contains("youtube.com") || host.contains("youtu.be") {
            return WebSourceKind::YouTube;
        }
    }
    WebSourceKind::Page
}

impl WebSourceKind {
    /// Human-readable classifier used when serialising web sources.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Pdf => "pdf",
            Self::YouTube => "youtube",
        }
    }
}

/// Trait abstracting the existing `websearch` tool.
///
/// Production wiring delegates to the real tool from
/// `ragent-tools-extended`; tests provide an in-memory fake.
#[async_trait]
pub trait WebSearchTool: Send + Sync {
    /// Run a web search for `query` and return up to `max_results` hits.
    async fn search(&self, query: &str, max_results: usize) -> anyhow::Result<Vec<WebSearchHit>>;
}

/// Trait abstracting the existing `webfetch` tool.
#[async_trait]
pub trait WebFetchTool: Send + Sync {
    /// Fetch `url` and return the rendered page body.
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage>;
}

/// Errors emitted by [`WebGatherer`].
#[derive(Debug, thiserror::Error)]
pub enum WebGatherError {
    /// The configured search limit was zero — there is nothing to gather.
    #[error("web gatherer called with max_results = 0")]
    ZeroLimit,
    /// An empty topic was supplied.
    #[error("web gatherer called with an empty topic")]
    EmptyTopic,
}

/// Diagnostic events emitted by [`WebGatherer`] during a gather pass.
///
/// These are surfaced to the UI so users can see *why* no web sources were
/// captured (missing API key, network failure, fetch timeout, etc.).
#[derive(Debug, Clone)]
pub enum GatherEvent {
    /// The query decomposition step produced these sub-queries.
    QueriesDecomposed {
        /// Sub-queries that will be issued to the search tool.
        queries: Vec<String>,
    },
    /// A single candidate page was fetched and captured as a source.
    /// Emitted inline as each fetch succeeds so the UI can show
    /// successfully retrieved URLs as they arrive, rather than only
    /// seeing failures during the gather and successes at the end.
    SourceCaptured {
        /// URL of the captured page.
        url: String,
        /// Page title (may be empty).
        title: String,
        /// Search tool that produced this hit.
        search_tool: String,
        /// Backend search engine(s) that returned this URL.
        search_engine: String,
    },
    /// The underlying search tool returned an error.
    SearchFailed {
        /// Error message from the search tool.
        error: String,
    },
    /// A single page fetch failed after the search produced a candidate URL.
    FetchFailed {
        /// URL that could not be fetched.
        url: String,
        /// Error message from the fetch tool.
        error: String,
    },
    /// Search succeeded but returned zero hits.
    SearchReturnedNoHits,
}

/// Observer receiving [`GatherEvent`]s from [`WebGatherer`].
pub trait GatherObserver: Send + Sync {
    /// Receive a diagnostic event.
    fn on_event(&self, event: GatherEvent);
}

/// Orchestrates a single web-gathering pass for one research topic.
///
/// `WebGatherer` is cheap to clone (internally an `Arc` pair) so the TUI
/// and CLI can hold one instance and call [`gather`] many times.
#[derive(Clone)]
pub struct WebGatherer {
    search: Arc<dyn WebSearchTool>,
    fetch: Arc<dyn WebFetchTool>,
    decomposer: Option<Arc<dyn QueryDecomposer>>,
    /// Upper bound on the number of concurrent page fetches issued during the
    /// capture phase of [`gather_with_observer`]. Defaults to
    /// [`DEFAULT_FETCH_CONCURRENCY`]; override via [`with_fetch_concurrency`].
    fetch_concurrency: usize,
    /// When `true`, every fetched page is retained regardless of its
    /// relevance score, disabling the default filter that discards
    /// "Low"/"Very low" sources. Defaults to `false`.
    keep_low_relevance: bool,
}

impl std::fmt::Debug for WebGatherer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebGatherer")
            .field("has_decomposer", &self.decomposer.is_some())
            .field("fetch_concurrency", &self.fetch_concurrency)
            .field("keep_low_relevance", &self.keep_low_relevance)
            .finish_non_exhaustive()
    }
}

impl WebGatherer {
    /// Construct a new gatherer from a search tool and a fetch tool.
    ///
    /// The fetch-phase concurrency defaults to [`DEFAULT_FETCH_CONCURRENCY`]
    /// (10); override it with [`WebGatherer::with_fetch_concurrency`].
    pub fn new(search: Arc<dyn WebSearchTool>, fetch: Arc<dyn WebFetchTool>) -> Self {
        Self {
            search,
            fetch,
            decomposer: None,
            fetch_concurrency: DEFAULT_FETCH_CONCURRENCY,
            keep_low_relevance: false,
        }
    }

    /// Attach a query decomposer.  When present, [`gather_with_observer`]
    /// decomposes the topic into parallel sub-queries and deduplicates the
    /// combined results.
    pub fn with_decomposer(mut self, decomposer: Arc<dyn QueryDecomposer>) -> Self {
        self.decomposer = Some(decomposer);
        self
    }

    /// Override the fetch-phase concurrency limit.
    ///
    /// Controls how many candidate page fetches are issued in parallel during
    /// [`gather_with_observer`].  Values of `0` are clamped up to `1` so the
    /// stream always makes progress.  Larger values reduce wall-clock latency
    /// when a search returns many hits, at the cost of more in-flight HTTP
    /// connections and memory.  The default is [`DEFAULT_FETCH_CONCURRENCY`]
    /// (10).
    #[must_use]
    pub fn with_fetch_concurrency(mut self, n: usize) -> Self {
        self.fetch_concurrency = n.max(1);
        self
    }

    /// Keep low-relevance web sources instead of filtering them out.
    ///
    /// When enabled, [`gather_with_observer`] retains every fetched page
    /// regardless of its query-match relevance score, disabling the default
    /// filter that discards "Low"/"Very low" sources.
    #[must_use]
    pub fn with_keep_low_relevance(mut self, keep: bool) -> Self {
        self.keep_low_relevance = keep;
        self
    }

    /// Fetch a single URL and return it as a [`Source::Web`] plus the raw
    /// [`WebFetchedPage`].
    ///
    /// Used by `--from-url` to capture a user-supplied page as the primary
    /// research subject *before* the normal web-search phase runs. The body is
    /// fenced via [`fence_captured_body`] so it stays within the same byte
    /// budget as pages captured during gathering. The `body_path` is set to
    /// `web-01.md` (index 0); the manager renumbers supporting files by
    /// position at write time, so this is purely a metadata hint.
    ///
    /// # Errors
    ///
    /// Returns the underlying fetch error when the page cannot be retrieved.
    pub async fn fetch_url_as_source(&self, url: &str) -> anyhow::Result<(Source, WebFetchedPage)> {
        let page = self.fetch.fetch(url).await?;
        let body = fence_captured_body(&page.body);
        let title = clean_web_source_title(&page.title, url);
        let media_type = classify_web_source(url, page.content_type.as_deref())
            .as_str()
            .to_string();
        let source = Source::Web {
            url: page.url.clone(),
            title,
            captured_at: chrono::Utc::now(),
            published_at: page.published_at,
            body_path: web_body_path(0),
            body,
            relevance: "User-supplied seed URL".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: page.content_type.clone(),
            page_type: page.page_type.clone(),
            media_type,
            language: page.language.clone(),
        };
        Ok((source, page))
    }

    /// Gather up to `max_results` web sources for `topic`.
    ///
    /// Returns an empty `Vec` (not an error) when:
    ///
    /// - The search tool returns no hits (FR-006 graceful degradation).
    /// - Every fetch call fails for transient reasons (logged at info,
    ///   not surfaced as an error to the caller — the local-gathering
    ///   phase can still produce a useful RESEARCH.md).
    ///
    /// Returns a [`WebGatherError`] only for programmer mistakes such as
    /// `max_results == 0` or `topic.is_empty()`.
    pub async fn gather(
        &self,
        topic: &str,
        max_results: usize,
    ) -> Result<Vec<Source>, WebGatherError> {
        let result = self.gather_with_observer(topic, max_results, None).await?;
        Ok(result.sources)
    }

    /// Gather web sources with an optional observer for diagnostic events.
    ///
    /// When a decomposer is configured the topic is first split into focused
    /// sub-queries; each sub-query is issued in parallel, results are
    /// deduplicated by URL, and up to `max_results` unique pages are fetched
    /// **concurrently** up to [`WebGatherer::fetch_concurrency`] at a time
    /// (default [`DEFAULT_FETCH_CONCURRENCY`], 10).  [`GatherEvent`]
    /// diagnostics (`SourceCaptured` / `FetchFailed`) fire in fetch-completion
    /// order so the UI can render each page as soon as it arrives; the returned
    /// `sources` vector is re-sorted into the original search-ranking order so
    /// the `web-NN.md` supporting-file names track hit position.  The returned
    /// [`GatherResult`] lists the sub-queries that were used so the caller can
    /// persist them in `RESEARCH.md`.
    pub async fn gather_with_observer(
        &self,
        topic: &str,
        max_results: usize,
        observer: Option<&dyn GatherObserver>,
    ) -> Result<GatherResult, WebGatherError> {
        if max_results == 0 {
            return Err(WebGatherError::ZeroLimit);
        }
        if topic.trim().is_empty() {
            return Err(WebGatherError::EmptyTopic);
        }

        tracing::info!(topic, max_results, "research: starting web-gathering phase");

        // Determine the set of sub-queries.  If no decomposer is configured
        // we still treat the original topic as a single query so callers see
        // a consistent [`GatherResult`].
        let queries: Vec<String> = match &self.decomposer {
            Some(d) => match d.decompose(topic).await {
                Ok(qs) if !qs.is_empty() => qs,
                Ok(_) => {
                    tracing::warn!("research: decomposer returned empty queries; using topic");
                    vec![topic.to_string()]
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "research: query decomposition failed; falling back to single query"
                    );
                    vec![topic.to_string()]
                }
            },
            None => vec![topic.to_string()],
        };

        if let Some(obs) = observer {
            obs.on_event(GatherEvent::QueriesDecomposed {
                queries: queries.clone(),
            });
        }

        // Run each sub-query in parallel with bounded concurrency. Each
        // future owns its query string so we don't borrow `queries`.
        let search_tool = self.search.clone();
        let search_futures: Vec<_> = queries
            .iter()
            .map(|q| {
                let q = q.clone();
                let tool = search_tool.clone();
                async move { tool.search(&q, max_results).await }
            })
            .collect();
        let mut results = futures::stream::iter(search_futures)
            .buffer_unordered(4)
            .enumerate();

        let mut hits_by_url: Vec<(String, WebSearchHit)> = Vec::new();
        let mut seen_urls: HashSet<String> = HashSet::new();
        let mut any_search_error: Option<String> = None;

        while let Some((idx, result)) = results.next().await {
            let query = queries
                .get(idx)
                .cloned()
                .unwrap_or_else(|| topic.to_string());
            match result {
                Ok(hits) => {
                    for mut hit in hits {
                        let url_key = hit.url.to_lowercase();
                        if seen_urls.insert(url_key) {
                            hit.matched_query = query.clone();
                            hits_by_url.push((query.clone(), hit));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        query = %query,
                        error = %e,
                        "research: sub-query search failed"
                    );
                    any_search_error = Some(format!("{query}: {e}"));
                }
            }
        }

        if hits_by_url.is_empty() {
            if let Some(err) = any_search_error {
                if let Some(obs) = observer {
                    obs.on_event(GatherEvent::SearchFailed { error: err });
                }
            } else if let Some(obs) = observer {
                obs.on_event(GatherEvent::SearchReturnedNoHits);
            }
            tracing::info!("research: websearch returned 0 hits");
            return Ok(GatherResult {
                queries,
                sources: Vec::new(),
                pdf_count: 0,
                youtube_count: 0,
                excluded_count: 0,
            });
        }

        // Fetch each unique candidate concurrently up to `fetch_concurrency`
        // at a time.  `SourceCaptured` / `FetchFailed` events fire in
        // completion order (so the UI renders pages as they arrive); the
        // collected `(index, Option<Source>)` pairs are re-sorted into the
        // original search-ranking order afterwards so `web-NN.md` supporting
        // file names track hit position rather than completion timing.
        let fetch_concurrency = self.fetch_concurrency.max(1);
        let fetch_tool = self.fetch.clone();
        let candidates: Vec<(usize, String, WebSearchHit)> = hits_by_url
            .into_iter()
            .take(max_results)
            .enumerate()
            .map(|(index, (query, hit))| (index, query, hit))
            .collect();
        let fetch_futures = candidates.into_iter().map(|(index, query, hit)| {
            let fetch_tool = fetch_tool.clone();
            async move {
                let result = fetch_tool.fetch(&hit.url).await;
                (index, query, hit, result)
            }
        });
        let mut collected: Vec<(usize, Option<Source>)> = Vec::with_capacity(max_results);
        let mut excluded_count = 0usize;
        let mut stream = futures::stream::iter(fetch_futures).buffer_unordered(fetch_concurrency);
        while let Some((index, query, hit, result)) = stream.next().await {
            match result {
                Ok(page) => {
                    let title = clean_web_source_title(&page.title, &hit.title);
                    let body_path = web_body_path(index);
                    let body = fence_captured_body(&page.body);
                    let (relevance, retained) =
                        compute_relevance_label(&query, &title, &hit.snippet, &page.url);
                    if !retained && !self.keep_low_relevance {
                        excluded_count += 1;
                        tracing::info!(
                            query = %query,
                            url = %page.url,
                            relevance = %relevance,
                            "research: skipping web source due to low relevance"
                        );
                        if let Some(obs) = observer {
                            obs.on_event(GatherEvent::FetchFailed {
                                url: page.url.clone(),
                                error: format!("relevance too low ({relevance})"),
                            });
                        }
                        collected.push((index, None));
                        continue;
                    }
                    tracing::info!(
                        query = %query,
                        url = %page.url,
                        title = %title,
                        body_path = %body_path.display(),
                        body_chars = body.chars().count(),
                        relevance = %relevance,
                        "research: captured web source"
                    );
                    if let Some(obs) = observer {
                        obs.on_event(GatherEvent::SourceCaptured {
                            url: page.url.clone(),
                            title: title.clone(),
                            search_tool: hit.search_tool.clone(),
                            search_engine: hit.search_engine.clone(),
                        });
                    }
                    collected.push((
                        index,
                        Some(Source::Web {
                            url: page.url.clone(),
                            title,
                            captured_at: Utc::now(),
                            published_at: page.published_at,
                            body_path,
                            body,
                            relevance,
                            search_tool: hit.search_tool,
                            search_engine: hit.search_engine,
                            content_type: page.content_type.clone(),
                            page_type: page.page_type.clone(),
                            media_type: classify_web_source(
                                &page.url,
                                page.content_type.as_deref(),
                            )
                            .as_str()
                            .to_string(),
                            language: page.language.clone(),
                        }),
                    ));
                }
                Err(e) => {
                    if let Some(obs) = observer {
                        obs.on_event(GatherEvent::FetchFailed {
                            url: hit.url.clone(),
                            error: e.to_string(),
                        });
                    }
                    tracing::warn!(
                        query = %query,
                        url = %hit.url,
                        error = %e,
                        "research: webfetch failed; skipping"
                    );
                    collected.push((index, None));
                }
            }
        }
        // Restore search-ranking order so `web-NN.md` numbers track hit
        // position rather than fetch-completion timing.
        collected.sort_by_key(|(index, _)| *index);
        let mut pdf_count = 0usize;
        let mut youtube_count = 0usize;
        let sources: Vec<Source> = collected
            .into_iter()
            .filter_map(|(_, src)| {
                if let Some(Source::Web {
                    url, content_type, ..
                }) = src.as_ref()
                {
                    match classify_web_source(url, content_type.as_deref()) {
                        WebSourceKind::Pdf => pdf_count += 1,
                        WebSourceKind::YouTube => youtube_count += 1,
                        WebSourceKind::Page => {}
                    }
                }
                src
            })
            .collect();
        tracing::info!(
            count = sources.len(),
            pdf_count,
            youtube_count,
            excluded_count,
            "research: web-gathering phase complete"
        );
        Ok(GatherResult {
            queries,
            sources,
            pdf_count,
            youtube_count,
            excluded_count,
        })
    }
}

/// Compute a deterministic relevance note for a captured web source.
///
/// The score is based only on the search query that produced the hit and the
/// hit's title, snippet, and URL domain, so it adds zero LLM cost and is fully
/// reproducible. It returns a short human-readable string like:
///
/// - "High — title + snippet match query"
/// - "Medium — snippet matches query"
/// - "Low — weak match"
/// - "Very high — exact title match"
fn compute_relevance_label(query: &str, title: &str, snippet: &str, url: &str) -> (String, bool) {
    let query_lc = query.to_lowercase();
    let query_terms: Vec<String> = query_lc
        .split_whitespace()
        .filter(|t| !is_stopword(t))
        .filter(|t| t.len() > 2 || t.chars().any(char::is_alphabetic))
        .map(std::string::ToString::to_string)
        .collect();
    if query_terms.is_empty() {
        return ("Match score unavailable".into(), true);
    }

    let hay = format!(
        "{} {} {}",
        title.to_lowercase(),
        snippet.to_lowercase(),
        url.to_lowercase()
    );
    let mut hits = 0usize;
    let mut title_hits = 0usize;
    let mut snippet_hits = 0usize;
    let title_lc = title.to_lowercase();
    let snippet_lc = snippet.to_lowercase();
    for term in &query_terms {
        if hay.contains(term) {
            hits += 1;
            if title_lc.contains(term) {
                title_hits += 1;
            }
            if snippet_lc.contains(term) {
                snippet_hits += 1;
            }
        }
    }
    let ratio = hits as f64 / query_terms.len() as f64;

    let label = if !title.is_empty() && title_lc == query.to_lowercase() {
        "Very high — exact title match"
    } else if ratio >= 0.75 && title_hits > 0 && snippet_hits > 0 {
        "High — title + snippet match query"
    } else if ratio >= 0.6 && title_hits > 0 {
        "High — title matches query"
    } else if ratio >= 0.6 && snippet_hits > 0 {
        "Medium-high — snippet matches query"
    } else if ratio >= 0.45 {
        "Medium — partial query match"
    } else if ratio >= 0.2 {
        "Low — weak query match"
    } else {
        "Very low — no clear query match"
    };

    let retained = !label.starts_with("Low") && !label.starts_with("Very low");
    (label.into(), retained)
}

/// Returns true for common English stopwords that should not dilute the
/// relevance ratio. Removing them prevents a question like "What is Rust?"
/// from being scored as low relevance just because the auxiliary words do not
/// appear in the title or snippet.
fn is_stopword(word: &str) -> bool {
    const STOPWORDS: &[&str] = &[
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "must", "can",
        "shall", "of", "in", "on", "at", "to", "for", "with", "from", "by", "about", "as", "and",
        "or", "but", "not", "no", "yes", "what", "which", "who", "when", "where", "why", "how",
        "this", "that", "these", "those", "i", "you", "he", "she", "it", "we", "they", "their",
        "there", "them", "his", "her", "its", "our", "your", "my", "me", "him", "us",
    ];
    STOPWORDS.contains(&word.to_lowercase().as_str())
}

/// Compute the zero-padded supporting-file path for the Nth web source.
///
/// Index 0 → `web-01.md`, index 1 → `web-02.md`, etc. The path is
/// relative to the research item directory (`research/<name>/`).
fn web_body_path(index: usize) -> PathBuf {
    PathBuf::from(format!("sources/web-{:02}.md", index + 1))
}

#[cfg(test)]
mod title_tests {
    use super::*;

    #[test]
    fn clean_title_strips_markdown_reference_links() {
        let out = clean_web_source_title("[Skip to main content][1]", "");
        // The whole title was nav chrome → reduces to empty → fallback empty.
        assert!(
            out.is_empty(),
            "pure-noise title with empty fallback should be empty, got {out:?}"
        );
    }

    #[test]
    fn clean_title_strips_markdown_links_but_keeps_text() {
        let out = clean_web_source_title("[DeepSeek V4 Pro][1] model card", "");
        assert_eq!(out, "DeepSeek V4 Pro model card");
    }

    #[test]
    fn clean_title_strips_inline_markdown_links() {
        let out = clean_web_source_title("[DeepSeek](https://deepseek.com) overview", "");
        assert_eq!(out, "DeepSeek overview");
    }

    #[test]
    fn clean_title_strips_leading_cookie_banner() {
        let long = "We use essential cookies to make our site work. With your consent, we may also use non-essential cookies to improve your site for you and your experience";
        let out = clean_web_source_title(long, "");
        // Leading cookie phrase is stripped; remainder is truncated to the cap.
        assert!(
            out.chars().count() <= MAX_WEB_SOURCE_TITLE_CHARS,
            "got {} chars: {out}",
            out.chars().count()
        );
        assert!(!out.to_lowercase().contains("we use essential cookies"));
        assert!(out.ends_with('…'));
    }

    #[test]
    fn clean_title_truncates_long_title_at_word_boundary() {
        let long = "This is a genuinely long and meaningful article title that goes well beyond the one hundred and twenty character cap so it must be truncated by the gatherer";
        let out = clean_web_source_title(long, "");
        assert!(
            out.chars().count() <= MAX_WEB_SOURCE_TITLE_CHARS,
            "got {} chars: {out}",
            out.chars().count()
        );
        assert!(out.ends_with('…'));
        // Should not split a word mid-way.
        assert!(!out.ends_with("… "));
    }

    #[test]
    fn clean_title_falls_back_when_primary_is_noise() {
        // page.title is pure nav chrome; fallback (search-hit title) should win.
        let out = clean_web_source_title("[Skip to main content][1]", "Real Article Title");
        assert_eq!(out, "Real Article Title");
    }

    #[test]
    fn clean_title_falls_back_when_primary_is_empty() {
        let out = clean_web_source_title("", "Hit Title");
        assert_eq!(out, "Hit Title");
    }

    #[test]
    fn clean_title_preserves_short_meaningful_title() {
        let out = clean_web_source_title("A — resolved", "fallback");
        assert_eq!(out, "A — resolved");
    }

    #[test]
    fn clean_title_returns_raw_fallback_when_both_reduce_to_empty() {
        let out = clean_web_source_title("[Skip to content][2]", "");
        assert!(
            out.is_empty(),
            "both-noise with empty fallback yields empty, got {out:?}"
        );
    }

    #[test]
    fn clean_title_url_fallback_is_preserved() {
        // fetch_url_as_source passes the URL as fallback; a URL is not noise.
        let out = clean_web_source_title("", "https://example.com/deepseek-v4");
        assert_eq!(out, "https://example.com/deepseek-v4");
    }

    #[test]
    fn strip_leading_noise_is_case_insensitive() {
        let out = clean_title_text("SKIP TO MAIN CONTENT: DeepSeek V4 Pro");
        assert_eq!(out, "DeepSeek V4 Pro");
    }

    #[test]
    fn truncate_title_words_keeps_short_input_intact() {
        let out = truncate_title_words("short title", 120);
        assert_eq!(out, "short title");
    }

    #[test]
    fn truncate_title_words_returns_empty_for_empty_input() {
        let out = truncate_title_words("", 120);
        assert!(out.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// In-memory `WebSearchTool` for tests.
    #[derive(Default)]
    struct FakeSearch {
        hits: Vec<WebSearchHit>,
        calls: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl WebSearchTool for FakeSearch {
        async fn search(
            &self,
            query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            self.calls.lock().unwrap().push(query.to_string());
            // Tag each returned hit with the query that produced it so the
            // gatherer's relevance computation has realistic metadata.
            let mut out = self.hits.clone();
            for hit in &mut out {
                hit.matched_query = query.to_string();
            }
            Ok(out)
        }
    }

    /// In-memory `WebFetchTool` for tests. Each URL maps to an optional
    /// `WebFetchedPage`; missing URLs produce an error.
    #[derive(Default)]
    struct FakeFetch {
        pages: std::collections::HashMap<String, WebFetchedPage>,
        fail_urls: Vec<String>,
        calls: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl WebFetchTool for FakeFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            self.calls.lock().unwrap().push(url.to_string());
            if self.fail_urls.iter().any(|u| u == url) {
                anyhow::bail!("simulated fetch failure for {url}");
            }
            self.pages
                .get(url)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no fake page registered for {url}"))
        }
    }

    fn gatherer_with(
        hits: Vec<WebSearchHit>,
        pages: std::collections::HashMap<String, WebFetchedPage>,
        fail_urls: Vec<String>,
    ) -> (WebGatherer, Arc<FakeSearch>, Arc<FakeFetch>) {
        let search = Arc::new(FakeSearch {
            hits,
            calls: Mutex::new(Vec::new()),
        });
        let fetch = Arc::new(FakeFetch {
            pages,
            fail_urls,
            calls: Mutex::new(Vec::new()),
        });
        let g = WebGatherer::new(search.clone(), fetch.clone());
        (g, search, fetch)
    }

    #[tokio::test]
    async fn gather_returns_empty_vec_when_search_returns_no_hits() {
        let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let sources = g.gather("rust async", 5).await.unwrap();
        assert!(sources.is_empty());
    }

    #[tokio::test]
    async fn gather_returns_empty_vec_when_search_tool_errors() {
        struct AlwaysFailSearch;
        #[async_trait]
        impl WebSearchTool for AlwaysFailSearch {
            async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
                anyhow::bail!("network down")
            }
        }
        struct OkFetch;
        #[async_trait]
        impl WebFetchTool for OkFetch {
            async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: "u".into(),
                    title: "t".into(),
                    body: "b".into(),
                    content_type: None,
                    page_type: None,
                    language: None,
                })
            }
        }
        let g = WebGatherer::new(Arc::new(AlwaysFailSearch), Arc::new(OkFetch));
        let sources = g.gather("topic", 5).await.unwrap();
        assert!(
            sources.is_empty(),
            "search failure must not surface as an error"
        );
    }

    #[tokio::test]
    async fn gather_creates_web_source_per_hit_with_sequential_body_paths() {
        let hits = vec![
            WebSearchHit {
                url: "https://a.example".into(),
                title: "A".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
            WebSearchHit {
                url: "https://b.example".into(),
                title: "B".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
            WebSearchHit {
                url: "https://c.example".into(),
                title: "C".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://a.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://a.example".into(),
                title: "A — resolved".into(),
                body: "body a".into(),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        pages.insert(
            "https://b.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://b.example".into(),
                title: "B — resolved".into(),
                body: "body b".into(),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        pages.insert(
            "https://c.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://c.example".into(),
                title: String::new(), // empty title should fall back to search hit title
                body: "body c".into(),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("topic", 5).await.unwrap();
        assert_eq!(sources.len(), 3);

        for (i, src) in sources.iter().enumerate() {
            let Source::Web {
                published_at: None,
                url,
                title,
                body_path,
                ..
            } = src
            else {
                panic!("expected Source::Web, got {src:?}");
            };
            assert_eq!(
                body_path.as_path(),
                PathBuf::from(format!("sources/web-{:02}.md", i + 1)).as_path()
            );
            assert!(!url.is_empty());
            assert!(!title.is_empty());
        }
        // The third source had an empty page title, so it should have
        // fallen back to the search-hit title "C".
        if let Source::Web { title, .. } = &sources[2] {
            assert_eq!(title, "C");
        }
    }
    #[tokio::test]
    async fn gather_skips_individual_fetch_failures() {
        let hits = vec![
            WebSearchHit {
                url: "https://ok".into(),
                title: "OK".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
            WebSearchHit {
                url: "https://bad".into(),
                title: "Bad".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://ok".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://ok".into(),
                title: "OK".into(),
                body: "b".into(),

                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, vec!["https://bad".into()]);
        let sources = g.gather("topic", 5).await.unwrap();
        assert_eq!(
            sources.len(),
            1,
            "failed fetch should be skipped, not abort"
        );
        if let Source::Web { url, .. } = &sources[0] {
            assert_eq!(url, "https://ok");
        }
    }
    #[tokio::test]
    async fn gather_respects_max_results() {
        let hits = vec![
            WebSearchHit {
                url: "https://1".into(),
                title: "1".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
            WebSearchHit {
                url: "https://2".into(),
                title: "2".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
            WebSearchHit {
                url: "https://3".into(),
                title: "3".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        for u in ["https://1", "https://2", "https://3"] {
            pages.insert(
                u.into(),
                WebFetchedPage {
                    published_at: None,
                    url: u.into(),
                    title: u.into(),
                    body: "b".into(),

                    content_type: None,
                    page_type: None,
                    language: None,
                },
            );
        }
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("topic", 2).await.unwrap();
        assert_eq!(sources.len(), 2, "must not exceed max_results");
    }
    #[tokio::test]
    async fn gather_rejects_zero_max_results() {
        let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let err = g.gather("topic", 0).await.unwrap_err();
        assert!(matches!(err, WebGatherError::ZeroLimit));
    }

    #[tokio::test]
    async fn gather_rejects_empty_topic() {
        let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let err = g.gather("   ", 5).await.unwrap_err();
        assert!(matches!(err, WebGatherError::EmptyTopic));
    }

    #[tokio::test]
    async fn gather_records_search_call() {
        let (g, search, _) =
            gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let _ = g.gather("rust async", 5).await.unwrap();
        let calls = search.calls.lock().unwrap();
        assert_eq!(calls.as_slice(), &["rust async".to_string()]);
    }
    #[test]
    fn web_body_path_zero_pads_and_uses_one_based_index() {
        assert_eq!(web_body_path(0), PathBuf::from("sources/web-01.md"));
        assert_eq!(web_body_path(8), PathBuf::from("sources/web-09.md"));
        assert_eq!(web_body_path(9), PathBuf::from("sources/web-10.md"));
    }

    #[tokio::test]
    async fn gather_with_observer_emits_search_failed_on_search_error() {
        struct FailSearch;
        #[async_trait]
        impl WebSearchTool for FailSearch {
            async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
                anyhow::bail!("api key missing")
            }
        }
        struct OkFetch;
        #[async_trait]
        impl WebFetchTool for OkFetch {
            async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: "u".into(),
                    title: "t".into(),
                    body: "b".into(),

                    content_type: None,
                    page_type: None,
                    language: None,
                })
            }
        }
        #[derive(Default)]
        struct CollectEvents(Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        let g = WebGatherer::new(Arc::new(FailSearch), Arc::new(OkFetch));
        let obs = CollectEvents::default();
        let result = g
            .gather_with_observer("topic", 5, Some(&obs))
            .await
            .unwrap();
        assert!(result.sources.is_empty());
        assert_eq!(result.queries, vec!["topic".to_string()]);
        let events = obs.0.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[0], GatherEvent::QueriesDecomposed { queries } if queries == &["topic".to_string()])
        );
        assert!(
            matches!(&events[1],
                GatherEvent::SearchFailed { error } if error.contains("api key missing")
            ),
            "got {:?}",
            events[1]
        );
    }
    #[tokio::test]
    async fn gather_with_observer_emits_no_hits_when_search_is_empty() {
        #[derive(Default)]
        struct CollectEvents(Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let obs = CollectEvents::default();
        let result = g
            .gather_with_observer("rust async", 5, Some(&obs))
            .await
            .unwrap();
        assert!(result.sources.is_empty());
        assert_eq!(result.queries, vec!["rust async".to_string()]);
        let events = obs.0.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[0], GatherEvent::QueriesDecomposed { queries } if queries == &["rust async".to_string()])
        );
        assert!(matches!(events[1], GatherEvent::SearchReturnedNoHits));
    }
    #[tokio::test]
    async fn gather_with_observer_emits_fetch_failed_for_each_bad_url() {
        let hits = vec![
            WebSearchHit {
                url: "https://ok".into(),
                title: "OK".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
            WebSearchHit {
                url: "https://bad".into(),
                title: "Bad".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://ok".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://ok".into(),
                title: "OK".into(),
                body: "b".into(),

                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, vec!["https://bad".into()]);
        #[derive(Default)]
        struct CollectEvents(Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        let obs = CollectEvents::default();
        let result = g
            .gather_with_observer("topic", 5, Some(&obs))
            .await
            .unwrap();
        assert_eq!(result.sources.len(), 1);
        let events = obs.0.lock().unwrap();
        assert!(
                events.iter().any(|e| matches!(
                    e,
                    GatherEvent::FetchFailed { url, error } if url == "https://bad" && error.contains("simulated fetch failure")
                )),
                "got {:?}",
                  *events
              );
    }

    #[tokio::test]
    async fn gather_with_decomposer_runs_parallel_sub_queries_and_dedupes() {
        struct RecordingSearch {
            responses: std::collections::HashMap<String, Vec<WebSearchHit>>,
            calls: Mutex<Vec<String>>,
        }
        #[async_trait]
        impl WebSearchTool for RecordingSearch {
            async fn search(
                &self,
                query: &str,
                _max_results: usize,
            ) -> anyhow::Result<Vec<WebSearchHit>> {
                self.calls.lock().unwrap().push(query.to_string());
                Ok(self.responses.get(query).cloned().unwrap_or_default())
            }
        }
        struct OkFetch;
        #[async_trait]
        impl WebFetchTool for OkFetch {
            async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: url.to_string(),
                    title: format!("title-{url}"),
                    body: format!("body-{url}"),
                    content_type: None,
                    page_type: None,
                    language: None,
                })
            }
        }

        let responses = std::collections::HashMap::from([
            (
                "Rust async".to_string(),
                vec![WebSearchHit {
                    url: "https://a.example".into(),
                    title: "A".into(),
                    snippet: "topic Rust async Tokio runtime".into(),
                    matched_query: String::new(),
                    search_tool: String::new(),
                    search_engine: String::new(),
                }],
            ),
            (
                "Tokio runtime".to_string(),
                vec![
                    WebSearchHit {
                        url: "https://a.example".into(), // duplicate URL
                        title: "A2".into(),
                        snippet: "topic Rust async Tokio runtime".into(),
                        matched_query: String::new(),
                        search_tool: String::new(),
                        search_engine: String::new(),
                    },
                    WebSearchHit {
                        url: "https://b.example".into(),
                        title: "B".into(),
                        snippet: "topic Rust async Tokio runtime".into(),
                        matched_query: String::new(),
                        search_tool: String::new(),
                        search_engine: String::new(),
                    },
                ],
            ),
        ]);
        let search = Arc::new(RecordingSearch {
            responses,
            calls: Mutex::new(Vec::new()),
        });
        let gatherer = WebGatherer::new(search.clone(), Arc::new(OkFetch))
            .with_decomposer(Arc::new(HeuristicQueryDecomposer));

        let result = gatherer
            .gather_with_observer("Rust async and Tokio runtime", 5, None)
            .await
            .unwrap();

        // Both sub-queries plus the catch-all full topic were issued.
        let calls = search.calls.lock().unwrap();
        assert!(calls.contains(&"Rust async".to_string()));
        assert!(calls.contains(&"Tokio runtime".to_string()));
        assert!(calls.contains(&"Rust async and Tokio runtime".to_string()));

        // The duplicate https://a.example URL is fetched only once.
        assert_eq!(
            result.sources.len(),
            2,
            "dedup should leave two unique URLs"
        );
        assert_eq!(result.queries.len(), 3);
    }

    #[tokio::test]
    async fn llm_decomposer_parses_json_queries() {
        use ragent_llm::llm::{ChatRequest, LlmClient, StreamEvent};
        use ragent_llm::providers::ProviderRegistry;
        use std::pin::Pin;

        struct JsonReplyClient {
            text: String,
        }

        #[async_trait]
        impl LlmClient for JsonReplyClient {
            async fn chat(
                &self,
                _request: ChatRequest,
            ) -> anyhow::Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>>
            {
                let events = vec![
                    StreamEvent::TextDelta {
                        text: self.text.clone(),
                    },
                    StreamEvent::Finish {
                        reason: ragent_llm::llm::LlmFinishReason::Stop,
                    },
                ];
                Ok(Box::pin(futures::stream::iter(events)))
            }
        }

        struct JsonProvider;

        #[async_trait]
        impl ragent_llm::provider::Provider for JsonProvider {
            fn id(&self) -> &'static str {
                "json"
            }

            fn name(&self) -> &'static str {
                "JSON"
            }

            fn default_models(&self) -> Vec<ragent_llm::provider::ModelInfo> {
                Vec::new()
            }

            async fn create_client(
                &self,
                _api_key: &str,
                _base_url: Option<&str>,
                _options: &std::collections::HashMap<String, serde_json::Value>,
            ) -> anyhow::Result<Box<dyn LlmClient>> {
                Ok(Box::new(JsonReplyClient {
                            text: r#"{"queries":["Rust async internals","Tokio runtime","Rust async and Tokio runtime"]}"#.into(),
                        }))
            }

            fn set_event_bus(&self, _event_bus: Option<Arc<ragent_types::event::EventBus>>) {}

            fn as_any_static(&self) -> &dyn std::any::Any {
                self
            }
        }

        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(JsonProvider));
        let decomposer = LlmQueryDecomposer::new(Arc::new(registry), "json", "json-model");
        let queries = decomposer
            .decompose("Rust async and Tokio runtime")
            .await
            .unwrap();
        assert_eq!(
            queries,
            vec![
                "Rust async internals".to_string(),
                "Tokio runtime".to_string(),
                "Rust async and Tokio runtime".to_string(),
            ]
        );
    }
    #[tokio::test]
    async fn llm_decomposer_falls_back_to_heuristic_on_bad_json() {
        use ragent_llm::llm::{ChatRequest, LlmClient, StreamEvent};
        use ragent_llm::providers::ProviderRegistry;
        use std::pin::Pin;

        struct BadJsonClient;

        #[async_trait]
        impl LlmClient for BadJsonClient {
            async fn chat(
                &self,
                _request: ChatRequest,
            ) -> anyhow::Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>>
            {
                let events = vec![
                    StreamEvent::TextDelta {
                        text: "not json".into(),
                    },
                    StreamEvent::Finish {
                        reason: ragent_llm::llm::LlmFinishReason::Stop,
                    },
                ];
                Ok(Box::pin(futures::stream::iter(events)))
            }
        }

        struct BadJsonProvider;

        #[async_trait]
        impl ragent_llm::provider::Provider for BadJsonProvider {
            fn id(&self) -> &'static str {
                "badjson"
            }

            fn name(&self) -> &'static str {
                "Bad JSON"
            }

            fn default_models(&self) -> Vec<ragent_llm::provider::ModelInfo> {
                Vec::new()
            }

            async fn create_client(
                &self,
                _api_key: &str,
                _base_url: Option<&str>,
                _options: &std::collections::HashMap<String, serde_json::Value>,
            ) -> anyhow::Result<Box<dyn LlmClient>> {
                Ok(Box::new(BadJsonClient))
            }

            fn set_event_bus(&self, _event_bus: Option<Arc<ragent_types::event::EventBus>>) {}

            fn as_any_static(&self) -> &dyn std::any::Any {
                self
            }
        }

        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(BadJsonProvider));
        let decomposer = LlmQueryDecomposer::new(Arc::new(registry), "badjson", "badjson-model");
        let queries = decomposer
            .decompose("Rust async and Tokio runtime")
            .await
            .unwrap();
        assert!(queries.contains(&"Rust async".to_string()));
        assert!(queries.contains(&"Tokio runtime".to_string()));
        assert!(queries.contains(&"Rust async and Tokio runtime".to_string()));
    }

    /// A fetch tool that sleeps for a fixed duration before returning, and
    /// tracks the maximum number of concurrently in-flight `fetch` calls via
    /// an [`AtomicUsize`].
    struct ConcurrencyTrackingFetch {
        delay: std::time::Duration,
        in_flight: Arc<std::sync::atomic::AtomicUsize>,
        max_in_flight: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait]
    impl WebFetchTool for ConcurrencyTrackingFetch {
        async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
            let prev = self
                .in_flight
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // Track the high-water mark of concurrent in-flight fetches.
            let now = prev + 1;
            let mut max = self.max_in_flight.load(std::sync::atomic::Ordering::SeqCst);
            while now > max {
                match self.max_in_flight.compare_exchange(
                    max,
                    now,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(actual) => max = actual,
                }
            }
            tokio::time::sleep(self.delay).await;
            self.in_flight
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            Ok(WebFetchedPage {
                published_at: None,
                url: _url.to_string(),
                title: format!("title-{_url}"),
                body: format!("body-{_url}"),
                content_type: None,
                page_type: None,
                language: None,
            })
        }
    }

    /// `with_fetch_concurrency(0)` is clamped up to `1` so the stream always
    /// makes progress; the field reflects the clamped value.
    #[test]
    fn with_fetch_concurrency_clamps_zero_to_one() {
        let search: Arc<dyn WebSearchTool> = Arc::new(FakeSearch::default());
        let fetch: Arc<dyn WebFetchTool> = Arc::new(ConcurrencyTrackingFetch {
            delay: std::time::Duration::ZERO,
            in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            max_in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });
        let g = WebGatherer::new(search, fetch).with_fetch_concurrency(0);
        assert_eq!(g.fetch_concurrency, 1);
        let g = g.with_fetch_concurrency(7);
        assert_eq!(g.fetch_concurrency, 7);
    }

    /// The fetch phase of [`WebGatherer::gather_with_observer`] issues up to
    /// `fetch_concurrency` page fetches concurrently. With 6 candidate URLs
    /// and `fetch_concurrency = 6`, all six fetches should be in flight at
    /// once (high-water mark == 6); with `fetch_concurrency = 2` the
    /// high-water mark is capped at 2.
    #[tokio::test]
    async fn gather_fetches_pages_concurrently_up_to_fetch_concurrency() {
        let hits: Vec<WebSearchHit> = (0..6)
            .map(|i| WebSearchHit {
                url: format!("https://h{i}.example"),
                title: format!("H{i}"),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            })
            .collect();

        // fetch_concurrency = 6 → high-water mark should reach 6.
        let in_flight = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max_in_flight = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let search: Arc<dyn WebSearchTool> = Arc::new(FakeSearch {
            hits: hits.clone(),
            calls: Mutex::new(Vec::new()),
        });
        let fetch: Arc<dyn WebFetchTool> = Arc::new(ConcurrencyTrackingFetch {
            delay: std::time::Duration::from_millis(40),
            in_flight,
            max_in_flight: max_in_flight.clone(),
        });
        let g = WebGatherer::new(search, fetch).with_fetch_concurrency(6);
        let sources = g.gather("topic", 6).await.unwrap();
        assert_eq!(sources.len(), 6, "all six hits should be captured");
        assert_eq!(
            max_in_flight.load(std::sync::atomic::Ordering::SeqCst),
            6,
            "all 6 fetches should have been in flight simultaneously"
        );

        // fetch_concurrency = 2 → high-water mark should be capped at 2.
        let in_flight2 = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max_in_flight2 = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let search2: Arc<dyn WebSearchTool> = Arc::new(FakeSearch {
            hits,
            calls: Mutex::new(Vec::new()),
        });
        let fetch2: Arc<dyn WebFetchTool> = Arc::new(ConcurrencyTrackingFetch {
            delay: std::time::Duration::from_millis(40),
            in_flight: in_flight2,
            max_in_flight: max_in_flight2.clone(),
        });
        let g2 = WebGatherer::new(search2, fetch2).with_fetch_concurrency(2);
        let sources2 = g2.gather("topic", 6).await.unwrap();
        assert_eq!(sources2.len(), 6);
        let max2 = max_in_flight2.load(std::sync::atomic::Ordering::SeqCst);
        assert!(
            max2 <= 2,
            "fetch_concurrency=2 should cap in-flight at 2, got {max2}"
        );
        assert_eq!(
            max2, 2,
            "with 6 hits and concurrency 2 the high-water mark should reach 2"
        );
    }

    /// The default `fetch_concurrency` on a freshly-constructed
    /// [`WebGatherer`] is [`DEFAULT_FETCH_CONCURRENCY`].
    #[test]
    fn default_fetch_concurrency_is_ten() {
        let search: Arc<dyn WebSearchTool> = Arc::new(FakeSearch::default());
        let fetch: Arc<dyn WebFetchTool> = Arc::new(ConcurrencyTrackingFetch {
            delay: std::time::Duration::ZERO,
            in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            max_in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });
        let g = WebGatherer::new(search, fetch);
        assert_eq!(g.fetch_concurrency, DEFAULT_FETCH_CONCURRENCY);
        assert_eq!(DEFAULT_FETCH_CONCURRENCY, 10);
    }

    /// `fetch_url_as_source` classifies the media type from the fetched page's
    /// content type so PDF and YouTube seed URLs are reported correctly.
    #[tokio::test]
    async fn fetch_url_as_source_classifies_pdf_and_youtube_media_types() {
        struct TypedFetch {
            pages: std::collections::HashMap<String, WebFetchedPage>,
        }
        #[async_trait]
        impl WebFetchTool for TypedFetch {
            async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
                self.pages
                    .get(url)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("no fake page for {url}"))
            }
        }

        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://example.com/paper.pdf".into(),
            WebFetchedPage {
                url: "https://example.com/paper.pdf".into(),
                title: "Paper".into(),
                body: "extracted pdf text".into(),
                content_type: Some("application/pdf".into()),
                page_type: Some("pdf".into()),
                published_at: None,
                language: None,
            },
        );
        pages.insert(
            "https://www.youtube.com/watch?v=abc123".into(),
            WebFetchedPage {
                url: "https://www.youtube.com/watch?v=abc123".into(),
                title: "Video".into(),
                body: "transcript text".into(),
                content_type: Some("text/html; charset=utf-8".into()),
                page_type: Some("youtube".into()),
                published_at: None,
                language: None,
            },
        );

        let g = WebGatherer::new(
            Arc::new(FakeSearch::default()),
            Arc::new(TypedFetch { pages }),
        );

        let (pdf_source, _) = g
            .fetch_url_as_source("https://example.com/paper.pdf")
            .await
            .unwrap();
        if let Source::Web { media_type, .. } = &pdf_source {
            assert_eq!(media_type, "pdf");
        } else {
            panic!("expected Source::Web for PDF");
        }

        let (yt_source, _) = g
            .fetch_url_as_source("https://www.youtube.com/watch?v=abc123")
            .await
            .unwrap();
        if let Source::Web { media_type, .. } = &yt_source {
            assert_eq!(media_type, "youtube");
        } else {
            panic!("expected Source::Web for YouTube");
        }
    }

    /// `gather` copies the detected language from the fetched page into the
    /// web source so the References Index can render it.
    #[tokio::test]
    async fn gather_propagates_detected_language_to_source() {
        let hits = vec![WebSearchHit {
            url: "https://fr.example".into(),
            title: "Article".into(),
            snippet: "topic Rust async".into(),
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://fr.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://fr.example".into(),
                title: "Article".into(),
                body: "corps de texte".into(),
                content_type: None,
                page_type: None,
                language: Some("French".into()),
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("topic", 5).await.unwrap();
        assert_eq!(sources.len(), 1);
        if let Source::Web { language, .. } = &sources[0] {
            assert_eq!(language.as_deref(), Some("French"));
        } else {
            panic!("expected Source::Web");
        }
    }

    /// `fetch_url_as_source` copies the detected language from the fetched page
    /// into the returned web source.
    #[tokio::test]
    async fn fetch_url_as_source_propagates_detected_language() {
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://es.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://es.example".into(),
                title: "Página".into(),
                body: "cuerpo".into(),
                content_type: None,
                page_type: None,
                language: Some("Spanish".into()),
            },
        );
        let g = WebGatherer::new(
            Arc::new(FakeSearch::default()),
            Arc::new(FakeFetch {
                pages,
                ..Default::default()
            }),
        );
        let (source, _) = g.fetch_url_as_source("https://es.example").await.unwrap();
        if let Source::Web { language, .. } = &source {
            assert_eq!(language.as_deref(), Some("Spanish"));
        } else {
            panic!("expected Source::Web");
        }
    }
    #[tokio::test]
    async fn gather_counts_pdf_and_youtube_sources() {
        let hits = vec![
            WebSearchHit {
                url: "https://example.com/paper.pdf".into(),
                title: "PDF".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "test".into(),
            },
            WebSearchHit {
                url: "https://www.youtube.com/watch?v=abc123".into(),
                title: "YouTube".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "test".into(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://example.com/paper.pdf".into(),
            WebFetchedPage {
                url: "https://example.com/paper.pdf".into(),
                title: "PDF".into(),
                body: "pdf body".into(),
                content_type: Some("application/pdf".into()),
                page_type: Some("pdf".into()),
                published_at: None,
                language: None,
            },
        );
        pages.insert(
            "https://www.youtube.com/watch?v=abc123".into(),
            WebFetchedPage {
                url: "https://www.youtube.com/watch?v=abc123".into(),
                title: "YouTube".into(),
                body: "youtube transcript".into(),
                content_type: Some("text/html".into()),
                page_type: Some("youtube".into()),
                published_at: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let result = g.gather_with_observer("topic", 5, None).await.unwrap();
        assert_eq!(result.pdf_count, 1);
        assert_eq!(result.youtube_count, 1);
        assert_eq!(result.sources.len(), 2);
    }
}

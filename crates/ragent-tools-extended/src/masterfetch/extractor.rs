//! Content extraction chain for masterfetch.
//!
//! Implements FR-002 and NFR-003.
//!
//! This module ports Hound's `extractor.py` content-extraction chain, adapted
//! to ragent's existing extraction crates. The chain is:
//!
//! 1. **`readability-rs`** (primary) — extracts the main article text,
//!    stripping navigation, ads, and page chrome. Used when the page looks
//!    like an article and the extracted text is ≥ [`MIN_READABILITY_CHARS`]
//!    characters.
//! 2. **`html2text`** (fallback) — converts the full HTML to formatted plain
//!    text. Used when readability fails or produces very short output (list
//!    pages, tables, JS shells).
//! 3. **Raw text** (last resort) — strips all HTML tags with
//!    [`ragent_types::html::strip_tags`]. Used when `html2text` panics or
//!    errors.
//!
//! Additional features:
//!
//! - **`css_selector` narrowing** — when provided, the HTML is narrowed to
//!   the first element matching the selector *before* extraction. Supports
//!   tag, `.class`, `#id`, and compound forms (`article.main`, `div#content`).
//!   Implemented without the `scraper` crate (not a workspace dependency) using
//!   a lightweight HTML walker.
//! - **`format` parameter** — `markdown` (default, runs the full chain),
//!   `html` (cleaned HTML with noise tags stripped), `text` (all tags
//!   stripped), `raw` (body returned unchanged).
//!
//! All public functions are pure and take their inputs by reference, enabling
//! unit tests without live network calls (NFR-003).

use thiserror::Error;

/// Minimum extracted text length for readability to be considered successful.
///
/// Below this threshold the extractor falls back to `html2text`, matching
/// ragent's existing `webfetch` behaviour.
pub const MIN_READABILITY_CHARS: usize = 500;

/// Default maximum content size in characters.
pub const DEFAULT_MAX_CONTENT_CHARS: usize = 40_000;

/// Minimum value for `max_content_chars` (FR-002 requires `min: 500`).
pub const MIN_MAX_CONTENT_CHARS: usize = 500;

/// Text width for `html2text` rendering (columns).
const TEXT_WIDTH: usize = 120;

// ---------------------------------------------------------------------------
// Output format
// ---------------------------------------------------------------------------

/// Output format requested by the `mf_fetch` `format` parameter.
///
/// Controls how the extracted content is rendered. See FR-002.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Extracted content as plain text via the readability → html2text → raw
    /// chain (default).
    #[default]
    Markdown,
    /// Cleaned HTML with noise tags (script, style, nav, etc.) stripped.
    Html,
    /// Plain text with all HTML tags removed.
    Text,
    /// Raw response body, unchanged.
    Raw,
}

impl OutputFormat {
    /// Parse a format string from the tool's `format` parameter.
    ///
    /// Returns [`OutputFormat::Markdown`] for unrecognised or empty input,
    /// matching the "default: markdown" behaviour specified in FR-002.
    ///
    /// Not implemented as `std::str::FromStr` because this function returns
    /// a default value on unrecognised input rather than an `Err`.
    #[must_use]
    pub fn parse_format(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "html" => Self::Html,
            "text" => Self::Text,
            "raw" => Self::Raw,
            _ => Self::Markdown,
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Markdown => "markdown",
            Self::Html => "html",
            Self::Text => "text",
            Self::Raw => "raw",
        };
        f.write_str(s)
    }
}

// ---------------------------------------------------------------------------
// Extraction method (which stage of the chain produced the output)
// ---------------------------------------------------------------------------

/// Which stage of the extraction chain produced the output.
///
/// Recorded in [`ExtractResult::method`] for diagnostics and the
/// `fetcher_used` envelope signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtractMethod {
    /// `readability-rs` successfully extracted article text.
    Readability,
    /// `html2text` fallback produced the text.
    #[default]
    Html2Text,
    /// Raw text fallback (tag stripping) produced the text.
    RawText,
    /// Raw HTML was returned unchanged (`format=raw`).
    RawHtml,
    /// Cleaned HTML was returned (`format=html`).
    CleanedHtml,
    /// Plain text was returned via tag stripping (`format=text`).
    StrippedText,
}

impl std::fmt::Display for ExtractMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Readability => "readability",
            Self::Html2Text => "html2text",
            Self::RawText => "raw_text",
            Self::RawHtml => "raw_html",
            Self::CleanedHtml => "cleaned_html",
            Self::StrippedText => "stripped_text",
        };
        f.write_str(s)
    }
}

// ---------------------------------------------------------------------------
// Options and result
// ---------------------------------------------------------------------------

/// Options controlling the extraction chain.
///
/// Built from the `mf_fetch` tool parameters. All fields have sensible
/// defaults via [`ExtractOptions::default`].
#[derive(Debug, Clone)]
pub struct ExtractOptions {
    /// Output format (markdown/html/text/raw). See [`OutputFormat`].
    pub format: OutputFormat,
    /// Optional CSS selector to narrow extraction scope before running the
    /// chain. Supports `tag`, `.class`, `#id`, and compound forms.
    pub css_selector: Option<String>,
    /// Maximum content characters. Output is truncated at a char boundary.
    /// Clamped to [`MIN_MAX_CONTENT_CHARS`] minimum.
    pub max_content_chars: usize,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            format: OutputFormat::Markdown,
            css_selector: None,
            max_content_chars: DEFAULT_MAX_CONTENT_CHARS,
        }
    }
}

/// Result of a content extraction.
///
/// Returned by [`extract`]. The `content` field is the final output string;
/// the other fields carry diagnostic metadata for the fetch envelope.
#[derive(Debug, Clone)]
pub struct ExtractResult {
    /// Extracted text content (markdown, html, text, or raw per `format`).
    pub content: String,
    /// Page title extracted by readability (if available).
    pub title: Option<String>,
    /// Which stage of the extraction chain produced the output.
    pub method: ExtractMethod,
    /// `true` when the content was truncated to fit `max_content_chars`.
    pub is_truncated: bool,
    /// Total characters in the extracted content (before truncation).
    pub total_chars: usize,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during content extraction.
///
/// Extraction is designed to degrade gracefully — the chain falls through to
/// raw text rather than erroring — so most errors are internal diagnostics
/// that surface in [`ExtractMethod`] rather than as `Err` variants. The
/// `InvalidSelector` variant is returned when a CSS selector cannot be
/// parsed.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExtractError {
    /// The CSS selector could not be parsed.
    #[error("invalid CSS selector: {0}")]
    InvalidSelector(String),
}

// ---------------------------------------------------------------------------
// Public extraction entry point
// ---------------------------------------------------------------------------

/// Extract content from an HTML body using the masterfetch extraction chain.
///
/// This is the primary entry point, called by `mf_fetch` (and internally by
/// `mf_crawl`) after the HTTP response body has been retrieved.
///
/// # Arguments
///
/// * `html` — the raw HTML response body.
/// * `url` — the final URL (after redirects), used by readability for
///   relative-link resolution.
/// * `content_type` — the HTTP `Content-Type` header value. If it does not
///   contain `text/html` or `application/xhtml`, the body is returned as-is
///   (raw) regardless of the `format` parameter.
/// * `opts` — extraction options (format, `css_selector`, `max_content_chars`).
///
/// # Returns
///
/// An [`ExtractResult`] containing the extracted content and diagnostic
/// metadata. This function never returns `Err` for extraction failures — it
/// degrades gracefully through the chain. The only `Err` case is
/// [`ExtractError::InvalidSelector`].
///
/// # Example
///
/// ```
/// use ragent_tools_extended::masterfetch::extractor::{extract, ExtractOptions, OutputFormat};
///
/// let html = r#"<html><head><title>Test</title></head>
/// <body><article><p>Hello world. </p></article></body></html>"#;
/// let result = extract(html, "https://example.com", "text/html", &ExtractOptions::default());
/// assert!(!result.unwrap().content.is_empty());
/// ```
pub fn extract(
    html: &str,
    url: &str,
    content_type: &str,
    opts: &ExtractOptions,
) -> Result<ExtractResult, ExtractError> {
    let max_chars = opts.max_content_chars.max(MIN_MAX_CONTENT_CHARS);

    // Non-HTML content: return as-is.
    if !is_html(content_type) {
        let (content, is_truncated) = truncate(html, max_chars);
        return Ok(ExtractResult {
            content,
            title: None,
            method: ExtractMethod::RawHtml,
            is_truncated,
            total_chars: html.chars().count(),
        });
    }

    // Format = raw: return HTML body unchanged.
    if opts.format == OutputFormat::Raw {
        let (content, is_truncated) = truncate(html, max_chars);
        return Ok(ExtractResult {
            content,
            title: None,
            method: ExtractMethod::RawHtml,
            is_truncated,
            total_chars: html.chars().count(),
        });
    }

    // Narrow HTML if a CSS selector was provided.
    let narrowed;
    let source_html: &str = if let Some(ref selector) = opts.css_selector {
        narrowed = narrow_by_selector(html, selector)?;
        narrowed.as_str()
    } else {
        html
    };

    // Dispatch by format.
    match opts.format {
        OutputFormat::Html => {
            let cleaned = strip_noise_tags(source_html);
            let (content, is_truncated) = truncate(&cleaned, max_chars);
            Ok(ExtractResult {
                content,
                title: None,
                method: ExtractMethod::CleanedHtml,
                is_truncated,
                total_chars: cleaned.chars().count(),
            })
        }
        OutputFormat::Text => {
            let text = ragent_types::html::strip_tags(source_html);
            let normalised = collapse_whitespace(&text);
            let (content, is_truncated) = truncate(&normalised, max_chars);
            Ok(ExtractResult {
                content,
                title: None,
                method: ExtractMethod::StrippedText,
                is_truncated,
                total_chars: normalised.chars().count(),
            })
        }
        OutputFormat::Markdown | OutputFormat::Raw => extract_markdown(source_html, url, max_chars),
    }
}

// ---------------------------------------------------------------------------
// Markdown extraction chain (readability → html2text → raw text)
// ---------------------------------------------------------------------------

/// Run the readability → html2text → raw text chain and return the best
/// result.
fn extract_markdown(
    html: &str,
    url: &str,
    max_chars: usize,
) -> Result<ExtractResult, ExtractError> {
    // Strip noise tags (script, style, nav, etc.) before extraction so their
    // text content never leaks into the output.
    let cleaned = strip_noise_tags(html);

    // Stage 1: readability-rs (primary).
    if let Some((text, title)) = extract_readability(&cleaned, url) {
        if text.chars().count() >= MIN_READABILITY_CHARS {
            tracing::debug!(
                chars = text.chars().count(),
                "extractor: readability succeeded"
            );
            let (content, is_truncated) = truncate(&text, max_chars);
            return Ok(ExtractResult {
                content,
                title: Some(title),
                method: ExtractMethod::Readability,
                is_truncated,
                total_chars: text.chars().count(),
            });
        }
        tracing::debug!(
            chars = text.chars().count(),
            min = MIN_READABILITY_CHARS,
            "extractor: readability text too short, falling back to html2text"
        );
    } else {
        tracing::debug!("extractor: readability failed, falling back to html2text");
    }

    // Stage 2: html2text (fallback).
    if let Some(text) = extract_html2text(&cleaned)
        && !text.trim().is_empty()
    {
        let (content, is_truncated) = truncate(&text, max_chars);
        return Ok(ExtractResult {
            content,
            title: None,
            method: ExtractMethod::Html2Text,
            is_truncated,
            total_chars: text.chars().count(),
        });
    }

    // Stage 3: raw text (last resort).
    let text = ragent_types::html::strip_tags(&cleaned);
    let normalised = collapse_whitespace(&text);
    let (content, is_truncated) = truncate(&normalised, max_chars);
    Ok(ExtractResult {
        content,
        title: None,
        method: ExtractMethod::RawText,
        is_truncated,
        total_chars: normalised.chars().count(),
    })
}

/// Extract article text and title using `readability-rs`.
///
/// Returns `None` if readability fails to parse or produces empty text.
/// Isolated in its own function so the caller can fall through to the next
/// stage of the chain.
fn extract_readability(html: &str, url: &str) -> Option<(String, String)> {
    let parsed_url = url::Url::parse(url).ok()?;
    let mut input = std::io::Cursor::new(html.as_bytes());
    let readable = readability::extract(
        &mut input,
        &parsed_url,
        readability::ExtractOptions::default(),
    )
    .ok()?;
    let text = readable.text.trim().to_string();
    let title = readable.title.trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some((text, title))
}

/// Convert HTML to formatted text using `html2text`.
///
/// `html2text` can panic on some malformed HTML documents, so we isolate it
/// in `catch_unwind`. Returns `None` if the call panics or errors.
fn extract_html2text(html: &str) -> Option<String> {
    let result = std::panic::catch_unwind(|| html2text::from_read(html.as_bytes(), TEXT_WIDTH));
    match result {
        Ok(Ok(text)) => Some(text),
        Ok(Err(e)) => {
            tracing::debug!(error = %e, "extractor: html2text errored");
            None
        }
        Err(_) => {
            tracing::debug!("extractor: html2text panicked");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// CSS selector narrowing
// ---------------------------------------------------------------------------

/// A parsed simple CSS selector.
///
/// Supports tag name, class, and id in compound form (e.g. `div.main`,
/// `article#content`, `.post-body`). Does not support descendant or child
/// combinators — the selector matches the *first* element whose tag/class/id
/// all match.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ParsedSelector {
    tag: Option<String>,
    class: Option<String>,
    id: Option<String>,
}

/// Parse a simple CSS selector string.
///
/// Accepts: `tag`, `.class`, `#id`, `tag.class`, `tag#id`, `#id.class`,
/// `tag.class#id`, etc. Returns [`ExtractError::InvalidSelector`] for empty
/// selectors or selectors containing unsupported combinators (`>`, `+`, `~`,
/// ` ` descendant, `,` multiple).
fn parse_selector(selector: &str) -> Result<ParsedSelector, ExtractError> {
    let trimmed = selector.trim();
    if trimmed.is_empty() {
        return Err(ExtractError::InvalidSelector(
            "selector is empty".to_string(),
        ));
    }

    // Reject combinators and multiple selectors.
    if trimmed.contains(['>', '+', '~', ',']) {
        return Err(ExtractError::InvalidSelector(format!(
            "combinators ('>', '+', '~', ',') are not supported: {trimmed}"
        )));
    }
    if trimmed.contains(' ') {
        return Err(ExtractError::InvalidSelector(format!(
            "descendant combinator (space) is not supported: {trimmed}"
        )));
    }

    let mut result = ParsedSelector::default();
    let mut current = String::new();
    let mut mode = SelectorMode::Tag;

    for ch in trimmed.chars() {
        match ch {
            '.' => {
                if !current.is_empty() {
                    set_selector_part(&mut result, mode, &current);
                    current.clear();
                }
                mode = SelectorMode::Class;
            }
            '#' => {
                if !current.is_empty() {
                    set_selector_part(&mut result, mode, &current);
                    current.clear();
                }
                mode = SelectorMode::Id;
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        set_selector_part(&mut result, mode, &current);
    }

    if result.tag.is_none() && result.class.is_none() && result.id.is_none() {
        return Err(ExtractError::InvalidSelector(format!(
            "selector has no tag, class, or id: {trimmed}"
        )));
    }

    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectorMode {
    Tag,
    Class,
    Id,
}

fn set_selector_part(result: &mut ParsedSelector, mode: SelectorMode, value: &str) {
    match mode {
        SelectorMode::Tag => result.tag = Some(value.to_ascii_lowercase()),
        SelectorMode::Class => result.class = Some(value.to_string()),
        SelectorMode::Id => result.id = Some(value.to_string()),
    }
}

/// Narrow HTML to the inner content of the first element matching `selector`.
///
/// Returns the full HTML unchanged if no matching element is found (graceful
/// degradation — better to return full content than nothing).
fn narrow_by_selector(html: &str, selector: &str) -> Result<String, ExtractError> {
    let parsed = parse_selector(selector)?;
    let narrowed = extract_element_inner(html, &parsed);
    if narrowed.is_empty() {
        tracing::debug!(
            selector,
            "extractor: no element matched CSS selector, returning full HTML"
        );
        return Ok(html.to_string());
    }
    tracing::debug!(selector, bytes = narrowed.len(), "extractor: narrowed HTML");
    Ok(narrowed)
}

/// Walk the HTML and extract the inner content of the first element matching
/// the selector.
///
/// This is a lightweight tag-walking parser — not a full DOM. It tracks
/// opening and closing tags at the matching depth to extract everything
/// between the first matching opening tag and its corresponding closing tag.
fn extract_element_inner(html: &str, selector: &ParsedSelector) -> String {
    let tokens = tokenize_tags(html);
    let mut depth = 0i32;
    let mut matching_depth = -1i32;
    let mut result = String::new();

    for token in &tokens {
        match token {
            HtmlToken::OpenTag { name, attrs } => {
                if matching_depth < 0 && element_matches(name, attrs, selector) {
                    matching_depth = depth;
                    // Start capturing after this opening tag.
                }
                if matching_depth >= 0 && depth > matching_depth {
                    // Re-emit nested opening tags inside the matched element.
                    result.push('<');
                    result.push_str(name);
                    emit_attrs(&mut result, attrs);
                    result.push('>');
                }
                depth += 1;
            }
            HtmlToken::SelfClosingTag { name, attrs } => {
                if matching_depth >= 0 && depth > matching_depth {
                    result.push('<');
                    result.push_str(name);
                    emit_attrs(&mut result, attrs);
                    result.push_str(" />");
                }
                // Self-closing tags don't affect depth.
            }
            HtmlToken::CloseTag { name } => {
                depth -= 1;
                if matching_depth >= 0 && depth == matching_depth {
                    // We've reached the closing tag of the matched element.
                    return result;
                }
                if matching_depth >= 0 && depth > matching_depth {
                    result.push_str("</");
                    result.push_str(name);
                    result.push('>');
                }
            }
            HtmlToken::Text(text) => {
                if matching_depth >= 0 && depth > matching_depth {
                    result.push_str(text);
                }
            }
        }
    }
    result
}

/// Check if an HTML element matches the parsed selector.
fn element_matches(tag: &str, attrs: &[HtmlAttr], selector: &ParsedSelector) -> bool {
    if let Some(ref sel_tag) = selector.tag
        && tag != sel_tag
    {
        return false;
    }
    if let Some(ref sel_class) = selector.class {
        let has_class = attrs
            .iter()
            .any(|a| a.name == "class" && a.value.split_whitespace().any(|c| c == sel_class));
        if !has_class {
            return false;
        }
    }
    if let Some(ref sel_id) = selector.id {
        let has_id = attrs.iter().any(|a| a.name == "id" && a.value == *sel_id);
        if !has_id {
            return false;
        }
    }
    true
}

fn emit_attrs(result: &mut String, attrs: &[HtmlAttr]) {
    for attr in attrs {
        result.push(' ');
        result.push_str(&attr.name);
        if !attr.value.is_empty() {
            result.push_str("=\"");
            result.push_str(&attr.value);
            result.push('"');
        }
    }
}

// ---------------------------------------------------------------------------
// Lightweight HTML tokenizer
// ---------------------------------------------------------------------------

/// A token produced by the lightweight HTML tokenizer.
#[derive(Debug, Clone)]
pub(crate) enum HtmlToken<'a> {
    /// An opening tag: `<div class="foo">`.
    OpenTag { name: &'a str, attrs: Vec<HtmlAttr> },
    /// A self-closing tag: `<br />`, `<img ... />`.
    SelfClosingTag { name: &'a str, attrs: Vec<HtmlAttr> },
    /// A closing tag: `</div>`.
    CloseTag { name: &'a str },
    /// Raw text content between tags.
    Text(&'a str),
}

/// An HTML attribute name/value pair.
#[derive(Debug, Clone)]
pub(crate) struct HtmlAttr {
    pub name: String,
    pub value: String,
}

/// Tokenize HTML into a sequence of [`HtmlToken`]s.
///
/// This is a minimal tokenizer sufficient for CSS selector matching and noise
/// tag stripping. It does not build a full DOM — it produces a flat sequence
/// of opening tags, closing tags, self-closing tags, and text nodes.
pub(crate) fn tokenize_tags(html: &str) -> Vec<HtmlToken<'_>> {
    let mut tokens = Vec::new();
    let bytes = html.as_bytes();
    let mut pos = 0;
    let mut text_start = 0;

    while pos < bytes.len() {
        if bytes[pos] == b'<' {
            // Emit any accumulated text.
            if pos > text_start {
                let text = &html[text_start..pos];
                if !text.trim().is_empty() {
                    tokens.push(HtmlToken::Text(text));
                }
            }
            // Find the closing '>'.
            if let Some(end) = find_tag_end(html, pos) {
                let tag_content = &html[pos + 1..end];
                if let Some(token) = parse_tag(tag_content) {
                    tokens.push(token);
                }
                pos = end + 1;
                text_start = pos;
            } else {
                // No closing '>' — treat the rest as text.
                break;
            }
        } else {
            pos += 1;
        }
    }
    if pos > text_start && text_start < html.len() {
        let text = &html[text_start..];
        if !text.trim().is_empty() {
            tokens.push(HtmlToken::Text(text));
        }
    }
    tokens
}

/// Find the position of the `>` that closes the tag starting at `start`.
fn find_tag_end(html: &str, start: usize) -> Option<usize> {
    let bytes = html.as_bytes();
    let mut pos = start + 1;
    let mut in_quote = b'\0';
    while pos < bytes.len() {
        let ch = bytes[pos];
        if in_quote != 0 {
            if ch == in_quote {
                in_quote = 0;
            }
        } else if ch == b'"' || ch == b'\'' {
            in_quote = ch;
        } else if ch == b'>' {
            return Some(pos);
        }
        pos += 1;
    }
    None
}

/// Parse the content between `<` and `>` into a token.
fn parse_tag(content: &str) -> Option<HtmlToken<'_>> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Closing tag: </div>
    if let Some(rest) = trimmed.strip_prefix('/') {
        let name = rest.trim().to_ascii_lowercase();
        if name.is_empty() {
            return None;
        }
        return Some(HtmlToken::CloseTag {
            name: leak_str(&name, rest),
        });
    }
    // Comment or doctype: skip.
    if trimmed.starts_with("!--") || trimmed.starts_with('!') || trimmed.starts_with('?') {
        return None;
    }
    // Opening or self-closing tag.
    let is_self_closing = trimmed.ends_with('/');
    let inner = if is_self_closing {
        &trimmed[..trimmed.len() - 1]
    } else {
        trimmed
    };
    let (name_part, attrs_part) = split_tag_name(inner);
    let name = name_part.to_ascii_lowercase();
    let attrs = parse_attrs(attrs_part);
    // Determine self-closing: explicit `/` or known void elements.
    let void = matches!(
        name.as_str(),
        "br" | "hr"
            | "img"
            | "input"
            | "meta"
            | "link"
            | "area"
            | "base"
            | "col"
            | "embed"
            | "source"
            | "track"
            | "wbr"
    );
    if is_self_closing || void {
        Some(HtmlToken::SelfClosingTag {
            name: leak_str(&name, name_part),
            attrs,
        })
    } else {
        Some(HtmlToken::OpenTag {
            name: leak_str(&name, name_part),
            attrs,
        })
    }
}

/// Return a `&'a str` referencing the original slice that matches `value`.
///
/// We need the token to borrow from the input HTML, but we've computed a
/// lowercased name. We search the original tag-name slice for a match — since
/// tag names are ASCII, the lowercased version will appear in the original if
/// it was already lowercase, or we fall back to a case-insensitive search.
fn leak_str<'a>(value: &str, original: &'a str) -> &'a str {
    // Try exact match first (common case: already lowercase).
    if let Some(pos) = original.find(value) {
        return &original[pos..pos + value.len()];
    }
    // Case-insensitive search.
    let original_lower = original.to_ascii_lowercase();
    if let Some(pos) = original_lower.find(value) {
        return &original[pos..pos + value.len()];
    }
    // Fallback: return the original (shouldn't happen for valid HTML).
    original
}

/// Split a tag's inner content into the tag name and the attribute string.
fn split_tag_name(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    let end = s.find(|c: char| c.is_whitespace()).unwrap_or(s.len());
    let (name, rest) = s.split_at(end);
    (name, rest.trim_start())
}

/// Parse the attribute portion of a tag into a list of [`HtmlAttr`]s.
fn parse_attrs(s: &str) -> Vec<HtmlAttr> {
    let mut attrs = Vec::new();
    let mut chars = s.char_indices().peekable();
    while let Some(&(i, ch)) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        // Read attribute name.
        let name_start = i;
        while let Some(&(_, ch)) = chars.peek() {
            if ch == '=' || ch.is_whitespace() || ch == '/' {
                break;
            }
            chars.next();
        }
        let name_end = chars.peek().map_or(s.len(), |&(j, _)| j);
        let name = &s[name_start..name_end];
        if name.is_empty() {
            break;
        }
        // Skip whitespace before '='.
        while let Some(&(_, ch)) = chars.peek() {
            if ch.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }
        // Read value if '=' is present.
        let value = if chars.peek().map(|&(_, ch)| ch) == Some('=') {
            chars.next();
            while let Some(&(_, ch)) = chars.peek() {
                if ch.is_whitespace() {
                    chars.next();
                } else {
                    break;
                }
            }
            if let Some('"' | '\'') = chars.peek().map(|&(_, ch)| ch) {
                let quote = chars.next().unwrap().1;
                let val_start = chars.peek().map_or(s.len(), |&(j, _)| j);
                while let Some(&(_, ch)) = chars.peek() {
                    if ch == quote {
                        break;
                    }
                    chars.next();
                }
                let val_end = chars.peek().map_or(s.len(), |&(j, _)| j);
                if chars.peek().is_some() {
                    chars.next(); // consume closing quote
                }
                s[val_start..val_end].to_string()
            } else {
                let val_start = chars.peek().map_or(s.len(), |&(j, _)| j);
                while let Some(&(_, ch)) = chars.peek() {
                    if ch.is_whitespace() || ch == '/' {
                        break;
                    }
                    chars.next();
                }
                let val_end = chars.peek().map_or(s.len(), |&(j, _)| j);
                s[val_start..val_end].to_string()
            }
        } else {
            String::new()
        };
        attrs.push(HtmlAttr {
            name: name.to_ascii_lowercase(),
            value,
        });
    }
    attrs
}

// ---------------------------------------------------------------------------
// HTML cleaning utilities
// ---------------------------------------------------------------------------

/// HTML tags that are noise and should be stripped from cleaned HTML output.
const NOISE_TAGS: &[&str] = &[
    "script", "style", "nav", "footer", "aside", "noscript", "iframe", "svg",
];

/// Strip noise tags (script, style, nav, footer, etc.) from HTML, returning
/// the cleaned HTML with everything else preserved.
fn strip_noise_tags(html: &str) -> String {
    let tokens = tokenize_tags(html);
    let mut result = String::with_capacity(html.len());
    let mut skip_depth = 0i32;

    for token in &tokens {
        match token {
            HtmlToken::OpenTag { name, attrs } => {
                if skip_depth > 0 {
                    skip_depth += 1;
                } else if NOISE_TAGS.contains(name) {
                    skip_depth = 1;
                } else {
                    result.push('<');
                    result.push_str(name);
                    emit_attrs(&mut result, attrs);
                    result.push('>');
                }
            }
            HtmlToken::SelfClosingTag { name, attrs } => {
                if skip_depth == 0 && !NOISE_TAGS.contains(name) {
                    result.push('<');
                    result.push_str(name);
                    emit_attrs(&mut result, attrs);
                    result.push_str(" />");
                }
            }
            HtmlToken::CloseTag { name } => {
                if skip_depth > 0 {
                    skip_depth -= 1;
                } else {
                    result.push_str("</");
                    result.push_str(name);
                    result.push('>');
                }
            }
            HtmlToken::Text(text) => {
                if skip_depth == 0 {
                    result.push_str(text);
                }
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Check if a `Content-Type` header value indicates HTML content.
fn is_html(content_type: &str) -> bool {
    let ct = content_type.to_ascii_lowercase();
    ct.contains("text/html") || ct.contains("application/xhtml")
}

/// Truncate a string at a character boundary, appending a truncation marker.
///
/// Returns `(truncated_string, was_truncated)`.
fn truncate(s: &str, max_chars: usize) -> (String, bool) {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        return (s.to_string(), false);
    }
    let end = s
        .char_indices()
        .map(|(i, _)| i)
        .take(max_chars)
        .last()
        .unwrap_or(0);
    let mut truncated = s[..end].to_string();
    truncated.push_str("\n\n[Content truncated]");
    (truncated, true)
}

/// Collapse multiple whitespace characters into single spaces and trim.
fn collapse_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                result.push(' ');
            }
            prev_ws = true;
        } else {
            result.push(ch);
            prev_ws = false;
        }
    }
    result.trim().to_string()
}

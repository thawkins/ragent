//! Publication-date extraction from raw HTML pages.
//!
//! The research web-gathering phase records a publication date for each
//! captured web source so `RESEARCH.md` can show the age of each reference
//! and a date range per finding (FR-011 enhancement).
//!
//! Pages expose publication dates through a variety of conventions. This
//! module checks them in rough order of reliability:
//!
//! 1. **JSON-LD** `"datePublished"` / `"dateCreated"` in a
//!    `<script type="application/ld+json">` block.
//! 2. **`OpenGraph` / article meta tags** - `article:published_time`,
//!    `og:article:published_time`, `og:published_time`, and the generic
//!    `pubdate` / `date` / `publishdate` / `dc.date` names.
//! 3. **`<time datetime="...">`** elements.
//! 4. **Visible date patterns** - a last resort that scans the first few
//!    lines of rendered text for a `YYYY-MM-DD` or `Month D, YYYY` token.
//!
//! All parsing is defensive: malformed input simply yields `None` rather
//! than panicking, so a single bad page never aborts a research run.

use chrono::{DateTime, NaiveDate, Utc};
use regex::Regex;
use std::sync::LazyLock;

use ragent_types::html::strip_tags;

// ---------------------------------------------------------------------------
// Hoisted regexes (PERF-065)
//
// All seven patterns are per-invocation today; compile them once per process
// instead (the convention already used in `title.rs` and `analysis/parser.rs`).
// ---------------------------------------------------------------------------

/// `<script type="application/ld+json">...</script>` block (body in group 1).
static JSONLD_SCRIPT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<script[^>]*type=["']application/ld\+json["'][^>]*>(.*?)</script>"#)
        // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
        .expect("valid json-ld regex")
});

/// JSON `"datePublished"|"dateCreated"|"dateModified"` value token.
static JSON_DATE_KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)"date(?:Published|Created|Modified)"\s*:\s*"([^"]+)""#)
        // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
        .expect("valid date-key regex")
});

/// Opening `<meta ...>` tag carrying a `property`/`name` attribute (group 1).
static META_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<meta\s+[^>]*(?:property|name)\s*=\s*["']([^"']+)["'][^>]*>"#)
        // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
        .expect("valid meta regex")
});

/// `content="..."` attribute value.
static CONTENT_ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
    // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
    Regex::new(r#"(?i)content\s*=\s*["']([^"']*)["']"#).expect("valid content regex")
});

/// `<time datetime="...">` element (group 1).
static TIME_DATETIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
    Regex::new(r#"(?i)<time[^>]*datetime\s*=\s*["']([^"']+)["']"#).expect("valid time regex")
});

/// ISO date token `YYYY-MM-DD`.
static ISO_DATE_RE: LazyLock<Regex> =
    // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
    LazyLock::new(|| Regex::new(r"\b(\d{4})-(\d{2})-(\d{2})\b").expect("valid iso regex"));

/// Long-form date token `Month D, YYYY` / `D Month YYYY`.
static LONG_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
    Regex::new(r"(?i)\b([A-Za-z]{3,9})\s+(\d{1,2}),?\s+(\d{4})\b").expect("valid long regex")
});

/// Extract the most likely publication date from a raw HTML document.
///
/// Returns `None` when no parseable date can be found. The returned
/// timestamp is always in UTC; dates without a time component are mapped
/// to midnight UTC of that day.
#[must_use]
pub fn extract_published_at(html: &str) -> Option<DateTime<Utc>> {
    // 1. JSON-LD blocks.
    if let Some(dt) = extract_from_json_ld(html) {
        return Some(dt);
    }
    // 2. Meta tags.
    if let Some(dt) = extract_from_meta(html) {
        return Some(dt);
    }
    // 3. <time datetime="..."> elements.
    if let Some(dt) = extract_from_time_elements(html) {
        return Some(dt);
    }
    // 4. Visible date patterns near the top of the rendered text.
    extract_from_visible_text(html)
}

/// Pull `datePublished` / `dateCreated` out of any `<script
/// type="application/ld+json">` block.
fn extract_from_json_ld(html: &str) -> Option<DateTime<Utc>> {
    for cap in JSONLD_SCRIPT_RE.captures_iter(html) {
        let raw = cap.get(1)?.as_str();
        // The JSON-LD block may be a single object or an array; try both,
        // and tolerate trailing commas / wrapped strings.
        if let Some(dt) = find_date_in_json_value(raw) {
            return Some(dt);
        }
    }
    None
}

/// Search a raw JSON string for `datePublished` or `dateCreated` values and
/// parse the first hit. Handles both object and array forms.
fn find_date_in_json_value(raw: &str) -> Option<DateTime<Utc>> {
    for cap in JSON_DATE_KEY_RE.captures_iter(raw) {
        let val = cap.get(1)?.as_str();
        if let Some(dt) = parse_date_string(val) {
            return Some(dt);
        }
    }
    None
}

/// Extract from `<meta>` tags. Checks a prioritised list of `property`/`name`
/// attribute values.
fn extract_from_meta(html: &str) -> Option<DateTime<Utc>> {
    // Ordered roughly by reliability for article publication dates.
    const KEYS: &[&str] = &[
        "article:published_time",
        "article:published",
        "og:article:published_time",
        "og:published_time",
        "pubdate",
        "publishdate",
        "publication_date",
        "dc.date",
        "dc.date.issued",
        "sailthru.date",
        "date",
    ];
    // Match <meta property="KEY" content="VAL"> OR <meta name="KEY" content="VAL">.
    // HTML attributes are case-insensitive; the regex uses the inline `(?i)`
    // flag and allows single or double quotes.
    for meta_cap in META_TAG_RE.captures_iter(html) {
        let attr_key = meta_cap.get(1)?.as_str().to_lowercase();
        if KEYS.iter().any(|k| *k == attr_key) {
            // Re-scan this meta tag for a content="..." attribute.
            let full = meta_cap.get(0)?.as_str();
            if let Some(content) = extract_content_attr(full)
                && let Some(dt) = parse_date_string(&content)
            {
                return Some(dt);
            }
        }
    }
    None
}

/// Pull the `content="..."` attribute value out of a single `<meta ...>` tag.
fn extract_content_attr(tag: &str) -> Option<String> {
    CONTENT_ATTR_RE
        .captures(tag)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Extract from `<time datetime="...">` elements, preferring the first one
/// (which is typically the article publication time in article markup).
fn extract_from_time_elements(html: &str) -> Option<DateTime<Utc>> {
    for cap in TIME_DATETIME_RE.captures_iter(html) {
        let val = cap.get(1)?.as_str();
        if let Some(dt) = parse_date_string(val) {
            return Some(dt);
        }
    }
    None
}

/// Last-resort: scan the first ~500 chars of visible text for a date-like
/// token. Strips tags crudely first.
fn extract_from_visible_text(html: &str) -> Option<DateTime<Utc>> {
    let text = strip_tags(html);
    let head: String = text.chars().take(500).collect();
    // ISO date first: YYYY-MM-DD.
    if let Some(cap) = ISO_DATE_RE.captures(&head) {
        let s = format!("{}-{}-{}", &cap[1], &cap[2], &cap[3]);
        if let Some(dt) = parse_date_string(&s) {
            return Some(dt);
        }
    }
    // Long-form: "Month D, YYYY" or "D Month YYYY". Case-insensitive on the
    // month name so "january" / "JANUARY" also match.
    if let Some(cap) = LONG_DATE_RE.captures(&head) {
        let month = cap[1].to_lowercase();
        let day = &cap[2];
        let year = &cap[3];
        // Map the common English month names to their numeric form so we can
        // use a numeric strptime format that ignores locale.
        if let Some(m) = month_name_to_number(&month) {
            let s = format!("{year}-{m:02}-{day}");
            if let Some(dt) = parse_date_string(&s) {
                return Some(dt);
            }
        }
        // Also try the human parser directly for completeness.
        let s = format!("{} {}, {}", &cap[1], day, year);
        if let Some(dt) = parse_human_date(&s) {
            return Some(dt);
        }
    }
    None
}

/// Parse a date or datetime string into a UTC `DateTime`.
///
/// Uses [`ragent_types::html::strip_tags`] to remove HTML markup before
/// attempting date extraction (DUPPLAN.md Milestone F).
///
/// Tries, in order:
///
/// 1. RFC3339 / ISO 8601 with timezone (`chrono` `parse_from_rfc3339`).
/// 2. A bare `YYYY-MM-DD` (mapped to midnight UTC).
/// 3. A `YYYY/MM/DD` or `YYYY.MM.DD` variant.
/// 4. A human-readable `"Month D, YYYY"` / `"D Month YYYY"` form.
fn parse_date_string(s: &str) -> Option<DateTime<Utc>> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    // 1. RFC3339 (handles trailing Z, offsets, and fractional seconds).
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(dt.with_timezone(&Utc));
    }
    // 2. Bare ISO date.
    if let Ok(nd) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return Some(nd.and_hms_opt(0, 0, 0)?.and_utc());
    }
    // 3. Slash / dot variants of ISO date.
    for sep in ['/', '.'] {
        let fmt = format!("%Y{sep}%m{sep}%d");
        if let Ok(nd) = NaiveDate::parse_from_str(trimmed, &fmt) {
            return Some(nd.and_hms_opt(0, 0, 0)?.and_utc());
        }
    }
    // 4. Human forms.
    parse_human_date(trimmed)
}

/// Parse human-readable date forms such as `"January 15, 2024"` or
/// `"15 January 2024"`.
fn parse_human_date(s: &str) -> Option<DateTime<Utc>> {
    let trimmed = s.trim();
    for fmt in &[
        "%B %d, %Y",
        "%B %d %Y",
        "%b %d, %Y",
        "%b %d %Y",
        "%d %B %Y",
        "%d %b %Y",
    ] {
        if let Ok(nd) = NaiveDate::parse_from_str(trimmed, fmt) {
            return Some(nd.and_hms_opt(0, 0, 0)?.and_utc());
        }
    }
    None
}

/// Map a lowercase English month name (full or abbreviated) to its 1-based
/// number. Returns `None` for unrecognised names.
fn month_name_to_number(name: &str) -> Option<u32> {
    match name {
        "january" | "jan" => Some(1),
        "february" | "feb" => Some(2),
        "march" | "mar" => Some(3),
        "april" | "apr" => Some(4),
        "may" => Some(5),
        "june" | "jun" => Some(6),
        "july" | "jul" => Some(7),
        "august" | "aug" => Some(8),
        "september" | "sep" | "sept" => Some(9),
        "october" | "oct" => Some(10),
        "november" | "nov" => Some(11),
        "december" | "dec" => Some(12),
        _ => None,
    }
}

#[cfg(test)]
#[path = "../tests/inline/web_date_tests.rs"]
mod tests;

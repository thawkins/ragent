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
//! 2. **OpenGraph / article meta tags** — `article:published_time`,
//!    `og:article:published_time`, `og:published_time`, and the generic
//!    `pubdate` / `date` / `publishdate` / `dc.date` names.
//! 3. **`<time datetime="...">`** elements.
//! 4. **Visible date patterns** — a last resort that scans the first few
//!    lines of rendered text for a `YYYY-MM-DD` or `Month D, YYYY` token.
//!
//! All parsing is defensive: malformed input simply yields `None` rather
//! than panicking, so a single bad page never aborts a research run.

use chrono::{DateTime, NaiveDate, Utc};
use regex::Regex;

use ragent_types::html::strip_tags;

/// Extract the most likely publication date from a raw HTML document.
///
/// Returns `None` when no parseable date can be found. The returned
/// timestamp is always in UTC; dates without a time component are mapped
/// to midnight UTC of that day.
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
    let re =
        Regex::new(r#"(?is)<script[^>]*type=["']application/ld\+json["'][^>]*>(.*?)</script>"#)
            .expect("valid json-ld regex");
    for cap in re.captures_iter(html) {
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
    let key_re = Regex::new(r#"(?i)"date(?:Published|Created|Modified)"\s*:\s*"([^"]+)""#)
        .expect("valid date-key regex");
    for cap in key_re.captures_iter(raw) {
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
    let meta_re = Regex::new(r#"(?is)<meta\s+[^>]*(?:property|name)\s*=\s*["']([^"']+)["'][^>]*>"#)
        .expect("valid meta regex");
    for meta_cap in meta_re.captures_iter(html) {
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
    let re = Regex::new(r#"(?i)content\s*=\s*["']([^"']*)["']"#).expect("valid content regex");
    re.captures(tag)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Extract from `<time datetime="...">` elements, preferring the first one
/// (which is typically the article publication time in article markup).
fn extract_from_time_elements(html: &str) -> Option<DateTime<Utc>> {
    let re =
        Regex::new(r#"(?i)<time[^>]*datetime\s*=\s*["']([^"']+)["']"#).expect("valid time regex");
    for cap in re.captures_iter(html) {
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
    let iso_re = Regex::new(r"\b(\d{4})-(\d{2})-(\d{2})\b").expect("valid iso regex");
    if let Some(cap) = iso_re.captures(&head) {
        let s = format!("{}-{}-{}", &cap[1], &cap[2], &cap[3]);
        if let Some(dt) = parse_date_string(&s) {
            return Some(dt);
        }
    }
    // Long-form: "Month D, YYYY" or "D Month YYYY". Case-insensitive on the
    // month name so "january" / "JANUARY" also match.
    let long_re =
        Regex::new(r"(?i)\b([A-Za-z]{3,9})\s+(\d{1,2}),?\s+(\d{4})\b").expect("valid long regex");
    if let Some(cap) = long_re.captures(&head) {
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
mod tests {
    use super::*;

    fn web_dt(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
    }

    // Small chrono arithmetic helpers for readable test assertions.
    trait DtArith {
        fn plus_hours(self, h: u32) -> DateTime<Utc>;
        fn plus_mins(self, m: u32) -> DateTime<Utc>;
    }
    impl DtArith for DateTime<Utc> {
        fn plus_hours(self, h: u32) -> DateTime<Utc> {
            self + chrono::Duration::hours(h as i64)
        }
        fn plus_mins(self, m: u32) -> DateTime<Utc> {
            self + chrono::Duration::minutes(m as i64)
        }
    }

    #[test]
    fn test_extracts_from_article_published_time_meta() {
        let html = r#"<html><head>
            <meta property="article:published_time" content="2024-03-22T10:30:00Z">
            </head><body>hi</body></html>"#;
        assert_eq!(
            extract_published_at(html),
            Some(web_dt(2024, 3, 22).plus_hours(10).plus_mins(30))
        );
    }

    #[test]
    fn test_extracts_from_json_ld_date_published() {
        let html = r#"<html><head>
            <script type="application/ld+json">
            {"@type":"Article","datePublished":"2023-11-05T08:00:00+00:00"}
            </script>
            </head><body>x</body></html>"#;
        assert_eq!(
            extract_published_at(html),
            Some(web_dt(2023, 11, 5).plus_hours(8))
        );
    }

    #[test]
    fn test_extracts_from_json_ld_array() {
        let html = r#"<html><head>
            <script type="application/ld+json">
            [{"@type":"NewsArticle"},{"datePublished":"2022-01-10"}]
            </script>
            </head><body>x</body></html>"#;
        assert_eq!(extract_published_at(html), Some(web_dt(2022, 1, 10)));
    }

    #[test]
    fn test_extracts_from_time_element() {
        let html = r#"<html><body>
            <time datetime="2021-06-07T12:00:00Z">June 7, 2021</time>
            </body></html>"#;
        assert_eq!(
            extract_published_at(html),
            Some(web_dt(2021, 6, 7).plus_hours(12))
        );
    }

    #[test]
    fn test_falls_back_to_visible_iso_date() {
        let html = "<html><body><p>Published on 2020-09-01 by example.</p></body></html>";
        assert_eq!(extract_published_at(html), Some(web_dt(2020, 9, 1)));
    }

    #[test]
    fn test_falls_back_to_human_date() {
        let html = "<html><body><h1>Report</h1><p>January 15, 2019 — summary.</p></body></html>";
        assert_eq!(extract_published_at(html), Some(web_dt(2019, 1, 15)));
    }

    #[test]
    fn test_returns_none_when_no_date_present() {
        let html = "<html><body><p>No dates here at all.</p></body></html>";
        assert_eq!(extract_published_at(html), None);
    }

    #[test]
    fn test_returns_none_for_malformed_meta_content() {
        let html = r#"<meta property="article:published_time" content="not-a-date">"#;
        assert_eq!(extract_published_at(html), None);
    }

    #[test]
    fn test_handles_pubdate_meta_name() {
        let html = r#"<meta name="pubdate" content="2018-04-02T05:00:00Z">"#;
        assert_eq!(
            extract_published_at(html),
            Some(web_dt(2018, 4, 2).plus_hours(5))
        );
    }

    #[test]
    fn test_bare_iso_date_without_time() {
        assert_eq!(parse_date_string("2024-12-25"), Some(web_dt(2024, 12, 25)));
    }

    #[test]
    fn test_slash_and_dot_iso_variants() {
        assert_eq!(parse_date_string("2024/12/25"), Some(web_dt(2024, 12, 25)));
        assert_eq!(parse_date_string("2024.12.25"), Some(web_dt(2024, 12, 25)));
    }

    #[test]
    fn test_prefers_json_ld_over_meta() {
        // JSON-LD is checked first.
        let html = r#"<html><head>
            <meta property="article:published_time" content="2099-01-01">
            <script type="application/ld+json">{"datePublished":"2023-05-05"}</script>
            </head></html>"#;
        assert_eq!(extract_published_at(html), Some(web_dt(2023, 5, 5)));
    }
}

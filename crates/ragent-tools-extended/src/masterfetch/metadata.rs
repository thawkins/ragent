//! Structured metadata extraction for masterfetch.
//!
//! Implements FR-006 and NFR-003.
//!
//! This module extracts structured metadata from HTML pages using three
//! sources, in priority order:
//!
//! 1. **`OpenGraph` meta tags** — `<meta property="og:title" content="...">`,
//!    `og:description`, `og:site_name`, `og:type`, `og:image`,
//!    `article:published_time`, `article:modified_time`, `article:author`.
//! 2. **JSON-LD blocks** — `<script type="application/ld+json">` containing
//!    schema.org objects. Supports `Article`, `NewsArticle`, `BlogPosting`,
//!    `TechArticle`, and generic objects. Extracts `headline` (title),
//!    `description`, `datePublished`, `dateModified`, `author`, `image`.
//! 3. **Standard HTML fallbacks** — `<title>` tag, `<meta name="description">`,
//!    `<link rel="canonical">`, `<html lang="...">`.
//!
//! `OpenGraph` values take priority over JSON-LD, which takes priority over
//! standard HTML. This matches Hound's `metadata.py` merge strategy.
//!
//! All functions are pure — no network I/O — enabling unit tests without live
//! pages (NFR-003).

use regex::Regex;

use super::PageMetadata;

/// Meta name keys checked for Dublin Core creator information, in priority
/// order. Multiple authors are concatenated with `", "`.
const DC_CREATOR_KEYS: &[&str] = &[
    "dc.creator",
    "dcterms.creator",
    "dc:creator",
    "dcterms:creator",
    "dc.creator.personalname",
];

/// Standard `<meta name="...">` keys checked for byline information.
const STANDARD_AUTHOR_KEYS: &[&str] = &[
    "author",
    "citation_author",
    "byl",
    "parsely-author",
    "sailthru.author",
    "twitter:creator",
];

/// Find the first non-empty value for any of the given meta keys.
fn meta_values(metas: &[(String, String)], keys: &[&str]) -> Vec<String> {
    metas
        .iter()
        .filter(|(k, _)| keys.iter().any(|key| k.eq_ignore_ascii_case(key)))
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// Public extraction entry point
// ---------------------------------------------------------------------------

/// Extract structured metadata from an HTML page (FR-006).
///
/// This is a pure function: it parses the HTML string and returns a
/// [`PageMetadata`] struct with all recoverable fields populated. Fields that
/// cannot be found remain `None`.
///
/// The extraction sources, in priority order, are:
///
/// 1. `OpenGraph` meta tags (`og:*`, `article:*`)
/// 2. JSON-LD `<script type="application/ld+json">` blocks
/// 3. Standard HTML fallbacks (`<title>`, `<meta name="description">`,
///    `<link rel="canonical">`, `<html lang="...">`)
///
/// # Arguments
///
/// * `html` — the raw HTML response body.
///
/// # Returns
///
/// A [`PageMetadata`] with all recoverable fields. Never panics — malformed
/// HTML, invalid JSON-LD, and missing tags all produce `None` fields.
///
/// # Example
///
/// ```
/// use ragent_tools_extended::masterfetch::metadata::extract_metadata;
///
/// let html = r#"<html lang="en"><head>
/// <title>Example Page</title>
/// <meta property="og:title" content="Example Title">
/// <meta property="og:description" content="A test page.">
/// <meta property="og:site_name" content="ExampleSite">
/// <meta property="og:type" content="article">
/// <meta property="og:image" content="https://example.com/img.png">
/// <meta property="article:published_time" content="2024-01-15T10:00:00Z">
/// <meta property="article:modified_time" content="2024-06-20T12:00:00Z">
/// <meta property="article:author" content="Jane Doe">
/// <link rel="canonical" href="https://example.com/page">
/// </head><body><p>Hello</p></body></html>"#;
///
/// let md = extract_metadata(html);
/// assert_eq!(md.title.as_deref(), Some("Example Title"));
/// assert_eq!(md.site_name.as_deref(), Some("ExampleSite"));
/// assert_eq!(md.lang.as_deref(), Some("en"));
/// assert_eq!(md.canonical.as_deref(), Some("https://example.com/page"));
/// ```
#[must_use]
pub fn extract_metadata(html: &str) -> PageMetadata {
    // Collect all meta tags as (key, content) pairs.
    let metas = collect_meta_tags(html);

    // Extract OpenGraph and article: meta tags.
    let og_title = meta_value(&metas, "og:title");
    let og_description = meta_value(&metas, "og:description");
    let og_site_name = meta_value(&metas, "og:site_name");
    let og_type = meta_value(&metas, "og:type");
    let og_image = meta_value(&metas, "og:image");
    let article_published = meta_value(&metas, "article:published_time");
    let article_modified = meta_value(&metas, "article:modified_time");

    // Standard HTML fallbacks.
    let title_tag = extract_title_tag(html);
    let meta_description = meta_value(&metas, "description");
    let canonical = extract_canonical(html);
    let lang = extract_html_lang(html);

    // Parse JSON-LD blocks.
    let jsonld = extract_jsonld(html);

    // Author extraction priority:
    // 1. article:author (OpenGraph article namespace)
    // 2. Dublin Core creator tags (dc.creator, dcterms.creator, …)
    // 3. Standard meta author tags (author, citation_author, byl, …)
    // 4. JSON-LD author field
    // Multiple authors are joined with ", ".
    let author = meta_values(&metas, &["article:author"])
        .into_iter()
        .next()
        .map_or_else(
            || {
                let dc = meta_values(&metas, DC_CREATOR_KEYS);
                if !dc.is_empty() {
                    return dc;
                }
                let std = meta_values(&metas, STANDARD_AUTHOR_KEYS);
                if !std.is_empty() {
                    return std;
                }
                jsonld.author.clone().map(|s| vec![s]).unwrap_or_default()
            },
            |a| vec![a],
        );

    // Merge: OpenGraph > JSON-LD > standard HTML.
    PageMetadata {
        title: og_title.or(jsonld.headline).or(title_tag),
        description: og_description.or(jsonld.description).or(meta_description),
        site_name: og_site_name,
        og_type,
        image: og_image.or(jsonld.image),
        canonical,
        lang,
        published_time: article_published.or(jsonld.date_published),
        modified_time: article_modified.or(jsonld.date_modified),
        author: if author.is_empty() {
            None
        } else {
            Some(author.join(", "))
        },
        detected_language: None,
    }
}

// ---------------------------------------------------------------------------
// Meta tag collection
// ---------------------------------------------------------------------------

/// Collect all `<meta>` tags from the HTML as a vector of (key, content) pairs.
///
/// Handles both `property="..."` and `name="..."` attributes, and both
/// attribute orders (property-before-content and content-before-property).
/// Meta tags with no content attribute are skipped. Attribute names are
/// matched case-insensitively (`PROPERTY` and `property` both work).
fn collect_meta_tags(html: &str) -> Vec<(String, String)> {
    let re = Regex::new(r"(?i)<meta\s+([^>]*)/?>").expect("meta tag regex is valid");

    let key_re = Regex::new(r#"(?i)(?:property|name)\s*=\s*["']?([^"'\s>]+)["']?"#)
        .expect("meta key regex is valid");

    let content_re =
        Regex::new(r#"(?i)\bcontent\s*=\s*"([^"]*)""#).expect("meta content regex is valid");

    let content_single_re = Regex::new(r"(?i)\bcontent\s*=\s*'([^']*)'")
        .expect("meta content single-quote regex is valid");

    let mut metas = Vec::new();
    for cap in re.captures_iter(html) {
        let attrs = cap.get(1).map_or("", |m| m.as_str());
        if let Some(key_cap) = key_re.captures(attrs) {
            let key = key_cap.get(1).map_or("", |m| m.as_str());
            if key.is_empty() {
                continue;
            }
            // Try double-quoted content first, then single-quoted.
            let content = content_re
                .captures(attrs)
                .or_else(|| content_single_re.captures(attrs))
                .and_then(|c| c.get(1))
                .map_or("", |m| m.as_str());
            if !content.trim().is_empty() {
                metas.push((key.to_ascii_lowercase(), content.trim().to_string()));
            }
        }
    }
    metas
}

/// Find the first value for a given meta key (case-insensitive).
fn meta_value(metas: &[(String, String)], key: &str) -> Option<String> {
    metas
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.clone())
}

// ---------------------------------------------------------------------------
// <title> tag extraction
// ---------------------------------------------------------------------------

/// Extract the text content of the first `<title>` tag.
fn extract_title_tag(html: &str) -> Option<String> {
    let re = Regex::new(r"(?si)<title[^>]*>(.*?)</title>").expect("title regex is valid");
    let cap = re.captures(html)?;
    let title = cap.get(1)?.as_str().trim().to_string();
    if title.is_empty() { None } else { Some(title) }
}

// ---------------------------------------------------------------------------
// Canonical URL extraction
// ---------------------------------------------------------------------------

/// Extract the `href` attribute from `<link rel="canonical" href="...">`.
fn extract_canonical(html: &str) -> Option<String> {
    let link_re = Regex::new(r"(?i)<link\s+([^>]*)>").expect("link regex is valid");
    let rel_re = Regex::new(r#"(?i)\brel\s*=\s*["']?canonical["']?"#).expect("rel regex is valid");
    let href_re = Regex::new(r#"(?i)\bhref\s*=\s*"([^"]*)""#).expect("href regex is valid");
    let href_single_re =
        Regex::new(r"(?i)\bhref\s*=\s*'([^']*)'").expect("href single-quote regex is valid");

    for cap in link_re.captures_iter(html) {
        let attrs = cap.get(1).map_or("", |m| m.as_str());
        if rel_re.is_match(attrs) {
            let href = href_re
                .captures(attrs)
                .or_else(|| href_single_re.captures(attrs))
                .and_then(|c| c.get(1))
                .map(|m| m.as_str())?;
            if !href.is_empty() {
                return Some(href.to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// <html lang="..."> extraction
// ---------------------------------------------------------------------------

/// Extract the `lang` attribute from the opening `<html>` tag.
fn extract_html_lang(html: &str) -> Option<String> {
    let re = Regex::new(r"(?i)<html\s+([^>]*)>").expect("html tag regex is valid");
    let lang_re = Regex::new(r#"(?i)\blang\s*=\s*["']([^"']+)["']"#).expect("lang regex is valid");

    let cap = re.captures(html)?;
    let attrs = cap.get(1)?.as_str();
    let lang_cap = lang_re.captures(attrs)?;
    let lang = lang_cap.get(1)?.as_str().to_string();
    if lang.is_empty() { None } else { Some(lang) }
}

// ---------------------------------------------------------------------------
// JSON-LD extraction
// ---------------------------------------------------------------------------

/// Fields extracted from JSON-LD blocks.
#[derive(Debug, Default)]
struct JsonLdFields {
    headline: Option<String>,
    description: Option<String>,
    date_published: Option<String>,
    date_modified: Option<String>,
    author: Option<String>,
    image: Option<String>,
}

/// Extract and merge fields from all `<script type="application/ld+json">`
/// blocks.
///
/// Parses each block as JSON and extracts schema.org fields. Blocks that fail
/// to parse are silently skipped. Both single objects and arrays of objects
/// are supported.
fn extract_jsonld(html: &str) -> JsonLdFields {
    let re = Regex::new(
        r#"(?si)<script\s+[^>]*type\s*=\s*["']application/ld\+json["'][^>]*>(.*?)</script>"#,
    )
    .expect("json-ld script regex is valid");

    let mut fields = JsonLdFields::default();

    for cap in re.captures_iter(html) {
        let raw = cap.get(1).map_or("", |m| m.as_str()).trim();
        if raw.is_empty() {
            continue;
        }
        let parsed = parse_jsonld_block(raw);
        merge_jsonld(&mut fields, parsed);
    }
    fields
}

/// Parse a single JSON-LD block (raw JSON string) into [`JsonLdFields`].
///
/// Handles both a single JSON object and a JSON array of objects. Non-object
/// values (strings, numbers) produce an empty [`JsonLdFields`].
fn parse_jsonld_block(raw: &str) -> Vec<JsonLdFields> {
    let value: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    match value {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .filter_map(|v| {
                if v.is_object() {
                    Some(extract_jsonld_object(&v))
                } else {
                    None
                }
            })
            .collect(),
        serde_json::Value::Object(_) => vec![extract_jsonld_object(&value)],
        _ => Vec::new(),
    }
}

/// Extract fields from a single JSON-LD object.
fn extract_jsonld_object(obj: &serde_json::Value) -> JsonLdFields {
    let get_str = |key: &str| -> Option<String> {
        obj.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    // `author` can be a string or an object with `name`.
    let author = obj
        .get("author")
        .and_then(extract_author)
        .or_else(|| get_str("author"));

    // `image` can be a string (URL) or an array of strings or an object.
    let image = obj
        .get("image")
        .and_then(extract_image)
        .or_else(|| get_str("image"));

    JsonLdFields {
        headline: get_str("headline"),
        description: get_str("description"),
        date_published: get_str("datePublished"),
        date_modified: get_str("dateModified"),
        author,
        image,
    }
}

/// Extract author name(s) from a JSON-LD `author` field.
///
/// Handles: string, `{"name": "..."}`, `{"@type": "Person", "name": "..."}`,
/// and arrays of any of the above. For arrays, all names are extracted and
/// joined with `", "`, so multi-author articles keep every contributor rather
/// than only the first entry.
fn extract_author(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => {
            let trimmed = s.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        serde_json::Value::Object(obj) => obj
            .get("name")
            .and_then(|n| n.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        serde_json::Value::Array(arr) => {
            let joined = arr
                .iter()
                .filter_map(extract_author)
                .collect::<Vec<_>>()
                .join(", ");
            if joined.is_empty() {
                None
            } else {
                Some(joined)
            }
        }
        _ => None,
    }
}

/// Extract an image URL from a JSON-LD `image` field.
///
/// Handles: string (URL), array of strings (first URL), object with `url`.
fn extract_image(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => {
            let trimmed = s.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        serde_json::Value::Array(arr) => arr.first().and_then(extract_image),
        serde_json::Value::Object(obj) => obj
            .get("url")
            .and_then(|u| u.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                obj.get("@id")
                    .and_then(|u| u.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            }),
        _ => None,
    }
}

/// Merge JSON-LD fields: only fill in fields that are not already set.
fn merge_jsonld(target: &mut JsonLdFields, source: Vec<JsonLdFields>) {
    for s in source {
        if target.headline.is_none() {
            target.headline = s.headline;
        }
        if target.description.is_none() {
            target.description = s.description;
        }
        if target.date_published.is_none() {
            target.date_published = s.date_published;
        }
        if target.date_modified.is_none() {
            target.date_modified = s.date_modified;
        }
        if target.author.is_none() {
            target.author = s.author;
        }
        if target.image.is_none() {
            target.image = s.image;
        }
    }
}

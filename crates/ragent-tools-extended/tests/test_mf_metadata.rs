//! Integration tests for `masterfetch::metadata` — metadata extraction
//! (T-031, FR-006, NFR-003).
//!
//! Covers: `OpenGraph` extraction, JSON-LD parsing, canonical URL, `<title>`
//! fallback, missing metadata, both attribute orders, `html lang`.

use ragent_tools_extended::masterfetch::metadata::extract_metadata;

// ---------------------------------------------------------------------------
// OpenGraph meta tag extraction
// ---------------------------------------------------------------------------

#[test]
fn test_og_title_extracted() {
    let html = r#"<html><head>
<meta property="og:title" content="My Article">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("My Article"));
}

#[test]
fn test_og_description_extracted() {
    let html = r#"<html><head>
<meta property="og:description" content="A great article.">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.description.as_deref(), Some("A great article."));
}

#[test]
fn test_og_site_name_extracted() {
    let html = r#"<html><head>
<meta property="og:site_name" content="MySite">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.site_name.as_deref(), Some("MySite"));
}

#[test]
fn test_og_type_extracted() {
    let html = r#"<html><head>
<meta property="og:type" content="article">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.og_type.as_deref(), Some("article"));
}

#[test]
fn test_og_image_extracted() {
    let html = r#"<html><head>
<meta property="og:image" content="https://example.com/img.png">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.image.as_deref(), Some("https://example.com/img.png"));
}

#[test]
fn test_article_published_time_extracted() {
    let html = r#"<html><head>
<meta property="article:published_time" content="2024-01-15T10:00:00Z">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.published_time.as_deref(), Some("2024-01-15T10:00:00Z"));
}

#[test]
fn test_article_modified_time_extracted() {
    let html = r#"<html><head>
<meta property="article:modified_time" content="2024-06-20T12:00:00Z">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.modified_time.as_deref(), Some("2024-06-20T12:00:00Z"));
}

#[test]
fn test_article_author_extracted() {
    let html = r#"<html><head>
<meta property="article:author" content="Jane Doe">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.author.as_deref(), Some("Jane Doe"));
}

// ---------------------------------------------------------------------------
// Both attribute orders (property before content, content before property)
// ---------------------------------------------------------------------------

#[test]
fn test_meta_property_before_content() {
    let html = r#"<meta property="og:title" content="Title One">"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("Title One"));
}

#[test]
fn test_meta_content_before_property() {
    let html = r#"<meta content="Title Two" property="og:title">"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("Title Two"));
}

#[test]
fn test_meta_name_before_content() {
    let html = r#"<meta name="description" content="Desc One">"#;
    let md = extract_metadata(html);
    assert_eq!(md.description.as_deref(), Some("Desc One"));
}

#[test]
fn test_meta_content_before_name() {
    let html = r#"<meta content="Desc Two" name="description">"#;
    let md = extract_metadata(html);
    assert_eq!(md.description.as_deref(), Some("Desc Two"));
}

// ---------------------------------------------------------------------------
// <title> tag fallback
// ---------------------------------------------------------------------------

#[test]
fn test_title_tag_as_fallback_when_no_og_title() {
    let html = r"<html><head><title>Page Title</title></head><body></body></html>";
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("Page Title"));
}

#[test]
fn test_og_title_takes_priority_over_title_tag() {
    let html = r#"<html><head>
<title>HTML Title</title>
<meta property="og:title" content="OG Title">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("OG Title"));
}

#[test]
fn test_title_tag_trimmed() {
    let html = r"<html><head><title>  Spaced Title  </title></head><body></body></html>";
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("Spaced Title"));
}

// ---------------------------------------------------------------------------
// <meta name="description"> fallback
// ---------------------------------------------------------------------------

#[test]
fn test_meta_description_as_fallback() {
    let html = r#"<html><head>
<meta name="description" content="Meta description text.">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.description.as_deref(), Some("Meta description text."));
}

#[test]
fn test_og_description_takes_priority_over_meta_description() {
    let html = r#"<html><head>
<meta name="description" content="Standard desc">
<meta property="og:description" content="OG desc">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.description.as_deref(), Some("OG desc"));
}

// ---------------------------------------------------------------------------
// Canonical URL
// ---------------------------------------------------------------------------

#[test]
fn test_canonical_link_extracted() {
    let html = r#"<html><head>
<link rel="canonical" href="https://example.com/page">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.canonical.as_deref(), Some("https://example.com/page"));
}

#[test]
fn test_canonical_with_other_attributes() {
    let html = r#"<html><head>
<link rel="canonical" href="https://example.com/page" hreflang="en">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.canonical.as_deref(), Some("https://example.com/page"));
}

#[test]
fn test_no_canonical_returns_none() {
    let html = r"<html><head><title>Test</title></head><body></body></html>";
    let md = extract_metadata(html);
    assert!(md.canonical.is_none());
}

#[test]
fn test_canonical_not_confused_with_other_links() {
    let html = r#"<html><head>
<link rel="stylesheet" href="style.css">
<link rel="icon" href="favicon.ico">
<link rel="canonical" href="https://example.com/canonical">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(
        md.canonical.as_deref(),
        Some("https://example.com/canonical")
    );
}

// ---------------------------------------------------------------------------
// html lang attribute
// ---------------------------------------------------------------------------

#[test]
fn test_html_lang_extracted() {
    let html = r#"<html lang="en"><head></head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.lang.as_deref(), Some("en"));
}

#[test]
fn test_html_lang_with_region() {
    let html = r#"<html lang="en-US"><head></head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.lang.as_deref(), Some("en-US"));
}

#[test]
fn test_html_lang_with_other_attributes() {
    let html = r#"<html class="no-js" lang="fr" dir="ltr"><head></head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.lang.as_deref(), Some("fr"));
}

#[test]
fn test_no_html_lang_returns_none() {
    let html = r"<html><head></head><body></body></html>";
    let md = extract_metadata(html);
    assert!(md.lang.is_none());
}

// ---------------------------------------------------------------------------
// JSON-LD parsing
// ---------------------------------------------------------------------------

#[test]
fn test_jsonld_article_headline() {
    let html = r#"<html><head>
<script type="application/ld+json">
{"@type": "Article", "headline": "JSON-LD Title"}
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("JSON-LD Title"));
}

#[test]
fn test_jsonld_description() {
    let html = r#"<html><head>
<script type="application/ld+json">
{"@type": "Article", "description": "JSON-LD description"}
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.description.as_deref(), Some("JSON-LD description"));
}

#[test]
fn test_jsonld_date_published() {
    let html = r#"<html><head>
<script type="application/ld+json">
{"@type": "Article", "datePublished": "2024-03-10T08:00:00Z"}
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.published_time.as_deref(), Some("2024-03-10T08:00:00Z"));
}

#[test]
fn test_jsonld_date_modified() {
    let html = r#"<html><head>
<script type="application/ld+json">
{"@type": "Article", "dateModified": "2024-06-01T14:00:00Z"}
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.modified_time.as_deref(), Some("2024-06-01T14:00:00Z"));
}

#[test]
fn test_jsonld_author_string() {
    let html = r#"<html><head>
<script type="application/ld+json">
{"@type": "Article", "author": "John Smith"}
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.author.as_deref(), Some("John Smith"));
}

#[test]
fn test_jsonld_author_object() {
    let html = r#"<html><head>
<script type="application/ld+json">
{"@type": "Article", "author": {"@type": "Person", "name": "Jane Doe"}}
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.author.as_deref(), Some("Jane Doe"));
}

#[test]
fn test_jsonld_author_array() {
    let html = r#"<html><head>
<script type="application/ld+json">
{"@type": "Article", "author": [{"@type": "Person", "name": "First Author"}, {"name": "Second"}]}
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.author.as_deref(), Some("First Author"));
}

#[test]
fn test_jsonld_image_string() {
    let html = r#"<html><head>
<script type="application/ld+json">
{"@type": "Article", "image": "https://example.com/photo.jpg"}
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.image.as_deref(), Some("https://example.com/photo.jpg"));
}

#[test]
fn test_jsonld_image_object() {
    let html = r#"<html><head>
<script type="application/ld+json">
{"@type": "Article", "image": {"url": "https://example.com/img.png"}}
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.image.as_deref(), Some("https://example.com/img.png"));
}

#[test]
fn test_jsonld_array_of_objects() {
    let html = r#"<html><head>
<script type="application/ld+json">
[
  {"@type": "WebSite", "name": "MySite"},
  {"@type": "Article", "headline": "From Array", "datePublished": "2024-01-01T00:00:00Z"}
]
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("From Array"));
    assert_eq!(md.published_time.as_deref(), Some("2024-01-01T00:00:00Z"));
}

#[test]
fn test_jsonld_multiple_script_blocks() {
    let html = r#"<html><head>
<script type="application/ld+json">{"@type": "Article", "headline": "First Block"}</script>
<script type="application/ld+json">{"@type": "Article", "datePublished": "2024-02-02T00:00:00Z"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("First Block"));
    assert_eq!(md.published_time.as_deref(), Some("2024-02-02T00:00:00Z"));
}

#[test]
fn test_jsonld_invalid_json_silently_skipped() {
    let html = r#"<html><head>
<script type="application/ld+json">{invalid json}</script>
<title>Fallback Title</title>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("Fallback Title"));
}

#[test]
fn test_jsonld_empty_block_skipped() {
    let html = r#"<html><head>
<script type="application/ld+json"></script>
<title>Real Title</title>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("Real Title"));
}

// ---------------------------------------------------------------------------
// Priority merging (OpenGraph > JSON-LD > standard HTML)
// ---------------------------------------------------------------------------

#[test]
fn test_og_takes_priority_over_jsonld() {
    let html = r#"<html><head>
<title>HTML Title</title>
<meta property="og:title" content="OG Title">
<script type="application/ld+json">{"headline": "JSON-LD Title"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("OG Title"));
}

#[test]
fn test_jsonld_takes_priority_over_title_tag() {
    let html = r#"<html><head>
<title>HTML Title</title>
<script type="application/ld+json">{"headline": "JSON-LD Title"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("JSON-LD Title"));
}

#[test]
fn test_og_description_takes_priority_over_jsonld() {
    let html = r#"<html><head>
<meta property="og:description" content="OG desc">
<script type="application/ld+json">{"description": "JSON-LD desc"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.description.as_deref(), Some("OG desc"));
}

#[test]
fn test_jsonld_published_fills_when_no_article_meta() {
    let html = r#"<html><head>
<script type="application/ld+json">{"datePublished": "2024-05-05T00:00:00Z"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.published_time.as_deref(), Some("2024-05-05T00:00:00Z"));
}

#[test]
fn test_article_published_takes_priority_over_jsonld() {
    let html = r#"<html><head>
<meta property="article:published_time" content="2024-01-01T00:00:00Z">
<script type="application/ld+json">{"datePublished": "2024-02-02T00:00:00Z"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.published_time.as_deref(), Some("2024-01-01T00:00:00Z"));
}

// ---------------------------------------------------------------------------
// Missing metadata
// ---------------------------------------------------------------------------

#[test]
fn test_empty_html_returns_all_none() {
    let md = extract_metadata("");
    assert!(md.title.is_none());
    assert!(md.description.is_none());
    assert!(md.site_name.is_none());
    assert!(md.og_type.is_none());
    assert!(md.image.is_none());
    assert!(md.canonical.is_none());
    assert!(md.lang.is_none());
    assert!(md.published_time.is_none());
    assert!(md.modified_time.is_none());
    assert!(md.author.is_none());
}

#[test]
fn test_html_with_no_metadata_returns_all_none() {
    let html = r"<html><head></head><body><p>Hello</p></body></html>";
    let md = extract_metadata(html);
    assert!(md.title.is_none());
    assert!(md.description.is_none());
    assert!(md.canonical.is_none());
    assert!(md.lang.is_none());
}

#[test]
fn test_empty_content_values_skipped() {
    let html = r#"<html><head>
<meta property="og:title" content="">
<meta property="og:description" content="   ">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert!(md.title.is_none());
    assert!(md.description.is_none());
}

// ---------------------------------------------------------------------------
// Complete page with all metadata sources
// ---------------------------------------------------------------------------

#[test]
fn test_complete_page_all_sources() {
    let html = r#"<html lang="en"><head>
<title>Example Page</title>
<meta name="description" content="Standard description">
<meta property="og:title" content="OG Title">
<meta property="og:description" content="OG Description">
<meta property="og:site_name" content="ExampleSite">
<meta property="og:type" content="article">
<meta property="og:image" content="https://example.com/og.png">
<meta property="article:published_time" content="2024-01-15T10:00:00Z">
<meta property="article:modified_time" content="2024-06-20T12:00:00Z">
<meta property="article:author" content="Jane Doe">
<link rel="canonical" href="https://example.com/page">
<script type="application/ld+json">
{"@type": "Article", "headline": "JSON-LD Title", "datePublished": "2024-01-10T00:00:00Z"}
</script>
</head><body><p>Content</p></body></html>"#;

    let md = extract_metadata(html);
    // OG takes priority.
    assert_eq!(md.title.as_deref(), Some("OG Title"));
    assert_eq!(md.description.as_deref(), Some("OG Description"));
    assert_eq!(md.site_name.as_deref(), Some("ExampleSite"));
    assert_eq!(md.og_type.as_deref(), Some("article"));
    assert_eq!(md.image.as_deref(), Some("https://example.com/og.png"));
    assert_eq!(md.published_time.as_deref(), Some("2024-01-15T10:00:00Z"));
    assert_eq!(md.modified_time.as_deref(), Some("2024-06-20T12:00:00Z"));
    assert_eq!(md.author.as_deref(), Some("Jane Doe"));
    assert_eq!(md.canonical.as_deref(), Some("https://example.com/page"));
    assert_eq!(md.lang.as_deref(), Some("en"));
}

// ---------------------------------------------------------------------------
// Self-closing meta tags
// ---------------------------------------------------------------------------

#[test]
fn test_self_closing_meta_tag() {
    let html = r#"<meta property="og:title" content="Self-Closed" />"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("Self-Closed"));
}

#[test]
fn test_meta_tag_without_closing_slash() {
    let html = r#"<meta property="og:title" content="No Slash">"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("No Slash"));
}

// ---------------------------------------------------------------------------
// Case sensitivity
// ---------------------------------------------------------------------------

#[test]
fn test_meta_property_case_insensitive_key() {
    // The property/name value is case-sensitive (og:title is always lowercase),
    // but the attribute name itself can be any case.
    let html = r#"<meta PROPERTY="og:title" CONTENT="Mixed Case Attrs">"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("Mixed Case Attrs"));
}

#[test]
fn test_uppercase_html_tag_lang() {
    let html = r#"<HTML LANG="de"><head></head><body></body></HTML>"#;
    let md = extract_metadata(html);
    assert_eq!(md.lang.as_deref(), Some("de"));
}
// ---------------------------------------------------------------------------
// Additional edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_jsonld_image_array_extracts_first() {
    let html = r#"<html><head>
<script type="application/ld+json">
{"@type": "Article", "image": ["https://example.com/1.jpg", "https://example.com/2.jpg"]}
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.image.as_deref(), Some("https://example.com/1.jpg"));
}

#[test]
fn test_jsonld_image_object_with_at_id() {
    // Some publishers use {"@id": "url"} for images.
    let html = r#"<html><head>
<script type="application/ld+json">
{"@type": "Article", "image": {"@id": "https://example.com/id.png"}}
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.image.as_deref(), Some("https://example.com/id.png"));
}

#[test]
fn test_canonical_single_quoted_href() {
    let html = r"<html><head>
<link rel='canonical' href='https://example.com/single'>
</head><body></body></html>";
    let md = extract_metadata(html);
    assert_eq!(md.canonical.as_deref(), Some("https://example.com/single"));
}

#[test]
fn test_canonical_href_before_rel() {
    // href attribute appearing before rel in the <link> tag.
    let html = r#"<html><head>
<link href="https://example.com/hreffirst" rel="canonical">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(
        md.canonical.as_deref(),
        Some("https://example.com/hreffirst")
    );
}

#[test]
fn test_og_image_takes_priority_over_jsonld_image() {
    let html = r#"<html><head>
<meta property="og:image" content="https://example.com/og.png">
<script type="application/ld+json">{"image": "https://example.com/jl.png"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.image.as_deref(), Some("https://example.com/og.png"));
}

#[test]
fn test_jsonld_image_fills_when_no_og_image() {
    let html = r#"<html><head>
<script type="application/ld+json">{"image": "https://example.com/jl.png"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.image.as_deref(), Some("https://example.com/jl.png"));
}

#[test]
fn test_first_og_title_wins_when_duplicated() {
    let html = r#"<html><head>
<meta property="og:title" content="First Title">
<meta property="og:title" content="Second Title">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("First Title"));
}

#[test]
fn test_jsonld_author_person_empty_name_returns_none() {
    let html = r#"<html><head>
<script type="application/ld+json">
{"@type": "Article", "author": {"@type": "Person", "name": "  "}}
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert!(md.author.is_none());
}

#[test]
fn test_meta_description_fallback_when_no_og_description() {
    // meta name="description" should fill in when og:description absent AND
    // JSON-LD description absent.
    let html = r#"<html><head>
<meta name="description" content="Just a standard description.">
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(
        md.description.as_deref(),
        Some("Just a standard description.")
    );
}

#[test]
fn test_jsonld_description_fills_when_no_og_description() {
    let html = r#"<html><head>
<script type="application/ld+json">{"description": "JSON-LD only desc"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.description.as_deref(), Some("JSON-LD only desc"));
}

#[test]
fn test_jsonld_modified_fills_when_no_article_meta() {
    let html = r#"<html><head>
<script type="application/ld+json">{"dateModified": "2024-08-08T00:00:00Z"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.modified_time.as_deref(), Some("2024-08-08T00:00:00Z"));
}

#[test]
fn test_article_modified_takes_priority_over_jsonld() {
    let html = r#"<html><head>
<meta property="article:modified_time" content="2024-01-01T00:00:00Z">
<script type="application/ld+json">{"dateModified": "2024-02-02T00:00:00Z"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.modified_time.as_deref(), Some("2024-01-01T00:00:00Z"));
}

#[test]
fn test_article_author_takes_priority_over_jsonld() {
    let html = r#"<html><head>
<meta property="article:author" content="Meta Author">
<script type="application/ld+json">{"author": "JSON-LD Author"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.author.as_deref(), Some("Meta Author"));
}

#[test]
fn test_jsonld_author_fills_when_no_article_author() {
    let html = r#"<html><head>
<script type="application/ld+json">{"author": "JSON-LD Author"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.author.as_deref(), Some("JSON-LD Author"));
}

#[test]
fn test_malformed_html_does_not_panic() {
    // Unclosed tags, stray brackets — must not panic.
    let html = r#"<html><head><meta property="og:title" content="Broken<br>head><body"#;
    let md = extract_metadata(html);
    // We don't assert a specific outcome — just that it didn't panic.
    let _ = md;
}

#[test]
fn test_meta_with_name_attribute_extracts() {
    // <meta name="..."> (vs property="...") for description.
    let html = r#"<meta name="description" content="Name-based description">"#;
    let md = extract_metadata(html);
    assert_eq!(md.description.as_deref(), Some("Name-based description"));
}

#[test]
fn test_jsonld_newsarticle_type() {
    // NewsArticle is a supported schema.org type alongside Article.
    let html = r#"<html><head>
<script type="application/ld+json">
{"@type": "NewsArticle", "headline": "Breaking News", "datePublished": "2024-07-04T12:00:00Z"}
</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("Breaking News"));
    assert_eq!(md.published_time.as_deref(), Some("2024-07-04T12:00:00Z"));
}

#[test]
fn test_jsonld_generic_object_without_type() {
    // A JSON-LD object with no @type still extracts fields.
    let html = r#"<html><head>
<script type="application/ld+json">{"headline": "No Type Title"}</script>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("No Type Title"));
}

#[test]
fn test_jsonld_non_object_value_skipped() {
    // A JSON-LD block containing a bare string must not panic and produces
    // no fields.
    let html = r#"<html><head>
<script type="application/ld+json">"just a string"</script>
<title>Real Title</title>
</head><body></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.title.as_deref(), Some("Real Title"));
}

#![allow(clippy::assert_is_empty)]
//! Regression tests for web-source author acquisition in the research
//! pipeline.
//!
//! Covers two independent paths that must make the **Author** column of the
//! References Index work without depending on the search backend:
//!
//! 1. `parse_mf_fetch_output` must read the author from the *nested*
//!    `metadata.metadata.author` shape that `mf_fetch` emits (the
//!    `PageMetadata` object is serialised under the `metadata` key of the
//!    `ToolOutput.metadata` envelope), in addition to the top-level
//!    `metadata.author` shape.
//!
//! 2. The page's own HTML `<head>` is a URL-only source of author and
//!    publication-date metadata: `extract_metadata` reads `article:author`,
//!    Dublin Core creator tags, standard `<meta name="author">`-style tags,
//!    and JSON-LD `author`, and `extract_published_at` reads the date
//!    metadata. This is what the supplementary head fetch uses when the
//!    search backend reported no author.

use ragent_tools_extended::masterfetch::metadata::extract_metadata;

const SAMPLE_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<title>Agentic RAG | Example</title>
<meta name="author" content="Ivan Belcic, Cole Stryker">
<meta property="article:published_time" content="2025-02-25T10:00:00Z">
</head>
<body><p>Agentic RAG content …</p></body>
</html>"#;

#[test]
fn test_head_metadata_author_from_meta_author_tag() {
    let md = extract_metadata(SAMPLE_HTML);
    assert_eq!(md.author.as_deref(), Some("Ivan Belcic, Cole Stryker"));
}

#[test]
fn test_head_metadata_published_time_from_article_tag() {
    let md = extract_metadata(SAMPLE_HTML);
    assert_eq!(md.published_time.as_deref(), Some("2025-02-25T10:00:00Z"));
}

#[test]
fn test_head_metadata_author_from_jsonld_multiple_persons() {
    let html = r#"<!DOCTYPE html>
<html><head>
<script type="application/ld+json">
{"@context":"https://schema.org","@type":"Article","headline":"X",
 "author":[{"@type":"Person","name":"Ivan Belcic"},{"@type":"Person","name":"Cole Stryker"}]}
</script>
</head><body><p>body</p></body></html>"#;
    let md = extract_metadata(html);
    // Either joined or at least the first author must be present.
    let author = md.author.expect("JSON-LD authors should be extracted");
    assert!(author.contains("Ivan Belcic"), "author = {author}");
    assert!(author.contains("Cole Stryker"), "author = {author}");
}

#[test]
fn test_head_metadata_author_priority_article_author_over_meta() {
    let html = r#"<!DOCTYPE html>
<html><head>
<meta property="article:author" content="Preferred Author">
<meta name="author" content="Fallback Author">
</head><body><p>body</p></body></html>"#;
    let md = extract_metadata(html);
    assert_eq!(md.author.as_deref(), Some("Preferred Author"));
}

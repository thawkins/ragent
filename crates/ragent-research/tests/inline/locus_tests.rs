//! Inline tests for `locus.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::source::Source;
use std::path::PathBuf;

fn web_source(index: usize, body: &str) -> Source {
    Source::Web {
        url: format!("https://example.com/{index}"),
        title: format!("Source {index}"),
        captured_at: chrono::Utc::now(),
        published_at: None,
        body_path: PathBuf::new(),
        body: body.into(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    }
}

#[test]
fn loci_empty_when_fewer_than_two_sources() {
    let sources = vec![web_source(1, "The system improves performance.")];
    let set = analyze_loci(&sources);
    assert!(set.is_empty());
}

#[test]
fn loci_detects_shared_dimension() {
    let sources = vec![
        web_source(1, "The system improves performance dramatically."),
        web_source(2, "Performance remains a key concern for users."),
    ];
    let set = analyze_loci(&sources);
    let locus = set
        .loci
        .iter()
        .find(|l| l.keyword == "performance")
        .expect("performance locus should be detected");
    assert_eq!(locus.mentions, 2);
    assert!(locus.source_indices.contains(&1));
    assert!(locus.source_indices.contains(&2));
    assert_eq!(locus.label, "Performance");
}

#[test]
fn loci_sorted_by_mentions() {
    let sources = vec![
        web_source(1, "Performance is great and cost is low."),
        web_source(2, "Performance is excellent."),
        web_source(3, "Performance is okay."),
        web_source(4, "Cost is the only metric."),
    ];
    let set = analyze_loci(&sources);
    assert!(!set.is_empty());
    assert_eq!(set.loci[0].keyword, "performance");
    assert_eq!(set.loci[0].mentions, 3);
}

#[test]
fn depth_classifies_by_source_count() {
    let loci = LocusSet {
        loci: vec![
            Locus {
                keyword: "performance".into(),
                label: "Performance".into(),
                source_indices: vec![1],
                snippets: Vec::new(),
                mentions: 1,
            },
            Locus {
                keyword: "cost".into(),
                label: "Cost".into(),
                source_indices: vec![1, 2],
                snippets: Vec::new(),
                mentions: 2,
            },
            Locus {
                keyword: "safety".into(),
                label: "Safety".into(),
                source_indices: vec![1, 2, 3, 4],
                snippets: Vec::new(),
                mentions: 4,
            },
        ],
    };
    let depths = investigate_depth(&loci);
    assert_eq!(depths.len(), 3);
    assert_eq!(depths[0].depth, DepthLevel::Surface);
    assert_eq!(depths[1].depth, DepthLevel::Moderate);
    assert_eq!(depths[2].depth, DepthLevel::Deep);
}

#[test]
fn snippet_extracts_context_around_keyword() {
    let body = "The quick brown fox improves performance under load and scales well.";
    let snippet = extract_snippet(body, "performance");
    assert!(snippet.to_lowercase().contains("performance"));
    assert!(snippet.contains("improves"));
}

#[test]
fn loci_case_insensitive_match() {
    let sources = vec![
        web_source(1, "PERFORMANCE is critical."),
        web_source(2, "We measured Performance carefully."),
    ];
    let set = analyze_loci(&sources);
    assert!(set.loci.iter().any(|l| l.keyword == "performance"));
}

#[test]
fn loci_ignores_short_bodies() {
    let sources = vec![
        web_source(1, "Performance"),
        web_source(2, "Performance is great and performance is good."),
    ];
    let set = analyze_loci(&sources);
    let perf = set.loci.iter().find(|l| l.keyword == "performance");
    assert_eq!(perf.map(|l| l.mentions), Some(1));
}

#[test]
fn snippet_does_not_panic_on_multibyte_boundary() {
    // Reproduces the panic: an em-dash ('-', 3 bytes) sits inside the slice
    // window when the start offset is computed from a lowercased copy whose
    // earlier bytes differ in length. The snippet must extract without
    // panicking on a non-char-boundary byte index.
    let prefix = "I".repeat(512); // 'I' lowercases to a longer sequence, shifting offsets
    let body = format!("{prefix}lead in - performance is the key metric here and more.");
    let snippet = extract_snippet(&body, "performance");
    assert!(snippet.to_lowercase().contains("performance"));
}

#![allow(clippy::assert_is_empty)]
//! Integration tests for the contradiction-graph builder (T-007).
//!
//! Migrated from `crates/ragent-research/src/contradiction.rs` inline tests
//! per the project convention that all tests live in `tests/`.

use ragent_research::contradiction::{
    ContradictionConfig, PolarityDimension, build_contradiction_graph,
};
use ragent_research::source::Source;
use std::path::PathBuf;

fn web_source(index: usize, url: &str, body: &str) -> Source {
    Source::Web {
        url: url.to_string(),
        title: format!("Source {index}"),
        captured_at: chrono::Utc::now(),
        published_at: None,
        body_path: PathBuf::new(),
        body: body.to_string(),
        relevance: "High".to_string(),
        search_tool: "mf_search".to_string(),
        search_engine: "duckduckgo".to_string(),
        content_type: None,
        page_type: None,
        media_type: "page".to_string(),
        language: None,
        oa_recovery: None,
        author: None,
    }
}

#[test]
fn graph_empty_when_fewer_than_two_sources() {
    let sources = vec![web_source(1, "https://a.example", " improves performance")];
    let graph = build_contradiction_graph(&sources);
    assert!(graph.is_empty());
}

#[test]
fn graph_detects_opposing_performance_claims() {
    let sources = vec![
        web_source(
            1,
            "https://a.example",
            "The new system improves performance.",
        ),
        web_source(
            2,
            "https://b.example",
            "The legacy system degrades performance significantly.",
        ),
    ];
    let graph = build_contradiction_graph(&sources);
    assert_eq!(graph.edges.len(), 1);
    let edge = &graph.edges[0];
    assert_eq!(edge.dimension, "performance");
    assert!(edge.strength > 0);
    assert_eq!(edge.claim_a.source_index, 1);
    assert_eq!(edge.claim_b.source_index, 2);
}

#[test]
fn graph_ignores_short_bodies() {
    let sources = vec![
        web_source(1, "https://a.example", "improves"),
        web_source(2, "https://b.example", "worsens"),
    ];
    let graph = build_contradiction_graph(&sources);
    assert!(graph.is_empty());
}

#[test]
fn graph_filters_sources_with_both_polarities() {
    let sources = vec![
        web_source(
            1,
            "https://a.example",
            "The drug improves safety in adults but adverse effects in children make it less safe overall.",
        ),
        web_source(
            2,
            "https://b.example",
            "The drug is well tolerated and safe.",
        ),
    ];
    let graph = build_contradiction_graph(&sources);
    assert!(graph.is_empty());
}

#[test]
fn graph_ranks_stronger_edges_first() {
    let sources = vec![
        web_source(
            1,
            "https://a.example",
            "The intervention improves performance and reduces cost.",
        ),
        web_source(
            2,
            "https://b.example",
            "The intervention degrades performance and increases cost.",
        ),
    ];
    let graph = build_contradiction_graph(&sources);
    assert_eq!(graph.edges.len(), 1);
    assert!(graph.edges[0].strength > 30);
}

#[test]
fn graph_can_lookup_edges_by_source_index() {
    let sources = vec![
        web_source(1, "https://a.example", "X improves performance."),
        web_source(2, "https://b.example", "X degrades performance."),
        web_source(3, "https://c.example", "X is neutral."),
    ];
    let graph = build_contradiction_graph(&sources);
    assert_eq!(graph.edges_for_source(1).len(), 1);
    assert_eq!(graph.edges_for_source(2).len(), 1);
    assert!(graph.edges_for_source(3).is_empty());
}

#[test]
fn graph_deduplicates_pairs() {
    let sources = vec![
        web_source(
            1,
            "https://a.example",
            "X improves performance and reduces cost.",
        ),
        web_source(
            2,
            "https://b.example",
            "X degrades performance and increases cost.",
        ),
    ];
    let graph = build_contradiction_graph(&sources);
    let count = graph.edges.len();
    assert_eq!(count, 1);
}

#[test]
fn graph_empty_when_no_dimensions_configured() {
    let sources = vec![
        web_source(1, "https://a.example", "X improves performance."),
        web_source(2, "https://b.example", "X degrades performance."),
    ];
    let config = ContradictionConfig::empty();
    let graph = ragent_research::contradiction::build_contradiction_graph_with(&sources, &config);
    assert!(graph.is_empty());
}

#[test]
fn graph_uses_custom_dimensions_for_non_medical_topics() {
    // Custom dimensions for a climate research topic.
    let config = ContradictionConfig::new(vec![PolarityDimension::new(
        "warming",
        &["temperature rising", "warming trend", "hotter"],
        &["temperature falling", "cooling trend", "colder"],
    )]);
    let sources = vec![
        web_source(
            1,
            "https://a.example",
            "The data shows a clear warming trend over the last decade.",
        ),
        web_source(
            2,
            "https://b.example",
            "Recent measurements indicate a cooling trend in the same period.",
        ),
    ];
    let graph = ragent_research::contradiction::build_contradiction_graph_with(&sources, &config);
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].dimension, "warming");
}

#[test]
fn graph_default_config_preserves_original_dimensions() {
    let config = ContradictionConfig::default();
    assert_eq!(config.dimensions.len(), 6);
    assert_eq!(config.dimensions[0].keyword, "effect");
    assert_eq!(config.dimensions[1].keyword, "mortality");
    assert_eq!(config.dimensions[2].keyword, "performance");
    assert_eq!(config.dimensions[3].keyword, "cost");
    assert_eq!(config.dimensions[4].keyword, "adoption");
    assert_eq!(config.dimensions[5].keyword, "safety");
}

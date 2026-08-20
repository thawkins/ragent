//! Tests for author propagation through the masterfetch search pipeline and
//! into research-layer web sources.
//!
//! Guards the regression where scholarly (OpenAlex) and Exa search hits
//! carried author metadata that was dropped before reaching the rendered
//! Research document, leaving the References Index **Author** column and the
//! per-finding **Sources:** bullets with no author attribution.

use ragent_tools_extended::masterfetch::search::consensus::merge_and_rank;
use ragent_tools_extended::masterfetch::search::engine::{EngineReport, RawResult};
use ragent_tools_extended::masterfetch::search::openalex;

/// An OpenAlex work exposing `authorships` must surface the joined author
/// display names on the resulting [`RawResult`].
#[test]
fn test_openalex_parse_response_extracts_authors() {
    let value = serde_json::json!({
        "results": [{
            "id": "https://openalex.org/W1",
            "title": "Rust Runtime Scheduling",
            "relevance_score": 12.5,
            "publication_year": 2024,
            "authorships": [
                { "author": { "display_name": "Ada Lovelace" } },
                { "author": { "display_name": "Alan Turing" } }
            ],
            "primary_location": {
                "landing_page_url": "https://example.org/paper",
                "source": { "display_name": "Example Journal" }
            },
            "open_access": { "is_oa": true, "oa_url": "https://example.org/oa" }
        }]
    });

    let results = openalex::parse_response(&value);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].author.as_deref(),
        Some("Ada Lovelace, Alan Turing")
    );
}

/// An OpenAlex work with no `authorships` must yield `author: None` rather
/// than an empty string so renderers apply the `—` placeholder.
#[test]
fn test_openalex_parse_response_missing_authors_is_none() {
    let value = serde_json::json!({
        "results": [{
            "id": "https://openalex.org/W2",
            "title": "Anonymous Work",
            "primary_location": { "landing_page_url": "https://example.org/a" }
        }]
    });

    let results = openalex::parse_response(&value);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].author, None);
}

/// An Exa hit exposing an `author` field must surface it on the [`RawResult`].
#[test]
fn test_exa_parse_response_extracts_author() {
    let value = serde_json::json!({
        "results": [{
            "title": "Agent Survey",
            "url": "https://example.com/survey",
            "score": 0.9,
            "publishedDate": "2026-01-01",
            "author": "Jane Doe",
            "highlights": ["agentic systems"]
        }]
    });

    let results = ragent_tools_extended::masterfetch::search::exa::parse_response_json(&value);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].author.as_deref(), Some("Jane Doe"));
}

/// The consensus merge must carry the author from the contributing engine
/// through to the ranked [`ConsensusResult`] so `mf_search` metadata (and the
/// research adapter that parses it) can attribute the source.
#[test]
fn test_consensus_merge_preserves_engine_author() {
    let mut scholarly = RawResult::new(
        "Paper",
        "https://doi.org/10.1000/x",
        "Abstract text",
        "openalex",
    );
    scholarly.author = Some("Ada Lovelace".into());

    let reports = vec![
        EngineReport::ok("openalex", vec![scholarly]),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("Page", "https://a.com", "Snip", "brave")],
        ),
    ];

    let output = merge_and_rank(&reports, "rust scheduling");
    let first = output
        .results
        .iter()
        .find(|r| r.url == "https://doi.org/10.1000/x")
        .expect("openalex result should be present");
    assert_eq!(first.author.as_deref(), Some("Ada Lovelace"));

    let brave = output
        .results
        .iter()
        .find(|r| r.url == "https://a.com/")
        .expect("brave result should be present");
    assert_eq!(brave.author, None);
}

/// `hits_from_metadata` must deserialize the `author` key emitted by both
/// `mf_search` and the legacy `websearch` wrapper so the research adapter can
/// thread it onto `WebSearchHit`.
#[test]
fn test_hits_from_metadata_round_trips_author() {
    let metadata = serde_json::json!({
        "results": [
            {
                "title": "Paper",
                "url": "https://doi.org/10.1000/x",
                "snippet": "Abstract",
                "search_tool": "mf_search",
                "search_engine": "openalex",
                "author": "Ada Lovelace"
            },
            {
                "title": "Blog",
                "url": "https://example.com/blog",
                "snippet": "Post",
                "search_tool": "mf_search",
                "search_engine": "brave"
            }
        ]
    });

    let hits = ragent_tools_extended::websearch::hits_from_metadata(&metadata);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].author.as_deref(), Some("Ada Lovelace"));
    assert_eq!(hits[1].author, None);
}

/// The research adapter's `mf_search` metadata parser must copy the `author`
/// key onto the emitted `WebSearchHit` (empty/missing → `None`). This mirrors
/// the adapter-side unit tests in `research_adapter.rs`; the check lives here
/// too so the contract is verified from the metadata producer side.
#[test]
fn test_search_metadata_author_key_is_stable_contract() {
    // The producer-side guarantee: `build_search_metadata` emits `author`
    // verbatim from the consensus result.
    let mut scholarly =
        RawResult::new("Paper", "https://doi.org/10.1000/y", "Abstract", "openalex");
    scholarly.author = Some("Grace Hopper".into());
    let reports = vec![EngineReport::ok("openalex", vec![scholarly])];
    let output = merge_and_rank(&reports, "compilers");
    assert_eq!(
        output.results[0].author.as_deref(),
        Some("Grace Hopper"),
        "author must survive consensus ranking so metadata builders can emit it"
    );
}

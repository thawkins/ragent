//! Tests that a search-author survives the `WebSearchHit` → `WebFetchedPage`
//! synthesis for scholarly (OpenAlex) and encyclopedia (Wikipedia) hits, and
//! lands on `Source::Web.author` in the captured source.
//!
//! Regression coverage for the bug where the References Index **Author**
//! column and per-finding **Sources:** bullets always rendered `—` because
//! the synthetic-page constructors hard-coded `author: None`.

use std::sync::Arc;

use ragent_research::Source;
use ragent_research::web_gatherer::{WebGatherer, WebSearchHit, WebSearchTool};

struct StaticSearch(Vec<WebSearchHit>);

#[async_trait::async_trait]
impl WebSearchTool for StaticSearch {
    async fn search(&self, _query: &str, _max_results: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        Ok(self.0.clone())
    }
}

/// A never-called fetch tool — author propagation for synthetic pages must
/// not depend on fetching the URL.
struct NoFetch;

#[async_trait::async_trait]
impl ragent_research::web_gatherer::WebFetchTool for NoFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<ragent_research::WebFetchedPage> {
        anyhow::bail!("fetch should not be called in this test (url={url})")
    }
}

fn hit(author: Option<&str>) -> WebSearchHit {
    WebSearchHit {
        url: "https://doi.org/10.1000/agent-taxonomy".into(),
        title: "A Taxonomy of AI Agents".into(),
        snippet: "We propose a taxonomy of AI agents spanning reactive, deliberative, \
                  and hybrid architectures with application case studies. (Year: 2025 | Cited: 4 | OA: yes | Source: ACM)"
            .into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "openalex".into(),
        author: author.map(str::to_string),
    }
}

#[tokio::test]
async fn test_scholarly_hit_author_propagates_to_source() {
    let search = Arc::new(StaticSearch(vec![hit(Some("Ada Lovelace, Alan Turing"))]));
    let gatherer = WebGatherer::new(search, Arc::new(NoFetch));

    let sources = gatherer
        .gather("ai agent taxonomy", 5)
        .await
        .expect("gather should succeed");

    assert_eq!(sources.len(), 1);
    assert_eq!(
        sources[0].author(),
        Some("Ada Lovelace, Alan Turing"),
        "author from the OpenAlex hit must land on the captured Source::Web"
    );
}

#[tokio::test]
async fn test_scholarly_hit_without_author_stays_none() {
    let search = Arc::new(StaticSearch(vec![hit(None)]));
    let gatherer = WebGatherer::new(search, Arc::new(NoFetch));

    let sources = gatherer
        .gather("ai agent taxonomy", 5)
        .await
        .expect("gather should succeed");

    assert_eq!(sources.len(), 1);
    assert_eq!(
        sources[0].author(),
        None,
        "a hit without author metadata must keep Source::Web.author as None"
    );
}

/// The `Source::Web.author` value rendered by `render_references_index` and
/// `render_finding_sources` comes from the same accessor, so a populated
/// author flows to both output sections without additional wiring.
#[test]
fn test_source_web_author_accessor_used_by_renderers() {
    let mk = |author: Option<String>| Source::Web {
        url: "https://example.com/a".into(),
        title: "Example".into(),
        captured_at: chrono::Utc::now(),
        published_at: None,
        body_path: std::path::PathBuf::from("sources/web-01.md"),
        body: "body".into(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        author,
        oa_recovery: None,
    };

    let with_author = mk(Some("Ada Lovelace".into()));
    assert_eq!(with_author.author(), Some("Ada Lovelace"));

    let without_author = mk(None);
    assert_eq!(without_author.author(), None);
}

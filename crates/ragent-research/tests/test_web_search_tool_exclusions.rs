//! Integration tests for the `WebSearchTool::search_with_exclusions` contract
//! (spec `researchnoacc`, T-006; FR-004, FR-006).
//!
//! The trait gains an engine-exclusion entry point with a default
//! implementation that ignores the exclusions, so existing implementations and
//! test doubles keep compiling and behaving exactly as before. Implementations
//! that can steer engine selection override it and receive the names verbatim.

use std::sync::Mutex;

use ragent_research::{WebSearchHit, WebSearchTool};

fn hit(url: &str, engine: &str) -> WebSearchHit {
    WebSearchHit {
        url: url.to_string(),
        title: format!("title-{url}"),
        snippet: "snippet".to_string(),
        matched_query: String::new(),
        search_tool: "mf_search".to_string(),
        search_engine: engine.to_string(),
        author: None,
    }
}

/// A minimal implementation that does not override the new method: the
/// default must delegate to `search` and ignore the exclusions.
struct BaseOnly {
    calls: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl WebSearchTool for BaseOnly {
    async fn search(&self, query: &str, _max: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        self.calls.lock().unwrap().push(query.to_string());
        Ok(vec![hit("https://a.example", "openalex")])
    }
}

/// An implementation that overrides the new method, recording the exclusions it
/// received so the test can assert they are passed through unchanged.
struct ExclusionAware {
    seen: Mutex<Vec<Vec<String>>>,
}

#[async_trait::async_trait]
impl WebSearchTool for ExclusionAware {
    async fn search(&self, _query: &str, _max: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        panic!("search_with_exclusions override must be called, not the base search");
    }

    async fn search_with_exclusions(
        &self,
        _query: &str,
        _max: usize,
        exclude_engines: &[&str],
    ) -> anyhow::Result<Vec<WebSearchHit>> {
        self.seen
            .lock()
            .unwrap()
            .push(exclude_engines.iter().map(|s| s.to_string()).collect());
        Ok(vec![hit("https://b.example", "wikipedia")])
    }
}

#[tokio::test]
async fn default_impl_ignores_exclusions_and_delegates_to_search() {
    // FR-004 compatibility: an implementation that predates the exclusion
    // parameter still works; the default ignores the names.
    let tool = BaseOnly {
        calls: Mutex::new(Vec::new()),
    };

    let hits = tool
        .search_with_exclusions("rust async", 10, &["openalex"])
        .await
        .expect("default impl should succeed");

    assert_eq!(hits.len(), 1);
    assert_eq!(tool.calls.lock().unwrap().as_slice(), ["rust async"]);
}

#[tokio::test]
async fn default_impl_with_empty_exclusions_is_identical_to_search() {
    let tool = BaseOnly {
        calls: Mutex::new(Vec::new()),
    };

    let hits = tool
        .search_with_exclusions("rust async", 10, &[])
        .await
        .expect("default impl should succeed");

    assert_eq!(hits.len(), 1);
    assert_eq!(tool.calls.lock().unwrap().as_slice(), ["rust async"]);
}

#[tokio::test]
async fn overridden_impl_receives_exclusion_names_verbatim() {
    // FR-006: the gatherer-supplied academic engine names reach the
    // implementation untouched.
    let tool = ExclusionAware {
        seen: Mutex::new(Vec::new()),
    };

    let hits = tool
        .search_with_exclusions("rust async", 10, &["openalex", "wikipedia"])
        .await
        .expect("overridden impl should succeed");

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].search_engine, "wikipedia");
    let seen = tool.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        seen[0],
        vec!["openalex".to_string(), "wikipedia".to_string()]
    );
}

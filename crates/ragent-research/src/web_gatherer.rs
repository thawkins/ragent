//! Web-gathering phase for the research system (FR-006, FR-007).
//!
//! This module implements the orchestration logic that turns a research
//! topic into a list of [`Source::Web`] entries. The actual HTTP calls
//! are made through the [`WebSearchTool`] and [`WebFetchTool`] trait
//! abstractions so the gatherer can be unit-tested without network access
//! and reused from any integration context (TUI agent loop, CLI, HTTP
//! endpoint, tests).
//!
//! ## Flow
//!
//! 1. [`WebGatherer::gather`] issues a [`WebSearchTool::search`] for the
//!    topic and collects up to `max_results` candidate URLs.
//! 2. For each candidate URL it calls [`WebFetchTool::fetch`] to obtain
//!    the page body and title.
//! 3. Each captured page becomes a [`Source::Web`] entry with a synthetic
//!    supporting-file path of the form `sources/web-NN.md` (zero-padded,
//!    starting at 01) — the actual supporting-file write is done by the IO
//!    layer (T-015) once we have an item directory on disk; this module
//!    only returns the captured metadata.
//! 4. If the search or fetch tools return zero results the gatherer
//!    returns an empty `Vec` (FR-006: graceful degradation).
//!
//! ## Reuse, not reimplementation
//!
//! Per the spec constraints, the gatherer does **not** reimplement search
//! or fetch — it delegates entirely to the provided `WebSearchTool` /
//! `WebFetchTool` implementations. In production these wrap the existing
//! `websearch` and `webfetch` tools in `crates/ragent-tools-extended`.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;

use crate::source::Source;

/// Search-result row returned by a [`WebSearchTool`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearchHit {
    /// Page URL.
    pub url: String,
    /// Page title as reported by the search provider (may be empty).
    pub title: String,
    /// One- or two-line snippet (may be empty).
    pub snippet: String,
}

/// Page body returned by a [`WebFetchTool`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebFetchedPage {
    /// Page URL — must match the URL passed in.
    pub url: String,
    /// Resolved page title (may be empty if the page lacked a title).
    pub title: String,
    /// Rendered text body of the page, in UTF-8. HTML tags should already
    /// have been stripped by the implementation.
    pub body: String,
}

/// Trait abstracting the existing `websearch` tool.
///
/// Production wiring delegates to the real tool from
/// `ragent-tools-extended`; tests provide an in-memory fake.
#[async_trait]
pub trait WebSearchTool: Send + Sync {
    /// Run a web search for `query` and return up to `max_results` hits.
    async fn search(&self, query: &str, max_results: usize) -> anyhow::Result<Vec<WebSearchHit>>;
}

/// Trait abstracting the existing `webfetch` tool.
#[async_trait]
pub trait WebFetchTool: Send + Sync {
    /// Fetch `url` and return the rendered page body.
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage>;
}

/// Errors emitted by [`WebGatherer`].
#[derive(Debug, thiserror::Error)]
pub enum WebGatherError {
    /// The configured search limit was zero — there is nothing to gather.
    #[error("web gatherer called with max_results = 0")]
    ZeroLimit,
    /// An empty topic was supplied.
    #[error("web gatherer called with an empty topic")]
    EmptyTopic,
}

/// Orchestrates a single web-gathering pass for one research topic.
///
/// `WebGatherer` is cheap to clone (internally an `Arc` pair) so the TUI
/// and CLI can hold one instance and call [`gather`] many times.
#[derive(Clone)]
pub struct WebGatherer {
    search: Arc<dyn WebSearchTool>,
    fetch: Arc<dyn WebFetchTool>,
}

impl std::fmt::Debug for WebGatherer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebGatherer").finish_non_exhaustive()
    }
}

impl WebGatherer {
    /// Construct a new gatherer from a search tool and a fetch tool.
    pub fn new(search: Arc<dyn WebSearchTool>, fetch: Arc<dyn WebFetchTool>) -> Self {
        Self { search, fetch }
    }

    /// Gather up to `max_results` web sources for `topic`.
    ///
    /// Returns an empty `Vec` (not an error) when:
    ///
    /// - The search tool returns no hits (FR-006 graceful degradation).
    /// - Every fetch call fails for transient reasons (logged at info,
    ///   not surfaced as an error to the caller — the local-gathering
    ///   phase can still produce a useful RESEARCH.md).
    ///
    /// Returns a [`WebGatherError`] only for programmer mistakes such as
    /// `max_results == 0` or `topic.is_empty()`.
    pub async fn gather(
        &self,
        topic: &str,
        max_results: usize,
    ) -> Result<Vec<Source>, WebGatherError> {
        if max_results == 0 {
            return Err(WebGatherError::ZeroLimit);
        }
        if topic.trim().is_empty() {
            return Err(WebGatherError::EmptyTopic);
        }

        tracing::info!(topic, max_results, "research: starting web-gathering phase");

        // 1. Discover candidates via search.
        let hits = match self.search.search(topic, max_results).await {
            Ok(hits) => hits,
            Err(e) => {
                tracing::warn!(error = %e, "research: websearch failed; returning no web sources");
                return Ok(Vec::new());
            }
        };

        if hits.is_empty() {
            tracing::info!("research: websearch returned 0 hits");
            return Ok(Vec::new());
        }

        // 2. Fetch each candidate in order until we have `max_results`.
        let mut sources = Vec::with_capacity(hits.len().min(max_results));
        for (index, hit) in hits.into_iter().enumerate().take(max_results) {
            match self.fetch.fetch(&hit.url).await {
                Ok(page) => {
                    let title = if page.title.is_empty() {
                        hit.title
                    } else {
                        page.title
                    };
                    let body_path = web_body_path(index);
                    tracing::info!(
                        url = %page.url,
                        title = %title,
                        body_path = %body_path.display(),
                        "research: captured web source"
                    );
                    sources.push(Source::Web {
                        url: page.url,
                        title,
                        captured_at: Utc::now(),
                        body_path,
                    });
                }
                Err(e) => {
                    // One failed fetch is not a reason to abort the whole
                    // gathering pass; skip it and move on.
                    tracing::warn!(url = %hit.url, error = %e, "research: webfetch failed; skipping");
                }
            }
        }

        tracing::info!(
            count = sources.len(),
            "research: web-gathering phase complete"
        );
        Ok(sources)
    }
}

/// Compute the zero-padded supporting-file path for the Nth web source.
///
/// Index 0 → `web-01.md`, index 1 → `web-02.md`, etc. The path is
/// relative to the research item directory (`research/<name>/`).
fn web_body_path(index: usize) -> PathBuf {
    PathBuf::from(format!("sources/web-{:02}.md", index + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// In-memory `WebSearchTool` for tests.
    #[derive(Default)]
    struct FakeSearch {
        hits: Vec<WebSearchHit>,
        calls: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl WebSearchTool for FakeSearch {
        async fn search(
            &self,
            query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            self.calls.lock().unwrap().push(query.to_string());
            Ok(self.hits.clone())
        }
    }

    /// In-memory `WebFetchTool` for tests. Each URL maps to an optional
    /// `WebFetchedPage`; missing URLs produce an error.
    #[derive(Default)]
    struct FakeFetch {
        pages: std::collections::HashMap<String, WebFetchedPage>,
        fail_urls: Vec<String>,
        calls: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl WebFetchTool for FakeFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            self.calls.lock().unwrap().push(url.to_string());
            if self.fail_urls.iter().any(|u| u == url) {
                anyhow::bail!("simulated fetch failure for {url}");
            }
            self.pages
                .get(url)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no fake page registered for {url}"))
        }
    }

    fn gatherer_with(
        hits: Vec<WebSearchHit>,
        pages: std::collections::HashMap<String, WebFetchedPage>,
        fail_urls: Vec<String>,
    ) -> (WebGatherer, Arc<FakeSearch>, Arc<FakeFetch>) {
        let search = Arc::new(FakeSearch {
            hits,
            calls: Mutex::new(Vec::new()),
        });
        let fetch = Arc::new(FakeFetch {
            pages,
            fail_urls,
            calls: Mutex::new(Vec::new()),
        });
        let g = WebGatherer::new(search.clone(), fetch.clone());
        (g, search, fetch)
    }

    #[tokio::test]
    async fn gather_returns_empty_vec_when_search_returns_no_hits() {
        let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let sources = g.gather("rust async", 5).await.unwrap();
        assert!(sources.is_empty());
    }

    #[tokio::test]
    async fn gather_returns_empty_vec_when_search_tool_errors() {
        struct AlwaysFailSearch;
        #[async_trait]
        impl WebSearchTool for AlwaysFailSearch {
            async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
                anyhow::bail!("network down")
            }
        }
        struct OkFetch;
        #[async_trait]
        impl WebFetchTool for OkFetch {
            async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    url: "u".into(),
                    title: "t".into(),
                    body: "b".into(),
                })
            }
        }
        let g = WebGatherer::new(Arc::new(AlwaysFailSearch), Arc::new(OkFetch));
        let sources = g.gather("topic", 5).await.unwrap();
        assert!(
            sources.is_empty(),
            "search failure must not surface as an error"
        );
    }

    #[tokio::test]
    async fn gather_creates_web_source_per_hit_with_sequential_body_paths() {
        let hits = vec![
            WebSearchHit {
                url: "https://a.example".into(),
                title: "A".into(),
                snippet: "".into(),
            },
            WebSearchHit {
                url: "https://b.example".into(),
                title: "B".into(),
                snippet: "".into(),
            },
            WebSearchHit {
                url: "https://c.example".into(),
                title: "C".into(),
                snippet: "".into(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://a.example".into(),
            WebFetchedPage {
                url: "https://a.example".into(),
                title: "A — resolved".into(),
                body: "body a".into(),
            },
        );
        pages.insert(
            "https://b.example".into(),
            WebFetchedPage {
                url: "https://b.example".into(),
                title: "B — resolved".into(),
                body: "body b".into(),
            },
        );
        pages.insert(
            "https://c.example".into(),
            WebFetchedPage {
                url: "https://c.example".into(),
                title: "".into(), // empty title should fall back to search hit title
                body: "body c".into(),
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("topic", 5).await.unwrap();
        assert_eq!(sources.len(), 3);

        for (i, src) in sources.iter().enumerate() {
            let Source::Web {
                url,
                title,
                body_path,
                ..
            } = src
            else {
                panic!("expected Source::Web, got {src:?}");
            };
            assert_eq!(
                body_path.as_path(),
                PathBuf::from(format!("sources/web-{:02}.md", i + 1)).as_path()
            );
            assert!(!url.is_empty());
            assert!(!title.is_empty());
        }
        // The third source had an empty page title, so it should have
        // fallen back to the search-hit title "C".
        if let Source::Web { title, .. } = &sources[2] {
            assert_eq!(title, "C");
        }
    }

    #[tokio::test]
    async fn gather_skips_individual_fetch_failures() {
        let hits = vec![
            WebSearchHit {
                url: "https://ok".into(),
                title: "OK".into(),
                snippet: "".into(),
            },
            WebSearchHit {
                url: "https://bad".into(),
                title: "Bad".into(),
                snippet: "".into(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://ok".into(),
            WebFetchedPage {
                url: "https://ok".into(),
                title: "OK".into(),
                body: "b".into(),
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, vec!["https://bad".into()]);
        let sources = g.gather("topic", 5).await.unwrap();
        assert_eq!(
            sources.len(),
            1,
            "failed fetch should be skipped, not abort"
        );
        if let Source::Web { url, .. } = &sources[0] {
            assert_eq!(url, "https://ok");
        }
    }

    #[tokio::test]
    async fn gather_respects_max_results() {
        let hits = vec![
            WebSearchHit {
                url: "https://1".into(),
                title: "1".into(),
                snippet: "".into(),
            },
            WebSearchHit {
                url: "https://2".into(),
                title: "2".into(),
                snippet: "".into(),
            },
            WebSearchHit {
                url: "https://3".into(),
                title: "3".into(),
                snippet: "".into(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        for u in ["https://1", "https://2", "https://3"] {
            pages.insert(
                u.into(),
                WebFetchedPage {
                    url: u.into(),
                    title: u.into(),
                    body: "b".into(),
                },
            );
        }
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("topic", 2).await.unwrap();
        assert_eq!(sources.len(), 2, "must not exceed max_results");
    }

    #[tokio::test]
    async fn gather_rejects_zero_max_results() {
        let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let err = g.gather("topic", 0).await.unwrap_err();
        assert!(matches!(err, WebGatherError::ZeroLimit));
    }

    #[tokio::test]
    async fn gather_rejects_empty_topic() {
        let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let err = g.gather("   ", 5).await.unwrap_err();
        assert!(matches!(err, WebGatherError::EmptyTopic));
    }

    #[tokio::test]
    async fn gather_records_search_call() {
        let (g, search, _) =
            gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let _ = g.gather("rust async", 5).await.unwrap();
        let calls = search.calls.lock().unwrap();
        assert_eq!(calls.as_slice(), &["rust async".to_string()]);
    }

    #[test]
    fn web_body_path_zero_pads_and_uses_one_based_index() {
        assert_eq!(web_body_path(0), PathBuf::from("sources/web-01.md"));
        assert_eq!(web_body_path(8), PathBuf::from("sources/web-09.md"));
        assert_eq!(web_body_path(9), PathBuf::from("sources/web-10.md"));
    }
}

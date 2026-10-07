//! Inline tests for `research_adapter.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::tool::ToolOutput;

#[test]
fn test_parse_websearch_output_from_metadata() {
    let metadata = serde_json::json!({
        "query": "example query",
        "count": 2,
        "line_count": 6,
        "results": [
            {"title": "Example Site", "url": "https://example.com", "snippet": "A useful example page."},
            {"title": "Another Site", "url": "https://another.example.com", "snippet": ""}
        ]
    });
    let hits = ragent_tools_extended::websearch::hits_from_metadata(&metadata)
        .into_iter()
        .map(|r| WebSearchHit {
            title: r.title,
            url: r.url,
            snippet: r.snippet,
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            author: r.author,
        })
        .collect::<Vec<_>>();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].title, "Example Site");
    assert_eq!(hits[0].url, "https://example.com");
    assert_eq!(hits[0].snippet, "A useful example page.");
    assert_eq!(hits[1].title, "Another Site");
    assert_eq!(hits[1].url, "https://another.example.com");
}

#[test]
fn test_parse_websearch_output() {
    let text = "1. Example Site\n   https://example.com\n   A useful example page.\n2. Another Site\n   https://another.example.com\n";
    let hits = parse_websearch_output(text);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].title, "Example Site");
    assert_eq!(hits[0].url, "https://example.com");
    assert_eq!(hits[0].snippet, "A useful example page.");
    assert_eq!(hits[1].title, "Another Site");
    assert_eq!(hits[1].url, "https://another.example.com");
}

#[test]
fn test_parse_grep_output() {
    let text =
        "5 matches in 2 files searched\n\nsrc/foo.rs:12:let x = 1;\nsrc/bar.rs:3:fn main() {}";
    let matches = parse_grep_output(text);
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].line, 12);
    assert_eq!(matches[0].text, "let x = 1;");
    assert_eq!(matches[1].line, 3);
}

#[test]
fn test_parse_specs_list() {
    let text = "/project/specs/
├── auth-refactor/
├── model-router/
└── researchsystem/
";
    let ids = parse_specs_list(text);
    assert_eq!(ids, vec!["auth-refactor", "model-router", "researchsystem"]);
}

#[test]
fn test_parse_mf_fetch_youtube_body_extracts_title_and_transcript() {
    let content = "mf_fetch: https://www.youtube.com/watch?v=dQw4w9WgXcQ\nStatus: 200\nContent type: text/plain\n\nTitle: Never Gonna Give You Up\n\n[Intro]\nWe're no strangers to love\n[Verse 1]\nYou know the rules and so do I\n";
    let (title, transcript) = parse_mf_fetch_youtube_body(content).expect("parse succeeded");
    assert_eq!(title, "Never Gonna Give You Up");
    assert_eq!(
        transcript,
        "[Intro]\nWe're no strangers to love\n[Verse 1]\nYou know the rules and so do I"
    );
}

#[test]
fn test_parse_mf_fetch_youtube_body_missing_title_line_returns_none() {
    let content = "mf_fetch: https://example.com/\nStatus: 200\n\nSome plain content without a title prefix.\n";
    assert!(parse_mf_fetch_youtube_body(content).is_none());
}

#[test]
fn test_agent_web_fetch_tool_uses_mf_fetch_youtube_title() {
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct FakeMfFetchTool {
        called_with_youtube: AtomicBool,
    }

    #[async_trait]
    impl AgentTool for FakeMfFetchTool {
        fn name(&self) -> &'static str {
            "mf_fetch"
        }

        fn description(&self) -> &'static str {
            "fake"
        }

        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }

        fn permission_category(&self) -> &'static str {
            "web:read"
        }

        async fn execute(
            &self,
            input: serde_json::Value,
            _ctx: &AgentToolContext,
        ) -> Result<ToolOutput> {
            if input
                .get("url")
                .and_then(|v| v.as_str())
                .is_some_and(|u| u.contains("youtube.com"))
            {
                self.called_with_youtube.store(true, Ordering::SeqCst);
            }
            Ok(ToolOutput {
                content: "mf_fetch: https://www.youtube.com/watch?v=abc\nStatus: 200\nContent type: text/plain\n\nTitle: A Video Title\n\nTranscript line one\nTranscript line two".to_string(),
                metadata: Some(serde_json::json!({
                    "page_type": "youtube",
                    "content_type": "text/plain",
                    "title": "A Video Title",
                })),
            })
        }
    }

    let fake = Arc::new(FakeMfFetchTool {
        called_with_youtube: AtomicBool::new(false),
    });
    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let page = rt.block_on(async {
        let fetcher = AgentWebFetchTool {
            tool: fake.clone(),
            ctx: AgentToolContext {
                session_id: "test".to_string(),
                working_dir: std::env::current_dir().expect("current_dir"),
                event_bus: Arc::new(crate::event::EventBus::new(8)),
                storage: None,
                agent_manager: None,
                active_model: None,
                provider_registry: None,
                team_context: None,
                team_manager: None,
                code_index: None,
                bg_service: None,
                spec_manager: None,
                active_spec_id: None,
                config: None,
                allowed_roots: Vec::new(),
                tool_registry: Arc::new(crate::tool::create_default_registry()),
                read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
                cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
                permission_checker: None,
                canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
            },
            legacy_verifier: None,
        };
        fetcher
            .fetch("https://www.youtube.com/watch?v=abc")
            .await
            .expect("fetch succeeded")
    });
    assert!(fake.called_with_youtube.load(Ordering::SeqCst));
    assert_eq!(page.title, "A Video Title");
    assert_eq!(page.page_type.as_deref(), Some("youtube"));
    assert_eq!(page.content_type.as_deref(), Some("text/plain"));
    assert_eq!(&*page.body, "Transcript line one\nTranscript line two");
}

#[test]
fn test_agent_web_fetch_tool_youtube_error_output_fails_fetch() {
    // A YouTube watch page whose caption extraction failed: `mf_fetch`
    // reports it via metadata (`error` + `content_ok: false`) rather than
    // by aborting the tool call. The adapter must turn that into a fetch
    // error so the gatherer suppresses the video with the real reason
    // instead of storing the placeholder bracket text as the source body
    // and suppressing it later with an opaque "content too short" gate.
    use async_trait::async_trait;

    struct FakeYoutubeErrorTool;

    #[async_trait]
    impl AgentTool for FakeYoutubeErrorTool {
        fn name(&self) -> &'static str {
            "mf_fetch"
        }
        fn description(&self) -> &'static str {
            "fake"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        fn permission_category(&self) -> &'static str {
            "web:read"
        }
        async fn execute(
            &self,
            _input: serde_json::Value,
            _ctx: &AgentToolContext,
        ) -> Result<ToolOutput> {
            Ok(ToolOutput {
                content: "mf_fetch: https://www.youtube.com/watch?v=abc\nStatus: 200\nContent type: text/html\nPage type: youtube\nContent OK: false\nFetcher: http\n\nTitle: Some Video\n\n[YouTube transcript extraction failed: no caption tracks available for this YouTube video]".to_string(),
                metadata: Some(serde_json::json!({
                    "page_type": "youtube",
                    "content_type": "text/html",
                    "title": "Some Video",
                    "content_ok": false,
                    "next_action": "this video may not have captions; try a different source",
                    "error": "no caption tracks available for this YouTube video",
                })),
            })
        }
    }

    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let err = rt.block_on(async {
        let fetcher = AgentWebFetchTool {
            tool: Arc::new(FakeYoutubeErrorTool),
            ctx: test_tool_context(),
            legacy_verifier: None,
        };
        fetcher
            .fetch("https://www.youtube.com/watch?v=abc")
            .await
            .expect_err("youtube error output must fail the fetch")
    });
    assert!(
        err.to_string().contains("no caption tracks available"),
        "error should carry the real failure reason, got: {err}"
    );
}

#[test]
fn test_agent_web_fetch_tool_content_not_ok_fails_fetch() {
    // Generic mf_fetch failure metadata (e.g. an HTTP body-read error or a
    // youtube error output) carries `content_ok: false` without an
    // `error` string - the adapter falls back to `next_action` as the
    // failure reason.
    use async_trait::async_trait;

    struct FakeNotOkTool;

    #[async_trait]
    impl AgentTool for FakeNotOkTool {
        fn name(&self) -> &'static str {
            "mf_fetch"
        }
        fn description(&self) -> &'static str {
            "fake"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        fn permission_category(&self) -> &'static str {
            "web:read"
        }
        async fn execute(
            &self,
            _input: serde_json::Value,
            _ctx: &AgentToolContext,
        ) -> Result<ToolOutput> {
            Ok(ToolOutput {
                content: "mf_fetch: https://example.com\nStatus: 500\nContent type: text/html\nContent OK: false\n\nbroken".to_string(),
                metadata: Some(serde_json::json!({
                    "content_type": "text/html",
                    "content_ok": false,
                    "next_action": "retry, check connectivity, or try a different URL",
                    "extraction_method": "readability",
                })),
            })
        }
    }

    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let err = rt.block_on(async {
        let fetcher = AgentWebFetchTool {
            tool: Arc::new(FakeNotOkTool),
            ctx: test_tool_context(),
            legacy_verifier: None,
        };
        fetcher
            .fetch("https://example.com")
            .await
            .expect_err("content_ok=false output must fail the fetch")
    });
    assert!(
        err.to_string().contains("retry, check connectivity"),
        "error should use next_action as the reason, got: {err}"
    );
}

#[test]
fn test_agent_web_fetch_tool_content_not_ok_takes_priority_over_readability() {
    // A failed HTML fetch (`content_ok: false`, fallback extraction) must
    // be rejected with the mf_fetch failure reason - not the readability
    // message - because the page never produced real content at all.
    use async_trait::async_trait;

    struct FakeFailedHtmlTool;

    #[async_trait]
    impl AgentTool for FakeFailedHtmlTool {
        fn name(&self) -> &'static str {
            "mf_fetch"
        }
        fn description(&self) -> &'static str {
            "fake"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        fn permission_category(&self) -> &'static str {
            "web"
        }
        async fn execute(
            &self,
            _input: serde_json::Value,
            _ctx: &AgentToolContext,
        ) -> anyhow::Result<ToolOutput> {
            Ok(ToolOutput {
                content: "error placeholder body".to_string(),
                metadata: Some(serde_json::json!({
                    "content_type": "text/html",
                    "content_ok": false,
                    "error": "failed to read response body",
                    "extraction_method": "html2text",
                })),
            })
        }
    }

    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let err = rt.block_on(async {
        let fetcher = AgentWebFetchTool {
            tool: Arc::new(FakeFailedHtmlTool),
            ctx: test_tool_context(),
            legacy_verifier: None,
        };
        fetcher
            .fetch("https://example.com")
            .await
            .expect_err("failed fetch must error out")
    });
    let msg = err.to_string();
    assert!(
        msg.contains("failed to read response body"),
        "expected mf_fetch error reason, got: {msg}"
    );
    assert!(
        !msg.contains("readability"),
        "readability check must not run for failed fetches, got: {msg}"
    );
}

// -- Mandatory readability enforcement tests ----------------------------

/// Build a minimal `AgentToolContext` for fetch-adapter tests.
fn test_tool_context() -> AgentToolContext {
    AgentToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::current_dir().expect("current_dir"),
        event_bus: Arc::new(crate::event::EventBus::new(8)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        tool_registry: Arc::new(crate::tool::create_default_registry()),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

/// A fake `mf_fetch` tool whose `extraction_method` metadata is
/// configurable per test.
struct FakeMfFetch {
    extraction_method: Option<&'static str>,
    content_type: &'static str,
}

#[async_trait::async_trait]
impl AgentTool for FakeMfFetch {
    fn name(&self) -> &'static str {
        "mf_fetch"
    }
    fn description(&self) -> &'static str {
        "fake"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    fn permission_category(&self) -> &'static str {
        "web"
    }
    async fn execute(
        &self,
        _input: serde_json::Value,
        _ctx: &AgentToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let mut metadata = serde_json::json!({
            "content_type": self.content_type,
        });
        if let Some(method) = self.extraction_method {
            metadata["extraction_method"] = serde_json::json!(method);
        }
        Ok(ToolOutput {
            content: "Article body text extracted from the page".to_string(),
            metadata: Some(metadata),
        })
    }
}

/// A fake legacy `webfetch` tool.
struct FakeLegacyWebfetch;

#[async_trait::async_trait]
impl AgentTool for FakeLegacyWebfetch {
    fn name(&self) -> &'static str {
        "webfetch"
    }
    fn description(&self) -> &'static str {
        "fake"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    fn permission_category(&self) -> &'static str {
        "web"
    }
    async fn execute(
        &self,
        _input: serde_json::Value,
        _ctx: &AgentToolContext,
    ) -> anyhow::Result<ToolOutput> {
        Ok(ToolOutput {
            content: "Fallback-extracted page text".to_string(),
            metadata: None,
        })
    }
}

/// A fake `mf_fetch` tool that records the requested URL and serves a
/// raw-text body, simulating the `raw.githubusercontent.com` endpoint.
#[derive(Default)]
struct RecordingMfFetch {
    requested_urls: std::sync::Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl AgentTool for RecordingMfFetch {
    fn name(&self) -> &'static str {
        "mf_fetch"
    }
    fn description(&self) -> &'static str {
        "fake"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    fn permission_category(&self) -> &'static str {
        "web"
    }
    async fn execute(
        &self,
        input: serde_json::Value,
        _ctx: &AgentToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        self.requested_urls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(url);
        Ok(ToolOutput {
            content: "raw file body".to_string(),
            metadata: Some(serde_json::json!({
                "content_type": "text/plain; charset=utf-8",
            })),
        })
    }
}

#[test]
fn test_mf_fetch_readability_method_accepted() {
    let fake = Arc::new(FakeMfFetch {
        extraction_method: Some("readability"),
        content_type: "text/html",
    });
    let rt = tokio::runtime::Runtime::new().unwrap();
    let page = rt.block_on(async {
        let fetcher = AgentWebFetchTool {
            tool: fake,
            ctx: test_tool_context(),
            legacy_verifier: None,
        };
        fetcher.fetch("https://example.com/article").await
    });
    assert!(
        page.is_ok(),
        "readability-extracted page must be accepted: {page:?}"
    );
}

#[test]
fn test_mf_fetch_html2text_fallback_rejected() {
    let fake = Arc::new(FakeMfFetch {
        extraction_method: Some("html2text"),
        content_type: "text/html",
    });
    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let result = rt.block_on(async {
        let fetcher = AgentWebFetchTool {
            tool: fake,
            ctx: test_tool_context(),
            legacy_verifier: None,
        };
        fetcher.fetch("https://example.com/article").await
    });
    let err = result.expect_err("html2text fallback must be rejected");
    assert!(
        err.to_string().contains("readability extraction failed"),
        "error should explain the readability requirement: {err}"
    );
    // The typed failure carries the fine-grained cause so the research
    // gatherer can break it out into the `extr` column.
    assert_eq!(
        err.downcast_ref::<FetchFailure>().map(|f| f.kind),
        Some(FetchFailureKind::Extraction),
        "readability rejection must be an Extraction FetchFailure: {err}"
    );
}

#[test]
fn test_fetch_failure_classifies_by_page_type_and_message() {
    assert_eq!(
        fetch_failure(Some("paywall"), "content_ok = false").kind,
        FetchFailureKind::AuthWall
    );
    assert_eq!(
        fetch_failure(Some("auth_wall"), "login required").kind,
        FetchFailureKind::AuthWall
    );
    assert_eq!(
        fetch_failure(Some("js_shell"), "no static content").kind,
        FetchFailureKind::JsShell
    );
    assert_eq!(
        fetch_failure(None, "request timed out after 30s").kind,
        FetchFailureKind::Timeout
    );
    assert_eq!(
        fetch_failure(None, "URL rejected by SSRF security check").kind,
        FetchFailureKind::SecurityBlocked
    );
    assert_eq!(
        fetch_failure(None, "HTTP status 404 not found").kind,
        FetchFailureKind::HttpStatus
    );
    assert_eq!(
        fetch_failure(None, "pdf extraction produced no text").kind,
        FetchFailureKind::Extraction
    );
    assert_eq!(
        fetch_failure(None, "connection reset by peer").kind,
        FetchFailureKind::Network
    );
}

#[test]
fn test_mf_fetch_missing_extraction_method_rejected() {
    // Older mf_fetch metadata (cache entries without the signal, manual
    // envelopes) must be treated as non-readability and rejected.
    let fake = Arc::new(FakeMfFetch {
        extraction_method: None,
        content_type: "text/html",
    });
    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let result = rt.block_on(async {
        let fetcher = AgentWebFetchTool {
            tool: fake,
            ctx: test_tool_context(),
            legacy_verifier: None,
        };
        fetcher.fetch("https://example.com/article").await
    });
    assert!(
        result.is_err(),
        "missing extraction_method must be rejected as non-readability"
    );
}

#[test]
fn test_mf_fetch_pdf_bypasses_readability_check() {
    // PDFs are extracted with pdf-extract instead of readability; the
    // mandatory guarantee only applies to HTML pages.
    let fake = Arc::new(FakeMfFetch {
        extraction_method: None,
        content_type: "application/pdf",
    });
    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let page = rt.block_on(async {
        let fetcher = AgentWebFetchTool {
            tool: fake,
            ctx: test_tool_context(),
            legacy_verifier: None,
        };
        fetcher.fetch("https://example.com/paper.pdf").await
    });
    assert!(
        page.is_ok(),
        "PDF sources must bypass the readability check: {page:?}"
    );
}

#[test]
fn test_mf_fetch_non_html_content_type_bypasses_readability_check() {
    // Non-HTML content types (e.g. `text/plain` from
    // raw.githubusercontent.com) are served verbatim by `mf_fetch` - no
    // extraction chain runs, so there is no fallback to reject. The body
    // IS the document content.
    let fake = Arc::new(FakeMfFetch {
        extraction_method: None,
        content_type: "text/plain; charset=utf-8",
    });
    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let page = rt.block_on(async {
        let fetcher = AgentWebFetchTool {
            tool: fake,
            ctx: test_tool_context(),
            legacy_verifier: None,
        };
        fetcher
            .fetch("https://raw.githubusercontent.com/o/r/main/README.md")
            .await
    });
    assert!(
        page.is_ok(),
        "text/plain sources must bypass the readability check: {page:?}"
    );
}

#[test]
fn test_normalize_github_blob_url_rewrites_to_raw() {
    let rewritten = normalize_github_blob_url(
        "https://github.com/MajorCommotion/hermes-agent-setup-guide/blob/main/HERMES-AGENT-SETUP-GUIDE.md",
    );
    assert_eq!(
        rewritten,
        "https://raw.githubusercontent.com/MajorCommotion/hermes-agent-setup-guide/main/HERMES-AGENT-SETUP-GUIDE.md"
    );
}

#[test]
fn test_normalize_github_blob_url_multi_segment_ref() {
    let rewritten =
        normalize_github_blob_url("https://github.com/o/r/blob/refs/heads/main/docs/guide.md");
    assert_eq!(
        rewritten,
        "https://raw.githubusercontent.com/o/r/refs/heads/main/docs/guide.md"
    );
}

#[test]
fn test_normalize_github_blob_url_leaves_non_blob_urls_unchanged() {
    // Repo root, issue, pull request, and short directory-listing URLs
    // must pass through untouched.
    for url in [
        "https://github.com/o/r",
        "https://github.com/o/r/issues/42",
        "https://github.com/o/r/pull/7",
        "https://github.com/o/r/blob/main",
        "https://example.com/blob/main/file.md",
    ] {
        assert_eq!(
            normalize_github_blob_url(url),
            url,
            "url must be unchanged: {url}"
        );
    }
}

#[test]
fn test_github_blob_fetch_uses_rewritten_raw_url() {
    // End-to-end: fetching a github.com blob URL must hit the rewritten
    // raw.githubusercontent.com URL, and the recorded page URL must be the
    // rewritten one.
    let fake = Arc::new(RecordingMfFetch::default());
    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let page = rt.block_on(async {
        let fetcher = AgentWebFetchTool {
            tool: fake,
            ctx: test_tool_context(),
            legacy_verifier: None,
        };
        fetcher
            .fetch(
                "https://github.com/MajorCommotion/hermes-agent-setup-guide/blob/main/HERMES-AGENT-SETUP-GUIDE.md",
            )
            .await
    });
    let page = page.expect("rewritten raw URL must be accepted");
    assert_eq!(
        page.url,
        "https://raw.githubusercontent.com/MajorCommotion/hermes-agent-setup-guide/main/HERMES-AGENT-SETUP-GUIDE.md"
    );
}

#[test]
fn test_legacy_webfetch_fallback_verified_rejected() {
    // When the legacy webfetch tool is used and the raw-HTML re-check
    // says readability could not extract, the page must be rejected.
    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let result = rt.block_on(async {
        let fetcher = AgentWebFetchTool {
            tool: Arc::new(FakeLegacyWebfetch),
            ctx: test_tool_context(),
            legacy_verifier: Some(Box::new(|| false)),
        };
        fetcher.fetch("https://example.com/article").await
    });
    assert!(
        result.is_err(),
        "legacy webfetch page failing the readability re-check must be rejected"
    );
}

#[test]
fn test_legacy_webfetch_fallback_verified_accepted() {
    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let page = rt.block_on(async {
        let fetcher = AgentWebFetchTool {
            tool: Arc::new(FakeLegacyWebfetch),
            ctx: test_tool_context(),
            legacy_verifier: Some(Box::new(|| true)),
        };
        fetcher.fetch("https://example.com/article").await
    });
    assert!(
        page.is_ok(),
        "legacy webfetch page passing the readability re-check must be accepted: {page:?}"
    );
}

#[test]
fn test_readability_extract_ok_on_article_html() {
    let body = "Readability is a content-extraction library. ".repeat(40);
    let html = format!(
        "<html><head><title>On Readability</title></head>\
         <body><article><h1>On Readability</h1><p>{body}</p></article></body></html>"
    );
    assert!(
        readability_extract_ok(&html, "https://example.com/on-readability"),
        "real readability must accept a long article page"
    );
}

#[test]
fn test_readability_extract_ok_rejects_nav_only_html() {
    // Tiny pages with no article body must not pass the readability check.
    let html =
        "<html><head><title>Nav</title></head><body><nav><a href='/'>home</a></nav></body></html>";
    assert!(
        !readability_extract_ok(html, "https://example.com/nav"),
        "nav-only page must fail the readability check"
    );
}

#[test]
fn test_readability_extract_ok_rejects_invalid_url() {
    let html = "<html><body><article><p>text</p></article></body></html>";
    assert!(!readability_extract_ok(html, "not-a-url"));
}

#[test]
fn test_build_research_session_wires_available_tools() {
    use crate::event::EventBus;
    use crate::tool::create_default_registry;
    let registry = Arc::new(create_default_registry());
    let manager = ResearchManager::new("research");
    let session = build_research_session(
        &registry,
        manager,
        "test-session".into(),
        std::env::current_dir().expect("current_dir"),
        Arc::new(EventBus::new(256)),
        None,
        None,
        None,
        None,
        None,
    );
    // Debug output prints has_web/has_local flags.
    let debug = format!("{:?}", session);
    assert!(
        debug.contains("has_web: true"),
        "default registry should provide websearch+webfetch tools: {debug}"
    );
    assert!(
        debug.contains("has_local: true"),
        "default registry should provide glob/grep/read/list tools: {debug}"
    );
}

// -- Engine-exclusion adapter tests (spec researchnoacc, T-007) ---------

/// A fake search tool that records the JSON input it was invoked with and
/// emits a `results` array shaped like `mf_search` metadata.
struct RecordingSearchTool {
    tool_name: &'static str,
    inputs: std::sync::Mutex<Vec<serde_json::Value>>,
}

#[async_trait::async_trait]
impl AgentTool for RecordingSearchTool {
    fn name(&self) -> &'static str {
        self.tool_name
    }
    fn description(&self) -> &'static str {
        "fake search"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    fn permission_category(&self) -> &'static str {
        "web"
    }
    async fn execute(
        &self,
        input: serde_json::Value,
        _ctx: &AgentToolContext,
    ) -> Result<ToolOutput> {
        self.inputs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(input);
        Ok(ToolOutput {
            content: "mf_search: \"q\"\nResults: 1\n".to_string(),
            metadata: Some(serde_json::json!({
                "results": [{
                    "title": "T",
                    "url": "https://example.com",
                    "snippet": "s",
                    "source": "wikipedia",
                    "search_engine": "wikipedia",
                }],
            })),
        })
    }
}

fn recording_search_adapter(
    tool_name: &'static str,
) -> (Arc<dyn AgentTool>, Arc<RecordingSearchTool>) {
    let concrete = Arc::new(RecordingSearchTool {
        tool_name,
        inputs: std::sync::Mutex::new(Vec::new()),
    });
    let as_dyn: Arc<dyn AgentTool> = concrete.clone();
    (as_dyn, concrete)
}

#[test]
fn test_adapter_forwards_exclusions_to_mf_search() {
    // FR-004/FR-006: the academic engine names must reach the underlying
    // `mf_search` call as its `exclude_engines` parameter.
    let (as_dyn, concrete) = recording_search_adapter("mf_search");
    let adapter = AgentWebSearchTool {
        tool: as_dyn,
        ctx: test_tool_context(),
        tool_name: "mf_search".to_string(),
        supports_exclusions: true,
    };

    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let hits = rt
        .block_on(adapter.search_with_exclusions("rust", 7, &["openalex"]))
        .expect("search should succeed");
    assert_eq!(hits.len(), 1);

    let inputs = concrete
        .inputs
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(inputs.len(), 1);
    assert_eq!(
        inputs[0]["exclude_engines"],
        serde_json::json!(["openalex"])
    );
    assert_eq!(inputs[0]["query"], "rust");
    assert_eq!(inputs[0]["max_results"], 7);
}

#[test]
fn test_adapter_omits_exclusions_when_no_engines_named() {
    // FR-011: an empty exclusion list produces the original call shape.
    let (as_dyn, concrete) = recording_search_adapter("mf_search");
    let adapter = AgentWebSearchTool {
        tool: as_dyn,
        ctx: test_tool_context(),
        tool_name: "mf_search".to_string(),
        supports_exclusions: true,
    };

    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    rt.block_on(adapter.search_with_exclusions("rust", 5, &[]))
        .expect("search should succeed");

    let inputs = concrete
        .inputs
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(
        inputs[0].get("exclude_engines").is_none(),
        "no exclusion parameter should be sent when nothing is excluded"
    );
}

#[test]
fn test_adapter_omits_exclusions_for_legacy_websearch() {
    // The legacy `websearch` schema is `additionalProperties: false`, so
    // the exclusion parameter must never be sent to it (FR-004, FR-011).
    let (as_dyn, concrete) = recording_search_adapter("websearch");
    let adapter = AgentWebSearchTool {
        tool: as_dyn,
        ctx: test_tool_context(),
        tool_name: "websearch".to_string(),
        supports_exclusions: false,
    };

    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    rt.block_on(adapter.search_with_exclusions("rust", 5, &["openalex"]))
        .expect("search should succeed");

    let inputs = concrete
        .inputs
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(
        inputs[0].get("exclude_engines").is_none(),
        "legacy websearch must not receive the exclusion parameter"
    );
}

#[test]
fn test_adapter_base_search_never_sends_exclusions() {
    // The base `search` path (no exclusions) keeps the pre-change shape.
    let (as_dyn, concrete) = recording_search_adapter("mf_search");
    let adapter = AgentWebSearchTool {
        tool: as_dyn,
        ctx: test_tool_context(),
        tool_name: "mf_search".to_string(),
        supports_exclusions: true,
    };

    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    rt.block_on(adapter.search("rust", 5))
        .expect("search should succeed");

    let inputs = concrete
        .inputs
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(inputs[0].get("exclude_engines").is_none());
}

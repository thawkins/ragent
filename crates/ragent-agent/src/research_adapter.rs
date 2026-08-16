//! Adapters that wire the agent tool registry into the research system's
//! web/local gatherers.
//!
//! The research crate defines its own small traits ([`ragent_research::WebSearchTool`],
//! [`ragent_research::WebFetchTool`], [`ragent_research::LocalTool`]).  The agent
//! crate has access to the full tool registry (`glob`, `grep`, `read`, `list`,
//! `websearch`, `webfetch`), so this module provides thin wrappers that implement
//! the research traits by calling those agent tools.
//!
//! This module is shared between the TUI, the HTTP server, and the CLI so each
//! entry point builds research sessions the same way and web sources are not
//! silently dropped.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;

use crate::{
    Config,
    agent::ModelRef,
    event::EventBus,
    provider::ProviderRegistry,
    storage::Storage,
    tool::{Tool as AgentTool, ToolContext as AgentToolContext, ToolRegistry},
};
use ragent_research::{
    AnalysisEngine, Critic, GrepMatch, HeuristicPlanner, HeuristicQueryDecomposer,
    LlmAnalysisEngine, LlmPlanner, LlmQueryDecomposer, LocalGatherer, LocalTool,
    NoopAnalysisEngine, Planner, QueryDecomposer, ResearchManager, ResearchSession, SimpleCritic,
    WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool,
};

/// Build a [`ResearchSession`] backed by the agent tool `registry`.
///
/// If `websearch`/`webfetch` are present, web gathering is enabled.  If
/// `glob`/`grep`/`read`/`list` are present, local gathering is enabled.
/// Missing tools are silently omitted so the session degrades gracefully.
///
/// If `active_model` is supplied and a `ProviderRegistry` is available, an
/// LLM-backed analysis engine is wired in so the final `RESEARCH.md` contains
/// synthesized summary/findings/cross-references/open questions.
/// * `research_name` — when `Some`, is sanitised and used in the JSONL
///   gather-log file name (`log/research-<name>-<ts>-<rand>-web.jsonl`)
///   recording every considered/captured/rejected URL.
#[allow(clippy::too_many_arguments)]
pub fn build_research_session(
    registry: &Arc<ToolRegistry>,
    manager: ResearchManager,
    session_id: String,
    working_dir: PathBuf,
    event_bus: Arc<EventBus>,
    storage: Option<Arc<Storage>>,
    config: Option<Arc<Config>>,
    provider_registry: Option<Arc<ProviderRegistry>>,
    active_model: Option<ModelRef>,
    research_name: Option<&str>,
) -> ResearchSession {
    let web = build_web_gatherer(
        registry,
        session_id.clone(),
        working_dir.clone(),
        event_bus.clone(),
        storage.clone(),
        config.clone(),
        provider_registry.clone(),
        active_model.clone(),
    );
    let local = build_local_gatherer(
        registry,
        session_id.clone(),
        working_dir.clone(),
        event_bus.clone(),
        storage.clone(),
        config.clone(),
    );
    let analysis: Arc<dyn AnalysisEngine> = match (provider_registry.clone(), active_model.clone())
    {
        (Some(registry), Some(model_ref)) => {
            let base_url = resolve_base_url(
                &model_ref.provider_id,
                storage.as_deref(),
                config.as_deref(),
            );
            let api_key = storage
                .as_deref()
                .and_then(|s| s.get_provider_auth(&model_ref.provider_id).ok().flatten());
            Arc::new(
                LlmAnalysisEngine::new(registry, &model_ref.provider_id, &model_ref.model_id)
                    .with_api_key(api_key)
                    .with_base_url(base_url),
            )
        }
        _ => Arc::new(NoopAnalysisEngine),
    };

    let planner: Arc<dyn Planner> = match (provider_registry.clone(), active_model.clone()) {
        (Some(reg), Some(m)) => {
            let api_key = storage
                .as_deref()
                .and_then(|s| s.get_provider_auth(&m.provider_id).ok().flatten());
            let base_url = resolve_base_url(&m.provider_id, storage.as_deref(), config.as_deref());
            Arc::new(
                LlmPlanner::new(reg, &m.provider_id, &m.model_id)
                    .with_api_key(api_key)
                    .with_base_url(base_url),
            )
        }
        _ => Arc::new(HeuristicPlanner::new()),
    };
    let critic: Arc<dyn Critic> = Arc::new(SimpleCritic);

    let model_label = active_model
        .as_ref()
        .map(|m| format!("{}/{}", m.provider_id, m.model_id));

    // Build an optional LLM summarizer used by the `--from-url` and
    // `--from-file` pre-steps to derive a concise topic + clean title from the
    // fetched/extracted body. We reuse the same provider/model wiring as the
    // analysis engine so it works for every configured provider (including
    // local Ollama); when no model is configured the session falls back to
    // the local heuristics.
    let summarizer = match (provider_registry.clone(), active_model.clone()) {
        (Some(reg), Some(m)) => {
            let api_key = storage
                .as_deref()
                .and_then(|s| s.get_provider_auth(&m.provider_id).ok().flatten());
            let base_url = resolve_base_url(&m.provider_id, storage.as_deref(), config.as_deref());
            Some(Arc::new(
                LlmAnalysisEngine::new(reg, &m.provider_id, &m.model_id)
                    .with_api_key(api_key)
                    .with_base_url(base_url),
            ))
        }
        _ => None,
    };

    let session = ResearchSession::new(manager, web, local, analysis)
        .with_planner(planner)
        .with_critic(critic);
    let session = match summarizer {
        Some(sum) => session.with_summarizer(sum),
        None => session,
    };
    let session = match research_name
        .map(|n| ragent_research::gather_log::GatherLog::new(&working_dir.join("log"), n))
    {
        Some(Ok(log)) => session.with_gather_log(log),
        Some(Err(e)) => {
            tracing::warn!(
                error = %e,
                "research: failed to create web URL gather log; continuing without it"
            );
            session
        }
        None => session,
    };
    match model_label {
        Some(label) => session.with_model(label),
        None => session,
    }
}

/// Resolve a provider-specific base URL from storage/config/env, mirroring the
/// resolution used by the TUI and benchmark runner.
fn resolve_base_url(
    provider_id: &str,
    storage: Option<&Storage>,
    config: Option<&Config>,
) -> Option<String> {
    match provider_id {
        "copilot" => storage.and_then(|s| s.get_setting("copilot_api_base").ok().flatten()),
        "generic_openai" => storage
            .and_then(|s| s.get_setting("generic_openai_api_base").ok().flatten())
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                config
                    .and_then(|c| c.provider.get("generic_openai"))
                    .and_then(|p| p.api.as_ref())
                    .and_then(|api| api.base_url.clone())
            })
            .or_else(|| {
                std::env::var("GENERIC_OPENAI_API_BASE")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            }),
        "azure_foundry" => storage
            .and_then(|s| s.get_setting("azure_foundry_api_base").ok().flatten())
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                config
                    .and_then(|c| c.provider.get("azure_foundry"))
                    .and_then(|p| p.api.as_ref())
                    .and_then(|api| api.base_url.clone())
            })
            .or_else(|| {
                std::env::var("AZURE_AI_FOUNDRY_BASE")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            }),
        _ => None,
    }
}

fn build_tool_context(
    session_id: String,
    working_dir: PathBuf,
    event_bus: Arc<EventBus>,
    storage: Option<Arc<Storage>>,
    config: Option<Arc<Config>>,
    active_model: Option<ModelRef>,
) -> AgentToolContext {
    AgentToolContext {
        session_id,
        working_dir,
        event_bus,
        storage,
        agent_manager: None,
        active_model,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config,
        cached_team_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    }
}

fn build_web_gatherer(
    registry: &Arc<ToolRegistry>,
    session_id: String,
    working_dir: PathBuf,
    event_bus: Arc<EventBus>,
    storage: Option<Arc<Storage>>,
    config: Option<Arc<Config>>,
    provider_registry: Option<Arc<ProviderRegistry>>,
    active_model: Option<ModelRef>,
) -> Option<WebGatherer> {
    let search = registry
        .get("mf_search")
        .or_else(|| registry.get("websearch"))?;
    let fetch = registry
        .get("mf_fetch")
        .or_else(|| registry.get("webfetch"))?;
    let search_tool_name = search.name().to_string();
    let ctx = build_tool_context(
        session_id,
        working_dir,
        event_bus,
        storage.clone(),
        config.clone(),
        active_model.clone(),
    );

    let decomposer: Arc<dyn QueryDecomposer> = match (provider_registry, active_model) {
        (Some(registry), Some(model_ref)) => {
            let api_key = storage
                .as_deref()
                .and_then(|s| s.get_provider_auth(&model_ref.provider_id).ok().flatten());
            let base_url = resolve_base_url(
                &model_ref.provider_id,
                storage.as_deref(),
                config.as_deref(),
            );
            Arc::new(
                LlmQueryDecomposer::new(registry, &model_ref.provider_id, &model_ref.model_id)
                    .with_api_key(api_key)
                    .with_base_url(base_url),
            )
        }
        _ => Arc::new(HeuristicQueryDecomposer),
    };

    Some(
        WebGatherer::new(
            Arc::new(AgentWebSearchTool {
                tool: search,
                ctx: ctx.clone(),
                tool_name: search_tool_name,
            }),
            Arc::new(AgentWebFetchTool {
                tool: fetch,
                ctx,
                #[cfg(test)]
                legacy_verifier: None,
            }),
        )
        .with_decomposer(decomposer),
    )
}

fn build_local_gatherer(
    registry: &Arc<ToolRegistry>,
    session_id: String,
    working_dir: PathBuf,
    event_bus: Arc<EventBus>,
    storage: Option<Arc<Storage>>,
    config: Option<Arc<Config>>,
) -> Option<LocalGatherer> {
    let glob = registry.get("glob")?;
    let grep = registry.get("grep")?;
    let read = registry.get("read")?;
    let list = registry.get("list")?;
    let ctx = build_tool_context(session_id, working_dir, event_bus, storage, config, None);
    Some(LocalGatherer::new(Arc::new(AgentLocalTool {
        glob,
        grep,
        read,
        list,
        ctx,
    })))
}

struct AgentWebSearchTool {
    tool: Arc<dyn AgentTool>,
    ctx: AgentToolContext,
    tool_name: String,
}

#[async_trait]
impl WebSearchTool for AgentWebSearchTool {
    async fn search(&self, query: &str, max_results: usize) -> Result<Vec<WebSearchHit>> {
        let input = serde_json::json!({
            "query": query,
            "max_results": max_results,
        });
        let output = self.tool.execute(input, &self.ctx).await?;

        // Prefer the structured JSON metadata emitted by the underlying
        // search tool. `mf_search` populates a `results` array with engine
        // provenance; legacy `websearch` populates a Tavily-only `results`
        // array. Fall back to parsing the human-readable text.
        if let Some(ref metadata) = output.metadata {
            let from_json: Vec<WebSearchHit> =
                ragent_tools_extended::websearch::hits_from_metadata(metadata)
                    .into_iter()
                    .map(|r| WebSearchHit {
                        title: r.title,
                        url: r.url,
                        snippet: r.snippet,
                        matched_query: String::new(),
                        search_tool: if r.search_tool.is_empty() {
                            self.tool_name.clone()
                        } else {
                            r.search_tool
                        },
                        search_engine: if r.search_engine.is_empty() {
                            self.tool_name.clone()
                        } else {
                            r.search_engine
                        },
                    })
                    .collect();
            if !from_json.is_empty() {
                return Ok(from_json);
            }

            // Try the `mf_search`-specific metadata shape if the legacy
            // `results` key was absent or empty.
            let from_mf = parse_mf_search_metadata(metadata, &self.tool_name);
            if !from_mf.is_empty() {
                return Ok(from_mf);
            }
        }

        // Legacy websearch plain-text fallback.
        let mut hits = parse_websearch_output(&output.content);
        for hit in &mut hits {
            hit.search_tool = self.tool_name.clone();
        }
        Ok(hits)
    }
}

/// Parse the structured metadata produced by the `mf_search` tool into
/// research-layer [`WebSearchHit`] rows.
fn parse_mf_search_metadata(metadata: &serde_json::Value, tool_name: &str) -> Vec<WebSearchHit> {
    metadata
        .get("results")
        .and_then(|r| serde_json::from_value::<Vec<serde_json::Value>>(r.clone()).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| {
            let title = v.get("title")?.as_str()?.to_string();
            let url = v.get("url")?.as_str()?.to_string();
            let snippet = v
                .get("snippet")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let search_engine = v
                .get("search_engine")
                .and_then(|s| s.as_str())
                .or_else(|| v.get("source").and_then(|s| s.as_str()))
                .unwrap_or(tool_name)
                .to_string();
            Some(WebSearchHit {
                title,
                url,
                snippet,
                matched_query: String::new(),
                search_tool: tool_name.to_string(),
                search_engine,
            })
        })
        .collect()
}

struct AgentWebFetchTool {
    tool: Arc<dyn AgentTool>,
    ctx: AgentToolContext,
    /// Verifier hook used to enforce the mandatory readability guarantee on
    /// the legacy `webfetch` path (which does not report which extraction
    /// stage produced its output). `None` disables verification (used in
    /// tests to exercise the bail branch deterministically).
    #[cfg(test)]
    legacy_verifier: Option<Box<dyn Fn() -> bool + Send + Sync>>,
}

/// Maximum number of bytes of raw HTML to download when extracting a
/// publication date. The date metadata lives in the page `<head>`, which is
/// always near the start of the document, so a small cap is plenty and keeps
/// the extra request cheap.
const DATE_EXTRACTION_MAX_HTML_BYTES: usize = 64 * 1024;

/// User-Agent used for the supplementary raw-HTML fetch performed to extract
/// a publication date. Mirrors the one used by the `webfetch` tool.
const DATE_EXTRACTION_USER_AGENT: &str = "ragent/0.1 (https://github.com/thawkins/ragent)";

#[async_trait]
impl WebFetchTool for AgentWebFetchTool {
    async fn fetch(&self, url: &str) -> Result<WebFetchedPage> {
        // Prefer `mf_fetch` for richer metadata (content_type, page_type, title).
        // Fall back to the legacy `webfetch` tool when `mf_fetch` is not
        // registered.
        let is_mf_fetch = self.tool.name() == "mf_fetch";
        let input = if is_mf_fetch {
            json!({
                "url": url,
                "format": "markdown",
            })
        } else {
            json!({
                "url": url,
                "format": "text",
            })
        };
        let output = self.tool.execute(input, &self.ctx).await?;

        // `mf_fetch` returns a structured envelope. If the envelope is present,
        // use its metadata and content; otherwise treat the legacy `webfetch`
        // output as the page body directly.
        // Detect YouTube transcript output before the generic envelope parser
        // moves `output.content`. YouTube `mf_fetch` results are plain text with
        // the video title on a `Title:` line followed by the transcript.
        let youtube_override = is_mf_fetch
            && output
                .metadata
                .as_ref()
                .and_then(|m| m.get("page_type"))
                .and_then(|v| v.as_str())
                == Some("youtube");
        let youtube_parsed = if youtube_override {
            parse_mf_fetch_youtube_body(&output.content)
        } else {
            None
        };

        let (mut body, mut title, content_type, page_type, language) = if is_mf_fetch {
            parse_mf_fetch_output(url, &output.content, output.metadata.as_ref())
        } else {
            let title = output
                .metadata
                .as_ref()
                .and_then(|m| m.get("title"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| {
                    output
                        .content
                        .lines()
                        .find(|l| !l.trim().is_empty())
                        .unwrap_or(url)
                        .to_string()
                });
            (
                output.content,
                title,
                None,
                output.metadata.as_ref().and_then(|m| {
                    m.get("page_type")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                }),
                None,
            )
        };

        // Apply the YouTube-specific transcript/title parse that was computed
        // before the generic envelope parser moved `output.content`.
        let mut page_type = page_type;
        if let Some((yt_title, transcript)) = youtube_parsed {
            body = transcript;
            title = yt_title;
            page_type = Some("youtube".to_string());
        }

        // Failed fetch (transcript extraction failed, HTTP/body error, …):
        // `mf_fetch` reports tool-level failures as `ToolOutput` metadata
        // (`error`, `content_ok = false`) rather than by aborting the call, so
        // the previous flow kept a placeholder body (e.g. the
        // `[YouTube transcript extraction failed: …]` bracket text) and let the
        // gatherer suppress it later with an opaque "extracted content too
        // short" message. Bail here instead: the gatherer's existing
        // `Ok(Err(e))` branch emits `FetchFailed` with the real reason and the
        // source is suppressed explicitly, never entering the research corpus.
        if is_mf_fetch && let Some(metadata) = output.metadata.as_ref() {
            let fetch_error = metadata
                .get("error")
                .and_then(|v| v.as_str())
                .filter(|e| !e.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    let content_ok = metadata
                        .get("content_ok")
                        .and_then(serde_json::Value::as_bool);
                    (content_ok == Some(false)).then(|| {
                        metadata
                            .get("next_action")
                            .and_then(|v| v.as_str())
                            .filter(|a| !a.is_empty())
                            .unwrap_or("fetch reported content_ok = false")
                            .to_string()
                    })
                });
            if let Some(error) = fetch_error {
                anyhow::bail!("mf_fetch failed for {url}: {error}");
            }
        }

        // Mandatory readability guarantee (research web-gather phase): every
        // HTML page captured as a research source must have been extracted by
        // the `readability-rs` crate. Pages where readability failed — and the
        // fetch tool silently fell back to html2text / raw tag-stripping — are
        // rejected so fallback-extracted noise never enters the research
        // corpus. PDFs and YouTube transcripts bypass readability entirely by
        // design, so they are exempt from the check.
        let media_kind = ragent_research::classify_web_source(url, content_type.as_deref());
        if media_kind == ragent_research::WebSourceKind::Page {
            let readability_used = if is_mf_fetch {
                // `mf_fetch` reports which extraction stage produced the body
                // via the `extraction_method` envelope signal.
                output
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("extraction_method"))
                    .and_then(|v| v.as_str())
                    == Some("readability")
            } else {
                // The legacy `webfetch` tool does not report which extraction
                // stage produced its output. Re-verify by re-running
                // readability on the raw HTML: a second (cheap) fetch keeps
                // the guarantee honest without trusting the tool implicitly.
                #[cfg(test)]
                if let Some(verifier) = self.legacy_verifier.as_ref() {
                    verifier()
                } else {
                    verify_readability_on_raw_html(&self.tool, &self.ctx, url).await
                }
                #[cfg(not(test))]
                verify_readability_on_raw_html(&self.tool, &self.ctx, url).await
            };
            if !readability_used {
                anyhow::bail!(
                    "readability extraction failed for {url}; \
                     page rejected because the research web-gather phase requires \
                     readability-rs-extracted content (fallback extraction is not accepted)"
                );
            }
        }

        // Opportunistically fetch the raw HTML head to extract a publication
        // date from the page's embedded metadata. This is a best-effort step:
        // any failure (network error, non-HTML content, missing date) simply
        // leaves `published_at` as `None` so the research run is never aborted
        // by a date-extraction failure.
        let published_at = extract_published_at_for_url(url).await.unwrap_or(None);

        Ok(WebFetchedPage {
            url: url.to_string(),
            title,
            body,
            published_at,
            content_type,
            page_type,
            language,
        })
    }
}

/// Re-verify readability extraction for the legacy `webfetch` tool path.
///
/// The legacy tool applies readability → html2text fallbacks internally but
/// does not report which stage produced its output. To keep the mandatory
/// readability guarantee for research sources honest, fetch the raw HTML via
/// the same tool (`format=raw`) and run `readability-rs` on it directly,
/// applying the same minimum-length threshold the fetch tools use
/// ([`MIN_READABILITY_EXTRACT_CHARS`]).
///
/// Returns `true` when readability successfully extracts article text of at
/// least the threshold length; `false` on any fetch/parse error or short
/// extraction.
async fn verify_readability_on_raw_html(
    tool: &Arc<dyn AgentTool>,
    ctx: &AgentToolContext,
    url: &str,
) -> bool {
    let raw_output = tool
        .execute(json!({"url": url, "format": "raw"}), ctx)
        .await;
    let Ok(raw_output) = raw_output else {
        tracing::warn!(
            url,
            "research: legacy webfetch raw-HTML refetch failed; rejecting page"
        );
        return false;
    };
    readability_extract_ok(&raw_output.content, url)
}

/// Minimum extracted text length for a readability extraction to count as
/// successful. Mirrors the threshold used by `webfetch` and the masterfetch
/// extractor so the research guarantee matches the tools' own acceptance
/// criteria.
pub const MIN_READABILITY_EXTRACT_CHARS: usize = 500;

/// Run the `readability-rs` extractor on `html` and return `true` when it
/// produces non-trivial article text (≥ [`MIN_READABILITY_EXTRACT_CHARS`]
/// characters). Wrapped in `catch_unwind` so a parser panic degrades to
/// `false` instead of aborting the gather task.
///
/// Exposed for the integration tests in
/// `crates/ragent-agent/tests/test_research_readability.rs`.
pub fn readability_extract_ok(html: &str, url: &str) -> bool {
    let result = std::panic::catch_unwind(|| {
        let Ok(parsed_url) = url::Url::parse(url) else {
            return false;
        };
        let mut input = std::io::Cursor::new(html.as_bytes());
        let Ok(readable) = readability::extract(
            &mut input,
            &parsed_url,
            readability::ExtractOptions::default(),
        ) else {
            return false;
        };
        readable.text.trim().chars().count() >= MIN_READABILITY_EXTRACT_CHARS
    });
    result.unwrap_or(false)
}

/// Parse the `mf_fetch` tool's output envelope.
///
/// `mf_fetch` normally returns a JSON object with `content`, `content_type`,
/// `page_type`, `content_ok`, and nested `metadata.title` /
/// `metadata.published_time`. The body is returned as Markdown/text. The legacy
/// `webfetch` tool returns plain text without this envelope.
///
/// For non-JSON outputs (cache hits, PDF pages, YouTube transcripts, and error
/// responses) the tool still prefixes the human-readable payload with a header
/// block starting with `mf_fetch: <url>`. This header is useful in the raw tool
/// response but redundant in the research output, where the URL already appears
/// in the References Index and per-finding source lists. The function strips
/// that header so the research layer only stores the actual content.
///
/// Returns the `(body, title, content_type, page_type, detected_language)`
/// tuple. The `detected_language` is read from the top-level
/// `detected_language` field (present on HTML, PDF, and YouTube responses) with
/// a fallback to the nested `metadata.detected_language` for older payloads.
#[allow(clippy::type_complexity)]
fn parse_mf_fetch_output(
    url: &str,
    content: &str,
    metadata: Option<&serde_json::Value>,
) -> (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    // If the content looks like the mf_fetch envelope, parse it.
    if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(body) = envelope
            .get("content")
            .and_then(|v| v.as_str())
            .map(String::from)
        {
            let title = envelope
                .get("metadata")
                .and_then(|m| m.get("title"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(ToString::to_string)
                .or_else(|| {
                    body.lines()
                        .find(|l| !l.trim().is_empty())
                        .map(|l| l.to_string())
                })
                .unwrap_or_else(|| url.to_string());
            let content_type = envelope
                .get("content_type")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| {
                    metadata
                        .and_then(|m| m.get("content_type"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                });
            let page_type = envelope
                .get("page_type")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| {
                    metadata
                        .and_then(|m| m.get("page_type"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                });
            // Detected human language: prefer the top-level field (set on
            // HTML, PDF, and YouTube responses), fall back to the nested
            // PageMetadata object for older payloads.
            let detected_language = envelope
                .get("detected_language")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
                .or_else(|| {
                    envelope
                        .get("metadata")
                        .and_then(|m| m.get("detected_language"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                })
                .or_else(|| {
                    metadata
                        .and_then(|m| m.get("detected_language"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                });
            return (body, title, content_type, page_type, detected_language);
        }
    }

    // Not an envelope: strip the `mf_fetch:` header block if present and use
    // the remaining text as the page body.
    let body = strip_mf_fetch_header(content);
    let title = metadata
        .and_then(|m| m.get("title"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            body.lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or(url)
                .to_string()
        });
    let content_type = metadata
        .and_then(|m| m.get("content_type"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let page_type = metadata
        .and_then(|m| m.get("page_type"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let detected_language = metadata
        .and_then(|m| m.get("detected_language"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    (
        body.to_string(),
        title,
        content_type,
        page_type,
        detected_language,
    )
}

/// Strip the leading `mf_fetch:` header block from a plain-text tool output.
///
/// The header has the form:
///
/// ```text
/// mf_fetch: <url>
/// Status: ...
/// Content type: ...
/// ...
///
/// <actual body>
/// ```
///
/// If the first non-empty line does not start with `mf_fetch:` the content is
/// returned unchanged so that unrelated plain-text responses are not damaged.
fn strip_mf_fetch_header(content: &str) -> &str {
    let first_line = content.lines().find(|l| !l.trim().is_empty());
    if first_line.is_none_or(|l| !l.starts_with("mf_fetch:")) {
        return content;
    }
    content
        .split_once("\n\n")
        .map(|(_, rest)| rest)
        .unwrap_or(content)
}

/// Parse the plain-text payload returned by `mf_fetch` for a YouTube URL.
///
/// The tool output starts with the standard `mf_fetch:` header block, followed
/// by a body whose first line is `Title: <video title>` and the rest is the
/// caption transcript. This function strips both the header block and the title
/// prefix so the research layer stores only the transcript and records the
/// actual video title.
///
/// Returns `Some((title, transcript))` when the expected format is found, or
/// `None` if the output is missing a `Title:` line.
fn parse_mf_fetch_youtube_body(content: &str) -> Option<(String, String)> {
    let body = strip_mf_fetch_header(content);
    let mut lines = body.lines();
    let title_line = lines.next()?;
    let title = title_line
        .strip_prefix("Title:")
        .map(str::trim)
        .filter(|t| !t.is_empty())?
        .to_string();
    let transcript = lines.collect::<Vec<_>>().join("\n");
    Some((title, transcript.trim_start().to_string()))
}

/// Fetch the raw HTML for `url` and attempt to extract a publication date.
///
/// Only the first [`DATE_EXTRACTION_MAX_HTML_BYTES`] bytes are read because
/// publication-date metadata lives in the document `<head>`, which appears
/// near the start of the response. Non-HTML responses and any network error
/// are mapped to `Ok(None)` so callers can treat date extraction as
/// best-effort.
async fn extract_published_at_for_url(url: &str) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Ok(None);
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent(DATE_EXTRACTION_USER_AGENT)
        .build()
        .context("failed to build HTTP client for date extraction")?;
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("date-extraction fetch failed for {url}"))?;
    if !response.status().is_success() {
        return Ok(None);
    }
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !content_type.contains("text/html") && !content_type.contains("application/xhtml") {
        return Ok(None);
    }
    // Read only the leading chunk: the <head> with the date metadata is at the
    // top of the document. `text()` reads the whole body; instead we take the
    // first bytes via a streaming read.
    let body_bytes = response
        .bytes()
        .await
        .with_context(|| format!("failed to read body for date extraction: {url}"))?;
    let head_chunk: &[u8] = if body_bytes.len() > DATE_EXTRACTION_MAX_HTML_BYTES {
        &body_bytes[..DATE_EXTRACTION_MAX_HTML_BYTES]
    } else {
        &body_bytes
    };
    let html = String::from_utf8_lossy(head_chunk);
    Ok(ragent_research::extract_published_at(&html))
}

struct AgentLocalTool {
    glob: Arc<dyn AgentTool>,
    grep: Arc<dyn AgentTool>,
    read: Arc<dyn AgentTool>,
    list: Arc<dyn AgentTool>,
    ctx: AgentToolContext,
}

#[async_trait]
impl LocalTool for AgentLocalTool {
    async fn glob(&self, project_root: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
        let input = json!({
            "pattern": pattern,
            "path": project_root.display().to_string(),
        });
        let output = self.glob.execute(input, &self.ctx).await?;
        Ok(output
            .content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(PathBuf::from)
            .collect())
    }

    async fn grep(&self, path: &Path, terms: &[String]) -> Result<Vec<GrepMatch>> {
        let pattern = terms.join("|");
        let input = json!({
            "pattern": pattern,
            "path": path.display().to_string(),
            "case_insensitive": true,
        });
        let output = self.grep.execute(input, &self.ctx).await?;
        Ok(parse_grep_output(&output.content))
    }

    async fn read(&self, path: &Path) -> Result<String> {
        let input = json!({
            "path": path.display().to_string(),
        });
        let output = self.read.execute(input, &self.ctx).await?;
        Ok(output.content)
    }

    async fn list_specs(&self, project_root: &Path) -> Result<Vec<String>> {
        let specs_dir = project_root.join("specs");
        let input = json!({
            "path": specs_dir.display().to_string(),
            "depth": 2,
        });
        let output = self.list.execute(input, &self.ctx).await?;
        Ok(parse_specs_list(&output.content))
    }

    async fn spec_title(&self, project_root: &Path, spec_id: &str) -> Result<String> {
        let path = project_root.join("specs").join(spec_id).join("SPEC.md");
        let input = json!({
            "path": path.display().to_string(),
            "num_lines": 30,
        });
        let output = self.read.execute(input, &self.ctx).await?;
        Ok(output
            .content
            .lines()
            .map(|l| l.trim())
            .find(|l| l.starts_with("# "))
            .unwrap_or("")
            .trim_start_matches("# ")
            .to_string())
    }
}

/// Parse the human-readable text emitted by the agent `websearch` tool into
/// structured [`WebSearchHit`] rows for the research gatherer.
pub fn parse_websearch_output(content: &str) -> Vec<WebSearchHit> {
    let mut hits = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_url: Option<String> = None;
    let mut current_snippet = String::new();

    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some((num_part, rest)) = trimmed.split_once('.') {
            if num_part.parse::<usize>().is_ok() && !rest.is_empty() && rest.starts_with(' ') {
                if let (Some(title), Some(url)) = (current_title.take(), current_url.take()) {
                    hits.push(WebSearchHit {
                        title,
                        url,
                        snippet: current_snippet.trim().to_string(),
                        matched_query: String::new(),
                        search_tool: "websearch".to_string(),
                        search_engine: "tavily".to_string(),
                    });
                }
                current_snippet.clear();
                current_title = Some(rest.trim_start().to_string());
                continue;
            }
        }

        let t = line.trim();
        if t.starts_with("http://") || t.starts_with("https://") {
            current_url = Some(t.to_string());
        } else if current_title.is_some() && current_url.is_some() && !t.is_empty() {
            if !current_snippet.is_empty() {
                current_snippet.push(' ');
            }
            current_snippet.push_str(t);
        }
    }

    if let (Some(title), Some(url)) = (current_title.take(), current_url.take()) {
        hits.push(WebSearchHit {
            title,
            url,
            snippet: current_snippet.trim().to_string(),
            matched_query: String::new(),
            search_tool: "websearch".to_string(),
            search_engine: "tavily".to_string(),
        });
    }

    hits
}

fn parse_grep_output(content: &str) -> Vec<GrepMatch> {
    let mut matches = Vec::new();
    for line in content.lines() {
        // The grep tool formats matches as "relative/path:line_number:line_content".
        let Some((path_part, rest)) = line.split_once(':') else {
            continue;
        };
        let Some((num_part, text)) = rest.split_once(':') else {
            continue;
        };
        let Ok(line_num) = num_part.parse::<usize>() else {
            continue;
        };
        if line_num == 0 {
            continue;
        }
        // Skip the summary line that has no path (e.g. "5 matches in 12 files searched").
        if path_part.contains(' ') && !path_part.contains('/') && !path_part.contains('\\') {
            continue;
        }
        matches.push(GrepMatch {
            line: line_num,
            text: text.to_string(),
        });
    }
    matches
}

fn parse_specs_list(content: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for line in content.lines().skip(1) {
        let connector = if line.contains("├── ") {
            "├── "
        } else if line.contains("└── ") {
            "└── "
        } else {
            continue;
        };
        let Some(idx) = line.find(connector) else {
            continue;
        };
        let rest = &line[idx + connector.len()..];
        let name = rest.trim_end_matches('/').trim();
        if !name.is_empty() && !name.contains(' ') {
            ids.push(name.to_string());
        }
    }
    ids
}

#[cfg(test)]
mod tests {
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
                    team_context: None,
                    team_manager: None,
                    code_index: None,
                    bg_service: None,
                    spec_manager: None,
                    active_spec_id: None,
                    config: None,
                    read_timestamps: Arc::new(std::sync::RwLock::new(
                        std::collections::HashMap::new(),
                    )),
                    cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
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
        assert_eq!(page.body, "Transcript line one\nTranscript line two");
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
        // `error` string — the adapter falls back to `next_action` as the
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
        // be rejected with the mf_fetch failure reason — not the readability
        // message — because the page never produced real content at all.
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

    // ── Mandatory readability enforcement tests ────────────────────────────

    /// Build a minimal `AgentToolContext` for fetch-adapter tests.
    fn test_tool_context() -> AgentToolContext {
        AgentToolContext {
            session_id: "test".to_string(),
            working_dir: std::env::current_dir().expect("current_dir"),
            event_bus: Arc::new(crate::event::EventBus::new(8)),
            storage: None,
            agent_manager: None,
            active_model: None,
            team_context: None,
            team_manager: None,
            code_index: None,
            bg_service: None,
            spec_manager: None,
            active_spec_id: None,
            config: None,
            read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
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
        let html = "<html><head><title>Nav</title></head><body><nav><a href='/'>home</a></nav></body></html>";
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
}

#[test]
fn test_parse_mf_search_metadata_extracts_hits_and_engine_provenance() {
    let metadata = serde_json::json!({
        "query": "rust lifetimes",
        "results": [
            {
                "title": "Rust Lifetimes",
                "url": "https://doc.rust-lang.org/nomicon/lifetimes.html",
                "snippet": "A deep dive into lifetimes.",
                "source": "duckduckgo, brave",
                "search_engine": "duckduckgo, brave",
                "position": 1,
                "relevance_score": 0.95,
                "fetch_relevance": "high",
                "engines_consensus": 2
            }
        ]
    });
    let hits = parse_mf_search_metadata(&metadata, "mf_search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "Rust Lifetimes");
    assert_eq!(
        hits[0].url,
        "https://doc.rust-lang.org/nomicon/lifetimes.html"
    );
    assert_eq!(hits[0].snippet, "A deep dive into lifetimes.");
    assert_eq!(hits[0].search_tool, "mf_search");
    assert_eq!(hits[0].search_engine, "duckduckgo, brave");
}

#[test]
fn test_parse_mf_search_metadata_falls_back_to_source_field() {
    let metadata = serde_json::json!({
        "results": [
            {
                "title": "No engine field",
                "url": "https://example.com",
                "snippet": "fallback",
                "source": "brave"
            }
        ]
    });
    let hits = parse_mf_search_metadata(&metadata, "mf_search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].search_engine, "brave");
}

#[test]
fn test_parse_mf_search_metadata_returns_empty_on_missing_results() {
    let hits = parse_mf_search_metadata(&serde_json::json!({"query": "x"}), "mf_search");
    assert!(hits.is_empty());
}

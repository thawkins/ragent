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

use anyhow::Result;
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
    AnalysisEngine, GrepMatch, HeuristicQueryDecomposer, LlmAnalysisEngine, LlmQueryDecomposer,
    LocalGatherer, LocalTool, NoopAnalysisEngine, QueryDecomposer, ResearchManager,
    ResearchSession, WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool,
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
#[must_use]
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
    let analysis: Arc<dyn AnalysisEngine> = match (provider_registry, active_model) {
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
    ResearchSession::new(manager, web, local, analysis)
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
        task_manager: None,
        active_model,
        team_context: None,
        team_manager: None,
        code_index: None,
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
    let search = registry.get("websearch")?;
    let fetch = registry.get("webfetch")?;
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
            }),
            Arc::new(AgentWebFetchTool { tool: fetch, ctx }),
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
}

#[async_trait]
impl WebSearchTool for AgentWebSearchTool {
    async fn search(&self, query: &str, max_results: usize) -> Result<Vec<WebSearchHit>> {
        let input = json!({
            "query": query,
            "num_results": max_results,
        });
        let output = self.tool.execute(input, &self.ctx).await?;

        // Prefer the structured JSON metadata emitted by the websearch tool,
        // which carries the raw Tavily results without loss. Fall back to
        // parsing the human-readable text so older tool versions still work.
        if let Some(ref metadata) = output.metadata {
            let from_json: Vec<WebSearchHit> =
                ragent_tools_extended::websearch::hits_from_metadata(metadata)
                    .into_iter()
                    .map(|r| WebSearchHit {
                        title: r.title,
                        url: r.url,
                        snippet: r.snippet,
                    })
                    .collect();
            if !from_json.is_empty() {
                return Ok(from_json);
            }
        }

        Ok(parse_websearch_output(&output.content))
    }
}

struct AgentWebFetchTool {
    tool: Arc<dyn AgentTool>,
    ctx: AgentToolContext,
}

#[async_trait]
impl WebFetchTool for AgentWebFetchTool {
    async fn fetch(&self, url: &str) -> Result<WebFetchedPage> {
        let input = json!({
            "url": url,
            "format": "text",
        });
        let output = self.tool.execute(input, &self.ctx).await?;
        let title = output
            .content
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or(url)
            .to_string();
        Ok(WebFetchedPage {
            url: url.to_string(),
            title,
            body: output.content,
        })
    }
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
    fn test_build_research_session_wires_available_tools() {
        use crate::event::EventBus;
        use crate::tool::create_default_registry;
        let registry = Arc::new(create_default_registry());
        let manager = ResearchManager::new("research");
        let session = build_research_session(
            &registry,
            manager,
            "test-session".into(),
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            Arc::new(EventBus::new(256)),
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

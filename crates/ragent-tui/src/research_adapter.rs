//! Adapters that wire the TUI's agent tool registry into the research system's
//! web/local gatherers.
//!
//! The research crate defines its own small traits ([`ragent_research::WebSearchTool`],
//! [`ragent_research::WebFetchTool`], [`ragent_research::LocalTool`]).  The TUI has
//! access to the full agent tool registry (`glob`, `grep`, `read`, `list`,
//! `websearch`, `webfetch`), so this module provides thin wrappers that implement
//! the research traits by calling those agent tools.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;

use ragent_core::{
    Config, event::EventBus, storage::Storage, tool::Tool as AgentTool,
    tool::ToolContext as AgentToolContext,
};
use ragent_research::{
    GrepMatch, LocalGatherer, LocalTool, ResearchManager, ResearchSession, WebFetchTool,
    WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool,
};

/// Build a [`ResearchSession`] backed by the agent tool `registry`.
///
/// If `websearch`/`webfetch` are present, web gathering is enabled.  If
/// `glob`/`grep`/`read`/`list` are present, local gathering is enabled.
/// Missing tools are silently omitted so the session degrades gracefully.
#[must_use]
pub fn build_research_session(
    registry: &Arc<ragent_core::tool::ToolRegistry>,
    manager: ResearchManager,
    session_id: String,
    working_dir: PathBuf,
    event_bus: Arc<EventBus>,
    storage: Option<Arc<Storage>>,
    config: Option<Arc<Config>>,
) -> ResearchSession {
    let web = build_web_gatherer(
        registry,
        session_id.clone(),
        working_dir.clone(),
        event_bus.clone(),
        storage.clone(),
        config.clone(),
    );
    let local = build_local_gatherer(
        registry,
        session_id.clone(),
        working_dir.clone(),
        event_bus.clone(),
        storage.clone(),
        config.clone(),
    );
    ResearchSession::new(manager, web, local)
}

fn build_tool_context(
    session_id: String,
    working_dir: PathBuf,
    event_bus: Arc<EventBus>,
    storage: Option<Arc<Storage>>,
    config: Option<Arc<Config>>,
) -> AgentToolContext {
    AgentToolContext {
        session_id,
        working_dir,
        event_bus,
        storage,
        task_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        spec_manager: None,
        active_spec_id: None,
        config,
        cached_team_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
    }
}

fn build_web_gatherer(
    registry: &Arc<ragent_core::tool::ToolRegistry>,
    session_id: String,
    working_dir: PathBuf,
    event_bus: Arc<EventBus>,
    storage: Option<Arc<Storage>>,
    config: Option<Arc<Config>>,
) -> Option<WebGatherer> {
    let search = registry.get("websearch")?;
    let fetch = registry.get("webfetch")?;
    let ctx = build_tool_context(session_id, working_dir, event_bus, storage, config);
    Some(WebGatherer::new(
        Arc::new(AgentWebSearchTool {
            tool: search,
            ctx: ctx.clone(),
        }),
        Arc::new(AgentWebFetchTool { tool: fetch, ctx }),
    ))
}

fn build_local_gatherer(
    registry: &Arc<ragent_core::tool::ToolRegistry>,
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
    let ctx = build_tool_context(session_id, working_dir, event_bus, storage, config);
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

fn parse_websearch_output(content: &str) -> Vec<WebSearchHit> {
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
        let text = "/project/specs/\n├── auth-refactor/\n├── model-router/\n└── researchsystem/\n";
        let ids = parse_specs_list(text);
        assert_eq!(ids, vec!["auth-refactor", "model-router", "researchsystem"]);
    }

    #[test]
    fn test_build_research_session_wires_available_tools() {
        use ragent_core::{event::EventBus, tool::create_default_registry};
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

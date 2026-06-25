//! Source analysis engine — turns gathered evidence into a structured
//! `AnalysisResult` using an LLM.
//!
//! The default [`LlmAnalysisEngine`] sends a single synthesis prompt to the
//! configured provider/model. The prompt asks for four sections that map
//! directly to the `RESEARCH.md` structure:
//!
//! - Summary
//! - Findings
//! - In-Project Cross-References
//! - Open Questions
//!
//! A [`NoopAnalysisEngine`] is provided so callers can disable synthesis or use
//! the legacy mechanical fallback.

use crate::document::CrossReference;
use crate::source::Source;
use futures::StreamExt;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};
use ragent_llm::provider::ProviderRegistry;
use std::collections::HashMap;
use std::sync::Arc;

/// One captured source plus its body text, ready to be fed into the synthesis
/// prompt. Web bodies are the fetched page text; local bodies are excerpts;
/// spec bodies are the spec title.
#[derive(Debug, Clone)]
pub struct SourceBody {
    /// Reference number matching the position in the source list (1-based).
    pub index: usize,
    /// Type string: `web`, `local`, `spec`, `other`.
    pub kind: String,
    /// Title or label for the source.
    pub title: String,
    /// URL or project-relative path.
    pub path_or_url: String,
    /// Relevance note (for local/spec sources).
    pub relevance: String,
    /// Body text of the source, already truncated/fenced by the gatherers.
    pub body: String,
}

/// Structured result returned by an analysis engine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnalysisResult {
    /// One-paragraph synthesis of the gathered evidence.
    pub summary: String,
    /// Numbered findings. Each entry is the markdown body of one finding.
    pub findings: Vec<String>,
    /// In-project files that are relevant, with one-line notes.
    pub cross_references: Vec<CrossReference>,
    /// Bulleted open questions for further investigation.
    pub open_questions: Vec<String>,
}

/// Abstraction over analysis implementations.
#[async_trait::async_trait]
pub trait AnalysisEngine: Send + Sync {
    /// Analyze the provided sources and topic, returning structured content.
    async fn analyze(&self, topic: &str, sources: &[SourceBody]) -> anyhow::Result<AnalysisResult>;

    /// Marker used by [`crate::session::ResearchSession`] to distinguish the
    /// no-op engine from real LLM engines without resorting to fragile
    /// `Any` downcasting tricks. Defaults to `false`; only
    /// [`NoopAnalysisEngine`] overrides it to `true`.
    fn is_noop_marker(&self) -> bool {
        false
    }
}

/// Analysis engine that returns empty/default content, preserving the legacy
/// mechanical summary/finding behavior.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopAnalysisEngine;

#[async_trait::async_trait]
impl AnalysisEngine for NoopAnalysisEngine {
    async fn analyze(
        &self,
        _topic: &str,
        _sources: &[SourceBody],
    ) -> anyhow::Result<AnalysisResult> {
        Ok(AnalysisResult::default())
    }

    fn is_noop_marker(&self) -> bool {
        true
    }
}

/// LLM-backed analysis engine.
#[derive(Clone)]
pub struct LlmAnalysisEngine {
    provider_registry: Arc<ProviderRegistry>,
    api_key: Option<String>,
    provider_id: String,
    model_id: String,
    base_url: Option<String>,
}

impl std::fmt::Debug for LlmAnalysisEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmAnalysisEngine")
            .field("provider_id", &self.provider_id)
            .field("model_id", &self.model_id)
            .field("base_url", &self.base_url)
            .field("has_api_key", &self.api_key.is_some())
            .finish_non_exhaustive()
    }
}

impl LlmAnalysisEngine {
    /// Build a new engine. If the provider/model is unknown, creation succeeds
    /// but [`analyze`] will return an error when called.
    pub fn new(
        provider_registry: Arc<ProviderRegistry>,
        provider_id: impl Into<String>,
        model_id: impl Into<String>,
    ) -> Self {
        Self {
            provider_registry,
            api_key: None,
            provider_id: provider_id.into(),
            model_id: model_id.into(),
            base_url: None,
        }
    }

    /// Provide an API key for the provider.
    pub fn with_api_key(mut self, api_key: Option<String>) -> Self {
        self.api_key = api_key;
        self
    }

    /// Override the API base URL. If unset, the engine resolves it from storage
    /// / config / env at analysis time.
    pub fn with_base_url(mut self, base_url: Option<String>) -> Self {
        self.base_url = base_url;
        self
    }
}

#[async_trait::async_trait]
impl AnalysisEngine for LlmAnalysisEngine {
    async fn analyze(&self, topic: &str, sources: &[SourceBody]) -> anyhow::Result<AnalysisResult> {
        let provider = self
            .provider_registry
            .get(&self.provider_id)
            .ok_or_else(|| anyhow::anyhow!("unknown provider '{}'", self.provider_id))?;

        let api_key = self.api_key.clone().unwrap_or_default();
        let client = provider
            .create_client(&api_key, self.base_url.as_deref(), &HashMap::new())
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "failed to create LLM client for {}/{}: {e}",
                    self.provider_id,
                    self.model_id
                )
            })?;

        let prompt = build_synthesis_prompt(topic, sources);
        let request = ChatRequest {
            model: self.model_id.clone(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text(prompt),
            }]),
            tools: Arc::new(vec![]),
            temperature: Some(0.2),
            top_p: Some(1.0),
            max_tokens: Some(8192),
            system: Some(std::sync::Arc::from(
                "You are a careful research analyst. Read the provided sources and produce a structured markdown analysis. Use only the evidence in the sources; do not invent facts.",
            )),
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: Some(300),
            thinking: None,
        };

        let mut stream = client.chat(request).await?;
        let mut text = String::new();
        while let Some(event) = stream.next().await {
            match event {
                StreamEvent::TextDelta { text: delta } => text.push_str(&delta),
                StreamEvent::Error { message } => anyhow::bail!("provider error: {message}"),
                StreamEvent::Finish { .. } => break,
                _ => {}
            }
        }

        Ok(parse_analysis_response(&text))
    }
}

/// Build the synthesis prompt. Sources are listed with their index so the model
/// can cite them as `[#N]`.
fn build_synthesis_prompt(topic: &str, sources: &[SourceBody]) -> String {
    let mut prompt = format!(
        "You are writing the analysis section of a research report for the topic:\n\n{topic}\n\n"
    );
    if sources.is_empty() {
        prompt.push_str("No sources were captured. Write a brief note that no sources were available and suggest refining the topic.\n");
    } else {
        prompt.push_str(&format!(
            "{count} source(s) were captured. Read them and produce a structured markdown response with exactly these four top-level sections (in this order):\n\n"
        , count = sources.len()));
        prompt.push_str("## Summary\n");
        prompt.push_str(
            "A concise one-paragraph summary of what the sources collectively say about the topic.\n\n"
        );
        prompt.push_str("## Findings\n");
        prompt.push_str(
            "A numbered list of concrete findings. Each finding should be 1–3 sentences and cite sources using `[#N]` markers where N is the source index."
        );
        prompt.push_str(" Put each finding on its own line starting with `1. `, `2. `, etc.\n\n");
        prompt.push_str("## In-Project Cross-References\n");
        prompt.push_str(
            "A bullet list of relevant in-project files, formatted as `* `path` — note`. Only include files that are actually mentioned in the local sources.\n\n"
        );
        prompt.push_str("## Open Questions\n");
        prompt.push_str(
            "A bullet list of gaps, uncertainties, or follow-up questions that remain after reading the sources.\n\n"
        );
        prompt.push_str("---\n\n### Sources\n\n");
        for src in sources {
            prompt.push_str(&format!(
                "#### Source [#{index}] ({kind}) {title}\nPath/URL: {path}\nRelevance: {rel}\n```text\n{body}\n```\n\n",
                index = src.index,
                kind = src.kind,
                title = src.title,
                path = src.path_or_url,
                rel = if src.relevance.is_empty() { "—".to_string() } else { src.relevance.clone() },
                body = truncate_body(&src.body, 4000),
            ));
        }
    }
    prompt.push_str(
        "\nNow produce only the four sections above. Do not include a title or any other preamble.",
    );
    prompt
}

/// Parse the LLM response into an [`AnalysisResult`]. We look for the four
/// expected section headings and extract content underneath.
fn parse_analysis_response(text: &str) -> AnalysisResult {
    let mut result = AnalysisResult::default();
    let sections = split_sections(text);
    for (title, body) in sections {
        match title.to_lowercase().as_str() {
            "summary" => result.summary = body.trim().to_string(),
            "findings" => {
                result.findings = parse_numbered_list(&body);
            }
            "in-project cross-references" | "cross-references" | "cross references" => {
                result.cross_references = parse_cross_reference_list(&body);
            }
            "open questions" => {
                result.open_questions = parse_bullet_list(&body);
            }
            _ => {}
        }
    }
    result
}

/// Split a markdown response into (heading, body) pairs based on `## ` H2
/// headings.
fn split_sections(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut current_title = String::new();
    let mut current_body = String::new();
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            if !current_title.is_empty() {
                out.push((current_title.clone(), current_body.clone()));
            }
            current_title = title.trim().to_string();
            current_body.clear();
        } else {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }
    if !current_title.is_empty() {
        out.push((current_title, current_body));
    }
    out
}

/// Parse a numbered markdown list (`1. ...`) into plain item strings.
fn parse_numbered_list(body: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some((num_part, rest)) = trimmed.split_once(". ") {
            if num_part.parse::<usize>().is_ok() && !rest.is_empty() {
                if !current.is_empty() {
                    items.push(current.trim().to_string());
                }
                current = rest.to_string();
                continue;
            }
        }
        if !trimmed.is_empty() {
            current.push('\n');
            current.push_str(trimmed);
        }
    }
    if !current.is_empty() {
        items.push(current.trim().to_string());
    }
    items
}

/// Parse a bullet list (`* ...` or `- ...`) into plain item strings.
fn parse_bullet_list(body: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("* ") || trimmed.starts_with("- ") {
            if !current.is_empty() {
                items.push(current.trim().to_string());
            }
            current = trimmed[2..].trim().to_string();
        } else if !trimmed.is_empty() {
            current.push('\n');
            current.push_str(trimmed);
        }
    }
    if !current.is_empty() {
        items.push(current.trim().to_string());
    }
    items
}

/// Parse cross-reference bullets into [`CrossReference`] structs. Expected
/// format: `* `path` — note` or `* path — note`.
fn parse_cross_reference_list(body: &str) -> Vec<CrossReference> {
    let mut out = Vec::new();
    for item in parse_bullet_list(body) {
        let (path, relevance) = if let Some(idx) = item.find(" — ") {
            let split_at = idx + " — ".len();
            (
                item[..idx].trim().to_string(),
                item[split_at..].trim().to_string(),
            )
        } else {
            (item.clone(), String::new())
        };
        let path = path.trim_matches('`').to_string();
        out.push(CrossReference { path, relevance });
    }
    out
}

/// Truncate a source body to a character budget so the prompt fits in common
/// context windows. The limit is approximate and errs on the side of inclusion.
fn truncate_body(body: &str, max_chars: usize) -> String {
    if body.chars().count() <= max_chars {
        return body.to_string();
    }
    let mut out = String::with_capacity(max_chars);
    let mut count = 0;
    for ch in body.chars() {
        if count >= max_chars {
            out.push_str("\n\n… (truncated for prompt size)");
            break;
        }
        out.push(ch);
        count += 1;
    }
    out
}

/// Resolve a provider-specific base URL from the environment. This is the
/// minimal set needed for research synthesis; it mirrors the values used by
/// the benchmark runner and the TUI.
#[allow(dead_code)]
fn resolve_base_url(provider_id: &str) -> Option<String> {
    match provider_id {
        "generic_openai" => std::env::var("GENERIC_OPENAI_API_BASE")
            .ok()
            .filter(|s| !s.trim().is_empty()),
        "azure_foundry" => std::env::var("AZURE_AI_FOUNDRY_BASE")
            .ok()
            .filter(|s| !s.trim().is_empty()),
        _ => None,
    }
}

/// Build [`SourceBody`] values from the gathered [`Source`] list and a function
/// that can read each source's captured body text.
pub fn build_source_bodies<S: AsRef<str>>(
    sources: &[Source],
    mut read_body: impl FnMut(&Source) -> Option<S>,
) -> Vec<SourceBody> {
    sources
        .iter()
        .enumerate()
        .map(|(idx, src)| SourceBody {
            index: idx + 1,
            kind: src.type_str().to_string(),
            title: src.title().to_string(),
            path_or_url: src.path_or_url().to_string(),
            relevance: src.relevance().unwrap_or("").to_string(),
            body: read_body(src)
                .map(|s| s.as_ref().to_string())
                .unwrap_or_default(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_analysis_response_extracts_all_sections() {
        let text = "## Summary\n\nThis is the summary.\n\n## Findings\n\n1. First finding.\n2. Second finding.\n\n## In-Project Cross-References\n\n* `src/lib.rs` — main entry\n* `src/foo.rs` — helper\n\n## Open Questions\n\n* What about X?\n* How does Y work?\n";
        let result = parse_analysis_response(text);
        assert_eq!(result.summary, "This is the summary.");
        assert_eq!(result.findings, vec!["First finding.", "Second finding."]);
        assert_eq!(result.cross_references.len(), 2);
        assert_eq!(result.cross_references[0].path, "src/lib.rs");
        assert_eq!(result.cross_references[0].relevance, "main entry");
        assert_eq!(
            result.open_questions,
            vec!["What about X?", "How does Y work?"]
        );
    }

    #[test]
    fn parse_numbered_list_ignores_wrapped_lines() {
        let body = "1. First\n   continuation\n2. Second\n";
        assert_eq!(
            parse_numbered_list(body),
            vec!["First\ncontinuation", "Second"]
        );
    }

    #[test]
    fn parse_bullet_list_handles_dash_and_star() {
        let body = "* one\n- two\n* three\n";
        assert_eq!(parse_bullet_list(body), vec!["one", "two", "three"]);
    }

    #[test]
    fn truncate_body_adds_ellipsis_when_cut() {
        let body = "a".repeat(5000);
        let truncated = truncate_body(&body, 4000);
        assert!(truncated.len() < 5000);
        assert!(truncated.contains("… (truncated for prompt size)"));
    }
}

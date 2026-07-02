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
use regex::Regex;
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
                      "A numbered list of concrete findings. Aim for around 20 distinct findings when the sources have enough breadth and depth to support that many; for narrower topics, include every worthwhile point rather than padding. Each finding must contain at least \
                      **four markdown paragraphs** with these bold labels, in this order:\n\n\
                      **Observation:** State the concrete evidence or fact observed in the sources, including at least one `[#N]` citation. You may cite multiple sources in a finding if several support the same point.\n\n\
                      **Analysis:** Explain why the observation matters for the topic and how it connects to the broader research question.\n\n\
                      **Cross-reference / Dependencies:** Name any other finding(s) this one builds on, contradicts, or is prerequisite to, using `Finding N` references. If there are no dependencies, write \"No direct dependencies.\"\n\n\
                      **Implication:** Summarize the practical consequence, open risk, or recommended follow-up action.\n\n\
                      Put each label on its own line, and separate every paragraph with a blank line. \
                      You may add additional paragraphs after the four required ones (for example, \
                      extra evidence, related work, caveats, or implementation notes). Each additional \
                      paragraph must also begin with a bold label such as **Label:** so it is easy to \
                      parse. Put each finding on its own line starting with `1. `, `2. `, etc.\n\n"
                  );
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
            prompt.push_str(
                          &format!(
                              "#### Source [#{index}] ({kind}) {title}\nPath/URL: {path}\nRelevance: {rel}\n```text\n{body}\n```\n\n",
                              index = src.index,
                              kind = src.kind,
                              title = src.title,
                              path = src.path_or_url,
                              rel = if src.relevance.is_empty() {
                                  "—".to_string()
                              } else {
                                  src.relevance.clone()
                              },
                              body = truncate_body(&src.body, 4000),
                          ),
                      );
        }
    }
    prompt.push_str(
              "\nNow produce only the four sections above. Do not include a title or any other preamble. ");
    prompt.push_str(
        "Within Findings, always include the four required paragraphs (Observation, Analysis, ",
    );
    prompt.push_str(
              "Cross-reference / Dependencies, Implication) and feel free to add more labeled paragraphs if the sources support it.");
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
                let raw = parse_numbered_list(&body);
                result.findings = reorder_findings_by_dependency(&raw);
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
///
/// Handles the common LLM output patterns:
/// * `1. First finding.` — number, dot, space, content on the same line
/// * `1.` followed by blank line and paragraphs — number on its own line,
///   content starts on subsequent lines
fn parse_numbered_list(body: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    for line in body.lines() {
        let trimmed = line.trim();
        let mut is_item = false;
        let mut rest = "";
        if let Some((num_part, after_dot)) = trimmed.split_once(". ") {
            if num_part.parse::<usize>().is_ok() {
                is_item = true;
                rest = after_dot;
            }
        } else if let Some(num_part) = trimmed.strip_suffix('.')
            && !num_part.is_empty()
            && num_part.parse::<usize>().is_ok()
        {
            is_item = true;
            rest = "";
        }
        if is_item {
            if !current.is_empty() {
                items.push(current.trim().to_string());
            }
            current = rest.to_string();
            continue;
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

/// Reorder findings so any finding that depends on another appears after its
/// dependency, then renumber all internal `Finding N` references consistently.
///
/// The parser receives the raw numbered list in the order the LLM produced it.
/// Often the model lists a child finding before its prerequisite, which makes
/// the final document harder to read. This helper builds a directed graph from
/// the **Cross-reference / Dependencies** paragraph of each finding, topologically
/// sorts it, and rewrites dependency references so they point to the new
/// positions.
///
/// Cycles (e.g. Finding 2 depends on Finding 3 and Finding 3 depends on
/// Finding 2) are broken by falling back to the original order for the involved
/// items.
fn reorder_findings_by_dependency(findings: &[String]) -> Vec<String> {
    if findings.len() <= 1 {
        return findings.to_vec();
    }

    // Build an adjacency list: edge i -> j means finding i depends on finding j,
    // so j must come before i.
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); findings.len()];
    let finding_re = Regex::new(r"(?i)\bfinding\s+(\d+)\b").expect("valid regex");
    for (idx, finding) in findings.iter().enumerate() {
        for cap in finding_re.captures_iter(finding) {
            let dep_num: usize = cap[1].parse().unwrap_or(0);
            if dep_num == 0 || dep_num > findings.len() {
                continue;
            }
            let dep_idx = dep_num - 1;
            if dep_idx != idx && !adj[idx].contains(&dep_idx) {
                adj[idx].push(dep_idx);
            }
        }
    }

    // Kahn's algorithm. `in_degree[i]` is the number of dependencies finding i
    // has (the count of edges leaving node i toward its prerequisites).
    let mut in_degree: Vec<usize> = adj.iter().map(|deps| deps.len()).collect();

    // Roots (no dependencies) keep their original relative order via a FIFO
    // queue. Each queue item is placed before its dependants are released.
    let mut queue: Vec<usize> = (0..findings.len()).filter(|&i| in_degree[i] == 0).collect();
    let mut order = Vec::with_capacity(findings.len());
    let mut processed = vec![false; findings.len()];
    let mut front = 0usize;

    // We built edges as dependant -> dependency. To apply Kahn's we need the
    // reverse graph: dependency -> dependant, so we can decrement in-degrees of
    // dependants once a dependency is placed.
    let mut reverse: Vec<Vec<usize>> = vec![Vec::new(); findings.len()];
    for (idx, deps) in adj.iter().enumerate() {
        for &d in deps {
            reverse[d].push(idx);
        }
    }

    while front < queue.len() {
        let node = queue[front];
        front += 1;
        if processed[node] {
            continue;
        }
        processed[node] = true;
        order.push(node);
        for &dependant in &reverse[node] {
            if processed[dependant] {
                continue;
            }
            in_degree[dependant] -= 1;
            if in_degree[dependant] == 0 {
                queue.push(dependant);
            }
        }
    }

    // If we couldn't place everything, there is a cycle. Append the remaining
    // nodes in original order so we still emit all findings.
    for (i, was_processed) in processed.iter().enumerate() {
        if !was_processed {
            order.push(i);
        }
    }

    // Remap old numbers (1-based, index+1) to new numbers.
    let mut old_to_new = vec![0usize; findings.len()];
    for (new_pos, &old_idx) in order.iter().enumerate() {
        old_to_new[old_idx] = new_pos + 1;
    }

    // Rewrite each finding's Finding N references.
    order
        .into_iter()
        .map(|old_idx| {
            let text = &findings[old_idx];
            let mut out = String::with_capacity(text.len());
            let mut last_end = 0;
            for cap in finding_re.captures_iter(text) {
                let m = cap.get(0).expect("full match");
                out.push_str(&text[last_end..m.start()]);
                let old_num: usize = cap[1].parse().unwrap_or(0);
                if old_num > 0 && old_num <= findings.len() {
                    out.push_str(&format!("Finding {}", old_to_new[old_num - 1]));
                } else {
                    out.push_str(m.as_str());
                }
                last_end = m.end();
            }
            out.push_str(&text[last_end..]);
            out
        })
        .collect()
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
    for (count, ch) in body.chars().enumerate() {
        if count >= max_chars {
            out.push_str("\n\n… (truncated for prompt size)");
            break;
        }
        out.push(ch);
    }
    out
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
    fn reorder_puts_dependencies_first_and_renumbers_references() {
        // Element 0 is the child, element 1 is the root.
        let findings = vec![
                        "**Observation:** child. **Analysis:** a. **Cross-reference / Dependencies:** Builds on Finding 2. **Implication:** i.".into(),
                        "**Observation:** root. **Analysis:** b. **Cross-reference / Dependencies:** No direct dependencies. **Implication:** j.".into(),
                    ];
        let ordered = reorder_findings_by_dependency(&findings);
        assert_eq!(ordered.len(), 2);
        // Root must come before its dependant.
        assert!(
            ordered[0].contains("No direct dependencies."),
            "first finding should be the root, got: {}",
            ordered[0]
        );
        assert!(
            ordered[1].contains("Finding 1"),
            "dependant should reference the renumbered root, got: {}",
            ordered[1]
        );
        assert!(
            !ordered[1].contains("Finding 2"),
            "dependant must not retain the old root number"
        );
    }
    #[test]
    fn reorder_preserves_original_order_for_unrelated_findings() {
        let findings = vec![
            "A — no deps".into(),
            "B — no deps".into(),
            "C — no deps".into(),
        ];
        let ordered = reorder_findings_by_dependency(&findings);
        assert_eq!(ordered, findings);
    }

    #[test]
    fn reorder_handles_chains_and_multiple_dependencies() {
        // Original order: leaf (depends on old 2 and 3), mid (depends on old 3), root.
        let findings = vec![
            "Leaf depends on Finding 2 and Finding 3.".into(),
            "Mid depends on Finding 3.".into(),
            "Root has no dependencies.".into(),
        ];
        let ordered = reorder_findings_by_dependency(&findings);
        assert_eq!(ordered[0], "Root has no dependencies.");
        // Mid is now Finding 2 and only depends on the root (Finding 1).
        assert!(
            ordered[1].contains("Finding 1"),
            "mid should reference root, got: {}",
            ordered[1]
        );
        assert!(
            !ordered[1].contains("Finding 3"),
            "mid should not retain old root number"
        );
        // Leaf is now Finding 3 and depends on mid (Finding 2) and root (Finding 1).
        assert!(ordered[2].contains("Finding 1") && ordered[2].contains("Finding 2"));
    }
    #[test]
    fn reorder_breaks_cycles_without_dropping_findings() {
        let findings = vec![
            "A depends on Finding 2.".into(),
            "B depends on Finding 1.".into(),
        ];
        let ordered = reorder_findings_by_dependency(&findings);
        assert_eq!(ordered.len(), 2);
        assert!(
            ordered[0].contains("Finding 2") || ordered[1].contains("Finding 1"),
            "cycle should be broken by keeping original order, got: {:?}",
            ordered
        );
    }

    #[test]
    fn reorder_is_noop_for_empty_or_single_finding() {
        assert!(reorder_findings_by_dependency(&[]).is_empty());
        let single = vec!["Only finding.".into()];
        assert_eq!(reorder_findings_by_dependency(&single), single);
    }

    #[test]
    fn parse_analysis_response_reorders_findings_by_dependency() {
        let text = "## Findings\n\n1. **Observation:** two. **Analysis:** a. **Cross-reference / Dependencies:** Depends on Finding 2. **Implication:** i.\n2. **Observation:** one. **Analysis:** b. **Cross-reference / Dependencies:** No direct dependencies. **Implication:** j.\n";
        let result = parse_analysis_response(text);
        assert_eq!(result.findings.len(), 2);
        assert!(
            result.findings[0].contains("No direct dependencies."),
            "first finding should be the root"
        );
        assert!(
            result.findings[1].contains("Finding 1"),
            "second finding should reference renumbered root"
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
    fn parse_numbered_list_handles_number_on_its_own_line() {
        let body = "1.\n\n**Observation:** obs1\n\n**Analysis:** a1\n\n2.\n\n**Observation:** obs2\n\n**Analysis:** a2\n";
        assert_eq!(
            parse_numbered_list(body),
            vec![
                "**Observation:** obs1\n**Analysis:** a1",
                "**Observation:** obs2\n**Analysis:** a2"
            ]
        );
    }
    #[test]
    fn parse_numbered_list_number_with_content_same_line() {
        let body = "1. **Observation:** obs1\n**Analysis:** a1\n2. **Observation:** obs2\n**Analysis:** a2\n";
        assert_eq!(
            parse_numbered_list(body),
            vec![
                "**Observation:** obs1\n**Analysis:** a1",
                "**Observation:** obs2\n**Analysis:** a2"
            ]
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

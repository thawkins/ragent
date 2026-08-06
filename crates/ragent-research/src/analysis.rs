//! Source analysis engine — turns gathered evidence into a structured
//! `AnalysisResult` using an LLM.
//!
//! The default [`LlmAnalysisEngine`] sends a single synthesis prompt to the
//! configured provider/model. The prompt asks for five sections that map
//! directly to the `RESEARCH.md` structure:
//!
//! - Executive Summary
//! - Top 5 Implications
//! - Findings
//! - In-Project Cross-References
//! - Open Questions
//!
//! A [`NoopAnalysisEngine`] is provided so callers can disable synthesis or use
//! the legacy mechanical fallback.

use crate::document::CrossReference;
use crate::item::strip_control_chars;
use crate::run_config::OutputFormat;
use crate::source::Source;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};
use ragent_llm::provider::ProviderRegistry;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

mod parser;
mod prompt;

use parser::parse_analysis_response_with_outcome;
use prompt::SynthesisPromptBuilder;

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
    /// Publication date parsed from the source's embedded metadata, when
    /// available. Populated by [`build_source_bodies`] from
    /// [`Source::published_at`]. `None` for local/spec sources and for web
    /// sources that did not expose a parseable publication date. Surfaced in
    /// the synthesis prompt (T-003) so the model can produce the
    /// **Sources Cited / Date Spread** paragraph.
    pub published_at: Option<DateTime<Utc>>,
}

/// Structured result returned by an analysis engine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnalysisResult {
    /// One-paragraph synthesis of the gathered evidence.
    pub summary: String,
    /// Numbered findings. Each entry is the markdown body of one finding.
    pub findings: Vec<String>,
    /// Top-ranked practical implications derived from the findings. The LLM
    /// is asked to produce exactly five numbered entries; fewer may be present
    /// when the model output is malformed or the mechanical fallback is used.
    pub top_implications: Vec<String>,
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

    /// Analyze the provided sources and topic, returning structured content
    /// plus an [`AnalysisOutcome`] that tells the caller whether the result
    /// came from a clean LLM parse or from the deterministic fallback path
    /// (FR-005 / T-005).
    ///
    /// The default implementation delegates to [`analyze`][Self::analyze] and
    /// tags the result [`AnalysisOutcome::Llm`]. Engines that perform their
    /// own malformed-output detection (e.g. [`LlmAnalysisEngine`]) override
    /// this to surface [`AnalysisOutcome::FallbackEmpty`] when the model
    /// output cannot be parsed into the required structure.
    async fn analyze_with_outcome(
        &self,
        topic: &str,
        sources: &[SourceBody],
    ) -> anyhow::Result<(AnalysisResult, AnalysisOutcome)> {
        let result = self.analyze(topic, sources).await?;
        Ok((result, AnalysisOutcome::Llm))
    }
}

/// Outcome of an analysis pass, surfaced by
/// [`AnalysisEngine::analyze_with_outcome`]. Mirrors the user-facing
/// [`crate::session::SynthesizeOutcome`] but lives in `analysis.rs` so the
/// engine can return it without a circular dependency on `session.rs`.
/// `session.rs` maps this to [`SynthesizeOutcome`] when emitting the
/// `SynthesizeResult` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisOutcome {
    /// The model produced a structured [`AnalysisResult`] that parsed cleanly.
    Llm,
    /// The model output was empty or could not be parsed into the required
    /// structure; the deterministic mechanical fallback supplied the
    /// summary/findings (FR-005).
    FallbackEmpty,
    /// The LLM-backed engine returned an error (no key, network failure, …)
    /// and the mechanical fallback supplied the summary/findings. Surfaced by
    /// `session.rs` mapping an `Err` from [`AnalysisEngine::analyze`] to
    /// [`SynthesizeOutcome::FallbackError`]; engines that override
    /// [`AnalysisEngine::analyze_with_outcome`] generally return
    /// [`AnalysisOutcome::FallbackEmpty`] instead.
    FallbackError,
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
    /// Optional override for the `system` message persona (FR-009 / T-008).
    /// When `None`, the engine uses its default "careful research analyst"
    /// system prompt. When `Some`, the supplied string replaces the default
    /// system message verbatim, letting callers tailor voice, audience, and
    /// domain framing (e.g. `"You are a senior security research analyst for
    /// a venture-capital audience."`).
    persona: Option<String>,
    /// Optional output format requested via `--format`.
    output_format: Option<OutputFormat>,
    /// Per-source character budget for the heuristic summarizer
    /// (Milestone E-001). When `Some(n)`, each source body is collapsed to
    /// at most `n` characters before entering the synthesis prompt. When
    /// `None`, the default `truncate_body` limit (4000 chars) in
    /// [`render_sources_block`] is the only truncation applied.
    source_summary_budget: Option<usize>,
    /// Total source-body character threshold that triggers chunked LLM
    /// synthesis (Milestone E-002). When the sum of all source body
    /// characters exceeds this value, sources are split into chunks and
    /// sent in separate LLM calls; partial results are merged via
    /// [`merge_chunk_results`]. When `None`, chunking is disabled and the
    /// engine sends a single call regardless of corpus size.
    synthesis_chunk_threshold: Option<usize>,
    /// Maximum total source-body characters per chunk (Milestone E-002).
    /// Defaults to 48_000 when `synthesis_chunk_threshold` is set but this
    /// is `None`. Each chunk's total body chars stays at or below this value.
    synthesis_chunk_size: Option<usize>,
}

impl std::fmt::Debug for LlmAnalysisEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmAnalysisEngine")
            .field("provider_id", &self.provider_id)
            .field("model_id", &self.model_id)
            .field("base_url", &self.base_url)
            .field("has_api_key", &self.api_key.is_some())
            .field("has_persona", &self.persona.is_some())
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
            persona: None,
            output_format: None,
            source_summary_budget: None,
            synthesis_chunk_threshold: None,
            synthesis_chunk_size: None,
        }
    }

    /// Provide an API key for the provider.
    #[must_use]
    pub fn with_api_key(mut self, api_key: Option<String>) -> Self {
        self.api_key = api_key;
        self
    }

    /// Override the API base URL. If unset, the engine resolves it from storage
    /// / config / env at analysis time.
    #[must_use]
    pub fn with_base_url(mut self, base_url: Option<String>) -> Self {
        self.base_url = base_url;
        self
    }

    /// Override the `system` message persona (FR-009 / T-008). When set, the
    /// supplied string replaces the default "careful research analyst" system
    /// prompt verbatim. Pass `None` (or never call this) to keep the default.
    #[must_use]
    pub fn with_persona(mut self, persona: Option<String>) -> Self {
        self.persona = persona;
        self
    }

    /// Set the output format requested via `--format`.
    #[must_use]
    pub const fn with_output_format(mut self, fmt: Option<OutputFormat>) -> Self {
        self.output_format = fmt;
        self
    }

    /// Set the per-source character budget for the heuristic summarizer
    /// (Milestone E-001). When set, each source body is collapsed to at most
    /// `budget` characters via [`HeuristicSummarizer`] before entering the
    /// synthesis prompt.
    #[must_use]
    pub const fn with_source_summary_budget(mut self, budget: Option<usize>) -> Self {
        self.source_summary_budget = budget;
        self
    }

    /// Set the total source-body character threshold that triggers chunked
    /// LLM synthesis (Milestone E-002). When the total body chars across all
    /// sources exceeds `threshold`, sources are split into chunks and sent
    /// in separate LLM calls; partial results are merged.
    #[must_use]
    pub const fn with_synthesis_chunk_threshold(mut self, threshold: Option<usize>) -> Self {
        self.synthesis_chunk_threshold = threshold;
        self
    }

    /// Set the maximum total source-body characters per chunk (Milestone
    /// E-002). Defaults to 48_000 when chunking is enabled but this is not
    /// set.
    #[must_use]
    pub const fn with_synthesis_chunk_size(mut self, size: Option<usize>) -> Self {
        self.synthesis_chunk_size = size;
        self
    }

    /// Default per-chunk body character budget (Milestone E-002).
    const DEFAULT_CHUNK_SIZE: usize = 48_000;
}

#[async_trait::async_trait]
impl AnalysisEngine for LlmAnalysisEngine {
    async fn analyze(&self, topic: &str, sources: &[SourceBody]) -> anyhow::Result<AnalysisResult> {
        let (result, _) = self.analyze_with_outcome(topic, sources).await?;
        Ok(result)
    }

    /// Override [`AnalysisEngine::analyze_with_outcome`] so the LLM engine
    /// can distinguish a clean parse ([`AnalysisOutcome::Llm`]) from a
    /// malformed response rescued by the mechanical fallback
    /// ([`AnalysisOutcome::FallbackEmpty`]) — FR-005 / T-005. Provider
    /// errors still surface as `Err`, which `session.rs` maps to
    /// [`crate::session::SynthesizeOutcome::FallbackError`].
    ///
    /// **Milestone E-001/E-002:** Before sending the prompt, each source body
    /// is collapsed to `source_summary_budget` chars (when configured) via
    /// [`HeuristicSummarizer`]. When the total body volume exceeds
    /// `synthesis_chunk_threshold`, sources are split into chunks and sent
    /// in separate LLM calls; partial results are merged via
    /// [`merge_chunk_results`]. The outcome is `Llm` when at least one chunk
    /// produced a clean parse; `FallbackEmpty` when every chunk fell back.
    async fn analyze_with_outcome(
        &self,
        topic: &str,
        sources: &[SourceBody],
    ) -> anyhow::Result<(AnalysisResult, AnalysisOutcome)> {
        // E-001: collapse each source body to the configured budget.
        let prepared: Vec<SourceBody> = if let Some(budget) = self.source_summary_budget {
            let summarizer = HeuristicSummarizer;
            summarize_source_bodies(sources, &summarizer, budget)
        } else {
            sources.to_vec()
        };

        // E-002: decide whether to chunk.
        let threshold = self.synthesis_chunk_threshold;
        let total = total_body_chars(&prepared);
        let needs_chunking = threshold.is_some_and(|t| total > t);

        if !needs_chunking {
            // Single-call path (legacy behavior).
            let text = self.stream_synthesis(topic, &prepared).await?;
            return Ok(parse_analysis_response_with_outcome(&text, &prepared));
        }

        // Chunked path: split sources, send each chunk, merge results.
        let chunk_size = self
            .synthesis_chunk_size
            .unwrap_or(Self::DEFAULT_CHUNK_SIZE);
        let chunks = chunk_source_bodies(&prepared, chunk_size);
        tracing::info!(
            chunks = chunks.len(),
            total_body_chars = total,
            chunk_size,
            "research: chunked synthesis enabled"
        );

        let mut parts: Vec<AnalysisResult> = Vec::with_capacity(chunks.len());
        let mut all_clean = true;
        // Collect all source bodies across chunks for citation validation.
        for chunk in &chunks {
            let text = self.stream_synthesis(topic, chunk).await?;
            let (result, outcome) = parse_analysis_response_with_outcome(&text, chunk);
            if outcome == AnalysisOutcome::FallbackEmpty {
                all_clean = false;
            }
            parts.push(result);
        }

        let merged = merge_chunk_results(&parts);
        let outcome = if all_clean && !merged.findings.is_empty() {
            AnalysisOutcome::Llm
        } else {
            AnalysisOutcome::FallbackEmpty
        };
        Ok((merged, outcome))
    }
}

impl LlmAnalysisEngine {
    /// Ask the LLM to summarise a document body so a caller can derive a
    /// concise research topic and a clean human-readable title from it.
    ///
    /// This is used by the `--from-url` / `--from-file` pre-steps to replace
    /// brittle heuristic topic extraction (first-sentence scraping of a
    /// readability-stripped page) with a model-generated summary that
    /// understands the full document.
    ///
    /// Returns `Some((topic, title))` on success; `None` when the provider
    /// is unavailable, the request fails, or the model output cannot be
    /// parsed. Callers should fall back to their local heuristics when
    /// `None` is returned so the feature degrades gracefully without an LLM.
    pub async fn summarize_subject(&self, body: &str) -> Option<(String, String)> {
        let provider = self.provider_registry.get(&self.provider_id)?;
        let api_key = self.api_key.clone().unwrap_or_default();
        let client = provider
            .create_client(&api_key, self.base_url.as_deref(), &HashMap::new())
            .await
            .ok()?;

        const MAX_INPUT_CHARS: usize = 12_000;
        let truncated: String = body.chars().take(MAX_INPUT_CHARS).collect();
        let prompt = format!(
            "Summarize the following document into:\n\
             1. a concise research topic (1-2 sentences, <= 160 chars)\n\
             2. a clean human-readable title (<= 80 chars)\n\
             Return ONLY a JSON object with keys \"topic\" and \"title\".\n\n\
             Document:\n{truncated}"
        );
        let request = ChatRequest {
            model: self.model_id.clone(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text(prompt),
            }]),
            tools: Arc::new(vec![]),
            temperature: Some(0.2),
            top_p: Some(1.0),
            max_tokens: Some(512),
            system: Some(std::sync::Arc::from(
                "Return only valid JSON. No prose, no markdown fences.",
            )),
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: Some(60),
            thinking: None,
        };

        let mut stream = client.chat(request).await.ok()?;
        let mut text = String::new();
        while let Some(event) = stream.next().await {
            match event {
                StreamEvent::TextDelta { text: delta } => text.push_str(&delta),
                StreamEvent::Error { .. } | StreamEvent::Finish { .. } => break,
                _ => {}
            }
        }
        parse_subject_summary(&text)
    }

    /// Issue the synthesis request to the provider and return the raw model
    /// text. Shared by [`AnalysisEngine::analyze`] (which parses strictly)
    /// and [`AnalysisEngine::analyze_with_outcome`] (which parses with
    /// fallback detection) so the streaming code lives in one place.
    async fn stream_synthesis(
        &self,
        topic: &str,
        sources: &[SourceBody],
    ) -> anyhow::Result<String> {
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

        let prompt = SynthesisPromptBuilder::new(topic)
            .sources(sources)
            .output_format(self.output_format.unwrap_or(OutputFormat::Report))
            .build();
        // T-008 / FR-009: allow a configurable analysis persona. When
        // `config.persona` is supplied via `ragent.json`
        // (`research.analysis_persona`), it overrides the default
        // "careful research analyst" system message. The default persona is
        // preserved when `persona` is `None`, so the legacy behavior is
        // unchanged for callers that don't wire the new config in.
        let system_persona: std::sync::Arc<str> = match &self.persona {
            Some(p) => std::sync::Arc::from(p.as_str()),
            None => std::sync::Arc::from(
                "You are a careful research analyst. Read the provided sources and produce a structured markdown analysis. Use only the evidence in the sources; do not invent facts.",
            ),
        };
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
            system: Some(system_persona),
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
        Ok(text)
    }
}

/// Parse the LLM JSON response from [`LlmAnalysisEngine::summarize_subject`]
/// into a `(topic, title)` pair.
///
/// The model is asked to return only a JSON object, but in practice providers
/// wrap output in markdown fences, prepend whitespace, or emit BOM/control
/// characters. This helper strips that noise before parsing and returns
/// `None` when no usable JSON object can be recovered, so callers can fall
/// back to their local heuristics.
fn parse_subject_summary(text: &str) -> Option<(String, String)> {
    #[derive(serde::Deserialize)]
    struct SubjectSummary {
        topic: String,
        title: String,
    }

    // Strip control chars (except whitespace we rely on) and BOM so JSON
    // parsing isn't tripped up by provider quirks.
    let sanitized = strip_control_chars(text);
    let trimmed = sanitized.trim().trim_start_matches('\u{feff}').trim();

    // Fast path: the whole response is a JSON object.
    let parsed: Option<SubjectSummary> = serde_json::from_str(trimmed).ok();
    // Slow path: locate the outermost `{...}` span (handles ```json fences
    // and any surrounding prose).
    let parsed = parsed.or_else(|| {
        let start = trimmed.find('{')?;
        let end = trimmed.rfind('}')?;
        serde_json::from_str::<SubjectSummary>(&trimmed[start..=end]).ok()
    })?;

    let topic = parsed.topic.trim().to_string();
    let title = parsed.title.trim().to_string();
    if topic.is_empty() || title.is_empty() {
        return None;
    }
    Some((topic, title))
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
            published_at: src.published_at(),
        })
        .collect()
}

// ── E-001: SourceSummarizer trait + heuristic implementation ──────────────

/// Trait for collapsing a source body to a fixed character budget before it
/// enters the synthesis prompt (Milestone E-001).
///
/// The default [`HeuristicSummarizer`] keeps the leading portion of the body,
/// snaps to a paragraph boundary when possible, and appends a truncation
/// marker. Future implementations could use an LLM to produce a true summary;
/// the trait abstraction lets callers swap summarizers without touching the
/// synthesis pipeline.
pub trait SourceSummarizer: Send + Sync {
    /// Summarize `body` so the result fits within `budget_chars` characters.
    fn summarize(&self, body: &str, budget_chars: usize) -> String;
}

/// Heuristic source-body summarizer (Milestone E-001).
///
/// Strategy:
/// 1. If the body already fits the budget, return it unchanged.
/// 2. Otherwise, take the first `budget_chars` characters, then back up to the
///    last paragraph break (`\n\n`) within that window so the summary ends on a
///    clean paragraph boundary.
/// 3. If no paragraph break exists in the window, cut at the last sentence
///    boundary (`.` followed by whitespace or end-of-line).
/// 4. If no sentence boundary exists either, cut at the last whitespace.
/// 5. Append a `\n\n… (summarized — see full source for remaining content)`
///    marker so the model knows the body was condensed.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeuristicSummarizer;

impl SourceSummarizer for HeuristicSummarizer {
    fn summarize(&self, body: &str, budget_chars: usize) -> String {
        if body.chars().count() <= budget_chars {
            return body.to_string();
        }
        let mut window: String = body.chars().take(budget_chars).collect();
        // Try to snap to the last paragraph boundary.
        if let Some(pos) = window.rfind("\n\n") {
            if pos > budget_chars / 4 {
                window.truncate(pos);
            }
        } else if let Some(pos) = window
            .char_indices()
            .rev()
            .find(|(i, c)| {
                *c == '.'
                    && window
                        .get(*i + 1..)
                        .and_then(|rest| rest.chars().next())
                        .is_some_and(|next| next.is_whitespace() || next == '\n')
            })
            .map(|(i, _)| i)
        {
            if pos > budget_chars / 4 {
                window.truncate(pos + 1);
            }
        } else if let Some(pos) = window.rfind(|c: char| c.is_whitespace())
            && pos > budget_chars / 4
        {
            window.truncate(pos);
        }
        window.push_str("\n\n… (summarized — see full source for remaining content)");
        window
    }
}

/// Apply a [`SourceSummarizer`] to every body in `bodies`, returning new
/// [`SourceBody`] values whose `body` field has been collapsed to
/// `budget_chars`. Non-body fields (index, kind, title, etc.) are preserved
/// verbatim (Milestone E-001).
pub fn summarize_source_bodies(
    bodies: &[SourceBody],
    summarizer: &dyn SourceSummarizer,
    budget_chars: usize,
) -> Vec<SourceBody> {
    bodies
        .iter()
        .map(|sb| SourceBody {
            body: summarizer.summarize(&sb.body, budget_chars),
            ..sb.clone()
        })
        .collect()
}

/// Compute the total character count of all source bodies in `bodies`.
/// Used to decide whether chunked synthesis is needed (Milestone E-002).
pub fn total_body_chars(bodies: &[SourceBody]) -> usize {
    bodies.iter().map(|sb| sb.body.chars().count()).sum()
}

/// Split `bodies` into chunks whose total body character count does not exceed
/// `max_chars_per_chunk`. Each chunk is a contiguous slice of the input;
/// source indices are preserved so `[#N]` citations remain valid across
/// chunks (Milestone E-002).
///
/// A single source whose body exceeds `max_chars_per_chunk` forms its own
/// chunk (it will be summarized by the caller before reaching this function,
/// so this is a defense-in-depth guard).
pub fn chunk_source_bodies(
    bodies: &[SourceBody],
    max_chars_per_chunk: usize,
) -> Vec<Vec<SourceBody>> {
    let mut chunks: Vec<Vec<SourceBody>> = Vec::new();
    let mut current: Vec<SourceBody> = Vec::new();
    let mut current_chars: usize = 0;
    for sb in bodies {
        let body_chars = sb.body.chars().count();
        if !current.is_empty() && current_chars + body_chars > max_chars_per_chunk {
            chunks.push(std::mem::take(&mut current));
            current_chars = 0;
        }
        current.push(sb.clone());
        current_chars += body_chars;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Merge multiple partial [`AnalysisResult`]s from chunked LLM calls into a
/// single combined result (Milestone E-002).
///
/// - **Summary**: the first non-empty summary is used. When multiple chunks
///   produce summaries, they are concatenated with a separator so the model's
///   per-chunk overviews are preserved.
/// - **Findings**: all findings from all chunks are concatenated and
///   renumbered sequentially (the `1.`, `2.` prefixes are rewritten so the
///   final document has a contiguous numbering).
/// - **Cross-references**: deduplicated by path (first occurrence wins).
/// - **Open questions**: concatenated, removing exact duplicates.
pub fn merge_chunk_results(parts: &[AnalysisResult]) -> AnalysisResult {
    if parts.is_empty() {
        return AnalysisResult::default();
    }
    if parts.len() == 1 {
        return parts[0].clone();
    }

    // Merge summaries: collect non-empty ones and join.
    let summaries: Vec<&str> = parts
        .iter()
        .map(|p| p.summary.as_str())
        .filter(|s| !s.is_empty())
        .collect();
    let summary = if summaries.is_empty() {
        String::new()
    } else if summaries.len() == 1 {
        summaries[0].to_string()
    } else {
        summaries.join("\n\n---\n\n")
    };

    // Merge findings: concatenate, then renumber.
    let mut all_findings: Vec<String> = Vec::new();
    for part in parts {
        all_findings.extend(part.findings.clone());
    }
    let findings = renumber_findings(&all_findings);

    // Merge cross-references: dedup by path.
    let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut cross_references = Vec::new();
    for part in parts {
        for cr in &part.cross_references {
            if seen_paths.insert(cr.path.clone()) {
                cross_references.push(cr.clone());
            }
        }
    }

    // Merge top implications: concatenate, dedup exact matches, preserve rank.
    let mut seen_implications: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut top_implications = Vec::new();
    for part in parts {
        for imp in &part.top_implications {
            if seen_implications.insert(imp.clone()) {
                top_implications.push(imp.clone());
            }
        }
    }

    // Merge open questions: concatenate, dedup exact matches.
    let mut seen_questions: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut open_questions = Vec::new();
    for part in parts {
        for q in &part.open_questions {
            if seen_questions.insert(q.clone()) {
                open_questions.push(q.clone());
            }
        }
    }

    AnalysisResult {
        summary,
        findings,
        top_implications,
        cross_references,
        open_questions,
    }
}

/// Renumber the `1.`, `2.`, … prefixes in a list of findings so they are
/// contiguous starting from 1. Findings without a numeric prefix are left
/// unchanged (Milestone E-002).
fn renumber_findings(findings: &[String]) -> Vec<String> {
    let num_re = Regex::new(r"^(\d+)\.\s*").expect("valid renumber regex");
    findings
        .iter()
        .enumerate()
        .map(|(i, finding)| {
            if num_re.is_match(finding) {
                num_re.replace(finding, format!("{}. ", i + 1)).to_string()
            } else {
                finding.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parser::{
        mechanical_fallback_findings, parse_analysis_response, parse_bullet_list,
        parse_numbered_list, reorder_findings_by_dependency, truncate_body,
        validate_citations_and_dates,
    };
    use super::prompt::SynthesisPromptConfig;
    use super::*;
    use prompt::{SynthesisPromptBuilder, build_synthesis_prompt};

    #[test]
    fn parse_analysis_response_extracts_all_sections() {
        let text = "## Executive Summary\n\nThis is the executive summary.\n\n## Findings\n\n1. First finding.\n2. Second finding.\n\n## Top 5 Implications\n\n1. Implication A.\n2. Implication B.\n\n## In-Project Cross-References\n\n* `src/lib.rs` — main entry\n* `src/foo.rs` — helper\n\n## Open Questions\n\n* What about X?\n* How does Y work?\n";
        let result = parse_analysis_response(text);
        assert_eq!(result.summary, "This is the executive summary.");
        assert_eq!(result.findings, vec!["First finding.", "Second finding."]);
        assert_eq!(
            result.top_implications,
            vec!["Implication A.", "Implication B."]
        );
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
            "cycle should be broken by keeping original order, got: {ordered:?}"
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
        let text = "## Findings\n\n1. **Headline:** Two\n\n**Observation:** two. **Analysis:** a. **Cross-reference / Dependencies:** Depends on Finding 2. **Implication:** i.\n2. **Headline:** One\n\n**Observation:** one. **Analysis:** b. **Cross-reference / Dependencies:** No direct dependencies. **Implication:** j.\n";
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

    // ── researchprompt T-011: builder + parser/fallback tests ─────────────

    /// Helper: build a minimal [`SourceBody`] with the given index and
    /// optional publication date.
    fn src_body(index: usize, published_at: Option<DateTime<Utc>>) -> SourceBody {
        SourceBody {
            index,
            kind: "web".to_string(),
            title: format!("Source {index}"),
            path_or_url: format!("https://example.com/{index}"),
            relevance: String::new(),
            body: format!("Body of source {index}"),
            published_at,
        }
    }

    #[test]
    fn output_format_executive_summary_shortens_instructions() {
        let sources = vec![src_body(1, None)];
        let prompt = SynthesisPromptBuilder::new("topic")
            .sources(&sources)
            .output_format(OutputFormat::ExecutiveSummary)
            .build();
        assert!(prompt.contains("very concise executive summary"));
        assert!(prompt.contains("At most 5 high-level findings"));
    }

    #[test]
    fn output_format_comparison_table_includes_table_request() {
        let sources = vec![src_body(1, None)];
        let prompt = SynthesisPromptBuilder::new("topic")
            .sources(&sources)
            .output_format(OutputFormat::ComparisonTable)
            .build();
        assert!(prompt.contains("## Comparison Table"));
        assert!(prompt.contains("markdown table"));
    }

    #[test]
    fn output_format_source_bibliography_annotated_entries() {
        let sources = vec![src_body(1, None)];
        let prompt = SynthesisPromptBuilder::new("topic")
            .sources(&sources)
            .output_format(OutputFormat::SourceBibliography)
            .build();
        assert!(prompt.contains("annotated bibliography"));
    }

    #[test]
    fn builder_default_includes_top_5_implications_section() {
        let sources = vec![src_body(1, None), src_body(2, None)];
        let legacy = build_synthesis_prompt("topic", &sources);
        let builder = SynthesisPromptBuilder::new("topic")
            .sources(&sources)
            .build();
        // The default-config builder now asks for the Top 5 Implications
        // section, so it is no longer byte-identical to the legacy four-section
        // prompt. Both the legacy wrapper and the builder must include it.
        assert!(legacy.contains("## Top 5 Implications"));
        assert!(builder.contains("## Top 5 Implications"));
        assert!(legacy.contains("## In-Project Cross-References"));
        assert!(builder.contains("## Open Questions"));
    }

    #[test]
    fn builder_emits_five_required_finding_labels() {
        let sources = vec![src_body(1, None)];
        let prompt = SynthesisPromptBuilder::new("topic")
            .sources(&sources)
            .build();
        assert!(prompt.contains("**Headline:**"));
        assert!(prompt.contains("**Observation:**"));
        assert!(prompt.contains("**Analysis:**"));
        assert!(prompt.contains("**Cross-reference / Dependencies:**"));
        assert!(prompt.contains("**Implication:**"));
    }

    #[test]
    fn builder_emits_top_5_implications_section() {
        let sources = vec![src_body(1, None)];
        let prompt = SynthesisPromptBuilder::new("topic")
            .sources(&sources)
            .build();
        assert!(prompt.contains("## Top 5 Implications"));
        assert!(prompt.contains("rank the top 5 implications"));
    }

    #[test]
    fn builder_date_spread_paragraph_adds_sixth_label_and_published_line() {
        let sources = vec![
            src_body(
                1,
                Some(DateTime::from_naive_utc_and_offset(
                    chrono::NaiveDate::from_ymd_opt(2026, 1, 15)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap(),
                    Utc,
                )),
            ),
            src_body(2, None),
        ];
        let config = SynthesisPromptConfig {
            date_spread_paragraph: true,
            ..Default::default()
        };
        let prompt = SynthesisPromptBuilder::new("topic")
            .sources(&sources)
            .config(config)
            .build();
        assert!(
            prompt.contains("**Sources Cited / Date Spread:**"),
            "date-spread paragraph must be required when configured"
        );
        assert!(
            prompt.contains("Published (UTC): 2026-01-15"),
            "dated web sources must surface their publication date in the source block"
        );
        assert!(
            prompt.contains("Published (UTC): undated"),
            "undated web sources must be labelled undated in the source block"
        );
    }

    #[test]
    fn builder_recency_rule_emits_recency_instructions() {
        let sources = vec![src_body(1, None)];
        let config = SynthesisPromptConfig {
            recency_rule: true,
            ..Default::default()
        };
        let prompt = SynthesisPromptBuilder::new("topic")
            .sources(&sources)
            .config(config)
            .build();
        assert!(
            prompt.contains("Recency-weighting rule"),
            "recency rule block must be emitted when configured"
        );
        assert!(prompt.contains("prefer the more recently published source"));
    }

    #[test]
    fn builder_few_shot_appends_exemplars() {
        let sources = vec![src_body(1, None)];
        let exemplar = "**Observation:** example obs [#1].\n\n**Analysis:** a.\n\n\
             **Cross-reference / Dependencies:** No direct dependencies.\n\n\
             **Implication:** i."
            .to_string();
        let config = SynthesisPromptConfig {
            few_shot_examples: vec![exemplar],
            ..Default::default()
        };
        let prompt = SynthesisPromptBuilder::new("topic")
            .sources(&sources)
            .config(config)
            .build();
        assert!(prompt.contains("Few-shot exemplar findings"));
        assert!(prompt.contains("### Exemplar Finding 1"));
        assert!(prompt.contains("example obs [#1]"));
    }

    #[test]
    fn builder_few_shot_caps_at_two_exemplars() {
        let sources = vec![src_body(1, None)];
        let make = |n: usize| {
            format!(
                "**Observation:** obs {n} [#1].\n\n**Analysis:** a.\n\n\
                 **Cross-reference / Dependencies:** No direct dependencies.\n\n\
                 **Implication:** i."
            )
        };
        let config = SynthesisPromptConfig {
            few_shot_examples: vec![make(1), make(2), make(3)],
            ..Default::default()
        };
        let prompt = SynthesisPromptBuilder::new("topic")
            .sources(&sources)
            .config(config)
            .build();
        assert!(prompt.contains("### Exemplar Finding 1"));
        assert!(prompt.contains("### Exemplar Finding 2"));
        assert!(
            !prompt.contains("### Exemplar Finding 3"),
            "few-shot block must cap at two exemplars to bound context cost"
        );
    }

    #[test]
    fn parse_with_outcome_clean_response_returns_llm() {
        let text = "## Executive Summary\n\nAn executive summary.\n\n## Findings\n\n\
             1. **Headline:** Observation summary\n\n**Observation:** obs [#1].\n\n\
             **Analysis:** a.\n\n\
             **Cross-reference / Dependencies:** No direct dependencies.\n\n\
             **Implication:** i.\n";
        let sources = vec![src_body(1, None)];
        let (result, outcome) = parse_analysis_response_with_outcome(text, &sources);
        assert_eq!(outcome, AnalysisOutcome::Llm);
        assert_eq!(result.findings.len(), 1);
        assert!(result.findings[0].contains("**Observation:** obs [#1]"));
    }

    #[test]
    fn parse_with_outcome_empty_response_falls_back() {
        let sources = vec![src_body(1, None)];
        let (result, outcome) = parse_analysis_response_with_outcome("", &sources);
        assert_eq!(outcome, AnalysisOutcome::FallbackEmpty);
        // FR-011 / T-010: fallback always produces >=1 finding.
        assert!(!result.findings.is_empty());
        assert!(result.findings[0].contains("**Observation:**"));
        assert!(result.findings[0].contains("**Analysis:**"));
        assert!(result.findings[0].contains("**Analysis:**"));
        assert!(result.findings[0].contains("**Cross-reference / Dependencies:**"));
        assert!(result.findings[0].contains("**Implication:**"));
    }

    #[test]
    fn parse_with_outcome_no_findings_section_falls_back() {
        // A response that only has a summary (no ## Findings) is malformed.
        let text = "## Executive Summary\n\nOnly an executive summary, no findings section.\n";
        let sources = vec![src_body(1, None)];
        let (result, outcome) = parse_analysis_response_with_outcome(text, &sources);
        assert_eq!(outcome, AnalysisOutcome::FallbackEmpty);
        assert!(!result.findings.is_empty());
    }

    #[test]
    fn parse_with_outcome_finding_missing_labels_falls_back() {
        // A finding that lacks the required bold labels is malformed.
        let text = "## Findings\n\n1. Just a plain finding with no labels and no citation.\n";
        let sources = vec![src_body(1, None)];
        let (result, outcome) = parse_analysis_response_with_outcome(text, &sources);
        assert_eq!(outcome, AnalysisOutcome::FallbackEmpty);
        assert!(!result.findings.is_empty());
        // The mechanical fallback inserts the missing labels as placeholders.
        assert!(result.findings[0].contains("**Observation:**"));
    }

    #[test]
    fn mechanical_fallback_never_returns_empty_vec() {
        // FR-011 / T-010 non-empty guarantee: exercise several degenerate
        // inputs and confirm at least one finding is always produced.
        for input in [
            "",
            "   \n\n  ",
            "## Executive Summary\n\nonly executive summary",
            "no headings at all",
        ] {
            let findings = mechanical_fallback_findings(input);
            assert!(
                !findings.is_empty(),
                "input {input:?} should yield >=1 finding"
            );
            for f in &findings {
                assert!(f.contains("**Observation:**"));
                assert!(f.contains("**Analysis:**"));
                assert!(f.contains("**Cross-reference / Dependencies:**"));
                assert!(f.contains("**Implication:**"));
            }
        }
    }

    #[test]
    fn mechanical_fallback_preserves_raw_text_in_placeholder() {
        // An empty model response hits the placeholder branch that quotes the
        // raw model output (FR-011 / T-010 non-empty guarantee).
        let findings = mechanical_fallback_findings("");
        assert_eq!(findings.len(), 1);
        assert!(
            findings[0].contains("(findings could not be structured — see below)"),
            "placeholder must use the spec's wording, got: {}",
            findings[0]
        );
        assert!(
            findings[0].contains("(no model response was returned)"),
            "empty-response placeholder must explain the model returned no content, got: {}",
            findings[0]
        );
    }

    #[test]
    fn mechanical_fallback_preserves_nonempty_raw_in_placeholder() {
        // A whitespace-only response has no extractable structure and hits the
        // placeholder branch (extract_candidate_findings returns [] because
        // parse_numbered_list and the paragraph splitter both yield nothing).
        let findings = mechanical_fallback_findings("   \n\n  \n");
        assert_eq!(findings.len(), 1);
        assert!(
            findings[0].contains("(findings could not be structured — see below)"),
            "placeholder must use the spec's wording for whitespace-only input, got: {}",
            findings[0]
        );
    }

    #[test]
    fn validate_citations_flags_out_of_range() {
        // Source list has 2 entries; a [#5] citation is out of range.
        let sources = vec![src_body(1, None), src_body(2, None)];
        let mut findings = vec![
            "**Observation:** obs [#1] and [#5].\n\n**Analysis:** a.\n\n\
             **Cross-reference / Dependencies:** No direct dependencies.\n\n\
             **Implication:** i."
                .to_string(),
        ];
        let warnings = validate_citations_and_dates(&mut findings, &sources);
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("out of range") || w.contains("source(s) were captured")),
            "expected an out-of-range citation warning, got {warnings:?}"
        );
        assert!(
            findings[0].contains("[#5?] (out of range"),
            "out-of-range citation must be rewritten inline, got: {}",
            findings[0]
        );
        assert!(
            findings[0].contains("[#1]"),
            "in-range citations must be preserved verbatim"
        );
    }

    #[test]
    fn validate_dates_flags_unsupported_claim() {
        let valid = DateTime::from_naive_utc_and_offset(
            chrono::NaiveDate::from_ymd_opt(2026, 1, 15)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            Utc,
        );
        let sources = vec![src_body(1, Some(valid))];
        // The finding cites [#1] (valid) but claims a date (1999-12-31) that
        // is not among the captured sources' publication dates.
        let mut findings = vec![
            "**Observation:** obs [#1].\n\n**Analysis:** a.\n\n\
             **Cross-reference / Dependencies:** No direct dependencies.\n\n\
             **Implication:** i.\n\n\
             **Sources Cited / Date Spread:** [#1] published 1999-12-31..2026-01-15."
                .to_string(),
        ];
        let warnings = validate_citations_and_dates(&mut findings, &sources);
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("1999-12-31") && w.contains("not among")),
            "expected an unsupported-date warning, got {warnings:?}"
        );
        assert!(
            findings[0].contains("(unsupported date)"),
            "unsupported date must be rewritten inline, got: {}",
            findings[0]
        );
        // The valid date (2026-01-15) must be preserved.
        assert!(findings[0].contains("2026-01-15"));
    }

    #[test]
    fn validate_leaves_valid_finding_untouched() {
        let valid = DateTime::from_naive_utc_and_offset(
            chrono::NaiveDate::from_ymd_opt(2026, 1, 15)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            Utc,
        );
        let sources = vec![src_body(1, Some(valid))];
        let original = "**Observation:** obs [#1].\n\n**Analysis:** a.\n\n\
             **Cross-reference / Dependencies:** No direct dependencies.\n\n\
             **Implication:** i.\n\n\
             **Sources Cited / Date Spread:** [#1] published 2026-01-15."
            .to_string();
        let mut findings = vec![original.clone()];
        let warnings = validate_citations_and_dates(&mut findings, &sources);
        assert!(
            warnings.is_empty(),
            "no warnings expected, got {warnings:?}"
        );
        assert_eq!(findings[0], original, "valid finding must be unchanged");
    }

    #[test]
    fn parse_with_outcome_preserves_valid_summary_on_fallback() {
        // A response with a valid ## Executive Summary but malformed findings (no
        // required bold labels) must preserve the parsed summary rather
        // than discarding it for a diagnostic placeholder.
        let text = "## Executive Summary\n\nThis is a valid executive summary that must be preserved.\n\n\
             ## Findings\n\n1. Just a plain finding with no labels and no citation.\n";
        let sources = vec![src_body(1, None)];
        let (result, outcome) = parse_analysis_response_with_outcome(text, &sources);
        assert_eq!(outcome, AnalysisOutcome::FallbackEmpty);
        assert!(
            result
                .summary
                .contains("This is a valid executive summary that must be preserved."),
            "valid executive summary must be preserved on fallback, got: {}",
            result.summary
        );
    }

    #[test]
    fn parse_with_outcome_strips_control_chars_from_clean_parse() {
        // A clean response that contains C0 control characters (e.g. 0x01)
        // must have them stripped from findings and summary.
        let text = "## Executive Summary\n\nExecutive summary with \x01 control char.\n\n\
             ## Findings\n\n\
             1. **Headline:** Obs\x02summary\n\n**Observation:** obs [#1].\x03\n\n\
             **Analysis:** a.\n\n\
             **Cross-reference / Dependencies:** No direct dependencies.\n\n\
             **Implication:** i.\n\n\
             ## Top 5 Implications\n\n1. \x04Implication.\n";
        let sources = vec![src_body(1, None)];
        let (result, outcome) = parse_analysis_response_with_outcome(text, &sources);
        assert_eq!(outcome, AnalysisOutcome::Llm);
        assert!(
            !result.summary.contains('\x01'),
            "control chars must be stripped from summary, got: {:?}",
            result.summary
        );
        assert!(
            !result.findings[0].contains('\x02') && !result.findings[0].contains('\x03'),
            "control chars must be stripped from findings, got: {:?}",
            result.findings[0]
        );
        assert!(
            !result.top_implications[0].contains('\x04'),
            "control chars must be stripped from top implications, got: {:?}",
            result.top_implications[0]
        );
    }

    #[test]
    fn parse_with_outcome_strips_control_chars_from_fallback() {
        // A malformed response with control chars must sanitize them before
        // mechanical extraction so the placeholder finding is clean.
        let text = "## Summary\n\n\x01Bad summary\x02\n\nNo findings here.\n";
        let sources = vec![src_body(1, None)];
        let (result, outcome) = parse_analysis_response_with_outcome(text, &sources);
        assert_eq!(outcome, AnalysisOutcome::FallbackEmpty);
        for finding in &result.findings {
            assert!(
                !finding.contains('\x01') && !finding.contains('\x02'),
                "control chars must be stripped from fallback findings, got: {:?}",
                finding
            );
        }
    }

    // ── Milestone E-001: SourceSummarizer / HeuristicSummarizer tests ────

    #[test]
    fn heuristic_summarizer_returns_body_unchanged_when_within_budget() {
        let s = HeuristicSummarizer;
        let body = "Short body.";
        assert_eq!(s.summarize(body, 100), body);
    }

    #[test]
    fn heuristic_summarizer_truncates_to_budget_chars() {
        let s = HeuristicSummarizer;
        let body = "a".repeat(500);
        let summarized = s.summarize(&body, 100);
        assert!(
            summarized.chars().count() <= 160,
            "summarized body must be approximately within budget, got {} chars",
            summarized.chars().count()
        );
        assert!(
            summarized.contains("… (summarized"),
            "truncation marker must be present"
        );
    }

    #[test]
    fn heuristic_summarizer_snaps_to_paragraph_boundary() {
        let s = HeuristicSummarizer;
        let body = "First paragraph with enough text to fill the budget.\n\nSecond paragraph that should be cut.";
        let summarized = s.summarize(body, 60);
        assert!(
            summarized.contains("First paragraph"),
            "should keep the first paragraph"
        );
        assert!(
            !summarized.contains("Second paragraph"),
            "should cut at the paragraph boundary"
        );
    }

    #[test]
    fn summarize_source_bodies_preserves_metadata() {
        let bodies = vec![SourceBody {
            index: 5,
            kind: "web".to_string(),
            title: "Test".to_string(),
            path_or_url: "https://example.com".to_string(),
            relevance: "High".to_string(),
            body: "a".repeat(500),
            published_at: None,
        }];
        let summarizer = HeuristicSummarizer;
        let summarized = summarize_source_bodies(&bodies, &summarizer, 100);
        assert_eq!(summarized.len(), 1);
        assert_eq!(summarized[0].index, 5);
        assert_eq!(summarized[0].title, "Test");
        assert_eq!(summarized[0].relevance, "High");
        assert!(summarized[0].body.chars().count() < 200);
    }

    // ── Milestone E-002: chunking + merge tests ───────────────────────────

    #[test]
    fn total_body_chars_sums_all_bodies() {
        let bodies = vec![
            SourceBody {
                index: 1,
                kind: "web".to_string(),
                title: "A".to_string(),
                path_or_url: String::new(),
                relevance: String::new(),
                body: "hello".to_string(),
                published_at: None,
            },
            SourceBody {
                index: 2,
                kind: "web".to_string(),
                title: "B".to_string(),
                path_or_url: String::new(),
                relevance: String::new(),
                body: "world!".to_string(),
                published_at: None,
            },
        ];
        assert_eq!(total_body_chars(&bodies), 11);
    }

    #[test]
    fn chunk_source_bodies_splits_on_budget() {
        let make = |i: usize, body: &str| SourceBody {
            index: i,
            kind: "web".to_string(),
            title: format!("S{i}"),
            path_or_url: String::new(),
            relevance: String::new(),
            body: body.to_string(),
            published_at: None,
        };
        let bodies = vec![
            make(1, &"a".repeat(40)),
            make(2, &"b".repeat(40)),
            make(3, &"c".repeat(40)),
        ];
        let chunks = chunk_source_bodies(&bodies, 50);
        assert_eq!(
            chunks.len(),
            3,
            "each source should be its own chunk at budget 50"
        );
    }

    #[test]
    fn chunk_source_bodies_groups_small_sources() {
        let make = |i: usize, body: &str| SourceBody {
            index: i,
            kind: "web".to_string(),
            title: format!("S{i}"),
            path_or_url: String::new(),
            relevance: String::new(),
            body: body.to_string(),
            published_at: None,
        };
        let bodies = vec![make(1, "small1"), make(2, "small2"), make(3, "small3")];
        let chunks = chunk_source_bodies(&bodies, 100);
        assert_eq!(chunks.len(), 1, "all small sources should fit in one chunk");
    }

    #[test]
    fn merge_chunk_results_concatenates_findings_and_renumbers() {
        let part1 = AnalysisResult {
            summary: "Summary 1".to_string(),
            findings: vec![
                "1. **Headline:** A\n\n**Observation:** obs [#1].\n\n**Analysis:** a.\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** i.".to_string(),
                "2. **Headline:** B\n\n**Observation:** obs [#2].\n\n**Analysis:** b.\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** j.".to_string(),
            ],
            top_implications: vec!["Adopt A.".to_string()],
            cross_references: Vec::new(),
            open_questions: vec!["Q1?".to_string()],
        };
        let part2 = AnalysisResult {
            summary: "Summary 2".to_string(),
            findings: vec![
                "1. **Headline:** C\n\n**Observation:** obs [#3].\n\n**Analysis:** c.\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** k.".to_string(),
            ],
            top_implications: vec!["Adopt A.".to_string(), "Consider C.".to_string()],
            cross_references: Vec::new(),
            open_questions: vec!["Q2?".to_string()],
        };
        let merged = merge_chunk_results(&[part1, part2]);
        assert_eq!(merged.findings.len(), 3);
        // Findings should be renumbered 1, 2, 3.
        assert!(merged.findings[0].starts_with("1. "));
        assert!(merged.findings[1].starts_with("2. "));
        assert!(merged.findings[2].starts_with("3. "));
        // Summaries should be joined.
        assert!(merged.summary.contains("Summary 1"));
        assert!(merged.summary.contains("Summary 2"));
        // Top implications merged and deduped.
        assert_eq!(
            merged.top_implications,
            vec!["Adopt A.".to_string(), "Consider C.".to_string()]
        );
        // Open questions merged.
        assert_eq!(merged.open_questions, vec!["Q1?", "Q2?"]);
    }

    #[test]
    fn merge_chunk_results_dedup_cross_references() {
        let cr = CrossReference {
            path: "src/lib.rs".to_string(),
            relevance: "main".to_string(),
        };
        let part1 = AnalysisResult {
            summary: String::new(),
            findings: Vec::new(),
            top_implications: Vec::new(),
            cross_references: vec![cr.clone()],
            open_questions: Vec::new(),
        };
        let part2 = AnalysisResult {
            summary: String::new(),
            findings: Vec::new(),
            top_implications: Vec::new(),
            cross_references: vec![
                cr.clone(),
                CrossReference {
                    path: "src/main.rs".to_string(),
                    relevance: "entry".to_string(),
                },
            ],
            open_questions: Vec::new(),
        };
        let merged = merge_chunk_results(&[part1, part2]);
        assert_eq!(merged.cross_references.len(), 2);
        assert_eq!(merged.cross_references[0].path, "src/lib.rs");
        assert_eq!(merged.cross_references[1].path, "src/main.rs");
    }

    #[test]
    fn merge_chunk_results_single_part_is_clone() {
        let part = AnalysisResult {
            summary: "Only".to_string(),
            findings: vec!["1. Finding.".to_string()],
            top_implications: vec!["Only implication.".to_string()],
            cross_references: Vec::new(),
            open_questions: Vec::new(),
        };
        let merged = merge_chunk_results(std::slice::from_ref(&part));
        assert_eq!(merged, part);
    }

    #[test]
    fn merge_chunk_results_empty_returns_default() {
        let merged = merge_chunk_results(&[]);
        assert_eq!(merged, AnalysisResult::default());
    }
}

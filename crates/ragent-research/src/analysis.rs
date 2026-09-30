//! Source analysis engine - turns gathered evidence into a structured
//! `AnalysisResult` using an LLM.
//!
//! The default [`LlmAnalysisEngine`] sends a single synthesis prompt to the
//! configured provider/model. The prompt asks for five sections that map
//! directly to the `RESEARCH.md` structure:
//!
//! - Executive Summary
//! - Top 10 Implications
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
use std::sync::OnceLock;

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
    /// Author name extracted from the source's embedded metadata, when
    /// available. Populated by [`build_source_bodies`] from
    /// [`Source::author`]. `None` for local/spec sources and for web sources
    /// that did not expose parseable author information. Surfaced in the
    /// synthesis prompt so the model can credit authors in citations.
    pub author: Option<String>,
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

    /// Validate that the engine's configured provider/model is available
    /// before the expensive gathering phase runs. The default implementation
    /// succeeds; LLM-backed engines override this to check the provider
    /// registry.
    fn validate_provider(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Return an engine instance configured with the supplied research brief.
    ///
    /// Implementations must override this; the trait provides no default
    /// because a silently-ignored brief (the previous no-op default) broke
    /// mock-engine tests that expected the brief to reach
    /// [`analyze_with_outcome`][Self::analyze_with_outcome]. Requiring the
    /// method makes the compiler enforce what the old `unimplemented!()`
    /// panic enforced at runtime.
    fn with_brief(&self, brief: Option<String>) -> Arc<dyn AnalysisEngine>;

    /// Analyze the provided sources and topic, returning structured content
    /// plus an [`AnalysisOutcome`] that tells the caller whether the result
    /// came from a clean LLM parse or from the deterministic fallback path
    /// (FR-005 / T-005).
    ///
    /// The default implementation delegates to `analyze` and
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
/// `crate::session::SynthesizeOutcome` but lives in `analysis.rs` so the
/// engine can return it without a circular dependency on `session.rs`.
/// `session.rs` maps this to `SynthesizeOutcome` when emitting the
/// `SynthesizeResult` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisOutcome {
    /// The model produced a structured [`AnalysisResult`] that parsed cleanly.
    Llm,
    /// The model output was empty or could not be parsed into the required
    /// structure; the deterministic mechanical fallback supplied the
    /// summary/findings (FR-005).
    FallbackEmpty,
    /// The LLM-backed engine returned an error (no key, network failure, ...)
    /// and the mechanical fallback supplied the summary/findings. Surfaced by
    /// `session.rs` mapping an `Err` from `AnalysisEngine::analyze` to
    /// `SynthesizeOutcome::FallbackError`; engines that override
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

    fn with_brief(&self, _brief: Option<String>) -> Arc<dyn AnalysisEngine> {
        // No-op engines cannot use a brief; return a new Arc wrapping the
        // same value so the trait contract is satisfied without mutating state.
        Arc::new(*self)
    }

    fn is_noop_marker(&self) -> bool {
        true
    }

    fn validate_provider(&self) -> anyhow::Result<()> {
        Ok(())
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
    /// Optional research brief used as the mission statement in the synthesis
    /// prompt (FR-004 / T-004). When `Some`, the prompt preamble includes the
    /// brief and instructs the model to treat it as the guiding mission.
    brief: Option<String>,
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
    /// but `analyze` will return an error when called.
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
            brief: None,
        }
    }
    /// Provide an API key for the provider.
    #[must_use]
    pub fn with_api_key(mut self, api_key: Option<String>) -> Self {
        self.api_key = api_key;
        self
    }

    /// Returns the configured API key, if any.
    #[must_use]
    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    /// Override the API base URL. If unset, the engine resolves it from storage
    /// / config / env at analysis time.
    #[must_use]
    pub fn with_base_url(mut self, base_url: Option<String>) -> Self {
        self.base_url = base_url;
        self
    }

    /// Returns the configured base URL, if any.
    #[must_use]
    pub fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
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

    /// Set the research brief that guides synthesis (FR-004 / T-004).
    #[must_use]
    pub fn with_brief(mut self, brief: Option<String>) -> Self {
        self.brief = brief;
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
}

#[async_trait::async_trait]
impl AnalysisEngine for LlmAnalysisEngine {
    fn validate_provider(&self) -> anyhow::Result<()> {
        if self.provider_registry.get(&self.provider_id).is_none() {
            anyhow::bail!(
                "unknown provider '{}' for model '{}'",
                self.provider_id,
                self.model_id
            );
        }
        Ok(())
    }

    fn with_brief(&self, brief: Option<String>) -> Arc<dyn AnalysisEngine> {
        let mut clone = self.clone();
        clone.brief = brief;
        Arc::new(clone)
    }

    async fn analyze(&self, topic: &str, sources: &[SourceBody]) -> anyhow::Result<AnalysisResult> {
        let (result, _) = self.analyze_with_outcome(topic, sources).await?;
        Ok(result)
    }

    /// Override [`AnalysisEngine::analyze_with_outcome`] so the LLM engine
    /// can distinguish a clean parse ([`AnalysisOutcome::Llm`]) from a
    /// malformed response rescued by the mechanical fallback
    /// ([`AnalysisOutcome::FallbackEmpty`]) - FR-005 / T-005. Provider
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
        let summarized: Vec<SourceBody>;
        let prepared: &[SourceBody] = if let Some(budget) = self.source_summary_budget {
            summarized = summarize_source_bodies(sources, &HeuristicSummarizer, budget);
            &summarized
        } else {
            sources
        };

        // E-002: decide whether to chunk.
        let threshold = self.synthesis_chunk_threshold;
        let total = total_body_chars(prepared);
        let needs_chunking = threshold.is_some_and(|t| total > t);

        if !needs_chunking {
            // Single-call path (legacy behavior).
            let text = self.stream_synthesis(topic, prepared).await?;
            return Ok(parse_analysis_response_with_outcome(&text, prepared));
        }

        // Chunked path: split sources, send each chunk, merge results.
        let chunks = chunk_source_bodies(prepared, total / 2 + 1);
        tracing::info!(
            chunks = chunks.len(),
            total_body_chars = total,
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
        let api_key = self.api_key.as_deref().unwrap_or("");
        let client = match provider
            .create_client(api_key, self.base_url.as_deref(), &HashMap::new())
            .await
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "summarize_subject: failed to create client");
                return None;
            }
        };

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

        let mut stream = match client.chat(request).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "summarize_subject: chat request failed");
                return None;
            }
        };
        let mut text = String::new();
        while let Some(event) = stream.next().await {
            match event {
                StreamEvent::TextDelta { text: delta } => text.push_str(&delta),
                StreamEvent::Error { message, .. } => {
                    tracing::warn!(error = %message, "summarize_subject: stream error");
                    return None;
                }
                StreamEvent::Finish { .. } => break,
                _ => {}
            }
        }
        match parse_subject_summary(&text) {
            Some(result) => Some(result),
            None => {
                tracing::debug!(
                    text_len = text.len(),
                    "summarize_subject: could not parse model output"
                );
                None
            }
        }
    }

    /// Issue the synthesis request to the provider and return the raw model
    /// text. Shared by `AnalysisEngine::analyze` (which parses strictly)
    /// and [`AnalysisEngine::analyze_with_outcome`] (which parses with
    /// fallback detection) so the streaming code lives in one place.
    async fn stream_synthesis(
        &self,
        topic: &str,
        sources: &[SourceBody],
    ) -> anyhow::Result<String> {
        let prompt = SynthesisPromptBuilder::new(topic)
            .sources(sources)
            .output_format(self.output_format.unwrap_or(OutputFormat::Report))
            .brief(self.brief.as_deref())
            .build();
        // T-008 / FR-009: allow a configurable analysis persona. When
        // `config.persona` is supplied via `ragent.json`
        // (`research.analysis_persona`), it overrides the default
        // "careful research analyst" system message. The default persona is
        // preserved when `persona` is `None`, so the legacy behavior is
        // unchanged for callers that don't wire the new config in.
        let system_persona: Option<String> = self.persona.clone().or_else(|| {
            Some(
                "You are a careful research analyst. Read the provided sources and produce a structured markdown analysis. Use only the evidence in the sources; do not invent facts."
                    .to_string(),
            )
        });
        self.complete_raw(&prompt, system_persona.as_deref(), 8192)
            .await
    }

    /// Summarize a single webpage body for use as a source/vault entry
    /// (FR-002 / T-012 / T-013). The prompt asks for a concise, factual summary
    /// that preserves claims and numbers relevant to a research report.
    ///
    /// # Errors
    ///
    /// Returns an error when the provider is unavailable or the stream fails.
    pub async fn summarize_page(
        &self,
        url: &str,
        body: &str,
        max_tokens: u32,
    ) -> anyhow::Result<String> {
        let prompt = format!(
            "Summarize the following webpage content in a concise paragraph (at most a few sentences). \
             Keep key facts, numbers, named entities, and concrete claims that would be useful for a research report. \
             Omit navigation, ads, footer boilerplate, and unrelated asides. \
             Do not introduce information not present in the text. \
             URL: {url}\n\nContent:\n\n{body}"
        );
        self.complete_raw(
            &prompt,
            Some("You are a precise research summarizer."),
            max_tokens,
        )
        .await
    }

    /// Send a raw prompt to the wired provider/model and return the full
    /// response text. This is the shared low-level completion path for the
    /// synthesis stream and the `/research create` concept-extraction step:
    /// it resolves the provider client from the registry, streams the reply,
    /// and accumulates the text deltas into one string.
    ///
    /// # Errors
    ///
    /// Returns an error when the provider is unknown, the client cannot be
    /// created, or the stream reports a provider-level error.
    pub async fn complete_raw(
        &self,
        prompt: &str,
        system: Option<&str>,
        max_tokens: u32,
    ) -> anyhow::Result<String> {
        let provider = self
            .provider_registry
            .get(&self.provider_id)
            .ok_or_else(|| anyhow::anyhow!("unknown provider '{}'", self.provider_id))?;
        let api_key = self.api_key.as_deref().unwrap_or("");
        let client = provider
            .create_client(api_key, self.base_url.as_deref(), &HashMap::new())
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "failed to create LLM client for {}/{}: {e}",
                    self.provider_id,
                    self.model_id
                )
            })?;

        let request = ChatRequest {
            model: self.model_id.clone(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text(prompt.to_string()),
            }]),
            tools: Arc::new(vec![]),
            temperature: Some(0.2),
            top_p: Some(1.0),
            max_tokens: Some(max_tokens),
            system: system.map(std::sync::Arc::from),
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
        // `get` returns None when the span is invalid (e.g. `}` appears
        // before `{`), instead of panicking on an inverted slice index.
        serde_json::from_str::<SubjectSummary>(trimmed.get(start..=end)?).ok()
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
            author: src.author().map(str::to_string),
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
/// 5. Append a `\n\n... (summarized - see full source for remaining content)`
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
        window.push_str("\n\n... (summarized - see full source for remaining content)");
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

    // Merge findings: concatenate, then renumber. `parts` is consumed by
    // reference only, but the findings are moved out via `clone()` on the
    // per-part vectors - the previous double clone (`clone()` + `extend`)
    // collapsed into a single clone per part.
    let mut all_findings: Vec<String> = Vec::new();
    for part in parts {
        all_findings.extend(part.findings.iter().cloned());
    }
    let findings = renumber_findings(&all_findings);

    // Merge cross-references: dedup by path.
    let mut seen_paths: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut cross_references = Vec::new();
    for part in parts {
        for cr in &part.cross_references {
            if seen_paths.insert(cr.path.as_str()) {
                cross_references.push(cr.clone());
            }
        }
    }

    // Merge top implications: concatenate, dedup exact matches, preserve rank.
    let mut seen_implications: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut top_implications = Vec::new();
    for part in parts {
        for imp in &part.top_implications {
            if seen_implications.insert(imp.as_str()) {
                top_implications.push(imp.clone());
            }
        }
    }

    // Merge open questions: concatenate, dedup exact matches.
    let mut seen_questions: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut open_questions = Vec::new();
    for part in parts {
        for q in &part.open_questions {
            if seen_questions.insert(q.as_str()) {
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

/// Renumber the `1.`, `2.`, ... prefixes in a list of findings so they are
/// contiguous starting from 1. Also handles bold-labelled findings such as
/// `**Finding 1:**` or `**1.**` prefixes. Findings without a numeric prefix
/// are left unchanged (Milestone E-002, FUNC-ANL-08).
fn renumber_findings(findings: &[String]) -> Vec<String> {
    static NUM_RE: OnceLock<Regex> = OnceLock::new();
    static BOLD_NUM_RE: OnceLock<Regex> = OnceLock::new();
    // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
    let num_re = NUM_RE.get_or_init(|| Regex::new(r"^(\d+)\.\s*").expect("valid renumber regex"));
    let bold_num_re = BOLD_NUM_RE.get_or_init(|| {
        // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
        Regex::new(r"^\*\*(?:Finding\s*)?(\d+)[.:]?\*\*\s*").expect("valid bold renumber regex")
    });
    findings
        .iter()
        .enumerate()
        .map(|(i, finding)| {
            let replacement = format!("{}. ", i + 1);
            if num_re.is_match(finding) {
                num_re.replace(finding, replacement.as_str()).to_string()
            } else if bold_num_re.is_match(finding) {
                bold_num_re
                    .replace(finding, format!("**{}.** ", i + 1))
                    .to_string()
            } else {
                finding.clone()
            }
        })
        .collect()
}

/// Order findings most-relevant-first and cap them to `max_findings`
/// (spec researchmax; FR-003, FR-010, FR-020, FR-022).
///
/// Findings are ranked with the shared reverse-relevance helper (highest cited
/// [`Source::relevance_rank`] first; ties break to cited count, then the
/// model's original order). The retained findings are truncated to
/// `max_findings` and renumbered contiguously from 1 with the existing
/// `renumber_findings` pass so `Finding N` labels stay sequential.
///
/// `max_findings == 0` means "unbounded" (FR-016) and applies no truncation. At
/// least one finding is always retained when any were supplied (FR-020), and
/// the list is never padded when fewer are available (FR-022).
#[must_use]
pub fn cap_findings_to_limit(
    findings: Vec<String>,
    max_findings: usize,
    sources: &[Source],
) -> Vec<String> {
    if findings.is_empty() {
        return findings;
    }
    let mut ordered = crate::limits::rank_entries_by_reverse_relevance(
        findings,
        String::as_str,
        crate::limits::source_rank_lookup(sources),
    );
    if let Some(cap) = crate::limits::effective_limit(max_findings) {
        // A positive limit truncates to at least one entry (never zero) when
        // findings exist (FR-020); `0` resolves to `None` and applies no
        // truncation because it means "unbounded" (FR-016).
        ordered.truncate(cap);
    }
    renumber_findings(&ordered)
}

#[cfg(test)]
#[path = "../tests/inline/analysis_tests.rs"]
mod tests;

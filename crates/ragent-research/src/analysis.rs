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
use chrono::{DateTime, Utc};
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

    /// Override the `system` message persona (FR-009 / T-008). When set, the
    /// supplied string replaces the default "careful research analyst" system
    /// prompt verbatim. Pass `None` (or never call this) to keep the default.
    pub fn with_persona(mut self, persona: Option<String>) -> Self {
        self.persona = persona;
        self
    }
}

#[async_trait::async_trait]
impl AnalysisEngine for LlmAnalysisEngine {
    async fn analyze(&self, topic: &str, sources: &[SourceBody]) -> anyhow::Result<AnalysisResult> {
        let text = self.stream_synthesis(topic, sources).await?;
        Ok(parse_analysis_response(&text))
    }

    /// Override [`AnalysisEngine::analyze_with_outcome`] so the LLM engine
    /// can distinguish a clean parse ([`AnalysisOutcome::Llm`]) from a
    /// malformed response rescued by the mechanical fallback
    /// ([`AnalysisOutcome::FallbackEmpty`]) — FR-005 / T-005. Provider
    /// errors still surface as `Err`, which `session.rs` maps to
    /// [`crate::session::SynthesizeOutcome::FallbackError`].
    async fn analyze_with_outcome(
        &self,
        topic: &str,
        sources: &[SourceBody],
    ) -> anyhow::Result<(AnalysisResult, AnalysisOutcome)> {
        let text = self.stream_synthesis(topic, sources).await?;
        Ok(parse_analysis_response_with_outcome(&text, sources))
    }
}

impl LlmAnalysisEngine {
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

        let prompt = build_synthesis_prompt(topic, sources);
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

/// Configuration knobs for the synthesis prompt builder.
///
/// All fields are `Option`/default so the default-constructed builder
/// reproduces the legacy `build_synthesis_prompt(topic, sources)` byte stream
/// exactly. Later tasks (T-003..T-008) extend this with recency instructions,
/// the **Sources Cited / Date Spread** paragraph, few-shot exemplars, and an
/// optional persona.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // populated by T-003..T-008; default path uses none of it
pub(crate) struct SynthesisPromptConfig {
    /// Optional audience/domain framing appended to the task preamble
    /// (FR-009 / Finding 12). `None` preserves the legacy preamble.
    pub audience_scope: Option<String>,
    /// When `true`, append the recency-weighting rule block (FR-004 / T-004).
    pub recency_rule: bool,
    /// When `true`, require the fifth **Sources Cited / Date Spread**
    /// paragraph in every finding (FR-003 / T-003).
    pub date_spread_paragraph: bool,
    /// Optional few-shot exemplar findings appended after the template
    /// instructions (FR-008 / T-007). Each entry is one finding body.
    pub few_shot_examples: Vec<String>,
    /// Optional override for the `system` message persona (FR-009 / T-008).
    pub persona: Option<String>,
    /// Optional template body merged with the structured synthesis
    /// requirements (FR-007 / T-006).
    pub template_body: Option<String>,
}

/// Versioned, composable synthesis-prompt builder.
///
/// Introduced by `researchprompt` T-002 to replace the monolithic
/// `build_synthesis_prompt` string concatenation with a builder whose parts
/// (preamble, output-template, recency rule, few-shot, sources block) can be
/// extended independently. The legacy free function is preserved as a thin
/// wrapper that calls `SynthesisPromptBuilder::new(topic).sources(sources)
/// .build()` so existing callers — including `LlmAnalysisEngine::analyze` —
/// are unchanged.
///
/// ## Output stability
///
/// With the default [`SynthesisPromptConfig`], `build()` returns the exact
/// bytes the legacy `build_synthesis_prompt` returned. Tasks T-003..T-008 opt
/// in to additional prompt sections via the config; they never alter the
/// default output.
#[derive(Debug, Clone)]
#[allow(dead_code)] // setters exercised by T-003..T-008
pub(crate) struct SynthesisPromptBuilder<'a> {
    topic: &'a str,
    sources: &'a [SourceBody],
    config: SynthesisPromptConfig,
}

impl<'a> SynthesisPromptBuilder<'a> {
    /// Begin building a synthesis prompt for `topic`.
    pub fn new(topic: &'a str) -> Self {
        Self {
            topic,
            sources: &[],
            config: SynthesisPromptConfig::default(),
        }
    }

    /// Attach the captured source corpus. Required before [`build`].
    pub fn sources(mut self, sources: &'a [SourceBody]) -> Self {
        self.sources = sources;
        self
    }

    /// Attach the full prompt configuration (T-003..T-008 knobs).
    #[allow(dead_code)] // exercised by T-003..T-008
    pub fn config(mut self, config: SynthesisPromptConfig) -> Self {
        self.config = config;
        self
    }

    /// Borrow the active config immutably.
    #[allow(dead_code)] // exercised by T-003..T-008
    pub fn cfg(&self) -> &SynthesisPromptConfig {
        &self.config
    }

    /// Produce the final prompt string.
    pub fn build(&self) -> String {
        let mut prompt = String::new();
        prompt.push_str(&render_preamble(self.topic, &self.config));
        if self.sources.is_empty() {
            prompt.push_str(
                "No sources were captured. Write a brief note that no sources were available and suggest refining the topic.\n",
            );
        } else {
            prompt.push_str(&format!(
                "{count} source(s) were captured. Read them and produce a structured markdown response with exactly these four top-level sections (in this order):\n\n",
                count = self.sources.len()
            ));
            prompt.push_str(&render_output_template(&self.config));
            prompt.push_str(&render_sources_block(
                self.sources,
                self.config.date_spread_paragraph,
            ));
        }
        prompt.push_str(&render_closing(&self.config));
        prompt
    }
}

/// Render the task preamble. With the default config this is byte-identical to
/// the legacy opening of `build_synthesis_prompt`.
fn render_preamble(topic: &str, _config: &SynthesisPromptConfig) -> String {
    format!(
        "You are writing the analysis section of a research report for the topic:\n\n{topic}\n\n"
    )
}

/// Render the four mandatory top-level section instructions plus the
/// per-finding labeled-paragraph template. With the default config this is
/// byte-identical to the legacy middle of `build_synthesis_prompt`.
///
/// Tasks T-003 (date-spread paragraph) and T-004 (recency rule) extend this
/// function by gating new instruction blocks on
/// `config.date_spread_paragraph` and `config.recency_rule` respectively; the
/// default (both `false`) path is unchanged.
fn render_output_template(config: &SynthesisPromptConfig) -> String {
    let mut out = String::new();
    out.push_str("## Summary\n");
    out.push_str(
        "A concise one-paragraph summary of what the sources collectively say about the topic.\n\n",
    );
    out.push_str("## Findings\n");
    out.push_str(
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
    // T-003 (FR-003): require a fifth **Sources Cited / Date Spread**
    // paragraph in every finding. Gated on `config.date_spread_paragraph` so
    // the default-config output stays byte-identical to the legacy prompt.
    if config.date_spread_paragraph {
        out.push_str(
            "In addition to the four required paragraphs above, every finding must end with a fifth paragraph labeled:\n\n\
            **Sources Cited / Date Spread:**\n\
            List every `[#N]` citation used in the finding, then report the earliest and latest publication dates among those cited web sources (use the `Published` line in each source header below; write `undated` when a cited source has no publication date). Add one sentence explaining how the date range — and the recency of the evidence — affects the finding's confidence, relevance, or conclusions. If every cited source is undated, say so explicitly and explain the implication.\n\n\
            Example: `**Sources Cited / Date Spread:** [#3] [#7] — published 2024-01-05..2026-04-07; the finding relies on 2026 sources, so recency weighting increases confidence in current behavior.`\n\n"
        );
    }
    // T-004 (FR-004): recency-weighting rule. Gated on `config.recency_rule`
    // so the default-config output stays byte-identical to the legacy prompt.
    if config.recency_rule {
        out.push_str(
            "Recency-weighting rule (apply to every finding):\n\
            - When two cited web sources disagree, prefer the more recently published source unless the older source is a primary/peer-reviewed publication and the newer one is not.\n\
            - In the **Analysis** paragraph, explicitly note any conflict between older and newer sources and state which view you are following and why.\n\
            - In the **Sources Cited / Date Spread** paragraph (when required), note when a finding relies primarily on older sources and explain how that affects confidence.\n\
            - When ranking evidence quality, prefer sources with clear publication dates and structured metadata; down-weight anonymous forums and undated pages unless they provide unique empirical signal.\n\n"
        );
    }
    out.push_str("## In-Project Cross-References\n");
    out.push_str(
                "A bullet list of relevant in-project files, formatted as `* `path` — note`. Only include files that are actually mentioned in the local sources.\n\n"
            );
    out.push_str("## Open Questions\n");
    out.push_str(
                "A bullet list of gaps, uncertainties, or follow-up questions that remain after reading the sources.\n\n"
            );
    // Allow T-006 to append template-merge guidance here without touching the
    // default path. No-op for the default config.
    if let Some(template) = &config.template_body {
        // FR-007 / T-006: when a `--template` is supplied, instruct the model
        // to populate the template's placeholder sections IN ADDITION to the
        // four/five required finding paragraphs. The template never replaces
        // the structured synthesis requirements — it only adds extra sections
        // or tone guidance. Keep this instruction short so it does not blow
        // up the context window when the template body is large; the full
        // template body is not echoed here (the caller wires it into the
        // document assembly separately).
        let _ = template; // referenced for future expansion
        out.push_str(
            "A research template with extra placeholder sections is in effect. \
            Populate every placeholder the template defines (for example \
            {{title}}, {{topic}}, {{date}}, or any custom `{{section}}` markers), \
            but do NOT let the template replace the required Findings structure: \
            every finding must still contain the four required labeled paragraphs \
            (Observation, Analysis, Cross-reference / Dependencies, Implication) \
            and, when requested, the fifth **Sources Cited / Date Spread** \
            paragraph. Treat template sections as additional output, not as a \
            substitute for the structured findings.\n\n",
        );
    }
    // T-007 (FR-008): append few-shot exemplar findings so the model can
    // calibrate the exact label structure, `[#N]` citations, and (when
    // enabled) the **Sources Cited / Date Spread** paragraph. Gated on
    // `config.few_shot_examples` being non-empty so the default-config output
    // stays byte-identical to the legacy prompt. Each entry is one finding
    // body; we render up to two to keep the context-window cost low.
    if !config.few_shot_examples.is_empty() {
        out.push_str(
            "Few-shot exemplar findings (for format calibration only — do NOT \
            copy their content into your answer; derive findings from the \
            supplied sources):\n\n",
        );
        for (idx, example) in config.few_shot_examples.iter().take(2).enumerate() {
            out.push_str(&format!("### Exemplar Finding {}\n\n", idx + 1));
            out.push_str(example.trim());
            if !example.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
    }
    out
}

/// Render the per-source `### Sources` block.
///
/// With the default config (`include_published = false`) this is
/// byte-identical to the legacy tail of `build_synthesis_prompt`. When T-003
/// enables the **Sources Cited / Date Spread** paragraph, the caller passes
/// `include_published = true` so each web source header gains a `Published`
/// line the model can quote in its date-spread analysis.
fn render_sources_block(sources: &[SourceBody], include_published: bool) -> String {
    let mut out = String::new();
    out.push_str("---\n\n### Sources\n\n");
    for src in sources {
        let published_line = if include_published {
            match src.published_at {
                Some(dt) => format!("\nPublished (UTC): {d}", d = dt.format("%Y-%m-%d")),
                None => "\nPublished (UTC): undated".to_string(),
            }
        } else {
            String::new()
        };
        out.push_str(&format!(
            "#### Source [#{index}] ({kind}) {title}\nPath/URL: {path}{published}\nRelevance: {rel}\n```text\n{body}\n```\n\n",
            index = src.index,
            kind = src.kind,
            title = src.title,
            path = src.path_or_url,
            published = published_line,
            rel = if src.relevance.is_empty() {
                "—".to_string()
            } else {
                src.relevance.clone()
            },
            body = truncate_body(&src.body, 4000),
        ));
    }
    out
}

/// Render the closing instruction line. With the default config this is
/// byte-identical to the legacy final lines of `build_synthesis_prompt`.
fn render_closing(_config: &SynthesisPromptConfig) -> String {
    let mut out = String::new();
    out.push_str(
        "\nNow produce only the four sections above. Do not include a title or any other preamble. ",
    );
    out.push_str(
        "Within Findings, always include the four required paragraphs (Observation, Analysis, ",
    );
    out.push_str(
        "Cross-reference / Dependencies, Implication) and feel free to add more labeled paragraphs if the sources support it.",
    );
    out
}

/// Build the synthesis prompt. Sources are listed with their index so the model
/// can cite them as `[#N]`.
///
/// This free function is preserved as the stable, backward-compatible entry
/// point. It delegates to [`SynthesisPromptBuilder`] with the default config,
/// so its output is byte-identical to the pre-refactor implementation. Callers
/// that need the extended knobs (T-003..T-008) should use the builder directly.
fn build_synthesis_prompt(topic: &str, sources: &[SourceBody]) -> String {
    SynthesisPromptBuilder::new(topic).sources(sources).build()
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

/// Parse the LLM response into an [`AnalysisResult`] paired with an
/// [`AnalysisOutcome`] (FR-005 / T-005).
///
/// Runs [`parse_analysis_response`] first. If the result is malformed
/// (see [`is_malformed_analysis_result`]), the mechanical fallback
/// ([`mechanical_fallback_findings`] + a placeholder summary) rescues the
/// raw text into structured findings and the outcome is
/// [`AnalysisOutcome::FallbackEmpty`]; otherwise the outcome is
/// [`AnalysisOutcome::Llm`]. Provider-level errors are surfaced by
/// [`LlmAnalysisEngine::analyze_with_outcome`] as `Err`, which `session.rs`
/// maps to [`crate::session::SynthesizeOutcome::FallbackError`].
fn parse_analysis_response_with_outcome(
    text: &str,
    sources: &[SourceBody],
) -> (AnalysisResult, AnalysisOutcome) {
    let parsed = parse_analysis_response(text);
    if is_malformed_analysis_result(&parsed) {
        let mut rescued = AnalysisResult::default();
        rescued.findings = mechanical_fallback_findings(text);
        rescued.summary = if rescued.findings.is_empty() {
            "(the model response could not be parsed into structured findings; \
             see the raw response below)"
                .to_string()
        } else {
            "(the model response was malformed; the following findings were \
             extracted mechanically and may be incomplete)"
                .to_string()
        };
        (rescued, AnalysisOutcome::FallbackEmpty)
    } else {
        // FR-010 / T-009: validate citations and dates even on a "clean" parse.
        // Out-of-range `[#N]` citations and unsupported date claims are replaced
        // inline with warning placeholders so hallucinated evidence is visible
        // rather than silently propagated.
        let mut validated = parsed;
        let warnings = validate_citations_and_dates(&mut validated.findings, sources);
        if !warnings.is_empty() {
            for w in &warnings {
                tracing::warn!(warning = %w, "research: citation/date validation");
            }
        }
        (validated, AnalysisOutcome::Llm)
    }
}

/// Validate every `[#N]` citation and claimed publication date in `findings`
/// against the actual `sources` corpus (FR-010 / T-009).
///
/// Mutates `findings` in place:
/// - Out-of-range `[#N]` citations (N == 0 or N > `sources.len()`) are
///   rewritten to `[#N?] (out of range — not in source list)`.
/// - Explicit publication dates in a **Sources Cited / Date Spread** paragraph
///   that do not match any cited source's `published_at` are rewritten to
///   `(unsupported date)`.
///
/// Returns a list of human-readable warning strings (one per invalid claim)
/// so the caller can log them. Findings that pass validation are left
/// untouched.
fn validate_citations_and_dates(findings: &mut [String], sources: &[SourceBody]) -> Vec<String> {
    let mut warnings = Vec::new();
    let citation_re = Regex::new(r"\[#(\d+)\]").expect("valid citation regex");
    // Match `published YYYY-MM-DD` or a bare `YYYY-MM-DD` inside a
    // **Sources Cited / Date Spread** paragraph. We keep this conservative
    // so we don't rewrite dates that appear in the Observation/Analysis
    // prose (which may legitimately reference unrelated dates).
    let date_re = Regex::new(r"(\d{4}-\d{2}-\d{2})").expect("valid date regex");
    let valid_dates: Vec<String> = sources
        .iter()
        .filter_map(|s| s.published_at.map(|dt| dt.format("%Y-%m-%d").to_string()))
        .collect();

    for finding in findings.iter_mut() {
        // ── Citation range validation ──────────────────────────────────────
        let mut new_finding = String::with_capacity(finding.len());
        let mut last_end = 0;
        for cap in citation_re.captures_iter(finding) {
            let m = cap.get(0).expect("full match");
            new_finding.push_str(&finding[last_end..m.start()]);
            let n: usize = cap[1].parse().unwrap_or(0);
            if n == 0 || n > sources.len() {
                let replacement = format!("[#{n}?] (out of range — not in source list)");
                new_finding.push_str(&replacement);
                warnings.push(format!(
                    "finding cites [#{n}] but only {} source(s) were captured",
                    sources.len()
                ));
            } else {
                new_finding.push_str(m.as_str());
            }
            last_end = m.end();
        }
        new_finding.push_str(&finding[last_end..]);
        *finding = new_finding;

        // ── Date claim validation (only inside the Sources Cited / Date
        // Spread paragraph, to avoid rewriting prose dates) ─────────────────
        if let Some(spread_start) = finding.find("**Sources Cited / Date Spread:**") {
            let spread = &finding[spread_start..];
            let mut validated_spread = String::with_capacity(spread.len());
            let mut last_end = 0;
            for cap in date_re.captures_iter(spread) {
                let m = cap.get(0).expect("full match");
                validated_spread.push_str(&spread[last_end..m.start()]);
                let claimed = &cap[1];
                if valid_dates.iter().any(|d| d == claimed) {
                    validated_spread.push_str(claimed);
                } else {
                    validated_spread.push_str("(unsupported date)");
                    warnings.push(format!(
                        "finding claims publication date {claimed} which is not among the captured sources' publication dates"
                    ));
                }
                last_end = m.end();
            }
            validated_spread.push_str(&spread[last_end..]);
            let prefix = &finding[..spread_start];
            *finding = format!("{prefix}{validated_spread}");
        }
    }
    warnings
}

/// Return `true` when `result` should be treated as a malformed LLM response
/// (FR-005): empty findings, any finding missing one of the four required
/// bold labels, or any finding that contains no `[#N]` citation.
fn is_malformed_analysis_result(result: &AnalysisResult) -> bool {
    if result.findings.is_empty() {
        return true;
    }
    let required = [
        "**Observation:**",
        "**Analysis:**",
        "**Cross-reference / Dependencies:**",
        "**Implication:**",
    ];
    let citation_re = Regex::new(r"\[#\d+\]").expect("valid citation regex");
    for finding in &result.findings {
        if !required.iter().all(|label| finding.contains(label)) {
            return true;
        }
        if !citation_re.is_match(finding) {
            return true;
        }
    }
    false
}

/// Deterministic mechanical extraction (FR-005) that turns a raw model
/// response into a list of findings, each carrying the four required bold
/// labels. Missing labels are inserted as placeholders; existing labels and
/// any `[#N]` citations are preserved verbatim.
///
/// **Non-empty guarantee (FR-011 / T-010):** this function ALWAYS returns at
/// least one finding. When the raw response has no extractable candidate
/// findings, a single placeholder finding is emitted whose **Observation**
/// paragraph reads "(findings could not be structured — see below)" and
/// includes the raw model output in a fenced code block so the research
/// item remains usable. Callers can rely on `findings.is_empty()` never
/// being true for the returned `Vec`.
///
/// Strategy:
/// 1. If the response contains a `## Findings` section, split its numbered
///    list items; each becomes a candidate finding.
/// 2. Otherwise, fall back to splitting the whole response on blank-line
///    paragraphs (or, if that yields a single blob, wrap the whole text as
///    one finding).
/// 3. For each candidate, ensure the four required labels are present,
///    inserting `**Label:** (missing)` placeholders for any that are absent.
/// 4. If no candidate text could be extracted, return a single placeholder
///    finding that quotes the raw response (truncated) so the research item
///    remains usable.
fn mechanical_fallback_findings(text: &str) -> Vec<String> {
    let candidates = extract_candidate_findings(text);
    let required = [
        "**Observation:**",
        "**Analysis:**",
        "**Cross-reference / Dependencies:**",
        "**Implication:**",
    ];
    let mut findings = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let mut normalized = candidate.trim().to_string();
        for label in required {
            if !normalized.contains(label) {
                let placeholder = format!("\n\n{label} (missing)");
                normalized.push_str(&placeholder);
            }
        }
        findings.push(normalized);
    }
    if findings.is_empty() {
        // FR-011 / T-010: the model returned fewer than one valid finding.
        // The fallback takes precedence — emit a single placeholder finding
        // so RESEARCH.md is never left with an empty Findings section. The
        // raw model output is preserved in a fenced code block for manual
        // review.
        let raw = text.trim();
        if raw.is_empty() {
            findings.push(
                "**Observation:** (findings could not be structured — see below)\n\n\
                 (no model response was returned)\n\n\
                 **Analysis:** (missing)\n\n\
                 **Cross-reference / Dependencies:** No direct dependencies.\n\n\
                 **Implication:** Re-run `/research create` with a configured \
                 model; the model returned no content to analyze."
                    .to_string(),
            );
        } else {
            let truncated = truncate_body(raw, 2000);
            findings.push(format!(
                "**Observation:** (findings could not be structured — see below)\n\n\
                 The raw model response (truncated) is preserved for manual review:\n\n\
                 ```text\n{raw}\n```\n\n\
                 **Analysis:** (extracted mechanically — the model output did not \
                 contain the four required labeled paragraphs)\n\n\
                 **Cross-reference / Dependencies:** No direct dependencies.\n\n\
                 **Implication:** Re-run `/research create` or refine the topic; \
                 the raw model output is preserved above for manual review.",
                raw = truncated,
            ));
        }
    }
    findings
}

/// Extract candidate finding bodies from a raw model response.
///
/// Prefers numbered items found under a `## Findings` heading; falls back
/// to numbered items anywhere in the response; finally falls back to
/// blank-line-separated paragraphs.
fn extract_candidate_findings(text: &str) -> Vec<String> {
    // 1. Prefer items under a `## Findings` heading.
    let findings_body = split_sections(text)
        .into_iter()
        .find(|(title, _)| title.to_lowercase() == "findings")
        .map(|(_, body)| body);
    if let Some(body) = findings_body {
        let items = parse_numbered_list(&body);
        if !items.is_empty() {
            return items;
        }
        // `## Findings` present but no numbered items — fall through to
        // whole-response strategies.
    }
    // 2. Numbered items anywhere in the response.
    let anywhere = parse_numbered_list(text);
    if !anywhere.is_empty() {
        return anywhere;
    }
    // 3. Blank-line-separated paragraphs (skip headings and rule lines).
    let paragraphs: Vec<String> = text
        .trim()
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty() && !p.starts_with('#') && !p.starts_with("---"))
        .map(str::to_string)
        .collect();
    if paragraphs.is_empty() {
        Vec::new()
    } else {
        paragraphs
    }
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
            published_at: src.published_at(),
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
    fn builder_default_is_byte_identical_to_legacy() {
        // The default-config builder must produce the same bytes the legacy
        // build_synthesis_prompt produced — this is the backward-compat
        // guarantee that lets existing callers upgrade without behavior change.
        let sources = vec![src_body(1, None), src_body(2, None)];
        let legacy = build_synthesis_prompt("topic", &sources);
        let builder = SynthesisPromptBuilder::new("topic")
            .sources(&sources)
            .build();
        assert_eq!(legacy, builder);
    }

    #[test]
    fn builder_emits_four_required_labels() {
        let sources = vec![src_body(1, None)];
        let prompt = SynthesisPromptBuilder::new("topic")
            .sources(&sources)
            .build();
        assert!(prompt.contains("**Observation:**"));
        assert!(prompt.contains("**Analysis:**"));
        assert!(prompt.contains("**Cross-reference / Dependencies:**"));
        assert!(prompt.contains("**Implication:**"));
    }

    #[test]
    fn builder_date_spread_paragraph_adds_fifth_label_and_published_line() {
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
            few_shot_examples: vec![exemplar.clone()],
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
        let text = "## Summary\n\nA summary.\n\n## Findings\n\n\
             1. **Observation:** obs [#1].\n\n**Analysis:** a.\n\n\
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
        assert!(result.findings[0].contains("**Cross-reference / Dependencies:**"));
        assert!(result.findings[0].contains("**Implication:**"));
    }

    #[test]
    fn parse_with_outcome_no_findings_section_falls_back() {
        // A response that only has a summary (no ## Findings) is malformed.
        let text = "## Summary\n\nOnly a summary, no findings section.\n";
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
            "## Summary\n\nonly summary",
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
}

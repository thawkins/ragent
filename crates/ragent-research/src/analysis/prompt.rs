//! Synthesis prompt construction — build the LLM prompt that asks for the
//! six required sections (Executive Summary, Top 10 Implications, Findings,
//! In-Project Cross-References, Open Questions).
//!
//! These helpers were previously inline in `analysis.rs`.

use super::SourceBody;
use super::parser::truncate_body;
use crate::run_config::OutputFormat;

#[derive(Debug, Clone, Default)]
// reason: only consumed inside this crate - `pub` here never escapes the crate.
#[allow(unreachable_pub)]
pub struct SynthesisPromptConfig {
    /// Optional audience/domain framing appended to the task preamble
    /// (FR-009 / Finding 12). `None` preserves the legacy preamble.
    #[allow(dead_code)] // reserved for T-008 persona/audience wiring; not yet read
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
    #[allow(dead_code)] // reserved for T-008 persona/audience wiring; not yet read
    pub persona: Option<String>,
    /// Optional template body merged with the structured synthesis
    /// requirements (FR-007 / T-006).
    pub template_body: Option<String>,
    /// Optional research brief used as the mission statement in the preamble
    /// (FR-004 / T-004). When `Some`, the prompt includes the brief and
    /// instructs the model to follow it instead of the raw topic.
    pub brief: Option<String>,
    /// Optional output format used to specialize the synthesis template
    /// (FR-012 / specs/imradreport).
    pub output_format: Option<OutputFormat>,
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
// reason: only consumed inside this crate - `pub` here never escapes the crate.
#[allow(unreachable_pub)]
pub struct SynthesisPromptBuilder<'a> {
    topic: &'a str,
    sources: &'a [SourceBody],
    config: SynthesisPromptConfig,
}

impl<'a> SynthesisPromptBuilder<'a> {
    /// Begin building a synthesis prompt for `topic`.
    // reason: only consumed inside this crate - `pub` here never escapes the crate.
    #[allow(unreachable_pub)]
    pub fn new(topic: &'a str) -> Self {
        Self {
            topic,
            sources: &[],
            config: SynthesisPromptConfig::default(),
        }
    }

    /// Attach the captured source corpus. Required before [`build`].
    // reason: only consumed inside this crate - `pub` here never escapes the crate.
    #[allow(unreachable_pub)]
    pub const fn sources(mut self, sources: &'a [SourceBody]) -> Self {
        self.sources = sources;
        self
    }

    /// Attach the full prompt configuration (T-003..T-008 knobs).
    #[allow(dead_code)]
    // reserved for T-003..T-008 prompt configuration wiring
    // reason: only consumed inside this crate - `pub` here never escapes the crate.
    #[allow(unreachable_pub)]
    pub fn config(mut self, config: SynthesisPromptConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the output artifact for this prompt (FR-012).
    // reason: only consumed inside this crate - `pub` here never escapes the crate.
    #[allow(unreachable_pub)]
    pub const fn output_format(mut self, fmt: OutputFormat) -> Self {
        self.config.output_format = Some(fmt);
        self
    }

    /// Set the research brief that guides the synthesis (FR-004 / T-004).
    // reason: only consumed inside this crate - `pub` here never escapes the crate.
    #[allow(unreachable_pub)]
    pub fn brief(mut self, brief: Option<&'a str>) -> Self {
        self.config.brief = brief.map(String::from);
        self
    }

    /// Borrow the active config immutably.
    #[allow(dead_code)]
    // reserved for T-003..T-008 prompt configuration wiring
    // reason: only consumed inside this crate - `pub` here never escapes the crate.
    #[allow(unreachable_pub)]
    pub const fn cfg(&self) -> &SynthesisPromptConfig {
        &self.config
    }

    /// Produce the final prompt string.
    // reason: only consumed inside this crate - `pub` here never escapes the crate.
    #[allow(unreachable_pub)]
    pub fn build(&self) -> String {
        let mut prompt = String::new();
        prompt.push_str(&render_preamble(self.topic, &self.config));
        if self.sources.is_empty() {
            prompt.push_str(
                "No sources were captured. Write a brief note that no sources were available and suggest refining the topic.\n",
            );
        } else {
            prompt.push_str(&format!(
                "{count} source(s) were captured. Read them and produce a structured markdown response with exactly these five top-level sections (in this order):\n\n",
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
/// the legacy opening of `build_synthesis_prompt`. When a research brief is
/// supplied, the preamble includes it as the guiding mission statement
/// (FR-004 / T-004).
// reason: only consumed inside this crate - `pub` here never escapes the crate.
#[allow(unreachable_pub)]
pub fn render_preamble(topic: &str, config: &SynthesisPromptConfig) -> String {
    let mut out = String::new();
    if let Some(brief) = config.brief.as_deref().filter(|b| !b.is_empty()) {
        out.push_str("Research Brief (use this as your mission statement):\n\n");
        out.push_str(brief);
        out.push_str("\n\n");
    }
    out.push_str(&format!(
        "You are writing the analysis section of a research report for the topic:\n\n{topic}\n\n"
    ));
    out
}

/// Render the mandatory top-level section instructions plus the
/// per-finding labeled-paragraph template. With the default config this is
/// byte-identical to the legacy middle of `build_synthesis_prompt`.
///
/// The prompt asks the model for the same raw sections regardless of the final
/// output format: Executive Summary, Top 10 Implications, Findings,
/// In-Project Cross-References, and Open Questions. The document assembler
/// reorders these sections to match the selected `OutputFormat`.
///
/// The `IMRaD` output format (FR-012 / specs/imradreport) is handled specially:
/// the model is still asked for the same raw sections so the parser remains
/// unchanged, and an extra paragraph encourages results-oriented phrasing so
/// the final `IMRaD` layout reads naturally in the `## Results` section.
// reason: only consumed inside this crate - `pub` here never escapes the crate.
#[allow(unreachable_pub)]
pub fn render_output_template(config: &SynthesisPromptConfig) -> String {
    let mut out = String::new();
    match config.output_format {
        Some(OutputFormat::ExecutiveSummary) => {
            out.push_str("## Executive Summary\n");
            out.push_str("A very concise executive summary in 2-3 sentences.\n\n");
            out.push_str("## Top 10 Implications\n");
            out.push_str(
                "Rank the top 10 practical consequences implied by the evidence. Output a numbered list `1.`..`10.` ordered by importance. Each entry must be one or two sentences. If fewer than 10 are justified, list only those.\n\n",
            );
            out.push_str("## Findings\n");
            out.push_str(
                "At most 5 high-level findings. Keep each finding to one compact paragraph per required label. \
                 Begin each finding with a short **Headline:** paragraph (maximum 15 words) that summarizes the **Observation** paragraph. \
                 Each finding must contain at least **five markdown paragraphs** with these bold labels, in this order:\n\n\
                 **Headline:** A concise, no-more-than-15-word summary of the observation.\n\n\
                 **Observation:** State the concrete evidence or fact observed in the sources, including at least one `[#N]` citation.\n\n\
                 **Analysis:** Explain why the observation matters for the topic. This paragraph must be substantive — write more than 512 characters (several detailed sentences). Draw on specific evidence from the sources: cite concrete data points, quote relevant passages using `[#N]` references, compare or contrast with other findings, discuss causal mechanisms, weigh supporting and contradicting evidence, and explore the broader implications of the observation. When the sources provide enough detail, write a longer analysis covering multiple angles, limitations of the evidence, and connections to the broader topic. Every sentence should carry analytical weight — do not pad with filler or repetition.\n\n\
                 **Cross-reference / Dependencies:** Name any other finding(s) this one builds on, or write \"No direct dependencies.\"\n\n\
                 **Implication:** Summarize the practical consequence or follow-up action.\n\n\
                 Put each label on its own line, and separate every paragraph with a blank line.\n\n",
            );
        }
        Some(OutputFormat::ComparisonTable) => {
            out.push_str("## Executive Summary\n");
            out.push_str("One-paragraph overview of the entities being compared.\n\n");
            out.push_str("## Comparison Table\n");
            out.push_str(
                "A markdown table with columns: Entity | Key strengths | Key weaknesses | Best for | Sources. \
                 Cite web sources with `[#N]` in the Sources column.\n\n",
            );
            out.push_str("## Findings\n");
            out.push_str(
                "3-7 findings that explain the comparison and cite sources with `[#N]`. \
                 Begin each finding with a short **Headline:** paragraph (maximum 15 words) that summarizes the **Observation** paragraph. \
                 Each finding must contain at least **five markdown paragraphs** with these bold labels, in this order:\n\n\
                 **Headline:** A concise, no-more-than-15-word summary of the observation.\n\n\
                 **Observation:** State the concrete evidence or fact observed in the sources, including at least one `[#N]` citation.\n\n\
                 **Analysis:** Explain why the observation matters for the comparison. This paragraph must be substantive — write more than 512 characters (several detailed sentences). Draw on specific evidence from the sources: cite concrete data points, quote relevant passages using `[#N]` references, compare or contrast the entities being compared, discuss trade-offs and causal mechanisms, weigh supporting and contradicting evidence, and explore the broader implications of the observation for the comparison. When the sources provide enough detail, write a longer analysis covering multiple angles, limitations of the evidence, and connections to the broader topic. Every sentence should carry analytical weight — do not pad with filler or repetition.\n\n\
                 **Cross-reference / Dependencies:** Name any other finding(s) this one builds on, or write \"No direct dependencies.\"\n\n\
                 **Implication:** Summarize the practical consequence or follow-up action.\n\n\
                 Put each label on its own line, and separate every paragraph with a blank line.\n\n",
            );
        }
        Some(OutputFormat::SourceBibliography) => {
            out.push_str("## Executive Summary\n");
            out.push_str("One paragraph summarizing the corpus.\n\n");
            out.push_str("## Findings\n");
            out.push_str(
                "An annotated bibliography: one entry per major source, describing its contribution and citing `[#N]`. \
                 Begin each entry with a short **Headline:** paragraph (maximum 15 words) that summarizes the **Observation** paragraph. \
                 Each entry must contain at least **five markdown paragraphs** with these bold labels, in this order:\n\n\
                 **Headline:** A concise, no-more-than-15-word summary of the observation.\n\n\
                 **Observation:** State the concrete evidence or fact from the source, including at least one `[#N]` citation.\n\n\
                 **Analysis:** Explain the source's contribution to the topic. This paragraph must be substantive — write more than 512 characters (several detailed sentences). Draw on specific evidence from the source: cite concrete data points, quote relevant passages using `[#N]` references, assess the source's methodology and credibility, discuss how it supports or contradicts other sources, and explore the broader implications of the source's contribution. When the source provides enough detail, write a longer analysis covering multiple angles, limitations of the evidence, and connections to the broader topic. Every sentence should carry analytical weight — do not pad with filler or repetition.\n\n\
                 **Cross-reference / Dependencies:** Name any other source or finding this one relates to, or write \"No direct dependencies.\"\n\n\
                 **Implication:** Summarize how this source should influence conclusions.\n\n\
                 Put each label on its own line, and separate every paragraph with a blank line.\n\n",
            );
        }
        _ => {
            out.push_str("## Executive Summary\n");
            out.push_str(
                "A concise one-paragraph executive summary of what the sources collectively say about the topic.\n\n",
            );
            out.push_str("## Top 10 Implications\n");
            out.push_str(
                "Rank the top 10 practical consequences implied by the evidence. Output a numbered list `1.`..`10.` ordered by importance. Each entry must be one or two sentences. If fewer than 10 are justified, list only those.\n\n",
            );
            out.push_str("## Findings\n");
            out.push_str(
                "A numbered list of concrete findings. Aim for around 20 distinct findings when the sources have enough breadth and depth to support that many; for narrower topics, include every worthwhile point rather than padding. Begin each finding with a short **Headline:** paragraph (maximum 15 words) that summarizes the **Observation** paragraph. Each finding must contain at least \
                      **five markdown paragraphs** with these bold labels, in this order:\n\n\
                      **Headline:** A concise, no-more-than-15-word summary of the observation.\n\n\
                      **Observation:** State the concrete evidence or fact observed in the sources, including at least one `[#N]` citation. You may cite multiple sources in a finding if several support the same point.\n\n\
                      **Analysis:** Explain why the observation matters for the topic and how it connects to the broader research question. This paragraph must be substantive — write more than 512 characters (several detailed sentences). Draw on specific evidence from the sources: cite concrete data points, quote relevant passages using `[#N]` references, compare or contrast with other findings, discuss causal mechanisms, weigh supporting and contradicting evidence, and explore the broader implications of the observation. When the sources provide enough detail, write a longer analysis covering multiple angles, limitations of the evidence, alternative interpretations, and connections to the broader research question. Every sentence should carry analytical weight — do not pad with filler or repetition.\n\n\
                      **Cross-reference / Dependencies:** Name any other finding(s) this one builds on, contradicts, or is prerequisite to, using `Finding N` references. If there are no dependencies, write \"No direct dependencies.\"\n\n\
                      **Implication:** Summarize the practical consequence, open risk, or recommended follow-up action.\n\n\
                      Put each label on its own line, and separate every paragraph with a blank line. \
                      You may add additional paragraphs after the five required ones (for example, \
                      extra evidence, related work, caveats, or implementation notes). Each additional \
                      paragraph must also begin with a bold label such as **Label:** so it is easy to \
                      parse. Put each finding on its own line starting with `1. `, `2. `, etc.\n\n"
            );
        }
    }
    // FR-012 / specs/imradreport: IMRaD format uses the same raw sections, but
    // the model should phrase findings as results-oriented statements.
    if config.output_format == Some(OutputFormat::Imrad) {
        out.push_str(
            "\nThe final report will be restructured into IMRaD order by the document assembler, \
             so continue to use the section headings above. Phrase each finding as a results-oriented \
             statement suitable for an IMRaD Results section: state the discovery, support it with \
             `[#N]` citations, and reserve interpretation and broader implications for the \
             Analysis and Implication paragraphs.\n\n"
        );
    }
    // T-003 (FR-003): require a sixth **Sources Cited / Date Spread**
    // paragraph in every finding. Gated on `config.date_spread_paragraph` so
    // the default-config output stays byte-identical to the legacy prompt.
    if config.date_spread_paragraph {
        out.push_str(
            "In addition to the five required paragraphs above, every finding must end with a sixth paragraph labeled:\n\n\
            **Sources Cited / Date Spread:**\n\
            List every `[#N]` citation used in the finding, then report the earliest and latest publication dates among those cited web sources (use the `Published` line in each source header below; write `undated` when a cited source has no publication date). When the `Author` line for a cited source is not `unknown`, include the author name in this paragraph as well. Add one sentence explaining how the date range — and the recency of the evidence — affects the finding's confidence, relevance, or conclusions. If every cited source is undated, say so explicitly and explain the implication.\n\n\
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
    out.push_str("## Top 10 Implications\n");
    out.push_str(
        "Analyze the practical consequences of the evidence and rank the top 10 implications. \
         Output a numbered list `1.`..`10.` ordered by importance or practicality. Each entry must be one \
         or two sentences: state the consequence, why it matters for the topic, and (when relevant) \
         cite the finding or source `[#N]` that supports it. If fewer than 10 distinct implications are \
         justified by the evidence, list only the justified ones — do not pad with speculation.\n\n",
    );
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
            every finding must still contain the five required labeled paragraphs \
            (Headline, Observation, Analysis, Cross-reference / Dependencies, Implication) \
            and, when requested, the sixth **Sources Cited / Date Spread** \
            paragraph. Treat template sections as additional output, not as a \
            substitute for the structured findings or the Top 10 Implications section.\n\n",
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
            "Few-shot exemplar findings (for format calibration only — do NOT \\
            copy their content into your answer; derive findings from the \\
            supplied sources):\\n\\n",
        );
        for (idx, example) in config.few_shot_examples.iter().take(2).enumerate() {
            out.push_str(&format!("### Exemplar Finding {}\\n\\n", idx + 1));
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
// reason: only consumed inside this crate - `pub` here never escapes the crate.
#[allow(unreachable_pub)]
pub fn render_sources_block(sources: &[SourceBody], include_published: bool) -> String {
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
        let author_line = match src.author.as_deref() {
            Some(a) if !a.is_empty() => format!("\nAuthor: {a}"),
            _ => "\nAuthor: unknown".to_string(),
        };
        out.push_str(&format!(
            "#### Source [#{index}] ({kind}) {title}\nPath/URL: {path}{published}{author}\nRelevance: {rel}\n```text\n{body}\n```\n\n",
            index = src.index,
            kind = src.kind,
            title = src.title,
            path = src.path_or_url,
            published = published_line,
            author = author_line,
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
// reason: only consumed inside this crate - `pub` here never escapes the crate.
#[allow(unreachable_pub)]
pub fn render_closing(_config: &SynthesisPromptConfig) -> String {
    let mut out = String::new();
    out.push_str(
        "\nNow produce only the six sections above: Executive Summary, Top 10 Implications, Findings, In-Project Cross-References, and Open Questions. Do not include a title or any other preamble. ",
    );
    out.push_str(
        "Within Findings, always begin with a **Headline:** paragraph (maximum 15 words) and include the four required paragraphs (Observation, Analysis, ",
    );
    out.push_str(
        "Cross-reference / Dependencies, Implication) after it. Feel free to add more labeled paragraphs if the sources support it. \
         The **Analysis** paragraph in every finding must be substantive — write more than 512 characters, drawing on all available source data. \
         When the sources are rich, write a longer analysis that covers multiple angles, evidence limitations, and connections to the broader topic.",
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
#[allow(dead_code)]
// preserved for backward-compat byte-identical tests
// reason: only consumed inside this crate - `pub` here never escapes the crate.
#[allow(unreachable_pub)]
pub fn build_synthesis_prompt(topic: &str, sources: &[SourceBody]) -> String {
    SynthesisPromptBuilder::new(topic)
        .sources(sources)
        .output_format(OutputFormat::Report)
        .build()
}

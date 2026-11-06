//! LLM-assisted enrichment of a research diff deck (spec `diffdeck`, FR-023,
//! FR-024).
//!
//! [`crate::deck`] renders a deck that is deterministic and model-free: a
//! finding shows the first sentence of its own prose, and a concept lists no
//! findings at all. This module adds the two things a reader needs that the
//! raw report and the pure renderer cannot supply on their own:
//!
//! 1. **One paragraph per finding** (FR-023). The report writes each finding as
//!    a headline followed by `**Observation:**`, `**Analysis:**` and
//!    `**Implication:**` blocks; [`crate::deck::DeckItem::detail`] carries
//!    those blocks. The enrichment stage condenses them into a single paragraph
//!    that stands under the headline on the finding slide.
//! 2. **Findings placed under the concept they belong to** (FR-024, FR-026).
//!    The report's `## Concepts` section names the corpus's themes but not the
//!    findings behind them; the enrichment stage decides, for each concept,
//!    which findings belong to it so the deck can render one slide per concept
//!    with its findings grouped beneath the concept's definition.
//!
//! ## The model is optional, the result is not
//!
//! When a model is wired in ([`DeckEnricher`] backed by
//! [`crate::analysis::LlmAnalysisEngine`]) each stage asks it once and parses a
//! strict line-oriented reply. When no model is wired in - or the call fails,
//! times out, or returns something unusable - every stage falls back to a
//! deterministic local computation:
//!
//! - the paragraph is the first sentence of Observation and the first sentence
//!   of Analysis, joined ([`crate::deck::DeckItem::deterministic_summary`]);
//! - the concept-to-findings mapping is a term-overlap score, with the
//!   best-scoring findings above a floor kept for each concept.
//!
//! So a deck is never worse than its deterministic baseline, never refuses to
//! render because the provider is down, and a run with no provider configured
//! still produces the improved layout.
//!
//! ## Purity
//!
//! Everything except [`DeckEnricher`] is a pure function of its arguments:
//! [`deterministic_enrichment`], [`parse_summary_reply`],
//! [`parse_relevance_reply`] and [`relevance_ranking`] take values and return
//! values. [`enrich_deck`] is the one asynchronous entry point; it consults the
//! enricher and delegates to the pure functions for every fallback, so the
//! renderer downstream stays a pure function of its inputs (NFR-002).
//!
//! ## Dependencies
//!
//! [`crate::deck`] for the parsed report types and its text helpers,
//! [`crate::analysis::LlmAnalysisEngine`] for the provider call, and
//! `std::collections` for the term sets. Nothing here reads a file, the
//! configuration or the database.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use tracing::debug;

use crate::analysis::LlmAnalysisEngine;
use crate::deck::{DeckElement, DeckItem, ParsedReport, collapse_whitespace, escape_markdown};
use crate::deck_progress::{self, DeckEvent, DeckProgress, DeckStage};

/// The most findings one concept slide may list beneath its definition.
///
/// A slide stays readable with a short list; the model can name more and the
/// extra names are dropped in rank order.
pub const MAX_FINDINGS_PER_CONCEPT: usize = 3;

/// The most concept/finding pairs a single relevance call is asked about.
///
/// The prompt names every finding by its 1-based index, so the reply is a set of
/// indices rather than a restatement of the headline (which the model could
/// paraphrase and break the match). A report with more findings than this is
/// truncated to the first [`MAX_RANKED_FINDINGS`], which is the order the report
/// itself presents them in.
pub const MAX_RANKED_FINDINGS: usize = 60;

/// Character budget for one finding's prompt block.
///
/// A finding's Observation and Analysis together run to a few thousand
/// characters; the cap keeps a single very long finding from crowding every
/// other finding's evidence out of the prompt.
pub const FINDING_PROMPT_MAX_CHARS: usize = 1600;

/// Character budget for one concept's prompt line.
pub const CONCEPT_PROMPT_MAX_CHARS: usize = 400;

/// Character cap for the paragraph written under a finding headline.
///
/// The paragraph is prose, not a bullet, so it is allowed more room than
/// [`crate::deck::CONDENSE_MAX_CHARS`]; beyond this a "summary" is a second
/// report and the slide stops being a slide.
pub const SUMMARY_MAX_CHARS: usize = 700;

/// One finding reduced to the values the enrichment prompts need.
///
/// It borrows from the parsed report rather than copying it, so building the
/// prompt costs no clone of the report itself.
#[derive(Debug, Clone, Copy)]
struct FindingRef<'a> {
    /// The finding's 1-based position among the newer report's findings.
    index: usize,
    /// The parsed finding, whose `text` is the headline and `detail` the labels.
    item: &'a DeckItem,
}

/// The result of enriching a deck: everything the renderer needs that is not a
/// plain function of the two reports.
///
/// It is keyed by the item's section identity ([`DeckItem::key`]), so it can be
/// looked up whichever element slide is being rendered and never depends on an
/// item's position in a list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeckEnrichment {
    /// The one-paragraph summary of each finding, by finding identity key.
    ///
    /// A finding with no entry is rendered from its deterministic fallback, so
    /// a partial enrichment never loses a finding.
    pub summaries: HashMap<String, String>,
    /// The findings belonging to each concept, by concept identity key, most
    /// relevant first.
    ///
    /// Each entry holds finding identity keys (the same values
    /// [`DeckItem::key`] carries), so the renderer can label a concept's
    /// findings without holding a second copy of the report.
    pub concept_findings: HashMap<String, Vec<String>>,
    /// `true` when a model produced at least one value in this enrichment.
    ///
    /// It lets the orchestration layer report whether the run spent tokens, and
    /// lets a test assert the deterministic path was taken.
    pub used_model: bool,
}

impl DeckEnrichment {
    /// The paragraph to render under `finding`'s headline.
    ///
    /// A model-written summary wins when present; otherwise the item's
    /// deterministic fallback is used, so the finding slide always carries a
    /// paragraph for a finding that has any prose at all.
    #[must_use]
    pub fn summary_for(&self, finding: &DeckItem) -> Option<String> {
        if let Some(written) = self.summaries.get(&finding.key) {
            let written = written.trim();
            if !written.is_empty() {
                return Some(written.to_string());
            }
        }
        let fallback = finding.deterministic_summary();
        if fallback.is_empty() {
            None
        } else {
            Some(fallback)
        }
    }

    /// The findings that belong to the concept `concept`.
    ///
    /// Returns the finding items, most relevant first, capped at
    /// [`MAX_FINDINGS_PER_CONCEPT`]. A concept with no entry lists no findings,
    /// and the concept slide says so explicitly rather than being omitted.
    #[must_use]
    pub fn findings_for<'a>(
        &self,
        concept: &DeckItem,
        findings: &'a [DeckItem],
    ) -> Vec<&'a DeckItem> {
        let Some(keys) = self.concept_findings.get(&concept.key) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for key in keys.iter().take(MAX_FINDINGS_PER_CONCEPT) {
            if let Some(finding) = findings.iter().find(|finding| &finding.key == key) {
                out.push(finding);
            }
        }
        out
    }

    /// Every finding identity key this enrichment has placed under a concept.
    ///
    /// The renderer's Findings slide lists only the findings no concept claimed,
    /// so that each finding appears on the deck exactly once (under its concept)
    /// and a finding that fits no concept is still shown rather than dropped.
    #[must_use]
    pub fn claimed_finding_keys(&self) -> HashSet<&str> {
        self.concept_findings
            .values()
            .flat_map(|keys| {
                keys.iter()
                    .take(MAX_FINDINGS_PER_CONCEPT)
                    .map(String::as_str)
            })
            .collect()
    }
}

/// The seam between the deck and a language model.
///
/// One method per enrichment stage, so a test can supply a fake and assert both
/// the prompt it is handed and how its reply is parsed. Every method returns
/// `anyhow::Result<String>` because a provider failure is recoverable: the
/// caller falls back to the deterministic value rather than failing the run.
#[async_trait]
pub trait DeckModel: Send + Sync {
    /// Write one paragraph summarising a finding from its labelled blocks.
    async fn summarize_finding(
        &self,
        headline: &str,
        blocks: &[String],
        max_chars: usize,
    ) -> anyhow::Result<String>;

    /// Choose, for every concept, the findings that belong to it.
    ///
    /// `concepts` and `findings` are `(display text, prompt text)` pairs; the
    /// reply is parsed by [`parse_relevance_reply`], so the model answers with
    /// concept and finding indices.
    async fn rank_concept_findings(
        &self,
        concepts: &[(String, String)],
        findings: &[(String, String)],
    ) -> anyhow::Result<String>;
}

/// A [`DeckModel`] backed by the research crate's LLM analysis engine.
///
/// It reuses [`LlmAnalysisEngine::complete_raw`], the same low-level completion
/// path the title and subject summarisers use, so it inherits the wired
/// provider, model, API key and base URL unchanged.
#[derive(Debug, Clone)]
pub struct LlmDeckModel {
    engine: Arc<LlmAnalysisEngine>,
}

impl LlmDeckModel {
    /// Wrap an analysis engine as a deck model.
    #[must_use]
    pub fn new(engine: Arc<LlmAnalysisEngine>) -> Self {
        Self { engine }
    }
}

/// The system message shared by both enrichment calls.
const ENRICH_SYSTEM: &str = "You are a research analyst preparing slide notes. \
     Answer only with the format the user specifies - no preamble, no markdown \
     fences, no commentary.";

#[async_trait]
impl DeckModel for LlmDeckModel {
    async fn summarize_finding(
        &self,
        headline: &str,
        blocks: &[String],
        max_chars: usize,
    ) -> anyhow::Result<String> {
        let prompt = build_summary_prompt(headline, blocks, max_chars);
        self.engine
            .complete_raw(&prompt, Some(ENRICH_SYSTEM), 400)
            .await
    }

    async fn rank_concept_findings(
        &self,
        concepts: &[(String, String)],
        findings: &[(String, String)],
    ) -> anyhow::Result<String> {
        let prompt = build_relevance_prompt(concepts, findings);
        self.engine
            .complete_raw(&prompt, Some(ENRICH_SYSTEM), 1200)
            .await
    }
}

/// Build the prompt that asks for one finding's paragraph.
fn build_summary_prompt(headline: &str, blocks: &[String], max_chars: usize) -> String {
    let mut prompt = String::new();
    prompt.push_str("Condense the research finding below into ONE paragraph of ");
    prompt.push_str(&format!("at most {max_chars} characters. "));
    prompt.push_str(
        "The paragraph must summarise what was observed AND what the analysis \
         concluded, in plain prose, with no heading, no bullet, no bold markers \
         and no citation markers. Write it as a single paragraph that could sit \
         under the finding's title on a slide.\n\n",
    );
    prompt.push_str("Title: ");
    prompt.push_str(headline.trim());
    prompt.push_str("\n\n");
    for block in blocks {
        let block = truncate_chars(&collapse_whitespace(block), FINDING_PROMPT_MAX_CHARS);
        prompt.push_str(&block);
        prompt.push_str("\n\n");
    }
    prompt.push_str("Reply with the paragraph only.");
    prompt
}

/// Build the prompt that asks which findings belong to which concept.
fn build_relevance_prompt(concepts: &[(String, String)], findings: &[(String, String)]) -> String {
    let mut prompt = String::new();
    prompt.push_str(
        "You are grouping research findings under the concepts they belong to.\n\n\
         The CONCEPTS below are numbered. For each one, choose the findings from \
         the FINDINGS list that report its evidence. A finding belongs to at most \
         two concepts, and a concept with no matching finding may be left out.\n\n\
         Reply with one line per concept, in this exact form:\n\
         <concept number>: <finding number>[, <finding number>...]\n\
         Use only the numbers given. Do not explain, do not add text.\n\n",
    );
    prompt.push_str("CONCEPTS\n");
    for (idx, (_, text)) in concepts.iter().enumerate() {
        let text = truncate_chars(&collapse_whitespace(text), CONCEPT_PROMPT_MAX_CHARS);
        prompt.push_str(&format!("{}. {text}\n", idx + 1));
    }
    prompt.push_str("\nFINDINGS\n");
    for (idx, (_, text)) in findings.iter().enumerate() {
        let text = truncate_chars(&collapse_whitespace(text), FINDING_PROMPT_MAX_CHARS);
        prompt.push_str(&format!("{}. {text}\n", idx + 1));
    }
    prompt
}

/// Cap `text` at `limit` characters without splitting a UTF-8 code point.
fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let head: String = text.chars().take(limit).collect();
    format!("{}...", head.trim_end())
}

/// Sanitise a model-written finding paragraph into one slide-safe line.
///
/// The reply is reduced to a single paragraph - every whitespace run collapsed,
/// a leading `#` escaped so it cannot open a heading, `|` and backticks escaped
/// so they cannot break a table or a code span - and capped at `max_chars` on a
/// word boundary. A reply that is empty after sanitising yields `None`, which
/// the caller treats as "use the deterministic fallback".
#[must_use]
pub fn clean_summary(reply: &str, max_chars: usize) -> Option<String> {
    let text = collapse_whitespace(reply.trim());
    // A model sometimes prefixes a label or wraps the paragraph in quotes.
    let text = text
        .strip_prefix("Summary:")
        .or_else(|| text.strip_prefix("summary:"))
        .unwrap_or(&text)
        .trim()
        .to_string();
    let text = text.trim_matches('"').trim();
    if !text.chars().any(|c| c.is_alphanumeric()) {
        return None;
    }
    let escaped = escape_markdown(text);
    Some(truncate_words_local(&escaped, max_chars))
}

/// Cap `text` at `limit` characters on a word boundary with a trailing `...`.
fn truncate_words_local(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut head: String = text.chars().take(limit).collect();
    if let Some(cut) = head.rfind(' ') {
        head.truncate(cut);
    }
    format!("{}...", head.trim_end())
}

/// Parse the `N: a, b, c` lines of a relevance reply into 1-based index pairs.
///
/// The parser is deliberately forgiving: numbering markers (`1.`, `1)`, `-`), a
/// leading `Concept`/`Concepts`/`Implication`/`Finding` word and surrounding
/// bold markers are all tolerated, because a model that understands the task
/// still formats it inconsistently. Anything else - a line with no colon, a non-numeric token, an
/// index of `0` - is skipped rather than failing the reply, so one malformed
/// line cannot cost the whole mapping.
///
/// Returns `(concept_index, finding_index)` pairs, both 1-based and in reply
/// order.
#[must_use]
pub fn parse_relevance_reply(reply: &str) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for line in reply.lines() {
        // `:` separates a mapping line; `)` is the other spelling a model
        // uses for the concept's ordinal (`2) Finding 3`).
        let Some((left, right)) = split_mapping_line(line) else {
            continue;
        };
        let Some(concept) = leading_index(left) else {
            continue;
        };
        for token in right.split([',', ';', '|']) {
            if let Some(finding) = leading_index(token) {
                pairs.push((concept, finding));
            }
        }
    }
    pairs
}

/// Split a mapping line into its concept side and its finding side.
///
/// A line closes with a finding list after a `:` (`1: 2, 3`, `- 1 : 2`), but a
/// model that writes `2) Finding 3` uses `)` for the ordinal instead. Both are
/// accepted; the separator must be the first one after the concept's own
/// number, so a finding label containing a colon cannot split the line early.
fn split_mapping_line(line: &str) -> Option<(&str, &str)> {
    let colon = line.find(':');
    let paren = line.find(')');
    match (colon, paren) {
        (Some(colon), Some(paren)) if paren < colon => {
            Some((line.get(..paren)?, line.get(paren + 1..)?))
        }
        (Some(colon), _) => Some((line.get(..colon)?, line.get(colon + 1..)?)),
        // No separator at all: `2) Finding 3` closes its ordinal with a paren
        // and names the finding after it.
        (None, Some(paren)) => Some((line.get(..paren)?, line.get(paren + 1..)?)),
        (None, None) => None,
    }
}

/// The 1-based index at the start of `text`, if there is one.
///
/// A model writes the mapping as `1:`, `1.`, `1)`, `- 1:`, `**1**:`,
/// `Concept 1:`, `Concepts 1:` and `Finding 3`; every one of those reduces to
/// the number that follows a leading decoration and a known label word.
fn leading_index(text: &str) -> Option<usize> {
    let mut cleaned = text
        .trim()
        .trim_start_matches(['*', '#', '-', '(', '[', ')', ' '])
        .trim();
    for label in [
        "Concepts",
        "concepts",
        "Concept",
        "concept",
        "Implication",
        "implication",
        "Finding",
        "finding",
    ] {
        if let Some(rest) = cleaned.strip_prefix(label) {
            cleaned = rest
                .trim_start_matches(['*', '#', '-', '(', '[', ')', ' ', '\t'])
                .trim_end_matches('*')
                .trim_start_matches(['*', ' ', ':'])
                .trim();
            break;
        }
    }
    let digits: String = cleaned.chars().take_while(char::is_ascii_digit).collect();
    let value: usize = digits.parse().ok()?;
    (value > 0).then_some(value)
}

/// Split a model reply into finding summaries keyed by 1-based finding index.
///
/// The summary prompt asks for one paragraph per call, so this accepts the whole
/// reply as the paragraph for `index`; it exists as a named function so the
/// orchestration stage and its tests agree on how a reply becomes a value.
#[must_use]
pub fn parse_summary_reply(reply: &str, max_chars: usize) -> Option<String> {
    clean_summary(reply, max_chars)
}

/// The findings of `report`, in document order, reduced to what the prompts need.
fn findings_of(report: &ParsedReport) -> Vec<FindingRef<'_>> {
    report
        .items(DeckElement::Findings)
        .iter()
        .filter(|item| !item.detail.is_empty())
        .enumerate()
        .take(MAX_RANKED_FINDINGS)
        .map(|(offset, item)| FindingRef {
            index: offset + 1,
            item,
        })
        .collect()
}

/// The concepts of `report`, in document order.
///
/// A report with no `## Concepts` section - an older run, or one whose concept
/// extraction did not run - yields no concepts, and the deck then renders every
/// finding on the Findings slide with no concept grouping at all.
fn concepts_of(report: &ParsedReport) -> &[DeckItem] {
    report.items(DeckElement::Concepts)
}

/// The `(display text, prompt text)` pair of a concept for the grouping prompt.
///
/// The display text is the concept's name; the prompt text adds its definition
/// so the model can tell two similarly named concepts apart.
fn item_prompt_pair(item: &DeckItem) -> (String, String) {
    (item.text.clone(), item_prompt_text(item))
}

/// The prompt text for a concept: its name and its definition blocks.
fn item_prompt_text(item: &DeckItem) -> String {
    let mut text = item.text.clone();
    for block in &item.detail {
        text.push(' ');
        text.push_str(block);
    }
    text
}

/// The prompt text for one finding: its headline and its labelled blocks.
fn finding_prompt_text(item: &DeckItem) -> String {
    let mut text = item.text.clone();
    for block in &item.detail {
        text.push(' ');
        text.push_str(block);
    }
    text
}

/// Enrich a deck from two parsed reports, consulting `model` when supplied.
///
/// The newer report supplies the findings and concepts the diff stage will
/// render; a finding that also exists in `older` keeps its newer wording, which
/// is what the slide shows.
///
/// Every model call is best-effort. A summary call that fails leaves that
/// finding out of [`DeckEnrichment::summaries`], so the renderer falls back to
/// [`DeckItem::deterministic_summary`]; a relevance call that fails - or whose
/// reply names no valid pair - leaves the mapping to
/// [`deterministic_enrichment`]'s term-overlap ranking. The returned enrichment
/// always carries a complete mapping for the concepts that have evidence, so
/// `model: None` is a supported call, not a degraded one.
pub async fn enrich_deck(
    newer: &ParsedReport,
    older: ParsedReport,
    model: Option<&dyn DeckModel>,
) -> DeckEnrichment {
    enrich_deck_with_progress(newer, older, model, None).await
}

/// The stage-progress form of [`enrich_deck`] (FR-031).
///
/// The enrichment is the slowest part of a deck run - one model call per finding
/// plus one grouping call - so it reports its own unit progress: a start report
/// naming how many findings and concepts it will visit, one report per finding
/// as its paragraph is written, and one for the grouping call. A run with no
/// model reports a single `skipped` pair instead, so the surface can tell "no
/// provider" apart from "still working".
///
/// The reports describe only what is *attempted*, never a model's success: the
/// per-finding report is emitted before its call so a slow call is visible while
/// it runs, and the deterministic fallback behind a failed call is not a
/// separate stage (FR-025 makes it invisible to the artifact).
pub async fn enrich_deck_with_progress(
    newer: &ParsedReport,
    older: ParsedReport,
    model: Option<&dyn DeckModel>,
    progress: Option<&dyn DeckProgress>,
) -> DeckEnrichment {
    let findings = findings_of(newer);
    let concepts = concepts_of(newer);
    let mut enrichment = deterministic_enrichment(newer, &older);
    let Some(model) = model else {
        deck_progress::report(
            progress,
            DeckEvent::skipped(
                DeckStage::EnrichFindings,
                format!(
                    "no model selected; {} finding paragraph(s) use the deterministic summary",
                    findings.len()
                ),
            ),
        );
        deck_progress::report(
            progress,
            DeckEvent::skipped(
                DeckStage::EnrichConcepts,
                "no model selected; concept grouping uses the term-overlap ranking".to_string(),
            ),
        );
        return enrichment;
    };

    deck_progress::report(
        progress,
        DeckEvent::started(
            DeckStage::EnrichFindings,
            format!("summarising {} finding(s) with the model", findings.len()),
        ),
    );

    // Stage 1 (FR-023): one paragraph per finding.
    // One report per finding, emitted before its call: a model call can take
    // seconds, so the report names the finding being worked on *now*, and
    // `completed` counts the findings already finished. The headline is capped so
    // a very long one cannot flood a progress line; it names the finding, it does
    // not reproduce it.
    let total_findings = findings.len();
    for (offset, finding) in findings.iter().enumerate() {
        deck_progress::report(
            progress,
            DeckEvent::progress(
                DeckStage::EnrichFindings,
                offset,
                total_findings,
                progress_headline(&finding.item.text),
            ),
        );
        match model
            .summarize_finding(&finding.item.text, &finding.item.detail, SUMMARY_MAX_CHARS)
            .await
        {
            Ok(reply) => {
                if let Some(summary) = parse_summary_reply(&reply, SUMMARY_MAX_CHARS) {
                    enrichment
                        .summaries
                        .insert(finding.item.key.clone(), summary);
                    enrichment.used_model = true;
                }
            }
            Err(error) => {
                tracing::debug!(
                    error = %error,
                    finding = %finding.item.text,
                    "research deck: finding summary call failed; using the deterministic paragraph"
                );
            }
        }
    }
    deck_progress::report(
        progress,
        DeckEvent::done(
            DeckStage::EnrichFindings,
            format!(
                "{} of {} finding paragraph(s) written by the model",
                enrichment.summaries.len(),
                total_findings
            ),
        ),
    );

    // Stage 2 (FR-024, FR-026): which findings belong to each concept.
    if !concepts.is_empty() && !findings.is_empty() {
        deck_progress::report(
            progress,
            DeckEvent::started(
                DeckStage::EnrichConcepts,
                format!(
                    "grouping {} finding(s) under {} concept(s)",
                    findings.len(),
                    concepts.len()
                ),
            ),
        );
        let concept_prompts: Vec<(String, String)> =
            concepts.iter().map(item_prompt_pair).collect();
        let finding_prompts: Vec<(String, String)> = findings
            .iter()
            .map(|finding| (finding.item.text.clone(), finding_prompt_text(finding.item)))
            .collect();
        match model
            .rank_concept_findings(&concept_prompts, &finding_prompts)
            .await
        {
            Ok(reply) => {
                if apply_relevance_reply(
                    &reply,
                    concepts,
                    &findings,
                    &mut enrichment.concept_findings,
                )
                .is_some()
                {
                    enrichment.used_model = true;
                }
            }
            Err(error) => {
                tracing::debug!(
                    error = %error,
                    "research deck: concept grouping call failed; keeping the term-overlap ranking"
                );
            }
        }
        deck_progress::report(
            progress,
            DeckEvent::done(
                DeckStage::EnrichConcepts,
                format!("{} concept(s) grouped", enrichment.concept_findings.len()),
            ),
        );
    } else {
        deck_progress::report(
            progress,
            DeckEvent::skipped(
                DeckStage::EnrichConcepts,
                format!(
                    "{} concept(s) and {} finding(s): nothing to group",
                    concepts.len(),
                    findings.len()
                ),
            ),
        );
    }
    enrichment
}

/// A finding headline shortened for a progress line (FR-031).
///
/// It exists so a per-finding report names the finding being worked on without
/// reproducing it: the text is collapsed to one line, Markdown-escaped so a `|`
/// or a backtick in a headline cannot break the message window's layout, and
/// capped at 60 characters with an ellipsis.
fn progress_headline(text: &str) -> String {
    // Escape first, then cap: escaping can lengthen the text (`|` becomes
    // `\|`), and capping before it would let an escaped headline exceed the
    // budget the cap is there to enforce.
    let escaped = escape_markdown(&collapse_whitespace(text));
    if escaped.chars().count() <= 60 {
        return escaped;
    }
    let mut capped: String = escaped.chars().take(57).collect();
    capped.push_str("...");
    capped
}

/// Replace the concept mapping with the one a relevance reply names.
///
/// Returns the number of concepts the reply covered, or `None` when it named no
/// valid pair for any concept - in which case the caller keeps the
/// deterministic ranking rather than blanking the slide.
fn apply_relevance_reply(
    reply: &str,
    concepts: &[DeckItem],
    findings: &[FindingRef<'_>],
    mapping: &mut HashMap<String, Vec<String>>,
) -> Option<usize> {
    let pairs = parse_relevance_reply(reply);
    if pairs.is_empty() {
        return None;
    }
    let mut per_concept: HashMap<usize, Vec<usize>> = HashMap::new();
    for (concept, finding) in pairs {
        if concept > concepts.len() || finding > findings.len() {
            continue;
        }
        let entry = per_concept.entry(concept).or_default();
        if !entry.contains(&finding) {
            entry.push(finding);
        }
    }
    if per_concept.is_empty() {
        return None;
    }
    for (concept_index, finding_indices) in per_concept {
        let concept = &concepts[concept_index - 1];
        let keys: Vec<String> = finding_indices
            .iter()
            .filter_map(|index| findings.get(index - 1))
            .take(MAX_FINDINGS_PER_CONCEPT)
            .map(|finding| finding.item.key.clone())
            .collect();
        mapping.insert(concept.key.clone(), keys);
    }
    Some(mapping.len())
}

/// The deterministic enrichment: a term-overlap ranking of findings behind each
/// concept, and no model-written summaries.
///
/// The score for one (concept, finding) pair is the weighted size of the
/// intersection between the concept's content terms and the finding's content
/// terms, with a headline match counting double because a finding whose title
/// speaks to the concept is a stronger signal than one whose body happens to
/// share a word. A concept keeps the [`MAX_FINDINGS_PER_CONCEPT`]
/// highest-scoring findings that clear the floor, so a concept with no related
/// finding lists none rather than an arbitrary neighbour.
///
/// `older` is accepted so the caller does not have to special-case the
/// signature; it contributes nothing today (the mapping describes the newer
/// report, which is what the deck renders) and is kept for the next stage that
/// needs a cross-report signal.
#[must_use]
pub fn deterministic_enrichment(newer: &ParsedReport, older: &ParsedReport) -> DeckEnrichment {
    debug!(
        older.sections = older.sections.len(),
        "deterministic enrichment describes the newer report only"
    );
    let findings = findings_of(newer);
    let concepts = concepts_of(newer);
    DeckEnrichment {
        summaries: HashMap::new(),
        concept_findings: relevance_ranking(concepts, &findings),
        used_model: false,
    }
}

/// Rank findings behind each concept by term overlap.
///
/// It is a pure function over the two lists so a test can drive it directly.
/// Findings with no content terms are ignored, and a concept with no scoring
/// finding is absent from the map.
fn relevance_ranking(
    concepts: &[DeckItem],
    findings: &[FindingRef<'_>],
) -> HashMap<String, Vec<String>> {
    let finding_terms: Vec<(HashSet<String>, HashSet<String>)> = findings
        .iter()
        .map(|finding| {
            (
                content_terms(&finding.item.text),
                content_terms(&finding_prompt_text(finding.item)),
            )
        })
        .collect();
    let mut mapping = HashMap::new();
    for concept in concepts {
        let concept_terms = content_terms(&item_prompt_text(concept));
        if concept_terms.is_empty() {
            continue;
        }
        let mut scored: Vec<(usize, &FindingRef<'_>)> = findings
            .iter()
            .zip(&finding_terms)
            .filter_map(|(finding, (headline_terms, body_terms))| {
                let score = relevance_score(&concept_terms, headline_terms, body_terms);
                (score >= RELEVANCE_FLOOR).then_some((score, finding))
            })
            .collect();
        if scored.is_empty() {
            continue;
        }
        // Highest score first; ties keep the report's own order so the deck is
        // stable run to run.
        scored.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.index.cmp(&right.1.index)));
        let keys: Vec<String> = scored
            .into_iter()
            .take(MAX_FINDINGS_PER_CONCEPT)
            .map(|(_, finding)| finding.item.key.clone())
            .collect();
        mapping.insert(concept.key.clone(), keys);
    }
    mapping
}

/// The minimum term-overlap score a finding needs to be listed under a concept.
///
/// Two shared content terms is the smallest overlap that is usually a real
/// relationship rather than a coincidence of common nouns; below it the concept
/// is rendered with no findings rather than a wrong one.
const RELEVANCE_FLOOR: usize = 2;

/// Score one finding against a concept's term set.
fn relevance_score(
    concept_terms: &HashSet<String>,
    headline_terms: &HashSet<String>,
    body_terms: &HashSet<String>,
) -> usize {
    let headline_hits = concept_terms.intersection(headline_terms).count();
    let body_hits = concept_terms.intersection(body_terms).count();
    headline_hits * 2 + body_hits
}

/// The content terms of `text`: lower-case alphanumeric words minus stopwords.
///
/// Words shorter than three characters are dropped (an article, a pronoun or a
/// number carries no topical signal), as is every entry in [`STOPWORDS`]. The
/// set is unordered because only membership matters.
fn content_terms(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(|word| word.to_lowercase())
        .filter(|word| word.chars().count() >= 3 && !STOPWORDS.contains(&word.as_str()))
        .collect()
}

/// The words that carry no topical signal in a finding or a concept.
///
/// It is deliberately short: over-filtering removes the terms two items would
/// otherwise share, and the floor already rejects a single common word.
const STOPWORDS: &[&str] = &[
    "and", "are", "but", "can", "for", "from", "has", "have", "into", "its", "not", "that", "the",
    "their", "them", "then", "there", "these", "they", "this", "was", "were", "will", "with",
    "what", "when", "where", "which", "while", "who", "why", "you", "your", "our", "out", "all",
    "any", "also", "than", "over", "more", "most", "such", "only", "every", "each", "both", "may",
    "must", "should", "could", "would", "about", "across", "after", "before", "between", "because",
    "been", "being", "does", "doing", "had", "how", "than", "very", "just", "make", "made",
];

/// Every distinct identity key the enrichment references, for diagnostics.
///
/// It is used by the orchestration layer's log line and by tests that assert no
/// key was invented.
#[must_use]
pub fn referenced_keys(enrichment: &DeckEnrichment) -> BTreeSet<&str> {
    let mut keys = BTreeSet::new();
    for key in enrichment.summaries.keys() {
        keys.insert(key.as_str());
    }
    for (key, findings) in &enrichment.concept_findings {
        keys.insert(key.as_str());
        for finding in findings {
            keys.insert(finding.as_str());
        }
    }
    keys
}

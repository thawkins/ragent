//! Deterministic, LLM-free diff deck for a research item (spec `diffdeck`).
//!
//! A research item accumulates one dated report per run
//! (`RESEARCH-<YYYY-MM-DD>.md`). This module holds the **pure, in-memory**
//! stages of the `/research deck` feature that turn a pair of those reports into
//! a slide deck:
//!
//! 1. [`parse_report`] splits a report's text into its recognised top-level
//!    sections and each section's item units (FR-003, FR-009, FR-019). Every
//!    element is split into the unit its row of the spec's element-to-unit
//!    mapping names - a paragraph for the Executive Summary, a
//!    `### <n>. <concept>` block for Concepts, a `### **Finding N** -` block for
//!    Findings, a bullet for the Cross-References list, and a `[#N]` table entry
//!    for References - and is keyed by the finding headline, the concept name,
//!    or its own normalised text, so it can be matched against the other
//!    report. [`normalize_identity`] defines that key: citations, Markdown
//!    emphasis, backticks and whitespace runs are removed and the text is
//!    lower-cased, so a cosmetically drifted item still matches itself.
//! 2. [`ReportSeries`] classifies an item directory's file names and selects the
//!    pair of reports a run covers - the two most recent by default, or the pair
//!    an explicit `--from` / `--to` names, refusing when the item cannot supply
//!    two reports (FR-006, FR-007, FR-011, FR-012, FR-020). It is the one stage
//!    that reasons about file *names*; it still reads no file.
//! 3. [`ElementIndex`] keys one section's items by that identity so the later
//!    diff stage matches two reports by hash lookup rather than a nested scan
//!    (NFR-001), then [`diff_reports`] assigns every item of the newer report its
//!    one FR-004 status (`NEW` / `ADDED` / `UPDATE` / `OLD`), and [`render_deck`]
//!    emits the slide document: a cover slide, one `---`-separated slide per
//!    changed element in FR-003 precedence carrying every item of that element
//!    (and, for Concepts, one slide per concept with the findings ranked under
//!    it, FR-026), the FR-013 no-changes notice in place of element slides, and
//!    a closing summary slide (FR-003, FR-005, FR-008, FR-010, FR-013).
//!
//! The deck is a pure function of the two input report texts and the flag set:
//! no network request, no LLM call and no ambient state enter this module
//! (FR-021, NFR-002, NFR-004), which is why every function here takes and
//! returns owned or borrowed values only.
//!
//! ## Pure entry points
//!
//! The two stages NFR-004 names are public functions over in-memory values, so
//! a test (or a caller) can run them with no filesystem access at all:
//!
//! - [`diff_reports`] takes two already-parsed reports and returns the
//!   [`DeckDiff`] - the per-element status assignment, and
//! - [`render_deck`] takes that [`DeckDiff`] plus a [`DeckRenderOptions`] and
//!   returns the finished [`DeckDocument`] text.
//!
//! [`parse_report`] sits in front of both ([`DeckDiff`] is built from two
//! [`ParsedReport`]s), and [`ReportSeries`] is the only other stage that looks
//! at anything resembling a file - and even it reasons about supplied *file
//! names*, never file contents.
//!
//! ## Dependencies
//!
//! Standard library, `chrono` and `thiserror`, plus three sibling modules of
//! this crate:
//!
//! - [`crate::document`] for the report section names and the finding headline
//!   extraction the rendered report uses ([`crate::document::make_headline_from_observation`]).
//! - [`crate::io`] for the dated-report file-name contract
//!   (`RESEARCH-<YYYY-MM-DD>.md` and the legacy undated name) that
//!   [`ReportSeries`] classifies, so the deck and the writer cannot disagree
//!   about which file is which report.
//! - `crate::item::strip_control_chars` so a parsed item never carries a
//!   control character that could break the deck's Markdown structure (FR-022).
//!
//! Nothing here reaches the network, an LLM provider, the configuration or the
//! database; the caller ([`crate::manager::ResearchManager::deck`]) supplies
//! every input, including the generation date stamp.
//!
//! ## Safety and errors (NFR-003)
//!
//! The module contains no `unsafe` code (the crate root carries
//! `#![deny(unsafe_code)]`) and no `unwrap()`/`expect()` on a production path.
//! Every fallible step returns a typed [`thiserror::Error`] result -
//! [`DeckParseError`], [`DeckSelectError`] and [`DeckWriteError`] - and the
//! `unwrap_or` calls that remain are deliberate fallbacks for an optional value
//! (a missing heading line, an absent table cell, an undated legacy report),
//! not error paths.
//!
//! The parser is deliberately tolerant: a section heading it does not
//! recognise is recorded as a [`ParsedReport::warnings`] entry and skipped, so a
//! report whose headings drifted in an older run still parses. A report with
//! *no* recognised section at all cannot produce a deck and is refused
//! ([`DeckParseError`], FR-019).

use crate::deck_pptx::DeckPptxOutcome;
use crate::document::make_headline_from_observation;
use crate::io::{LEGACY_RESEARCH_FILE_NAME, dated_research_file_name, is_dated_research_file_name};
use crate::item::strip_control_chars;
use chrono::NaiveDate;
use std::collections::HashMap;
use std::fmt::Write as _;
use thiserror::Error;

/// The six major elements of a research report that can carry a deck slide.
///
/// The variant order is the **FR-003 element precedence**: element slides are
/// emitted as `ExecutiveSummary`, `Concepts`, `Findings`, `CrossReferences`,
/// `References`, and never in any other order. `PartialOrd`/`Ord` are derived,
/// so sorting by the enum's natural order yields exactly that precedence.
///
/// `Concepts` and `Findings` are the two elements the deck renders as its
/// topic structure (FR-026): one slide per concept, each listing the findings
/// that belong to it. The report's `Top Implications` and `Open Questions` are
/// not deck elements - they are reader-facing narrative the deck deliberately
/// leaves out - so neither has a variant and neither can be selected with
/// `--elements`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeckElement {
    /// `## Executive Summary` - one paragraph unit.
    ExecutiveSummary,
    /// `## Concepts` - one `### N. <concept>` block per concept.
    Concepts,
    /// `## Findings` - one `### **Finding N** - <headline>` block per finding.
    Findings,
    /// `## In-Project Cross-References` - one bullet item per cross-reference.
    CrossReferences,
    /// `## References Index` - one `| [#N] | ... |` table entry per source.
    References,
}

impl DeckElement {
    /// Every element, in the FR-003 precedence order.
    pub const ALL: [Self; 5] = [
        Self::ExecutiveSummary,
        Self::Concepts,
        Self::Findings,
        Self::CrossReferences,
        Self::References,
    ];

    /// The human-readable element name used on a slide heading.
    ///
    /// This is the name the FR-014 `--elements` filter accepts, matched
    /// case-insensitively against it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ExecutiveSummary => "Executive Summary",
            Self::Concepts => "Concepts",
            Self::Findings => "Findings",
            Self::CrossReferences => "Cross-References",
            Self::References => "References",
        }
    }

    /// The report heading this element is extracted from (the
    /// element-to-unit mapping table in the spec).
    ///
    /// `Findings` and `Concepts` are matched by name because the same sections
    /// are rendered as `## Findings` / `## Concepts` in the report layout and
    /// as `### Findings` / `### Concepts` sub-sections of `## Results` in the
    /// `IMRaD` layout.
    #[must_use]
    pub const fn heading(self) -> &'static str {
        match self {
            Self::ExecutiveSummary => "Executive Summary",
            Self::Concepts => "Concepts",
            Self::Findings => "Findings",
            Self::CrossReferences => "In-Project Cross-References",
            Self::References => "References Index",
        }
    }

    /// Resolve an element from a heading or a user-supplied element name.
    ///
    /// The match is case-insensitive over both the element label
    /// ([`Self::label`]) and the report heading ([`Self::heading`]) and accepts
    /// hyphen/space/underscore spellings, so `"cross references"`,
    /// `"Cross-References"` and `"In-Project Cross-References"` all name
    /// [`Self::CrossReferences`]. Returns `None` for an unknown name, which is
    /// what the FR-014 filter rejection reports.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let wanted = normalise_name(name);
        if wanted.is_empty() {
            return None;
        }
        Self::ALL.into_iter().find(|element| {
            normalise_name(element.label()) == wanted || normalise_name(element.heading()) == wanted
        })
    }

    /// Whether a report heading names this element.
    fn matches_heading(self, heading: &str) -> bool {
        let heading = normalise_name(heading);
        match self {
            // `## Findings` in the report layout, `### Findings` under
            // `## Results` in the IMRaD layout.
            Self::Findings => heading == "findings" || heading == "results findings",
            // `## Concepts` in the report layout, `### Concepts` under
            // `## Results` in the IMRaD layout.
            Self::Concepts => heading == "concepts" || heading == "results concepts",
            _ => heading == normalise_name(self.heading()),
        }
    }
}

/// Lower-case a name, collapsing every run of non-alphanumeric characters to a
/// single space so label spellings compare equal.
fn normalise_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut pending_space = false;
    for ch in name.trim().chars() {
        if ch.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.extend(ch.to_lowercase());
        } else {
            pending_space = true;
        }
    }
    out
}

/// One extracted item unit of a report section.
///
/// The `key` is the section-scoped identity used to match the item between two
/// reports (FR-009). It is produced by [`normalize_identity`]: the extracted
/// finding headline for [`DeckElement::Findings`], the concept name for
/// [`DeckElement::Concepts`], the normalised source URL for
/// [`DeckElement::References`], and the item's own normalised text otherwise -
/// with `[#N]` citations, Markdown emphasis, backticks and whitespace runs
/// removed, so a renumbered citation or an added emphasis does not read as a
/// changed item. `text` keeps the item's readable content so a later stage can
/// condense it to a bullet and detect a changed `UPDATE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckItem {
    /// The section this item belongs to.
    pub element: DeckElement,
    /// Section-scoped identity key (see the struct docs).
    pub key: String,
    /// The item's readable content, control characters stripped.
    pub text: String,
    /// The `Path/URL` column of a [`DeckElement::References`] row, when the item
    /// is a reference; `None` for every other element.
    ///
    /// The reference key is derived from it, and the FR-004 source-set
    /// comparison reads it.
    pub source_url: Option<String>,
    /// The set of sources the item cites, as normalised identities.
    ///
    /// FR-004 promotes a newer-only item to `ADDED` when it "has at least one
    /// source the older report did not cite", so the diff needs each item's own
    /// source set. A finding's set is read from its `**Sources:**` block (one
    /// `[#N]` bullet per source, keyed by URL); a reference's set is its own
    /// `source_url`; every other element carries no per-item source list, so its
    /// set is empty and the item can only ever be `NEW`.
    pub sources: Vec<String>,
    /// The item's supporting prose, folded out of the report's labelled blocks.
    ///
    /// A finding rendered by the report as
    /// `### **Finding N** - <headline>` followed by `**Observation:**`,
    /// `**Analysis:**` and `**Implication:**` paragraphs carries those three
    /// paragraphs here (in that order); every other item carries none. The
    /// deck's enrichment stage condenses them into the one paragraph the
    /// finding slide shows, and the deterministic fallback renders their first
    /// sentences, so a deck built without a model still explains the finding.
    pub detail: Vec<String>,
}

impl DeckItem {
    /// The single paragraph that describes this item on a slide.
    ///
    /// A finding's paragraph is written by [`crate::deck_enrich::enrich_deck`]
    /// when a model is wired in; every other item (and any finding the
    /// enrichment stage left alone) falls back to
    /// [`Self::deterministic_summary`].
    #[must_use]
    pub fn deterministic_summary(&self) -> String {
        self.detail
            .iter()
            .filter_map(|paragraph| {
                let sentence = first_sentence(&collapse_whitespace(paragraph));
                let sentence = sentence.trim().to_string();
                if sentence.is_empty() || !sentence.chars().any(|c| c.is_alphanumeric()) {
                    None
                } else {
                    Some(sentence)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// A report section recognised by [`parse_report`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckSection {
    /// Which element this section feeds.
    pub element: DeckElement,
    /// The heading text as it appeared in the report (without the `## `).
    pub heading: String,
    /// The section's item units, in document order.
    pub items: Vec<DeckItem>,
}

/// One report parsed into the sections the deck can consume.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedReport {
    /// The report title from the `# Title: <title>` line, when present.
    pub title: Option<String>,
    /// The recognised sections, ordered by [`DeckElement`] precedence.
    pub sections: Vec<DeckSection>,
    /// Non-fatal observations about the report, surfaced by the render stage.
    ///
    /// A warning names an unknown top-level section that was skipped or an
    /// element heading that appeared more than once (only the first is used).
    pub warnings: Vec<String>,
}

impl ParsedReport {
    /// The items of `element`, or an empty slice when the section is absent.
    #[must_use]
    pub fn items(&self, element: DeckElement) -> &[DeckItem] {
        self.sections
            .iter()
            .find(|section| section.element == element)
            .map_or(&[], |section| section.items.as_slice())
    }

    /// Whether the report carried at least one recognised section.
    #[must_use]
    pub fn has_recognised_section(&self) -> bool {
        !self.sections.is_empty()
    }
}

/// Why a report text could not be parsed into a deck-consumable report.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DeckParseError {
    /// The report contains no recognised top-level section (FR-019).
    #[error("no recognised top-level section (expected one of: {expected})")]
    NoRecognisedSection {
        /// The accepted element headings, comma separated.
        expected: String,
    },
}

/// Split a report text into its recognised sections and their item units.
///
/// The parser walks the text line by line, collecting `## ` headings and the
/// body that follows each one, and splits that body into the section's item
/// units per the spec's element-to-unit mapping:
///
/// | Element           | Item unit                                     |
/// | ----------------- | --------------------------------------------- |
/// | Executive Summary | the paragraph                                 |
/// | Top Implications  | one numbered item                             |
/// | Open Questions    | one bullet item                               |
/// | Findings          | one `### ` `**Finding N**` block per finding    |
/// | Cross-References  | one bullet item, or one table data row         |
/// | References        | one `[#N]` table entry per source              |
///
/// A `## ` heading the parser does not recognise is skipped and recorded in
/// [`ParsedReport::warnings`] (the report's own non-element sections, such as
/// `## Topic` and `## Concepts`, are expected and produce no warning). A `### `
/// sub-heading stays inside the section that opened it, and a `### Findings`
/// under `## Results` opens the findings section in the `IMRaD` layout. When no
/// section is recognised at all the report cannot feed a deck and is refused
/// with [`DeckParseError::NoRecognisedSection`] (FR-019) - the caller names the
/// offending file path.
///
/// # Errors
///
/// Returns [`DeckParseError::NoRecognisedSection`] when the text carries no
/// recognised top-level section.
pub fn parse_report(text: &str) -> Result<ParsedReport, DeckParseError> {
    let text = strip_control_chars(text);
    let mut parsed = ParsedReport::default();
    let mut seen: Vec<DeckElement> = Vec::new();

    // The heading currently being accumulated, and its body lines.
    let mut current: Option<(DeckElement, String)> = None;
    let mut body: Vec<&str> = Vec::new();

    for raw_line in text.lines() {
        if let Some(heading) = top_level_heading(raw_line) {
            if let Some((element, heading_text)) = current.take() {
                push_section(&mut parsed, &mut seen, element, &heading_text, &body);
                body.clear();
            }
            match DeckElement::ALL
                .into_iter()
                .find(|element| element.matches_heading(&heading))
            {
                Some(element) => current = Some((element, heading.to_string())),
                None => {
                    // Known report sections the deck does not consume are not
                    // drift; only a genuinely unrecognised heading is a warning.
                    if !is_known_non_element_heading(&heading) {
                        parsed
                            .warnings
                            .push(format!("unknown top-level section '## {heading}' ignored"));
                    }
                }
            }
            continue;
        }
        if current.is_some() {
            // A `### ` sub-heading that names a *different* element closes the
            // enclosing section and opens its own. The reports render
            // `### Open Questions` inside `## Top 10 Implications`, so keeping
            // every sub-heading in the enclosing body swallowed the element
            // whole: the questions became trailing lines of the last implication.
            if is_sub_heading(raw_line) {
                if let Some(heading) = sub_heading_text(raw_line) {
                    if let Some(element) = nested_element_heading(&heading) {
                        let opens_new = current
                            .as_ref()
                            .is_some_and(|(enclosing, _)| *enclosing != element);
                        if opens_new {
                            if let Some((enclosing, heading_text)) = current.take() {
                                push_section(
                                    &mut parsed,
                                    &mut seen,
                                    enclosing,
                                    &heading_text,
                                    &body,
                                );
                                body.clear();
                            }
                            current = Some((element, heading));
                            continue;
                        }
                    }
                }
            }
            // Any other `### ` sub-heading stays in the enclosing section's
            // body: the Findings blocks live under one.
            body.push(raw_line);
            continue;
        }
        if is_sub_heading(raw_line) {
            // `### Findings` under `## Results` in the IMRaD layout opens the
            // findings section even though its parent is not an element.
            if let Some(heading) = sub_heading_text(raw_line) {
                if let Some(element) = nested_element_heading(&heading) {
                    current = Some((element, heading));
                }
            }
            continue;
        }
        if let Some(title) = title_line(raw_line) {
            parsed.title = Some(title);
        }
    }
    if let Some((element, heading_text)) = current.take() {
        push_section(&mut parsed, &mut seen, element, &heading_text, &body);
    }

    if parsed.sections.is_empty() {
        let expected = DeckElement::ALL
            .iter()
            .map(|element| element.heading())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(DeckParseError::NoRecognisedSection { expected });
    }

    // FR-003: sections are ordered by the fixed element precedence, never by
    // their position in the report.
    parsed.sections.sort_by_key(|section| section.element);
    Ok(parsed)
}

/// Return the heading text of a `## ` line (excluding `###`+ and the hashes).
fn top_level_heading(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("## ")?;
    Some(rest.trim())
}

/// Whether a line opens a `### ` or deeper sub-heading.
fn is_sub_heading(line: &str) -> bool {
    line.starts_with("### ")
}

/// Return the heading text of a `### ` sub-heading line.
fn sub_heading_text(line: &str) -> Option<String> {
    let rest = line.strip_prefix("### ")?;
    let rest = rest.trim();
    (!rest.is_empty()).then(|| rest.to_string())
}

/// The element a `### ` sub-heading opens, when it names one.
///
/// The report layout renders `### Open Questions` inside `## Top 10
/// Implications`, and the `IMRaD` layout renders `### Findings` under
/// `## Results`, so a sub-heading that names an element opens that element's
/// section whether or not its parent is itself an element.
fn nested_element_heading(heading: &str) -> Option<DeckElement> {
    DeckElement::ALL
        .into_iter()
        .find(|element| element.matches_heading(heading))
}

/// Return the title of a `# Title: <title>` line, or a bare `# <title>` line.
fn title_line(line: &str) -> Option<String> {
    let rest = line.strip_prefix("# ")?;
    if rest.starts_with('#') {
        return None;
    }
    let title = rest
        .trim()
        .strip_prefix("Title:")
        .map_or(rest.trim(), |stripped| stripped.trim());
    (!title.is_empty()).then(|| title.to_string())
}

/// Report headings that are legitimate but carry no deck element, so their
/// presence is not reported as heading drift.
const KNOWN_NON_ELEMENT_HEADINGS: &[&str] = &[
    "topic",
    "research brief",
    "search queries",
    "search engine summary",
    "search provider requests",
    "data quality consistency",
    "concepts",
    "findings relationship diagram",
    "evidence digest",
    "triple draft",
    "gap fill fetch",
    "surgical patch",
    "citation check",
    "polish",
    "readability audit",
    "corpus quality scoreboard",
    "abstract",
    "introduction",
    "methods",
    "results",
    "discussion",
];

/// Whether a heading names a report section the deck deliberately ignores.
fn is_known_non_element_heading(heading: &str) -> bool {
    let heading = normalise_name(heading);
    KNOWN_NON_ELEMENT_HEADINGS.contains(&heading.as_str())
}

/// Extract the item units of one section body and record the section.
fn push_section(
    parsed: &mut ParsedReport,
    seen: &mut Vec<DeckElement>,
    element: DeckElement,
    heading: &str,
    body: &[&str],
) {
    if seen.contains(&element) {
        // The first occurrence wins; a duplicated heading is reported rather
        // than merged so the deck cannot silently double-count an element.
        parsed.warnings.push(format!(
            "duplicate section '## {heading}' ignored (first '{}' section used)",
            element.label()
        ));
        return;
    }
    seen.push(element);
    let items = extract_items(element, body);
    parsed.sections.push(DeckSection {
        element,
        heading: heading.to_string(),
        items,
    });
}

/// Extract one section body's item units per the element-to-unit mapping
/// (FR-009).
///
/// This is the only place that decides *which* unit kind a section is split
/// into, so the mapping table and the parser cannot drift:
///
/// | Element           | Unit                                      |
/// | ----------------- | ----------------------------------------- |
/// | Executive Summary | paragraph                                 |
/// | Concepts          | `### N. <concept>` block                  |
/// | Findings          | `### **Finding N** -` block               |
/// | Cross-References  | bullet item, or one table data row        |
/// | References        | `[#N]` reference-table entry              |
fn extract_items(element: DeckElement, body: &[&str]) -> Vec<DeckItem> {
    match element {
        DeckElement::ExecutiveSummary => extract_paragraphs(element, body),
        DeckElement::Concepts => extract_concepts(body),
        DeckElement::CrossReferences => extract_bullets(element, body),
        DeckElement::Findings => extract_findings(body),
        DeckElement::References => extract_references(body),
    }
}

/// Build a [`DeckItem`], dropping an item whose content is empty or is one of
/// the report's `_(no ... yet)_` placeholders.
fn make_item(element: DeckElement, text: &str) -> Option<DeckItem> {
    let text = text.trim();
    if text.is_empty() || is_placeholder(text) {
        return None;
    }
    if element == DeckElement::Findings {
        return make_finding_item(text);
    }
    Some(DeckItem {
        element,
        key: item_key(element, text),
        text: text.to_string(),
        source_url: None,
        sources: citation_markers(text),
        detail: Vec::new(),
    })
}

/// The report citation numbers `[#N]` an item's text carries (FR-004).
///
/// FR-004 promotes an unmatched item to `ADDED` when it cites a source the older
/// report did not, which is only observable when the prose and listicle elements
/// record the citations their text actually carries: the `**Sources:**` block is
/// a Findings block, and an Executive Summary paragraph, a concept or a
/// cross-reference that gained `[#12]` is otherwise indistinguishable from one
/// that did not, so `ADDED` could never be assigned.
///
/// The marker spelling matches what the reports and
/// [`strip_citations`] use: `[#N]` with one or more digits. Returns the
/// de-duplicated numbers in first-appearance order, so the result is stable
/// across runs (NFR-003).
fn citation_markers(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut idx = 0;
    while idx < chars.len() {
        if chars[idx] != '[' {
            idx += 1;
            continue;
        }
        let mut cursor = idx + 1;
        if cursor < chars.len() && chars[cursor] == '#' {
            cursor += 1;
        } else {
            idx += 1;
            continue;
        }
        let digits_start = cursor;
        while cursor < chars.len() && chars[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor > digits_start && cursor < chars.len() && chars[cursor] == ']' {
            let marker: String = chars[digits_start..cursor].iter().collect();
            if !out.contains(&marker) {
                out.push(marker);
            }
            idx = cursor + 1;
        } else {
            idx += 1;
        }
    }
    out
}

/// Whether a section body line is one of the report's placeholder notices.
fn is_placeholder(text: &str) -> bool {
    text.starts_with("_(") && text.ends_with(")_")
}

/// The section-scoped identity of an item (FR-009).
///
/// Findings are keyed by their extracted headline, concepts by their declared
/// name, references by their normalised source URL (set by
/// [`extract_references`] once the row is built); every other element is keyed
/// by its normalised text. See [`normalize_identity`].
fn item_key(element: DeckElement, text: &str) -> String {
    match element {
        DeckElement::Findings => {
            let heading = text.lines().next().unwrap_or(text);
            debug_assert!(
                !heading.is_empty(),
                "a parsed finding always has a headline"
            );
            normalize_identity(element, &strip_finding_number(heading))
        }
        DeckElement::Concepts => {
            let name = text.lines().next().unwrap_or(text);
            normalize_identity(element, name)
        }
        DeckElement::References => normalize_identity(element, text),
        DeckElement::ExecutiveSummary | DeckElement::CrossReferences => {
            normalize_identity(element, text)
        }
    }
}

/// The section-scoped normalised identity an item is matched on between two
/// reports (FR-009, NFR-001).
///
/// The identity is what makes two runs of an item *the same item* even when the
/// rendered text drifted cosmetically. For every element it:
///
/// 1. strips report noise - `[#N]` citation markers, Markdown emphasis (`**`,
///    `__`, `*`, `_`) and backticks - so a renumbered citation, an added bold
///    run or a code span does not read as a change;
/// 2. collapses every whitespace run to one space (a wrapped paragraph and an
///    unwrapped one compare equal) and trims the ends;
/// 3. lower-cases the result.
///
/// The remaining per-element difference is the text handed in, which
/// `item_key` chooses: the extracted finding headline for
/// [`DeckElement::Findings`], the concept name for [`DeckElement::Concepts`],
/// the `Path/URL` column for [`DeckElement::References`], and the item's own
/// text otherwise.
///
/// The identity is computed once per item at parse time, so matching two reports
/// is a hash lookup per item rather than a nested scan (NFR-001).
///
/// ```
/// use ragent_research::deck::{DeckElement, normalize_identity};
///
/// assert_eq!(
///     normalize_identity(DeckElement::ExecutiveSummary, "Bold **claim** here [#12]"),
///     "bold claim here"
/// );
/// ```
#[must_use]
pub fn normalize_identity(element: DeckElement, text: &str) -> String {
    let text = strip_control_chars(text);
    let text = match element {
        // Citations belong to the prose elements; a finding headline, a concept
        // name or a source URL that happens to contain brackets keeps its own
        // brackets.
        DeckElement::Findings | DeckElement::Concepts | DeckElement::References => text.to_string(),
        DeckElement::ExecutiveSummary | DeckElement::CrossReferences => strip_citations(&text),
    };
    let text = collapse_whitespace(&strip_markdown_noise(&text));
    text.to_lowercase()
}

/// Remove every report citation marker (`[#N]`, and `[1]`-style numeric
/// citations) from `text`.
///
/// Only a bracketed run whose content is `#` plus digits, or digits alone, is a
/// citation; any other bracketed text (`[see note]`, `[^1]`) is left alone.
fn strip_citations(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut idx = 0;
    while idx < chars.len() {
        if chars[idx] != '[' {
            out.push(chars[idx]);
            idx += 1;
            continue;
        }
        let mut end = idx + 1;
        if end < chars.len() && chars[end] == '#' {
            end += 1;
        }
        let digits_start = end;
        while end < chars.len() && chars[end].is_ascii_digit() {
            end += 1;
        }
        let is_citation = end > digits_start && end < chars.len() && chars[end] == ']';
        if is_citation {
            // The marker is dropped whole; the space that surrounded it is
            // collapsed by the caller.
            idx = end + 1;
        } else {
            out.push(chars[idx]);
            idx += 1;
        }
    }
    out
}

/// Remove Markdown emphasis and code-span markers from `text`.
///
/// `**`, `__`, `~~`, `*`, `_` and the backtick are dropped, which turns
/// `**Work stealing**` into `Work stealing` and `` `io_uring` `` into `io_uring`.
/// Emphasis inside a word (`foo_bar_baz`) loses its underscore too: the identity
/// is a matching key, not the rendered text, so a stricter rule buys nothing.
///
/// `pub(crate)` because the PowerPoint plan renders item text straight to a
/// slide, where a literal `###` or a backticked tag is exactly the noise this
/// removes.
pub(crate) fn strip_markdown_noise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if matches!(ch, '*' | '`' | '~') {
            // A doubled marker (`**`, `~~`) is consumed as a pair.
            if chars.peek() == Some(&ch) {
                chars.next();
            }
            continue;
        }
        if ch == '_' {
            continue;
        }
        out.push(ch);
    }
    out
}

/// A section's items indexed by their normalised identity (NFR-001).
///
/// Matching two reports element by element is then one hash lookup per item
/// instead of a nested linear scan, so the work is linear in the item count.
/// The index is **section-scoped**: it is built for one [`DeckElement`] of one
/// report, so an item can never match an item of a different element.
///
/// `ElementIndex::matching` returns the identity string the indexed side carried,
/// so the diff stage can decide `UPDATE` by comparing the two normalised
/// identities (FR-004) without re-normalising either.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ElementIndex {
    /// Which element's items this index holds.
    element: Option<DeckElement>,
    /// Identity -> normalised identity of the item that claimed it.
    entries: HashMap<String, String>,
}

impl ElementIndex {
    /// Build the index over one section's items.
    ///
    /// A repeated identity (the report listed the same item twice) keeps the
    /// **first** occurrence, matching the report's own first-one-wins rule.
    #[must_use]
    pub fn new(element: DeckElement, items: &[DeckItem]) -> Self {
        let mut entries = HashMap::with_capacity(items.len());
        for item in items {
            entries
                .entry(item.key.clone())
                .or_insert_with(|| normalize_identity(element, &item.text));
        }
        Self {
            element: Some(element),
            entries,
        }
    }

    /// The element this index was built for, or `None` for an empty index.
    #[must_use]
    pub const fn element(&self) -> Option<DeckElement> {
        self.element
    }

    /// The number of distinct identities in the index.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index holds no item.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up `item`'s identity, returning the identity string the indexed side
    /// carried for it.
    ///
    /// The item must belong to this index's element; an item of a different
    /// element never matches, because the index is section-scoped.
    #[must_use]
    pub fn matching(&self, item: &DeckItem) -> Option<&str> {
        if self.element != Some(item.element) {
            return None;
        }
        self.entries.get(&item.key).map(String::as_str)
    }
}

/// The status tag the diff assigns to one item (FR-004).
///
/// The set is closed and every rendered item carries exactly one tag.
///
/// - [`ItemStatus::New`] - the item exists only in the newer report;
/// - [`ItemStatus::Added`] - the item exists only in the newer report **and**
///   cites at least one source the older report did not cite;
/// - [`ItemStatus::Update`] - the item exists in both reports with changed
///   condensed text;
/// - [`ItemStatus::Old`] - the item exists in both reports with unchanged
///   condensed text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemStatus {
    /// Only in the newer report.
    New,
    /// Only in the newer report, with a source the older report did not cite.
    Added,
    /// In both reports, with changed condensed text.
    Update,
    /// In both reports, unchanged.
    Old,
}

impl ItemStatus {
    /// The status tag exactly as the deck renders it.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::New => "NEW",
            Self::Added => "ADDED",
            Self::Update => "UPDATE",
            Self::Old => "OLD",
        }
    }

    /// Whether the status counts as a change for slide emission (FR-008) and
    /// for the [`DeckDiff::has_changes`] test FR-013 branches on.
    #[must_use]
    pub const fn is_change(self) -> bool {
        matches!(self, Self::New | Self::Added | Self::Update)
    }
}

/// One item of the newer report together with the status the diff assigned it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffedItem {
    /// The newer report's item.
    pub item: DeckItem,
    /// Its status tag (FR-004).
    pub status: ItemStatus,
    /// The condensed bullet the deck renders (FR-009).
    pub bullet: String,
}

/// The diffed items of one element.
///
/// The section carries **every** item of the element from both reports, in the
/// newer report's document order, with the older-only items (`NEW`'s mirror -
/// items present only in the baseline - are not rendered by FR-008, so they are
/// not in `items`; see [`ElementDiff::removed`]) split out.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ElementDiff {
    /// Which element this section covers.
    pub element: Option<DeckElement>,
    /// The newer report's items, each with its status, in document order.
    pub items: Vec<DiffedItem>,
    /// Items present only in the older report, in the older report's order.
    ///
    /// FR-004 defines no status for a removed item - the closed tag set covers
    /// the newer report's items - so these are reported separately rather than
    /// tagged, and the renderer omits them (FR-008 lists the newer report's
    /// items). Keeping them lets a caller report what disappeared.
    pub removed: Vec<DeckItem>,
}

impl ElementDiff {
    /// The element this diff covers, or `None` when it carries no item.
    #[must_use]
    pub const fn element(&self) -> Option<DeckElement> {
        self.element
    }

    /// The items carrying `status`.
    pub fn with_status(&self, status: ItemStatus) -> impl Iterator<Item = &DiffedItem> {
        self.items
            .iter()
            .filter(move |entry| entry.status == status)
    }

    /// The per-status count of the items in `status`.
    #[must_use]
    pub fn count(&self, status: ItemStatus) -> usize {
        self.with_status(status).count()
    }

    /// Whether the element yielded at least one `NEW`, `ADDED` or `UPDATE` item.
    ///
    /// FR-008 emits a slide exactly when this is true; FR-015 adds a slide for a
    /// no-change element only under `--include-old`.
    #[must_use]
    pub fn has_changes(&self) -> bool {
        self.items.iter().any(|entry| entry.status.is_change())
    }

    /// Whether the element carries no newer-report item at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// The whole difference between two parsed reports, one [`ElementDiff`] per
/// element that the pair contributes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeckDiff {
    /// One entry per element, in FR-003 precedence order, holding only the
    /// elements that carry at least one item in either report.
    pub elements: Vec<ElementDiff>,
}

impl DeckDiff {
    /// The diff for `element`, or `None` when neither report carried it.
    #[must_use]
    pub fn element(&self, element: DeckElement) -> Option<&ElementDiff> {
        self.elements
            .iter()
            .find(|entry| entry.element == Some(element))
    }

    /// Whether every element is unchanged, which is the FR-013 no-changes case.
    #[must_use]
    pub fn has_changes(&self) -> bool {
        self.elements.iter().any(ElementDiff::has_changes)
    }

    /// The elements that produced a change, in FR-003 precedence order.
    pub fn changed_elements(&self) -> impl Iterator<Item = DeckElement> + '_ {
        self.elements
            .iter()
            .filter(|entry| entry.has_changes())
            .filter_map(ElementDiff::element)
    }

    /// The total count of items carrying `status` across every element.
    #[must_use]
    pub fn count(&self, status: ItemStatus) -> usize {
        self.elements.iter().map(|entry| entry.count(status)).sum()
    }

    /// The total count of items in the diff, of every status.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.elements.iter().map(|entry| entry.items.len()).sum()
    }
}

/// Diff two parsed reports, assigning every item of the newer report exactly one
/// [`ItemStatus`] under the FR-004 definitions (FR-004, FR-008, FR-013).
///
/// The comparison is per element and never crosses an element boundary: a
/// finding and a cross-reference with the same text are different items, because
/// [`ElementIndex`] is section-scoped (FR-009).
///
/// For each element the newer report's items are matched against the older
/// report's index by normalised identity:
///
/// - **no match** - the item is [`ItemStatus::Added`] when it cites at least one
///   source the older report did not cite, and [`ItemStatus::New`] otherwise.
///   The comparison is against the *older report's whole reference set*, not just
///   the matched item's own sources, so a finding that existed under a different
///   identity is not what makes a source "new" - FR-004 asks whether the older
///   report cited the source at all;
/// - **match, unchanged identity** - [`ItemStatus::Old`];
/// - **match, changed identity** - [`ItemStatus::Update`].
///
/// The older report's items that the newer report does not match are collected
/// in [`ElementDiff::removed`].
///
/// ```
/// use ragent_research::deck::{ItemStatus, diff_reports, parse_report};
///
/// let older = parse_report("## Executive Summary\n\nIt will ship.\n").unwrap();
/// let newer = parse_report("## Executive Summary\n\nIt will ship.\n").unwrap();
/// assert_eq!(diff_reports(&older, &newer).count(ItemStatus::Old), 1);
/// ```
#[must_use]
pub fn diff_reports(older: &ParsedReport, newer: &ParsedReport) -> DeckDiff {
    // The older report's cited sources, across every element: FR-004 asks
    // whether the older report cited a source *at all*, so a finding's sources
    // count as cited even when the matched item was a reference row.
    let older_sources = cited_sources(older);
    let mut elements = Vec::new();
    for element in DeckElement::ALL {
        let older_items = older.items(element);
        let newer_items = newer.items(element);
        if older_items.is_empty() && newer_items.is_empty() {
            continue;
        }
        let index = ElementIndex::new(element, older_items);
        let mut items = Vec::with_capacity(newer_items.len());
        for item in newer_items {
            let status = match index.matching(item) {
                // Matched, and the two identities are the text the index carried
                // versus the text this item carries.
                Some(older_identity) => {
                    if older_identity == normalize_identity(element, &item.text) {
                        ItemStatus::Old
                    } else {
                        ItemStatus::Update
                    }
                }
                // Unmatched: `ADDED` exactly when it brings a source the older
                // report never cited.
                None => {
                    if item
                        .sources
                        .iter()
                        .any(|source| !older_sources.contains(source))
                    {
                        ItemStatus::Added
                    } else {
                        ItemStatus::New
                    }
                }
            };
            items.push(DiffedItem {
                bullet: condense_bullet(item),
                status,
                item: item.clone(),
            });
        }
        let newer_keys: Vec<&str> = newer_items.iter().map(|item| item.key.as_str()).collect();
        let removed: Vec<DeckItem> = older_items
            .iter()
            .filter(|item| !newer_keys.contains(&item.key.as_str()))
            .cloned()
            .collect();
        elements.push(ElementDiff {
            element: Some(element),
            items,
            removed,
        });
    }
    DeckDiff { elements }
}

/// The exact FR-013 notice a no-change deck carries instead of element slides.
pub const NO_CHANGES_NOTICE: &str = "_No changes detected between the two reports._";

/// The `---` rule that separates two slides (FR-003).
const SLIDE_RULE: &str = "---";

/// Default deck title when the newer report carries no `# Title:` line.
const UNTITLED_DECK: &str = "Untitled research deck";

/// A deck rendered from a [`DeckDiff`], ready to be written (FR-003, FR-005,
/// FR-010, FR-013).
///
/// The renderer is a pure function of the diff and its metadata: it formats, it
/// does not decide. Which elements produce a slide is FR-008's rule (a change,
/// or every item under `--include-old`), and the counts it reports are the
/// diff's own counts, so the summary can never disagree with the slides.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeckDocument {
    /// The document's text, slide for slide, as written to disk.
    pub markdown: String,
    /// The element slides the cover slide counts, in FR-003 precedence order.
    pub elements: Vec<DeckElement>,
}

/// How a deck is rendered (the metadata the pure render stage needs, FR-021).
///
/// Everything here is caller-supplied rather than read from the ambient
/// environment, so the same inputs always render the same bytes (NFR-002).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckRenderOptions {
    /// The research item name, shown on the cover slide (FR-005).
    pub name: String,
    /// The UTC date the deck is generated on (FR-005).
    ///
    /// It is a parameter rather than a clock read so the renderer stays a pure
    /// function; the orchestration stage supplies today's UTC date.
    pub generated_on: NaiveDate,
    /// The older report's stamp label, or `legacy` for `RESEARCH.md`.
    pub from: String,
    /// The newer report's stamp label.
    pub to: String,
    /// The research item title from the newer report's `# Title:` line.
    pub title: Option<String>,
    /// Whether a no-change element also produces a slide (FR-015 `--include-old`).
    pub include_old: bool,
    /// The newer report's finding items, in document order (FR-024).
    ///
    /// An implication slide lists the findings ranked behind it, which live in
    /// the Findings element of the same deck; carrying them here keeps
    /// [`render_element_slide`] a function of the options alone rather than of
    /// the diff's element ordering.
    pub findings: Vec<DeckItem>,
    /// The finding paragraphs and implication-to-finding mapping the slides
    /// render (FR-023, FR-024).
    ///
    /// It defaults to the empty enrichment, which is the model-free baseline: a
    /// finding slide then shows the first sentence of the finding's observation
    /// and analysis, and every finding stays on the Findings slide because no
    /// concept claimed it.
    pub enrichment: crate::deck_enrich::DeckEnrichment,
    /// The diffed Findings items, so a concept slide can render the findings
    /// grouped under it with their FR-004 status tags.
    ///
    /// It defaults to empty, which renders a concept's findings without a
    /// status tag rather than omitting them; [`render_deck`] fills it from the
    /// diff it was handed.
    pub diffed_findings: Vec<DiffedItem>,
}

impl DeckRenderOptions {
    /// Build the options a deck run reads off its selected report pair.
    ///
    /// The title is taken from the newer report and falls back to the older
    /// report's, so a baseline-only title still names the deck.
    #[must_use]
    pub fn new(
        name: &str,
        generated_on: NaiveDate,
        pair: &ReportPair,
        older: &ParsedReport,
        newer: &ParsedReport,
    ) -> Self {
        Self {
            name: name.to_string(),
            generated_on,
            from: pair.older_label().to_string(),
            to: pair.newer_label().to_string(),
            title: newer.title.clone().or_else(|| older.title.clone()),
            include_old: false,
            findings: newer.items(DeckElement::Findings).to_vec(),
            enrichment: crate::deck_enrich::DeckEnrichment::default(),
            diffed_findings: Vec::new(),
        }
    }

    /// The same options carrying the enrichment the deck was rendered from.
    ///
    /// The enrichment is a separate stage ([`crate::deck_enrich::enrich_deck`])
    /// because it can consult a model; keeping it out of `new` means the
    /// options a test builds are the deterministic baseline unless it says
    /// otherwise.
    #[must_use]
    pub fn with_enrichment(mut self, enrichment: crate::deck_enrich::DeckEnrichment) -> Self {
        self.enrichment = enrichment;
        self
    }

    /// The findings a concept slide can list, in the order they are ranked.
    ///
    /// The renderer holds no report of its own, so the newer report's finding
    /// items are carried here; the concept-to-findings mapping is the one
    /// [`crate::deck_enrich::DeckEnrichment::findings_for`] supplies.
    #[must_use]
    pub fn enrichment_findings(&self) -> Vec<DeckItem> {
        self.findings.clone()
    }

    /// The same options carrying the newer report's finding items, so a concept
    /// slide can list the findings grouped under it.
    #[must_use]
    pub fn with_findings(mut self, findings: Vec<DeckItem>) -> Self {
        self.findings = findings;
        self
    }

    /// The same options with FR-015 `--include-old` set.
    #[must_use]
    pub fn with_include_old(mut self, include_old: bool) -> Self {
        self.include_old = include_old;
        self
    }

    /// The diffed finding with identity key `key`, when the deck's diff carries
    /// one.
    ///
    /// A concept slide lists findings by their identity, and the diff is what
    /// carries each finding's FR-004 status tag, so the renderer resolves the
    /// key against the Findings entry it was handed rather than re-deriving a
    /// status. The caller supplies that entry through [`Self::with_diffed_findings`].
    #[must_use]
    pub fn diffed_finding(&self, key: &str) -> Option<&DiffedItem> {
        self.diffed_findings
            .iter()
            .find(|diffed| diffed.item.key == key)
    }

    /// The same options carrying the diffed Findings entry, so a concept slide
    /// can render its findings with their status tags.
    #[must_use]
    pub fn with_diffed_findings(mut self, diffed_findings: Vec<DiffedItem>) -> Self {
        self.diffed_findings = diffed_findings;
        self
    }
}

/// Render a diff as a Markdown slide deck (FR-003, FR-005, FR-008, FR-010,
/// FR-013).
///
/// The slides are, in this fixed order and separated by a `---` rule:
///
/// 1. **cover** - the title, the generation date, the coverage window
///    (`Coverage: <older> to <newer>`), the research id, and the element-slide,
///    change-item and unchanged-item counts (FR-005);
/// 2. **element slides**, one per element that changed, in [`DeckElement::ALL`]
///    precedence. Each carries **every** item of its element from both reports -
///    the changed items and the unchanged `OLD` items - so a slide is the whole
///    picture and not only its deltas (FR-008). Every item is tagged with its
///    [`ItemStatus::tag`] (FR-004). The Concepts element is the exception in
///    shape only: it emits one slide per concept, each listing the findings
///    grouped under that concept (FR-026), and the Findings slide then carries
///    only the findings no concept claimed;
/// 3. **the FR-013 notice** instead of step 2 when the pair changed nothing;
/// 4. **the summary** - the element-slide count, the per-status counts and the
///    names of the elements that produced a slide (FR-010).
///
/// With `include_old` an element that carries only `OLD` items also produces a
/// slide (FR-015). The function reads no clock and no environment, so it is a
/// pure function of its arguments (NFR-002, NFR-004).
#[must_use]
pub fn render_deck(diff: &DeckDiff, options: &DeckRenderOptions) -> DeckDocument {
    // A concept slide renders its findings with the status tags the diff
    // assigned, so the renderer takes them from the diff it was handed rather
    // than from a caller-supplied copy that could disagree with it.
    let mut owned;
    let options = if options.diffed_findings.is_empty() {
        owned = options.clone();
        owned.diffed_findings = diff
            .element(DeckElement::Findings)
            .map(|entry| entry.items.clone())
            .unwrap_or_default();
        &owned
    } else {
        options
    };
    let elements: Vec<DeckElement> = diff
        .elements
        .iter()
        .filter(|entry| entry.has_changes() || (options.include_old && !entry.is_empty()))
        .filter_map(ElementDiff::element)
        .collect();
    let old_count = diff.count(ItemStatus::Old);
    let change_count = diff.item_count() - old_count;

    let mut slides: Vec<String> = Vec::with_capacity(elements.len() + 2);
    slides.push(render_cover(
        options,
        elements.len(),
        change_count,
        old_count,
    ));
    for element in &elements {
        if let Some(entry) = diff.element(*element) {
            slides.push(render_element_slide(*element, entry, options));
        }
    }
    if elements.is_empty() {
        slides.push(render_no_changes());
    }
    slides.push(render_summary(diff, &elements));

    let mut markdown = String::new();
    markdown.push_str(&slides.join(&format!("\n{SLIDE_RULE}\n\n")));
    DeckDocument { markdown, elements }
}

/// Render the cover slide (FR-005).
fn render_cover(
    options: &DeckRenderOptions,
    element_count: usize,
    change_count: usize,
    old_count: usize,
) -> String {
    let title = options
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .unwrap_or(UNTITLED_DECK);
    let mut out = String::new();
    // `write!` to a String is infallible; the result is discarded rather than
    // unwrapped (NFR-003).
    let _ = writeln!(out, "# {title} - Research Diff Deck");
    let _ = writeln!(out);
    let _ = writeln!(out, "**Research id:** {}", options.name);
    let _ = writeln!(out, "**Generated:** {}", options.generated_on);
    let _ = writeln!(out, "**Coverage:** {} to {}", options.from, options.to);
    let _ = writeln!(out);
    let _ = writeln!(out, "| Metric | Count |");
    let _ = writeln!(out, "| ------ | ----- |");
    let _ = writeln!(out, "| Element slides | {element_count} |");
    let _ = writeln!(out, "| Change items | {change_count} |");
    let _ = writeln!(
        out,
        "| Unchanged ({}) items | {old_count} |",
        ItemStatus::Old.tag()
    );
    out
}

/// Render one element slide, carrying every item of the element (FR-008).
///
/// Two elements are rendered with their supporting prose:
///
/// - a **Concepts** element does not render as one slide: it renders one slide
///   per concept (see [`render_concept_slides`]), because the concept is the
///   deck's topic grouping (FR-026).
/// - a **Findings** item is its headline as a `###` sub-heading followed by the
///   one paragraph the enrichment supplied, or the finding's deterministic
///   fallback paragraph when it supplied none. A finding whose report block
///   carried no prose keeps its compact bullet, so a headline-only finding
///   still renders. A finding that a concept already lists is skipped here, so
///   every finding appears on the deck exactly once.
fn render_element_slide(
    element: DeckElement,
    entry: &ElementDiff,
    options: &DeckRenderOptions,
) -> String {
    if element == DeckElement::Concepts {
        return render_concept_slides(entry, options);
    }
    let mut out = String::new();
    let _ = writeln!(out, "## {}", element.label());
    let _ = writeln!(out);
    let _ = writeln!(out, "_{}_", element_counts(entry));
    let _ = writeln!(out);
    let claimed = options.enrichment.claimed_finding_keys();
    if element == DeckElement::Findings && !claimed.is_empty() {
        // The concepts already list the findings that belong to them, so this
        // slide carries only the findings no concept claimed. Its heading names
        // that: without it the slide would read as "Findings" and appear to have
        // lost the ones the concept slides now own.
        let _ = writeln!(out, "## {UNCLAIMED_FINDINGS_HEADING}");
        let _ = writeln!(out);
        let _ = writeln!(out, "_{}_", element_counts(entry));
        let _ = writeln!(out);
    }
    for diffed in &entry.items {
        match element {
            DeckElement::Findings if claimed.contains(diffed.item.key.as_str()) => {}
            DeckElement::Findings => render_finding_item(&mut out, diffed, options),
            _ => {
                let _ = writeln!(out, "- `{}` {}", diffed.status.tag(), diffed.bullet);
            }
        }
    }
    out
}

/// The heading of the Findings slide once the concepts carry the grouped
/// findings: it lists only what no concept claimed (FR-026).
const UNCLAIMED_FINDINGS_HEADING: &str = "Other Findings";

/// Render one slide per concept, each carrying its definition and its findings
/// (FR-026).
///
/// A concept slide is:
///
/// ```text
/// ## <Concept name>
///
/// _`<status>`_
///
/// <the concept's definition paragraph>
///
/// ### `NEW` <finding headline>
///
/// <the finding's paragraph>
/// ```
///
/// The findings are the newer report's Findings items as the diff carries them,
/// so each keeps its FR-004 status tag; a concept claims the findings the
/// enrichment ranked under it ([`crate::deck_enrich::DeckEnrichment::findings_for`]).
/// A concept with no ranked finding renders its definition and an explicit
/// `_No finding maps to this concept._` line, so an empty concept is visible
/// rather than silently omitted.
fn render_concept_slides(entry: &ElementDiff, options: &DeckRenderOptions) -> String {
    let mut out = String::new();
    for diffed in &entry.items {
        let _ = writeln!(out, "## {}", escape_markdown(diffed.item.text.trim()));
        let _ = writeln!(out);
        let _ = writeln!(out, "_`{}`_", diffed.status.tag());
        let _ = writeln!(out);
        let definition = concept_definition(&diffed.item);
        if !definition.is_empty() {
            let _ = writeln!(out, "{definition}");
            let _ = writeln!(out);
        }
        let findings = options
            .enrichment
            .findings_for(&diffed.item, &options.findings);
        if findings.is_empty() {
            let _ = writeln!(out, "_No finding maps to this concept._");
            let _ = writeln!(out);
        } else {
            for finding in findings {
                match options.diffed_finding(&finding.key) {
                    Some(slide) => render_finding_item(&mut out, slide, options),
                    // A finding the enrichment names but the diff does not carry
                    // (the enrichment describes the newer report, the diff only
                    // the pair) still lists by its headline.
                    None => {
                        let _ = writeln!(
                            out,
                            "- `{}` {}",
                            ItemStatus::Old.tag(),
                            escape_markdown(finding.text.trim())
                        );
                    }
                }
            }
        }
    }
    out
}

/// The paragraph a concept slide shows under the concept's name.
///
/// The report's `**Definition:**` block is the concept's meaning, so it leads;
/// when the block carried none, the concept's first prose paragraph stands in.
/// The result is one line and is empty for a concept that carries no prose.
fn concept_definition(item: &DeckItem) -> String {
    let paragraph = item
        .detail
        .first()
        .map(|block| collapse_whitespace(block))
        .map(|block| block.trim().to_string())
        .unwrap_or_default();
    escape_markdown(&paragraph)
}

/// Render one Findings item: its headline as a sub-heading, then its paragraph.
///
/// The paragraph is the enrichment's model-written summary when it has one and
/// the finding's deterministic fallback otherwise, so the slide always explains
/// the finding when the report carried prose for it. A finding with no prose at
/// all keeps the compact bullet form, which is what an older report's headline
/// only block renders as.
fn render_finding_item(out: &mut String, diffed: &DiffedItem, options: &DeckRenderOptions) {
    let headline = escape_markdown(diffed.item.text.trim());
    let paragraph = options.enrichment.summary_for(&diffed.item);
    match paragraph {
        Some(paragraph) => {
            let _ = writeln!(out, "### `{}` {headline}", diffed.status.tag());
            let _ = writeln!(out);
            let _ = writeln!(out, "{paragraph}");
            let _ = writeln!(out);
        }
        None => {
            let _ = writeln!(out, "- `{}` {}", diffed.status.tag(), diffed.bullet);
        }
    }
}

/// The per-status count line of an element slide.
fn element_counts(entry: &ElementDiff) -> String {
    let counts: Vec<String> = [
        ItemStatus::New,
        ItemStatus::Added,
        ItemStatus::Update,
        ItemStatus::Old,
    ]
    .into_iter()
    .map(|status| format!("{} {}", status.tag(), entry.count(status)))
    .collect();
    counts.join(" - ") + " - " + &format!("{} item(s)", entry.items.len())
}

/// Render the FR-013 notice slide that replaces the element slides.
fn render_no_changes() -> String {
    format!("## No changes\n\n{NO_CHANGES_NOTICE}\n")
}

/// Render the closing summary slide (FR-010).
fn render_summary(diff: &DeckDiff, elements: &[DeckElement]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "## Summary");
    let _ = writeln!(out);
    let _ = writeln!(out, "**Element slides:** {}", elements.len());
    let _ = writeln!(out);
    let _ = writeln!(out, "| Status | Items |");
    let _ = writeln!(out, "| ------ | ----- |");
    for status in [
        ItemStatus::New,
        ItemStatus::Added,
        ItemStatus::Update,
        ItemStatus::Old,
    ] {
        let _ = writeln!(out, "| {} | {} |", status.tag(), diff.count(status));
    }
    let _ = writeln!(out);
    if elements.is_empty() {
        let _ = writeln!(out, "**Elements:** none (no changes)");
    } else {
        let names: Vec<&str> = elements.iter().map(|element| element.label()).collect();
        let _ = writeln!(out, "**Elements:** {}", names.join(", "));
    }
    out
}

/// Every source identity either report cites, across all elements.
fn cited_sources(report: &ParsedReport) -> std::collections::HashSet<String> {
    report
        .sections
        .iter()
        .flat_map(|section| section.items.iter())
        .flat_map(|item| item.sources.iter().cloned())
        .collect()
}

/// Remove a leading `Finding N` label from a finding heading so the identity is
/// the headline itself and not the finding's position in the list.
///
/// The report renders findings as `### **Finding N** - <headline>`, and older
/// reports also used `### Finding N: <headline>`; both shapes reduce to the
/// headline, so a finding that only moved position matches between two reports
/// (FR-009). A heading that does not carry the label is returned unchanged.
fn strip_finding_number(heading: &str) -> String {
    let trimmed = heading.trim_start_matches('#').trim();
    let without_bold = trimmed.replace("**", " ");
    // The first token is the ordinal: the `Finding` label when there is one,
    // else a bare number left by a `### 3 - <headline>` heading.
    let words: Vec<&str> = without_bold.split_whitespace().collect();
    let Some(first) = words.first() else {
        return String::new();
    };
    let starts_with_number = first.eq_ignore_ascii_case("finding")
        || first.chars().all(|c| c.is_ascii_digit() || c == '-');
    if !starts_with_number {
        return words.join(" ");
    }
    // Drop the ordinal and the punctuation that trails it, then keep the rest:
    // the label token, a bare number, and any `-` / `:` / `.` separator that
    // introduces the headline rather than belonging to it.
    words
        .into_iter()
        .skip_while(|token| {
            token.starts_with(|c: char| c.is_ascii_digit())
                || token.eq_ignore_ascii_case("finding")
                || token.chars().all(|c| matches!(c, '-' | ':' | '.' | '#'))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Collect one item per paragraph unit (Executive Summary).
fn extract_paragraphs(element: DeckElement, body: &[&str]) -> Vec<DeckItem> {
    let mut items = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let mut fenced = false;
    for line in body {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            // A fenced block is neither prose nor an item boundary: its lines
            // are skipped whole so a diagram cannot become a summary unit.
            flush_paragraph(element, &mut paragraph, &mut items);
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if trimmed.is_empty() {
            flush_paragraph(element, &mut paragraph, &mut items);
        } else if !trimmed.starts_with('|') {
            paragraph.push(line);
        }
    }
    flush_paragraph(element, &mut paragraph, &mut items);
    items
}

/// Push the accumulated paragraph lines as one item, if any.
fn flush_paragraph(element: DeckElement, paragraph: &mut Vec<&str>, items: &mut Vec<DeckItem>) {
    if paragraph.is_empty() {
        return;
    }
    let text = paragraph.join(" ");
    paragraph.clear();
    if let Some(item) = make_item(element, &text) {
        items.push(item);
    }
}

/// Whether a line is a table rule or fence marker, not item content.
fn is_structural_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') || trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

/// Collect one item per bullet line (Cross-References).
///
/// A Markdown table (the cross-references table) contributes one item per data
/// row instead.
fn extract_bullets(element: DeckElement, body: &[&str]) -> Vec<DeckItem> {
    let mut items = Vec::new();
    let mut current: Option<String> = None;
    for line in body {
        let trimmed = line.trim();
        if is_table_rule(trimmed) {
            continue;
        }
        if trimmed.starts_with('|') {
            if let Some(text) = current.take() {
                if let Some(item) = make_item(element, &text) {
                    items.push(item);
                }
            }
            if !is_table_header(trimmed) {
                if let Some(item) = make_item(element, &table_row_text(trimmed)) {
                    items.push(item);
                }
            }
            continue;
        }
        if is_structural_line(line) {
            continue;
        }
        if let Some(rest) = strip_bullet_marker(trimmed) {
            if let Some(text) = current.take() {
                if let Some(item) = make_item(element, &text) {
                    items.push(item);
                }
            }
            current = Some(rest.to_string());
        } else if let Some(text) = current.as_mut() {
            text.push(' ');
            text.push_str(trimmed);
        }
    }
    if let Some(text) = current {
        if let Some(item) = make_item(element, &text) {
            items.push(item);
        }
    }
    items
}

/// Return the body of a `- ` / `* ` / `+ ` bullet line.
fn strip_bullet_marker(line: &str) -> Option<&str> {
    let rest = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* ").or_else(|| line.strip_prefix("+ ")))?;
    Some(rest.trim())
}

/// Whether a table line is the `|---|---|` header-alignment rule.
fn is_table_rule(line: &str) -> bool {
    line.starts_with('|')
        && line
            .chars()
            .all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
        && line.contains('-')
}

/// Whether a table line is a column-heading row rather than a data row.
///
/// A heading row is one whose first cell is a column title, not a value: the
/// references table opens with `#`, and the cross-references table opens with
/// `Path`. A data row always has a content-bearing first cell (`1`, `` `x.rs` ``).
fn is_table_header(line: &str) -> bool {
    table_cells(line).first().is_some_and(|first| {
        // Strip the inline-code ticks a path cell is wrapped in, then reject
        // anything that looks like a value rather than a column title.
        let first = first.trim_matches('`').trim();
        first.eq_ignore_ascii_case("#")
            || first.eq_ignore_ascii_case("path")
            || first.eq_ignore_ascii_case("url")
    })
}

/// Split a Markdown table row into its trimmed cells (outer pipes removed).
fn table_cells(line: &str) -> Vec<&str> {
    line.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(str::trim)
        .collect()
}

/// A cross-reference table row rendered as `path - relevance`.
fn table_row_text(line: &str) -> String {
    let cells = table_cells(line);
    match cells.as_slice() {
        [path, relevance, ..] => format!("{path} - {}", relevance_cell(relevance)),
        _ => cells.join(" - "),
    }
}

/// Normalise an empty or `-` table cell to an absence marker.
fn relevance_cell(cell: &str) -> &str {
    if cell.is_empty() { "-" } else { cell }
}

/// Collect one item per `### **Finding N** - <headline>` block (Findings).
///
/// The item text is the block's heading line; its identity key is the headline
/// extracted the same way the assembled report extracts it, so a finding matches
/// between two reports even when the finding numbers shift. A body before the
/// first `###` sub-heading contributes no item: it is the section's placeholder.
fn extract_findings(body: &[&str]) -> Vec<DeckItem> {
    let mut items = Vec::new();
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in body {
        if line.trim_start().starts_with("### ") && !line.trim_start().starts_with("#### ") {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            current = Some(vec![line]);
        } else if let Some(block) = current.as_mut() {
            block.push(line);
        }
    }
    if let Some(block) = current {
        blocks.push(block);
    }

    for block in blocks {
        let Some(heading) = block.first() else {
            continue;
        };
        let heading = heading.trim_start().trim_start_matches("### ").trim();
        let rest = block[1..].join("\n");
        // The whole block text is handed to `make_item`, which keeps only the
        // headline for the slide but can fall back to the block's body when the
        // heading carries no headline of its own.
        if let Some(item) = make_item(DeckElement::Findings, &format!("{heading}\n{rest}")) {
            items.push(item);
        }
    }
    items
}

/// Collect one `DeckItem` per `### <n>. <name>` concept block (report `## Concepts`).
///
/// The report renders each concept as a `### <n>. <name>` heading followed by a
/// `**Definition:**` paragraph, prose paragraphs and a `**Key Evidence:**`
/// bullet list. The item's `text` is the concept name (its identity, FR-009) and
/// its `detail` is the block's prose paragraphs, so the concept slide can show
/// what the concept *is* while its findings are listed beneath it (FR-026).
///
/// A `## Concepts` section with no `### ` sub-headings renders its concepts as
/// one paragraph each (older reports did this), so each non-empty paragraph
/// becomes a concept named by its first phrase.
fn extract_concepts(body: &[&str]) -> Vec<DeckItem> {
    let mut items = Vec::new();
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in body {
        if is_sub_heading(line) {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            current = Some(vec![line]);
        } else if let Some(block) = current.as_mut() {
            block.push(line);
        }
    }
    if let Some(block) = current {
        blocks.push(block);
    }

    if blocks.is_empty() {
        return extract_paragraphs(DeckElement::Concepts, body);
    }

    for block in blocks {
        let Some(heading) = block.first() else {
            continue;
        };
        let heading = heading.trim_start().trim_start_matches('#').trim();
        let name = strip_concept_number(heading);
        if name.is_empty() {
            continue;
        }
        let rest = block[1..].join("\n");
        // The definition is the concept's own prose: the `**Definition:**` block
        // leads, and a block with no recognised labels falls back to its plain
        // paragraphs rather than rendering the name bare.
        let mut detail = concept_prose(&rest);
        if detail.is_empty() {
            detail = plain_paragraphs(&rest);
        }
        items.push(DeckItem {
            element: DeckElement::Concepts,
            key: normalize_identity(DeckElement::Concepts, &name),
            text: name,
            source_url: None,
            // A concept has no per-item source block: its evidence bullets carry
            // `[#N]` markers, so the citation set keeps FR-004's `ADDED`
            // promotion observable when a concept gains a new source.
            sources: citation_markers(&rest),
            detail,
        });
    }
    items
}

/// Remove a leading `N.` / `N)` ordinal from a concept heading.
///
/// The report numbers its concepts (`### 1. Human Oversight`, and a run that
/// bolded its numbers writes `### **3.** Runtime Agent Governance`); the ordinal
/// is a position, not part of the concept's name, so it is stripped - with the
/// emphasis that wrapped it - before the name becomes the item's identity and
/// slide heading.
fn strip_concept_number(heading: &str) -> String {
    let heading = heading.trim();
    let Some(first) = heading.split_whitespace().next() else {
        return String::new();
    };
    let bare = first
        .trim_start_matches(['*', '_', '#'])
        .trim_end_matches(['*', '_']);
    let ordinal = !bare.is_empty()
        && bare
            .trim_end_matches(['.', ')'])
            .chars()
            .all(|ch| ch.is_ascii_digit());
    let rest = heading[first.len()..].trim();
    let rest = if ordinal && !rest.is_empty() {
        rest
    } else {
        heading
    };
    rest.trim_matches('*').trim_matches('_').trim().to_string()
}

/// The prose paragraphs of a concept block, in document order.
///
/// Concept blocks are written as `**Definition:**`, plain paragraphs and a
/// `**Key Evidence:**` bullet list, so this collects the prose and drops the
/// labels themselves (a label is structure, not text) and every bullet.
fn concept_prose(body: &str) -> Vec<String> {
    let mut blocks: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let flush = |current: &mut Vec<&str>, blocks: &mut Vec<String>| {
        if current.is_empty() {
            return;
        }
        let joined = current.join(" ");
        current.clear();
        let joined = joined.split_whitespace().collect::<Vec<_>>().join(" ");
        if !joined.is_empty() {
            blocks.push(joined);
        }
    };
    // Whether the run of prose being collected sits under a `**Key Evidence:**`
    // label, whose bullets are evidence and never definition prose.
    let mut in_evidence = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || is_structural_line(line) || trimmed.starts_with("### ") {
            flush(&mut current, &mut blocks);
            continue;
        }
        if let Some((_is_prose, label, rest)) = labelled_block_header(trimmed) {
            flush(&mut current, &mut blocks);
            in_evidence = label.eq_ignore_ascii_case("key evidence");
            if !rest.is_empty() {
                current.push(rest);
            }
            continue;
        }
        if in_evidence || strip_bullet_marker(trimmed).is_some() {
            continue;
        }
        current.push(trimmed);
    }
    flush(&mut current, &mut blocks);
    blocks
}

/// The plain paragraphs of `text`, used for a concept block whose labels are
/// not recognised: blank-line separated runs of prose lines.
fn plain_paragraphs(text: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut flush = |current: &mut Vec<&str>| {
        if current.is_empty() {
            return;
        }
        let joined = current.join(" ");
        current.clear();
        let joined = joined.split_whitespace().collect::<Vec<_>>().join(" ");
        if !joined.is_empty() {
            paragraphs.push(joined);
        }
    };
    for line in text.lines() {
        let trimmed = line.trim();
        let structural = trimmed.starts_with('|')
            || trimmed.starts_with("```")
            || trimmed.starts_with("~~~")
            || is_sub_heading(trimmed);
        if trimmed.is_empty() || structural {
            flush(&mut current);
            continue;
        }
        current.push(trimmed);
    }
    flush(&mut current);
    paragraphs
}

/// Build the [`DeckItem`] for one finding block (`heading\n<body>`).
fn make_finding_item(block: &str) -> Option<DeckItem> {
    // `### **Finding N** - <headline>`: keep the headline for the slide, and
    // fall back to the observation-derived headline the report itself would use.
    let (heading, rest) = block.split_once('\n').unwrap_or((block, ""));
    let headline = strip_finding_number(heading);
    let headline = if normalise_name(&headline).is_empty() {
        let body = rest.trim();
        // A body that is only the report's placeholder notice carries no
        // observation, so the block has no headline to derive one from.
        if body.is_empty() || is_placeholder(body) {
            return None;
        }
        make_headline_from_observation(body)
    } else {
        headline
    };
    let headline = headline.trim();
    if headline.is_empty() || headline == NO_HEADLINE_AVAILABLE {
        return None;
    }
    Some(DeckItem {
        element: DeckElement::Findings,
        key: normalize_identity(DeckElement::Findings, headline),
        text: headline.to_string(),
        source_url: None,
        // The finding's own `**Sources:**` block, which the FR-004 `ADDED`
        // promotion compares against the older report's source set.
        sources: finding_sources(rest),
        // The `**Observation:**` / `**Analysis:**` / `**Implication:**`
        // paragraphs the report renders under the heading, which the finding
        // slide condenses into one paragraph.
        detail: labelled_blocks(rest),
    })
}

/// The paragraphs of a finding block's `**Observation:**`, `**Analysis:**` and
/// `**Implication:**` labels, in that order.
///
/// The report renders each finding as the `### **Finding N** - <headline>`
/// heading followed by those three bold labels, each introducing one or more
/// paragraphs of prose and ending at the next bold label (`**Sources:**`,
/// `**Source date range:**`) or the next `###` heading. Only the three prose
/// labels are collected: a `**Sources:**` bullet list is not prose and the
/// cross-reference note is metadata, so neither becomes a summary input.
/// Blocks are returned whole (newlines collapsed to single spaces) so the
/// enrichment stage can condense them; a block with no prose contributes
/// nothing, and a block with no recognisable label at all yields an empty list.
fn labelled_blocks(body: &str) -> Vec<String> {
    let mut blocks: Vec<String> = Vec::new();
    let mut collecting = false;
    let mut current: Vec<&str> = Vec::new();
    let flush = |current: &mut Vec<&str>, blocks: &mut Vec<String>| {
        if current.is_empty() {
            return;
        }
        let text = current.join(" ");
        current.clear();
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !text.is_empty() {
            blocks.push(text);
        }
    };
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some((prose, _label, rest)) = labelled_block_header(trimmed) {
            flush(&mut current, &mut blocks);
            collecting = prose;
            if prose && !rest.is_empty() {
                current.push(rest);
            }
            continue;
        }
        if !collecting {
            continue;
        }
        // A table, a fenced block or a section heading ends the prose.
        if trimmed.starts_with('|')
            || trimmed.starts_with("```")
            || trimmed.starts_with("~~~")
            || trimmed.starts_with("### ")
            || trimmed.starts_with("#### ")
        {
            flush(&mut current, &mut blocks);
            collecting = false;
            continue;
        }
        if trimmed.is_empty() {
            flush(&mut current, &mut blocks);
            continue;
        }
        current.push(trimmed);
    }
    flush(&mut current, &mut blocks);
    blocks
}

/// Whether `line` is one of the finding prose labels, and what follows it.
///
/// Returns `(is_prose_label, label, remainder)` for `**Observation:**`,
/// `**Analysis:**` and `**Implication:**` (also accepted without the bold
/// markers), and `(false, label, remainder)` for any other bold label such as
/// `**Sources:**` or `**Key Evidence:**`, so the caller stops collecting prose
/// without mistaking the label for text.
fn labelled_block_header(line: &str) -> Option<(bool, &str, &str)> {
    let trimmed = line.trim();
    // A label is `**Name:**` or a bare `Name:`; split on the colon that closes
    // the label rather than the first colon in the line, which for
    // `**Source date range:**` sits inside the bold markers.
    let closed = trimmed
        .strip_prefix("**")
        .and_then(|rest| rest.split_once("**"))
        .map(|(label, rest)| {
            (
                label.trim_end_matches(':').trim(),
                rest.trim_start_matches(':'),
            )
        });
    let (label, rest) = match closed {
        Some((label, rest)) => (label, rest.trim()),
        None => match trimmed.split_once(':') {
            Some((label, rest)) => (label.trim(), rest.trim()),
            None => (trimmed, trimmed),
        },
    };
    let prose = ["observation", "analysis", "implication"]
        .iter()
        .any(|name| label.eq_ignore_ascii_case(name));
    // Only a bold label (or one of the three prose labels, which an older
    // report may render unbolded) ends the preceding block.
    if !prose && !trimmed.starts_with("**") {
        return None;
    }
    Some((prose, label, rest))
}

/// The normalised source identities a finding's `**Sources:**` block lists.
///
/// The report renders one bullet per source as
/// `- [#N] <title> - [<url>](<url>) (published <date>)`; the identity is the URL,
/// matching [`DeckElement::References`], so a renumbered citation does not read
/// as a different source. A block with no bullet (every finding older than the
/// per-finding source list) yields an empty set, and the item can then only be
/// `NEW`.
fn finding_sources(body: &str) -> Vec<String> {
    let mut sources = Vec::new();
    let mut in_sources = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("**Sources:**") || trimmed.eq_ignore_ascii_case("Sources:")
        {
            in_sources = true;
            continue;
        }
        if !in_sources {
            continue;
        }
        // The block ends at the next bold label (`**Source date range:**`) or any
        // other non-bullet content.
        let Some(rest) = strip_bullet_marker(trimmed) else {
            if trimmed.is_empty() {
                continue;
            }
            break;
        };
        let Some(url) = bullet_url(rest) else {
            continue;
        };
        let identity = normalize_identity(DeckElement::References, &url);
        if !identity.is_empty() && !sources.contains(&identity) {
            sources.push(identity);
        }
    }
    sources
}

/// The address of a source bullet: the target of its Markdown link when it has
/// one, else the first bare URL on the line.
fn bullet_url(text: &str) -> Option<String> {
    // `[<url>](<url>)` - the report's own shape. The link target is the second
    // group; the label is the URL again, so either group gives the same address.
    if let Some(open) = text.find("](") {
        let rest = &text[open + 2..];
        if let Some(close) = rest.find(')') {
            let url = rest[..close].trim();
            if !url.is_empty() {
                return Some(url.to_string());
            }
        }
    }
    // A bare `https://...` with no link wrapper.
    text.split_whitespace()
        .find(|word| word.starts_with("http://") || word.starts_with("https://"))
        .map(|word| word.trim_end_matches([')', ',', '.', ';']).to_string())
}

/// The placeholder [`make_headline_from_observation`] returns when the text it
/// was given carries no word at all.
///
/// The report assembler turns that into `Finding {n}`, but a deck finding with
/// no headline has no stable identity between two runs (its number is not its
/// identity, FR-009), so such a block contributes no deck item.
const NO_HEADLINE_AVAILABLE: &str = "(no headline available)";

/// Collect one item per `| [#N] | ... |` table row (References Index).
fn extract_references(body: &[&str]) -> Vec<DeckItem> {
    let mut items = Vec::new();
    for line in body {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || is_table_rule(trimmed) || is_table_header(trimmed) {
            continue;
        }
        // A row without a `[#N]` opening cell is not a reference entry.
        if reference_number(trimmed).is_none() {
            continue;
        }
        let cells = table_cells(trimmed);
        // The URL column carries the source's address; title and author follow
        // it. Keeping the URL in the text lets the later diff stage compare
        // source sets (FR-004 `ADDED`), and it is the reference's identity
        // (FR-009), so a source that keeps its address matches across runs even
        // when its `[#N]` citation was renumbered.
        let url = cells.get(4).copied().unwrap_or("");
        let title = cells.get(5).copied().unwrap_or("");
        let text = if title.is_empty() || title == "-" {
            url.to_string()
        } else {
            format!("{title} - {url}")
        };
        if let Some(mut item) = make_item(DeckElement::References, &text) {
            item.key = normalize_identity(DeckElement::References, url);
            item.source_url = Some(url.to_string());
            // A reference *is* one source, so its own set is that one identity.
            // FR-004's promotion then reads uniformly for every element.
            let identity = item.key.clone();
            if !identity.is_empty() {
                item.sources.push(identity);
            }
            items.push(item);
        }
    }
    items
}

/// Parse the `[#N]` reference number of a table row's opening cell.
fn reference_number(line: &str) -> Option<u64> {
    let cells = table_cells(line);
    let raw = *cells.first()?;
    let raw = raw.trim().trim_start_matches("[#").trim_end_matches(']');
    raw.trim().parse::<u64>().ok()
}

/// Longest condensed bullet, in characters, before the ellipsis (FR-022).
///
/// The cap counts the **escaped** text, so a bullet whose escaping grows it past
/// the limit is truncated after escaping: the emitted bullet never exceeds this
/// many characters plus the trailing ellipsis.
pub const CONDENSE_MAX_CHARS: usize = 200;

/// The trailing marker appended to a bullet that was cut short (FR-022).
const ELLIPSIS: &str = "...";

/// Condense an item to the single bullet line a deck slide shows (FR-022).
///
/// The deck is a slide deck, so each item appears as one bullet rather than as
/// the paragraph, numbered item or finding block the report carried. The bullet
/// is built in four steps:
///
/// 1. the item's first sentence (an item with no sentence terminator keeps all
///    of its text), so a multi-sentence summary cannot overflow a slide;
/// 2. every whitespace run collapsed to one space, so the bullet is a single
///    line whatever the source wrapped at;
/// 3. the Markdown-significant characters escaped - `\`, backtick and `|` so
///    they cannot open an escape, a code span or a table cell, and a leading
///    `#` so it cannot open a heading;
/// 4. the result capped at [`CONDENSE_MAX_CHARS`] on a word boundary with a
///    trailing `...` when anything was dropped.
///
/// The result carries no newline and no control character, so it can be emitted
/// on one bullet line without breaking the slide structure.
///
/// ```
/// use ragent_research::deck::{DeckItem, DeckElement, condense_bullet};
///
/// let item = DeckItem {
///     element: DeckElement::Concepts,
///     key: String::new(),
///     text: "Pipe | and `tick` are escaped.  A second sentence is dropped.".to_string(),
///     source_url: None,
///     sources: Vec::new(),
///     detail: Vec::new(),
/// };
/// assert_eq!(
///     condense_bullet(&item),
///     "Pipe \\| and \\`tick\\` are escaped."
/// );
/// ```
#[must_use]
pub fn condense_bullet(item: &DeckItem) -> String {
    let sentence = first_sentence(&collapse_whitespace(&strip_control_chars(&item.text)));
    let escaped = escape_markdown(&sentence);
    truncate_words(&escaped, CONDENSE_MAX_CHARS)
}

/// Collapse every whitespace run in `text` to a single space and trim the ends.
///
/// Interior newlines and tabs therefore become spaces, so the bullet is one
/// line even when it came from a wrapped paragraph or a multi-line finding.
pub(crate) fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Return the first sentence of `text`, terminator and closing quotes included.
///
/// A sentence ends at `.`, `!` or `?` followed by whitespace or the end of the
/// text, which keeps decimals (`0.91`) and version numbers from ending a bullet
/// early. A period also does not end the sentence when it closes a one-letter
/// initialism (`e.g.`, `i.e.`, `U.S.`): the tokens are joined by periods, so the
/// terminator is still inside a word. Text with no such terminator is returned
/// whole; a cut that leaves nothing meaningful (a bare `.` or `...`) falls back
/// to the whole text so an item can never condense to an empty bullet.
pub(crate) fn first_sentence(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    for (idx, &ch) in chars.iter().enumerate() {
        if !matches!(ch, '.' | '!' | '?') {
            continue;
        }
        // Absorb a run of terminators (`?!`, `...`) and closing quotes so the
        // bullet keeps the punctuation the sentence ended with.
        let mut end = idx + 1;
        while end < chars.len() && matches!(chars[end], '.' | '!' | '?' | '"' | '\'' | ')' | ']') {
            end += 1;
        }
        let at_boundary = end == chars.len() || chars[end].is_whitespace();
        if !at_boundary || closes_initialism(&chars, end) {
            continue;
        }
        let sentence: String = chars[..end].iter().collect();
        let trimmed = sentence.trim();
        if trimmed.chars().any(|c| c.is_alphanumeric()) {
            return trimmed.to_string();
        }
    }
    text.trim().to_string()
}

/// Whether the period run ending at `end` closes a one-letter initialism.
///
/// `e.g.` is `e`, `.`, `g`, `.`: the terminator follows a single letter that is
/// itself preceded by a period and another single letter. The token before that
/// run must be that shape, otherwise an ordinary word ending (`size.`) counts.
fn closes_initialism(chars: &[char], end: usize) -> bool {
    // Walk back over the trailing period run and the letter it closes.
    let mut idx = end;
    while idx > 0 && chars[idx - 1] == '.' {
        idx -= 1;
    }
    if idx == end || idx == 0 || !chars[idx - 1].is_alphabetic() {
        return false;
    }
    idx -= 1;
    chars[idx].is_alphabetic() && idx > 1 && chars[idx - 1] == '.' && chars[idx - 2].is_alphabetic()
}

/// Escape the characters that would let a bullet change the slide structure.
///
/// `|` would start a table cell, a backtick would open a code span, and a
/// leading `#` would turn the bullet into a heading; `\` is escaped because it is
/// the escape character itself. Nothing else is escaped, so ordinary prose keeps
/// its emphasis and inline code markup intact.
pub(crate) fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for (idx, ch) in text.chars().enumerate() {
        match ch {
            '\\' | '`' | '|' => {
                out.push('\\');
                out.push(ch);
            }
            '#' if idx == 0 => out.push_str("\\#"),
            _ => out.push(ch),
        }
    }
    out
}

/// Cap `text` at `limit` characters, cutting back to a word boundary.
///
/// A trailing `...` marks the cut. A single word longer than the limit is
/// hard-truncated rather than overflowing it, so a bullet can never exceed the
/// cap whatever the source text looked like.
pub(crate) fn truncate_words(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut head: String = text.chars().take(limit).collect();
    if let Some(cut) = head.rfind(' ') {
        head.truncate(cut);
    }
    let mut out = head.trim_end().to_string();
    out.push_str(ELLIPSIS);
    out
}

/// One report file of an item series, classified by its naming scheme.
///
/// A report is either dated (`RESEARCH-<YYYY-MM-DD>.md`, the current scheme) or
/// the legacy undated `RESEARCH.md` an older ragent wrote. The two are kept
/// distinct because they are ordered against each other rather than
/// alphabetically: the legacy file is always the **oldest** report of its series
/// (FR-012), never a file that happens to sort before `RESEARCH-2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportFile {
    /// The file name within the item directory, e.g. `RESEARCH-2025-02-14.md`.
    pub name: String,
    /// The report's `YYYY-MM-DD` stamp, or `None` for the legacy undated file.
    pub stamp: Option<String>,
}

impl ReportFile {
    /// Whether this is the legacy undated `RESEARCH.md` (FR-012).
    #[must_use]
    pub fn is_legacy(&self) -> bool {
        self.stamp.is_none()
    }

    /// The report's stamp, or `"legacy"` for the undated baseline.
    #[must_use]
    pub fn stamp_label(&self) -> &str {
        self.stamp.as_deref().unwrap_or(LEGACY_STAMP_LABEL)
    }
}

/// How a legacy undated `RESEARCH.md` is named in a coverage window.
const LEGACY_STAMP_LABEL: &str = "legacy";

/// The `RESEARCH-` prefix of a dated report file name.
const RESEARCH_PREFIX: &str = "RESEARCH-";

/// The `.md` suffix of a report file name.
const MARKDOWN_SUFFIX: &str = ".md";

/// The ordered report series of one research item (FR-006, FR-007, FR-012).
///
/// The series is sorted **newest first** by date stamp with the legacy
/// `RESEARCH.md` last, so the default "two most recent reports" selection is the
/// first two entries and the adjacent report for an explicit stamp is the entry
/// next to it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReportSeries {
    /// The reports of the item, newest first, legacy `RESEARCH.md` last.
    reports: Vec<ReportFile>,
}

impl ReportSeries {
    /// Classify and order a directory's file names into a report series.
    ///
    /// Dated names are recognised with [`crate::io::is_dated_research_file_name`]
    /// and the legacy name with [`crate::io::LEGACY_RESEARCH_FILE_NAME`]; every
    /// other entry in the directory (a `CORPA-*.md` companion, a `DECK-*.md`, a
    /// `state.json`) is not a report and is ignored. Ordering is by descending
    /// stamp, which the ISO date prefix makes chronological.
    #[must_use]
    pub fn classify(names: &[String]) -> Self {
        let mut reports: Vec<ReportFile> = names
            .iter()
            .filter_map(|name| classify_report_name(name))
            .collect();
        // Newest first. A dated report always precedes the legacy baseline, whose
        // missing stamp sorts last rather than first by byte order.
        reports.sort_by(|left, right| match (&left.stamp, &right.stamp) {
            (Some(a), Some(b)) => b.cmp(a),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.name.cmp(&right.name),
        });
        Self { reports }
    }

    /// Every report of the series, newest first.
    #[must_use]
    pub fn reports(&self) -> &[ReportFile] {
        &self.reports
    }

    /// The dated stamps in the series, newest first, for a refusal message.
    #[must_use]
    pub fn dated_stamps(&self) -> Vec<&str> {
        self.reports
            .iter()
            .filter_map(|report| report.stamp.as_deref())
            .collect()
    }

    /// The number of report files the item directory holds (FR-011).
    #[must_use]
    pub fn len(&self) -> usize {
        self.reports.len()
    }

    /// Whether the item directory holds no report at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.reports.is_empty()
    }

    /// Whether `stamp` names a dated report in this series.
    #[must_use]
    pub fn contains_stamp(&self, stamp: &str) -> bool {
        self.reports
            .iter()
            .any(|report| report.stamp.as_deref() == Some(stamp))
    }

    /// Select the report pair a deck run renders.
    ///
    /// `from` is the older (baseline) stamp and `to` the newer one, each optional:
    ///
    /// - neither: the two most recent dated reports (FR-006);
    /// - both: exactly that pair, which must be ordered older-then-newer;
    /// - `from` only: that report paired with the next newer one (FR-007);
    /// - `to` only: that report paired with the next older one (FR-007).
    ///
    /// The legacy `RESEARCH.md` is never chosen unless it is the adjacent older
    /// report of an explicit `--to`, because FR-012 makes it a baseline only.
    ///
    /// # Errors
    ///
    /// Returns a [`DeckSelectError`] naming the item and the reason when the
    /// series cannot produce a usable pair (FR-011, FR-012, FR-020).
    pub fn select_pair(
        &self,
        item: &str,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<ReportPair, DeckSelectError> {
        for stamp in [from, to].into_iter().flatten() {
            self.validate_stamp(item, stamp)?;
        }
        match (from, to) {
            (Some(from), Some(to)) => ReportPair::new(item, from, to),
            (Some(from), None) => {
                let newer = self.neighbour_newer(from).ok_or_else(|| {
                    DeckSelectError::NoAdjacentReport {
                        item: item.to_string(),
                        stamp: from.to_string(),
                        direction: ADJACENT_NEWER,
                    }
                })?;
                ReportPair::new(item, from, &newer)
            }
            (None, Some(to)) => {
                let older =
                    self.neighbour_older(to)
                        .ok_or_else(|| DeckSelectError::NoAdjacentReport {
                            item: item.to_string(),
                            stamp: to.to_string(),
                            direction: ADJACENT_OLDER,
                        })?;
                ReportPair::new(item, &older, to)
            }
            (None, None) => self.select_default(item),
        }
    }

    /// Select the two most recent dated reports (FR-006).
    fn select_default(&self, item: &str) -> Result<ReportPair, DeckSelectError> {
        // `dated_stamps` is newest first, so `[0]` is the target and `[1]` the
        // baseline. The legacy baseline is never selected by default.
        let dated = self.dated_stamps();
        if dated.len() < 2 {
            return Err(self.too_few_reports(item, &dated));
        }
        ReportPair::new(item, dated[1], dated[0])
    }

    /// Build the FR-011 / FR-012 refusal for a series without a usable pair.
    fn too_few_reports(&self, item: &str, dated: &[&str]) -> DeckSelectError {
        DeckSelectError::TooFewReports {
            item: item.to_string(),
            found: self.len(),
            only_legacy: !self.reports.is_empty() && self.reports.iter().all(ReportFile::is_legacy),
            stamps: dated.iter().map(|stamp| (*stamp).to_string()).collect(),
        }
    }

    /// The report immediately newer than `stamp` (FR-007 `--from`).
    ///
    /// Newest first, so the neighbour newer than `stamp` is the entry before it.
    fn neighbour_newer(&self, stamp: &str) -> Option<String> {
        self.reports
            .iter()
            .position(|report| report.stamp.as_deref() == Some(stamp))
            .and_then(|idx| idx.checked_sub(1))
            .and_then(|idx| self.reports[idx].stamp.clone())
    }

    /// The report immediately older than `stamp` (FR-007 `--to`).
    ///
    /// The legacy `RESEARCH.md` is the oldest report of the series, so when
    /// `stamp` is the oldest dated report the adjacent older entry is the legacy
    /// baseline, whose file name is returned as the pair's older stamp.
    fn neighbour_older(&self, stamp: &str) -> Option<String> {
        let idx = self
            .reports
            .iter()
            .position(|report| report.stamp.as_deref() == Some(stamp))?;
        let older = self.reports.get(idx + 1)?;
        Some(older.stamp.clone().unwrap_or_else(|| older.name.clone()))
    }

    /// Validate a `--from` / `--to` value against this series (FR-020).
    ///
    /// The value must be a real `YYYY-MM-DD` date (so `2025-04-31` is refused) and
    /// must name a report that exists (so `2025-01-11` is refused with the report
    /// dates that do exist).
    fn validate_stamp(&self, item: &str, stamp: &str) -> Result<(), DeckSelectError> {
        if NaiveDate::parse_from_str(stamp, "%Y-%m-%d").is_err() {
            return Err(DeckSelectError::InvalidStamp {
                item: item.to_string(),
                value: stamp.to_string(),
            });
        }
        let stamps = self.dated_stamps();
        let available: Vec<String> = stamps.iter().map(|s| (*s).to_string()).collect();
        if available.iter().any(|known| known == stamp) {
            return Ok(());
        }
        Err(DeckSelectError::UnknownStamp {
            item: item.to_string(),
            value: stamp.to_string(),
            available,
        })
    }
}

/// Classify one directory entry name as a report, or `None` when it is not one.
fn classify_report_name(name: &str) -> Option<ReportFile> {
    if is_dated_research_file_name(name) {
        let stamp = name
            .strip_prefix(RESEARCH_PREFIX)
            .and_then(|rest| rest.strip_suffix(MARKDOWN_SUFFIX))?;
        return Some(ReportFile {
            name: name.to_string(),
            stamp: Some(stamp.to_string()),
        });
    }
    (name == LEGACY_RESEARCH_FILE_NAME).then(|| ReportFile {
        name: name.to_string(),
        stamp: None,
    })
}

/// The `--from` / `--to` pairing direction words used in a refusal message.
const ADJACENT_NEWER: &str = "newer";
/// The `--to` pairing direction word used in a refusal message.
const ADJACENT_OLDER: &str = "older";

/// The two reports a deck run covers, older first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportPair {
    /// The older report (the diff baseline).
    pub older: ReportFile,
    /// The newer report (the diff target).
    pub newer: ReportFile,
}

impl ReportPair {
    /// Build a pair from an older and a newer stamp, refusing an inverted pair.
    ///
    /// # Errors
    ///
    /// Returns [`DeckSelectError::InvertedPair`] when `from` is not strictly older
    /// than `to`, because a deck compares a baseline against a later report.
    fn new(item: &str, from: &str, to: &str) -> Result<Self, DeckSelectError> {
        let older = report_file_for(from);
        let newer = report_file_for(to);
        if from == to {
            return Err(DeckSelectError::InvertedPair {
                item: item.to_string(),
                from: from.to_string(),
                to: to.to_string(),
            });
        }
        // A dated stamp is always newer than the undated legacy baseline, so only
        // two dated stamps are order-compared.
        if !older.is_legacy() && !newer.is_legacy() && from > to {
            return Err(DeckSelectError::InvertedPair {
                item: item.to_string(),
                from: from.to_string(),
                to: to.to_string(),
            });
        }
        Ok(Self { older, newer })
    }

    /// The older report's stamp label (`legacy` for `RESEARCH.md`).
    #[must_use]
    pub fn older_label(&self) -> &str {
        self.older.stamp_label()
    }

    /// The newer report's stamp label.
    #[must_use]
    pub fn newer_label(&self) -> &str {
        self.newer.stamp_label()
    }
}

/// Build a [`ReportFile`] for a dated stamp or for the legacy file name.
fn report_file_for(stamp: &str) -> ReportFile {
    if stamp == LEGACY_RESEARCH_FILE_NAME {
        ReportFile {
            name: LEGACY_RESEARCH_FILE_NAME.to_string(),
            stamp: None,
        }
    } else {
        ReportFile {
            name: dated_research_file_name(stamp),
            stamp: Some(stamp.to_string()),
        }
    }
}

/// Why a deck run could not select its report pair (FR-011, FR-012, FR-020).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DeckSelectError {
    /// The item directory holds fewer than two usable reports (FR-011).
    ///
    /// A series whose only report is the legacy undated `RESEARCH.md` adds the
    /// FR-012 sentence naming that scheme, because the remedy is different: the
    /// item needs its first dated report, not a second one.
    #[error(
        "cannot build a deck for '{item}': {found} report(s) found; a deck needs two reports \
         (run `/research update {item}` to add one){legacy_note}",
        legacy_note = legacy_note(*.only_legacy)
    )]
    TooFewReports {
        /// The research item name.
        item: String,
        /// How many report files the item directory holds.
        found: usize,
        /// Whether every report found is the legacy undated `RESEARCH.md`.
        only_legacy: bool,
        /// The dated stamps present, newest first.
        stamps: Vec<String>,
    },
    /// A `--from` / `--to` value is not a valid `YYYY-MM-DD` date (FR-020).
    #[error("invalid report date '{value}' for '{item}': expected a real date in YYYY-MM-DD form")]
    InvalidStamp {
        /// The research item name.
        item: String,
        /// The offending value.
        value: String,
    },
    /// A valid stamp names no report of the item (FR-020).
    #[error(
        "no report dated {value} for '{item}'; existing report dates: {available}",
        available = describe_available(.available)
    )]
    UnknownStamp {
        /// The research item name.
        item: String,
        /// The offending value.
        value: String,
        /// The report dates that do exist, newest first.
        available: Vec<String>,
    },
    /// An explicit pair is not ordered older-then-newer.
    #[error(
        "--from {from} is not older than --to {to} for '{item}'; a deck compares a baseline \
         against a later report"
    )]
    InvertedPair {
        /// The research item name.
        item: String,
        /// The `--from` value.
        from: String,
        /// The `--to` value.
        to: String,
    },
    /// A single named report has no adjacent report to pair with (FR-007).
    #[error(
        "cannot pair {stamp} with a {direction} report for '{item}': no dated report exists \
         {direction} than {stamp}"
    )]
    NoAdjacentReport {
        /// The research item name.
        item: String,
        /// The stamp that had no neighbour.
        stamp: String,
        /// Which side was searched: `"newer"` or `"older"`.
        direction: &'static str,
    },
}

/// The FR-012 sentence appended when the only report is legacy `RESEARCH.md`.
fn legacy_note(only_legacy: bool) -> &'static str {
    if only_legacy {
        "; the item holds only the legacy undated RESEARCH.md, so it needs a dated \
         RESEARCH-<YYYY-MM-DD>.md run to compare against"
    } else {
        ""
    }
}

/// Render the report dates that do exist for an FR-020 refusal.
fn describe_available(available: &[String]) -> String {
    if available.is_empty() {
        "none (the item holds only the legacy RESEARCH.md)".to_string()
    } else {
        available.join(", ")
    }
}

/// The `DECK-` prefix of a deck output file name (FR-002).
const DECK_PREFIX: &str = "DECK-";

/// `DECK-<YYYY-MM-DD>.md` - the default deck file name for `stamp` (FR-002).
///
/// The deck is named for the day it was generated, not for either contributing
/// report, so regenerating a deck on a later day writes a new file beside the
/// earlier one instead of overwriting it.
#[must_use]
pub fn deck_file_name(stamp: &str) -> String {
    format!("{DECK_PREFIX}{stamp}{MARKDOWN_SUFFIX}")
}

/// The default deck path inside the item directory (FR-002, FR-016).
///
/// `item_dir` is the item's directory (`research/<name>/`), which the
/// orchestration stage resolves through [`crate::io::ResearchIo::item_dir`].
#[must_use]
pub fn default_deck_path(item_dir: &std::path::Path, stamp: &str) -> std::path::PathBuf {
    item_dir.join(deck_file_name(stamp))
}

/// Why a deck run's output could not be written (FR-016, FR-017).
///
/// Kept separate from [`DeckSelectError`] because these refusals are about the
/// destination, not the report pair: a run that fails here has already read both
/// reports and rendered a deck, and has written nothing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DeckWriteError {
    /// The target deck file exists and `--force` was not given (FR-017).
    #[error(
        "deck file already exists: {path} (pass --force to overwrite it, or --out <path> to \
         write elsewhere)"
    )]
    Exists {
        /// The existing deck file.
        path: String,
    },
    /// An `--out` path is unusable as a destination.
    #[error("invalid --out path '{value}': {reason}")]
    InvalidOut {
        /// The offending `--out` value.
        value: String,
        /// Why it cannot be used.
        reason: &'static str,
    },
}

/// The flags a `/research deck` run carries into the orchestration stage
/// (FR-007, FR-014, FR-015, FR-016, FR-017).
///
/// The parser resolves every value (`--from` / `--to` dates, and `--elements`
/// names against [`DeckElement::from_name`]) so this struct holds only
/// already-valid values; the orchestration stage then has one job per field
/// rather than a parse.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeckRunOptions {
    /// `--from <YYYY-MM-DD>` - the older report's stamp (FR-007).
    pub from: Option<String>,
    /// `--to <YYYY-MM-DD>` - the newer report's stamp (FR-007).
    pub to: Option<String>,
    /// `--elements <list>` - the elements to restrict the deck to, in FR-003
    /// precedence order (FR-014). Empty means every element.
    pub elements: Vec<DeckElement>,
    /// `--include-old` - also emit a slide for a no-change element (FR-015).
    pub include_old: bool,
    /// `--out <path>` - write here instead of the item's
    /// `DECK-<YYYY-MM-DD>.md` (FR-016).
    pub out: Option<String>,
    /// `--force` - overwrite an existing deck file (FR-017).
    pub force: bool,
}

/// What a completed deck run produced.
///
/// The per-status counts are the rendered diff's own counts, so a surface that
/// reports them cannot disagree with the file on disk (FR-010).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckOutcome {
    /// The path the deck was written to.
    pub path: std::path::PathBuf,
    /// The report pair the deck covers, older first.
    pub pair: ReportPair,
    /// The rendered document.
    pub document: DeckDocument,
    /// `true` when the pair produced no changed item (the FR-013 case).
    pub no_changes: bool,
    /// The count of `NEW` items in the deck.
    pub new_count: usize,
    /// The count of `ADDED` items in the deck.
    pub added_count: usize,
    /// The count of `UPDATE` items in the deck.
    pub update_count: usize,
    /// The count of `OLD` items in the deck.
    pub old_count: usize,
    /// What the optional PowerPoint export did (FR-030).
    ///
    /// Defaults to [`DeckPptxOutcome::Unavailable`], which is the honest answer
    /// for a run that offered no sink; the orchestration stage replaces it once
    /// a sink has been consulted.
    pub pptx: DeckPptxOutcome,
}

impl DeckOutcome {
    /// Build the outcome from a written deck, its pair, and the rendered diff.
    #[must_use]
    pub fn new(
        path: std::path::PathBuf,
        pair: ReportPair,
        document: DeckDocument,
        diff: &DeckDiff,
    ) -> Self {
        Self {
            path,
            pair,
            no_changes: !diff.has_changes(),
            new_count: diff.count(ItemStatus::New),
            added_count: diff.count(ItemStatus::Added),
            update_count: diff.count(ItemStatus::Update),
            old_count: diff.count(ItemStatus::Old),
            document,
            pptx: crate::deck_pptx::DeckPptxOutcome::Unavailable,
        }
    }

    /// The total number of change items (`NEW` + `ADDED` + `UPDATE`).
    #[must_use]
    pub fn change_count(&self) -> usize {
        self.new_count + self.added_count + self.update_count
    }
}

/// Restrict a diff to the elements an FR-014 `--elements` filter names.
///
/// The element entries are kept in FR-003 precedence order, which is the order
/// [`DeckDiff::elements`] already holds, so the filter removes entries and never
/// reorders them. An empty selection means "no filter" and returns the diff
/// unchanged, so a run without `--elements` is byte-identical to one that names
/// all six.
#[must_use]
pub fn filter_elements(diff: &DeckDiff, elements: &[DeckElement]) -> DeckDiff {
    if elements.is_empty() {
        return diff.clone();
    }
    DeckDiff {
        elements: diff
            .elements
            .iter()
            .filter(|entry| {
                entry
                    .element
                    .is_some_and(|element| elements.contains(&element))
            })
            .cloned()
            .collect(),
    }
}

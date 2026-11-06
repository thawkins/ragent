//! Optional PowerPoint export of a diff deck (spec `diffdeck` FR-030).
//!
//! The markdown deck is the canonical artifact: it is deterministic, it needs
//! no capability beyond the filesystem, and every surface can produce it. A
//! `.pptx` is an *additional* rendering of the same data, and it can only be
//! produced when something on the machine knows how to write PowerPoint files.
//!
//! Nothing in this crate knows how that happens. [`DeckSlideSink`] is the seam:
//! the orchestration stage hands it a [`DeckSlidePlan`] describing the deck
//! slide for slide, and a transport-aware implementation elsewhere (in this
//! workspace, `ragent-agent`, over its MCP client) turns that plan into a file.
//! Keeping the trait here rather than in the transport crate is what lets
//! `ragent-research` stay free of an MCP dependency while still driving one.
//!
//! Three properties follow from the export being an addition rather than the
//! artifact:
//!
//! - a run with no sink at all is a complete run, not a degraded one;
//! - a sink that fails leaves the markdown deck untouched and the failure is
//!   reported rather than raised ([`DeckPptxOutcome::Failed`]);
//! - the destination is derived beside the markdown deck by
//!   [`deck_pptx_path`], so the two artifacts of one run always sit together.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::deck::{DeckDiff, DeckDocument, DeckElement, DiffedItem, ElementDiff, ItemStatus};
use crate::deck_progress::DeckProgress;

/// What a slide *is*, so a sink can lay it out instead of dumping text.
///
/// A plain list of body lines renders as a wall of text in every presentation
/// tool: the cover shows raw markdown, a section shows its own front matter, and
/// a finding's heading arrives as literal `###` and backticks. The kind names the
/// slide's job, and the sink chooses a layout, type scale, and geometry for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideKind {
    /// The title slide: deck title, research item, coverage, and counts.
    Cover,
    /// A section opener: the element's name, status, and one-line definition.
    Section,
    /// One finding: a headline, a short observation, and its status.
    Finding,
    /// A ranked table of findings or change counts.
    Table,
    /// The closing slide: what changed, by status.
    Summary,
}

/// The change counts a slide's badge and a summary row show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusCounts {
    /// Items only in the newer report.
    pub new: usize,
    /// Items added with a source the older report did not cite.
    pub added: usize,
    /// Items present in both with changed text.
    pub update: usize,
    /// Items unchanged in both.
    pub old: usize,
}

impl StatusCounts {
    /// The counts of the whole diff.
    #[must_use]
    pub fn from_counts(diff: &DeckDiff) -> Self {
        Self {
            new: diff.count(ItemStatus::New),
            added: diff.count(ItemStatus::Added),
            update: diff.count(ItemStatus::Update),
            old: diff.count(ItemStatus::Old),
        }
    }

    /// The counts of one element's items.
    #[must_use]
    pub fn from_counts_element(entry: &ElementDiff) -> Self {
        Self {
            new: entry.count(ItemStatus::New),
            added: entry.count(ItemStatus::Added),
            update: entry.count(ItemStatus::Update),
            old: entry.count(ItemStatus::Old),
        }
    }

    /// The total number of items the counts cover.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.new + self.added + self.update + self.old
    }

    /// The number of items that are a change (`NEW`, `ADDED`, `UPDATE`).
    #[must_use]
    pub const fn changed(&self) -> usize {
        self.new + self.added + self.update
    }
}

/// One row of a [`SlideKind::Table`] slide.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeckTableRow {
    /// The row's primary text (a finding's headline, or a status name).
    pub label: String,
    /// The row's status, when it has one; a sink colours it.
    pub status: Option<ItemStatus>,
    /// The row's trailing value column (an item count, or a short note).
    pub value: String,
}

/// One slide of the planned PowerPoint deck.
///
/// The plan is text plus structure: it names the slide's kind, a title, a short
/// lead paragraph, up to a handful of bullets, and optionally a table. It still
/// leaves theme, colour, and geometry to the sink, so `ragent-research` stays
/// independent of any particular presentation API, but a sink is no longer asked
/// to guess where a heading ends and a paragraph begins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckSlide {
    /// What the slide is for.
    pub kind: SlideKind,
    /// The slide title.
    pub title: String,
    /// A one- or two-line lead shown under the title (the element's definition,
    /// a finding's observation). Empty when the slide has no lead.
    pub lead: String,
    /// Short bullets shown under the lead, in reading order.
    pub bullets: Vec<String>,
    /// The table the slide shows, when it shows one.
    pub table: Option<Vec<DeckTableRow>>,
    /// The status the slide reports, when it reports one.
    pub status: Option<ItemStatus>,
    /// The change counts the slide's badge reports, when it reports any.
    pub counts: Option<StatusCounts>,
}

impl DeckSlide {
    /// Build a slide of `kind` with a title and no body.
    #[must_use]
    pub fn new(kind: SlideKind, title: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            lead: String::new(),
            bullets: Vec::new(),
            table: None,
            status: None,
            counts: None,
        }
    }

    /// Attach the item status this slide reports.
    #[must_use]
    pub fn with_status(mut self, status: ItemStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Attach the change counts this slide's badge reports.
    #[must_use]
    pub fn with_counts(mut self, counts: StatusCounts) -> Self {
        self.counts = Some(counts);
        self
    }

    /// Set the slide's lead paragraph.
    #[must_use]
    pub fn with_lead(mut self, lead: impl Into<String>) -> Self {
        self.lead = lead.into();
        self
    }

    /// Set the slide's bullets.
    #[must_use]
    pub fn with_bullets(mut self, bullets: Vec<String>) -> Self {
        self.bullets = bullets;
        self
    }

    /// Set the slide's table.
    #[must_use]
    pub fn with_table(mut self, rows: Vec<DeckTableRow>) -> Self {
        self.table = Some(rows);
        self
    }
}

/// Everything a sink needs to write the PowerPoint deck for one run.
///
/// Built by the orchestration stage from the same [`DeckDocument`] and diff the
/// markdown render used, so the two artifacts cannot describe different data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckSlidePlan {
    /// The research item name (the deck's own identifier).
    pub item: String,
    /// The research item title from the newer report, when it declared one.
    pub title: Option<String>,
    /// The older report's stamp label, or `legacy`.
    pub from: String,
    /// The newer report's stamp label.
    pub to: String,
    /// The generation date as `YYYY-MM-DD`.
    pub generated_on: String,
    /// The element slides the run covered, in precedence order.
    pub elements: Vec<DeckElement>,
    /// The slides, in order, including the cover and the closing summary.
    pub slides: Vec<DeckSlide>,
    /// Where the sink must write the presentation.
    pub path: PathBuf,
    /// Whether an existing file at `path` may be replaced.
    pub force: bool,
}

impl DeckSlidePlan {
    /// Render the plan for a completed markdown deck.
    ///
    /// The slides are derived from the same [`DeckDiff`] the markdown deck was
    /// rendered from, not from its markdown text: splitting markdown by `##`
    /// handed a sink raw `###` headings, backticked status tags, table rules and
    /// horizontal rules, and the result was a wall of monospaced text on every
    /// slide. Reading the diff means the presentation carries the data rather
    /// than a transcription of the file, and the two artifacts still describe the
    /// same run.
    #[must_use]
    pub fn from_document(
        item: &str,
        title: Option<String>,
        from: &str,
        to: &str,
        generated_on: &str,
        document: &DeckDocument,
        diff: &DeckDiff,
        include_old: bool,
        path: PathBuf,
        force: bool,
    ) -> Self {
        Self {
            item: item.to_string(),
            title,
            from: from.to_string(),
            to: to.to_string(),
            generated_on: generated_on.to_string(),
            elements: document.elements.clone(),
            slides: slides_from_diff(document, diff, include_old),
            path,
            force,
        }
    }

    /// The number of slides the plan will produce.
    #[must_use]
    pub fn slide_count(&self) -> usize {
        self.slides.len()
    }
}
/// How many bullets one slide may carry before the remainder moves to a
/// continuation slide.
///
/// A slide holding twenty findings is not readable at any font size, and
/// shrinking the text to fit is what produced the unreadable walls this replaced.
/// Splitting keeps every bullet at a legible size instead.
pub const MAX_BULLETS_PER_SLIDE: usize = 5;

/// How many table rows one slide may carry.
pub const MAX_TABLE_ROWS_PER_SLIDE: usize = 8;

/// Render the whole deck from the diff, slide for slide.
///
/// The order is fixed: the cover, then one section slide per element in FR-003
/// precedence order (followed by its findings, [`MAX_BULLETS_PER_SLIDE`] per
/// slide), then the ranked findings table, then the closing summary. Everything
/// here is a pure function of its arguments, so the same run always produces the
/// same presentation (NFR-002).
///
/// `include_old` mirrors the markdown deck's FR-015 switch: when it is set, an
/// element's unchanged items are shown too.
#[must_use]
pub fn slides_from_diff(
    document: &DeckDocument,
    diff: &DeckDiff,
    include_old: bool,
) -> Vec<DeckSlide> {
    let mut slides = vec![cover_slide(document, diff)];

    for element in &document.elements {
        let Some(entry) = diff.element(*element) else {
            continue;
        };
        // The Findings element is presented as a ranked table after the section
        // slides, so its items are not repeated as bullets here.
        if *element == DeckElement::Findings {
            continue;
        }
        let items: Vec<&DiffedItem> = entry
            .items
            .iter()
            .filter(|item| item.status.is_change() || include_old)
            .collect();
        slides.push(section_slide(*element, entry));
        slides.extend(finding_slides(*element, &items));
        slides.extend(bullet_slides(*element, &items));
    }

    if let Some(entry) = diff.element(DeckElement::Findings) {
        let changed: Vec<&DiffedItem> = entry
            .items
            .iter()
            .filter(|item| item.status.is_change())
            .collect();
        slides.extend(findings_table_slides(&changed));
    }

    slides.push(summary_slide(diff, document));
    slides
}

/// The opening title slide.
///
/// The cover names the research item, the comparison window, and the counts,
/// as figures rather than a markdown table.
fn cover_slide(document: &DeckDocument, diff: &DeckDiff) -> DeckSlide {
    let counts = StatusCounts::from_counts(diff);
    let total = diff.item_count();
    let change = counts.changed();
    DeckSlide::new(SlideKind::Cover, deck_title(document))
        .with_lead(format!(
            "{} items compared across {} element{}",
            total,
            document.elements.len(),
            if document.elements.len() == 1 {
                ""
            } else {
                "s"
            }
        ))
        .with_bullets(vec![format!(
            "{} change{}, {} unchanged",
            change,
            if change == 1 { "" } else { "s" },
            counts.old
        )])
        .with_counts(counts)
}

/// The deck's own title, read from its markdown `# ` heading.
///
/// The document carries no separate title field, and the heading is the one
/// place the renderer already writes it, so it is read back rather than taken
/// from a second source that could disagree.
fn deck_title(document: &DeckDocument) -> String {
    document
        .markdown
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(|heading| {
            heading
                .trim()
                .trim_end_matches(" - Research Diff Deck")
                .trim()
                .to_string()
        })
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "Research diff deck".to_string())
}

/// One element's opener slide: its name, whether it changed, and a definition.
///
/// A section slide never carries the element's items. Those are the following
/// slides' job, and putting them here is what made a section a wall of prose.
fn section_slide(element: DeckElement, entry: &ElementDiff) -> DeckSlide {
    let counts = StatusCounts::from_counts_element(entry);
    let status = if counts.changed() > 0 {
        Some(ItemStatus::Added)
    } else if counts.total() > 0 {
        Some(ItemStatus::Old)
    } else {
        None
    };
    let lead = entry
        .items
        .iter()
        .find(|item| item.status.is_change())
        .map(|item| headline(&item.item.text))
        .unwrap_or_default();
    let mut slide = DeckSlide::new(SlideKind::Section, element.label())
        .with_lead(lead)
        .with_counts(counts)
        .with_bullets(vec![format!(
            "{} item{}, {} changed",
            counts.total(),
            if counts.total() == 1 { "" } else { "s" },
            counts.changed()
        )]);
    if let Some(status) = status {
        slide = slide.with_status(status);
    }
    slide
}

/// One slide per non-finding item, showing its status and a readable summary.
///
/// The lead comes from the item's `detail` first: a concept's definition prose
/// and a finding's observation/analysis paragraphs both live there, while the
/// item's own text is only its heading. Reading the heading back gave every
/// concept slide a body that was a copy of its title, which the `lead != title`
/// guard below then dropped - so the slide showed a heading and an empty body.
fn finding_slides(element: DeckElement, items: &[&DiffedItem]) -> Vec<DeckSlide> {
    if element == DeckElement::Findings {
        return Vec::new();
    }
    items
        .iter()
        .filter_map(|item| {
            let title = headline(&item.item.text);
            if title.is_empty() {
                return None;
            }
            let lead = item
                .item
                .detail
                .iter()
                .find(|paragraph| !paragraph.trim().is_empty())
                .map_or_else(
                    || observation(&item.item.text, 340),
                    |paragraph| truncate_words(paragraph.trim(), 340),
                );
            let mut slide =
                DeckSlide::new(SlideKind::Finding, title.clone()).with_status(item.status);
            if !lead.is_empty() && lead != title {
                slide = slide.with_lead(lead);
            }
            Some(slide)
        })
        .collect()
}

/// The element's remaining items as short bullet slides, grouped by status.
///
/// Only items with no readable headline reach here; a bullet slide is the
/// fallback that loses the least text, and it is chunked to
/// [`MAX_BULLETS_PER_SLIDE`] so the type stays legible.
fn bullet_slides(element: DeckElement, items: &[&DiffedItem]) -> Vec<DeckSlide> {
    let bullets: Vec<(ItemStatus, String)> = items
        .iter()
        .map(|item| {
            let text = crate::deck::strip_markdown_noise(&item.item.text);
            let bullet = if element == DeckElement::References {
                truncate_words(&text, 150)
            } else {
                truncate_words(&text, 170)
            };
            (item.status, bullet)
        })
        .filter(|(_, bullet)| !bullet.is_empty())
        .collect();
    if bullets.is_empty() {
        return Vec::new();
    }
    let chunks: Vec<_> = bullets.chunks(MAX_BULLETS_PER_SLIDE).collect();
    let mut slides = Vec::with_capacity(chunks.len());
    for (index, chunk) in chunks.iter().enumerate() {
        let title = if chunks.len() == 1 {
            element.label().to_string()
        } else {
            format!("{} ({}/{})", element.label(), index + 1, chunks.len())
        };
        let slide = DeckSlide::new(SlideKind::Finding, title)
            .with_bullets(chunk.iter().map(|(_, text)| text.clone()).collect());
        let slide = match chunk.first().map(|(status, _)| *status) {
            Some(status) => slide.with_status(status),
            None => slide,
        };
        slides.push(slide);
    }
    slides
}

/// The ranked findings table, chunked across slides.
fn findings_table_slides(items: &[&DiffedItem]) -> Vec<DeckSlide> {
    if items.is_empty() {
        return Vec::new();
    }
    let chunks: Vec<_> = items.chunks(MAX_TABLE_ROWS_PER_SLIDE).collect();
    let mut slides = Vec::with_capacity(chunks.len());
    for (index, chunk) in chunks.iter().enumerate() {
        let title = if chunks.len() == 1 {
            "Findings".to_string()
        } else {
            format!("Findings ({}/{})", index + 1, chunks.len())
        };
        let rows = chunk
            .iter()
            .map(|item| DeckTableRow {
                label: headline(&item.item.text),
                status: Some(item.status),
                value: String::new(),
            })
            .collect();
        slides.push(
            DeckSlide::new(SlideKind::Table, title)
                .with_lead("Findings that changed, in report order")
                .with_table(rows),
        );
    }
    slides
}

/// The closing slide: what changed, by status.
fn summary_slide(diff: &DeckDiff, document: &DeckDocument) -> DeckSlide {
    let counts = StatusCounts::from_counts(diff);
    let rows = [
        ItemStatus::New,
        ItemStatus::Added,
        ItemStatus::Update,
        ItemStatus::Old,
    ]
    .into_iter()
    .map(|status| DeckTableRow {
        label: status.tag().to_string(),
        status: Some(status),
        value: diff.count(status).to_string(),
    })
    .collect();
    DeckSlide::new(SlideKind::Summary, "What changed")
        .with_lead(format!(
            "{} items compared across {} elements",
            counts.total(),
            document.elements.len()
        ))
        .with_table(rows)
        .with_counts(counts)
}

/// A finding's headline: its first sentence, markdown stripped and capped.
fn headline(text: &str) -> String {
    let clean = crate::deck::strip_markdown_noise(text);
    let clean = strip_leading_numbering(&clean);
    first_sentence(&clean, 90)
}

/// A finding's observation: the rest of its text, markdown stripped and capped.
///
/// The observation starts where the headline stopped. Returning the first
/// sentence again made every finding's lead a truncated copy of its own title,
/// which the caller then discarded as `lead == title` - so a single-finding
/// slide carried a title and an empty body. The tail is what the reader has not
/// already read in the heading.
fn observation(text: &str, limit: usize) -> String {
    let clean = crate::deck::strip_markdown_noise(text);
    let clean = strip_leading_numbering(&clean);
    let headline = crate::deck::first_sentence(&clean);
    let tail = clean
        .trim_start()
        .strip_prefix(headline.trim())
        .unwrap_or("");
    // A text that is a single sentence has no tail: its own words make the
    // weaker but truthful lead, and the slide still says what the finding is.
    let rest = if tail.trim().is_empty() {
        clean.as_str()
    } else {
        tail
    };
    truncate_words(rest.trim(), limit)
}

/// Drop a leading `N. ` or `N) ` numbering a report heading may carry.
fn strip_leading_numbering(text: &str) -> String {
    let trimmed = text.trim_start();
    let digits: String = trimmed.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return trimmed.to_string();
    }
    let rest = &trimmed[digits.len()..];
    let rest = rest
        .strip_prefix(". ")
        .or_else(|| rest.strip_prefix(") "))
        .or_else(|| rest.strip_prefix(' '));
    rest.unwrap_or(trimmed).trim_start().to_string()
}

/// The first sentence of `text`, capped at `limit` characters on a word boundary.
fn first_sentence(text: &str, limit: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let sentence = crate::deck::first_sentence(trimmed);
    let sentence = sentence.trim();
    truncate_words(sentence, limit)
}

/// `text` cut at `limit` characters on a word boundary, with an ellipsis.
fn truncate_words(text: &str, limit: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let cut: String = text.chars().take(limit).collect();
    let cut = cut
        .rsplit_once(' ')
        .map(|(head, _)| head)
        .unwrap_or(cut.as_str());
    format!("{}...", cut.trim_end_matches(['.', ',', ';', ':']))
}

/// What the PowerPoint export did, if anything (FR-030).
///
/// The variant is carried on [`crate::deck::DeckOutcome`] so every surface can
/// say what happened without re-deriving it: a run whose sink was absent, one
/// whose sink wrote a file, and one whose sink failed are all distinguishable,
/// and none of them changes the markdown deck's own success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeckPptxOutcome {
    /// No PowerPoint capability was available, so only the markdown deck was
    /// written.
    Unavailable,
    /// The presentation was written.
    Written {
        /// Where it was written.
        path: PathBuf,
        /// How many slides it holds.
        slides: usize,
        /// The MCP server (or other provider) that wrote it.
        provider: String,
    },
    /// A sink was available but did not produce the file.
    Failed {
        /// Why it failed, in a form fit to show the user.
        reason: String,
    },
}

impl DeckPptxOutcome {
    /// The written presentation, when one was written.
    #[must_use]
    pub fn written_path(&self) -> Option<&Path> {
        match self {
            Self::Written { path, .. } => Some(path),
            Self::Unavailable | Self::Failed { .. } => None,
        }
    }

    /// A one-line report suitable for a success line or a notice.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Unavailable => "no PowerPoint MCP server available".to_string(),
            Self::Written {
                path,
                slides,
                provider,
            } => format!(
                "wrote `{}` ({slides} slides, via {provider})",
                path.display()
            ),
            Self::Failed { reason } => format!("PowerPoint export failed: {reason}"),
        }
    }
}

/// The `.pptx` path that sits beside a markdown deck (FR-030).
///
/// The stem is kept and the extension replaced, so `DECK-2026-11-04.md` and
/// `DECK-2026-11-04.pptx` are the two artifacts of one run and neither can be
/// mistaken for a different run's.
#[must_use]
pub fn deck_pptx_path(markdown_path: &Path) -> PathBuf {
    markdown_path.with_extension("pptx")
}

/// The transport that writes a PowerPoint deck from a [`DeckSlidePlan`].
///
/// Implementations are responsible for their own capabilities: an
/// implementation backed by MCP servers reports [`DeckPptxOutcome::Unavailable`]
/// when none of its connected servers can write a presentation, rather than
/// treating that as an error. A returned `Failed` is a sink that tried and could
/// not; the orchestration stage logs either way and never fails the run.
#[async_trait]
pub trait DeckSlideSink: Send + Sync {
    /// Write the presentation described by `plan`.
    ///
    /// The implementation must write to `plan.path` (honouring `plan.force`)
    /// and must not touch the markdown deck.
    async fn write_deck(&self, plan: &DeckSlidePlan) -> DeckPptxOutcome;

    /// Write the presentation, reporting each slide as it is drawn (FR-031).
    ///
    /// A presentation is written one slide at a time over a tool channel, so a
    /// ten-slide deck is ten round trips: without per-slide reports the export
    /// is the longest silent stretch of the run. `progress` receives
    /// [`DeckStage::Export`] reports carrying the slide count.
    ///
    /// The default implementation delegates to [`Self::write_deck`] and reports
    /// nothing, so a sink that has no natural per-slide point keeps working
    /// unchanged; a sink that can see its slides overrides it.
    async fn write_deck_with_progress(
        &self,
        plan: &DeckSlidePlan,
        _progress: Option<&dyn DeckProgress>,
    ) -> DeckPptxOutcome {
        self.write_deck(plan).await
    }
}

//! Bounded local file/folder acquisition for `/spec govcreate` (spec `govdoc`
//! T-007).
//!
//! Implements **FR-005** (local content-reference acquisition), the local half
//! of **FR-016**-style bounding (file cap and total-character budget),
//! **NFR-002** (pure, filesystem-independent budget logic), **NFR-003**
//! (local acquisition cannot escape the invoking tree: no symlink is followed
//! and the walk never leaves the validated root), and supports **NFR-004**
//! (the walk and per-file extraction are deterministic local I/O).
//!
//! A local content reference is acquired by walking the validated path (a
//! single file, or a directory tree including nested subdirectories),
//! extracting text from every supported document via
//! [`extract_file_as_markdown`], and normalising the results into the same
//! source-agnostic [`GatheredCorpus`] shape the URL path (T-006) produces, so
//! the extraction stage never needs to know where the text came from.
//!
//! # Supported-format filtering
//!
//! A file whose extension is not a supported document type
//! ([`detect_document_format`]) is skipped and recorded as
//! [`ExclusionReason::UnsupportedFormat`]; a supported file whose extraction
//! fails or yields no usable text is recorded as
//! [`ExclusionReason::ReadFailed`] / [`ExclusionReason::NoContent`]. None of
//! these aborts the walk: one unreadable file never blocks the rest of a
//! directory tree.
//!
//! # Safety (NFR-003)
//!
//! Path resolution and escape checks happen once in
//! [`validate_local_path`](super::content_ref::validate_local_path) before
//! this module is entered; the walk itself never follows symlinked entries
//! (`DirEntry::file_type` does not resolve links) and never touches a path
//! outside the walked root.
//!
//! # Scope
//!
//! Mirroring T-006, an acquisition that yields no usable text returns an
//! empty corpus rather than an error: the FR-006 empty/unreadable termination
//! belongs to the orchestration stage (T-012), which sees the empty corpus
//! before scaffolding. The only errors returned here are per-file I/O
//! failures the walk cannot even start for (e.g. the root became unreadable
//! between validation and acquisition), surfaced as zero sources plus
//! excluded entries rather than a hard error wherever possible.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::document_extract::{detect_document_format, extract_file_as_markdown};
use crate::masterfetch::PageType;

use super::url_source::{
    AcquisitionStats, BudgetReason, ExcludedSource, GatheredCorpus, GatheredSource,
};

// ---------------------------------------------------------------------------
// Local acquisition budget (NFR-002)
// ---------------------------------------------------------------------------

/// The bounded budget for one local acquisition.
///
/// Local acquisition has no crawl depth or robots concern: the budget is a
/// file cap, a total-character budget, and a wall-clock deadline. `Default`
/// uses a 100-file cap (the NFR-004 sizing point: 100 files must acquire in
/// under 30 seconds), a 200,000-character text budget matching the crawl
/// engine's default total, and the same 120-second deadline the URL path
/// uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalAcquisitionBudget {
    /// Maximum number of supported files to extract.
    pub max_files: usize,
    /// Total character budget across all extracted documents.
    pub max_total_chars: usize,
    /// Wall-clock time budget in milliseconds.
    pub deadline_ms: u64,
}

impl Default for LocalAcquisitionBudget {
    fn default() -> Self {
        Self {
            max_files: DEFAULT_MAX_FILES,
            max_total_chars: DEFAULT_LOCAL_MAX_TOTAL_CHARS,
            deadline_ms: super::url_source::AcquisitionBudget::default().deadline_ms,
        }
    }
}

impl LocalAcquisitionBudget {
    /// Build a budget from explicit caps.
    #[must_use]
    pub const fn new(max_files: usize, max_total_chars: usize, deadline_ms: u64) -> Self {
        Self {
            max_files,
            max_total_chars,
            deadline_ms,
        }
    }
}

/// Default file cap: the NFR-004 sizing point (100 files under 30 seconds).
pub const DEFAULT_MAX_FILES: usize = 100;

/// Default total-character budget, aligned with the crawl engine's default so
/// URL and local acquisition hand similarly-sized corpora to extraction.
pub const DEFAULT_LOCAL_MAX_TOTAL_CHARS: usize = 200_000;

/// The measured outcome of a local acquisition, as the pure budget check
/// needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LocalAcquisitionStats {
    /// Number of supported files actually extracted (usable or not).
    pub files_extracted: usize,
    /// Total characters of gathered (usable) source text.
    pub total_chars: usize,
    /// Wall-clock duration of the acquisition in milliseconds.
    pub elapsed_ms: u64,
}

/// Which local budget stopped an acquisition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalBudgetReason {
    /// The file cap was reached.
    MaxFiles,
    /// The total-character budget was reached.
    MaxTotalChars,
    /// The wall-clock deadline was reached.
    Deadline,
}

impl LocalBudgetReason {
    /// Translate to the corpus-level [`BudgetReason`] for reporting.
    #[must_use]
    pub const fn as_corpus_reason(self) -> BudgetReason {
        match self {
            Self::MaxFiles => BudgetReason::MaxPages,
            Self::MaxTotalChars => BudgetReason::MaxTotalChars,
            Self::Deadline => BudgetReason::Deadline,
        }
    }
}

impl std::fmt::Display for LocalBudgetReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::MaxFiles => "file cap",
            Self::MaxTotalChars => "character budget",
            Self::Deadline => "deadline",
        };
        f.write_str(label)
    }
}

/// Report which budget stopped a local acquisition (NFR-002).
///
/// Pure: it compares measured [`LocalAcquisitionStats`] against a
/// [`LocalAcquisitionBudget`] with no I/O, checking the deadline first, then
/// the file cap, then the character budget - the same precedence the walk
/// applies.
#[must_use]
pub fn local_budget_exhausted(
    stats: &LocalAcquisitionStats,
    limits: &LocalAcquisitionBudget,
) -> Option<LocalBudgetReason> {
    if stats.elapsed_ms >= limits.deadline_ms {
        return Some(LocalBudgetReason::Deadline);
    }
    if stats.files_extracted >= limits.max_files {
        return Some(LocalBudgetReason::MaxFiles);
    }
    if stats.total_chars >= limits.max_total_chars {
        return Some(LocalBudgetReason::MaxTotalChars);
    }
    None
}

// ---------------------------------------------------------------------------
// Local exclusion reasons (FR-005)
// ---------------------------------------------------------------------------

/// Why a walked directory entry was not included in the gathered corpus
/// (FR-005).
///
/// `Copy`-free widened set for the local path; translated to
/// [`super::url_source::ExclusionReason`] when the corpus records an
/// exclusion so the two acquisition paths share one exclusion type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalExclusionReason {
    /// The file's extension is not a supported document format.
    UnsupportedFormat,
    /// The file is supported but text extraction failed.
    ReadFailed,
    /// The file was extracted but yielded no usable text.
    NoContent,
}

impl LocalExclusionReason {
    /// Translate to the shared corpus [`ExclusionReason`].
    ///
    /// `UnsupportedFormat` has no URL-path equivalent; it maps to
    /// `NoContent` because the corpus-level distinction that matters to the
    /// report is "this source contributed no text".
    #[must_use]
    pub const fn as_corpus_reason(self) -> super::url_source::ExclusionReason {
        match self {
            Self::UnsupportedFormat | Self::NoContent => {
                super::url_source::ExclusionReason::NoContent
            }
            Self::ReadFailed => super::url_source::ExclusionReason::FetchFailed,
        }
    }
}

impl std::fmt::Display for LocalExclusionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::UnsupportedFormat => "unsupported format",
            Self::ReadFailed => "read failed",
            Self::NoContent => "no usable content",
        };
        f.write_str(label)
    }
}

// ---------------------------------------------------------------------------
// Pure walk planning (NFR-002)
// ---------------------------------------------------------------------------

/// One entry selected for extraction, in deterministic walk order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalWalkEntry {
    /// The file path, relative to the walked root where possible.
    pub path: PathBuf,
    /// `true` when the extension is a supported document format.
    pub supported: bool,
}

/// Classify one walked file as supported or not, given its path only.
///
/// Pure and I/O-free: the decision is entirely extension-based. Extracted
/// into a named function so the supported-format rule is unit-testable
/// without touching the filesystem (NFR-002).
#[must_use]
pub fn is_supported_document(path: &Path) -> bool {
    detect_document_format(path).is_ok()
}

// ---------------------------------------------------------------------------
// Acquisition (FR-005)
// ---------------------------------------------------------------------------

/// Acquire a local content reference into a [`GatheredCorpus`] (FR-005).
///
/// `path` must already have passed
/// [`validate_local_path`](super::content_ref::validate_local_path): this
/// function assumes the path is canonical, in-tree, and is either a supported
/// file or a directory holding at least one supported document. The walk is
/// deterministic: directories are visited depth-first in lexical order of
/// their entries, and no symlink is followed, so a local acquisition cannot
/// escape the validated root (NFR-003).
///
/// Extraction runs on the calling thread; callers in an async context should
/// wrap the call in `tokio::task::spawn_blocking` (matching the document
/// extraction convention in
/// [`extract_file_as_markdown`](crate::document_extract::extract_file_as_markdown)).
#[must_use]
pub fn acquire_local(path: &Path, budget: &LocalAcquisitionBudget) -> GatheredCorpus {
    let started = Instant::now();
    let mut sources: Vec<GatheredSource> = Vec::new();
    let mut excluded: Vec<ExcludedSource> = Vec::new();
    let mut seen_excluded: HashSet<String> = HashSet::new();
    let mut stats = LocalAcquisitionStats::default();

    // Collect the candidate files first so the walk order is deterministic:
    // a single-file reference is a one-element list, a directory reference
    // walks depth-first with each level sorted lexically.
    let mut candidates: Vec<PathBuf> = Vec::new();
    if path.is_file() {
        candidates.push(path.to_path_buf());
    } else if path.is_dir() {
        let mut stack = vec![path.to_path_buf()];
        while let Some(current) = stack.pop() {
            let Ok(read_dir) = std::fs::read_dir(&current) else {
                push_local_excluded(
                    &mut excluded,
                    &mut seen_excluded,
                    &current,
                    LocalExclusionReason::ReadFailed,
                );
                continue;
            };
            let mut subdirs: Vec<PathBuf> = Vec::new();
            let mut files: Vec<PathBuf> = Vec::new();
            for entry in read_dir.flatten() {
                let entry_path = entry.path();
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_dir() {
                    subdirs.push(entry_path);
                } else if file_type.is_file() {
                    files.push(entry_path);
                }
                // Symlinks are deliberately ignored: DirEntry::file_type does
                // not follow links, and never pushing them keeps the walk
                // in-tree and loop-free (NFR-003).
            }
            files.sort();
            subdirs.sort();
            candidates.extend(files);
            // Push subdirs in reverse so the lexical-first directory pops
            // first, keeping a deterministic depth-first order.
            for subdir in subdirs.into_iter().rev() {
                stack.push(subdir);
            }
        }
    }

    for file in candidates {
        let elapsed_now = started.elapsed().as_millis() as u64;
        stats.elapsed_ms = elapsed_now;
        // The deadline is checked per file before extraction; with a zero
        // deadline at least the first supported file is still attempted so the
        // reason (Deadline) is reported rather than a bare empty corpus.
        if elapsed_now >= budget.deadline_ms && !sources.is_empty() {
            break;
        }
        if stats.files_extracted >= budget.max_files || stats.total_chars >= budget.max_total_chars
        {
            break;
        }

        let label = file.display().to_string();
        if !is_supported_document(&file) {
            push_local_excluded(
                &mut excluded,
                &mut seen_excluded,
                &file,
                LocalExclusionReason::UnsupportedFormat,
            );
            continue;
        }

        stats.files_extracted += 1;
        let document = match extract_file_as_markdown(&file) {
            Ok(document) => document,
            Err(_) => {
                push_local_excluded(
                    &mut excluded,
                    &mut seen_excluded,
                    &file,
                    LocalExclusionReason::ReadFailed,
                );
                continue;
            }
        };
        let content = document.content.trim();
        if content.is_empty() {
            push_local_excluded(
                &mut excluded,
                &mut seen_excluded,
                &file,
                LocalExclusionReason::NoContent,
            );
            continue;
        }

        let remaining = budget.max_total_chars.saturating_sub(stats.total_chars);
        let mut content = content.to_owned();
        if content.chars().count() > remaining {
            content = content.chars().take(remaining).collect();
        }
        stats.total_chars += content.chars().count();

        sources.push(GatheredSource {
            url: label.clone(),
            summary: label,
            // Local documents have no web page type; `Docs` is the nearest
            // truthful classification for architecture documents.
            page_type: PageType::Docs,
            content,
        });
        stats.elapsed_ms = started.elapsed().as_millis() as u64;
        // Label unused: see GatheredSource construction above.

        if stats.total_chars >= budget.max_total_chars {
            break;
        }
    }

    stats.elapsed_ms = started.elapsed().as_millis() as u64;
    let budget_reached = local_budget_exhausted(&stats, budget).map(|r| r.as_corpus_reason());

    let corpus_stats = AcquisitionStats {
        pages_fetched: stats.files_extracted,
        total_chars: stats.total_chars,
        elapsed_ms: stats.elapsed_ms,
    };

    let text = join_local_text(&sources);

    GatheredCorpus {
        reference: path.display().to_string(),
        sources,
        excluded,
        text,
        stats: corpus_stats,
        budget_reached,
    }
}

/// Record an excluded path once, preserving first-occurrence order.
fn push_local_excluded(
    excluded: &mut Vec<ExcludedSource>,
    seen: &mut HashSet<String>,
    path: &Path,
    reason: LocalExclusionReason,
) {
    let key = path.display().to_string();
    if seen.insert(key.clone()) {
        excluded.push(ExcludedSource {
            url: key,
            reason: reason.as_corpus_reason(),
        });
    }
}

/// Flatten the gathered local sources into one bounded text block (FR-005),
/// matching the URL path's `## <summary> (<path>)` header convention.
fn join_local_text(sources: &[GatheredSource]) -> String {
    let mut out = String::new();
    for source in sources {
        out.push_str(&format!(
            "## {}\n{}\n\n{}\n\n",
            source.summary, source.url, source.content
        ));
    }
    out
}

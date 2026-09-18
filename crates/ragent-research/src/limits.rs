//! Shared output limits and reverse-relevance ordering for `/research create`.
//!
//! Spec `researchmax` (FR-001, FR-002, FR-005, NFR-001) caps the number of
//! concepts and findings rendered by the `/research create` report and orders
//! both lists most-relevant-first before truncation. This module holds the
//! built-in defaults and the single ordering helper shared by the findings
//! (`analysis`) and concept-extraction (`cluster`) paths, so both derive an
//! entry's relevance identically from already-loaded citation markers and
//! source metadata.
//!
//! ## Dependencies
//!
//! - [`crate::source::Source`] - supplies [`Source::relevance_rank`], the
//!   existing 8..1 relevance vocabulary reused here.
//! - [`crate::polarity::citation_re`] - the shared `[#N]` citation regex, so
//!   the citation syntax stays consistent across the crate.

use crate::source::Source;
use regex::Regex;
use std::sync::OnceLock;

/// Default maximum number of concept sections in a `/research create` report.
pub const DEFAULT_MAX_CONCEPTS: usize = 5;

/// Default maximum number of findings in a `/research create` report.
pub const DEFAULT_MAX_FINDINGS: usize = 20;

/// Relevance rank assigned to an entry with no recognized citation (FR-005).
///
/// Matches the medium rank [`Source::relevance_rank`] returns for a source
/// without a relevance label, so uncited entries are ordered fairly rather
/// than dropped first.
pub const DEFAULT_ENTRY_RANK: u8 = 5;

/// The legacy `web-NN` supporting-file citation form.
///
/// Models trained on the older concept-extraction prompt still emit
/// `web-NN` filename citations; they are accepted here so ordering works both
/// before and after the rewrite to `[#N]`. Shared with `cluster`'s citation
/// rewriter so the parse pattern stays identical across the crate.
pub(crate) fn web_ref_re() -> &'static Regex {
    static WEB_REF_RE: OnceLock<Regex> = OnceLock::new();
    WEB_REF_RE.get_or_init(|| Regex::new(r"\bweb-(\d+)\b").expect("valid web-ref regex"))
}

/// The numeric prefix pattern used to strip leading counters from concept
/// headings (`## N. label` / `### N. label`).
///
/// Matches a leading run of digits followed by a delimiter (`.`, `)`, `:`,
/// `-`, en dash, em dash) and trailing whitespace, so `3. Topic`,
/// `12) Topic`, and `7 — Topic` all strip to `Topic`. Shared with
/// `cluster`'s heading renumbering so the parse pattern stays identical
/// across the crate.
pub(crate) fn num_prefix_re() -> &'static Regex {
    static NUM_PREFIX_RE: OnceLock<Regex> = OnceLock::new();
    NUM_PREFIX_RE.get_or_init(|| {
        Regex::new(r"^\d+\s*[.):\-\u{2013}\u{2014}]\s*").expect("valid num-prefix regex")
    })
}

/// Extract the distinct, 1-based source indices cited by `body`.
///
/// Recognises both the primary `[#N]` citation form and the legacy `web-NN`
/// filename form. Parsed indices are filtered to `> 0`, sorted, and
/// deduplicated so a body that cites the same source twice counts once.
/// Malformed or non-numeric markers are ignored and never panic (FR-021).
#[must_use]
pub fn cited_source_indices(body: &str) -> Vec<usize> {
    let mut indices: Vec<usize> = crate::polarity::citation_re()
        .captures_iter(body)
        .filter_map(|cap| cap[1].parse::<usize>().ok())
        .chain(
            web_ref_re()
                .captures_iter(body)
                .filter_map(|cap| cap[1].parse::<usize>().ok()),
        )
        .filter(|n| *n > 0)
        .collect();
    indices.sort_unstable();
    indices.dedup();
    indices
}

/// Resolve each distinct source cited by `body` to its relevance rank.
///
/// `rank_of` maps a 1-based source index to its [`Source::relevance_rank`] and
/// returns `None` for indices outside the gathered corpus; unknown indices are
/// dropped. The returned ranks follow ascending source-index order (they are
/// not sorted by value), and duplicates are not removed so the vector length
/// counts the distinct sources cited. A body with no recognized citation
/// yields an empty vector; callers substitute [`DEFAULT_ENTRY_RANK`] in that
/// case.
#[must_use]
pub fn cited_source_ranks(body: &str, rank_of: &impl Fn(usize) -> Option<u8>) -> Vec<u8> {
    cited_source_indices(body)
        .into_iter()
        .filter_map(rank_of)
        .collect()
}

/// Build a `[#N]` index to relevance-rank lookup over gathered `sources`.
///
/// `N` is the 1-based References Index position in the report, so indices
/// outside `sources` yield `None`. Findings cite sources directly in this
/// form, so the lookup is shared across report sections.
#[must_use]
pub fn source_rank_lookup(sources: &[Source]) -> impl Fn(usize) -> Option<u8> + '_ {
    move |index| {
        index
            .checked_sub(1)
            .and_then(|i| sources.get(i))
            .map(Source::relevance_rank)
    }
}

/// Order `entries` most-relevant-first (FR-003, FR-004).
///
/// Each entry's relevance is derived from the source ranks cited in its body,
/// which `body_of` returns. Entries are sorted by
/// `(highest cited rank descending, cited count descending, original index
/// ascending)`, so ties fall back to the model's original ordering and an
/// entry with no recognized citation is treated as [`DEFAULT_ENTRY_RANK`].
///
/// `rank_of` maps a 1-based source index to its [`Source::relevance_rank`];
/// see [`source_rank_lookup`] for the common gathered-source case. The pass is
/// `O(n log n)` and reads only the supplied bodies and metadata (NFR-001).
#[must_use]
pub fn rank_entries_by_reverse_relevance<T, F, R>(entries: Vec<T>, body_of: F, rank_of: R) -> Vec<T>
where
    F: Fn(&T) -> &str,
    R: Fn(usize) -> Option<u8>,
{
    let mut ranked: Vec<(u8, usize, usize, T)> = entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            let ranks = cited_source_ranks(body_of(&entry), &rank_of);
            let max_rank = ranks.iter().copied().max().unwrap_or(DEFAULT_ENTRY_RANK);
            (max_rank, ranks.len(), index, entry)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
    ranked.into_iter().map(|(_, _, _, entry)| entry).collect()
}

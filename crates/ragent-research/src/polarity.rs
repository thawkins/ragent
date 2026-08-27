//! Internal shared helpers used across analysis modules
//! (contradiction, corpus_critic, reconcile).
//!
//! These helpers are NOT part of the crate's public API. They live here to
//! eliminate the duplicated `source_body_text` and `depth_from_count` helpers
//! that previously existed in each module independently.

use crate::locus::DepthLevel;
use crate::source::Source;
use regex::Regex;
use std::sync::OnceLock;

/// Extract searchable body text from a source.
///
/// For `Web`, `Local`, and `Other` sources this returns the body text; for
/// `Spec` sources it returns the spec id (the only text available without
/// reading the spec file from disk).
pub(crate) fn source_body_text(source: &Source) -> String {
    match source {
        Source::Web { body, .. } => body.clone(),
        Source::Local { body, .. } => body.clone(),
        Source::Spec { spec_id, .. } => spec_id.clone(),
        Source::Other { body, .. } => body.clone(),
    }
}

/// Classify a source-count into the same depth levels used by `locus.rs`.
///
/// `0–1` → `Surface`, `2–3` → `Moderate`, `4+` → `Deep`.
pub(crate) fn depth_from_count(n: usize) -> DepthLevel {
    match n {
        0 | 1 => DepthLevel::Surface,
        2 | 3 => DepthLevel::Moderate,
        _ => DepthLevel::Deep,
    }
}

/// Return true when `body` contains any of the supplied tokens.
pub(crate) fn has_any_token(body: &str, tokens: &[&str]) -> bool {
    tokens.iter().any(|t| body.contains(t))
}

/// The shared `[#N]` citation-reference regex, compiled once.
///
/// Used by synthesis, verification, cite-checking, and document rendering;
/// a single definition keeps the citation syntax consistent everywhere.
pub(crate) fn citation_re() -> &'static Regex {
    static CITATION_RE: OnceLock<Regex> = OnceLock::new();
    CITATION_RE.get_or_init(|| Regex::new(r"\[#(\d+)\]").expect("valid citation regex"))
}

/// Extract the distinct, 1-based source indices cited by `text` via `[#N]`.
///
/// Indices are parsed, filtered to `> 0`, sorted, and deduplicated so callers
/// can iterate citations without repeating the capture loop. Shared by the
/// verification, synthesis, and cite-checking passes.
pub(crate) fn cited_indices(text: &str) -> Vec<usize> {
    let mut out: Vec<usize> = citation_re()
        .captures_iter(text)
        .filter_map(|cap| cap[1].parse().ok())
        .filter(|n| *n > 0)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

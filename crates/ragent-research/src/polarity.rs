//! Internal shared helpers used across analysis modules
//! (contradiction, corpus_critic, reconcile).
//!
//! These helpers are NOT part of the crate's public API. They live here to
//! eliminate the duplicated `source_body_text` and `depth_from_count` helpers
//! that previously existed in each module independently.

use crate::locus::DepthLevel;
use crate::source::Source;

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

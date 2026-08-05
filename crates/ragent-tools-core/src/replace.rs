//! Shared exact-byte replacement matcher.
//!
//! [`find_exact_replacement_range`] is the canonical matcher used by every
//! replace-style tool in ragent (`edit`, `multi_edit`, `apply_patch`;
//! historically also the legacy `memory_replace`). It locates a unique byte
//! range `[start, end)` in `content` where `needle` should be replaced.
//!
//! # Matching semantics
//!
//! Matching is **strict exact-byte**: the needle must occur exactly once,
//! byte-for-byte. There is no CRLF tolerance, no trailing/leading whitespace
//! tolerance, no indentation re-application, and no blank-line or final-newline
//! normalisation. What you read (bytes) is what you match.

/// Error returned by [`find_exact_replacement_range`] when no unique match is
/// found.
#[derive(Debug)]
pub enum FindError {
    /// The needle does not occur anywhere in the content.
    NotFound,
    /// The needle occurs at more than one location; carries the match count.
    MultipleMatches(usize),
}

/// Find the unique byte range `[start, end)` in `content` where `needle` should
/// be replaced using **only** exact substring matching.
///
/// Whitespace, indentation, and line endings must match exactly.
///
/// Returns `(start, end, new_str)` on success. `new_str` is returned unchanged
/// because exact matching never needs indentation re-application.
///
/// # Errors
///
/// - [`FindError::NotFound`] if `needle` does not occur in `content`.
/// - [`FindError::MultipleMatches(n)`] if `needle` occurs more than once.
pub fn find_exact_replacement_range(
    content: &str,
    needle: &str,
    new_str: &str,
) -> Result<(usize, usize, String), FindError> {
    let count = content.matches(needle).count();
    if count == 0 {
        return Err(FindError::NotFound);
    }
    if count > 1 {
        return Err(FindError::MultipleMatches(count));
    }
    let start = content.find(needle).unwrap();
    Ok((start, start + needle.len(), new_str.to_string()))
}

/// Diagnostic kind carried by [`FindDiag`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindDiagKind {
    /// The needle does not occur anywhere in the content.
    NotFound,
    /// The needle occurs at more than one location; carries the match count.
    MultipleMatches(usize),
}

/// A richer replacement-failure diagnostic used by `edit` and `multi_edit` to
/// produce actionable error messages.
#[derive(Debug, Clone)]
pub struct FindDiag {
    /// What kind of failure occurred.
    pub kind: FindDiagKind,
}

impl FindDiag {
    /// Build a `NotFound` diagnostic.
    #[must_use]
    pub const fn not_found() -> Self {
        Self {
            kind: FindDiagKind::NotFound,
        }
    }

    /// Build a `MultipleMatches` diagnostic.
    #[must_use]
    pub const fn multiple(count: usize) -> Self {
        Self {
            kind: FindDiagKind::MultipleMatches(count),
        }
    }
}

impl From<FindDiag> for FindError {
    fn from(d: FindDiag) -> Self {
        match d.kind {
            FindDiagKind::NotFound => Self::NotFound,
            FindDiagKind::MultipleMatches(n) => Self::MultipleMatches(n),
        }
    }
}

/// Format a [`FindDiag`] into an actionable error message.
///
/// The message names the file path, explains whether the needle was not found
/// or matched multiple times, and reminds the caller that a byte-for-byte
/// match is required (re-read the file to obtain exact bytes).
#[must_use]
pub fn format_match_failure(diag: &FindDiag, path: &std::path::Path) -> String {
    match diag.kind {
        FindDiagKind::NotFound => format!(
            "old_string not found in {}. Matching is byte-for-byte exact: \
             indentation, whitespace, and line endings must match precisely. \
             Re-read the file and include 3–5 lines of context around the \
             change point.",
            path.display(),
        ),
        FindDiagKind::MultipleMatches(n) => format!(
            "old_string found {} times in {}. It must match exactly once. \
             Add more surrounding context to make the match unique.",
            n,
            path.display(),
        ),
    }
}

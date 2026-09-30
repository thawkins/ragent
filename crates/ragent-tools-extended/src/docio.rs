//! Shared document-output helpers used by the Office and `LibreOffice` tool
//! families.
//!
//! The two families previously carried near-copies of the output-truncation
//! routine (see `ANTIPAT.md` M3.8); the shared implementation lives here.

use ragent_types::strutil::{floor_char_boundary, truncate_bytes_no_ellipsis};

/// Maximum output size in bytes before truncation (100 KB).
pub const MAX_OUTPUT_BYTES: usize = 100 * 1024;

/// Truncate `text` to [`MAX_OUTPUT_BYTES`], appending `suffix` when shortened
/// and preferring to cut on the last newline before the limit.
///
/// The caller owns the human-readable truncation notice (`suffix`) so the
/// Office and `LibreOffice` families can keep their distinct wording while
/// sharing the boundary logic (FUNC-003: the byte budget can land inside a
/// multibyte character, so the cut snaps back to a char boundary).
#[must_use]
pub(crate) fn truncate_output_with_suffix(text: String, suffix: &str) -> String {
    if text.len() <= MAX_OUTPUT_BYTES {
        return text;
    }
    let truncated = truncate_bytes_no_ellipsis(&text, MAX_OUTPUT_BYTES);
    // Leave room for the suffix when it is appended.
    let max_body = MAX_OUTPUT_BYTES.saturating_sub(suffix.len() + 1);
    let mut boundary = truncated.rfind('\n').unwrap_or(truncated.len());
    if boundary > max_body {
        boundary = max_body;
    }
    let boundary = floor_char_boundary(&truncated, boundary);
    format!("{}{suffix}", &truncated[..boundary])
}

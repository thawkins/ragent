//! Common helpers shared by the PDF read and write tools.
//!
//! Provides the shared path-resolution helper and the output-truncation
//! routine used by [`crate::pdf_read`] and [`crate::pdf_write`].

use ragent_types::strutil::floor_char_boundary;

/// Maximum output size in bytes before truncation (100 KB).
pub const MAX_OUTPUT_BYTES: usize = 100 * 1024;

/// Re-export of the shared path-resolution helper (DUPPLAN.md Milestone B).
///
/// See [`ragent_tools_core::path_util::resolve_path`] for the canonical
/// implementation and argument documentation.
pub use ragent_tools_core::path_util::resolve_path;

/// Truncates output text if it exceeds [`MAX_OUTPUT_BYTES`].
///
/// The result is shortened to fit the byte budget (including the appended
/// notice), preferring to cut on the last newline before the limit. FUNC-003:
/// the byte budget can land inside a multibyte character, so the cut snaps back
/// to a char boundary.
///
/// # Arguments
///
/// * `text` - The text to potentially truncate.
///
/// # Returns
///
/// The original text if within limits, or a truncated version with a notice.
#[must_use]
pub fn truncate_output(text: String) -> String {
    if text.len() <= MAX_OUTPUT_BYTES {
        return text;
    }
    let suffix = format!(
        "

... [Output truncated at {}KB. Use page range selection to read specific pages.]",
        MAX_OUTPUT_BYTES / 1024
    );
    // Leave room for the suffix when it is appended.
    let max_body = MAX_OUTPUT_BYTES.saturating_sub(suffix.len() + 1);
    // Only search for the cut newline within the byte budget; scanning the whole
    // string would always find a newline after `max_body` and then clamp back,
    // producing a mid-line cut.
    let search_end = max_body.min(text.len());
    let search_end = floor_char_boundary(&text, search_end);
    let boundary = text[..search_end]
        .rfind('\n')
        .map_or(max_body, |n| n.min(max_body));
    let boundary = floor_char_boundary(&text, boundary);
    format!("{}{suffix}", &text[..boundary])
}

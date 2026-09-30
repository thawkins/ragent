//! Common helpers for Office document tools.
//!
//! Provides format detection via file extension, an [`OfficeFormat`] enum,
//! and a path resolution helper shared by all office tool modules.

use std::path::Path;

use anyhow::{Result, bail};

/// Supported Office document formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficeFormat {
    /// Microsoft Word (.docx)
    Docx,
    /// Microsoft Excel (.xlsx)
    Xlsx,
    /// Microsoft `PowerPoint` (.pptx)
    Pptx,
}

impl std::fmt::Display for OfficeFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Docx => write!(f, "docx"),
            Self::Xlsx => write!(f, "xlsx"),
            Self::Pptx => write!(f, "pptx"),
        }
    }
}

/// Detects the Office document format from a file path's extension.
///
/// # Arguments
///
/// * `path` - Path to the Office document.
///
/// # Returns
///
/// The detected [`OfficeFormat`], or an error if the extension is not
/// a supported modern OOXML format.
///
/// # Errors
///
/// Returns an error if the file has no extension or the extension is not
/// `.docx`, `.xlsx`, or `.pptx`.
pub fn detect_format(path: &Path) -> Result<OfficeFormat> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);

    match ext.as_deref() {
        Some("docx") => Ok(OfficeFormat::Docx),
        Some("xlsx") => Ok(OfficeFormat::Xlsx),
        Some("pptx") => Ok(OfficeFormat::Pptx),
        Some("doc" | "xls" | "ppt") => {
            bail!(
                "Legacy Office format '{}' is not supported. \
                 Please convert to the modern OOXML format (.docx, .xlsx, .pptx).",
                ext.unwrap_or_default()
            )
        }
        Some(ext) => bail!("Unsupported file extension: .{ext}"),
        None => bail!("File has no extension; cannot detect Office format"),
    }
}

/// Re-export of the shared path-resolution helper (DUPPLAN.md Milestone B).
///
/// See [`ragent_tools_core::path_util::resolve_path`] for the canonical
/// implementation and argument documentation.
pub use ragent_tools_core::path_util::resolve_path;

/// Maximum output size in bytes before truncation (100 KB).
pub use crate::docio::MAX_OUTPUT_BYTES;

/// Truncates output text if it exceeds [`MAX_OUTPUT_BYTES`].
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
    crate::docio::truncate_output_with_suffix(
        text,
        &format!(
            "

... [Output truncated at {}KB. Use range/sheet/slide selection to read specific sections.]",
            MAX_OUTPUT_BYTES / 1024
        ),
    )
}

#[cfg(test)]
#[path = "../tests/inline/office_common_tests.rs"]
mod tests;

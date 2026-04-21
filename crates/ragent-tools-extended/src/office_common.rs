//! Common helpers for Office document tools.
//!
//! Provides format detection via file extension, an [`OfficeFormat`] enum,
//! and a path resolution helper shared by all office tool modules.

use std::path::{Path, PathBuf};

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

/// Resolves a path string against a working directory.
///
/// If `path_str` is absolute, it is returned as-is.
/// Otherwise it is joined to `working_dir`.
///
/// # Arguments
///
/// * `working_dir` - The base directory for relative paths.
/// * `path_str` - The path string to resolve.
///
/// # Returns
///
/// The resolved absolute [`PathBuf`].
#[must_use]
pub fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}

/// Maximum output size in bytes before truncation (100 KB).
pub const MAX_OUTPUT_BYTES: usize = 100 * 1024;

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
    if text.len() <= MAX_OUTPUT_BYTES {
        text
    } else {
        // Prepare suffix message first so we can ensure final output is smaller
        let suffix = format!(
            "\n\n... [Output truncated at {}KB. Use range/sheet/slide selection to read specific sections.]",
            MAX_OUTPUT_BYTES / 1024
        );
        let truncated = &text[..MAX_OUTPUT_BYTES];
        // Prefer cutting at the last newline, but ensure we leave room for the suffix
        let max_body = MAX_OUTPUT_BYTES.saturating_sub(suffix.len() + 1);
        let mut boundary = truncated.rfind('\n').unwrap_or(MAX_OUTPUT_BYTES);
        if boundary > max_body {
            boundary = max_body;
        }
        format!("{}{}", &text[..boundary], suffix)
    }
}

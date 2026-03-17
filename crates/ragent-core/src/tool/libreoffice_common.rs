//! Common helpers for LibreOffice (OpenDocument) formats.
//!
//! Provides format detection via file extension, a [`LibreFormat`] enum,
//! and a path resolution helper shared by all libreoffice tool modules.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

/// Supported LibreOffice / OpenDocument formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibreFormat {
    /// OpenDocument Text (.odt)
    Odt,
    /// OpenDocument Spreadsheet (.ods)
    Ods,
    /// OpenDocument Presentation (.odp)
    Odp,
}

impl std::fmt::Display for LibreFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Odt => write!(f, "odt"),
            Self::Ods => write!(f, "ods"),
            Self::Odp => write!(f, "odp"),
        }
    }
}

/// Detects the LibreOffice document format from a file path's extension.
///
/// # Arguments
///
/// * `path` - Path to the document.
///
/// # Returns
///
/// The detected [`LibreFormat`], or an error if the extension is not
/// a supported OpenDocument format.
pub fn detect_format(path: &Path) -> Result<LibreFormat> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match ext.as_deref() {
        Some("odt") => Ok(LibreFormat::Odt),
        Some("ods") => Ok(LibreFormat::Ods),
        Some("odp") => Ok(LibreFormat::Odp),
        Some(ext) => bail!("Unsupported file extension: .{ext}"),
        None => bail!("File has no extension; cannot detect LibreOffice format"),
    }
}

/// Resolves a path string against a working directory.
///
/// If `path_str` is absolute, it is returned as-is.
/// Otherwise it is joined to `working_dir`.
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
pub fn truncate_output(text: String) -> String {
    if text.len() <= MAX_OUTPUT_BYTES {
        text
    } else {
        let truncated = &text[..MAX_OUTPUT_BYTES];
        let boundary = truncated.rfind('\n').unwrap_or(MAX_OUTPUT_BYTES);
        format!(
            "{}\n\n... [Output truncated at {}KB.]",
            &text[..boundary],
            MAX_OUTPUT_BYTES / 1024
        )
    }
}

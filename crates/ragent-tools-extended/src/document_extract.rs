//! Unified document-to-markdown extraction.
//!
//! Provides [`extract_file_as_markdown`], a single entry point that detects the
//! format of a file by its extension and dispatches to the existing
//! [`pdf_read`], [`office_read`], and [`libreoffice_read`] extraction routines.
//! This lets callers (e.g. the `ragent-research` `--from-file` flag) turn any
//! supported document into markdown text without duplicating format-detection
//! logic or the per-format read functions.
//!
//! Supported formats:
//! - **PDF**: `.pdf`
//! - **Microsoft Office**: `.docx`, `.xlsx`, `.pptx`
//! - **LibreOffice / OpenDocument**: `.odt`, `.ods`, `.odp`
//!
//! Legacy binary Office formats (`.doc`, `.xls`, `.ppt`) are *not* supported;
//! callers should convert them to the modern OOXML equivalents first.

use std::path::Path;

use anyhow::{Result, bail};

/// Detected document category returned by [`detect_document_format`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentFormat {
    /// PDF (`.pdf`).
    Pdf,
    /// Microsoft Word (`.docx`).
    Docx,
    /// Microsoft Excel (`.xlsx`).
    Xlsx,
    /// Microsoft PowerPoint (`.pptx`).
    Pptx,
    /// `OpenDocument` Text (`.odt`).
    Odt,
    /// `OpenDocument` Spreadsheet (`.ods`).
    Ods,
    /// `OpenDocument` Presentation (`.odp`).
    Odp,
    /// Plain text / markdown (`.md`, `.txt`, no extension). Read directly.
    Text,
}

impl DocumentFormat {
    /// Short lowercase label used in metadata and log output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Pptx => "pptx",
            Self::Odt => "odt",
            Self::Ods => "ods",
            Self::Odp => "odp",
            Self::Text => "text",
        }
    }
}

impl std::fmt::Display for DocumentFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Detect the [`DocumentFormat`] from a file path's extension.
///
/// Returns `Ok(DocumentFormat::Text)` for `.md`, `.txt`, `.markdown`, and
/// extension-less files so callers can read them as plain text. Returns an
/// error for legacy binary Office formats and any other unknown extension.
///
/// # Errors
///
/// Returns an error for legacy binary Office formats (`.doc`, `.xls`, `.ppt`)
/// and for extensions that are not recognised.
pub fn detect_document_format(path: &Path) -> Result<DocumentFormat> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);

    let format = match ext.as_deref() {
        Some("pdf") => DocumentFormat::Pdf,
        Some("docx") => DocumentFormat::Docx,
        Some("xlsx") => DocumentFormat::Xlsx,
        Some("pptx") => DocumentFormat::Pptx,
        Some("odt") => DocumentFormat::Odt,
        Some("ods") => DocumentFormat::Ods,
        Some("odp") => DocumentFormat::Odp,
        // Treat markdown / plain text / extension-less files as text.
        Some("md" | "markdown" | "txt") | None => DocumentFormat::Text,
        Some("doc" | "xls" | "ppt") => {
            bail!(
                "Legacy Office format '.{}' is not supported. Please convert \
                 to the modern OOXML/ODF format (.docx/.xlsx/.pptx or \
                 .odt/.ods/.odp).",
                ext.unwrap_or_default()
            );
        }
        Some(ext) => bail!("Unsupported file extension: .{ext}"),
    };
    Ok(format)
}

/// Result of extracting content from a document file.
#[derive(Debug, Clone)]
pub struct ExtractedDocument {
    /// Detected format of the source file.
    pub format: DocumentFormat,
    /// Extracted markdown (or plain-text) content.
    pub content: String,
}

/// Extract the content of a document file as markdown.
///
/// Detects the format from the file extension and dispatches to the existing
/// `pdf_read`, `office_read`, and `libreoffice_read` extraction routines,
/// returning a single [`ExtractedDocument`] with the markdown content and the
/// detected format. Plain-text and markdown files are read verbatim.
///
/// This runs the extraction on the current thread (the underlying readers are
/// synchronous). Callers that need async should wrap the call in
/// `tokio::task::spawn_blocking`.
///
/// # Errors
///
/// Returns an error if the format is not supported (see
/// [`detect_document_format`]) or if the underlying reader fails to open or
/// parse the file.
pub fn extract_file_as_markdown(path: &Path) -> Result<ExtractedDocument> {
    let format = detect_document_format(path)?;
    // All readers run on the calling thread; the public tool wrappers
    // already wrap these in `spawn_blocking` so we do the same here for
    // consistency when called from async contexts.
    let content = match format {
        DocumentFormat::Pdf => super::pdf_read::read_pdf(path, None, None, "text")?,
        DocumentFormat::Docx => super::office_read::read_docx(path, "markdown")?,
        DocumentFormat::Xlsx => super::office_read::read_xlsx(path, None, None, "markdown")?,
        DocumentFormat::Pptx => super::office_read::read_pptx(path, None, "markdown")?,
        DocumentFormat::Odt => super::libreoffice_read::read_odt(path, "markdown")?,
        DocumentFormat::Ods => super::libreoffice_read::read_ods(path, None, None, "markdown")?,
        DocumentFormat::Odp => super::libreoffice_read::read_odp(path, None, "markdown")?,
        DocumentFormat::Text => std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read text file {}: {e}", path.display()))?,
    };
    Ok(ExtractedDocument { format, content })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_pdf() {
        assert_eq!(
            detect_document_format(Path::new("report.pdf")).unwrap(),
            DocumentFormat::Pdf
        );
    }

    #[test]
    fn detect_office_formats() {
        assert_eq!(
            detect_document_format(Path::new("doc.docx")).unwrap(),
            DocumentFormat::Docx
        );
        assert_eq!(
            detect_document_format(Path::new("sheet.xlsx")).unwrap(),
            DocumentFormat::Xlsx
        );
        assert_eq!(
            detect_document_format(Path::new("slides.pptx")).unwrap(),
            DocumentFormat::Pptx
        );
    }

    #[test]
    fn detect_libreoffice_formats() {
        assert_eq!(
            detect_document_format(Path::new("doc.odt")).unwrap(),
            DocumentFormat::Odt
        );
        assert_eq!(
            detect_document_format(Path::new("sheet.ods")).unwrap(),
            DocumentFormat::Ods
        );
        assert_eq!(
            detect_document_format(Path::new("slides.odp")).unwrap(),
            DocumentFormat::Odp
        );
    }

    #[test]
    fn detect_text_and_markdown() {
        assert_eq!(
            detect_document_format(Path::new("README.md")).unwrap(),
            DocumentFormat::Text
        );
        assert_eq!(
            detect_document_format(Path::new("notes.txt")).unwrap(),
            DocumentFormat::Text
        );
        assert_eq!(
            detect_document_format(Path::new("noext")).unwrap(),
            DocumentFormat::Text
        );
    }

    #[test]
    fn detect_legacy_office_returns_error() {
        assert!(detect_document_format(Path::new("old.doc")).is_err());
        assert!(detect_document_format(Path::new("old.xls")).is_err());
        assert!(detect_document_format(Path::new("old.ppt")).is_err());
    }

    #[test]
    fn detect_unknown_extension_returns_error() {
        assert!(detect_document_format(Path::new("file.xyz")).is_err());
    }

    #[test]
    fn extract_text_file_reads_verbatim() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "# Hello\n\nWorld").unwrap();
        let extracted = extract_file_as_markdown(tmp.path()).unwrap();
        assert_eq!(extracted.format, DocumentFormat::Text);
        assert_eq!(extracted.content, "# Hello\n\nWorld");
    }

    #[test]
    fn extract_markdown_file_reads_verbatim() {
        let tmp = tempfile::NamedTempFile::with_suffix(".md").unwrap();
        std::fs::write(tmp.path(), "# Title\n\nbody").unwrap();
        let extracted = extract_file_as_markdown(tmp.path()).unwrap();
        assert_eq!(extracted.format, DocumentFormat::Text);
        assert_eq!(extracted.content, "# Title\n\nbody");
    }

    #[test]
    fn format_as_str_and_display() {
        assert_eq!(DocumentFormat::Pdf.as_str(), "pdf");
        assert_eq!(DocumentFormat::Docx.as_str(), "docx");
        assert_eq!(format!("{}", DocumentFormat::Odp), "odp");
    }
}

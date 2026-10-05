//! Inline tests for `document_extract.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn detect_pdf() {
    assert_eq!(
        detect_document_format(Path::new("report.pdf")).unwrap(),
        DocumentFormat::Pdf
    );
}

#[test]
fn detect_office_formats_unsupported() {
    assert!(detect_document_format(Path::new("doc.docx")).is_err());
    assert!(detect_document_format(Path::new("sheet.xlsx")).is_err());
    assert!(detect_document_format(Path::new("slides.pptx")).is_err());
}

#[test]
fn detect_libreoffice_formats_unsupported() {
    assert!(detect_document_format(Path::new("doc.odt")).is_err());
    assert!(detect_document_format(Path::new("sheet.ods")).is_err());
    assert!(detect_document_format(Path::new("slides.odp")).is_err());
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
    assert_eq!(DocumentFormat::Text.as_str(), "text");
    assert_eq!(format!("{}", DocumentFormat::Text), "text");
}

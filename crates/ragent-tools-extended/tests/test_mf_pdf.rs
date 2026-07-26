//! Integration tests for `masterfetch::pdf` — PDF text and title extraction.
//!
//! These tests generate minimal PDFs with `printpdf` and exercise the
//! `extract_pdf_text` and `extract_pdf_title` helpers used by `mf_fetch`.

use printpdf::{
    BuiltinFont, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, PdfWarnMsg, Point,
    Pt, TextItem,
};
use ragent_tools_extended::masterfetch::pdf::{extract_pdf_text, extract_pdf_title};

/// Create a tiny one-page PDF containing the given text and optional title.
fn make_pdf_bytes(text: &str, title: Option<&str>) -> Vec<u8> {
    let mut doc = PdfDocument::new(title.unwrap_or(""));
    let font = PdfFontHandle::Builtin(BuiltinFont::Helvetica);

    let ops = vec![
        Op::StartTextSection,
        Op::SetFont {
            font: font.clone(),
            size: Pt(12.0),
        },
        Op::SetTextCursor {
            pos: Point::new(Mm(25.0), Mm(280.0)),
        },
        Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        },
        Op::EndTextSection,
    ];

    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    doc.pages = vec![page];

    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    doc.save(&PdfSaveOptions::default(), &mut warnings)
}

#[test]
fn test_extract_pdf_text_finds_embedded_text() {
    let bytes = make_pdf_bytes("Hello from the PDF body.", None);
    let text = extract_pdf_text(&bytes).expect("extracting text from generated PDF");
    assert!(
        text.contains("Hello from the PDF body."),
        "extracted text should contain the embedded sentence: {text}"
    );
}

#[test]
fn test_extract_pdf_title_reads_info_dictionary() {
    let bytes = make_pdf_bytes("Body text.", Some("My PDF Title"));
    let title = extract_pdf_title(&bytes).expect("extracting title from generated PDF");
    assert_eq!(title, "My PDF Title");
}

#[test]
fn test_extract_pdf_title_missing_when_no_title() {
    let bytes = make_pdf_bytes("No title here.", None);
    assert!(extract_pdf_title(&bytes).is_none());
}

#[test]
fn test_extract_pdf_text_errors_for_non_pdf_bytes() {
    let result = extract_pdf_text(b"this is not a pdf");
    assert!(result.is_err());
}

//! Regression test: a PDF whose embedded Type1C font declares CFF **Expert**
//! encoding must not panic `pdf_extract`.
//!
//! `cff-parser` 0.1.0's `Encoding::get_table()` does `EncodingKind::Expert =>
//! panic!()` — the vendored `pdf-extract` used to call it unconditionally when
//! building the CFF unicode map. The panic was caught by
//! `extract_pdf_text`'s `catch_unwind`, but the global panic hook still fired
//! (writing a panic log and tearing down the TUI terminal), which is what the
//! user saw as "ragent panicked during /research create".
//!
//! These tests build a minimal one-page PDF whose embedded font uses
//! `EncodingKind::Expert`, then assert that text extraction finishes without
//! panicking.

use lopdf::dictionary;

/// Build a minimal CFF (Compact Font Format / Type1C) font table whose
/// Encoding operand points to a subtable with format byte `1`
/// (= `EncodingKind::Expert` in cff-parser).
///
/// Layout (CFF spec):
/// - Header: major=1, minor=0, hdrSize=4, offSize=1
/// - Name INDEX: one name "A"
/// - Top DICT INDEX: `Encoding` offset (DICT op 15) and `CharStrings`
///   offset (DICT op 17)
/// - String INDEX: empty
/// - Global Subr INDEX: empty
/// - Encoding subtable (format 1 = Expert)
/// - CharStrings INDEX: one zero-length charstring
#[cfg(test)]
fn build_expert_cff() -> Vec<u8> {
    let mut d = Vec::new();
    // Header (4 bytes).
    d.extend_from_slice(&[1, 0, 4, 1]);
    // Name INDEX: count=1, offSize=2, offsets [1,2], name "A" (7 bytes).
    d.extend_from_slice(&[0x00, 0x01, 0x02, 0x00, 0x01, 0x00, 0x02, b'A']);

    // The Top DICT INDEX wraps its payload as: count(2), offSize(1),
    // offsets(count+1, 1 byte each), data. With a 4-byte payload the header
    // is 2+1+2 = 5 bytes, so the Top DICT INDEX lands at file offsets
    // [11..21): header at 11..16, operands at 16..21.
    //
    // After the top dict we append: String INDEX (2 bytes) at 21..23,
    // Global Subr INDEX (2 bytes) at 23..25, and the CharStrings INDEX at
    // 25. All operands are < 247 so the CFF number encoding is value+139 in
    // a single byte.
    //
    // Encoding is expressed with operand `1` — cff-parser treats operand 1
    // as the predefined *Expert* encoding (per the CFF spec, encoding
    // operands 0 and 1 are predefined IDs, not file offsets). That operand
    // makes `Encoding::get_table()` panic in cff-parser 0.1.0.
    let encoding_operand = 1u8 + 139; // Expert encoding ID
    let charstrings_offset = 25u8 + 139; // 164
    let top_dict_payload: [u8; 4] = [encoding_operand, 15u8, charstrings_offset, 17u8];
    d.extend_from_slice(&[0x00, 0x01, 0x01, 0x01, 0x05]); // count=1, offSize=1, offsets [1,5]
    d.extend_from_slice(&top_dict_payload);
    debug_assert_eq!(d.len(), 21);
    // String INDEX: empty.
    d.extend_from_slice(&[0x00, 0x00]);
    // Global Subr INDEX: empty.
    d.extend_from_slice(&[0x00, 0x00]);
    debug_assert_eq!(d.len(), 25);
    // CharStrings INDEX: count=1, offSize=1, offsets [1,1] (empty data).
    d.extend_from_slice(&[0x00, 0x01, 0x01, 0x01, 0x01]);
    d
}

/// Build a one-page PDF embedding the crafted CFF font.
///
/// The font is declared `Subtype /Type1` with a `FontDescriptor` whose
/// `FontFile3` stream has `Subtype /Type1C` — the path in `pdf-extract`'s
/// `make_font` that historically reached `Encoding::get_table()`.
#[cfg(test)]
fn build_pdf_with_expert_cff() -> Vec<u8> {
    use lopdf::{Document, Object, Stream};

    let mut doc = Document::with_version("1.5");

    let cff = build_expert_cff();
    let font_file3 = Stream::new(dictionary! {"Subtype" => "Type1C"}, cff);
    let ff3_id = doc.add_object(Object::Stream(font_file3));

    let descriptor = dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => "ExpertFont",
        "Flags" => 4,
        "FontBBox" => vec![0.into(), 0.into(), 1000.into(), 1000.into()],
        "ItalicAngle" => 0,
        "Ascent" => 800,
        "Descent" => -200,
        "CapHeight" => 700,
        "StemV" => 80,
        "FontFile3" => Object::Reference(ff3_id),
    };
    let desc_id = doc.add_object(Object::Dictionary(descriptor));

    let font = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "ExpertFont",
        "FirstChar" => 0,
        "LastChar" => 255,
        "Widths" => vec![600.into(); 256],
        "FontDescriptor" => Object::Reference(desc_id),
    };
    let font_id = doc.add_object(Object::Dictionary(font));

    let resources = dictionary! {
        "Font" => dictionary! { "F1" => Object::Reference(font_id) },
    };
    let res_id = doc.add_object(Object::Dictionary(resources));

    let contents = Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 72 720 Td (hi) Tj ET".to_vec(),
    );
    let contents_id = doc.add_object(Object::Stream(contents));

    let page = dictionary! {
        "Type" => "Page",
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => Object::Reference(res_id),
        "Contents" => Object::Reference(contents_id),
    };
    let page_id = doc.add_object(Object::Dictionary(page));

    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::Reference(page_id)],
        "Count" => 1,
    };
    let pages_id = doc.add_object(Object::Dictionary(pages));
    // Wire the page back up to the pages tree.
    if let Ok(Object::Dictionary(page)) = doc.get_object_mut(page_id) {
        page.set("Parent", Object::Reference(pages_id));
    }

    let catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    };
    let catalog_id = doc.add_object(Object::Dictionary(catalog));
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut out = Vec::new();
    doc.save_to(&mut out).expect("lopdf save");
    out
}

#[test]
fn test_expert_cff_encoding_does_not_panic() {
    let pdf = build_pdf_with_expert_cff();
    // Before the vendored patch this call panicked with "explicit panic"
    // inside cff_parser::encoding::Encoding::get_table on
    // EncodingKind::Expert. It must now return normally (either Ok or a
    // graceful extraction error).
    let result = pdf_extract::extract_text_from_mem(&pdf);
    let _ = result; // reaching this line without unwinding is the assertion
}

#[test]
fn test_expert_cff_through_extract_pdf_text() {
    use ragent_tools_extended::masterfetch::pdf::extract_pdf_text;
    let pdf = build_pdf_with_expert_cff();
    // The dedicated-OS-thread isolation path: must not abort the process,
    // and must not write a panic log via the global hook. Just assert it
    // returns (Ok or Err — either is fine as long as we don't crash).
    let _ = extract_pdf_text(&pdf);
}

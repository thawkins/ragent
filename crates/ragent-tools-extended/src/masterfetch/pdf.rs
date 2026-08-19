//! PDF text extraction helpers for `mf_fetch`.
//!
//! Provides a thin wrapper around the workspace-patched `pdf-extract` crate so
//! that `mf_fetch` can turn a fetched PDF document into readable text instead
//! of returning raw binary bytes.
//!
//! The `pdf-extract` crate is vendored under `vendor/pdf-extract` and patched
//! to fall back to `PDFDocEncoding` when a character is missing from the
//! Unicode map and no explicit encoding is present. The upstream crate panics
//! on that path; the patch avoids the panic entirely while preserving the
//! original `catch_unwind` isolation as a safety net.
//!
//! See `vendor/pdf-extract/src/lib.rs` lines 832 and 882 for the patched
//! fallback logic.

use anyhow::{Context, Result};

/// Extract plain text from a PDF byte slice.
///
/// Delegates to the patched `pdf_extract::extract_text_from_mem`. The output
/// is a single string with page breaks approximated by newlines where the
/// extractor provides them.
///
/// # Panic isolation
///
/// Although the patched dependency avoids the known `missing unicode map and
/// encoding` panic, the call is still wrapped in [`std::panic::catch_unwind`]
/// so any unexpected extraction panic is converted into an `Err` rather than
/// aborting the calling task or process.
///
/// # Errors
///
/// Returns an error if `pdf_extract` cannot parse the bytes, if the
/// extracted text cannot be converted to a UTF-8 string, or if extraction
/// panicked and was caught.
///
/// # Examples
///
/// ```no_run
/// use ragent_tools_extended::masterfetch::pdf::extract_pdf_text;
///
/// # fn demo(bytes: &[u8]) -> anyhow::Result<()> {
/// let text = extract_pdf_text(bytes)?;
/// assert!(!text.is_empty());
/// # Ok(()) }
/// ```
pub fn extract_pdf_text(bytes: &[u8]) -> Result<String> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pdf_extract::extract_text_from_mem(bytes)
    }));

    match result {
        Ok(Ok(text)) => Ok(text),
        Ok(Err(e)) => Err(e).with_context(|| "Failed to extract text from PDF"),
        Err(_) => anyhow::bail!(
            "PDF text extraction panicked (likely due to an unsupported or malformed font stream)"
        ),
    }
}

/// Extract document metadata title from a PDF byte slice.
///
/// Uses `lopdf` to read the document's `/Info` dictionary. Returns `None` when
/// the PDF has no title entry or when parsing fails.
pub fn extract_pdf_title(bytes: &[u8]) -> Option<String> {
    let doc = lopdf::Document::load_mem(bytes).ok()?;
    doc.trailer
        .get(b"Info")
        .ok()
        .and_then(|info| doc.dereference(info).ok())
        .and_then(|(_, obj)| match obj {
            lopdf::Object::Dictionary(dict) => dict.get(b"Title").ok().cloned(),
            _ => None,
        })
        .and_then(|obj| match obj {
            lopdf::Object::String(bytes, _) => Some(decode_pdf_text_string(&bytes)),
            _ => None,
        })
        .filter(|t| !t.is_empty())
}

/// Decode a PDF text string according to PDFDocEncoding / UTF-16BE rules.
///
/// PDF text strings are either PDFDocEncoding or UTF-16BE with an optional
/// byte-order mark (`0xFE 0xFF`). Hex/literal strings stored by modern
/// generators such as `printpdf` commonly use UTF-16BE.
fn decode_pdf_text_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        // UTF-16BE with BOM.
        return String::from_utf16(
            bytes[2..]
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .unwrap_or_default()
        .trim()
        .to_string();
    }

    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        // UTF-16LE with BOM.
        return String::from_utf16(
            bytes[2..]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .unwrap_or_default()
        .trim()
        .to_string();
    }

    String::from_utf8_lossy(bytes).trim().to_string()
}

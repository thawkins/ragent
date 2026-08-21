//! PDF text extraction helpers for `mf_fetch`.
//!
//! Provides a thin wrapper around the workspace-patched `pdf-extract` crate so
//! that `mf_fetch` can turn a fetched PDF document into readable text instead
//! of returning raw binary bytes.
//!
//! The `pdf-extract` crate is vendored under `vendor/pdf-extract` and patched
//! to fall back to `PDFDocEncoding` when a character is missing from the
//! Unicode map and no explicit encoding is present, and to skip CFF
//! `EncodingKind::Expert` (which makes `cff-parser` 0.1.0 panic). Despite
//! those source-level fixes, `pdf-extract` and its transitive dependencies
//! (`cff-parser`, `lopdf`) still contain other unguarded `panic!()` / `unwrap()`
//! call sites. To ensure no extraction panic can ever take down the process,
//! `extract_pdf_text` runs the extraction on a **dedicated OS thread** (not
//! merely `catch_unwind` inside a `spawn_blocking` task), matching the
//! isolation strategy used for `html2text` rendering.
//!
//! See `vendor/pdf-extract/src/lib.rs` lines 832 and 882 for the patched
//! fallback logic, and lines 421–448 for the CFF Expert encoding guard.

use anyhow::{Context, Result};

/// Extract plain text from a PDF byte slice.
///
/// Delegates to the patched `pdf_extract::extract_text_from_mem`. The output
/// is a single string with page breaks approximated by newlines where the
/// extractor provides them.
///
/// # Panic isolation
///
/// The extraction runs on a **dedicated OS thread** (spawned via
/// [`std::thread::Builder`]) with [`ragent_types::panic_guard::run`] inside.
/// This is the same strategy used by `run_html2text_isolated`: a panic inside
/// a `tokio::task::spawn_blocking` closure can still take down the entire
/// process even when wrapped in `catch_unwind`, because the Tokio runtime's
/// own panic handling and the global panic hook fire before the unwind reaches
/// the `catch_unwind` frame. By running on a separate join-able OS thread, any
/// panic kills only that disposable thread and [`std::thread::JoinHandle::join`]
/// returns `Err`, which we convert to a graceful error.
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
    // Move the bytes into the spawned thread so no lifetime / UnwindSafe
    // issues arise on the caller side.
    let bytes_owned = bytes.to_vec();

    let thread_result = std::thread::Builder::new()
        .name("mf-pdf-extract".to_string())
        .spawn(move || {
            ragent_types::panic_guard::run(|| pdf_extract::extract_text_from_mem(&bytes_owned))
        })
        .map_err(|e| anyhow::anyhow!("failed to spawn PDF extraction thread: {e}"))?
        .join();

    match thread_result {
        // Thread joined successfully; `run` returned Ok(Ok(text)).
        Ok(Ok(Ok(text))) => Ok(text),
        // Thread joined successfully; `run` returned Ok(Err(extract_error)).
        Ok(Ok(Err(e))) => Err(e).with_context(|| "Failed to extract text from PDF"),
        // Thread joined successfully; `run` returned Err (panic was caught).
        Ok(Err(_)) => anyhow::bail!(
            "PDF text extraction panicked (likely due to an unsupported or malformed font stream)"
        ),
        // Thread itself panicked (double-panic or abort-on-panic).
        Err(_) => anyhow::bail!("PDF text extraction thread panicked unexpectedly"),
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
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes(*c))
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
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .unwrap_or_default()
        .trim()
        .to_string();
    }

    String::from_utf8_lossy(bytes).trim().to_string()
}

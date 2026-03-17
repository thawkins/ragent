//! Common helpers for LibreOffice (OpenDocument) formats.
//!
//! Provides format detection via file extension, a [`LibreFormat`] enum,
//! a path resolution helper, and XML extraction utilities shared by all
//! libreoffice tool modules.
//!
//! OpenDocument files are ZIP archives containing XML files. The primary
//! content lives in `content.xml`; document metadata is in `meta.xml`.
//! `quick-xml` is used for robust XML parsing.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use quick_xml::events::Event;
use quick_xml::reader::Reader;

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
pub fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() { p } else { working_dir.join(p) }
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

/// Read the raw content of a named entry from an ODF ZIP archive as UTF-8.
pub fn read_zip_entry(path: &Path, entry_name: &str) -> Result<String> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("Cannot open {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file)
        .with_context(|| format!("Not a valid ZIP/ODF archive: {}", path.display()))?;
    let mut entry = zip.by_name(entry_name)
        .with_context(|| format!("Entry '{}' not found in {}", entry_name, path.display()))?;
    let mut buf = String::new();
    entry.read_to_string(&mut buf)
        .with_context(|| format!("Failed to read '{}' from {}", entry_name, path.display()))?;
    Ok(buf)
}

/// Decode a quick-xml text event to a `&str`, using the embedded decoder.
/// ODF files are always UTF-8, so this is straightforward.
fn decode_text<'a>(e: &'a quick_xml::events::BytesText<'a>) -> &'a str {
    // BytesText is always valid UTF-8 for UTF-8 documents; fall back to empty on error.
    std::str::from_utf8(e.as_ref()).unwrap_or("")
}

/// Decode a quick-xml CDATA event to a `&str`.
fn decode_cdata<'a>(e: &'a quick_xml::events::BytesCData<'a>) -> &'a str {
    std::str::from_utf8(e.as_ref()).unwrap_or("")
}

/// Decode a quick-xml attribute value to a `String`.
fn decode_attr_value(a: &quick_xml::events::attributes::Attribute<'_>) -> String {
    std::str::from_utf8(&a.value).unwrap_or("").to_string()
}

/// Extract all plain text from an XML string using `quick-xml`.
///
/// Collects all character data between tags, inserting newlines at paragraph
/// / heading / list-item boundaries so the output is readable.
pub fn xml_to_text(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut out = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(e)) => {
                let s = decode_text(&e);
                let s = s.trim();
                if !s.is_empty() {
                    if !out.is_empty() && !out.ends_with('\n') && !out.ends_with(' ') {
                        out.push(' ');
                    }
                    out.push_str(s);
                }
            }
            Ok(Event::CData(e)) => {
                let s = decode_cdata(&e);
                let s = s.trim();
                if !s.is_empty() {
                    if !out.is_empty() && !out.ends_with('\n') && !out.ends_with(' ') {
                        out.push(' ');
                    }
                    out.push_str(s);
                }
            }
            Ok(Event::Start(e)) => {
                // Insert newline at paragraph / heading / row boundaries.
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if matches!(name, "p" | "h" | "list-item" | "table-row" | "page") {
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out.trim().to_string()
}

/// Read a named metadata field from `meta.xml` inside the ODF archive.
/// Returns `None` if the field is absent or unreadable.
pub fn read_meta_field(path: &Path, field_local_name: &str) -> Option<String> {
    let xml = read_zip_entry(path, "meta.xml").ok()?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut inside = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local = e.local_name();
                inside = std::str::from_utf8(local.as_ref()).unwrap_or("") == field_local_name;
            }
            Ok(Event::Text(e)) if inside => {
                let s = decode_text(&e).trim().to_string();
                return Some(s);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => { inside = false; }
        }
        buf.clear();
    }
    None
}

/// Get an attribute value by local name from a BytesStart element.
pub fn attr_value(e: &quick_xml::events::BytesStart<'_>, local_name: &str) -> Option<String> {
    e.attributes().flatten().find(|a| {
        std::str::from_utf8(a.key.local_name().as_ref()).unwrap_or("") == local_name
    }).map(|a| decode_attr_value(&a))
}

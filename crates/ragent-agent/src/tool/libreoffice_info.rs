//! `LibreOffice` document info/metadata tool.
//!
//! Provides [`LibreInfoTool`], which returns metadata and structural statistics
//! about `OpenDocument` files without reading all content.
//!
//! - **ODS**: uses `calamine` to enumerate sheets and row/column counts.
//! - **ODT**: parses `meta.xml` for title/author/date; counts paragraphs in `content.xml`.
//! - **ODP**: counts slides in `content.xml`; parses `meta.xml` for metadata.

use anyhow::{Context, Result};
use calamine::{Reader as CalaReader, Sheets, open_workbook_auto};
use serde_json::{Value, json};
use std::path::Path;

use super::libreoffice_common::{
    LibreFormat, detect_format, read_meta_field, read_zip_entry, resolve_path,
};
use super::{Tool, ToolContext, ToolOutput};

/// Returns metadata and structural information about ODF documents.
pub struct LibreInfoTool;

#[async_trait::async_trait]
impl Tool for LibreInfoTool {
    fn name(&self) -> &'static str {
        "libre_info"
    }

    /// Returns the tool description.
    fn description(&self) -> &'static str {
        "Get metadata and structural info about an OpenDocument file: sheet list and dimensions \
         for ODS, word/paragraph counts for ODT, slide count for ODP. Returns JSON."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the OpenDocument file"
                }
            },
            "required": ["path"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    /// Executes the `LibreOffice` info query.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The required `path` parameter is missing
    /// - The file format cannot be detected from the extension
    /// - The background task panics or fails to read the document
    /// - The document cannot be opened or parsed (e.g., corrupt ZIP, invalid ODF structure)
    /// - JSON serialization fails
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;
        let path = resolve_path(&ctx.working_dir, path_str);
        let libre_format = detect_format(&path)?;
        let path_d = path_str.to_string();

        let info = tokio::task::spawn_blocking(move || -> Result<Value> {
            match libre_format {
                LibreFormat::Odt => info_odt(&path),
                LibreFormat::Ods => info_ods(&path),
                LibreFormat::Odp => info_odp(&path),
            }
        })
        .await
        .context("Background task panicked while reading info")??;

        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&info)?,
            metadata: Some(json!({ "path": path_d, "format": libre_format.to_string() })),
        })
    }
}

// ── ODS ──────────────────────────────────────────────────────────────────────

/// Extracts ODS metadata and structure.
///
/// # Errors
///
/// Returns an error if:
/// - The file cannot be opened by calamine
/// - The workbook format is invalid
fn info_ods(path: &Path) -> Result<Value> {
    let mut wb: Sheets<_> = open_workbook_auto(path)
        .with_context(|| format!("calamine failed to open {}", path.display()))?;

    let sheet_names = wb.sheet_names();
    let mut sheets_info = Vec::new();
    for name in &sheet_names {
        if let Ok(range) = wb.worksheet_range(name) {
            let (rows, cols) = range.get_size();
            sheets_info.push(json!({
                "name": name,
                "rows": rows,
                "columns": cols,
            }));
        }
    }

    let title = read_meta_field(path, "title").unwrap_or_default();
    let creator = read_meta_field(path, "creator").unwrap_or_default();
    let created = read_meta_field(path, "creation-date").unwrap_or_default();

    Ok(json!({
        "format": "ods",
        "path": path.display().to_string(),
        "title": title,
        "creator": creator,
        "created": created,
        "sheet_count": sheet_names.len(),
        "sheets": sheets_info,
    }))
}

// ── ODT ──────────────────────────────────────────────────────────────────────

fn info_odt(path: &Path) -> Result<Value> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let title = read_meta_field(path, "title").unwrap_or_default();
    let creator = read_meta_field(path, "creator").unwrap_or_default();
    let created = read_meta_field(path, "creation-date").unwrap_or_default();

    let xml = read_zip_entry(path, "content.xml")?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);

    let mut para_count = 0usize;
    let mut word_count = 0usize;
    let mut char_count = 0usize;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = e.local_name();
                if std::str::from_utf8(local.as_ref()).unwrap_or("") == "p" {
                    para_count += 1;
                }
            }
            Ok(Event::Text(e)) => {
                // Use raw bytes as UTF-8 (ODF is always UTF-8).
                if let Ok(text) = std::str::from_utf8(e.as_ref()) {
                    char_count += text.len();
                    word_count += text.split_whitespace().count();
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(json!({
        "format": "odt",
        "path": path.display().to_string(),
        "title": title,
        "creator": creator,
        "created": created,
        "paragraphs": para_count,
        "words": word_count,
        "characters": char_count,
    }))
}

// ── ODP ──────────────────────────────────────────────────────────────────────

fn info_odp(path: &Path) -> Result<Value> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let title = read_meta_field(path, "title").unwrap_or_default();
    let creator = read_meta_field(path, "creator").unwrap_or_default();
    let created = read_meta_field(path, "creation-date").unwrap_or_default();

    let xml = read_zip_entry(path, "content.xml")?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);

    let mut slide_count = 0usize;
    let mut slide_names: Vec<String> = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                let local = e.local_name();
                if std::str::from_utf8(local.as_ref()).unwrap_or("") == "page" {
                    slide_count += 1;
                    let name = e
                        .attributes()
                        .flatten()
                        .find(|a| {
                            std::str::from_utf8(a.key.local_name().as_ref()).unwrap_or("") == "name"
                        })
                        .map_or_else(
                            || format!("Slide {slide_count}"),
                            |a| std::str::from_utf8(&a.value).unwrap_or("").to_string(),
                        );
                    slide_names.push(name);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(json!({
        "format": "odp",
        "path": path.display().to_string(),
        "title": title,
        "creator": creator,
        "created": created,
        "slide_count": slide_count,
        "slides": slide_names,
    }))
}

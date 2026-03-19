//! LibreOffice document writing tool.
//!
//! Provides [`LibreWriteTool`], which creates or overwrites OpenDocument files.
//!
//! - **ODS**: written using `spreadsheet-ods`, which provides a proper in-memory
//!   workbook model for Calc files.
//! - **ODT / ODP**: generated as valid ODF ZIP archives using `zip` + `quick-xml`
//!   for structured XML output.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::Path;

use super::libreoffice_common::{LibreFormat, detect_format, resolve_path};
use super::{Tool, ToolContext, ToolOutput};

/// Creates or overwrites ODT, ODS, or ODP files.
pub struct LibreWriteTool;

#[async_trait::async_trait]
impl Tool for LibreWriteTool {
    fn name(&self) -> &str {
        "libre_write"
    }

    /// Returns the tool description.
    fn description(&self) -> &str {
        "Write content to OpenDocument files: Writer (.odt), Calc (.ods), Impress (.odp). \
         ODS uses spreadsheet-ods for full workbook support; ODT/ODP use XML generation."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path for the output file (created or overwritten)"
                },
                "content": {
                    "type": "string",
                    "description": "ODT/ODP: plain text content to write"
                },
                "rows": {
                    "type": "array",
                    "items": { "type": "array", "items": { "type": "string" } },
                    "description": "ODS: 2D array of cell values, first row treated as header"
                },
                "sheet_name": {
                    "type": "string",
                    "description": "ODS: name for the sheet (default: 'Sheet1')"
                },
                "title": {
                    "type": "string",
                    "description": "Document title for metadata"
                },
                "author": {
                    "type": "string",
                    "description": "Document author for metadata"
                }
            },
            "required": ["path"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:write"
    }

    /// Executes the LibreOffice write operation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The required `path` parameter is missing
    /// - The file format cannot be detected from the extension
    /// - The background task panics or fails to write the document
    /// - For ODS: the rows parameter is malformed or the spreadsheet cannot be created
    /// - The ZIP archive cannot be created or written
    /// - File I/O operations fail
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;
        let path = resolve_path(&ctx.working_dir, path_str);
        let libre_format = detect_format(&path)?;

        // Extract parameters before moving into spawn_blocking.
        let content = input["content"].as_str().unwrap_or("").to_string();
        let rows: Vec<Vec<String>> = input["rows"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|row| {
                        row.as_array()
                            .map(|cells| {
                                cells
                                    .iter()
                                    .map(|c| c.as_str().unwrap_or("").to_string())
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let sheet_name = input["sheet_name"].as_str().unwrap_or("Sheet1").to_string();
        let title = input["title"].as_str().unwrap_or("").to_string();
        let author = input["author"].as_str().unwrap_or("").to_string();
        let path_d = path_str.to_string();

        tokio::task::spawn_blocking(move || -> Result<()> {
            match libre_format {
                LibreFormat::Odt => write_odt(&path, &content, &title, &author),
                LibreFormat::Ods => write_ods(&path, &rows, &sheet_name, &title, &author),
                LibreFormat::Odp => write_odp(&path, &content, &title, &author),
            }
        })
        .await
        .context("Background task panicked while writing document")??;

        Ok(ToolOutput {
            content: format!("Successfully wrote {} file: {}", libre_format, path_d),
            metadata: Some(json!({ "path": path_d, "format": libre_format.to_string() })),
        })
    }
}

// ── ODT ──────────────────────────────────────────────────────────────────────

fn write_odt(path: &Path, text: &str, title: &str, author: &str) -> Result<()> {
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    let file =
        std::fs::File::create(path).with_context(|| format!("Cannot create {}", path.display()))?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // mimetype must be first and uncompressed.
    let mime_opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("mimetype", mime_opts)?;
    zip.write_all(b"application/vnd.oasis.opendocument.text")?;

    zip.start_file("META-INF/manifest.xml", opts)?;
    zip.write_all(odt_manifest().as_bytes())?;

    zip.start_file("meta.xml", opts)?;
    zip.write_all(odf_meta(title, author).as_bytes())?;

    zip.start_file("content.xml", opts)?;
    zip.write_all(odt_content(text).as_bytes())?;

    zip.start_file("styles.xml", opts)?;
    zip.write_all(odt_styles().as_bytes())?;

    zip.finish()?;
    Ok(())
}

fn odt_manifest() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0">
  <manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.text"/>
  <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
  <manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>
  <manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/>
</manifest:manifest>"#.to_string()
}

fn odf_meta(title: &str, author: &str) -> String {
    let title = xml_escape(title);
    let author = xml_escape(author);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0"
  xmlns:dc="http://purl.org/dc/elements/1.1/">
  <office:meta>
    <dc:title>{title}</dc:title>
    <dc:creator>{author}</dc:creator>
    <meta:creation-date>{}</meta:creation-date>
  </office:meta>
</office:document-meta>"#,
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
    )
}

fn odt_content(text: &str) -> String {
    let paras: String = text
        .lines()
        .map(|line| format!("    <text:p>{}</text:p>\n", xml_escape(line)))
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:body>
    <office:text>
{paras}    </office:text>
  </office:body>
</office:document-content>"#
    )
}

fn odt_styles() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0">
  <office:styles/>
</office:document-styles>"#
}

// ── ODS ──────────────────────────────────────────────────────────────────────

fn write_ods(
    path: &Path,
    rows: &[Vec<String>],
    sheet_name: &str,
    _title: &str,
    _author: &str,
) -> Result<()> {
    use spreadsheet_ods::{Sheet, WorkBook};

    let mut wb = WorkBook::new_empty();
    let mut sheet = Sheet::new(sheet_name);

    for (ri, row) in rows.iter().enumerate() {
        for (ci, cell) in row.iter().enumerate() {
            sheet.set_value(ri as u32, ci as u32, cell.as_str());
        }
    }

    wb.push_sheet(sheet);
    spreadsheet_ods::write_ods(&mut wb, path)
        .with_context(|| format!("spreadsheet-ods failed to write {}", path.display()))?;
    Ok(())
}

// ── ODP ──────────────────────────────────────────────────────────────────────

fn write_odp(path: &Path, text: &str, title: &str, author: &str) -> Result<()> {
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    let file =
        std::fs::File::create(path).with_context(|| format!("Cannot create {}", path.display()))?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mime_opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("mimetype", mime_opts)?;
    zip.write_all(b"application/vnd.oasis.opendocument.presentation")?;

    zip.start_file("META-INF/manifest.xml", opts)?;
    zip.write_all(odp_manifest().as_bytes())?;

    zip.start_file("meta.xml", opts)?;
    zip.write_all(odf_meta(title, author).as_bytes())?;

    zip.start_file("content.xml", opts)?;
    zip.write_all(odp_content(text).as_bytes())?;

    zip.start_file("styles.xml", opts)?;
    zip.write_all(odp_styles().as_bytes())?;

    zip.finish()?;
    Ok(())
}

fn odp_manifest() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0">
  <manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.presentation"/>
  <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
  <manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>
  <manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/>
</manifest:manifest>"#.to_string()
}

fn odp_content(text: &str) -> String {
    // Split into slides on blank lines.
    let slides: Vec<Vec<&str>> = text
        .split("\n\n")
        .map(|block| block.lines().collect())
        .collect();

    let mut slide_xml = String::new();
    for (i, lines) in slides.iter().enumerate() {
        let paras: String = lines
            .iter()
            .map(|l| {
                format!(
                    "        <draw:text-box><text:p>{}</text:p></draw:text-box>\n",
                    xml_escape(l)
                )
            })
            .collect();
        slide_xml.push_str(&format!(
            r#"    <draw:page draw:name="Slide {n}" draw:master-page-name="Default">
{paras}    </draw:page>
"#,
            n = i + 1,
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0">
  <office:body>
    <office:presentation>
{slide_xml}    </office:presentation>
  </office:body>
</office:document-content>"#
    )
}

fn odp_styles() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0">
  <office:styles/>
</office:document-styles>"#
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

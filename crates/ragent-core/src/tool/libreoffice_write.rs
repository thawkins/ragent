//! LibreOffice document writing tool.
//!
//! Provides [`LibreWriteTool`], which creates new OpenDocument Text (`.odt`),
//! Spreadsheet (`.ods`), and Presentation (`.odp`) files from structured JSON input.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::Path;

use super::libreoffice_common::{LibreFormat, detect_format, resolve_path};
use super::{Tool, ToolContext, ToolOutput};

/// Writes content to ODT, ODS, or ODP files.
///
/// Accepts structured JSON content and creates the specified document type.
/// Creates parent directories if needed.
pub struct LibreWriteTool;

#[async_trait::async_trait]
impl Tool for LibreWriteTool {
    fn name(&self) -> &str {
        "libre_write"
    }

    fn description(&self) -> &str {
        "Write content to OpenDocument files: .odt, .ods, .odp"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to write the document" },
                "type": { "type": "string", "enum": ["odt","ods","odp"], "description": "Document type (auto-detected from extension if omitted)" },
                "content": { "type": "object", "description": "Document content (structure depends on type)" }
            },
            "required": ["path","content"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"].as_str().context("Missing required 'path' parameter")?;
        let content = &input["content"];
        if content.is_null() {
            bail!("Missing required 'content' parameter. Provide the document content as a JSON object.");
        }

        let path = resolve_path(&ctx.working_dir, path_str);

        let doc_type = if let Some(t) = input["type"].as_str() {
            match t {
                "odt" => LibreFormat::Odt,
                "ods" => LibreFormat::Ods,
                "odp" => LibreFormat::Odp,
                other => bail!("Unsupported document type: '{other}'"),
            }
        } else {
            detect_format(&path)?
        };

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directories: {}", parent.display()))?;
        }

        let content_clone = content.clone();
        let path_clone = path.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            match doc_type {
                LibreFormat::Odt => write_odt(&path_clone, &content_clone),
                LibreFormat::Ods => write_ods(&path_clone, &content_clone),
                LibreFormat::Odp => write_odp(&path_clone, &content_clone),
            }
        })
        .await
        .context("Failed to write document: background task exited")??;

        let file_size = tokio::fs::metadata(&path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(ToolOutput {
            content: format!("Wrote {} file ({} bytes) to {}", doc_type, file_size, path.display()),
            metadata: Some(json!({ "path": path.display().to_string(), "format": doc_type.to_string(), "bytes": file_size })),
        })
    }
}

fn write_odt(path: &Path, content: &Value) -> Result<()> {
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;
    use std::io::Write;

    let paragraphs = content["paragraphs"].as_array().context("Missing 'paragraphs' array in odt content")?;

    // Minimal ODT container: generate content.xml with simple text in office:body/text:p
    let mut content_xml = String::new();
    content_xml.push_str(r#"<?xml version='1.0' encoding='UTF-8'?>"#);
    content_xml.push_str(r#"<office:document-content xmlns:office='urn:oasis:names:tc:opendocument:xmlns:office:1.0' xmlns:text='urn:oasis:names:tc:opendocument:xmlns:text:1.0'>"#);
    content_xml.push_str("<office:body><office:text>");
    for p in paragraphs {
        let text = p["text"].as_str().unwrap_or("");
        content_xml.push_str(&format!("<text:p>{}</text:p>", xml_escape(text)));
    }
    content_xml.push_str("</office:text></office:body></office:document-content>");

    let file = std::fs::File::create(path).with_context(|| format!("Failed to create file: {}", path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    zip.start_file("mimetype", SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))?;
    zip.write_all(b"application/vnd.oasis.opendocument.text")?;

    zip.start_file("content.xml", options)?;
    zip.write_all(content_xml.as_bytes())?;

    zip.finish()?;

    Ok(())
}

fn write_ods(path: &Path, content: &Value) -> Result<()> {
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;
    use std::io::Write;

    let sheets = content["sheets"].as_array().context("Missing 'sheets' array in ods content")?;

    // Minimal ODS with content.xml containing table:table elements
    let mut content_xml = String::new();
    content_xml.push_str(r#"<?xml version='1.0' encoding='UTF-8'?>"#);
    content_xml.push_str(r#"<office:document-content xmlns:office='urn:oasis:names:tc:opendocument:xmlns:office:1.0' xmlns:table='urn:oasis:names:tc:opendocument:xmlns:table:1.0' xmlns:text='urn:oasis:names:tc:opendocument:xmlns:text:1.0'>"#);
    content_xml.push_str("<office:body><office:spreadsheet>");

    for s in sheets {
        let name = s["name"].as_str().unwrap_or("Sheet1");
        content_xml.push_str(&format!("<table:table table:name=\"{}\">", xml_escape(name)));
        if let Some(rows) = s["rows"].as_array() {
            for r in rows {
                content_xml.push_str("<table:table-row>");
                if let Some(cells) = r.as_array() {
                    for cell in cells {
                        let cell_text = match cell {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            Value::Bool(b) => b.to_string(),
                            _ => cell.to_string(),
                        };
                        content_xml.push_str(&format!("<table:table-cell><text:p>{}</text:p></table:table-cell>", xml_escape(&cell_text)));
                    }
                }
                content_xml.push_str("</table:table-row>");
            }
        }
        content_xml.push_str("</table:table>");
    }

    content_xml.push_str("</office:spreadsheet></office:body></office:document-content>");

    let file = std::fs::File::create(path).with_context(|| format!("Failed to create file: {}", path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    zip.start_file("mimetype", SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))?;
    zip.write_all(b"application/vnd.oasis.opendocument.spreadsheet")?;

    zip.start_file("content.xml", options)?;
    zip.write_all(content_xml.as_bytes())?;

    zip.finish()?;

    Ok(())
}

fn write_odp(path: &Path, content: &Value) -> Result<()> {
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;
    use std::io::Write;

    let slides = content["slides"].as_array().context("Missing 'slides' array in odp content")?;

    let mut content_xml = String::new();
    content_xml.push_str(r#"<?xml version='1.0' encoding='UTF-8'?>"#);
    content_xml.push_str(r#"<office:document-content xmlns:office='urn:oasis:names:tc:opendocument:xmlns:office:1.0' xmlns:draw='urn:oasis:names:tc:opendocument:xmlns:drawing:1.0' xmlns:text='urn:oasis:names:tc:opendocument:xmlns:text:1.0'>"#);
    content_xml.push_str("<office:body><office:presentation>");

    for s in slides {
        let title = s["title"].as_str().unwrap_or("");
        let body = s["body"].as_str().unwrap_or("");
        content_xml.push_str("<draw:page>");
        if !title.is_empty() { content_xml.push_str(&format!("<text:p>{}</text:p>", xml_escape(title))); }
        if !body.is_empty() { content_xml.push_str(&format!("<text:p>{}</text:p>", xml_escape(body))); }
        content_xml.push_str("</draw:page>");
    }

    content_xml.push_str("</office:presentation></office:body></office:document-content>");

    let file = std::fs::File::create(path).with_context(|| format!("Failed to create file: {}", path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    zip.start_file("mimetype", SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))?;
    zip.write_all(b"application/vnd.oasis.opendocument.presentation")?;

    zip.start_file("content.xml", options)?;
    zip.write_all(content_xml.as_bytes())?;

    zip.finish()?;

    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

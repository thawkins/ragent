//! `LibreOffice` document writing tool.
//!
//! Provides [`LibreWriteTool`], which creates or overwrites `OpenDocument` files.
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
    fn name(&self) -> &'static str {
        "libre_write"
    }

    /// Returns the tool description.
    fn description(&self) -> &'static str {
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
                    "description": "ODT: structured content array or plain text string. \
                        Array elements: {type:'paragraph'|'heading'|'bullet_list'|'ordered_list'|'code_block', text:'...', level:1-6, items:[...]}. \
                        ODP: structured slides array [{title:'...', content:[...]}] or plain text (slides separated by blank lines). \
                        Also accepted: object with 'paragraphs' or 'content' array key."
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

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    /// Executes the `LibreOffice` write operation.
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
        // LLMs sometimes put slides/paragraphs at the top level instead of inside "content"
        let content = if !input["content"].is_null() {
            input["content"].clone()
        } else if !input["slides"].is_null() || !input["paragraphs"].is_null() {
            input.clone()
        } else {
            Value::Null
        };
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
            content: format!("Successfully wrote {libre_format} file: {path_d}"),
            metadata: Some(json!({ "path": path_d, "format": libre_format.to_string() })),
        })
    }
}

// ── ODT ──────────────────────────────────────────────────────────────────────

/// Normalised paragraph for ODT rendering.
struct OdtPara {
    text: String,
    style: OdtStyle,
}

enum OdtStyle {
    Normal,
    Heading(u64),
    ListBullet,
    ListNumber,
    Code,
}

/// Resolve the content `Value` into a flat list of `OdtPara`.
///
/// Accepts:
/// - Plain string → one paragraph per line
/// - Bare array / object with `content` or `paragraphs` key → structured elements
fn resolve_odt_paras(content: &Value) -> Vec<OdtPara> {
    // Plain text fast path
    if let Some(text) = content.as_str() {
        return text
            .lines()
            .map(|l| OdtPara {
                text: l.to_owned(),
                style: OdtStyle::Normal,
            })
            .collect();
    }

    let elements: Vec<Value> = if let Some(arr) = content.as_array() {
        arr.clone()
    } else if let Some(arr) = content["paragraphs"].as_array() {
        arr.clone()
    } else if let Some(arr) = content["content"].as_array() {
        arr.clone()
    } else {
        // Fallback: serialise to string
        let s = content.to_string();
        return s
            .lines()
            .map(|l| OdtPara {
                text: l.to_owned(),
                style: OdtStyle::Normal,
            })
            .collect();
    };

    let mut paras: Vec<OdtPara> = Vec::new();
    for elem in &elements {
        let elem_type = elem["type"].as_str().unwrap_or("paragraph");
        match elem_type {
            "heading" => {
                let text = elem["text"]
                    .as_str()
                    .or_else(|| elem["heading"].as_str())
                    .unwrap_or("");
                let level = elem["level"].as_u64().unwrap_or(1).clamp(1, 6);
                paras.push(OdtPara {
                    text: text.to_owned(),
                    style: OdtStyle::Heading(level),
                });
            }
            "bullet_list" => {
                if let Some(items) = elem["items"].as_array() {
                    for item in items {
                        let text = item
                            .as_str()
                            .unwrap_or_else(|| item["text"].as_str().unwrap_or(""));
                        paras.push(OdtPara {
                            text: text.to_owned(),
                            style: OdtStyle::ListBullet,
                        });
                    }
                }
            }
            "ordered_list" | "numbered_list" => {
                if let Some(items) = elem["items"].as_array() {
                    for item in items {
                        let text = item
                            .as_str()
                            .unwrap_or_else(|| item["text"].as_str().unwrap_or(""));
                        paras.push(OdtPara {
                            text: text.to_owned(),
                            style: OdtStyle::ListNumber,
                        });
                    }
                }
            }
            "code_block" => {
                paras.push(OdtPara {
                    text: elem["text"].as_str().unwrap_or("").to_owned(),
                    style: OdtStyle::Code,
                });
            }
            _ => {
                if elem["heading"].as_str().is_some() || elem["level"].as_u64().is_some() {
                    let t = elem["heading"]
                        .as_str()
                        .or_else(|| elem["text"].as_str())
                        .unwrap_or("");
                    let level = elem["level"].as_u64().unwrap_or(1).clamp(1, 6);
                    paras.push(OdtPara {
                        text: t.to_owned(),
                        style: OdtStyle::Heading(level),
                    });
                } else {
                    let text = elem["text"].as_str().unwrap_or("");
                    let style = match elem["style"].as_str().unwrap_or("Normal") {
                        "Heading1" | "heading1" | "heading_1" => OdtStyle::Heading(1),
                        "Heading2" | "heading2" | "heading_2" => OdtStyle::Heading(2),
                        "Heading3" | "heading3" | "heading_3" => OdtStyle::Heading(3),
                        "Heading4" | "heading4" | "heading_4" => OdtStyle::Heading(4),
                        "Heading5" | "heading5" | "heading_5" => OdtStyle::Heading(5),
                        "Heading6" | "heading6" | "heading_6" => OdtStyle::Heading(6),
                        "ListBullet" | "listbullet" | "list_bullet" => OdtStyle::ListBullet,
                        "ListNumber" | "listnumber" | "list_number" => OdtStyle::ListNumber,
                        "Code" | "code" | "Preformatted" => OdtStyle::Code,
                        _ => OdtStyle::Normal,
                    };
                    paras.push(OdtPara {
                        text: text.to_owned(),
                        style,
                    });
                }
            }
        }
    }
    paras
}

fn write_odt(path: &Path, content: &Value, title: &str, author: &str) -> Result<()> {
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

    let paras = resolve_odt_paras(content);
    zip.start_file("content.xml", opts)?;
    zip.write_all(odt_content_structured(&paras).as_bytes())?;

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

fn odt_content_structured(paras: &[OdtPara]) -> String {
    let body: String = paras.iter().map(|p| {
        let escaped = xml_escape(&p.text);
        match &p.style {
            OdtStyle::Normal => format!("    <text:p text:style-name=\"Text_20_Body\">{escaped}</text:p>\n"),
            OdtStyle::Heading(level) => format!("    <text:h text:style-name=\"Heading_20_{level}\" text:outline-level=\"{level}\">{escaped}</text:h>\n"),
            OdtStyle::ListBullet => format!("    <text:p text:style-name=\"List_20_Bullet\">{escaped}</text:p>\n"),
            OdtStyle::ListNumber => format!("    <text:p text:style-name=\"List_20_Number\">{escaped}</text:p>\n"),
            OdtStyle::Code => format!("    <text:p text:style-name=\"Preformatted_20_Text\">{escaped}</text:p>\n"),
        }
    }).collect();

    let styles = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        "\n",
        r#"<office:document-content"#,
        "\n",
        r#"  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0""#,
        "\n",
        r#"  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0""#,
        "\n",
        r#"  xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0""#,
        "\n",
        r#"  xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0">"#,
        "\n",
        r#"  <office:automatic-styles>"#,
        "\n",
        r#"    <style:style style:name="Text_20_Body" style:family="paragraph"><style:paragraph-properties fo:margin-top="0.1in" fo:margin-bottom="0.05in"/></style:style>"#,
        "\n",
        r#"    <style:style style:name="Heading_20_1" style:family="paragraph"><style:text-properties fo:font-size="18pt" fo:font-weight="bold"/><style:paragraph-properties fo:margin-top="0.2in" fo:margin-bottom="0.1in"/></style:style>"#,
        "\n",
        r#"    <style:style style:name="Heading_20_2" style:family="paragraph"><style:text-properties fo:font-size="14pt" fo:font-weight="bold"/><style:paragraph-properties fo:margin-top="0.15in" fo:margin-bottom="0.08in"/></style:style>"#,
        "\n",
        r#"    <style:style style:name="Heading_20_3" style:family="paragraph"><style:text-properties fo:font-size="12pt" fo:font-weight="bold"/><style:paragraph-properties fo:margin-top="0.1in" fo:margin-bottom="0.05in"/></style:style>"#,
        "\n",
        r#"    <style:style style:name="Heading_20_4" style:family="paragraph"><style:text-properties fo:font-size="11pt" fo:font-weight="bold" fo:font-style="italic"/></style:style>"#,
        "\n",
        r#"    <style:style style:name="Heading_20_5" style:family="paragraph"><style:text-properties fo:font-size="10pt" fo:font-weight="bold"/></style:style>"#,
        "\n",
        r#"    <style:style style:name="Heading_20_6" style:family="paragraph"><style:text-properties fo:font-size="10pt" fo:font-style="italic"/></style:style>"#,
        "\n",
        r#"    <style:style style:name="List_20_Bullet" style:family="paragraph"><style:paragraph-properties fo:margin-left="0.3in" fo:text-indent="-0.2in"/></style:style>"#,
        "\n",
        r#"    <style:style style:name="List_20_Number" style:family="paragraph"><style:paragraph-properties fo:margin-left="0.3in" fo:text-indent="-0.2in"/></style:style>"#,
        "\n",
        r#"    <style:style style:name="Preformatted_20_Text" style:family="paragraph"><style:text-properties style:font-name="Courier New" fo:font-size="10pt"/></style:style>"#,
        "\n",
        r#"  </office:automatic-styles>"#,
        "\n",
    );

    format!(
        "{styles}  <office:body>\n    <office:text>\n{body}    </office:text>\n  </office:body>\n</office:document-content>"
    )
}

const fn odt_styles() -> &'static str {
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

struct OdpSlide {
    title: String,
    lines: Vec<String>,
}

fn resolve_odp_slides(content: &Value) -> Vec<OdpSlide> {
    // If content is a JSON string, try parsing it first
    if let Some(s) = content.as_str() {
        if let Ok(parsed) = serde_json::from_str::<Value>(s) {
            let result = resolve_odp_slides(&parsed);
            if !result.is_empty() {
                return result;
            }
        }
        // Fall through to plain-text handling below
    }

    // Helper to convert a single slide object to OdpSlide
    let slide_from_obj = |s: &Value| -> OdpSlide {
        let title = s["title"].as_str().unwrap_or("").to_owned();
        // Accept "content", "body", or "text" for slide body
        let body_val = if !s["content"].is_null() {
            &s["content"]
        } else if !s["body"].is_null() {
            &s["body"]
        } else {
            &s["text"]
        };
        let lines: Vec<String> = if let Some(c) = body_val.as_array() {
            c.iter()
                .map(|item| {
                    item.as_str()
                        .unwrap_or_else(|| item["text"].as_str().unwrap_or(""))
                        .to_owned()
                })
                .collect()
        } else if let Some(t) = body_val.as_str() {
            t.lines().map(str::to_owned).collect()
        } else {
            Vec::new()
        };
        OdpSlide { title, lines }
    };

    // Bare array of slide objects
    if let Some(arr) = content.as_array() {
        if arr.first().is_some_and(serde_json::Value::is_object) {
            return arr.iter().map(|s| slide_from_obj(s)).collect();
        }
    }

    // Object with "slides" key
    if let Some(arr) = content["slides"].as_array() {
        return arr.iter().map(|s| slide_from_obj(s)).collect();
    }

    // Single-key wrapper — look inside
    if let Some(obj) = content.as_object() {
        if obj.len() == 1 {
            let inner = obj.values().next().unwrap();
            let result = resolve_odp_slides(inner);
            if !result.is_empty() {
                return result;
            }
        }
        // Heuristic: find first value that is an array of objects
        for v in obj.values() {
            if let Some(arr) = v.as_array() {
                if !arr.is_empty() && arr[0].is_object() {
                    return arr.iter().map(|s| slide_from_obj(s)).collect();
                }
            }
        }
    }

    // Plain text: split on blank lines
    let text = content.as_str().unwrap_or("");
    text.split("\n\n")
        .filter(|block| !block.trim().is_empty())
        .map(|block| {
            let mut lines = block.lines();
            let title = lines.next().unwrap_or("").to_owned();
            let rest: Vec<String> = lines.map(str::to_owned).collect();
            OdpSlide { title, lines: rest }
        })
        .collect()
}

fn write_odp(path: &Path, content: &Value, title: &str, author: &str) -> Result<()> {
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

    let slides = resolve_odp_slides(content);
    zip.start_file("content.xml", opts)?;
    zip.write_all(odp_content_structured(&slides).as_bytes())?;

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

fn odp_content_structured(slides: &[OdpSlide]) -> String {
    let mut slide_xml = String::new();
    for (i, slide) in slides.iter().enumerate() {
        let title_xml = if slide.title.is_empty() {
            String::new()
        } else {
            format!(
                "      <draw:frame draw:name=\"Title\" presentation:class=\"title\" \
                 svg:x=\"0.5in\" svg:y=\"0.5in\" svg:width=\"9in\" svg:height=\"1.2in\">\
                 <draw:text-box><text:p><text:span text:style-name=\"bold\">{}</text:span></text:p></draw:text-box>\
                 </draw:frame>\n",
                xml_escape(&slide.title)
            )
        };
        let body_xml: String = slide
            .lines
            .iter()
            .map(|l| {
                format!(
                    "      <draw:frame presentation:class=\"body\" \
                 svg:x=\"0.5in\" svg:y=\"2in\" svg:width=\"9in\" svg:height=\"5in\">\
                 <draw:text-box><text:p>{}</text:p></draw:text-box></draw:frame>\n",
                    xml_escape(l)
                )
            })
            .collect();
        slide_xml.push_str(&format!(
            "    <draw:page draw:name=\"Slide {n}\" draw:master-page-name=\"Default\">\n{title_xml}{body_xml}    </draw:page>\n",
            n = i + 1,
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0"
  xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0">
  <office:body>
    <office:presentation>
{slide_xml}    </office:presentation>
  </office:body>
</office:document-content>"#
    )
}

const fn odp_styles() -> &'static str {
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

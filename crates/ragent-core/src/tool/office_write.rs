//! Office document writing tool.
//!
//! Provides [`OfficeWriteTool`], which creates new Microsoft Word (`.docx`),
//! Excel (`.xlsx`), and PowerPoint (`.pptx`) files from structured JSON input.
//!
//! Depends on: `docx-rust`, `rust_xlsxwriter`, `ooxmlsdk`.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::Path;

use super::office_common::{OfficeFormat, detect_format, resolve_path};
use super::{Tool, ToolContext, ToolOutput};

/// Writes content to Word, Excel, or PowerPoint files.
///
/// Accepts structured JSON content and creates the specified document type.
/// Creates parent directories if needed.
pub struct OfficeWriteTool;

#[async_trait::async_trait]
impl Tool for OfficeWriteTool {
    fn name(&self) -> &str {
        "office_write"
    }

    fn description(&self) -> &str {
        "Write content to Word (.docx), Excel (.xlsx), or PowerPoint (.pptx) files."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to write the Office document"
                },
                "type": {
                    "type": "string",
                    "enum": ["docx", "xlsx", "pptx"],
                    "description": "Document type (auto-detected from extension if omitted)"
                },
                "title": {
                    "type": "string",
                    "description": "Optional document title (docx only)"
                },
                "content": {
                    "description": "Document content. For docx: either an array of paragraph/heading/list objects, or an object with a 'paragraphs' or 'content' array. Each item: {type:'paragraph'|'heading'|'bullet_list'|'code_block', text:'...', level:1-6, items:[...], style:'Normal'|'Heading1'|...}. For xlsx: {sheets:[{name,rows}]}. For pptx: {slides:[{title,content:[...]}]}."
                }
            },
            "required": ["path", "content"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;
        let content = &input["content"];
        if content.is_null() {
            bail!(
                "Missing required 'content' parameter. Provide the document content as a JSON object."
            );
        }

        let path = resolve_path(&ctx.working_dir, path_str);

        let doc_type = if let Some(t) = input["type"].as_str() {
            match t {
                "docx" => OfficeFormat::Docx,
                "xlsx" => OfficeFormat::Xlsx,
                "pptx" => OfficeFormat::Pptx,
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
                OfficeFormat::Docx => write_docx(&path_clone, &content_clone),
                OfficeFormat::Xlsx => write_xlsx(&path_clone, &content_clone),
                OfficeFormat::Pptx => write_pptx(&path_clone, &content_clone),
            }
        })
        .await
        .context("Failed to write document: the background task exited unexpectedly")??;

        let file_size = tokio::fs::metadata(&path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(ToolOutput {
            content: format!(
                "Wrote {} file ({} bytes) to {}",
                doc_type,
                file_size,
                path.display()
            ),
            metadata: Some(json!({
                "path": path.display().to_string(),
                "format": doc_type.to_string(),
                "bytes": file_size,
            })),
        })
    }
}

/// Writes a Word document from structured JSON content.
///
/// Accepts several content shapes to accommodate different LLM outputs:
///
/// 1. Legacy object with `paragraphs` array:
///    `{ "paragraphs": [{ "text": "...", "style": "Heading1" }] }`
///
/// 2. Object with `content` array:
///    `{ "content": [{ "type": "paragraph", "text": "..." }] }`
///
/// 3. Bare array at the top level:
///    `[{ "type": "heading", "text": "Title", "level": 1 }, ...]`
///
/// Each element in the array is normalised to a set of paragraphs:
/// - `{type: "paragraph", text}` / `{text, style: "Normal"}` → plain paragraph
/// - `{type: "heading", text, level}` / `{heading, level}` / `{text, style: "HeadingN"}` → heading
/// - `{type: "bullet_list", items: [...]}` → one bullet paragraph per item
/// - `{type: "ordered_list", items: [...]}` → one numbered paragraph per item
/// - `{type: "code_block", text}` → code-styled paragraph
fn write_docx(path: &Path, content: &Value) -> Result<()> {
    use docx_rust::document::Paragraph;
    use docx_rust::formatting::{CharacterProperty, ParagraphProperty};

    // ── Resolve the list of content elements ──────────────────────────────
    let elements: Vec<Value> = if let Some(arr) = content.as_array() {
        arr.clone()
    } else if let Some(arr) = content["paragraphs"].as_array() {
        arr.clone()
    } else if let Some(arr) = content["content"].as_array() {
        arr.clone()
    } else {
        bail!(
            "docx content must be an array of elements, or an object with a \
             'paragraphs' or 'content' array. \
             Example: {{\"paragraphs\":[{{\"text\":\"Hello\",\"style\":\"Normal\"}}]}}"
        );
    };

    // ── Normalise every element into owned (text, style_id) pairs ─────────
    //
    // We must collect these before creating `docx_rust::Docx` because the
    // library stores &str references tied to the document's lifetime.
    // Owning the strings here ensures they outlive the document builder.
    struct ParaEntry {
        text: String,
        style_id: String, // empty = Normal (no explicit style)
    }

    let mut paras: Vec<ParaEntry> = Vec::new();

    let style_canonical = |s: &str| -> String {
        match s {
            "Normal" | "normal" | "" => String::new(),
            "Heading1" | "heading1" => "Heading1".to_owned(),
            "Heading2" | "heading2" => "Heading2".to_owned(),
            "Heading3" | "heading3" => "Heading3".to_owned(),
            "Heading4" | "heading4" => "Heading4".to_owned(),
            "Heading5" | "heading5" => "Heading5".to_owned(),
            "Heading6" | "heading6" => "Heading6".to_owned(),
            "ListBullet" | "listbullet" | "list_bullet" => "ListBullet".to_owned(),
            "ListNumber" | "listnumber" | "list_number" => "ListNumber".to_owned(),
            "Code" | "code" => "Code".to_owned(),
            other => other.to_owned(),
        }
    };

    for elem in &elements {
        let elem_type = elem["type"].as_str().unwrap_or("paragraph");

        match elem_type {
            "heading" => {
                let text = elem["text"].as_str().or_else(|| elem["heading"].as_str()).unwrap_or("");
                let level = elem["level"].as_u64().unwrap_or(1).clamp(1, 6);
                paras.push(ParaEntry { text: text.to_owned(), style_id: format!("Heading{level}") });
            }
            "bullet_list" => {
                if let Some(items) = elem["items"].as_array() {
                    for item in items {
                        let text = item.as_str().unwrap_or_else(|| item["text"].as_str().unwrap_or(""));
                        paras.push(ParaEntry { text: text.to_owned(), style_id: "ListBullet".to_owned() });
                    }
                }
            }
            "ordered_list" | "numbered_list" => {
                if let Some(items) = elem["items"].as_array() {
                    for item in items {
                        let text = item.as_str().unwrap_or_else(|| item["text"].as_str().unwrap_or(""));
                        paras.push(ParaEntry { text: text.to_owned(), style_id: "ListNumber".to_owned() });
                    }
                }
            }
            "code_block" => {
                paras.push(ParaEntry {
                    text: elem["text"].as_str().unwrap_or("").to_owned(),
                    style_id: "Code".to_owned(),
                });
            }
            _ => {
                // "paragraph" or unknown — also handles legacy {heading, level} without "type"
                if elem["heading"].as_str().is_some() || elem["level"].as_u64().is_some() {
                    let heading_text = elem["heading"].as_str().or_else(|| elem["text"].as_str()).unwrap_or("");
                    let level = elem["level"].as_u64().unwrap_or(1).clamp(1, 6);
                    paras.push(ParaEntry { text: heading_text.to_owned(), style_id: format!("Heading{level}") });
                } else {
                    let text = elem["text"].as_str().unwrap_or("");
                    let style = elem["style"].as_str().unwrap_or("Normal");
                    paras.push(ParaEntry { text: text.to_owned(), style_id: style_canonical(style).to_owned() });
                }
            }
        }
    }

    // ── Build the document ─────────────────────────────────────────────────
    let mut docx = docx_rust::Docx::default();

    for entry in &paras {
        let sid: &str = &entry.style_id;
        let mut para = if sid.is_empty() {
            Paragraph::default()
        } else {
            Paragraph::default().property(ParagraphProperty::default().style_id(sid))
        };

        for (seg_text, bold, italic, code) in parse_inline_formatting(&entry.text) {
            let mut cp = CharacterProperty::default();
            if bold { cp = cp.bold(true); }
            if italic { cp = cp.italics(true); }
            if code { cp = cp.style_id("Code"); }
            let run = docx_rust::document::Run::default().property(cp).push_text(seg_text);
            para = para.push(run);
        }
        docx.document.push(para);
    }

    docx.write_file(path)
        .map_err(|e| anyhow::anyhow!("Failed to write docx: {e}"))?;

    Ok(())
}

/// Parses markdown-like inline formatting from text.
///
/// Supports `**bold**`, `*italic*`, and `` `code` `` syntax.
///
/// # Arguments
///
/// * `text` - The text to parse for inline formatting.
///
/// # Returns
///
/// A vector of `(text, is_bold, is_italic, is_code)` segments.
fn parse_inline_formatting(text: &str) -> Vec<(String, bool, bool, bool)> {
    let mut segments: Vec<(String, bool, bool, bool)> = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            if !current.is_empty() {
                segments.push((current.clone(), false, false, false));
                current.clear();
            }
            i += 2;
            let mut bold_text = String::new();
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '*') {
                bold_text.push(chars[i]);
                i += 1;
            }
            if i + 1 < len {
                i += 2;
            }
            if !bold_text.is_empty() {
                segments.push((bold_text, true, false, false));
            }
        } else if chars[i] == '*' && (i + 1 >= len || chars[i + 1] != '*') {
            if !current.is_empty() {
                segments.push((current.clone(), false, false, false));
                current.clear();
            }
            i += 1;
            let mut italic_text = String::new();
            while i < len && chars[i] != '*' {
                italic_text.push(chars[i]);
                i += 1;
            }
            if i < len {
                i += 1;
            }
            if !italic_text.is_empty() {
                segments.push((italic_text, false, true, false));
            }
        } else if chars[i] == '`' {
            if !current.is_empty() {
                segments.push((current.clone(), false, false, false));
                current.clear();
            }
            i += 1;
            let mut code_text = String::new();
            while i < len && chars[i] != '`' {
                code_text.push(chars[i]);
                i += 1;
            }
            if i < len {
                i += 1;
            }
            if !code_text.is_empty() {
                segments.push((code_text, false, false, true));
            }
        } else {
            current.push(chars[i]);
            i += 1;
        }
    }

    if !current.is_empty() {
        segments.push((current, false, false, false));
    }

    if segments.is_empty() {
        segments.push((text.to_string(), false, false, false));
    }

    segments
}

/// Writes an Excel workbook from structured JSON content.
///
/// Expected content format:
/// ```json
/// { "sheets": [{ "name": "Sheet1", "rows": [["A1", "B1"], ["A2", "B2"]] }] }
/// ```
///
/// # Arguments
///
/// * `path` - Output file path.
/// * `content` - JSON content describing the workbook.
///
/// # Returns
///
/// `Ok(())` on success, or an error.
fn write_xlsx(path: &Path, content: &Value) -> Result<()> {
    use rust_xlsxwriter::Workbook;

    let mut workbook = Workbook::new();

    let sheets = content["sheets"]
        .as_array()
        .context("Missing 'sheets' array in xlsx content")?;

    for sheet_def in sheets {
        let sheet_name = sheet_def["name"].as_str().unwrap_or("Sheet1");

        let worksheet = workbook.add_worksheet();
        worksheet
            .set_name(sheet_name)
            .map_err(|e| anyhow::anyhow!("Failed to set sheet name: {e}"))?;

        if let Some(rows) = sheet_def["rows"].as_array() {
            for (row_idx, row) in rows.iter().enumerate() {
                if let Some(cells) = row.as_array() {
                    for (col_idx, cell) in cells.iter().enumerate() {
                        let row_num = row_idx as u32;
                        let col_num = col_idx as u16;

                        match cell {
                            Value::Number(n) => {
                                if let Some(f) = n.as_f64() {
                                    worksheet.write_number(row_num, col_num, f).map_err(|e| {
                                        anyhow::anyhow!("Failed to write number: {e}")
                                    })?;
                                }
                            }
                            Value::Bool(b) => {
                                worksheet
                                    .write_boolean(row_num, col_num, *b)
                                    .map_err(|e| anyhow::anyhow!("Failed to write boolean: {e}"))?;
                            }
                            Value::String(s) => {
                                worksheet
                                    .write_string(row_num, col_num, s)
                                    .map_err(|e| anyhow::anyhow!("Failed to write string: {e}"))?;
                            }
                            Value::Null => {}
                            _ => {
                                let s = cell.to_string();
                                worksheet
                                    .write_string(row_num, col_num, &s)
                                    .map_err(|e| anyhow::anyhow!("Failed to write value: {e}"))?;
                            }
                        }
                    }
                }
            }
        }
    }

    workbook
        .save(path)
        .map_err(|e| anyhow::anyhow!("Failed to save xlsx: {e}"))?;

    Ok(())
}

/// Writes a PowerPoint presentation from structured JSON content.
///
/// Expected content format:
/// ```json
/// { "slides": [{ "title": "...", "body": "...", "notes": "..." }] }
/// ```
///
/// Uses raw ZIP-based XML generation since ooxmlsdk's creation API is limited.
///
/// # Arguments
///
/// * `path` - Output file path.
/// * `content` - JSON content describing the presentation.
///
/// # Returns
///
/// `Ok(())` on success, or an error.
fn write_pptx(path: &Path, content: &Value) -> Result<()> {
    use std::io::Write;

    let slides = content["slides"]
        .as_array()
        .context("Missing 'slides' array in pptx content")?;

    let file = std::fs::File::create(path)
        .with_context(|| format!("Failed to create file: {}", path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("[Content_Types].xml", options)
        .context("Failed to write content types")?;
    write!(zip, "{}", generate_content_types_xml(slides.len()))
        .context("Failed to write content types XML")?;

    zip.start_file("_rels/.rels", options)
        .context("Failed to write rels")?;
    write!(zip, "{}", generate_root_rels_xml()).context("Failed to write root rels XML")?;

    zip.start_file("ppt/presentation.xml", options)
        .context("Failed to write presentation")?;
    write!(zip, "{}", generate_presentation_xml(slides.len()))
        .context("Failed to write presentation XML")?;

    zip.start_file("ppt/_rels/presentation.xml.rels", options)
        .context("Failed to write presentation rels")?;
    write!(zip, "{}", generate_presentation_rels_xml(slides.len()))
        .context("Failed to write presentation rels XML")?;

    for (i, slide_def) in slides.iter().enumerate() {
        let slide_num = i + 1;
        let title = slide_def["title"].as_str().unwrap_or("");
        let body = slide_def["body"].as_str().unwrap_or("");

        zip.start_file(format!("ppt/slides/slide{slide_num}.xml"), options)
            .context("Failed to write slide")?;
        write!(zip, "{}", generate_slide_xml(title, body)).context("Failed to write slide XML")?;

        zip.start_file(
            format!("ppt/slides/_rels/slide{slide_num}.xml.rels"),
            options,
        )
        .context("Failed to write slide rels")?;
        write!(zip, "{}", generate_slide_rels_xml()).context("Failed to write slide rels XML")?;

        if let Some(notes) = slide_def["notes"].as_str() {
            if !notes.is_empty() {
                zip.start_file(
                    format!("ppt/notesSlides/notesSlide{slide_num}.xml"),
                    options,
                )
                .context("Failed to write notes slide")?;
                write!(zip, "{}", generate_notes_slide_xml(notes))
                    .context("Failed to write notes slide XML")?;
            }
        }
    }

    zip.start_file("ppt/slideMasters/slideMaster1.xml", options)
        .context("Failed to write slide master")?;
    write!(zip, "{}", generate_slide_master_xml()).context("Failed to write slide master XML")?;

    zip.start_file("ppt/slideLayouts/slideLayout1.xml", options)
        .context("Failed to write slide layout")?;
    write!(zip, "{}", generate_slide_layout_xml()).context("Failed to write slide layout XML")?;

    zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", options)
        .context("Failed to write slide master rels")?;
    write!(zip, "{}", generate_slide_master_rels_xml())
        .context("Failed to write slide master rels XML")?;

    zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", options)
        .context("Failed to write slide layout rels")?;
    write!(zip, "{}", generate_slide_layout_rels_xml())
        .context("Failed to write slide layout rels XML")?;

    zip.start_file("ppt/theme/theme1.xml", options)
        .context("Failed to write theme")?;
    write!(zip, "{}", generate_theme_xml()).context("Failed to write theme XML")?;

    zip.finish().context("Failed to finalize ZIP")?;

    Ok(())
}

/// XML escape helper for PowerPoint content.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn generate_content_types_xml(slide_count: usize) -> String {
    let mut overrides = String::new();
    for i in 1..=slide_count {
        overrides.push_str(&format!(
            r#"<Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="xml" ContentType="application/xml"/>
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
<Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
<Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
{overrides}
</Types>"#
    )
}

fn generate_root_rels_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#
}

fn generate_presentation_xml(slide_count: usize) -> String {
    let mut slide_list = String::new();
    for i in 1..=slide_count {
        slide_list.push_str(&format!(
            r#"<p:sldId id="{}" r:id="rId{}"/>"#,
            255 + i,
            i + 1
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>
<p:sldIdLst>{slide_list}</p:sldIdLst>
<p:sldSz cx="9144000" cy="6858000" type="screen4x3"/>
<p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>"#
    )
}

fn generate_presentation_rels_xml(slide_count: usize) -> String {
    let mut rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#,
    );
    for i in 1..=slide_count {
        rels.push_str(&format!(
            r#"
<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{i}.xml"/>"#,
            i + 1
        ));
    }
    rels.push_str("\n</Relationships>");
    rels
}

fn generate_slide_xml(title: &str, body: &str) -> String {
    let title_escaped = xml_escape(title);
    let body_escaped = xml_escape(body);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld>
<p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
<p:sp>
<p:nvSpPr><p:cNvPr id="2" name="Title 1"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
<p:spPr/>
<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US" dirty="0"/><a:t>{title_escaped}</a:t></a:r></a:p></p:txBody>
</p:sp>
<p:sp>
<p:nvSpPr><p:cNvPr id="3" name="Content 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph idx="1"/></p:nvPr></p:nvSpPr>
<p:spPr/>
<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US" dirty="0"/><a:t>{body_escaped}</a:t></a:r></a:p></p:txBody>
</p:sp>
</p:spTree>
</p:cSld>
</p:sld>"#
    )
}

fn generate_slide_rels_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#
}

fn generate_notes_slide_xml(notes: &str) -> String {
    let notes_escaped = xml_escape(notes);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:notes xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld>
<p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
<p:sp>
<p:nvSpPr><p:cNvPr id="2" name="Notes 1"/><p:cNvSpPr/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr>
<p:spPr/>
<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>{notes_escaped}</a:t></a:r></a:p></p:txBody>
</p:sp>
</p:spTree>
</p:cSld>
</p:notes>"#
    )
}

fn generate_slide_master_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld>
<p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg>
<p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
</p:spTree>
</p:cSld>
<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst>
<p:txStyles>
<p:titleStyle><a:lvl1pPr algn="ctr"><a:defRPr sz="4400"/></a:lvl1pPr></p:titleStyle>
<p:bodyStyle><a:lvl1pPr><a:defRPr sz="3200"/></a:lvl1pPr></p:bodyStyle>
<p:otherStyle><a:lvl1pPr><a:defRPr sz="1800"/></a:lvl1pPr></p:otherStyle>
</p:txStyles>
</p:sldMaster>"#
}

fn generate_slide_layout_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="titleOnly">
<p:cSld name="Title Only">
<p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
</p:spTree>
</p:cSld>
</p:sldLayout>"#
}

fn generate_slide_master_rels_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>"#
}

fn generate_slide_layout_rels_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#
}

fn generate_theme_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office Theme">
<a:themeElements>
<a:clrScheme name="Office">
<a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
<a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
<a:dk2><a:srgbClr val="44546A"/></a:dk2>
<a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>
<a:accent1><a:srgbClr val="5B9BD5"/></a:accent1>
<a:accent2><a:srgbClr val="ED7D31"/></a:accent2>
<a:accent3><a:srgbClr val="A5A5A5"/></a:accent3>
<a:accent4><a:srgbClr val="FFC000"/></a:accent4>
<a:accent5><a:srgbClr val="4472C4"/></a:accent5>
<a:accent6><a:srgbClr val="70AD47"/></a:accent6>
<a:hlink><a:srgbClr val="0563C1"/></a:hlink>
<a:folHlink><a:srgbClr val="954F72"/></a:folHlink>
</a:clrScheme>
<a:fontScheme name="Office">
<a:majorFont><a:latin typeface="Calibri Light"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont>
<a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont>
</a:fontScheme>
<a:fmtScheme name="Office">
<a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst>
<a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst>
<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>
<a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst>
</a:fmtScheme>
</a:themeElements>
</a:theme>"#
}

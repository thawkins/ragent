//! PDF writing tool.
//!
//! Provides [`PdfWriteTool`], which creates PDF files from structured JSON
//! content. Supports text paragraphs with headings, tables, and embedded
//! images.

use anyhow::{Context, Result, bail};
use printpdf::{
    BuiltinFont, Color, Greyscale, Line, LinePoint, Mm, Op, PdfDocument, PdfFontHandle, PdfPage,
    PdfSaveOptions, PdfWarnMsg, Point, Pt, RawImage, TextItem, XObjectTransform,
};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

use super::office_common::resolve_path;
use super::{Tool, ToolContext, ToolOutput};

/// Creates PDF files from structured JSON content.
///
/// Supports paragraphs (with heading/body styles), tables, and images.
pub struct PdfWriteTool;

// Layout constants (mm)
const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN_LEFT: f32 = 25.0;
const MARGIN_RIGHT: f32 = 25.0;
const MARGIN_TOP: f32 = 25.0;
const MARGIN_BOTTOM: f32 = 25.0;
const LINE_SPACING: f32 = 1.4;

const BODY_FONT_SIZE: f32 = 11.0;
const H1_FONT_SIZE: f32 = 22.0;
const H2_FONT_SIZE: f32 = 17.0;
const H3_FONT_SIZE: f32 = 13.0;

const CONTENT_WIDTH: f32 = PAGE_W - MARGIN_LEFT - MARGIN_RIGHT;

#[async_trait::async_trait]
impl Tool for PdfWriteTool {
    fn name(&self) -> &'static str {
        "pdf_write"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &'static str {
        "Create a PDF file from structured content. Supports text paragraphs with headings, tables, and embedded images."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path for the output PDF file"
                },
                "content": {
                    "type": "object",
                    "description": "Document content",
                    "properties": {
                        "title": {
                            "type": "string",
                            "description": "Optional document title (shown as first heading)"
                        },
                        "elements": {
                            "type": "array",
                            "description": "Ordered list of content elements",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "type": {
                                        "type": "string",
                                        "enum": ["paragraph", "heading", "table", "image"],
                                        "description": "Element type"
                                    },
                                    "text": {
                                        "type": "string",
                                        "description": "Text content (for paragraph and heading)"
                                    },
                                    "level": {
                                        "type": "integer",
                                        "description": "Heading level 1-3 (for heading type, default 1)"
                                    },
                                    "headers": {
                                        "type": "array",
                                        "items": { "type": "string" },
                                        "description": "Table column headers (for table type)"
                                    },
                                    "rows": {
                                        "type": "array",
                                        "items": {
                                            "type": "array",
                                            "items": { "type": "string" }
                                        },
                                        "description": "Table rows, each an array of cell strings (for table type)"
                                    },
                                    "image_path": {
                                        "type": "string",
                                        "description": "Path to image file — PNG or JPEG (for image type)"
                                    },
                                    "width_mm": {
                                        "type": "number",
                                        "description": "Image display width in mm (for image type, default: fit page width)"
                                    },
                                    "caption": {
                                        "type": "string",
                                        "description": "Optional caption below the image"
                                    }
                                },
                                "required": ["type"]
                            }
                        }
                    },
                    "required": ["elements"]
                }
            },
            "required": ["path", "content"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    /// # Errors
    ///
    /// Returns an error if the `path` or `content` parameters are missing,
    /// if PDF creation fails, or if the background task exits unexpectedly.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;
        let path = resolve_path(&ctx.working_dir, path_str);
        let content = input
            .get("content")
            .context("Missing 'content' parameter")?
            .clone();

        let working_dir = ctx.working_dir.clone();
        let path_clone = path.clone();

        let bytes_written =
            tokio::task::spawn_blocking(move || write_pdf(&path_clone, &content, &working_dir))
                .await
                .context("Failed to write PDF: the background task exited unexpectedly")??;

        Ok(ToolOutput {
            content: format!("Wrote PDF ({} bytes) to {}", bytes_written, path.display()),
            metadata: Some(json!({
                "path": path.display().to_string(),
                "bytes": bytes_written,
                "format": "pdf",
            })),
        })
    }
}

/// Cursor tracking Y position and managing page breaks.
struct Cursor {
    y: f32, // current Y in mm from page bottom
}

impl Cursor {
    fn new() -> Self {
        Self {
            y: PAGE_H - MARGIN_TOP,
        }
    }

    fn needs_new_page(&self, needed_height: f32) -> bool {
        self.y - needed_height < MARGIN_BOTTOM
    }

    fn advance(&mut self, height: f32) {
        self.y -= height;
    }

    fn reset(&mut self) {
        self.y = PAGE_H - MARGIN_TOP;
    }
}

fn write_pdf(path: &Path, content: &Value, working_dir: &Path) -> Result<usize> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    let title = content
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Document");

    let mut doc = PdfDocument::new(title);
    let font_regular = PdfFontHandle::Builtin(BuiltinFont::Helvetica);
    let font_bold = PdfFontHandle::Builtin(BuiltinFont::HelveticaBold);

    let mut cursor = Cursor::new();
    let mut ops: Vec<Op> = Vec::new();
    let mut pages: Vec<PdfPage> = Vec::new();

    let elements = content
        .get("elements")
        .and_then(|v| v.as_array())
        .context("Missing 'elements' array in content")?;

    let flush_page = |ops: &mut Vec<Op>, pages: &mut Vec<PdfPage>, cursor: &mut Cursor| {
        if !ops.is_empty() {
            pages.push(PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), std::mem::take(ops)));
        }
        cursor.reset();
    };

    // Render title if provided
    if let Some(title_text) = content.get("title").and_then(|v| v.as_str()) {
        let line_height_mm = H1_FONT_SIZE * LINE_SPACING * 0.3528;
        emit_text(
            &mut ops,
            title_text,
            H1_FONT_SIZE,
            &font_bold,
            MARGIN_LEFT,
            cursor.y,
        );
        cursor.advance(line_height_mm + 4.0);
    }

    for element in elements {
        let el_type = element
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("paragraph");

        match el_type {
            "heading" => {
                let text = element.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let level = element
                    .get("level")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(1) as usize;

                let font_size = match level {
                    1 => H1_FONT_SIZE,
                    2 => H2_FONT_SIZE,
                    _ => H3_FONT_SIZE,
                };
                let line_height_mm = font_size * LINE_SPACING * 0.3528;
                let spacing_before = match level {
                    1 => 6.0,
                    2 => 4.0,
                    _ => 3.0,
                };

                if cursor.needs_new_page(line_height_mm + spacing_before) {
                    flush_page(&mut ops, &mut pages, &mut cursor);
                }

                cursor.advance(spacing_before);
                emit_text(&mut ops, text, font_size, &font_bold, MARGIN_LEFT, cursor.y);
                cursor.advance(line_height_mm + 2.0);
            }
            "paragraph" => {
                let text = element.get("text").and_then(|v| v.as_str()).unwrap_or("");

                let line_height_mm = BODY_FONT_SIZE * LINE_SPACING * 0.3528;
                let chars_per_line = (CONTENT_WIDTH / (BODY_FONT_SIZE * 0.2116)) as usize;

                let wrapped = wrap_text(text, chars_per_line);
                for line in &wrapped {
                    if cursor.needs_new_page(line_height_mm) {
                        flush_page(&mut ops, &mut pages, &mut cursor);
                    }
                    emit_text(
                        &mut ops,
                        line,
                        BODY_FONT_SIZE,
                        &font_regular,
                        MARGIN_LEFT,
                        cursor.y,
                    );
                    cursor.advance(line_height_mm);
                }
                cursor.advance(2.0);
            }
            "table" => {
                let headers = element
                    .get("headers")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(String::from)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                let rows = element
                    .get("rows")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|row| {
                                row.as_array().map(|cells| {
                                    cells
                                        .iter()
                                        .map(|c| c.as_str().unwrap_or("").to_string())
                                        .collect::<Vec<_>>()
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                let num_cols = headers
                    .len()
                    .max(rows.iter().map(std::vec::Vec::len).max().unwrap_or(0))
                    .max(1);
                let col_width = CONTENT_WIDTH / num_cols as f32;
                let row_height_mm = (BODY_FONT_SIZE * LINE_SPACING).mul_add(0.3528, 2.0);

                // Header row
                if !headers.is_empty() {
                    if cursor.needs_new_page(row_height_mm * 2.0) {
                        flush_page(&mut ops, &mut pages, &mut cursor);
                    }

                    emit_hline(
                        &mut ops,
                        MARGIN_LEFT,
                        MARGIN_LEFT + CONTENT_WIDTH,
                        cursor.y + 1.0,
                    );

                    for (i, header) in headers.iter().enumerate() {
                        let x = (i as f32).mul_add(col_width, MARGIN_LEFT) + 1.0;
                        let truncated = truncate_cell(header, col_width);
                        emit_text(
                            &mut ops,
                            &truncated,
                            BODY_FONT_SIZE,
                            &font_bold,
                            x,
                            cursor.y - 3.0,
                        );
                    }
                    cursor.advance(row_height_mm);
                    emit_hline(
                        &mut ops,
                        MARGIN_LEFT,
                        MARGIN_LEFT + CONTENT_WIDTH,
                        cursor.y + 1.0,
                    );
                }

                // Data rows
                for row in &rows {
                    if cursor.needs_new_page(row_height_mm) {
                        flush_page(&mut ops, &mut pages, &mut cursor);
                    }

                    for (i, cell) in row.iter().enumerate() {
                        let x = (i as f32).mul_add(col_width, MARGIN_LEFT) + 1.0;
                        let truncated = truncate_cell(cell, col_width);
                        emit_text(
                            &mut ops,
                            &truncated,
                            BODY_FONT_SIZE,
                            &font_regular,
                            x,
                            cursor.y - 3.0,
                        );
                    }
                    cursor.advance(row_height_mm);
                }

                // Bottom border
                emit_hline(
                    &mut ops,
                    MARGIN_LEFT,
                    MARGIN_LEFT + CONTENT_WIDTH,
                    cursor.y + 1.0,
                );
                cursor.advance(3.0);
            }
            "image" => {
                let image_path_str = element
                    .get("image_path")
                    .and_then(|v| v.as_str())
                    .context("Image element requires 'image_path'")?;

                let image_path = if Path::new(image_path_str).is_absolute() {
                    std::path::PathBuf::from(image_path_str)
                } else {
                    working_dir.join(image_path_str)
                };

                if !image_path.exists() {
                    bail!("Image file not found: {}", image_path.display());
                }

                let image_bytes = fs::read(&image_path)
                    .with_context(|| format!("Failed to read image: {}", image_path.display()))?;

                let mut warnings: Vec<PdfWarnMsg> = Vec::new();
                let raw_image = RawImage::decode_from_bytes(&image_bytes, &mut warnings)
                    .map_err(|e| anyhow::anyhow!("Failed to decode image: {e}"))?;

                let img_width_px = raw_image.width as f32;
                let img_height_px = raw_image.height as f32;

                let max_width = element
                    .get("width_mm")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(f64::from(CONTENT_WIDTH)) as f32;
                let display_width = max_width.min(CONTENT_WIDTH);
                let aspect = img_height_px / img_width_px;
                let display_height = display_width * aspect;

                let total_needed = display_height + 8.0;
                if cursor.needs_new_page(total_needed) {
                    flush_page(&mut ops, &mut pages, &mut cursor);
                }

                let available = cursor.y - MARGIN_BOTTOM;
                let (final_w, final_h) = if display_height > available {
                    let scale = available / display_height;
                    (display_width * scale, available)
                } else {
                    (display_width, display_height)
                };

                let image_y = cursor.y - final_h;

                let xobj_id = doc.add_image(&raw_image);
                ops.push(Op::UseXobject {
                    id: xobj_id,
                    transform: XObjectTransform {
                        translate_x: Some(Mm(MARGIN_LEFT).into()),
                        translate_y: Some(Mm(image_y).into()),
                        scale_x: Some(final_w / img_width_px),
                        scale_y: Some(final_h / img_height_px),
                        ..Default::default()
                    },
                });

                cursor.advance(final_h + 2.0);

                // Optional caption
                if let Some(caption) = element.get("caption").and_then(|v| v.as_str()) {
                    let cap_height = BODY_FONT_SIZE * 0.3528 * LINE_SPACING;
                    if cursor.needs_new_page(cap_height) {
                        flush_page(&mut ops, &mut pages, &mut cursor);
                    }
                    emit_text(
                        &mut ops,
                        caption,
                        BODY_FONT_SIZE - 1.0,
                        &font_regular,
                        MARGIN_LEFT,
                        cursor.y,
                    );
                    cursor.advance(cap_height + 3.0);
                }
            }
            other => {
                bail!("Unknown element type: '{other}'");
            }
        }
    }

    // Flush the last page
    flush_page(&mut ops, &mut pages, &mut cursor);

    if pages.is_empty() {
        pages.push(PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), Vec::new()));
    }

    doc.pages = pages;

    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let opts = PdfSaveOptions::default();
    let bytes = doc.save(&opts, &mut warnings);

    fs::write(path, &bytes).with_context(|| format!("Failed to write PDF: {}", path.display()))?;

    Ok(bytes.len())
}

/// Emit a text string at given position (mm coordinates).
fn emit_text(ops: &mut Vec<Op>, text: &str, font_size: f32, font: &PdfFontHandle, x: f32, y: f32) {
    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: font.clone(),
        size: Pt(font_size),
    });
    ops.push(Op::SetTextCursor {
        pos: Point::new(Mm(x), Mm(y)),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(text.to_string())],
    });
    ops.push(Op::EndTextSection);
}

/// Emit a horizontal line for table borders.
fn emit_hline(ops: &mut Vec<Op>, x1: f32, x2: f32, y: f32) {
    ops.push(Op::SetOutlineColor {
        col: Color::Greyscale(Greyscale::new(0.7, None)),
    });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });
    ops.push(Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(x1), Mm(y)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(x2), Mm(y)),
                    bezier: false,
                },
            ],
            is_closed: false,
        },
    });
}

/// Simple word-wrap for plain text.
fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        let mut current_line = String::new();
        for word in words {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= max_chars {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    lines
}

/// Truncate a cell value to fit within a column width.
fn truncate_cell(text: &str, col_width_mm: f32) -> String {
    let max_chars = (col_width_mm / (BODY_FONT_SIZE * 0.2116)) as usize;
    if text.len() <= max_chars {
        text.to_string()
    } else if max_chars > 3 {
        let mut end = max_chars - 1;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &text[..end])
    } else {
        text.chars().take(max_chars).collect()
    }
}

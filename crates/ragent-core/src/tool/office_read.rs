//! Office document reading tool.
//!
//! Provides [`OfficeReadTool`], which reads content from Microsoft Word (`.docx`),
//! Excel (`.xlsx`), and PowerPoint (`.pptx`) files and returns structured text
//! that an LLM can reason about.
//!
//! Depends on: `docx-rust`, `calamine`, `ooxmlsdk`.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::Path;

use super::office_common::{OfficeFormat, detect_format, resolve_path, truncate_output};
use super::{Tool, ToolContext, ToolOutput};

/// Reads content from Word, Excel, or PowerPoint files.
///
/// Supports output in `text`, `markdown` (default), or `json` format.
/// Excel supports optional sheet and range selection. PowerPoint supports
/// optional slide number selection.
pub struct OfficeReadTool;

#[async_trait::async_trait]
impl Tool for OfficeReadTool {
    fn name(&self) -> &str {
        "office_read"
    }

    fn description(&self) -> &str {
        "Read content from Word (.docx), Excel (.xlsx), or PowerPoint (.pptx) files."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the Office document to read"
                },
                "sheet": {
                    "type": "string",
                    "description": "For Excel: sheet name or index (default: first sheet)"
                },
                "range": {
                    "type": "string",
                    "description": "For Excel: cell range e.g. 'A1:D10' (default: all data)"
                },
                "slide": {
                    "type": "integer",
                    "description": "For PowerPoint: specific slide number (default: all slides)"
                },
                "format": {
                    "type": "string",
                    "enum": ["text", "markdown", "json"],
                    "description": "Output format (default: markdown)"
                }
            },
            "required": ["path"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"].as_str().context("Missing 'path' parameter")?;
        let path = resolve_path(&ctx.working_dir, path_str);
        let format = input["format"].as_str().unwrap_or("markdown").to_string();

        let office_format = detect_format(&path)?;

        let sheet = input["sheet"].as_str().map(String::from);
        let range = input["range"].as_str().map(String::from);
        let slide = input["slide"].as_u64().map(|n| n as usize);
        let path_display = path_str.to_string();

        let content = tokio::task::spawn_blocking(move || -> Result<String> {
            match office_format {
                OfficeFormat::Docx => read_docx(&path, &format),
                OfficeFormat::Xlsx => read_xlsx(&path, sheet.as_deref(), range.as_deref(), &format),
                OfficeFormat::Pptx => read_pptx(&path, slide, &format),
            }
        })
        .await
        .context("Task join error")??;

        let content = truncate_output(content);

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "path": path_display,
                "format": office_format.to_string(),
            })),
        })
    }
}

/// Reads a Word document and returns its content as structured text.
///
/// # Arguments
///
/// * `path` - Path to the `.docx` file.
/// * `format` - Output format: `"text"`, `"markdown"`, or `"json"`.
///
/// # Returns
///
/// The document content as a formatted string.
fn read_docx(path: &Path, format: &str) -> Result<String> {
    let docx_file = docx_rust::DocxFile::from_file(path)
        .map_err(|e| anyhow::anyhow!("Failed to open docx: {e}"))?;
    let docx = docx_file
        .parse()
        .map_err(|e| anyhow::anyhow!("Failed to parse docx: {e}"))?;

    let mut paragraphs: Vec<(String, String)> = Vec::new();

    for content in &docx.document.body.content {
        match content {
            docx_rust::document::BodyContent::Paragraph(para) => {
                let text = para.text();
                if text.is_empty() {
                    continue;
                }
                let style = para
                    .property
                    .as_ref()
                    .and_then(|p| p.style_id.as_ref())
                    .map(|s| s.value.to_string())
                    .unwrap_or_else(|| "Normal".to_string());
                paragraphs.push((style, text));
            }
            docx_rust::document::BodyContent::Table(table) => {
                let table_text = extract_table_markdown(table);
                if !table_text.is_empty() {
                    paragraphs.push(("Table".to_string(), table_text));
                }
            }
            _ => {}
        }
    }

    match format {
        "json" => {
            let json_paras: Vec<Value> = paragraphs
                .iter()
                .map(|(style, text)| json!({"style": style, "text": text}))
                .collect();
            serde_json::to_string_pretty(&json!({"paragraphs": json_paras}))
                .context("Failed to serialize to JSON")
        }
        "text" => {
            let lines: Vec<String> = paragraphs.iter().map(|(_, text)| text.clone()).collect();
            Ok(lines.join("\n\n"))
        }
        _ => {
            let mut output = String::new();
            for (style, text) in &paragraphs {
                let line = style_to_markdown(style, text);
                output.push_str(&line);
                output.push_str("\n\n");
            }
            Ok(output.trim_end().to_string())
        }
    }
}

/// Converts a Word style name to markdown-formatted text.
///
/// # Arguments
///
/// * `style` - The Word paragraph style name (e.g., "Heading1", "ListBullet").
/// * `text` - The paragraph text content.
///
/// # Returns
///
/// The text with appropriate markdown formatting applied.
fn style_to_markdown(style: &str, text: &str) -> String {
    match style {
        "Heading1" | "heading 1" => format!("# {text}"),
        "Heading2" | "heading 2" => format!("## {text}"),
        "Heading3" | "heading 3" => format!("### {text}"),
        "Heading4" | "heading 4" => format!("#### {text}"),
        "Heading5" | "heading 5" => format!("##### {text}"),
        "Heading6" | "heading 6" => format!("###### {text}"),
        "ListBullet" | "ListBullet1" => format!("- {text}"),
        "ListNumber" | "ListNumber1" => format!("1. {text}"),
        "Code" => format!("```\n{text}\n```"),
        "Table" => text.to_string(),
        _ => text.to_string(),
    }
}

/// Extracts a table from a Word document as a markdown table.
///
/// # Arguments
///
/// * `table` - The docx-rust Table reference.
///
/// # Returns
///
/// A markdown-formatted table string.
fn extract_table_markdown(table: &docx_rust::document::Table<'_>) -> String {
    let mut rows_text: Vec<Vec<String>> = Vec::new();

    for row in &table.rows {
        let mut cells: Vec<String> = Vec::new();
        for content in &row.cells {
            match content {
                docx_rust::document::TableRowContent::TableCell(cell) => {
                    let cell_text: Vec<String> = cell
                        .content
                        .iter()
                        .filter_map(|c| match c {
                            docx_rust::document::TableCellContent::Paragraph(p) => {
                                let t = p.text();
                                if t.is_empty() { None } else { Some(t) }
                            }
                        })
                        .collect();
                    cells.push(cell_text.join(" "));
                }
                _ => cells.push(String::new()),
            }
        }
        rows_text.push(cells);
    }

    if rows_text.is_empty() {
        return String::new();
    }

    let col_count = rows_text.iter().map(|r| r.len()).max().unwrap_or(0);
    for row in &mut rows_text {
        while row.len() < col_count {
            row.push(String::new());
        }
    }

    let mut output = String::new();
    if let Some(header) = rows_text.first() {
        output.push_str("| ");
        output.push_str(&header.join(" | "));
        output.push_str(" |\n|");
        for _ in 0..col_count {
            output.push_str("---|");
        }
        output.push('\n');
    }
    for row in rows_text.iter().skip(1) {
        output.push_str("| ");
        output.push_str(&row.join(" | "));
        output.push_str(" |\n");
    }

    output.trim_end().to_string()
}

/// Reads an Excel spreadsheet and returns its content.
///
/// # Arguments
///
/// * `path` - Path to the `.xlsx` file.
/// * `sheet` - Optional sheet name or index.
/// * `range_str` - Optional cell range (e.g., "A1:D10").
/// * `format` - Output format: `"text"`, `"markdown"`, or `"json"`.
///
/// # Returns
///
/// The spreadsheet content as a formatted string.
fn read_xlsx(
    path: &Path,
    sheet: Option<&str>,
    range_str: Option<&str>,
    format: &str,
) -> Result<String> {
    use calamine::{Reader, Xlsx};

    let mut workbook: Xlsx<_> =
        calamine::open_workbook(path).map_err(|e| anyhow::anyhow!("Failed to open xlsx: {e}"))?;

    let sheet_names = workbook.sheet_names().to_owned();
    if sheet_names.is_empty() {
        bail!("Workbook contains no sheets");
    }

    let sheet_name = match sheet {
        Some(s) => {
            if let Ok(idx) = s.parse::<usize>() {
                sheet_names.get(idx).cloned().with_context(|| {
                    format!("Sheet index {idx} out of range (0..{})", sheet_names.len())
                })?
            } else if sheet_names.contains(&s.to_string()) {
                s.to_string()
            } else {
                bail!(
                    "Sheet '{s}' not found. Available sheets: {}",
                    sheet_names.join(", ")
                );
            }
        }
        None => sheet_names[0].clone(),
    };

    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|e| anyhow::anyhow!("Failed to read sheet '{sheet_name}': {e}"))?;

    let (total_rows, total_cols) = range.get_size();

    let (start_row, start_col, end_row, end_col) = if let Some(r) = range_str {
        parse_cell_range(r, total_rows, total_cols)?
    } else {
        (0, 0, total_rows, total_cols)
    };

    let mut rows_data: Vec<Vec<String>> = Vec::new();
    for row_idx in start_row..end_row {
        let mut row_cells: Vec<String> = Vec::new();
        for col_idx in start_col..end_col {
            let cell_value = range
                .get((row_idx, col_idx))
                .map(|d| format_cell_value(d))
                .unwrap_or_default();
            row_cells.push(cell_value);
        }
        rows_data.push(row_cells);
    }

    let num_rows = rows_data.len();
    let num_cols = if rows_data.is_empty() {
        0
    } else {
        rows_data[0].len()
    };

    match format {
        "json" => {
            let json_rows: Vec<Vec<String>> = rows_data;
            serde_json::to_string_pretty(&json!({
                "sheet": sheet_name,
                "rows": num_rows,
                "columns": num_cols,
                "data": json_rows,
            }))
            .context("Failed to serialize to JSON")
        }
        "text" => {
            let mut output =
                format!("Sheet: {sheet_name} ({num_rows} rows × {num_cols} columns)\n\n");
            for row in &rows_data {
                output.push_str(&row.join("\t"));
                output.push('\n');
            }
            Ok(output.trim_end().to_string())
        }
        _ => {
            let mut output =
                format!("Sheet: {sheet_name} ({num_rows} rows × {num_cols} columns)\n\n");
            if rows_data.is_empty() {
                output.push_str("(empty)");
                return Ok(output);
            }

            if let Some(header) = rows_data.first() {
                output.push_str("| ");
                output.push_str(&header.join(" | "));
                output.push_str(" |\n|");
                for _ in 0..num_cols {
                    output.push_str("---|");
                }
                output.push('\n');
            }
            for row in rows_data.iter().skip(1) {
                output.push_str("| ");
                output.push_str(&row.join(" | "));
                output.push_str(" |\n");
            }

            Ok(output.trim_end().to_string())
        }
    }
}

/// Formats a calamine cell value as a display string.
///
/// # Arguments
///
/// * `data` - The calamine cell data value.
///
/// # Returns
///
/// A human-readable string representation of the cell value.
fn format_cell_value(data: &calamine::Data) -> String {
    use calamine::Data;
    match data {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            if (*f - f.round()).abs() < f64::EPSILON {
                format!("{}", *f as i64)
            } else {
                format!("{f}")
            }
        }
        Data::Int(i) => format!("{i}"),
        Data::Bool(b) => format!("{b}"),
        Data::Error(e) => format!("#{e:?}"),
        Data::DateTime(dt) => format!("{dt}"),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
    }
}

/// Parses an Excel-style cell range string (e.g., "A1:D10") into row/column indices.
///
/// # Arguments
///
/// * `range_str` - The range string to parse.
/// * `max_rows` - Maximum rows in the sheet (for clamping).
/// * `max_cols` - Maximum columns in the sheet (for clamping).
///
/// # Returns
///
/// A tuple of `(start_row, start_col, end_row, end_col)` as 0-based indices.
fn parse_cell_range(
    range_str: &str,
    max_rows: usize,
    max_cols: usize,
) -> Result<(usize, usize, usize, usize)> {
    let parts: Vec<&str> = range_str.split(':').collect();
    if parts.len() != 2 {
        bail!("Invalid range format '{range_str}'. Expected format: 'A1:D10'");
    }

    let (start_col, start_row) = parse_cell_ref(parts[0])?;
    let (end_col, end_row) = parse_cell_ref(parts[1])?;

    Ok((
        start_row.min(max_rows),
        start_col.min(max_cols),
        (end_row + 1).min(max_rows),
        (end_col + 1).min(max_cols),
    ))
}

/// Parses a single cell reference (e.g., "A1") into column and row indices.
///
/// # Arguments
///
/// * `cell_ref` - The cell reference string.
///
/// # Returns
///
/// A tuple of `(column_index, row_index)` as 0-based values.
fn parse_cell_ref(cell_ref: &str) -> Result<(usize, usize)> {
    let cell_ref = cell_ref.trim().to_uppercase();
    let col_end = cell_ref
        .find(|c: char| c.is_ascii_digit())
        .with_context(|| format!("Invalid cell reference: '{cell_ref}'"))?;

    let col_str = &cell_ref[..col_end];
    let row_str = &cell_ref[col_end..];

    let col: usize = col_str
        .chars()
        .fold(0, |acc, c| acc * 26 + (c as usize - 'A' as usize + 1))
        .checked_sub(1)
        .with_context(|| format!("Invalid column in cell reference: '{cell_ref}'"))?;

    let row: usize = row_str
        .parse::<usize>()
        .with_context(|| format!("Invalid row in cell reference: '{cell_ref}'"))?
        .checked_sub(1)
        .with_context(|| format!("Row number must be >= 1 in cell reference: '{cell_ref}'"))?;

    Ok((col, row))
}

/// Reads a PowerPoint presentation and returns its content.
///
/// # Arguments
///
/// * `path` - Path to the `.pptx` file.
/// * `slide_num` - Optional 1-based slide number to read (None = all slides).
/// * `format` - Output format: `"text"`, `"markdown"`, or `"json"`.
///
/// # Returns
///
/// The presentation content as a formatted string.
fn read_pptx(path: &Path, slide_num: Option<usize>, format: &str) -> Result<String> {
    use ooxmlsdk::parts::presentation_document::PresentationDocument;

    let doc = PresentationDocument::new_from_file(path)
        .map_err(|e| anyhow::anyhow!("Failed to open pptx: {e}"))?;

    let slide_parts = &doc.presentation_part.slide_parts;
    let total_slides = slide_parts.len();

    let slides_to_read: Vec<(usize, &ooxmlsdk::parts::slide_part::SlidePart)> = match slide_num {
        Some(n) => {
            if n == 0 || n > total_slides {
                bail!("Slide {n} out of range (1..{total_slides})");
            }
            vec![(n, &slide_parts[n - 1])]
        }
        None => slide_parts
            .iter()
            .enumerate()
            .map(|(i, s)| (i + 1, s))
            .collect(),
    };

    let mut slide_data: Vec<(usize, String, String, String)> = Vec::new();

    for (num, slide_part) in &slides_to_read {
        let (title, body) = extract_slide_text(&slide_part.root_element);
        let notes = slide_part
            .notes_slide_part
            .as_ref()
            .map(|ns| extract_notes_text(&ns.root_element))
            .unwrap_or_default();
        slide_data.push((*num, title, body, notes));
    }

    match format {
        "json" => {
            let json_slides: Vec<Value> = slide_data
                .iter()
                .map(|(num, title, body, notes)| {
                    json!({
                        "slide": num,
                        "title": title,
                        "body": body,
                        "notes": notes,
                    })
                })
                .collect();
            serde_json::to_string_pretty(&json!({
                "total_slides": total_slides,
                "slides": json_slides,
            }))
            .context("Failed to serialize to JSON")
        }
        "text" => {
            let mut output = format!("Presentation: {total_slides} slides\n\n");
            for (num, title, body, notes) in &slide_data {
                output.push_str(&format!("--- Slide {num} ---\n"));
                if !title.is_empty() {
                    output.push_str(&format!("{title}\n"));
                }
                if !body.is_empty() {
                    output.push_str(&format!("{body}\n"));
                }
                if !notes.is_empty() {
                    output.push_str(&format!("Notes: {notes}\n"));
                }
                output.push('\n');
            }
            Ok(output.trim_end().to_string())
        }
        _ => {
            let mut output = format!("Presentation: {total_slides} slides\n\n");
            for (num, title, body, notes) in &slide_data {
                output.push_str(&format!("## Slide {num}"));
                if !title.is_empty() {
                    output.push_str(&format!(": {title}"));
                }
                output.push_str("\n\n");
                if !body.is_empty() {
                    output.push_str(body);
                    output.push_str("\n\n");
                }
                if !notes.is_empty() {
                    output.push_str(&format!("> **Notes:** {notes}\n\n"));
                }
            }
            Ok(output.trim_end().to_string())
        }
    }
}

/// Extracts title and body text from a PowerPoint slide.
///
/// # Arguments
///
/// * `slide` - The ooxmlsdk Slide reference.
///
/// # Returns
///
/// A tuple of `(title, body)` strings.
fn extract_slide_text(
    slide: &ooxmlsdk::schemas::schemas_openxmlformats_org_presentationml_2006_main::Slide,
) -> (String, String) {
    use ooxmlsdk::schemas::schemas_openxmlformats_org_presentationml_2006_main::*;

    let mut title = String::new();
    let mut body_parts: Vec<String> = Vec::new();

    for child in &slide.children {
        if let SlideChildChoice::PCSld(csd) = child {
            for shape_child in &csd.shape_tree.children {
                if let ShapeTreeChildChoice::PSp(shape) = shape_child {
                    if let Some(text_body) = &shape.text_body {
                        let text = extract_text_body_text(text_body);
                        if !text.is_empty() {
                            if title.is_empty() && is_title_shape(shape) {
                                title = text;
                            } else {
                                body_parts.push(text);
                            }
                        }
                    }
                }
            }
        }
    }

    (title, body_parts.join("\n"))
}

/// Checks if a shape is a title placeholder.
///
/// # Arguments
///
/// * `shape` - The Shape reference.
///
/// # Returns
///
/// `true` if the shape appears to be a title placeholder.
fn is_title_shape(
    shape: &ooxmlsdk::schemas::schemas_openxmlformats_org_presentationml_2006_main::Shape,
) -> bool {
    use ooxmlsdk::schemas::schemas_openxmlformats_org_presentationml_2006_main::*;

    let nvsp = &shape.non_visual_shape_properties;
    let app_props = &nvsp.application_non_visual_drawing_properties;
    for child in &app_props.children {
        if let ApplicationNonVisualDrawingPropertiesChildChoice::PPh(ph) = child {
            if let Some(ref ph_type) = ph.r#type {
                return matches!(
                    ph_type,
                    PlaceholderValues::Title | PlaceholderValues::CenteredTitle
                );
            }
            return true;
        }
    }
    false
}

/// Extracts plain text from a PowerPoint text body.
///
/// # Arguments
///
/// * `text_body` - The TextBody reference.
///
/// # Returns
///
/// The concatenated text content.
fn extract_text_body_text(
    text_body: &ooxmlsdk::schemas::schemas_openxmlformats_org_presentationml_2006_main::TextBody,
) -> String {
    use ooxmlsdk::schemas::schemas_openxmlformats_org_drawingml_2006_main::ParagraphChildChoice;

    let mut paragraphs: Vec<String> = Vec::new();

    for para in &text_body.a_p {
        let mut para_text = String::new();
        for child in &para.children {
            if let ParagraphChildChoice::AR(run) = child {
                if let Some(ref content) = run.text.xml_content {
                    para_text.push_str(content);
                }
            }
        }
        if !para_text.is_empty() {
            paragraphs.push(para_text);
        }
    }

    paragraphs.join("\n")
}

/// Extracts speaker notes text from a Notes slide.
///
/// # Arguments
///
/// * `notes_slide` - The NotesSlide reference.
///
/// # Returns
///
/// The notes text content.
fn extract_notes_text(
    notes_slide: &ooxmlsdk::schemas::schemas_openxmlformats_org_presentationml_2006_main::NotesSlide,
) -> String {
    use ooxmlsdk::schemas::schemas_openxmlformats_org_drawingml_2006_main::ParagraphChildChoice;
    use ooxmlsdk::schemas::schemas_openxmlformats_org_presentationml_2006_main::*;

    let mut notes_parts: Vec<String> = Vec::new();

    for child in &notes_slide.children {
        if let NotesSlideChildChoice::PCSld(csd) = child {
            for shape_child in &csd.shape_tree.children {
                if let ShapeTreeChildChoice::PSp(shape) = shape_child {
                    if let Some(text_body) = &shape.text_body {
                        for para in &text_body.a_p {
                            let mut para_text = String::new();
                            for p_child in &para.children {
                                if let ParagraphChildChoice::AR(run) = p_child {
                                    if let Some(ref content) = run.text.xml_content {
                                        para_text.push_str(content);
                                    }
                                }
                            }
                            if !para_text.is_empty() {
                                notes_parts.push(para_text);
                            }
                        }
                    }
                }
            }
        }
    }

    notes_parts.join(" ")
}

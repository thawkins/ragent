//! LibreOffice document metadata/info tool.
//!
//! Provides [`LibreInfoTool`], which extracts metadata and structural
//! information from OpenDocument Text (`.odt`), Spreadsheet (`.ods`), and
//! Presentation (`.odp`) files.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::Path;

use super::libreoffice_common::{LibreFormat, detect_format, resolve_path};
use super::{Tool, ToolContext, ToolOutput};

/// Extracts metadata and structural information from OpenDocument files.
///
/// Returns file type, sheet/slide/paragraph counts, author, title, file size, and
/// lightweight lists (sheet names, slide snippets).
pub struct LibreInfoTool;

#[async_trait::async_trait]
impl Tool for LibreInfoTool {
    fn name(&self) -> &str {
        "libre_info"
    }

    fn description(&self) -> &str {
        "Get metadata and structural information about an OpenDocument (odt/ods/odp) file."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the OpenDocument file" }
            },
            "required": ["path"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"].as_str().context("Missing required 'path' parameter")?;
        let path = resolve_path(&ctx.working_dir, path_str);
        let libre_format = detect_format(&path)?;

        let file_size = tokio::fs::metadata(&path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        let path_clone = path.clone();
        let (content, metadata) = tokio::task::spawn_blocking(move || -> Result<(String, Value)> {
            match libre_format {
                LibreFormat::Odt => info_odt(&path_clone, file_size),
                LibreFormat::Ods => info_ods(&path_clone, file_size),
                LibreFormat::Odp => info_odp(&path_clone, file_size),
            }
        })
        .await
        .context("Failed to process document: background task exited")??;

        Ok(ToolOutput { content, metadata: Some(metadata) })
    }
}

fn info_odt(path: &Path, file_size: u64) -> Result<(String, Value)> {
    use zip::ZipArchive;
    use std::fs::File;
    use std::io::Read;

    let file = File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
    let mut zip = ZipArchive::new(file).context("Failed to read ODT ZIP archive")?;

    let mut content_xml = String::new();
    if let Ok(mut f) = zip.by_name("content.xml") {
        f.read_to_string(&mut content_xml).context("Failed to read content.xml")?;
    }

    let mut meta_xml = String::new();
    if let Ok(mut f) = zip.by_name("meta.xml") {
        f.read_to_string(&mut meta_xml).ok();
    }

    // Count paragraphs and words by naive tag scanning
    let paragraph_count = count_tag_occurrences(&content_xml, "<text:p") as usize;
    let word_count = count_words_in_text(&extract_text_from_xml(&content_xml));

    // Try extract title/author from meta.xml
    let title = extract_tag_text(&meta_xml, "dc:title").unwrap_or_default();
    let author = extract_tag_text(&meta_xml, "dc:creator").unwrap_or_default();

    let metadata = json!({
        "format": "odt",
        "file_size_bytes": file_size,
        "paragraph_count": paragraph_count,
        "word_count": word_count,
        "title": title,
        "author": author,
    });

    let content = format!(
        "Format: OpenDocument Text (.odt)\nFile size: {} bytes\nTitle: {}\nAuthor: {}\nParagraphs: {}\nWord count: {}",
        file_size,
        if metadata["title"].as_str().unwrap_or("").is_empty() { "(none)" } else { metadata["title"].as_str().unwrap_or("") },
        if metadata["author"].as_str().unwrap_or("").is_empty() { "(none)" } else { metadata["author"].as_str().unwrap_or("") },
        paragraph_count,
        word_count,
    );

    Ok((content, metadata))
}

fn info_ods(path: &Path, file_size: u64) -> Result<(String, Value)> {
    use zip::ZipArchive;
    use std::fs::File;
    use std::io::Read;

    let file = File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
    let mut zip = ZipArchive::new(file).context("Failed to read ODS ZIP archive")?;

    let mut content_xml = String::new();
    zip.by_name("content.xml")?.read_to_string(&mut content_xml).context("Failed to read content.xml")?;

    // Extract simple table info
    let tables = extract_tables_info_from_ods(&content_xml);
    let sheet_count = tables.len();

    let mut sheets_info: Vec<Value> = Vec::new();
    for t in &tables {
        sheets_info.push(json!({ "name": t.name, "rows": t.rows as u64, "columns": t.cols as u64 }));
    }

    let metadata = json!({
        "format": "ods",
        "file_size_bytes": file_size,
        "sheet_count": sheet_count,
        "sheets": sheets_info,
    });

    let mut content_lines = vec![format!("Format: OpenDocument Spreadsheet (.ods)"), format!("File size: {} bytes", file_size), format!("Sheets: {}", sheet_count)];
    for s in &tables {
        content_lines.push(format!("  - {}: {} rows × {} columns", s.name, s.rows, s.cols));
    }

    Ok((content_lines.join("\n"), metadata))
}

fn info_odp(path: &Path, file_size: u64) -> Result<(String, Value)> {
    use zip::ZipArchive;
    use std::fs::File;
    use std::io::Read;

    let file = File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
    let mut zip = ZipArchive::new(file).context("Failed to read ODP ZIP archive")?;

    let mut content_xml = String::new();
    zip.by_name("content.xml")?.read_to_string(&mut content_xml).context("Failed to read content.xml")?;

    let slides = extract_slides_from_odp(&content_xml);
    let slide_count = slides.len();

    let mut titles: Vec<String> = Vec::new();
    for s in &slides {
        let t = s.lines().next().unwrap_or("").trim().to_string();
        titles.push(if t.is_empty() { "(none)".to_string() } else { t });
    }

    let metadata = json!({
        "format": "odp",
        "file_size_bytes": file_size,
        "slide_count": slide_count,
        "slides": titles,
    });

    let mut content_lines = vec![format!("Format: OpenDocument Presentation (.odp)"), format!("File size: {} bytes", file_size), format!("Slides: {}", slide_count)];
    for (i, t) in titles.iter().enumerate() {
        content_lines.push(format!("  - Slide {}: {}", i + 1, t));
    }

    Ok((content_lines.join("\n"), metadata))
}

// --- Helpers ---

fn extract_text_from_xml(xml: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in xml.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            '&' => out.push(' '),
            _ => if !in_tag { out.push(c) }
        }
    }
    out
}

fn count_tag_occurrences(hay: &str, tag: &str) -> usize {
    hay.to_lowercase().matches(&tag.to_lowercase()).count()
}

fn count_words_in_text(text: &str) -> usize {
    text.split_whitespace().count()
}

fn extract_tag_text(xml: &str, tag: &str) -> Option<String> {
    let lower = xml.to_lowercase();
    let tag_lower = tag.to_lowercase();
    if let Some(start) = lower.find(&format!("<{}", tag_lower)) {
        if let Some(gt) = xml[start..].find('>') {
            let s = start + gt + 1;
            if let Some(end) = xml[s..].find(&format!("</{}>", tag)) {
                return Some(xml[s..s + end].trim().to_string());
            }
        }
    }
    None
}

#[derive(Debug, Clone)]
struct SimpleTableInfo {
    name: String,
    rows: usize,
    cols: usize,
}

fn extract_tables_info_from_ods(xml: &str) -> Vec<SimpleTableInfo> {
    let mut tables: Vec<SimpleTableInfo> = Vec::new();
    let mut pos = 0;
    let lower = xml.to_lowercase();
    while let Some(start) = lower[pos..].find("<table:table") {
        let s = pos + start;
        if let Some(end_tag) = lower[s..].find("</table:table>") {
            let e = s + end_tag + "</table:table>".len();
            let table_xml = &xml[s..e];
            let name = extract_attribute(table_xml, "table:name").unwrap_or_else(|| "Sheet1".to_string());
            let rows = extract_rows_from_table(table_xml).len();
            let cols = extract_rows_from_table(table_xml).get(0).map(|r| r.len()).unwrap_or(0);
            tables.push(SimpleTableInfo { name, rows, cols });
            pos = e;
        } else { break; }
    }
    tables
}

fn extract_rows_from_table(table_xml: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let lower = table_xml.to_lowercase();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find("<table:table-row") {
        let s = pos + start;
        if let Some(end_tag) = lower[s..].find("</table:table-row>") {
            let e = s + end_tag + "</table:table-row>".len();
            let row_xml = &table_xml[s..e];
            let cells = extract_cells_from_row(row_xml);
            rows.push(cells);
            pos = e;
        } else { break; }
    }
    rows
}

fn extract_cells_from_row(row_xml: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let lower = row_xml.to_lowercase();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find("<table:table-cell") {
        let s = pos + start;
        if let Some(end_tag) = lower[s..].find("</table:table-cell>") {
            let e = s + end_tag + "</table:table-cell>".len();
            let cell_xml = &row_xml[s..e];
            let value = extract_text_from_xml(cell_xml).trim().to_string();
            cells.push(value);
            pos = e;
        } else { break; }
    }
    cells
}

fn extract_slides_from_odp(xml: &str) -> Vec<String> {
    let mut slides = Vec::new();
    let lower = xml.to_lowercase();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find("<draw:page") {
        let s = pos + start;
        if let Some(end_tag) = lower[s..].find("</draw:page>") {
            let e = s + end_tag + "</draw:page>".len();
            let slide_xml = &xml[s..e];
            let text = extract_text_from_xml(slide_xml).trim().to_string();
            slides.push(text);
            pos = e;
        } else { break; }
    }
    slides
}

fn extract_attribute(xml: &str, attr: &str) -> Option<String> {
    let lower = xml.to_lowercase();
    let attr_lower = attr.to_lowercase();
    if let Some(idx) = lower.find(&attr_lower) {
        let rest = &xml[idx + attr.len()..];
        if let Some(eq) = rest.find('=') {
            let val = rest[eq + 1..].trim();
            let quoted = val.chars().next()?;
            if quoted == '"' || quoted == '\'' {
                if let Some(end) = val[1..].find(quoted) {
                    return Some(val[1..1 + end].to_string());
                }
            }
        }
    }
    None
}

//! LibreOffice document reading tool.
//!
//! Provides [`LibreReadTool`], which reads content from OpenDocument Text (`.odt`),
//! Spreadsheet (`.ods`), and Presentation (`.odp`) files and returns structured text.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::Path;

use super::libreoffice_common::{LibreFormat, detect_format, resolve_path, truncate_output};
use super::{Tool, ToolContext, ToolOutput};

/// Reads content from ODT, ODS, or ODP files.
///
/// Supports output in `text`, `markdown` (default), or `json` format.
/// Spreadsheet supports optional sheet and range selection. Presentation supports
/// optional slide number selection.
pub struct LibreReadTool;

#[async_trait::async_trait]
impl Tool for LibreReadTool {
    fn name(&self) -> &str {
        "libre_read"
    }

    fn description(&self) -> &str {
        "Read content from OpenDocument files: .odt, .ods, .odp"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the document" },
                "sheet": { "type": "string", "description": "For ODS: sheet name or index (default: first sheet)" },
                "range": { "type": "string", "description": "For ODS: cell range e.g. 'A1:D10' (default: all data)" },
                "slide": { "type": "integer", "description": "For ODP: specific slide number (default: all slides)" },
                "format": { "type": "string", "enum": ["text","markdown","json"], "description": "Output format (default: markdown)" }
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
        let format = input["format"].as_str().unwrap_or("markdown").to_string();

        let libre_format = detect_format(&path)?;

        let sheet = input["sheet"].as_str().map(String::from);
        let range = input["range"].as_str().map(String::from);
        let slide = input["slide"].as_u64().map(|n| n as usize);
        let path_display = path_str.to_string();

        let content = tokio::task::spawn_blocking(move || -> Result<String> {
            match libre_format {
                LibreFormat::Odt => read_odt(&path, &format),
                LibreFormat::Ods => read_ods(&path, sheet.as_deref(), range.as_deref(), &format),
                LibreFormat::Odp => read_odp(&path, slide, &format),
            }
        })
        .await
        .context("Failed to read document: background task exited")??;

        let content = truncate_output(content);

        Ok(ToolOutput {
            content,
            metadata: Some(json!({ "path": path_display, "format": libre_format.to_string() })),
        })
    }
}

fn read_odt(path: &Path, format: &str) -> Result<String> {
    use zip::ZipArchive;
    use std::fs::File;
    use std::io::Read;

    let file = File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
    let mut zip = ZipArchive::new(file).context("Failed to read ODT ZIP archive")?;

    let mut content_xml = String::new();
    zip.by_name("content.xml")?.read_to_string(&mut content_xml).context("Failed to read content.xml")?;

    // Very small and conservative DOM-less extraction: grab text between tags.
    let text = extract_text_from_xml(&content_xml);

    match format {
        "json" => {
            let paras: Vec<Value> = text.lines().map(|l| json!({"text": l})).collect();
            serde_json::to_string_pretty(&json!({"paragraphs": paras})).context("Failed to serialize JSON")
        }
        "text" => Ok(text.replace('\u{a0}', " ")),
        _ => Ok(text),
    }
}

fn read_ods(path: &Path, sheet: Option<&str>, range: Option<&str>, format: &str) -> Result<String> {
    use zip::ZipArchive;
    use std::fs::File;
    use std::io::Read;

    let file = File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
    let mut zip = ZipArchive::new(file).context("Failed to read ODS ZIP archive")?;

    let mut content_xml = String::new();
    zip.by_name("content.xml")?.read_to_string(&mut content_xml).context("Failed to read content.xml")?;

    // Rudimentary table extraction: find <table:table> elements and extract rows/cells.
    let tables = extract_tables_from_ods(&content_xml);

    // choose sheet
    let table = if let Some(s) = sheet {
        if let Ok(idx) = s.parse::<usize>() {
            tables.get(idx).or_else(|| tables.get(0))
        } else {
            tables.iter().find(|t| t.name == s).or_else(|| tables.get(0))
        }
    } else {
        tables.get(0)
    };

    if table.is_none() {
        return Ok(String::new());
    }

    let table = table.unwrap();

    match format {
        "json" => serde_json::to_string_pretty(&json!({"sheet": table.name, "rows": table.rows})).context("Failed to serialize JSON"),
        "text" => {
            let mut out = String::new();
            for r in &table.rows {
                out.push_str(&r.join("\t"));
                out.push('\n');
            }
            Ok(out)
        }
        _ => {
            let mut md = String::new();
            for r in &table.rows {
                md.push_str(&format!("| {} |\n", r.join(" | ")));
            }
            Ok(md)
        }
    }
}

fn read_odp(path: &Path, slide: Option<usize>, format: &str) -> Result<String> {
    use zip::ZipArchive;
    use std::fs::File;
    use std::io::Read;

    let file = File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
    let mut zip = ZipArchive::new(file).context("Failed to read ODP ZIP archive")?;

    // Slides are typically under "content.xml" as <draw:page> elements.
    let mut content_xml = String::new();
    zip.by_name("content.xml")?.read_to_string(&mut content_xml).context("Failed to read content.xml")?;

    let slides = extract_slides_from_odp(&content_xml);

    let selection: Vec<String> = if let Some(n) = slide {
        if n == 0 || n > slides.len() {
            vec![]
        } else {
            vec![slides[n - 1].clone()]
        }
    } else {
        slides
    };

    match format {
        "json" => serde_json::to_string_pretty(&json!({"slides": selection})).context("Failed to serialize JSON"),
        "text" => Ok(selection.join("\n\n")),
        _ => Ok(selection.join("\n\n")),
    }
}

fn extract_text_from_xml(xml: &str) -> String {
    // Very naive: remove tags and decode simple entities.
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

#[derive(Debug, Clone)]
struct OdsTable {
    name: String,
    rows: Vec<Vec<String>>,
}

fn extract_tables_from_ods(xml: &str) -> Vec<OdsTable> {
    let mut tables: Vec<OdsTable> = Vec::new();
    let mut pos = 0;
    let lower = xml.to_lowercase();
    while let Some(start) = lower[pos..].find("<table:table") {
        let s = pos + start;
        if let Some(end_tag) = lower[s..].find("</table:table>") {
            let e = s + end_tag + "</table:table>".len();
            let table_xml = &xml[s..e];
            let name = extract_attribute(table_xml, "table:name").unwrap_or_else(|| "Sheet1".to_string());
            let rows = extract_rows_from_table(table_xml);
            tables.push(OdsTable { name, rows });
            pos = e;
        } else {
            break;
        }
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

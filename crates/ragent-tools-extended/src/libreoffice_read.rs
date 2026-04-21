//! `LibreOffice` document reading tool.
//!
//! Provides [`LibreReadTool`], which reads content from `OpenDocument` Text (`.odt`),
//! Spreadsheet (`.ods`), and Presentation (`.odp`) files.
//!
//! - **ODS**: parsed with `calamine`, which has full native ODS support.
//! - **ODT / ODP**: content.xml extracted from the ZIP archive and parsed with
//!   `quick-xml` for robust, standards-compliant text extraction.

use anyhow::{Context, Result};
use calamine::{Reader as CalaReader, Sheets, open_workbook_auto};
use serde_json::{Value, json};
use std::path::Path;

use super::libreoffice_common::{
    LibreFormat, detect_format, read_zip_entry, resolve_path, truncate_output, xml_to_text,
};
use super::{Tool, ToolContext, ToolOutput};

/// Reads content from ODT, ODS, or ODP files.
pub struct LibreReadTool;

#[async_trait::async_trait]
impl Tool for LibreReadTool {
    fn name(&self) -> &'static str {
        "libre_read"
    }

    /// Returns the tool description.
    fn description(&self) -> &'static str {
        "Read content from OpenDocument files: Writer (.odt), Calc (.ods), Impress (.odp). \
         ODS uses calamine for full spreadsheet fidelity; ODT/ODP use XML extraction."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the OpenDocument file"
                },
                "sheet": {
                    "type": "string",
                    "description": "ODS only: sheet name or 0-based index (default: first sheet)"
                },
                "range": {
                    "type": "string",
                    "description": "ODS only: cell range e.g. 'A1:D10' (default: all data)"
                },
                "slide": {
                    "type": "integer",
                    "description": "ODP only: 1-based slide number (default: all slides)"
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

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    /// Executes the `LibreOffice` read operation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The required `path` parameter is missing
    /// - The file format cannot be detected from the extension
    /// - The background task panics or fails to read the document
    /// - The document cannot be opened or parsed
    /// - For ODS: the sheet name/index is invalid or the range syntax is incorrect
    /// - For ODP: the slide number is out of bounds
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;
        let path = resolve_path(&ctx.working_dir, path_str);
        let fmt = input["format"].as_str().unwrap_or("markdown").to_string();
        let libre_format = detect_format(&path)?;
        let sheet = input["sheet"].as_str().map(String::from);
        let range = input["range"].as_str().map(String::from);
        let slide = input["slide"].as_u64().map(|n| n as usize);
        let path_d = path_str.to_string();

        let content = tokio::task::spawn_blocking(move || -> Result<String> {
            match libre_format {
                LibreFormat::Odt => read_odt(&path, &fmt),
                LibreFormat::Ods => read_ods(&path, sheet.as_deref(), range.as_deref(), &fmt),
                LibreFormat::Odp => read_odp(&path, slide, &fmt),
            }
        })
        .await
        .context("Background task panicked while reading document")??;

        let content = truncate_output(content);
        Ok(ToolOutput {
            content,
            metadata: Some(json!({ "path": path_d, "format": libre_format.to_string() })),
        })
    }
}

// ── ODT ──────────────────────────────────────────────────────────────────────

fn read_odt(path: &Path, fmt: &str) -> Result<String> {
    let xml = read_zip_entry(path, "content.xml")?;
    let text = xml_to_text(&xml);
    match fmt {
        "json" => Ok(serde_json::to_string_pretty(&json!({ "text": text }))?),
        _ => Ok(text),
    }
}

// ── ODS ──────────────────────────────────────────────────────────────────────

fn read_ods(path: &Path, sheet: Option<&str>, range: Option<&str>, fmt: &str) -> Result<String> {
    let mut wb: Sheets<_> = open_workbook_auto(path)
        .with_context(|| format!("calamine failed to open ODS: {}", path.display()))?;

    let sheet_names = wb.sheet_names();
    let target = match sheet {
        Some(name) => {
            if sheet_names.iter().any(|n| n == name) {
                name.to_string()
            } else if let Ok(idx) = name.parse::<usize>() {
                sheet_names
                    .get(idx)
                    .ok_or_else(|| anyhow::anyhow!("Sheet index {idx} out of range"))?
                    .clone()
            } else {
                anyhow::bail!("Sheet '{name}' not found");
            }
        }
        None => sheet_names
            .first()
            .ok_or_else(|| anyhow::anyhow!("Workbook has no sheets"))?
            .clone(),
    };

    let data = wb
        .worksheet_range(&target)
        .with_context(|| format!("Failed to read sheet '{target}'"))?;

    let rows: Vec<Vec<String>> = if let Some(range_str) = range {
        let (r1c1, r2c2) = parse_range(range_str)?;
        data.rows()
            .skip(r1c1.0)
            .take(r2c2.0.saturating_sub(r1c1.0) + 1)
            .map(|row| {
                row.iter()
                    .skip(r1c1.1)
                    .take(r2c2.1.saturating_sub(r1c1.1) + 1)
                    .map(std::string::ToString::to_string)
                    .collect()
            })
            .collect()
    } else {
        data.rows()
            .map(|row| row.iter().map(std::string::ToString::to_string).collect())
            .collect()
    };

    match fmt {
        "json" => Ok(serde_json::to_string_pretty(&json!({
            "sheet": target,
            "rows": rows,
        }))?),
        "text" => Ok(rows
            .iter()
            .map(|r| r.join("\t"))
            .collect::<Vec<_>>()
            .join("\n")),
        _ => {
            if rows.is_empty() {
                return Ok(format!("Sheet **{target}** is empty."));
            }
            let mut md = format!("### Sheet: {target}\n\n");
            if let Some(header) = rows.first() {
                md.push_str("| ");
                md.push_str(&header.join(" | "));
                md.push_str(" |\n");
                md.push_str("| ");
                md.push_str(&header.iter().map(|_| "---").collect::<Vec<_>>().join(" | "));
                md.push_str(" |\n");
                for row in rows.iter().skip(1) {
                    md.push_str("| ");
                    md.push_str(&row.join(" | "));
                    md.push_str(" |\n");
                }
            }
            Ok(md)
        }
    }
}

/// Parse an A1:B2 cell range into (row, col) pairs (0-based).
fn parse_range(s: &str) -> Result<((usize, usize), (usize, usize))> {
    let parts: Vec<&str> = s.split(':').collect();
    anyhow::ensure!(parts.len() == 2, "Range must be in A1:B2 format");
    Ok((cell_ref(parts[0])?, cell_ref(parts[1])?))
}

fn cell_ref(s: &str) -> Result<(usize, usize)> {
    let s = s.trim().to_uppercase();
    let col_end = s.find(|c: char| c.is_ascii_digit()).unwrap_or(s.len());
    let col_str = &s[..col_end];
    let row_str = &s[col_end..];
    let col = col_str
        .chars()
        .fold(0usize, |acc, c| acc * 26 + (c as usize - 'A' as usize + 1));
    let row: usize = row_str.parse().context("Invalid row number in range")?;
    Ok((row.saturating_sub(1), col.saturating_sub(1)))
}

// ── ODP ──────────────────────────────────────────────────────────────────────

fn read_odp(path: &Path, slide_num: Option<usize>, fmt: &str) -> Result<String> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let xml = read_zip_entry(path, "content.xml")?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);

    let mut slides: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if name == "page" {
                    current = Some(String::new());
                } else if matches!(name, "p" | "span")
                    && let Some(ref mut s) = current
                    && !s.is_empty()
                    && !s.ends_with('\n')
                {
                    s.push('\n');
                }
            }
            Ok(Event::Text(e)) => {
                let text = std::str::from_utf8(e.as_ref()).unwrap_or("").trim();
                if !text.is_empty()
                    && let Some(ref mut slide) = current
                {
                    if !slide.is_empty() && !slide.ends_with('\n') && !slide.ends_with(' ') {
                        slide.push(' ');
                    }
                    slide.push_str(text);
                }
            }
            Ok(Event::End(e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if name == "page"
                    && let Some(s) = current.take()
                {
                    slides.push(s.trim().to_string());
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    let selection: Vec<(usize, String)> = match slide_num {
        Some(n) => {
            let idx = n.saturating_sub(1);
            slides
                .get(idx)
                .map(|s| vec![(idx + 1, s.clone())])
                .unwrap_or_default()
        }
        None => slides
            .iter()
            .enumerate()
            .map(|(i, s)| (i + 1, s.clone()))
            .collect(),
    };

    match fmt {
        "json" => Ok(serde_json::to_string_pretty(&json!({
            "slides": selection.iter().map(|(i, s)| json!({ "slide": i, "text": s })).collect::<Vec<_>>()
        }))?),
        "text" => Ok(selection
            .iter()
            .map(|(_, s)| s.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")),
        _ => Ok(selection
            .iter()
            .map(|(i, s)| format!("### Slide {i}\n\n{s}"))
            .collect::<Vec<_>>()
            .join("\n\n")),
    }
}

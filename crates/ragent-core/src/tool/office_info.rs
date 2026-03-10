//! Office document metadata/info tool.
//!
//! Provides [`OfficeInfoTool`], which extracts metadata and structural
//! information from Microsoft Word (`.docx`), Excel (`.xlsx`), and
//! PowerPoint (`.pptx`) files.
//!
//! Depends on: `docx-rust`, `calamine`, `ooxmlsdk`.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::Path;

use super::office_common::{OfficeFormat, detect_format, resolve_path};
use super::{Tool, ToolContext, ToolOutput};

/// Extracts metadata and structural information from Office documents.
///
/// Returns file type, page/sheet/slide count, author, title, creation date,
/// word count (docx), sheet names (xlsx), slide titles (pptx).
pub struct OfficeInfoTool;

#[async_trait::async_trait]
impl Tool for OfficeInfoTool {
    fn name(&self) -> &str {
        "office_info"
    }

    fn description(&self) -> &str {
        "Get metadata and structural information about a Word, Excel, or PowerPoint file."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the Office document"
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
        let office_format = detect_format(&path)?;

        let file_size = tokio::fs::metadata(&path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        let path_clone = path.clone();
        let (content, metadata) =
            tokio::task::spawn_blocking(move || -> Result<(String, Value)> {
                match office_format {
                    OfficeFormat::Docx => info_docx(&path_clone, file_size),
                    OfficeFormat::Xlsx => info_xlsx(&path_clone, file_size),
                    OfficeFormat::Pptx => info_pptx(&path_clone, file_size),
                }
            })
            .await
            .context("Task join error")??;

        Ok(ToolOutput {
            content,
            metadata: Some(metadata),
        })
    }
}

/// Extracts metadata from a Word document.
///
/// # Arguments
///
/// * `path` - Path to the `.docx` file.
/// * `file_size` - File size in bytes.
///
/// # Returns
///
/// A tuple of `(display_text, json_metadata)`.
fn info_docx(path: &Path, file_size: u64) -> Result<(String, Value)> {
    let docx_file = docx_rust::DocxFile::from_file(path)
        .map_err(|e| anyhow::anyhow!("Failed to open docx: {e}"))?;
    let docx = docx_file
        .parse()
        .map_err(|e| anyhow::anyhow!("Failed to parse docx: {e}"))?;

    let mut paragraph_count = 0usize;
    let mut word_count = 0usize;
    let mut table_count = 0usize;

    for content in &docx.document.body.content {
        match content {
            docx_rust::document::BodyContent::Paragraph(para) => {
                paragraph_count += 1;
                let text = para.text();
                word_count += text.split_whitespace().count();
            }
            docx_rust::document::BodyContent::Table(_) => {
                table_count += 1;
            }
            _ => {}
        }
    }

    let title = docx
        .core
        .as_ref()
        .and_then(|c| match c {
            docx_rust::core::Core::CoreNamespace(cn) => cn.title.as_ref().map(|t| t.to_string()),
            docx_rust::core::Core::CoreNoNamespace(cn) => cn.title.as_ref().map(|t| t.to_string()),
        })
        .unwrap_or_default();

    let author = docx
        .core
        .as_ref()
        .and_then(|c| match c {
            docx_rust::core::Core::CoreNamespace(cn) => cn.creator.as_ref().map(|a| a.to_string()),
            docx_rust::core::Core::CoreNoNamespace(cn) => {
                cn.creator.as_ref().map(|a| a.to_string())
            }
        })
        .unwrap_or_default();

    let metadata = json!({
        "format": "docx",
        "file_size_bytes": file_size,
        "title": title,
        "author": author,
        "paragraph_count": paragraph_count,
        "word_count": word_count,
        "table_count": table_count,
    });

    let content = format!(
        "Format: Word Document (.docx)\n\
         File size: {} bytes\n\
         Title: {}\n\
         Author: {}\n\
         Paragraphs: {}\n\
         Word count: {}\n\
         Tables: {}",
        file_size,
        if title.is_empty() { "(none)" } else { &title },
        if author.is_empty() { "(none)" } else { &author },
        paragraph_count,
        word_count,
        table_count,
    );

    Ok((content, metadata))
}

/// Extracts metadata from an Excel workbook.
///
/// # Arguments
///
/// * `path` - Path to the `.xlsx` file.
/// * `file_size` - File size in bytes.
///
/// # Returns
///
/// A tuple of `(display_text, json_metadata)`.
fn info_xlsx(path: &Path, file_size: u64) -> Result<(String, Value)> {
    use calamine::{Reader, Xlsx};

    let mut workbook: Xlsx<_> =
        calamine::open_workbook(path).map_err(|e| anyhow::anyhow!("Failed to open xlsx: {e}"))?;

    let sheet_names = workbook.sheet_names().to_owned();
    let mut sheets_info: Vec<Value> = Vec::new();
    let mut content_lines: Vec<String> = Vec::new();

    content_lines.push(format!("Format: Excel Workbook (.xlsx)"));
    content_lines.push(format!("File size: {file_size} bytes"));
    content_lines.push(format!("Sheets: {}", sheet_names.len()));

    for name in &sheet_names {
        if let Ok(range) = workbook.worksheet_range(name) {
            let (rows, cols) = range.get_size();
            sheets_info.push(json!({
                "name": name,
                "rows": rows,
                "columns": cols,
            }));
            content_lines.push(format!("  - {name}: {rows} rows × {cols} columns"));
        } else {
            sheets_info.push(json!({
                "name": name,
                "rows": 0,
                "columns": 0,
            }));
            content_lines.push(format!("  - {name}: (unable to read)"));
        }
    }

    let metadata = json!({
        "format": "xlsx",
        "file_size_bytes": file_size,
        "sheet_count": sheet_names.len(),
        "sheets": sheets_info,
    });

    Ok((content_lines.join("\n"), metadata))
}

/// Extracts metadata from a PowerPoint presentation.
///
/// # Arguments
///
/// * `path` - Path to the `.pptx` file.
/// * `file_size` - File size in bytes.
///
/// # Returns
///
/// A tuple of `(display_text, json_metadata)`.
fn info_pptx(path: &Path, file_size: u64) -> Result<(String, Value)> {
    use ooxmlsdk::parts::presentation_document::PresentationDocument;
    use ooxmlsdk::schemas::schemas_openxmlformats_org_drawingml_2006_main::ParagraphChildChoice;
    use ooxmlsdk::schemas::schemas_openxmlformats_org_presentationml_2006_main::*;

    let doc = PresentationDocument::new_from_file(path)
        .map_err(|e| anyhow::anyhow!("Failed to open pptx: {e}"))?;

    let slide_parts = &doc.presentation_part.slide_parts;
    let slide_count = slide_parts.len();

    let mut slide_titles: Vec<String> = Vec::new();
    let mut content_lines: Vec<String> = Vec::new();

    content_lines.push(format!("Format: PowerPoint Presentation (.pptx)"));
    content_lines.push(format!("File size: {file_size} bytes"));
    content_lines.push(format!("Slides: {slide_count}"));

    for (i, slide_part) in slide_parts.iter().enumerate() {
        let mut title = String::new();
        for child in &slide_part.root_element.children {
            if let SlideChildChoice::PCSld(csd) = child {
                for shape_child in &csd.shape_tree.children {
                    if let ShapeTreeChildChoice::PSp(shape) = shape_child {
                        if let Some(text_body) = &shape.text_body {
                            if title.is_empty() {
                                let mut text = String::new();
                                for para in &text_body.a_p {
                                    for p_child in &para.children {
                                        if let ParagraphChildChoice::AR(run) = p_child {
                                            if let Some(ref content) = run.text.xml_content {
                                                text.push_str(content);
                                            }
                                        }
                                    }
                                }
                                if !text.is_empty() {
                                    title = text;
                                }
                            }
                        }
                    }
                }
            }
        }
        let display_title = if title.is_empty() {
            "(untitled)".to_string()
        } else {
            title.clone()
        };
        slide_titles.push(title);
        content_lines.push(format!("  {}. {display_title}", i + 1));
    }

    let metadata = json!({
        "format": "pptx",
        "file_size_bytes": file_size,
        "slide_count": slide_count,
        "slide_titles": slide_titles,
    });

    Ok((content_lines.join("\n"), metadata))
}

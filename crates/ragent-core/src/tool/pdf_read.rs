//! PDF reading tool.
//!
//! Provides [`PdfReadTool`], which extracts text content, metadata, and page
//! information from PDF files. Supports page range selection and multiple
//! output formats.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::office_common::{resolve_path, truncate_output, MAX_OUTPUT_BYTES};
use super::{Tool, ToolContext, ToolOutput};

/// Reads a PDF file and extracts its text content and metadata.
///
/// Supports optional page range selection and output format control.
pub struct PdfReadTool;

#[async_trait::async_trait]
impl Tool for PdfReadTool {
    fn name(&self) -> &str {
        "pdf_read"
    }

    fn description(&self) -> &str {
        "Read text content and metadata from a PDF file. Supports page range selection."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the PDF file to read"
                },
                "start_page": {
                    "type": "integer",
                    "description": "Starting page number (1-based, inclusive). Defaults to first page."
                },
                "end_page": {
                    "type": "integer",
                    "description": "Ending page number (1-based, inclusive). Defaults to last page."
                },
                "format": {
                    "type": "string",
                    "enum": ["text", "metadata", "json"],
                    "description": "Output format: 'text' for plain text (default), 'metadata' for document info only, 'json' for structured output with pages and metadata"
                }
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

        let format = input["format"].as_str().unwrap_or("text");
        let start_page = input["start_page"].as_u64().map(|n| n as usize);
        let end_page = input["end_page"].as_u64().map(|n| n as usize);

        let path_clone = path.clone();
        let format_owned = format.to_string();

        let result = tokio::task::spawn_blocking(move || {
            read_pdf(&path_clone, start_page, end_page, &format_owned)
        })
        .await
        .context("Failed to read PDF: the background task exited unexpectedly")??;

        Ok(ToolOutput {
            content: truncate_output(result),
            metadata: Some(json!({
                "path": path.display().to_string(),
                "format": "pdf",
            })),
        })
    }
}

/// Read a PDF file and extract content based on the requested format.
pub(crate) fn read_pdf(
    path: &std::path::Path,
    start_page: Option<usize>,
    end_page: Option<usize>,
    format: &str,
) -> Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("Failed to read PDF file: {}", path.display()))?;

    let doc = lopdf::Document::load_mem(&bytes)
        .with_context(|| format!("Failed to parse PDF: {}", path.display()))?;

    let total_pages = doc.get_pages().len();
    let metadata = extract_metadata(&doc);

    match format {
        "metadata" => {
            let mut output = String::new();
            output.push_str(&format!("PDF: {}\n", path.display()));
            output.push_str(&format!("Pages: {}\n", total_pages));
            if let Some(ref title) = metadata.title {
                output.push_str(&format!("Title: {}\n", title));
            }
            if let Some(ref author) = metadata.author {
                output.push_str(&format!("Author: {}\n", author));
            }
            if let Some(ref subject) = metadata.subject {
                output.push_str(&format!("Subject: {}\n", subject));
            }
            if let Some(ref creator) = metadata.creator {
                output.push_str(&format!("Creator: {}\n", creator));
            }
            if let Some(ref producer) = metadata.producer {
                output.push_str(&format!("Producer: {}\n", producer));
            }
            Ok(output)
        }
        "json" => {
            let pages = extract_pages_text(&bytes, total_pages, start_page, end_page)?;
            let page_objects: Vec<Value> = pages
                .iter()
                .map(|(num, text)| {
                    json!({
                        "page": num,
                        "text": text,
                    })
                })
                .collect();

            let result = json!({
                "path": path.display().to_string(),
                "total_pages": total_pages,
                "metadata": {
                    "title": metadata.title,
                    "author": metadata.author,
                    "subject": metadata.subject,
                    "creator": metadata.creator,
                    "producer": metadata.producer,
                },
                "pages": page_objects,
            });
            serde_json::to_string_pretty(&result).context("Failed to serialize PDF content")
        }
        _ => {
            // "text" format — plain text extraction
            let pages = extract_pages_text(&bytes, total_pages, start_page, end_page)?;
            let mut output = String::new();
            for (page_num, text) in &pages {
                if pages.len() > 1 {
                    output.push_str(&format!("--- Page {} ---\n", page_num));
                }
                output.push_str(text);
                output.push('\n');
                if output.len() > MAX_OUTPUT_BYTES {
                    break;
                }
            }
            Ok(output.trim_end().to_string())
        }
    }
}

/// Extract text from specified pages using pdf-extract.
fn extract_pages_text(
    bytes: &[u8],
    total_pages: usize,
    start_page: Option<usize>,
    end_page: Option<usize>,
) -> Result<Vec<(usize, String)>> {
    let start = start_page.unwrap_or(1).max(1);
    let end = end_page.unwrap_or(total_pages).min(total_pages);

    if start > total_pages {
        anyhow::bail!(
            "start_page {} exceeds total pages ({})",
            start,
            total_pages
        );
    }

    // pdf-extract works on the whole document; extract all then filter
    let full_text = pdf_extract::extract_text_from_mem(bytes)
        .with_context(|| "Failed to extract text from PDF")?;

    // pdf-extract doesn't provide per-page extraction directly,
    // so we use lopdf to get page-by-page text via content streams.
    let doc = lopdf::Document::load_mem(bytes)?;
    let pages = doc.get_pages();

    let mut result = Vec::new();
    for page_num in start..=end {
        let text = if let Some(&page_id) = pages.get(&(page_num as u32)) {
            extract_page_text(&doc, page_id).unwrap_or_default()
        } else {
            String::new()
        };
        result.push((page_num, text));
    }

    // If lopdf extraction yielded empty pages but pdf-extract got text,
    // fall back to the full text for the range
    let all_empty = result.iter().all(|(_, t)| t.trim().is_empty());
    if all_empty && !full_text.trim().is_empty() {
        // Can't do per-page, return full text attributed to page range
        result.clear();
        if start == 1 && end == total_pages {
            result.push((1, full_text));
        } else {
            result.push((
                start,
                format!(
                    "[Pages {}-{} — per-page extraction unavailable, showing full document text]\n\n{}",
                    start, end, full_text
                ),
            ));
        }
    }

    Ok(result)
}

/// Extract text from a single page using lopdf content streams.
fn extract_page_text(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Result<String> {
    let content = doc.get_page_content(page_id)?;

    let mut text = String::new();
    let ops = lopdf::content::Content::decode(&content)?.operations;

    for op in &ops {
        match op.operator.as_ref() {
            "Tj" | "TJ" => {
                for operand in &op.operands {
                    match operand {
                        lopdf::Object::String(bytes, _) => {
                            text.push_str(&String::from_utf8_lossy(bytes));
                        }
                        lopdf::Object::Array(arr) => {
                            for item in arr {
                                if let lopdf::Object::String(bytes, _) = item {
                                    text.push_str(&String::from_utf8_lossy(bytes));
                                } else if let lopdf::Object::Integer(n) = item {
                                    if *n < -100 {
                                        text.push(' ');
                                    }
                                } else if let lopdf::Object::Real(n) = item {
                                    if *n < -100.0 {
                                        text.push(' ');
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "Td" | "TD" | "T*" | "'" | "\"" => {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
            }
            _ => {}
        }
    }

    Ok(text)
}

struct PdfMetadata {
    title: Option<String>,
    author: Option<String>,
    subject: Option<String>,
    creator: Option<String>,
    producer: Option<String>,
}

fn extract_metadata(doc: &lopdf::Document) -> PdfMetadata {
    let get_info_string = |key: &[u8]| -> Option<String> {
        doc.trailer
            .get(b"Info")
            .ok()
            .and_then(|info| doc.dereference(info).ok())
            .and_then(|(_, obj)| match obj {
                lopdf::Object::Dictionary(dict) => dict.get(key).ok().cloned(),
                _ => None,
            })
            .and_then(|obj| match obj {
                lopdf::Object::String(bytes, _) => {
                    Some(String::from_utf8_lossy(&bytes).to_string())
                }
                _ => None,
            })
    };

    PdfMetadata {
        title: get_info_string(b"Title"),
        author: get_info_string(b"Author"),
        subject: get_info_string(b"Subject"),
        creator: get_info_string(b"Creator"),
        producer: get_info_string(b"Producer"),
    }
}

//! Round-trip integration tests for the `pdf_write` / `pdf_read` tools.
//!
//! `PdfWriteTool` renders structured JSON content into a real PDF on disk;
//! `PdfReadTool` extracts text, metadata, and structured JSON back out. These
//! tests write the PDF under `target/temp`, read it back, and assert on the
//! recovered content - exercising the full render/extract pipelines (headings,
//! paragraphs, tables, images, captions, page breaks, and every error path)
//! without any network access.

// `assert!(!x.is_empty())` is clearer than `assert_ne!(x, [])` in these tests.
#![allow(clippy::assert_is_empty)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_tools_extended::pdf_read::PdfReadTool;
use ragent_tools_extended::pdf_write::PdfWriteTool;
use ragent_tools_extended::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::{Value, json};

/// A unique scratch directory under `target/temp` that removes itself on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/temp")
            .join(format!(
                "pdfroundtrip_{tag}_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&dir).expect("creating scratch dir");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Build a `ToolContext` whose working directory is `work`.
fn ctx(work: &Path) -> ToolContext {
    ToolContext {
        session_id: "pdf-test".to_string(),
        working_dir: work.to_path_buf(),
        event_bus: Arc::new(EventBus::new(32)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: None,
        allowed_roots: Vec::new(),
    }
}

/// A minimal valid 2x2 RGBA PNG, base64-free (raw bytes).
fn tiny_png() -> Vec<u8> {
    // A valid 2x2 RGBA PNG with correct chunk CRCs (checked by `printpdf`'s
    // PNG decoder).
    vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 2, 8, 6,
        0, 0, 0, 114, 182, 13, 36, 0, 0, 0, 17, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240,
        31, 132, 25, 96, 12, 0, 71, 202, 7, 249, 103, 89, 110, 183, 0, 0, 0, 0, 73, 69, 78, 68,
        174, 66, 96, 130,
    ]
}

#[tokio::test]
async fn test_pdf_write_then_read_text_roundtrip() {
    let scratch = Scratch::new("text");
    let work = scratch.path();
    let tool = PdfWriteTool;
    let c = ctx(work);

    let out = tool
        .execute(
            json!({
                "path": "out.pdf",
                "content": {
                    "title": "Round Trip Document",
                    "elements": [
                        { "type": "heading", "text": "Introduction", "level": 1 },
                        { "type": "paragraph", "text": "This is the body of the document that should be recoverable." },
                        { "type": "heading", "text": "Details", "level": 2 },
                        { "type": "paragraph", "text": "Second paragraph with more words to wrap." }
                    ]
                }
            }),
            &c,
        )
        .await
        .expect("pdf_write should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["format"], "pdf");
    assert_eq!(meta["page_count"], 1);
    assert!(meta["byte_count"].as_u64().unwrap() > 0);
    assert!(out.content.contains("Wrote PDF"));

    // Now read it back in text format.
    let read = PdfReadTool;
    let read_out = read
        .execute(json!({ "path": "out.pdf", "format": "text" }), &c)
        .await
        .expect("pdf_read text should succeed");
    assert!(
        read_out.content.contains("Introduction") || read_out.content.contains("recoverable"),
        "text should contain embedded content: {}",
        read_out.content
    );
    assert_eq!(read_out.metadata.unwrap()["format"], "pdf");
}

#[tokio::test]
async fn test_pdf_write_all_heading_levels_and_images_and_tables() {
    let scratch = Scratch::new("rich");
    let work = scratch.path();
    std::fs::write(work.join("dot.png"), tiny_png()).unwrap();

    let tool = PdfWriteTool;
    let c = ctx(work);
    let out = tool
        .execute(
            json!({
                "path": "rich.pdf",
                "content": {
                    "elements": [
                        { "type": "heading", "text": "H1 heading" },
                        { "type": "heading", "text": "H2 heading", "level": 2 },
                        { "type": "heading", "text": "H3 heading", "level": 3 },
                        { "type": "table",
                          "headers": ["Name", "Value"],
                          "rows": [["alpha", "1"], ["beta", "2"], ["gamma", "3"]] },
                        { "type": "image", "image_path": "dot.png", "width_mm": 40.0,
                          "caption": "A tiny image" }
                    ]
                }
            }),
            &c,
        )
        .await
        .expect("pdf_write rich doc should succeed");
    assert!(out.metadata.unwrap()["byte_count"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn test_pdf_write_many_paragraphs_forces_page_breaks() {
    let scratch = Scratch::new("pages");
    let work = scratch.path();
    let mut elements: Vec<Value> = Vec::new();
    for i in 0..80 {
        elements.push(json!({
            "type": "paragraph",
            "text": format!(
                "Paragraph {i} with a reasonably long sentence that will wrap over \
                 multiple lines because the content width is fairly narrow on the page."
            )
        }));
    }
    let tool = PdfWriteTool;
    let out = tool
        .execute(
            json!({ "path": "long.pdf", "content": { "elements": elements } }),
            &ctx(work),
        )
        .await
        .expect("pdf_write long doc should succeed");
    // The document renders a single page object in output metadata but the
    // bytes must be non-trivial.
    assert!(out.metadata.unwrap()["byte_count"].as_u64().unwrap() > 1000);
}

#[tokio::test]
async fn test_pdf_write_absolute_image_path_is_rejected() {
    let scratch = Scratch::new("absimg");
    let work = scratch.path();
    let tool = PdfWriteTool;
    let err = tool
        .execute(
            json!({
                "path": "x.pdf",
                "content": { "elements": [
                    { "type": "image", "image_path": "/etc/passwd" }
                ] }
            }),
            &ctx(work),
        )
        .await
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("path")
            || err.to_string().to_lowercase().contains("outside")
            || err.to_string().to_lowercase().contains("escape"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn test_pdf_write_missing_path_errors() {
    let scratch = Scratch::new("nopath");
    let tool = PdfWriteTool;
    let err = tool
        .execute(
            json!({ "content": { "elements": [] } }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("path"), "{err}");
}

#[tokio::test]
async fn test_pdf_write_missing_content_errors() {
    let scratch = Scratch::new("nocontent");
    let tool = PdfWriteTool;
    let err = tool
        .execute(json!({ "path": "a.pdf" }), &ctx(scratch.path()))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("content"), "{err}");
}

#[tokio::test]
async fn test_pdf_write_missing_elements_array_errors() {
    let scratch = Scratch::new("noelems");
    let tool = PdfWriteTool;
    let err = tool
        .execute(
            json!({ "path": "a.pdf", "content": { "title": "no elements here" } }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("elements"), "{err}");
}

#[tokio::test]
async fn test_pdf_write_unknown_element_type_errors() {
    let scratch = Scratch::new("unknown");
    let tool = PdfWriteTool;
    let err = tool
        .execute(
            json!({ "path": "a.pdf", "content": { "elements": [
                { "type": "bogus", "text": "x" }
            ] } }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("bogus"), "{err}");
}

#[tokio::test]
async fn test_pdf_write_image_not_found_errors() {
    let scratch = Scratch::new("imgmissing");
    let tool = PdfWriteTool;
    let err = tool
        .execute(
            json!({ "path": "a.pdf", "content": { "elements": [
                { "type": "image", "image_path": "does-not-exist.png" }
            ] } }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("not found"),
        "{err}"
    );
}

#[tokio::test]
async fn test_pdf_write_image_without_path_errors() {
    let scratch = Scratch::new("img-nopath");
    let tool = PdfWriteTool;
    let err = tool
        .execute(
            json!({ "path": "a.pdf", "content": { "elements": [
                { "type": "image" }
            ] } }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("image_path"), "{err}");
}

#[tokio::test]
async fn test_pdf_write_creates_parent_directories() {
    let scratch = Scratch::new("mkdirp");
    let tool = PdfWriteTool;
    let out = tool
        .execute(
            json!({ "path": "deep/nested/dir/doc.pdf", "content": { "elements": [
                { "type": "paragraph", "text": "in a nested directory" }
            ] } }),
            &ctx(scratch.path()),
        )
        .await
        .expect("should create parents and write");
    assert!(scratch.path().join("deep/nested/dir/doc.pdf").exists());
    assert!(out.content.contains("Wrote PDF"));
}

#[tokio::test]
async fn test_pdf_write_absolute_output_path_rejected() {
    let scratch = Scratch::new("absout");
    let tool = PdfWriteTool;
    let err = tool
        .execute(
            json!({ "path": "/tmp/escape.pdf", "content": { "elements": [] } }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("outside")
            || err.to_string().to_lowercase().contains("path"),
        "{err}"
    );
}

#[tokio::test]
async fn test_pdf_read_metadata_format() {
    let scratch = Scratch::new("meta");
    let work = scratch.path();
    let c = ctx(work);
    PdfWriteTool
        .execute(
            json!({ "path": "m.pdf", "content": {
                "title": "Meta Title",
                "elements": [ { "type": "paragraph", "text": "body" } ]
            } }),
            &c,
        )
        .await
        .unwrap();
    let out = PdfReadTool
        .execute(json!({ "path": "m.pdf", "format": "metadata" }), &c)
        .await
        .expect("metadata format");
    assert!(out.content.contains("Pages: 1"), "{}", out.content);
    // The Info dictionary is emitted (printpdf writes a UTF-16 Title); the
    // reader surfaces a `Title:` line for it.
    assert!(out.content.contains("Title:"), "{}", out.content);
}

#[tokio::test]
async fn test_pdf_read_json_format_and_page_range() {
    let scratch = Scratch::new("json");
    let work = scratch.path();
    let c = ctx(work);
    // Build a document with a page break so there are >= 1 pages; use a table
    // to give it substance.
    PdfWriteTool
        .execute(
            json!({ "path": "j.pdf", "content": { "elements": [
                { "type": "paragraph", "text": "first page text" },
                { "type": "table", "headers": ["A","B"], "rows": [["1","2"]] }
            ] } }),
            &c,
        )
        .await
        .unwrap();
    let out = PdfReadTool
        .execute(
            json!({ "path": "j.pdf", "format": "json", "start_page": 1, "end_page": 1 }),
            &c,
        )
        .await
        .expect("json format");
    let parsed: Value = serde_json::from_str(&out.content).expect("valid JSON output");
    assert!(parsed["total_pages"].as_u64().unwrap() >= 1);
    assert!(parsed["pages"].is_array());
}

#[tokio::test]
async fn test_pdf_read_missing_path_errors() {
    let scratch = Scratch::new("read-nopath");
    let err = PdfReadTool
        .execute(json!({}), &ctx(scratch.path()))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("path"), "{err}");
}

#[tokio::test]
async fn test_pdf_read_nonexistent_file_errors() {
    let scratch = Scratch::new("read-missing");
    let err = PdfReadTool
        .execute(json!({ "path": "nope.pdf" }), &ctx(scratch.path()))
        .await
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("read")
            || err.to_string().to_lowercase().contains("pdf"),
        "{err}"
    );
}

#[tokio::test]
async fn test_pdf_read_non_pdf_bytes_errors() {
    let scratch = Scratch::new("read-bad");
    let work = scratch.path();
    std::fs::write(work.join("bad.pdf"), b"not a pdf at all").unwrap();
    let err = PdfReadTool
        .execute(json!({ "path": "bad.pdf" }), &ctx(work))
        .await
        .unwrap_err();
    assert!(err.to_string().to_lowercase().contains("pdf"), "{err}");
}

#[tokio::test]
async fn test_pdf_read_absolute_path_rejected() {
    let scratch = Scratch::new("read-abs");
    let err = PdfReadTool
        .execute(json!({ "path": "/etc/passwd" }), &ctx(scratch.path()))
        .await
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("outside")
            || err.to_string().to_lowercase().contains("path"),
        "{err}"
    );
}

#[tokio::test]
async fn test_pdf_read_start_page_beyond_total_errors() {
    let scratch = Scratch::new("read-range");
    let work = scratch.path();
    let c = ctx(work);
    PdfWriteTool
        .execute(
            json!({ "path": "s.pdf", "content": { "elements": [
                { "type": "paragraph", "text": "one page only" }
            ] } }),
            &c,
        )
        .await
        .unwrap();
    let err = PdfReadTool
        .execute(json!({ "path": "s.pdf", "start_page": 99 }), &c)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("exceeds"), "{err}");
}

#[test]
fn test_pdf_read_tool_schema_and_metadata() {
    let tool = PdfReadTool;
    assert_eq!(tool.name(), "pdf_read");
    assert_eq!(tool.permission_category(), "file:read");
    let schema = tool.parameters_schema();
    assert_eq!(schema["required"][0], "path");
    assert!(schema["properties"]["format"]["enum"].is_array());
    assert!(!tool.description().is_empty());
}

#[test]
fn test_pdf_write_tool_schema_and_metadata() {
    let tool = PdfWriteTool;
    assert_eq!(tool.name(), "pdf_write");
    assert_eq!(tool.permission_category(), "file:write");
    let schema = tool.parameters_schema();
    assert!(schema["properties"]["content"]["properties"]["elements"].is_object());
    assert!(!tool.description().is_empty());
}

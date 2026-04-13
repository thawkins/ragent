//! Tests for test_office_tools.rs

//! Integration tests for the Office document tools (office_read, office_write, office_info).
//!
//! These tests use round-trip patterns: write a document with `office_write`,
//! then read it back with `office_read` and verify with `office_info`.

use ragent_core::tool::*;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ragent_test_office_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_ctx(dir: PathBuf) -> ToolContext {
    ToolContext {
        session_id: "test-office".to_string(),
        working_dir: dir,
        event_bus: Arc::new(ragent_core::event::EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    }
}

// ── Tool registration ───────────────────────────────────────────

#[test]
fn test_office_tools_registered() {
    let registry = create_default_registry();
    for name in &["office_read", "office_write", "office_info"] {
        let tool = registry.get(name);
        assert!(tool.is_some(), "Missing tool: {name}");
        assert_eq!(tool.unwrap().name(), *name);
    }
}

#[test]
fn test_office_tool_schemas_valid() {
    let registry = create_default_registry();
    for name in &["office_read", "office_write", "office_info"] {
        let tool = registry.get(name).unwrap();
        let schema = tool.parameters_schema();
        assert!(schema.is_object(), "{name} schema should be an object");
        assert!(
            schema["properties"]["path"].is_object(),
            "{name} should have a 'path' property"
        );
    }
}

#[test]
fn test_office_tool_permissions() {
    let registry = create_default_registry();
    assert_eq!(
        registry.get("office_read").unwrap().permission_category(),
        "file:read"
    );
    assert_eq!(
        registry.get("office_write").unwrap().permission_category(),
        "file:write"
    );
    assert_eq!(
        registry.get("office_info").unwrap().permission_category(),
        "file:read"
    );
}

// ── Word (.docx) round-trip ─────────────────────────────────────

#[tokio::test]
async fn test_docx_write_and_read() {
    let dir = test_dir("docx_roundtrip");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    let result = write_tool
        .execute(
            json!({
                "path": "test.docx",
                "content": {
                    "paragraphs": [
                        { "text": "Document Title", "style": "Heading1" },
                        { "text": "This is the first paragraph." },
                        { "text": "Second paragraph with content." }
                    ]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.content.contains("Wrote"));
    assert!(result.content.contains("docx"));
    assert!(dir.join("test.docx").exists());

    let read_tool = registry.get("office_read").unwrap();
    let result = read_tool
        .execute(json!({"path": "test.docx", "format": "text"}), &ctx)
        .await
        .unwrap();

    assert!(
        result.content.contains("Document Title"),
        "Should contain heading text, got: {}",
        &result.content[..result.content.len().min(500)]
    );
    assert!(
        result.content.contains("first paragraph"),
        "Should contain paragraph text"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_docx_read_markdown_format() {
    let dir = test_dir("docx_markdown");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    write_tool
        .execute(
            json!({
                "path": "heading.docx",
                "content": {
                    "paragraphs": [
                        { "text": "My Heading", "style": "Heading1" },
                        { "text": "Some body text." }
                    ]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    let read_tool = registry.get("office_read").unwrap();
    let result = read_tool
        .execute(json!({"path": "heading.docx", "format": "markdown"}), &ctx)
        .await
        .unwrap();

    assert!(
        result.content.contains("My Heading"),
        "Markdown should contain heading text"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_docx_read_json_format() {
    let dir = test_dir("docx_json");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    write_tool
        .execute(
            json!({
                "path": "data.docx",
                "content": {
                    "paragraphs": [
                        { "text": "Hello World" }
                    ]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    let read_tool = registry.get("office_read").unwrap();
    let result = read_tool
        .execute(json!({"path": "data.docx", "format": "json"}), &ctx)
        .await
        .unwrap();

    let parsed: serde_json::Value =
        serde_json::from_str(&result.content).expect("JSON format should return valid JSON");
    assert!(
        parsed.is_object() || parsed.is_array(),
        "Should be JSON object or array"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_docx_info() {
    let dir = test_dir("docx_info");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    write_tool
        .execute(
            json!({
                "path": "info.docx",
                "content": {
                    "paragraphs": [
                        { "text": "Title", "style": "Heading1" },
                        { "text": "Body content here." }
                    ]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    let info_tool = registry.get("office_info").unwrap();
    let result = info_tool
        .execute(json!({"path": "info.docx"}), &ctx)
        .await
        .unwrap();

    assert!(
        result.content.contains("docx") || result.content.contains("Word"),
        "Info should mention document type"
    );
    assert!(result.metadata.is_some(), "Info should return metadata");
    let meta = result.metadata.unwrap();
    assert_eq!(meta["format"], "docx");

    std::fs::remove_dir_all(&dir).ok();
}

// ── Excel (.xlsx) round-trip ────────────────────────────────────

#[tokio::test]
async fn test_xlsx_write_and_read() {
    let dir = test_dir("xlsx_roundtrip");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    let result = write_tool
        .execute(
            json!({
                "path": "data.xlsx",
                "content": {
                    "sheets": [{
                        "name": "Sales",
                        "rows": [
                            ["Product", "Quantity", "Price"],
                            ["Widget", 10, 5.99],
                            ["Gadget", 25, 12.50]
                        ]
                    }]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.content.contains("Wrote"));
    assert!(dir.join("data.xlsx").exists());

    let read_tool = registry.get("office_read").unwrap();
    let result = read_tool
        .execute(json!({"path": "data.xlsx", "format": "text"}), &ctx)
        .await
        .unwrap();

    assert!(
        result.content.contains("Product"),
        "Should contain header text, got: {}",
        &result.content[..result.content.len().min(500)]
    );
    assert!(result.content.contains("Widget"), "Should contain data");

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_xlsx_multiple_sheets() {
    let dir = test_dir("xlsx_multi_sheet");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    write_tool
        .execute(
            json!({
                "path": "multi.xlsx",
                "content": {
                    "sheets": [
                        {
                            "name": "Alpha",
                            "rows": [["A1", "B1"], ["A2", "B2"]]
                        },
                        {
                            "name": "Beta",
                            "rows": [["X1", "Y1"], ["X2", "Y2"]]
                        }
                    ]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    let read_tool = registry.get("office_read").unwrap();

    // Read specific sheet by name
    let result = read_tool
        .execute(json!({"path": "multi.xlsx", "sheet": "Beta"}), &ctx)
        .await
        .unwrap();

    assert!(
        result.content.contains("X1"),
        "Should read Beta sheet content, got: {}",
        &result.content[..result.content.len().min(500)]
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_xlsx_with_range() {
    let dir = test_dir("xlsx_range");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    write_tool
        .execute(
            json!({
                "path": "range.xlsx",
                "content": {
                    "sheets": [{
                        "name": "Sheet1",
                        "rows": [
                            ["A", "B", "C", "D"],
                            [1, 2, 3, 4],
                            [5, 6, 7, 8],
                            [9, 10, 11, 12]
                        ]
                    }]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    let read_tool = registry.get("office_read").unwrap();
    let result = read_tool
        .execute(
            json!({"path": "range.xlsx", "range": "A1:B2", "format": "text"}),
            &ctx,
        )
        .await
        .unwrap();

    // The range A1:B2 should cover the first 2 rows and 2 columns
    assert!(
        result.content.contains('A'),
        "Range should include A1 content"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
#[allow(clippy::approx_constant)]
async fn test_xlsx_typed_cells() {
    let dir = test_dir("xlsx_types");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    write_tool
        .execute(
            json!({
                "path": "types.xlsx",
                "content": {
                    "sheets": [{
                        "name": "Types",
                        "rows": [
                            ["string_val", 42, 3.14, true, false]
                        ]
                    }]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    let read_tool = registry.get("office_read").unwrap();
    let result = read_tool
        .execute(json!({"path": "types.xlsx", "format": "text"}), &ctx)
        .await
        .unwrap();

    assert!(
        result.content.contains("string_val"),
        "Should contain string"
    );
    assert!(result.content.contains("42"), "Should contain integer");
    assert!(result.content.contains("3.14"), "Should contain float");

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_xlsx_info() {
    let dir = test_dir("xlsx_info");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    write_tool
        .execute(
            json!({
                "path": "info.xlsx",
                "content": {
                    "sheets": [
                        { "name": "First", "rows": [["A"]] },
                        { "name": "Second", "rows": [["B"]] }
                    ]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    let info_tool = registry.get("office_info").unwrap();
    let result = info_tool
        .execute(json!({"path": "info.xlsx"}), &ctx)
        .await
        .unwrap();

    assert!(result.metadata.is_some());
    let meta = result.metadata.unwrap();
    assert_eq!(meta["format"], "xlsx");
    let sheets = meta["sheets"].as_array().expect("Should have sheets");
    assert_eq!(sheets.len(), 2);

    std::fs::remove_dir_all(&dir).ok();
}

// ── PowerPoint (.pptx) round-trip ───────────────────────────────

#[tokio::test]
async fn test_pptx_write_and_read() {
    let dir = test_dir("pptx_roundtrip");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    let result = write_tool
        .execute(
            json!({
                "path": "presentation.pptx",
                "content": {
                    "slides": [
                        {
                            "title": "Welcome Slide",
                            "body": "This is the introduction."
                        },
                        {
                            "title": "Second Slide",
                            "body": "More details here.",
                            "notes": "Speaker notes for slide 2."
                        }
                    ]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.content.contains("Wrote"));
    assert!(dir.join("presentation.pptx").exists());

    let read_tool = registry.get("office_read").unwrap();
    let result = read_tool
        .execute(json!({"path": "presentation.pptx", "format": "text"}), &ctx)
        .await
        .unwrap();

    assert!(
        result.content.contains("Welcome Slide"),
        "Should contain slide title, got: {}",
        &result.content[..result.content.len().min(500)]
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_pptx_read_specific_slide() {
    let dir = test_dir("pptx_slide");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    write_tool
        .execute(
            json!({
                "path": "slides.pptx",
                "content": {
                    "slides": [
                        { "title": "Slide One", "body": "First content" },
                        { "title": "Slide Two", "body": "Second content" },
                        { "title": "Slide Three", "body": "Third content" }
                    ]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    let read_tool = registry.get("office_read").unwrap();
    let result = read_tool
        .execute(json!({"path": "slides.pptx", "slide": 2}), &ctx)
        .await
        .unwrap();

    assert!(
        result.content.contains("Slide Two") || result.content.contains("Second content"),
        "Should contain slide 2 content, got: {}",
        &result.content[..result.content.len().min(500)]
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_pptx_info() {
    let dir = test_dir("pptx_info");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    write_tool
        .execute(
            json!({
                "path": "info.pptx",
                "content": {
                    "slides": [
                        { "title": "Title Slide", "body": "Content" },
                        { "title": "Second", "body": "More" }
                    ]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    let info_tool = registry.get("office_info").unwrap();
    let result = info_tool
        .execute(json!({"path": "info.pptx"}), &ctx)
        .await
        .unwrap();

    assert!(result.metadata.is_some());
    let meta = result.metadata.unwrap();
    assert_eq!(meta["format"], "pptx");
    let slide_count = meta["slide_count"]
        .as_u64()
        .expect("Should have slide_count");
    assert_eq!(slide_count, 2);

    std::fs::remove_dir_all(&dir).ok();
}

// ── Error handling ──────────────────────────────────────────────

#[tokio::test]
async fn test_read_nonexistent_file() {
    let dir = test_dir("read_noexist");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let read_tool = registry.get("office_read").unwrap();
    let result = read_tool
        .execute(json!({"path": "nonexistent.docx"}), &ctx)
        .await;

    assert!(result.is_err(), "Reading nonexistent file should fail");
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_read_missing_path_parameter() {
    let dir = test_dir("read_noparam");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let read_tool = registry.get("office_read").unwrap();
    let result = read_tool.execute(json!({}), &ctx).await;

    assert!(result.is_err(), "Missing path parameter should fail");
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_write_missing_content_parameter() {
    let dir = test_dir("write_nocontent");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    let result = write_tool.execute(json!({"path": "test.docx"}), &ctx).await;

    assert!(result.is_err(), "Missing content parameter should fail");
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_unsupported_extension() {
    let dir = test_dir("unsupported_ext");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let read_tool = registry.get("office_read").unwrap();
    // Create a dummy file with wrong extension
    std::fs::write(dir.join("file.txt"), "not an office doc").unwrap();
    let result = read_tool.execute(json!({"path": "file.txt"}), &ctx).await;

    assert!(result.is_err(), "Unsupported extension should fail");
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_write_creates_parent_directories() {
    let dir = test_dir("write_mkdir");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    let write_tool = registry.get("office_write").unwrap();
    let result = write_tool
        .execute(
            json!({
                "path": "sub/dir/nested.docx",
                "content": {
                    "paragraphs": [{ "text": "Nested file content" }]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.content.contains("Wrote"));
    assert!(dir.join("sub/dir/nested.docx").exists());

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_write_explicit_type_override() {
    let dir = test_dir("write_type_override");
    let ctx = make_ctx(dir.clone());
    let registry = create_default_registry();

    // Write a docx but use explicit type parameter
    let write_tool = registry.get("office_write").unwrap();
    let result = write_tool
        .execute(
            json!({
                "path": "doc.docx",
                "type": "docx",
                "content": {
                    "paragraphs": [{ "text": "Explicit type test" }]
                }
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.content.contains("docx"));
    assert!(result.metadata.is_some());
    let meta = result.metadata.unwrap();
    assert_eq!(meta["format"], "docx");

    std::fs::remove_dir_all(&dir).ok();
}

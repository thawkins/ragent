//! Tests for metadata builder and field standardization
//!
//! This test suite verifies that the MetadataBuilder produces consistent
//! field names and values according to the tool output metadata schema.

use ragent_core::tool::metadata::MetadataBuilder;
use serde_json::Value;

// =============================================================================
// Basic Builder Tests
// =============================================================================

#[test]
fn test_empty_builder_returns_none() {
    let builder = MetadataBuilder::new();
    assert!(
        builder.build().is_none(),
        "Empty builder should return None"
    );
}

#[test]
fn test_single_field() {
    let metadata = MetadataBuilder::new()
        .count(42)
        .build()
        .expect("should build");

    assert_eq!(metadata.get("count"), Some(&Value::Number(42.into())));
}

#[test]
fn test_multiple_fields() {
    let metadata = MetadataBuilder::new()
        .path("src/main.rs")
        .line_count(100)
        .byte_count(1024)
        .build()
        .expect("should build");

    assert_eq!(
        metadata.get("path"),
        Some(&Value::String("src/main.rs".to_string()))
    );
    assert_eq!(metadata.get("line_count"), Some(&Value::Number(100.into())));
    assert_eq!(
        metadata.get("byte_count"),
        Some(&Value::Number(1024.into()))
    );
}

// =============================================================================
// Path Field Tests
// =============================================================================

#[test]
fn test_path_field() {
    let metadata = MetadataBuilder::new()
        .path("/absolute/path.txt")
        .build()
        .unwrap();

    assert_eq!(
        metadata.get("path").unwrap().as_str().unwrap(),
        "/absolute/path.txt"
    );
}

#[test]
fn test_path_relative() {
    let metadata = MetadataBuilder::new()
        .path("relative/path.rs")
        .build()
        .unwrap();

    assert_eq!(
        metadata.get("path").unwrap().as_str().unwrap(),
        "relative/path.rs"
    );
}

// =============================================================================
// Count Field Tests
// =============================================================================

#[test]
fn test_line_count_field() {
    let metadata = MetadataBuilder::new().line_count(42).build().unwrap();

    assert_eq!(metadata.get("line_count").unwrap().as_u64(), Some(42));
}

#[test]
fn test_total_lines_field() {
    let metadata = MetadataBuilder::new()
        .line_count(50)
        .total_lines(100)
        .build()
        .unwrap();

    assert_eq!(metadata.get("line_count").unwrap().as_u64(), Some(50));
    assert_eq!(metadata.get("total_lines").unwrap().as_u64(), Some(100));
}

#[test]
fn test_count_field() {
    let metadata = MetadataBuilder::new().count(5).build().unwrap();

    assert_eq!(metadata.get("count").unwrap().as_u64(), Some(5));
}

#[test]
fn test_file_count_field() {
    let metadata = MetadataBuilder::new().file_count(3).build().unwrap();

    assert_eq!(metadata.get("file_count").unwrap().as_u64(), Some(3));
}

#[test]
fn test_entries_field() {
    let metadata = MetadataBuilder::new().entries(10).build().unwrap();

    assert_eq!(metadata.get("entries").unwrap().as_u64(), Some(10));
}

#[test]
fn test_matches_field() {
    let metadata = MetadataBuilder::new().matches(7).build().unwrap();

    assert_eq!(metadata.get("matches").unwrap().as_u64(), Some(7));
}

// =============================================================================
// Status Field Tests
// =============================================================================

#[test]
fn test_exit_code_field() {
    let metadata = MetadataBuilder::new().exit_code(0).build().unwrap();

    assert_eq!(metadata.get("exit_code").unwrap().as_i64(), Some(0));
}

#[test]
fn test_exit_code_nonzero() {
    let metadata = MetadataBuilder::new().exit_code(1).build().unwrap();

    assert_eq!(metadata.get("exit_code").unwrap().as_i64(), Some(1));
}

#[test]
fn test_duration_ms_field() {
    let metadata = MetadataBuilder::new().duration_ms(150).build().unwrap();

    assert_eq!(metadata.get("duration_ms").unwrap().as_u64(), Some(150));
}

#[test]
fn test_timed_out_field() {
    let metadata = MetadataBuilder::new().timed_out(false).build().unwrap();

    assert_eq!(metadata.get("timed_out").unwrap().as_bool(), Some(false));
}

#[test]
fn test_timed_out_true() {
    let metadata = MetadataBuilder::new().timed_out(true).build().unwrap();

    assert_eq!(metadata.get("timed_out").unwrap().as_bool(), Some(true));
}

#[test]
fn test_status_code_field() {
    let metadata = MetadataBuilder::new().status_code(200).build().unwrap();

    assert_eq!(metadata.get("status_code").unwrap().as_u64(), Some(200));
}

// =============================================================================
// Size Field Tests
// =============================================================================

#[test]
fn test_byte_count_field() {
    let metadata = MetadataBuilder::new().byte_count(2048).build().unwrap();

    assert_eq!(metadata.get("byte_count").unwrap().as_u64(), Some(2048));
}

// =============================================================================
// Content Field Tests
// =============================================================================

#[test]
fn test_summarized_field() {
    let metadata = MetadataBuilder::new().summarized(true).build().unwrap();

    assert_eq!(metadata.get("summarized").unwrap().as_bool(), Some(true));
}

// =============================================================================
// Edit/Change Field Tests
// =============================================================================

#[test]
fn test_edit_lines_field() {
    let metadata = MetadataBuilder::new().edit_lines(5, 3).build().unwrap();

    assert_eq!(metadata.get("old_lines").unwrap().as_u64(), Some(5));
    assert_eq!(metadata.get("new_lines").unwrap().as_u64(), Some(3));
}

// =============================================================================
// Task/Agent Field Tests
// =============================================================================

#[test]
fn test_task_id_field() {
    let metadata = MetadataBuilder::new().task_id("task-001").build().unwrap();

    assert_eq!(
        metadata.get("task_id").unwrap().as_str().unwrap(),
        "task-001"
    );
}

// =============================================================================
// Custom Field Tests
// =============================================================================

#[test]
fn test_custom_field_string() {
    let metadata = MetadataBuilder::new()
        .custom("custom_key", "custom_value")
        .build()
        .unwrap();

    assert_eq!(
        metadata.get("custom_key").unwrap().as_str().unwrap(),
        "custom_value"
    );
}

#[test]
fn test_custom_field_number() {
    let metadata = MetadataBuilder::new()
        .custom("score", 99.5)
        .build()
        .unwrap();

    let score = metadata.get("score").unwrap().as_f64().unwrap();
    assert!((score - 99.5).abs() < f64::EPSILON);
}

#[test]
fn test_custom_field_bool() {
    let metadata = MetadataBuilder::new()
        .custom("is_valid", true)
        .build()
        .unwrap();

    assert_eq!(metadata.get("is_valid").unwrap().as_bool(), Some(true));
}

// =============================================================================
// Chaining Tests
// =============================================================================

#[test]
fn test_builder_chaining() {
    let metadata = MetadataBuilder::new()
        .path("test.txt")
        .count(10)
        .line_count(5)
        .byte_count(100)
        .exit_code(0)
        .duration_ms(50)
        .summarized(false)
        .build()
        .unwrap();

    assert!(metadata.get("path").is_some());
    assert!(metadata.get("count").is_some());
    assert!(metadata.get("line_count").is_some());
    assert!(metadata.get("byte_count").is_some());
    assert!(metadata.get("exit_code").is_some());
    assert!(metadata.get("duration_ms").is_some());
    assert!(metadata.get("summarized").is_some());
}

// =============================================================================
// Complete Use Case Tests
// =============================================================================

#[test]
fn test_file_operation_metadata() {
    // Simulating a write tool result
    let metadata = MetadataBuilder::new()
        .path("src/main.rs")
        .byte_count(1024)
        .line_count(42)
        .build()
        .unwrap();

    assert_eq!(
        metadata.get("path").unwrap().as_str().unwrap(),
        "src/main.rs"
    );
    assert_eq!(metadata.get("byte_count").unwrap().as_u64(), Some(1024));
    assert_eq!(metadata.get("line_count").unwrap().as_u64(), Some(42));
}

#[test]
fn test_search_metadata() {
    // Simulating a grep tool result
    let metadata = MetadataBuilder::new()
        .count(5)
        .file_count(3)
        .truncated(false)
        .build()
        .unwrap();

    assert_eq!(metadata.get("count").unwrap().as_u64(), Some(5));
    assert_eq!(metadata.get("file_count").unwrap().as_u64(), Some(3));
    assert_eq!(metadata.get("truncated").unwrap().as_bool(), Some(false));
}

#[test]
fn test_execution_metadata() {
    // Simulating a bash tool result
    let metadata = MetadataBuilder::new()
        .exit_code(0)
        .duration_ms(150)
        .line_count(10)
        .timed_out(false)
        .build()
        .unwrap();

    assert_eq!(metadata.get("exit_code").unwrap().as_i64(), Some(0));
    assert_eq!(metadata.get("duration_ms").unwrap().as_u64(), Some(150));
    assert_eq!(metadata.get("line_count").unwrap().as_u64(), Some(10));
    assert_eq!(metadata.get("timed_out").unwrap().as_bool(), Some(false));
}

#[test]
fn test_edit_metadata() {
    // Simulating a multiedit tool result
    let metadata = MetadataBuilder::new()
        .count(3)
        .file_count(2)
        .edit_lines(10, 8)
        .build()
        .unwrap();

    assert_eq!(metadata.get("count").unwrap().as_u64(), Some(3));
    assert_eq!(metadata.get("file_count").unwrap().as_u64(), Some(2));
    assert_eq!(metadata.get("old_lines").unwrap().as_u64(), Some(10));
    assert_eq!(metadata.get("new_lines").unwrap().as_u64(), Some(8));
}

// =============================================================================
// Field Name Standardization Tests
// =============================================================================

#[test]
fn test_standard_field_names() {
    // Verify that we're using the standard field names from the schema
    let metadata = MetadataBuilder::new()
        .path("file.txt")
        .line_count(10)
        .total_lines(100)
        .byte_count(1024)
        .count(5)
        .file_count(3)
        .entries(20)
        .matches(15)
        .exit_code(0)
        .duration_ms(100)
        .timed_out(false)
        .status_code(200)
        .summarized(false)
        .task_id("task-001")
        .edit_lines(5, 3)
        .build()
        .unwrap();

    // All standard field names should be present
    let expected_fields = vec![
        "path",
        "line_count",
        "total_lines",
        "byte_count",
        "count",
        "file_count",
        "entries",
        "matches",
        "exit_code",
        "duration_ms",
        "timed_out",
        "status_code",
        "summarized",
        "task_id",
        "old_lines",
        "new_lines",
    ];

    for field in expected_fields {
        assert!(metadata.get(field).is_some(), "Missing field: {}", field);
    }
}

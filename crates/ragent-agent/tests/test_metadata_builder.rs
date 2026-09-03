#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-agent/src/tool/metadata.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_agent::tool::metadata::*;

#[test]
fn test_empty_builder_returns_none() {
    let metadata = MetadataBuilder::new().build();
    assert!(metadata.is_none());
}

#[test]
fn test_single_field() {
    let metadata = MetadataBuilder::new().path("/test/file.txt").build();
    assert!(metadata.is_some());
    let obj = metadata.unwrap().as_object().unwrap().clone();
    assert_eq!(obj.get("path").unwrap().as_str().unwrap(), "/test/file.txt");
}

#[test]
fn test_multiple_fields() {
    let metadata = MetadataBuilder::new()
        .path("/test/file.txt")
        .line_count(42)
        .byte_count(1024)
        .build();

    let obj = metadata.unwrap().as_object().unwrap().clone();
    assert_eq!(obj.get("path").unwrap().as_str().unwrap(), "/test/file.txt");
    assert_eq!(obj.get("line_count").unwrap().as_u64().unwrap(), 42);
    assert_eq!(obj.get("byte_count").unwrap().as_u64().unwrap(), 1024);
}

#[test]
fn test_chaining() {
    let metadata = MetadataBuilder::new()
        .exit_code(0)
        .duration_ms(150)
        .timed_out(false)
        .build();

    let obj = metadata.unwrap().as_object().unwrap().clone();
    assert_eq!(obj.get("exit_code").unwrap().as_i64().unwrap(), 0);
    assert_eq!(obj.get("duration_ms").unwrap().as_u64().unwrap(), 150);
    assert!(!obj.get("timed_out").unwrap().as_bool().unwrap());
}

#[test]
fn test_edit_lines() {
    let metadata = MetadataBuilder::new()
        .path("/test/file.txt")
        .edit_lines(10, 5)
        .build();

    let obj = metadata.unwrap().as_object().unwrap().clone();
    assert_eq!(obj.get("old_lines").unwrap().as_u64().unwrap(), 10);
    assert_eq!(obj.get("new_lines").unwrap().as_u64().unwrap(), 5);
}

#[test]
fn test_summarized() {
    let metadata = MetadataBuilder::new()
        .line_count(100)
        .total_lines(500)
        .summarized(true)
        .build();

    let obj = metadata.unwrap().as_object().unwrap().clone();
    assert_eq!(obj.get("line_count").unwrap().as_u64().unwrap(), 100);
    assert_eq!(obj.get("total_lines").unwrap().as_u64().unwrap(), 500);
    assert!(obj.get("summarized").unwrap().as_bool().unwrap());
}

#[test]
fn test_custom_field() {
    let metadata = MetadataBuilder::new()
        .path("/test/file.txt")
        .custom("custom_key", "custom_value")
        .build();

    let obj = metadata.unwrap().as_object().unwrap().clone();
    assert_eq!(
        obj.get("custom_key").unwrap().as_str().unwrap(),
        "custom_value"
    );
}

#[test]
fn test_count_fields() {
    let metadata = MetadataBuilder::new()
        .count(42)
        .file_count(5)
        .entries(10)
        .matches(3)
        .build();

    let obj = metadata.unwrap().as_object().unwrap().clone();
    assert_eq!(obj.get("count").unwrap().as_u64().unwrap(), 42);
    assert_eq!(obj.get("file_count").unwrap().as_u64().unwrap(), 5);
    assert_eq!(obj.get("entries").unwrap().as_u64().unwrap(), 10);
    assert_eq!(obj.get("matches").unwrap().as_u64().unwrap(), 3);
}

#[test]
fn test_task_id() {
    let metadata = MetadataBuilder::new().task_id("task-001").build();
    let obj = metadata.unwrap().as_object().unwrap().clone();
    assert_eq!(obj.get("task_id").unwrap().as_str().unwrap(), "task-001");
}

#[test]
fn test_status_code() {
    let metadata = MetadataBuilder::new().status_code(200).build();
    let obj = metadata.unwrap().as_object().unwrap().clone();
    assert_eq!(obj.get("status_code").unwrap().as_u64().unwrap(), 200);
}

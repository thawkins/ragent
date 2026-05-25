//! Tests for the `read` tool, especially the `num_lines` parameter.

use serde_json::json;
use std::io::Write;
use std::sync::Arc;

use ragent_tools_core::read::ReadTool;
use ragent_tools_core::{Tool, ToolContext, ToolOutput};
use ragent_types::event::EventBus;

fn make_ctx() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::current_dir().unwrap(),
        event_bus: Arc::new(EventBus::new(1024)),
    }
}

/// Helper: execute the read tool and return the output.
async fn read_file_with(input: serde_json::Value) -> ToolOutput {
    ReadTool.execute(input, &make_ctx()).await.unwrap()
}

/// Build input JSON for read tool.
fn read_input(path: &str, extra: serde_json::Value) -> serde_json::Value {
    let mut obj = json!({ "path": path });
    if let Some(map) = extra.as_object() {
        for (k, v) in map {
            obj[k] = v.clone();
        }
    }
    obj
}

#[tokio::test]
async fn test_read_num_lines_basic() {
    let tmp = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
    {
        let mut f = std::fs::File::create(tmp.path()).unwrap();
        for i in 1..=500 {
            writeln!(f, "Line {i}").unwrap();
        }
    }

    let input = read_input(
        tmp.path().to_str().unwrap(),
        json!({ "start_line": 201, "num_lines": 100 }),
    );
    let out = read_file_with(input).await;

    // Should contain lines 201–300
    assert!(out.content.contains("Line 201"));
    assert!(out.content.contains("Line 300"));
    assert!(!out.content.contains("Line 200"));
    assert!(!out.content.contains("Line 301"));

    let meta = out.metadata.unwrap();
    assert_eq!(meta["start_line"], 201);
    assert_eq!(meta["end_line"], 300);
    assert_eq!(meta["line_count"], 100);
}

#[tokio::test]
async fn test_read_end_line_takes_precedence_over_num_lines() {
    let tmp = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
    {
        let mut f = std::fs::File::create(tmp.path()).unwrap();
        for i in 1..=500 {
            writeln!(f, "Line {i}").unwrap();
        }
    }

    let input = read_input(
        tmp.path().to_str().unwrap(),
        json!({ "start_line": 201, "end_line": 250, "num_lines": 100 }),
    );
    let out = read_file_with(input).await;

    // end_line=250 should win, so only lines 201–250
    assert!(out.content.contains("Line 201"));
    assert!(out.content.contains("Line 250"));
    assert!(!out.content.contains("Line 251"));

    let meta = out.metadata.unwrap();
    assert_eq!(meta["end_line"], 250);
    assert_eq!(meta["line_count"], 50);
}

#[tokio::test]
async fn test_read_num_lines_clamped_to_total_lines() {
    let tmp = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
    {
        let mut f = std::fs::File::create(tmp.path()).unwrap();
        for i in 1..=50 {
            writeln!(f, "Line {i}").unwrap();
        }
    }

    let input = read_input(
        tmp.path().to_str().unwrap(),
        json!({ "start_line": 40, "num_lines": 100 }),
    );
    let out = read_file_with(input).await;

    // Only 50 lines total, starting at 40 → should read 40–50 (11 lines)
    assert!(out.content.contains("Line 40"));
    assert!(out.content.contains("Line 50"));
    assert!(!out.content.contains("Line 51"));

    let meta = out.metadata.unwrap();
    assert_eq!(meta["end_line"], 50);
    assert_eq!(meta["line_count"], 11);
}

#[tokio::test]
async fn test_read_num_lines_with_start_line_one() {
    let tmp = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
    {
        let mut f = std::fs::File::create(tmp.path()).unwrap();
        for i in 1..=100 {
            writeln!(f, "Line {i}").unwrap();
        }
    }

    let input = read_input(
        tmp.path().to_str().unwrap(),
        json!({ "start_line": 1, "num_lines": 10 }),
    );
    let out = read_file_with(input).await;

    assert!(out.content.contains("Line 1"));
    assert!(out.content.contains("Line 10"));
    assert!(!out.content.contains("Line 11"));

    let meta = out.metadata.unwrap();
    assert_eq!(meta["start_line"], 1);
    assert_eq!(meta["end_line"], 10);
    assert_eq!(meta["line_count"], 10);
}

#[tokio::test]
async fn test_read_num_lines_without_start_line_is_ignored() {
    let tmp = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
    {
        let mut f = std::fs::File::create(tmp.path()).unwrap();
        for i in 1..=10 {
            writeln!(f, "Line {i}").unwrap();
        }
    }

    let input = read_input(tmp.path().to_str().unwrap(), json!({ "num_lines": 5 }));
    let out = read_file_with(input).await;

    // Without start_line, num_lines is ignored; full file returned (≤100 lines)
    assert!(out.content.contains("Line 1"));
    assert!(out.content.contains("Line 10"));

    let meta = out.metadata.unwrap();
    assert_eq!(meta["start_line"], 1);
    assert_eq!(meta["end_line"], 10);
    assert_eq!(meta["line_count"], 10);
}

#[tokio::test]
async fn test_read_num_lines_zero_is_error() {
    let tmp = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
    {
        let mut f = std::fs::File::create(tmp.path()).unwrap();
        writeln!(f, "Line 1").unwrap();
    }

    let input = read_input(
        tmp.path().to_str().unwrap(),
        json!({ "start_line": 1, "num_lines": 0 }),
    );
    let result = ReadTool.execute(input, &make_ctx()).await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("num_lines"));
}

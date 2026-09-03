#![allow(clippy::assert_is_empty)]
//! Tests for the `read` tool, especially the `num_lines` parameter.

use serde_json::json;
use std::io::Write;
use std::sync::Arc;

use ragent_tools_core::read::ReadTool;
use ragent_tools_core::{Tool, ToolContext, ToolOutput};
use ragent_types::event::EventBus;

fn make_ctx(dir: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(1024)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir.to_path_buf()],
    }
}

/// Helper: create a temp directory under the current working directory so the
/// read tool's path-containment check is satisfied.
fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("temp")
        .join(format!(
            "read_tool_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Helper: execute the read tool and return the output.
async fn read_file_with(input: serde_json::Value, dir: &std::path::Path) -> ToolOutput {
    ReadTool.execute(input, &make_ctx(dir)).await.unwrap()
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
    let dir = temp_dir();
    let tmp = dir.join("test.txt");
    {
        let mut f = std::fs::File::create(&tmp).unwrap();
        for i in 1..=500 {
            writeln!(f, "Line {i}").unwrap();
        }
    }

    let input = read_input(
        tmp.to_str().unwrap(),
        json!({ "start_line": 201, "num_lines": 100 }),
    );
    let out = read_file_with(input, &dir).await;

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
    let dir = temp_dir();
    let tmp = dir.join("test.txt");
    {
        let mut f = std::fs::File::create(&tmp).unwrap();
        for i in 1..=500 {
            writeln!(f, "Line {i}").unwrap();
        }
    }

    let input = read_input(
        tmp.to_str().unwrap(),
        json!({ "start_line": 201, "end_line": 250, "num_lines": 100 }),
    );
    let out = read_file_with(input, &dir).await;

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
    let dir = temp_dir();
    let tmp = dir.join("test.txt");
    {
        let mut f = std::fs::File::create(&tmp).unwrap();
        for i in 1..=50 {
            writeln!(f, "Line {i}").unwrap();
        }
    }

    let input = read_input(
        tmp.to_str().unwrap(),
        json!({ "start_line": 40, "num_lines": 100 }),
    );
    let out = read_file_with(input, &dir).await;

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
    let dir = temp_dir();
    let tmp = dir.join("test.txt");
    {
        let mut f = std::fs::File::create(&tmp).unwrap();
        for i in 1..=100 {
            writeln!(f, "Line {i}").unwrap();
        }
    }

    let input = read_input(
        tmp.to_str().unwrap(),
        json!({ "start_line": 1, "num_lines": 10 }),
    );
    let out = read_file_with(input, &dir).await;

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
    let dir = temp_dir();
    let tmp = dir.join("test.txt");
    {
        let mut f = std::fs::File::create(&tmp).unwrap();
        for i in 1..=10 {
            writeln!(f, "Line {i}").unwrap();
        }
    }

    let input = read_input(tmp.to_str().unwrap(), json!({ "num_lines": 5 }));
    let out = read_file_with(input, &dir).await;

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
    let dir = temp_dir();
    let tmp = dir.join("test.txt");
    {
        let mut f = std::fs::File::create(&tmp).unwrap();
        writeln!(f, "Line 1").unwrap();
    }

    let input = read_input(
        tmp.to_str().unwrap(),
        json!({ "start_line": 1, "num_lines": 0 }),
    );
    let result = ReadTool.execute(input, &make_ctx(&dir)).await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("num_lines"));
}

#[tokio::test]
async fn test_read_end_line_smaller_than_start_line_gives_actionable_error() {
    // The most common mistake we see in practice: a model writes
    //   { "start_line": 200, "end_line": 100 }
    // meaning "give me 100 lines starting at 200".  The old behaviour produced
    // a generic "start_line must be <= end_line" error.  The new behaviour
    // recognises the likely intent and tells the caller to use num_lines.
    let dir = temp_dir();
    let tmp = dir.join("test.txt");
    {
        let mut f = std::fs::File::create(&tmp).unwrap();
        for i in 1..=500 {
            writeln!(f, "Line {i}").unwrap();
        }
    }

    let input = read_input(
        tmp.to_str().unwrap(),
        json!({ "start_line": 200, "end_line": 100 }),
    );
    let result = ReadTool.execute(input, &make_ctx(&dir)).await;

    assert!(result.is_err(), "expected error for end_line < start_line");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("num_lines=100"),
        "should suggest num_lines: {err}"
    );
    assert!(err.contains("end_line"), "should mention end_line: {err}");
}

#[tokio::test]
async fn test_read_end_line_smaller_than_start_line_with_explicit_num_lines_still_errors() {
    // When num_lines is already provided explicitly, we trust the caller; the
    // end_line < start_line check still applies but the diagnostic doesn't
    // suggest num_lines (the caller already has it right).  We just report the
    // range error normally.
    let dir = temp_dir();
    let tmp = dir.join("test.txt");
    {
        let mut f = std::fs::File::create(&tmp).unwrap();
        for i in 1..=500 {
            writeln!(f, "Line {i}").unwrap();
        }
    }

    let input = read_input(
        tmp.to_str().unwrap(),
        json!({ "start_line": 200, "end_line": 50, "num_lines": 100 }),
    );
    let result = ReadTool.execute(input, &make_ctx(&dir)).await;

    assert!(
        result.is_err(),
        "end_line still takes precedence and is invalid"
    );
}

#[tokio::test]
async fn test_read_end_line_zero_is_error() {
    let dir = temp_dir();
    let tmp = dir.join("test.txt");
    {
        let mut f = std::fs::File::create(&tmp).unwrap();
        writeln!(f, "Line 1").unwrap();
    }

    let input = read_input(
        tmp.to_str().unwrap(),
        json!({ "start_line": 1, "end_line": 0 }),
    );
    let result = ReadTool.execute(input, &make_ctx(&dir)).await;

    assert!(result.is_err());
}

// ── editrenewal T-002: read-timestamp tracking (FR-003) ───────────────────────

/// Reading a file must record its last-modified time in the session
/// `read_timestamps` map so that edit tools can detect stale-file edits.
#[tokio::test]
async fn test_read_records_timestamp() {
    let dir = temp_dir();
    let tmp = dir.join("test.txt");
    {
        let mut f = std::fs::File::create(&tmp).unwrap();
        writeln!(f, "Line 1").unwrap();
    }

    let ctx = make_ctx(&dir);
    assert!(
        ctx.read_timestamps.read().unwrap().is_empty(),
        "timestamp map should start empty"
    );

    let input = read_input(tmp.to_str().unwrap(), json!({}));
    let _ = ReadTool.execute(input, &ctx).await.unwrap();

    let map = ctx.read_timestamps.read().unwrap();
    let canonical = tmp.canonicalize().unwrap_or_else(|_| tmp.to_path_buf());
    let recorded = map.get(&canonical).or_else(|| {
        // The read tool may store the unresolved path; check both.
        map.iter().find(|(p, _)| **p == tmp).map(|(_, v)| v)
    });
    assert!(
        recorded.is_some(),
        "timestamp should be recorded for the read file (map keys: {:?})",
        map.keys().collect::<Vec<_>>()
    );
    let ts = recorded.unwrap();
    assert!(
        *ts > 0,
        "recorded mtime must be a positive millisecond timestamp, got {ts}"
    );
}

/// Reading a file twice must update (not lose) the timestamp entry, and
/// reading two different files must record both.
#[tokio::test]
async fn test_read_timestamp_two_files() {
    let dir = temp_dir();
    let tmp1 = dir.join("alpha.txt");
    let tmp2 = dir.join("beta.txt");
    {
        let mut f = std::fs::File::create(&tmp1).unwrap();
        writeln!(f, "alpha").unwrap();
        let mut f = std::fs::File::create(&tmp2).unwrap();
        writeln!(f, "beta").unwrap();
    }

    let ctx = make_ctx(&dir);
    let _ = ReadTool
        .execute(read_input(tmp1.to_str().unwrap(), json!({})), &ctx)
        .await
        .unwrap();
    let _ = ReadTool
        .execute(read_input(tmp2.to_str().unwrap(), json!({})), &ctx)
        .await
        .unwrap();

    let map = ctx.read_timestamps.read().unwrap();
    assert_eq!(
        map.len(),
        2,
        "two distinct reads should produce two timestamp entries (keys: {:?})",
        map.keys().collect::<Vec<_>>()
    );
}

//! ANTIPAT F-09: the `read` tool must bound the size of a single tool result.
//!
//! These tests build files whose formatted output exceeds the crate's
//! `MAX_READ_OUTPUT_CHARS` budget and assert the returned content (and metadata)
//! reflect the truncation for both the explicit line-range path and the
//! whole/small-file path.

use std::io::Write;
use std::sync::Arc;

use ragent_tools_core::read::ReadTool;
use ragent_tools_core::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

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

/// Create a temp directory under the workspace `target/temp` so the read tool's
/// path-containment check is satisfied.
fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("temp")
        .join(format!(
            "read_cap_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A file whose formatted output for an explicit range exceeds the budget is
/// truncated with an omission marker and reports `metadata.truncated = true`.
#[tokio::test]
async fn test_read_range_output_is_capped() {
    let dir = temp_dir();
    let path = dir.join("wide.txt");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        // 2000 lines x ~200 chars => ~410k formatted chars, above the 200k cap.
        let filler = "x".repeat(200);
        for i in 1..=2000 {
            writeln!(f, "{i} {filler}").unwrap();
        }
    }

    let input = json!({
        "path": path.to_str().unwrap(),
        "start_line": 1,
        "num_lines": 2000,
    });
    let out = ReadTool.execute(input, &make_ctx(&dir)).await.unwrap();

    assert!(
        out.content.contains("output truncated"),
        "expected a truncation marker in the capped output"
    );
    let meta = out.metadata.expect("metadata present");
    assert_eq!(
        meta["truncated"], true,
        "expected metadata.truncated == true, got {meta}"
    );
}

/// A file small enough to fit the budget is returned in full and is not marked
/// truncated.
#[tokio::test]
async fn test_read_small_output_not_capped() {
    let dir = temp_dir();
    let path = dir.join("small.txt");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        for i in 1..=10 {
            writeln!(f, "line {i}").unwrap();
        }
    }

    let input = json!({ "path": path.to_str().unwrap() });
    let out = ReadTool.execute(input, &make_ctx(&dir)).await.unwrap();

    assert!(
        !out.content.contains("output truncated"),
        "small file must not be truncated"
    );
    let meta = out.metadata.expect("metadata present");
    assert!(
        meta.get("truncated").is_none(),
        "small file metadata must not set truncated"
    );
}

/// The whole-file (small-file) path also enforces the cap when a single line is
/// itself larger than the budget.
#[tokio::test]
async fn test_read_whole_file_single_huge_line_is_capped() {
    let dir = temp_dir();
    let path = dir.join("onehugeline.txt");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        // One line of 300k chars => exceeds the 200k cap on the whole-file path.
        writeln!(f, "{}", "y".repeat(300_000)).unwrap();
    }

    let input = json!({ "path": path.to_str().unwrap() });
    let out = ReadTool.execute(input, &make_ctx(&dir)).await.unwrap();

    assert!(
        out.content.contains("output truncated"),
        "single huge line on the whole-file path must be capped"
    );
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["truncated"], true);
}

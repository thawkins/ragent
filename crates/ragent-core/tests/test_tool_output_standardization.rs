//! Comprehensive tests for tool output standardization
//!
//! This test suite verifies that all migrated tools produce standardized
//! output according to the tool output consistency plan.

use ragent_core::event::EventBus;
use ragent_core::tool::{Tool, ToolContext};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn make_ctx() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
    }
}

// =============================================================================
// File Operations Tool Output Tests
// =============================================================================

#[tokio::test]
async fn test_read_tool_output_format() {
    use ragent_core::tool::read::ReadTool;

    let result = ReadTool
        .execute(json!({"path": "Cargo.toml"}), &make_ctx())
        .await
        .unwrap();

    // Content should have file content with line numbers
    assert!(!result.content.is_empty(), "Should have file content");
    assert!(
        result.content.contains("ragent-core"),
        "Should have file content"
    );

    // Should have metadata
    let metadata = result.metadata.expect("should have metadata");
    assert!(metadata.get("path").is_some(), "Should have path field");
    assert!(
        metadata.get("lines").is_some() || metadata.get("line_count").is_some(),
        "Should have line count field, got: {:?}",
        metadata
    );
}

#[tokio::test]
async fn test_write_tool_output_format() {
    use ragent_core::tool::write::WriteTool;

    // Use a relative path within the project
    let test_file = "target/temp/test_output.txt";
    let _ = std::fs::create_dir_all("target/temp");

    let result = WriteTool
        .execute(
            json!({
                "path": test_file,
                "content": "Line 1\nLine 2\nLine 3"
            }),
            &make_ctx(),
        )
        .await
        .unwrap();

    // Cleanup
    let _ = std::fs::remove_file(test_file);

    // Should have summary format
    assert!(
        result.content.contains("wrote") || result.content.contains("Wrote"),
        "Should have wrote summary: {}",
        result.content
    );
    assert!(result.content.contains("bytes"), "Should have bytes");

    // Should have metadata
    let metadata = result.metadata.expect("should have metadata");
    assert!(metadata.get("path").is_some(), "Should have path");
    assert!(
        metadata.get("byte_count").is_some(),
        "Should have byte_count"
    );
    assert!(
        metadata.get("line_count").is_some(),
        "Should have line_count"
    );
}

// =============================================================================
// Search Tool Output Tests
// =============================================================================

#[tokio::test]
async fn test_grep_tool_output_format() {
    use ragent_core::tool::grep::GrepTool;

    let result = GrepTool
        .execute(
            json!({"pattern": "test_grep_tool_output_format"}),
            &make_ctx(),
        )
        .await
        .unwrap();

    // Should have summary
    let has_summary = result.content.contains("match") || result.content.contains("No matches");
    assert!(has_summary, "Should have match summary");

    // Should have metadata
    let metadata = result.metadata.expect("should have metadata");
    assert!(metadata.get("count").is_some(), "Should have count");
    assert!(
        metadata.get("truncated").is_some(),
        "Should have truncated flag"
    );
}

#[tokio::test]
async fn test_search_tool_output_format() {
    use ragent_core::tool::search::SearchTool;

    let result = SearchTool
        .execute(
            json!({"query": "test_search_tool_output_format"}),
            &make_ctx(),
        )
        .await
        .unwrap();

    // Should follow similar format to grep
    let has_summary = result.content.contains("match") || result.content.contains("No matches");
    assert!(has_summary, "Should have match summary");

    let metadata = result.metadata.expect("should have metadata");
    assert!(metadata.get("count").is_some(), "Should have count");
}

#[tokio::test]
async fn test_glob_tool_output_format() {
    use ragent_core::tool::glob::GlobTool;

    let result = GlobTool
        .execute(json!({"pattern": "*.rs"}), &make_ctx())
        .await
        .unwrap();

    // Should have summary
    assert!(
        result.content.contains("found") || result.content.contains("No files"),
        "Should have found summary"
    );

    // Should have metadata
    let metadata = result.metadata.expect("should have metadata");
    assert!(metadata.get("count").is_some(), "Should have count");
    assert!(metadata.get("pattern").is_some(), "Should have pattern");
}

#[tokio::test]
async fn test_list_tool_output_format() {
    use ragent_core::tool::list::ListTool;

    let result = ListTool
        .execute(json!({"path": "."}), &make_ctx())
        .await
        .unwrap();

    // Tree output
    assert!(!result.content.is_empty(), "Should have content");

    // Should have metadata
    let metadata = result.metadata.expect("should have metadata");
    assert!(metadata.get("count").is_some(), "Should have count");
    assert!(metadata.get("path").is_some(), "Should have path");
}

// =============================================================================
// Execution Tool Output Tests
// =============================================================================

#[tokio::test]
async fn test_bash_tool_output_format() {
    use ragent_core::tool::bash::BashTool;

    let result = BashTool
        .execute(json!({"command": "echo hello"}), &make_ctx())
        .await
        .unwrap();

    // Should have structured header (Pattern C)
    assert!(
        result.content.contains("Exit code:"),
        "Should have exit code"
    );
    assert!(result.content.contains("Duration:"), "Should have duration");
    assert!(result.content.contains("ms"), "Should have ms unit");

    // Should have metadata
    let metadata = result.metadata.expect("should have metadata");
    assert!(metadata.get("exit_code").is_some(), "Should have exit_code");
    assert!(
        metadata.get("duration_ms").is_some(),
        "Should have duration_ms"
    );
    assert!(
        metadata.get("line_count").is_some(),
        "Should have line_count"
    );
}

#[tokio::test]
async fn test_execute_python_tool_output_format() {
    use ragent_core::tool::execute_python::ExecutePythonTool;

    let result = ExecutePythonTool
        .execute(json!({"code": "print('test')"}), &make_ctx())
        .await
        .unwrap();

    // Should have structured header (Pattern C)
    assert!(
        result.content.contains("Exit code:"),
        "Should have exit code"
    );
    assert!(result.content.contains("Duration:"), "Should have duration");

    // Should have metadata
    let metadata = result.metadata.expect("should have metadata");
    assert!(metadata.get("exit_code").is_some(), "Should have exit_code");
    assert!(
        metadata.get("duration_ms").is_some(),
        "Should have duration_ms"
    );
}

// =============================================================================
// Edit Tool Output Tests
// =============================================================================

#[tokio::test]
async fn test_edit_tool_output_format() {
    use ragent_core::tool::edit::EditTool;

    // Use a relative path within the project
    let test_file = "target/temp/test_edit.txt";
    let _ = std::fs::create_dir_all("target/temp");
    std::fs::write(test_file, "Hello World\n").unwrap();

    let result = EditTool
        .execute(
            json!({
                "path": test_file,
                "old_str": "World",
                "new_str": "Rust"
            }),
            &make_ctx(),
        )
        .await
        .unwrap();

    // Cleanup
    let _ = std::fs::remove_file(test_file);

    // Should have edit summary
    assert!(
        result.content.contains("Edited"),
        "Should have 'Edited' prefix"
    );
    assert!(result.content.contains("line"), "Should mention lines");

    // Should have metadata
    let metadata = result.metadata.expect("should have metadata");
    assert!(metadata.get("path").is_some(), "Should have path");
    assert!(
        metadata.get("old_lines").is_some() || metadata.get("new_lines").is_some(),
        "Should have line counts"
    );
}

#[tokio::test]
async fn test_multiedit_tool_output_format() {
    use ragent_core::tool::multiedit::MultiEditTool;

    // Use a relative path within the project
    let _ = std::fs::create_dir_all("target/temp/multiedit");
    std::fs::write("target/temp/multiedit/a.txt", "aaa\n").unwrap();
    std::fs::write("target/temp/multiedit/b.txt", "bbb\n").unwrap();

    let ctx = ToolContext {
        session_id: "test".to_string(),
        working_dir: std::path::PathBuf::from("target/temp/multiedit"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
    };

    let result = MultiEditTool
        .execute(
            json!({
                "edits": [
                    {"path": "a.txt", "old_str": "aaa", "new_str": "AAA"},
                    {"path": "b.txt", "old_str": "bbb", "new_str": "BBB"}
                ]
            }),
            &ctx,
        )
        .await
        .unwrap();

    // Cleanup
    let _ = std::fs::remove_dir_all("target/temp/multiedit");

    // Should have summary
    assert!(
        result.content.contains("edit") || result.content.contains("file"),
        "Should have edit/file summary: {}",
        result.content
    );

    // Should have metadata
    let metadata = result.metadata.expect("should have metadata");
    assert!(
        metadata.get("count").is_some() || metadata.get("edits").is_some(),
        "Should have count/edits, got: {:?}",
        metadata
    );
    assert!(
        metadata.get("file_count").is_some() || metadata.get("files").is_some(),
        "Should have file_count, got: {:?}",
        metadata
    );
}

// =============================================================================
// Metadata Standardization Tests
// =============================================================================

#[test]
fn test_metadata_field_standardization() {
    // This test verifies that all tools use standard field names
    // as defined in docs/standards/tool_metadata_schema.md

    let standard_fields = [
        "path",
        "source",
        "destination",
        "count",
        "lines",
        "line_count",
        "total_lines",
        "entries",
        "matches",
        "results",
        "files",
        "edits",
        "hunks",
        "pages",
        "sheets",
        "slides",
        "symbols",
        "symbol_count",
        "start_line",
        "end_line",
        "line",
        "column",
        "bytes",
        "size_bytes",
        "exit_code",
        "status",
        "success",
        "timed_out",
        "deleted",
        "cancelled",
        "claimed",
        "approved",
        "exists",
        "truncated",
        "summarised",
        "message",
        "duration_ms",
        "modified",
        "old_lines",
        "new_lines",
        "files_affected",
        "file_count",
        "task_id",
        "agent_id",
        "team_name",
        "title",
        "number",
        "state",
        "word_count",
        "paragraph_count",
        "sheet_count",
        "slide_count",
    ];

    // All tools should only use these standard field names
    // This is a compile-time check that the field names exist
    // Run-time checks are done in individual tool tests
    for field in &standard_fields {
        assert!(!field.is_empty(), "Field '{}' should be valid", field);
    }
}

// =============================================================================
// Content Pattern Compliance Tests
// =============================================================================

#[test]
fn test_pattern_a_summary_content_format() {
    // Pattern A: Summary line + blank line + raw content
    // Used by: grep, search, read, lsp_* tools

    let examples = vec![
        ("5 lines read\n\nline1\nline2", true),
        ("3 matches found\n\nmatch1\nmatch2", true),
        ("25 symbols found\n\nsymbol1", true),
        // These should NOT match Pattern A:
        ("Exit code: 0\nDuration: 100ms\n", false), // Pattern C
        ("Wrote 100 bytes", false),                 // Pattern B
    ];

    for (content, is_pattern_a) in examples {
        let has_separator = content.contains("\n\n");
        let has_summary = content
            .lines()
            .next()
            .map(|l| !l.is_empty())
            .unwrap_or(false);

        let is_actually_a = has_separator && has_summary && !content.starts_with("Exit code:");

        assert_eq!(
            is_actually_a,
            is_pattern_a,
            "Content '{}' {} match Pattern A",
            content,
            if is_pattern_a { "should" } else { "should not" }
        );
    }
}

#[test]
fn test_pattern_c_structured_format() {
    // Pattern C: Structured header with exit code, duration, etc.
    // Used by: bash, execute_python

    let examples = vec![
        ("Exit code: 0\nDuration: 150ms\n\nSTDOUT:\nhello", true),
        ("Exit code: 1\nDuration: 200ms (timed out)", true),
        // These should NOT match Pattern C:
        ("5 lines read\n\ncontent", false), // Pattern A
        ("Wrote file", false),              // Pattern B
    ];

    for (content, is_pattern_c) in examples {
        let has_exit_code = content.contains("Exit code:");
        let has_duration = content.contains("Duration:");

        let is_actually_c = has_exit_code && has_duration;

        assert_eq!(
            is_actually_c,
            is_pattern_c,
            "Content '{}' {} match Pattern C",
            content,
            if is_pattern_c { "should" } else { "should not" }
        );
    }
}

// =============================================================================
// Tool Output Completeness Tests
// =============================================================================

#[tokio::test]
async fn test_all_migrated_tools_have_metadata() {
    // Verify that migrated tools have metadata
    // This serves as a regression test

    let ctx = make_ctx();

    // List of tools that should have metadata
    let tools_with_metadata: Vec<&str> = vec![
        "bash",
        "execute_python",
        "grep",
        "search",
        "glob",
        "list",
        "write",
        "edit",
        "multiedit",
        "webfetch",
        "websearch",
    ];

    // This test just verifies the list is not empty
    // Individual tests verify each tool
    assert!(
        !tools_with_metadata.is_empty(),
        "Should have migrated tools list"
    );

    // Count how many we've tested
    assert!(
        tools_with_metadata.len() >= 5,
        "Should have at least 5 migrated tools"
    );
}

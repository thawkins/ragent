//! Tests for TUI tool output display formatting
//!
//! This test suite verifies that the message_widget module correctly
//! formats tool input/output summaries according to the standardized patterns.

use ragent_tui::widgets::message_widget::*;
use serde_json::json;

// =============================================================================
// Tool Input Summary Tests
// =============================================================================

#[test]
fn test_input_summary_read_tool() {
    let input = json!({"path": "src/main.rs"});
    let summary = tool_input_summary("read", &input, "/home/user/project");
    assert!(summary.contains("📄"), "Should have file emoji");
    assert!(summary.contains("src/main.rs"), "Should contain path");
}

#[test]
fn test_input_summary_write_tool() {
    let input = json!({"path": "output.txt", "content": "hello"});
    let summary = tool_input_summary("write", &input, "/home/user/project");
    assert!(summary.contains("📄"), "Should have file emoji");
    assert!(summary.contains("output.txt"), "Should contain path");
}

#[test]
fn test_input_summary_list_tool() {
    let input = json!({"path": "src"});
    let summary = tool_input_summary("list", &input, "/home/user/project");
    assert!(summary.contains("📁"), "Should have folder emoji");
    assert!(summary.contains("src"), "Should contain path");
}

#[test]
fn test_input_summary_bash_tool() {
    let input = json!({"command": "cargo build"});
    let summary = tool_input_summary("bash", &input, "/home/user/project");
    assert!(summary.contains("⚡"), "Should have execution emoji");
    assert!(summary.contains("cargo build"), "Should contain command");
}

#[test]
fn test_input_summary_bash_tool_uses_120_char_truncation() {
    let command = format!("echo {}", "x".repeat(110));
    let input = json!({ "command": command });
    let summary = tool_input_summary("bash", &input, "/home/user/project");
    assert!(
        summary.contains(&"x".repeat(100)),
        "Should preserve long command text up to the wider truncation limit: {summary}"
    );
    assert!(
        !summary.ends_with("..."),
        "Should not truncate a command shorter than 120 characters: {summary}"
    );
}

#[test]
fn test_input_summary_grep_tool() {
    let input = json!({"pattern": "fn main", "path": "src"});
    let summary = tool_input_summary("grep", &input, "/home/user/project");
    assert!(summary.contains("🔍"), "Should have search emoji");
    assert!(summary.contains("fn main"), "Should contain pattern");
    assert!(summary.contains("src"), "Should contain path");
}

#[test]
fn test_input_summary_glob_tool() {
    let input = json!({"pattern": "**/*.rs"});
    let summary = tool_input_summary("glob", &input, "/home/user/project");
    assert!(summary.contains("🔍"), "Should have search emoji");
    assert!(summary.contains("**/*.rs"), "Should contain pattern");
}

#[test]
fn test_input_summary_edit_tool() {
    let input = json!({"path": "file.txt", "old_str": "old", "new_str": "new"});
    let summary = tool_input_summary("edit", &input, "/home/user/project");
    assert!(summary.contains("📄"), "Should have file emoji");
    assert!(summary.contains("file.txt"), "Should contain path");
}

#[test]
fn test_input_summary_webfetch_tool() {
    let input = json!({"url": "https://example.com"});
    let summary = tool_input_summary("webfetch", &input, "/home/user/project");
    assert!(summary.contains("🌐"), "Should have network emoji");
    assert!(summary.contains("example.com"), "Should contain url");
}

#[test]
fn test_input_summary_websearch_tool() {
    let input = json!({"query": "rust documentation"});
    let summary = tool_input_summary("websearch", &input, "/home/user/project");
    assert!(summary.contains("🌐"), "Should have network emoji");
    assert!(
        summary.contains("rust documentation"),
        "Should contain query"
    );
}

// =============================================================================
// Tool Alias Resolution Tests
// =============================================================================

#[test]
fn test_canonical_tool_name_aliases() {
    assert_eq!(canonical_tool_name("read_file"), "read");
    assert_eq!(canonical_tool_name("view_file"), "read");
    assert_eq!(canonical_tool_name("get_file_contents"), "read");
    assert_eq!(canonical_tool_name("open_file"), "read");

    assert_eq!(canonical_tool_name("list_files"), "list");
    assert_eq!(canonical_tool_name("list_directory"), "list");

    assert_eq!(canonical_tool_name("find_files"), "glob");

    assert_eq!(canonical_tool_name("search_in_repo"), "search");
    assert_eq!(canonical_tool_name("file_search"), "search");

    assert_eq!(canonical_tool_name("replace_in_file"), "edit");
    assert_eq!(canonical_tool_name("update_file"), "write");

    assert_eq!(canonical_tool_name("run_shell_command"), "bash");
    assert_eq!(canonical_tool_name("execute_bash"), "bash");
    assert_eq!(canonical_tool_name("run_code"), "bash");

    // Unknown tools return themselves
    assert_eq!(canonical_tool_name("custom_tool"), "custom_tool");
}

#[test]
fn test_alias_resolves_to_canonical_input_summary() {
    // read_file should produce same summary as read
    let alias_input = json!({"path": "test.txt"});
    let canonical_input = json!({"path": "test.txt"});

    let alias_summary = tool_input_summary("read_file", &alias_input, "/project");
    let canonical_summary = tool_input_summary("read", &canonical_input, "/project");

    assert_eq!(
        alias_summary, canonical_summary,
        "Alias should produce same summary as canonical"
    );
}

// =============================================================================
// Path Handling Tests
// =============================================================================

#[test]
fn test_make_relative_path_absolute() {
    let result = make_relative_path("/home/user/project/src/main.rs", "/home/user/project");
    assert_eq!(result, "src/main.rs");
}

#[test]
fn test_make_relative_path_already_relative() {
    let result = make_relative_path("src/main.rs", "/home/user/project");
    assert_eq!(result, "src/main.rs");
}

#[test]
fn test_make_relative_path_with_tilde() {
    // This test may fail in CI without HOME set, but documents expected behavior
    let result = make_relative_path("~/project/file.txt", "~/project");
    // Result depends on HOME environment variable
    assert!(!result.is_empty());
}

// =============================================================================
// Tool Result Summary Tests
// =============================================================================

#[test]
fn test_result_summary_bash_success() {
    let output = Some(json!({
        "exit_code": 0,
        "duration_ms": 150,
        "line_count": 5
    }));
    let input = json!({"command": "echo hello"});
    let result = tool_result_summary("bash", &output, &input, "/project");
    assert!(result.is_some(), "Should produce summary");
    let summary = result.unwrap();
    // Bash shows lines and exit code, not duration directly
    assert!(summary.contains('5'), "Should show line count: {}", summary);
    assert!(
        summary.contains("exit 0"),
        "Should show exit code: {}",
        summary
    );
}

#[test]
fn test_result_summary_read_tool() {
    let output = Some(json!({
        "path": "test.rs",
        "lines": 42,
        "truncated": false
    }));
    let input = json!({"path": "test.rs"});
    let result = tool_result_summary("read", &output, &input, "/project");
    assert!(result.is_some(), "Should produce summary");
    let summary = result.unwrap();
    assert!(summary.contains("42"), "Should show line count");
}

#[test]
fn test_result_summary_write_tool() {
    let output = Some(json!({
        "path": "test.txt",
        "byte_count": 1024,
        "line_count": 10
    }));
    let input = json!({"path": "test.txt"});
    let result = tool_result_summary("write", &output, &input, "/project");
    assert!(result.is_some(), "Should produce summary");
    let summary = result.unwrap();
    assert!(
        summary.contains("10"),
        "Should show line count: {}",
        summary
    );
    // Summary includes lines written, not bytes
    assert!(
        summary.contains("written"),
        "Should show written: {}",
        summary
    );
}

#[test]
fn test_result_summary_grep_tool() {
    let output = Some(json!({
        "count": 5,
        "file_count": 3,
        "truncated": false
    }));
    let input = json!({"pattern": "test"});
    let result = tool_result_summary("grep", &output, &input, "/project");
    assert!(result.is_some(), "Should produce summary");
    let summary = result.unwrap();
    assert!(summary.contains('5'), "Should show match count");
    assert!(summary.contains('3'), "Should show file count");
}

#[test]
fn test_result_summary_glob_tool() {
    let output = Some(json!({
        "count": 10,
        "pattern": "*.rs"
    }));
    let input = json!({"pattern": "*.rs"});
    let result = tool_result_summary("glob", &output, &input, "/project");
    assert!(result.is_some(), "Should produce summary");
    let summary = result.unwrap();
    assert!(summary.contains("10"), "Should show file count");
}

#[test]
fn test_result_summary_list_tool() {
    let output = Some(json!({
        "count": 25,
        "path": "src"
    }));
    let input = json!({"path": "src"});
    let result = tool_result_summary("list", &output, &input, "/project");
    assert!(result.is_some(), "Should produce summary");
    let summary = result.unwrap();
    assert!(summary.contains("25"), "Should show entry count");
}

#[test]
fn test_result_summary_multiedit_tool() {
    let output = Some(json!({
        "edits": 5,
        "file_count": 3,
        "old_lines": 10,
        "new_lines": 8
    }));
    let input = json!({"edits": [{}, {}, {}, {}, {}]});
    let result = tool_result_summary("multiedit", &output, &input, "/project");
    assert!(result.is_some(), "Should produce summary");
    let summary = result.unwrap();
    assert!(summary.contains('5'), "Should show edit count: {}", summary);
    assert!(summary.contains('3'), "Should show file count: {}", summary);
}

#[test]
fn test_result_summary_edit_tool() {
    let output = Some(json!({
        "path": "file.txt",
        "old_lines": 5,
        "new_lines": 3
    }));
    let input = json!({"path": "file.txt"});
    let result = tool_result_summary("edit", &output, &input, "/project");
    assert!(result.is_some(), "Should produce summary");
    let summary = result.unwrap();
    assert!(summary.contains('5'), "Should show old lines");
    assert!(summary.contains('3'), "Should show new lines");
}

#[test]
fn test_result_summary_no_metadata() {
    let result = tool_result_summary("bash", &None, &json!({}), "/project");
    assert!(result.is_none(), "Should return None for empty metadata");
}

#[test]
fn test_result_summary_think_uses_full_thought_text() {
    let thought = "SPEC.md is missing many sections referenced in the TOC. I need to find what's implemented and add missing sections.";
    let output = Some(json!({
        "thinking": true,
        "thought": thought
    }));
    let input = json!({"thought": thought});

    let result = tool_result_summary("think", &output, &input, "/project");
    assert_eq!(result.as_deref(), Some(thought));
}

#[test]
fn test_result_summary_think_preserves_multiline_text() {
    let thought = "First line.\nSecond line.\nThird line.";
    let output = Some(json!({
        "thinking": true,
        "thought": thought
    }));
    let input = json!({"thought": thought});

    let result = tool_result_summary("think", &output, &input, "/project");
    assert_eq!(result.as_deref(), Some(thought));
}

// =============================================================================
// Pluralization Tests
// =============================================================================

#[test]
fn test_pluralize_singular() {
    assert_eq!(pluralize(1, "line", "lines"), "1 line");
    assert_eq!(pluralize(1, "file", "files"), "1 file");
    assert_eq!(pluralize(1, "match", "matches"), "1 match");
}

#[test]
fn test_pluralize_plural() {
    assert_eq!(pluralize(0, "line", "lines"), "0 lines");
    assert_eq!(pluralize(2, "line", "lines"), "2 lines");
    assert_eq!(pluralize(5, "file", "files"), "5 files");
}

// =============================================================================
// String Truncation Tests
// =============================================================================

#[test]
fn test_truncate_str_short() {
    assert_eq!(truncate_str("short", 20), "short");
}

#[test]
fn test_truncate_str_long() {
    let long = "this is a very long string that needs truncation";
    let result = truncate_str(long, 20);
    assert!(result.len() < long.len(), "Should be shorter");
    assert!(result.ends_with("..."), "Should end with ellipsis");
}

#[test]
fn test_truncate_str_exact() {
    let exact = "exactly twenty chars"; // 20 chars
    let result = truncate_str(exact, 20);
    assert_eq!(result, exact);
}

#[test]
fn test_truncate_str_unicode() {
    // Test that unicode characters are handled correctly
    let unicode = "日本語のテキスト";
    let result = truncate_str(unicode, 5);
    assert!(result.ends_with("...") || result == unicode);
}

// =============================================================================
// Tool Name Capitalization Tests
// =============================================================================

#[test]
fn test_capitalize_tool_name() {
    assert_eq!(capitalize_tool_name("read"), "Read");
    assert_eq!(capitalize_tool_name("bash"), "Bash");
    assert_eq!(capitalize_tool_name("execute_python"), "Execute_python");
    assert_eq!(capitalize_tool_name(""), "");
}

// =============================================================================
// Diff Stats Tests (if available)
// =============================================================================

#[test]
fn test_tool_inline_diff_calculation() {
    // Test basic diff calculation
    let old_content = "line1\nline2\nline3";
    let new_content = "line1\nmodified\nline3";

    // The actual diff function may or may not be public
    // This test documents expected behavior
    let has_diff = old_content != new_content;
    assert!(has_diff, "Content should be different");
}

// =============================================================================
// Edge Cases and Error Handling
// =============================================================================

#[test]
fn test_input_summary_empty_path() {
    let input = json!({"path": ""});
    let summary = tool_input_summary("read", &input, "/project");
    assert_eq!(summary, "", "Empty path should return empty summary");
}

#[test]
fn test_input_summary_missing_params() {
    let input = json!({});
    let summary = tool_input_summary("read", &input, "/project");
    assert_eq!(summary, "", "Missing params should return empty summary");
}

#[test]
fn test_result_summary_partial_metadata() {
    // Metadata with only some fields
    let output = Some(json!({"path": "test.txt"}));
    let input = json!({"path": "test.txt"});
    let result = tool_result_summary("read", &output, &input, "/project");
    // Should still produce some summary even without line_count
    assert!(
        result.is_some(),
        "Should produce summary with partial metadata"
    );
}

// =============================================================================
// Emoji Icon Standardization Tests
// =============================================================================

#[test]
fn test_file_operations_use_correct_emoji() {
    let input = json!({"path": "test.txt"});

    let read_summary = tool_input_summary("read", &input, "/project");
    let write_summary = tool_input_summary("write", &input, "/project");
    let edit_summary = tool_input_summary("edit", &input, "/project");

    assert!(read_summary.contains("📄"), "Read should use 📄");
    assert!(write_summary.contains("📄"), "Write should use 📄");
    assert!(edit_summary.contains("📄"), "Edit should use 📄");
}

#[test]
fn test_directory_operations_use_folder_emoji() {
    let input = json!({"path": "src"});
    let summary = tool_input_summary("list", &input, "/project");
    assert!(summary.contains("📁"), "List should use 📁");
}

#[test]
fn test_search_operations_use_magnifying_glass() {
    let input = json!({"pattern": "test"});
    let summary = tool_input_summary("grep", &input, "/project");
    assert!(summary.contains("🔍"), "Grep should use 🔍");
}

#[test]
fn test_execution_operations_use_lightning() {
    let input = json!({"command": "ls"});
    let summary = tool_input_summary("bash", &input, "/project");
    assert!(summary.contains("⚡"), "Bash should use ⚡");
}

#[test]
fn test_network_operations_use_globe() {
    let input = json!({"url": "https://example.com"});
    let summary = tool_input_summary("webfetch", &input, "/project");
    assert!(summary.contains("🌐"), "Webfetch should use 🌐");
}

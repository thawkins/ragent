//! Tests for the task completeness detection logic in the session processor.
//!
//! Validates that `detect_incomplete_file_task` correctly identifies when a
//! user requested file creation but no file-writing tool was executed.

use ragent_core::message::{MessagePart, ToolCallState, ToolCallStatus};
use ragent_core::session::processor::detect_incomplete_file_task;
use serde_json::json;

/// Helper to create a ToolCall MessagePart with the given tool name.
fn tool_call_part(tool: &str) -> MessagePart {
    MessagePart::ToolCall {
        tool: tool.to_string(),
        call_id: "call_1".to_string(),
        state: ToolCallState {
            status: ToolCallStatus::Completed,
            input: json!({}),
            output: Some(json!({"ok": true})),
            error: None,
            duration_ms: Some(100),
        },
    }
}

// --- Positive cases: should detect incomplete task ---

#[test]
fn test_detect_create_md_file_no_tools() {
    let parts: Vec<MessagePart> = vec![];
    assert!(detect_incomplete_file_task(
        "produce a plan called hugplan.md for adding a huggingface provider",
        &parts,
    ));
}

#[test]
fn test_detect_create_file_explicit() {
    let parts: Vec<MessagePart> = vec![];
    assert!(detect_incomplete_file_task(
        "create a file called output.txt with the results",
        &parts,
    ));
}

#[test]
fn test_detect_write_json_file() {
    let parts: Vec<MessagePart> = vec![];
    assert!(detect_incomplete_file_task(
        "write the configuration to config.json",
        &parts,
    ));
}

#[test]
fn test_detect_generate_rs_file() {
    let parts: Vec<MessagePart> = vec![];
    assert!(detect_incomplete_file_task(
        "generate a new module in provider.rs",
        &parts,
    ));
}

#[test]
fn test_detect_save_file() {
    let parts: Vec<MessagePart> = vec![];
    assert!(detect_incomplete_file_task(
        "save the report to report.pdf",
        &parts,
    ));
}

#[test]
fn test_no_detect_extensionless_file() {
    // "Makefile" has no extension — the detection is conservative and
    // only triggers on word.ext patterns to avoid false positives.
    let parts: Vec<MessagePart> = vec![];
    assert!(!detect_incomplete_file_task(
        "make a Makefile for this project",
        &parts,
    ));
}

#[test]
fn test_detect_with_path() {
    let parts: Vec<MessagePart> = vec![];
    assert!(detect_incomplete_file_task(
        "create docs/architecture.md documenting the system",
        &parts,
    ));
}

// --- Negative cases: should NOT detect incomplete task ---

#[test]
fn test_no_detect_when_write_tool_used() {
    let parts = vec![tool_call_part("write_file")];
    assert!(!detect_incomplete_file_task(
        "create a file called output.txt",
        &parts,
    ));
}

#[test]
fn test_no_detect_when_create_tool_used() {
    let parts = vec![tool_call_part("create_file")];
    assert!(!detect_incomplete_file_task(
        "produce a plan called plan.md",
        &parts,
    ));
}

#[test]
fn test_no_detect_when_edit_tool_used() {
    let parts = vec![tool_call_part("edit_file")];
    assert!(!detect_incomplete_file_task(
        "write changes to main.rs",
        &parts,
    ));
}

#[test]
fn test_no_detect_no_filename() {
    let parts: Vec<MessagePart> = vec![];
    assert!(!detect_incomplete_file_task(
        "create a plan for adding a huggingface provider",
        &parts,
    ));
}

#[test]
fn test_no_detect_no_action_verb() {
    let parts: Vec<MessagePart> = vec![];
    assert!(!detect_incomplete_file_task(
        "explain the contents of hugplan.md",
        &parts,
    ));
}

#[test]
fn test_no_detect_question_about_file() {
    let parts: Vec<MessagePart> = vec![];
    assert!(!detect_incomplete_file_task(
        "what does config.json contain?",
        &parts,
    ));
}

#[test]
fn test_no_detect_read_file() {
    let parts: Vec<MessagePart> = vec![];
    assert!(!detect_incomplete_file_task(
        "read the file main.rs and summarize it",
        &parts,
    ));
}

#[test]
fn test_no_detect_empty_message() {
    let parts: Vec<MessagePart> = vec![];
    assert!(!detect_incomplete_file_task("", &parts));
}

// --- Edge cases ---

#[test]
fn test_detect_mixed_case() {
    let parts: Vec<MessagePart> = vec![];
    assert!(detect_incomplete_file_task(
        "Create a file called README.md",
        &parts,
    ));
}

#[test]
fn test_detect_with_non_write_tools_present() {
    // read_file and bash are NOT write tools, so task is still incomplete
    let parts = vec![
        tool_call_part("read_file"),
        tool_call_part("bash"),
        tool_call_part("new_task"),
    ];
    assert!(detect_incomplete_file_task(
        "produce a plan called hugplan.md",
        &parts,
    ));
}

#[test]
fn test_no_detect_with_append_tool() {
    let parts = vec![tool_call_part("append_file")];
    assert!(!detect_incomplete_file_task(
        "write results to output.log",
        &parts,
    ));
}

#[test]
fn test_detect_filename_with_hyphens() {
    let parts: Vec<MessagePart> = vec![];
    assert!(detect_incomplete_file_task(
        "create my-awesome-plan.md for the feature",
        &parts,
    ));
}

#[test]
fn test_detect_filename_with_underscores() {
    let parts: Vec<MessagePart> = vec![];
    assert!(detect_incomplete_file_task(
        "generate test_helpers.rs with utility functions",
        &parts,
    ));
}

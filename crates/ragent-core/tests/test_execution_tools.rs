//! Tests for ExecutePythonTool and CalculatorTool — verify metadata standardization.

use ragent_core::event::EventBus;
use ragent_core::tool::{Tool, ToolContext};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn make_ctx() -> ToolContext {
    ToolContext {
        session_id: "test-execution".to_string(),
        working_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    }
}

// ── Execute Python Tool Tests ────────────────────────────────────

#[tokio::test]
async fn test_execute_python_success() {
    let tool = ragent_core::tool::execute_python::ExecutePythonTool;
    let result = tool
        .execute(json!({"code": "print('hello')"}), &make_ctx())
        .await;
    assert!(result.is_ok());
    let output = result.unwrap();
    let content = output.content;
    assert!(
        content.contains("Exit code: 0"),
        "Should contain exit code: {content}"
    );
    assert!(
        content.contains("hello"),
        "Should contain output: {content}"
    );
    assert!(
        content.contains("Duration:") && content.contains("ms"),
        "Should contain duration: {content}"
    );

    // Verify metadata fields
    let meta = output.metadata.expect("should have metadata");
    let obj = meta.as_object().expect("should be object");
    assert!(
        obj.contains_key("exit_code"),
        "metadata should contain exit_code"
    );
    assert!(
        obj.contains_key("duration_ms"),
        "metadata should contain duration_ms"
    );
    assert!(
        obj.contains_key("line_count"),
        "metadata should contain line_count"
    );
    assert_eq!(
        obj.get("exit_code").and_then(|v| v.as_i64()),
        Some(0),
        "exit_code should be 0"
    );
}

#[tokio::test]
async fn test_execute_python_stderr_output() {
    let tool = ragent_core::tool::execute_python::ExecutePythonTool;
    let result = tool
        .execute(
            json!({"code": "import sys; sys.stderr.write('error message')"}),
            &make_ctx(),
        )
        .await;
    assert!(result.is_ok());
    let output = result.unwrap();
    let content = output.content;
    assert!(
        content.contains("[stderr]"),
        "Should contain stderr section: {content}"
    );
    assert!(
        content.contains("error message"),
        "Should contain error: {content}"
    );

    // Verify metadata has standard fields
    let meta = output.metadata.expect("should have metadata");
    let obj = meta.as_object().expect("should be object");
    assert!(
        obj.contains_key("exit_code"),
        "metadata should contain exit_code"
    );
    assert!(
        obj.contains_key("duration_ms"),
        "metadata should contain duration_ms"
    );
    assert!(
        obj.contains_key("line_count"),
        "metadata should contain line_count"
    );
}

#[tokio::test]
async fn test_execute_python_empty_output() {
    let tool = ragent_core::tool::execute_python::ExecutePythonTool;
    let result = tool.execute(json!({"code": "pass"}), &make_ctx()).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    let content = output.content;
    // Should have "(no output)" when stdout is empty
    assert!(
        content.contains("(no output)"),
        "Should indicate no output: {content}"
    );

    let meta = output.metadata.expect("should have metadata");
    let obj = meta.as_object().expect("should be object");
    assert!(
        obj.contains_key("line_count"),
        "metadata should contain line_count"
    );
}

#[tokio::test]
async fn test_execute_python_error_exit_code() {
    let tool = ragent_core::tool::execute_python::ExecutePythonTool;
    let result = tool
        .execute(json!({"code": "import sys; sys.exit(42)"}), &make_ctx())
        .await;
    assert!(result.is_ok());
    let output = result.unwrap();
    let content = output.content;
    assert!(
        content.contains("Exit code: 42"),
        "Should contain exit code 42: {content}"
    );

    let meta = output.metadata.expect("should have metadata");
    let obj = meta.as_object().expect("should be object");
    assert_eq!(
        obj.get("exit_code").and_then(|v| v.as_i64()),
        Some(42),
        "exit_code should be 42"
    );
}

// ── Calculator Tool Tests ────────────────────────────────────────

#[tokio::test]
async fn test_calculator_simple_expression() {
    let tool = ragent_core::tool::calculator::CalculatorTool;
    let result = tool
        .execute(json!({"expression": "2 + 3"}), &make_ctx())
        .await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(
        output.content.contains("2 + 3 = 5"),
        "Should show result: {}",
        output.content
    );

    // Verify metadata fields
    let meta = output.metadata.expect("should have metadata");
    let obj = meta.as_object().expect("should be object");
    assert!(
        obj.contains_key("expression"),
        "metadata should contain expression"
    );
    assert!(obj.contains_key("result"), "metadata should contain result");
    assert_eq!(
        obj.get("expression").and_then(|v| v.as_str()),
        Some("2 + 3"),
        "expression should be '2 + 3'"
    );
    assert_eq!(
        obj.get("result").and_then(|v| v.as_str()),
        Some("5"),
        "result should be '5'"
    );
}

#[tokio::test]
async fn test_calculator_math_module() {
    let tool = ragent_core::tool::calculator::CalculatorTool;
    let result = tool
        .execute(json!({"expression": "math.sqrt(16)"}), &make_ctx())
        .await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(
        output.content.contains("4.0"),
        "Should contain sqrt result: {}",
        output.content
    );

    let meta = output.metadata.expect("should have metadata");
    let obj = meta.as_object().expect("should be object");
    assert_eq!(
        obj.get("expression").and_then(|v| v.as_str()),
        Some("math.sqrt(16)"),
        "expression should match"
    );
}

#[tokio::test]
async fn test_calculator_factorial() {
    let tool = ragent_core::tool::calculator::CalculatorTool;
    let result = tool
        .execute(json!({"expression": "math.factorial(5)"}), &make_ctx())
        .await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(
        output.content.contains("120"),
        "Should contain factorial result: {}",
        output.content
    );
}

#[tokio::test]
async fn test_calculator_complex_numbers() {
    let tool = ragent_core::tool::calculator::CalculatorTool;
    let result = tool
        .execute(json!({"expression": "(3+4j) * 2"}), &make_ctx())
        .await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(
        output.content.contains('6') && output.content.contains('8'),
        "Should contain complex number result: {}",
        output.content
    );
}

#[tokio::test]
async fn test_calculator_missing_param() {
    let tool = ragent_core::tool::calculator::CalculatorTool;
    let result = tool.execute(json!({}), &make_ctx()).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("expression") || err.contains("Missing"),
        "Should error about missing expression: {err}"
    );
}

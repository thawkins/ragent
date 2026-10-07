//! Integration tests for the `ragent-tools-core` `CalculatorTool` and its
//! recursive-descent `evaluate_expression` parser (audit T-701).
//!
//! `CalculatorTool` and `evaluate_expression` are public, so no `#[path]`
//! re-import is needed.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use ragent_tools_core::calculator::{CalculatorTool, evaluate_expression};
use ragent_tools_core::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

fn make_ctx() -> ToolContext {
    let dir = PathBuf::from("target/temp");
    ToolContext {
        session_id: "calculator-test".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(EventBus::new(16)),
        read_timestamps: Arc::new(RwLock::new(HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir],
    }
}

// ---------------------------------------------------------------------------
// Parser-level happy paths: precedence, associativity, constants
// ---------------------------------------------------------------------------

#[test]
fn test_precedence_multiplication_before_addition() {
    // 2 + 3 * 4 => 14, not 20.
    assert_eq!(evaluate_expression("2 + 3 * 4").unwrap(), 14.0);
    assert_eq!(evaluate_expression("2 * 3 + 4").unwrap(), 10.0);
}

#[test]
fn test_left_associative_subtraction() {
    // 10 - 3 - 2 => 5 (left-associative), not 11.
    assert_eq!(evaluate_expression("10 - 3 - 2").unwrap(), 5.0);
}

#[test]
fn test_parentheses_override_precedence() {
    assert_eq!(evaluate_expression("(2 + 3) * 4").unwrap(), 20.0);
    assert_eq!(evaluate_expression("2 ^ (2 + 1)").unwrap(), 8.0);
}

#[test]
fn test_power_is_right_associative() {
    // 2 ^ 3 ^ 2 => 2^(3^2) = 2^9 = 512, not (2^3)^2 = 64.
    assert_eq!(evaluate_expression("2 ^ 3 ^ 2").unwrap(), 512.0);
}

#[test]
fn test_unary_minus_and_modulo() {
    assert_eq!(evaluate_expression("-5 + 3").unwrap(), -2.0);
    assert_eq!(evaluate_expression("10 % 3").unwrap(), 1.0);
    assert_eq!(evaluate_expression("--4").unwrap(), 4.0);
}

#[test]
fn test_constants_pi_and_e() {
    assert!((evaluate_expression("pi").unwrap() - std::f64::consts::PI).abs() < f64::EPSILON);
    assert!((evaluate_expression("e").unwrap() - std::f64::consts::E).abs() < f64::EPSILON);
    assert!((evaluate_expression("tau").unwrap() - std::f64::consts::TAU).abs() < f64::EPSILON);
}

#[test]
fn test_whitelisted_functions() {
    assert_eq!(evaluate_expression("sqrt(16)").unwrap(), 4.0);
    assert_eq!(evaluate_expression("abs(-7)").unwrap(), 7.0);
    assert_eq!(evaluate_expression("floor(3.9)").unwrap(), 3.0);
    assert_eq!(evaluate_expression("ceil(3.1)").unwrap(), 4.0);
    assert_eq!(evaluate_expression("round(3.5)").unwrap(), 4.0);
    assert_eq!(evaluate_expression("pow(2, 10)").unwrap(), 1024.0);
    assert_eq!(evaluate_expression("min(3, 7, 1)").unwrap(), 1.0);
    assert_eq!(evaluate_expression("max(3, 7, 1)").unwrap(), 7.0);
    assert_eq!(evaluate_expression("log2(8)").unwrap(), 3.0);
    assert_eq!(evaluate_expression("ln(e)").unwrap(), 1.0);
}

// ---------------------------------------------------------------------------
// Divide-by-zero and undefined maths: the evaluator produces inf/NaN rather
// than an error (documented behaviour).
// ---------------------------------------------------------------------------

#[test]
fn test_divide_by_zero_produces_infinity_not_error() {
    let value = evaluate_expression("1 / 0").unwrap();
    assert!(value.is_infinite(), "1/0 must be infinite, got {value}");
    assert!(value.is_sign_positive());
    assert!(evaluate_expression("1 / 0").is_ok());
}

#[test]
fn test_zero_divided_by_zero_is_nan() {
    let value = evaluate_expression("0 / 0").unwrap();
    assert!(value.is_nan(), "0/0 must be NaN, got {value}");
}

// ---------------------------------------------------------------------------
// Malformed input: each path must return a clear error, not panic.
// ---------------------------------------------------------------------------

#[test]
fn test_unexpected_character_is_rejected() {
    let err = evaluate_expression("1 + $").unwrap_err().to_string();
    assert!(
        err.contains("unexpected character '$'"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_empty_expression_is_rejected() {
    let err = evaluate_expression("").unwrap_err().to_string();
    assert!(
        err.contains("unexpected end of expression"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_trailing_input_is_rejected() {
    let err = evaluate_expression("1 2").unwrap_err().to_string();
    assert!(
        err.contains("unexpected trailing input"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_missing_closing_paren_is_rejected() {
    let err = evaluate_expression("(1 + 2").unwrap_err().to_string();
    assert!(
        err.contains("missing closing ')'"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_unknown_bare_name_is_rejected() {
    let err = evaluate_expression("foo").unwrap_err().to_string();
    assert!(
        err.contains("unknown name 'foo'"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_unknown_function_is_rejected() {
    let err = evaluate_expression("nope(1)").unwrap_err().to_string();
    assert!(
        err.contains("unknown function 'nope'"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_function_arity_mismatch_is_rejected() {
    let err = evaluate_expression("sqrt(1, 2)").unwrap_err().to_string();
    assert!(
        err.contains("'sqrt' takes 1 argument(s), got 2"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_min_requires_at_least_one_argument() {
    let err = evaluate_expression("min()").unwrap_err().to_string();
    assert!(
        err.contains("'min' takes at least 1 argument(s), got 0"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_bad_function_argument_separator_is_rejected() {
    let err = evaluate_expression("pow(2 3)").unwrap_err().to_string();
    assert!(
        err.contains("expected ',' or ')' in function arguments"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_oversized_expression_is_rejected() {
    // 5000 digits exceeds the 4096-character cap before any parsing.
    let expr = "1".repeat(5000);
    let err = evaluate_expression(&expr).unwrap_err().to_string();
    assert!(
        err.contains("longer than the 4096 character limit"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_excessive_nesting_is_rejected() {
    // 65 nested parenthesis depth exceeds the 64-level cap.
    let expr = format!("{}1{}", "(".repeat(65), ")".repeat(65));
    let err = evaluate_expression(&expr).unwrap_err().to_string();
    assert!(
        err.contains("nests deeper than the 64 level limit"),
        "unexpected error message: {err}"
    );
}

// ---------------------------------------------------------------------------
// Tool surface
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tool_execute_returns_formatted_content_and_metadata() {
    let output = CalculatorTool
        .execute(json!({ "expression": "2^32 + sqrt(2)" }), &make_ctx())
        .await
        .expect("calculator execute");

    assert!(
        output.content.starts_with("2^32 + sqrt(2) = "),
        "content should echo the expression: {}",
        output.content
    );
    let metadata = output.metadata.expect("calculator metadata");
    assert_eq!(metadata["expression"], "2^32 + sqrt(2)");
    assert_eq!(metadata["result"], "4294967297.414213");
}

#[tokio::test]
async fn test_tool_execute_missing_expression_is_error() {
    let err = CalculatorTool
        .execute(json!({}), &make_ctx())
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("Missing required 'expression' parameter"),
        "unexpected error message: {err}"
    );
}

#[tokio::test]
async fn test_tool_execute_propagates_parse_error() {
    let err = CalculatorTool
        .execute(json!({ "expression": "1 +" }), &make_ctx())
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("unexpected end of expression"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_tool_metadata() {
    let tool = CalculatorTool;
    assert_eq!(tool.name(), "calculator");
    assert_eq!(tool.permission_category(), "none");
    assert_eq!(tool.parameters_schema()["required"][0], "expression");
}

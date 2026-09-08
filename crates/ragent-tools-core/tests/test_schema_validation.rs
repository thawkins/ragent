//! External tests for `schema::validate_required_args`.
//!
//! Relocated from the inline `#[cfg(test)]` module in
//! `crates/ragent-tools-core/src/schema.rs` per the workspace test-organization
//! rule (all tests live under `tests/`).

use ragent_tools_core::schema::validate_required_args;
use serde_json::json;

#[test]
fn test_schema_no_required_fields_passes() {
    let schema = json!({"type": "object", "properties": {"a": {"type": "string"}}});
    assert!(validate_required_args(&schema, &json!({})).is_ok());
    assert!(validate_required_args(&schema, &json!({ "a": "x" })).is_ok());
    assert!(validate_required_args(&schema, &json!([1])).is_ok());
}

#[test]
fn test_schema_missing_required_field_is_rejected() {
    let schema = json!({"type": "object", "required": ["path"]});
    let err = validate_required_args(&schema, &json!({})).unwrap_err();
    assert!(err.contains("missing required parameter 'path'"), "{err}");
}

#[test]
fn test_schema_non_object_input_is_rejected() {
    let schema = json!({"type": "object", "required": ["path"]});
    let err = validate_required_args(&schema, &json!([1])).unwrap_err();
    assert!(err.contains("must be a JSON object, got array"), "{err}");
    let err = validate_required_args(&schema, &json!("hello")).unwrap_err();
    assert!(err.contains("got string"), "{err}");
}

#[test]
fn test_schema_type_mismatch_is_rejected() {
    let schema = json!({
        "type": "object",
        "required": ["path", "limit"],
        "properties": {
            "path": {"type": "string"},
            "limit": {"type": "integer"}
        }
    });
    let err = validate_required_args(&schema, &json!({"path": 5, "limit": 3})).unwrap_err();
    assert!(err.contains("'path' must be of type 'string'"), "{err}");
    let err = validate_required_args(&schema, &json!({"path": "a", "limit": "x"})).unwrap_err();
    assert!(err.contains("'limit' must be of type 'integer'"), "{err}");
}

#[test]
fn test_schema_valid_args_pass() {
    let schema = json!({
        "type": "object",
        "required": ["path"],
        "properties": {"path": {"type": "string"}}
    });
    assert!(validate_required_args(&schema, &json!({"path": "a"})).is_ok());
    // Unknown fields are tolerated.
    assert!(validate_required_args(&schema, &json!({"path": "a", "extra": 1})).is_ok());
}

#[test]
fn test_schema_unknown_type_declaration_passes() {
    let schema = json!({
        "type": "object",
        "required": ["value"],
        "properties": {"value": {"anyOf": [{"type": "string"}, {"type": "null"}]}}
    });
    assert!(validate_required_args(&schema, &json!({"value": "x"})).is_ok());
}

#[test]
fn test_schema_missing_properties_map_checks_presence_only() {
    let schema = json!({"type": "object", "required": ["a"]});
    assert!(validate_required_args(&schema, &json!({"a": 1})).is_ok());
}

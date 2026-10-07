//! Minimal argument validation against a tool's declared JSON schema.
//!
//! Every [`Tool`](crate::Tool) publishes a JSON-Schema-shaped
//! `parameters_schema()` that is used to describe the tool to the model, but
//! nothing previously enforced it at dispatch time: any parsed arguments value
//! was handed to the tool implementation unchecked. This module provides the
//! central, deliberately conservative validator used by the session processor.
//!
//! Only two things are enforced, because both are universally safe:
//!
//! 1. every field listed in the schema's `required` array is present, and
//! 2. when a required property declares a primitive `type`, the argument's
//!    JSON type matches it.
//!
//! Unknown/extra fields, optional fields, enums, patterns, and compound types
//! (`anyOf` and friends) are intentionally not enforced - tool implementations
//! remain free to accept or reject them with their own, more precise errors.

use serde_json::Value;

/// Validates `input` against the required parameters declared in `schema`.
///
/// # Errors
///
/// Returns a machine-corrective message naming the offending parameter when a
/// required field is missing, a required field's type mismatches its declared
/// primitive `type`, or `input` is not a JSON object while the schema declares
/// required fields.
pub fn validate_required_args(schema: &Value, input: &Value) -> Result<(), String> {
    let Some(required) = schema.get("required").and_then(Value::as_array) else {
        return Ok(());
    };
    if required.is_empty() {
        return Ok(());
    }
    let properties = schema.get("properties").and_then(Value::as_object);
    let Some(object) = input.as_object() else {
        return Err(format!(
            "arguments must be a JSON object, got {}",
            json_type_name(input)
        ));
    };
    for required_field in required {
        let Some(field_name) = required_field.as_str() else {
            continue;
        };
        let Some(field_value) = object.get(field_name) else {
            return Err(format!("missing required parameter '{field_name}'"));
        };
        let Some(expected_type) = properties
            .and_then(|props| props.get(field_name))
            .and_then(|prop| prop.get("type"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        if !declared_type_matches(expected_type, field_value) {
            return Err(format!(
                "parameter '{field_name}' must be of type '{expected_type}', got {}",
                json_type_name(field_value)
            ));
        }
    }
    Ok(())
}

/// Returns `true` when `value`'s JSON type satisfies the schema `type` string.
///
/// Unrecognised type strings (compound declarations, vendor extensions) are
/// treated as satisfied.
fn declared_type_matches(expected: &str, value: &Value) -> bool {
    match expected {
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "number" => value.is_number(),
        // JSON has no integer type; serde_json exposes it as i64/u64.
        "integer" => value.is_i64() || value.is_u64(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        _ => true,
    }
}

/// Returns a human-readable name for `value`'s JSON type.
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

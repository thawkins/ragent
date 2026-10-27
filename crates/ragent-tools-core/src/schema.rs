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
        let declared_type = properties
            .and_then(|props| props.get(field_name))
            .and_then(|prop| prop.get("type"))
            .and_then(Value::as_str);
        let Some(field_value) = object.get(field_name) else {
            // FR-009 (spec `uniformtools`) requires the corrective result to
            // name the offending parameter *and its declared type*, so the
            // message states the type here rather than leaving the caller to
            // look it up a second time from the same schema.
            return Err(match declared_type {
                Some(expected_type) => format!(
                    "missing required parameter '{field_name}', which is declared as type \
                     '{expected_type}'"
                ),
                None => format!("missing required parameter '{field_name}'"),
            });
        };
        let Some(expected_type) = declared_type else {
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

/// Renames argument keys that differ from a declared property only by ASCII
/// letter case onto the declared spelling, leaving every other key untouched.
///
/// A model that capitalises a parameter (`EXPRESSION` for `expression`, `PATH`
/// for `path`) sent a value the tool can use; [`validate_required_args`], which
/// matches keys exactly, could not see it. Folding here - after the decode and
/// before the schema check - means the validator and the tool body read one
/// spelling, and a payload that never mentioned the key is still refused.
///
/// Only a *unique* case-insensitive match folds: two declared properties that
/// differ only by case must not make the choice depend on map order. A schema
/// with no declared `properties` (a parameterless tool) is returned unchanged,
/// and so is any input that is not a JSON object.
///
/// # Examples
///
/// ```
/// use ragent_tools_core::schema::fold_argument_keys;
/// use serde_json::json;
///
/// let schema = json!({"properties": {"expression": {"type": "string"}}});
/// let folded = fold_argument_keys(&schema, json!({"EXPRESSION": "6*7"}));
/// assert_eq!(folded, json!({"expression": "6*7"}));
/// ```
#[must_use]
pub fn fold_argument_keys(schema: &Value, input: Value) -> Value {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return input;
    };
    let Value::Object(mut map) = input else {
        return input;
    };
    let folded: Vec<(String, String)> = map
        .keys()
        .filter(|key| !properties.contains_key(*key))
        .filter_map(|key| {
            let mut matches = properties
                .keys()
                .filter(|declared| declared.eq_ignore_ascii_case(key));
            let declared = matches.next()?;
            // Only a unique match is a fold: two declared properties that differ
            // only by case must not make the choice depend on map order.
            if matches.next().is_some() {
                return None;
            }
            Some((key.clone(), declared.clone()))
        })
        .collect();
    for (from, to) in folded {
        if let Some(value) = map.remove(&from) {
            map.entry(to).or_insert(value);
        }
    }
    Value::Object(map)
}

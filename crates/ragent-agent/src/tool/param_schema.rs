//! Shared tool-parameter schema-property validation.
//!
//! A tool's [`parameters_schema`](crate::tool::Tool::parameters_schema) is the
//! contract the model is shown: it names every property the tool accepts and,
//! with `"additionalProperties": false`, promises that anything else is
//! refused. Nothing in the dispatch pipeline enforces that promise - a tool
//! that declares `additionalProperties: false` and then reads its parameters
//! by key silently ignores a misspelled one (`backgound: true` degrades to the
//! default and the call still "succeeds").
//!
//! This module is the one place that turns the declared schema into an actual
//! check (spec `agentssetup` FR-004 / FR-038): a tool calls
//! [`validate_tool_input`] at the top of its `execute`, and a call carrying a
//! property the schema does not declare is refused with an error naming the
//! offending key rather than being silently ignored (FR-002).
//!
//! Only the *property names* are validated. Value types, `enum` membership
//! and conditional requirements stay with each tool, because those need the
//! tool's own vocabulary to produce a useful message.

use anyhow::Result;
use serde_json::{Map, Value};

/// Returns the declared property names of `schema`, or `None` when the schema
/// does not describe a closed set of properties.
///
/// The set is closed when the schema is an object schema that either declares
/// `"additionalProperties": false` explicitly or declares a `"properties"`
/// object without an `additionalProperties` override (JSON Schema's
/// `additionalProperties` defaults to `true`, so only the explicit `false`
/// counts as a promise - plus the `{}`-with-properties shape this workspace's
/// tools all use, where the declared properties are the whole surface).
///
/// # Returns
///
/// `Some(names)` when unknown keys must be refused, `None` when the schema
/// accepts arbitrary keys (no `properties`, or a non-object schema).
#[must_use]
pub fn closed_property_names(schema: &Value) -> Option<Vec<String>> {
    let object = schema.as_object()?;
    let properties = object.get("properties")?.as_object()?;
    match object.get("additionalProperties") {
        Some(Value::Bool(false)) => {}
        Some(_) => return None,
        // Absent: fall back to "no extra keys" only when the schema declares
        // itself to be an object with properties (the shape every built-in
        // tool uses), so a tool that deliberately allows extras keeps working.
        None => {
            if object.get("type").and_then(Value::as_str) != Some("object") {
                return None;
            }
        }
    }
    Some(properties.keys().cloned().collect())
}

/// Refuses `input` when it carries a property the tool's schema does not
/// declare (FR-004, FR-038).
///
/// Call this first in a tool's `execute`, with the same schema the tool
/// returns from [`parameters_schema`](crate::tool::Tool::parameters_schema),
/// so a misspelled or invented parameter fails loudly instead of being
/// ignored (FR-002).
///
/// # Errors
///
/// Returns an error naming every unknown key and listing the declared
/// properties, when `input` is a JSON object carrying at least one key absent
/// from the schema's `properties`. A non-object `input` (the dispatch pipeline
/// coerces a missing argument object to `{}` before the tool runs) is never
/// refused here; per-parameter requirements remain the tool's own check.
///
/// # Example
///
/// ```
/// use ragent_agent::tool::param_schema::validate_tool_input;
/// use serde_json::json;
///
/// let schema = json!({
///     "type": "object",
///     "properties": { "background": { "type": "boolean" } },
///     "additionalProperties": false
/// });
///
/// assert!(validate_tool_input(&schema, &json!({ "background": true })).is_ok());
///
/// let err = validate_tool_input(&schema, &json!({ "backgound": true }))
///     .expect_err("a misspelled parameter must be refused");
/// assert!(err.to_string().contains("backgound"));
/// ```
pub fn validate_tool_input(schema: &Value, input: &Value) -> Result<()> {
    let Some(declared) = closed_property_names(schema) else {
        return Ok(());
    };
    let Some(map) = input.as_object() else {
        return Ok(());
    };

    let mut unknown: Vec<&str> = map
        .keys()
        .filter(|key| !declared.iter().any(|known| known == *key))
        .map(String::as_str)
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    unknown.sort_unstable();

    let mut sorted_declared = declared;
    sorted_declared.sort_unstable();

    Err(anyhow::anyhow!(
        "Unknown parameter(s): {}. Declared parameters are: {}. A parameter the tool's schema \
         does not declare is refused rather than ignored - check the spelling.",
        quoted_list(&unknown),
        if sorted_declared.is_empty() {
            "(none)".to_string()
        } else {
            quoted_list(
                &sorted_declared
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            )
        }
    ))
}

/// Formats the names as a comma-separated list of backtick-quoted keys.
fn quoted_list(names: &[&str]) -> String {
    names
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The `properties` table of a schema, for tools that want to reject an
/// unknown *value* against a declared `enum` while naming the property.
///
/// Returns `None` when the schema has no `properties` table.
#[must_use]
pub fn declared_properties(schema: &Value) -> Option<&Map<String, Value>> {
    schema.get("properties")?.as_object()
}

/// The prefix that introduces a machine-readable reason code in an error
/// message: `reason_code: <message>`.
///
/// A tool error crosses the dispatch boundary as a plain `anyhow::Error`, so
/// there is no metadata channel to carry a code alongside it. The code
/// therefore travels in the message itself, in a fixed `code: message` shape
/// that a caller (or a test) can match on: [`reason_in`] extracts it, and
/// [`prefixed`] writes it (FR-010).
const REASON_PREFIX: &str = "reason_code: ";

/// Returns the machine-readable reason code carried by `message`, or `None`
/// when the message carries none.
///
/// The code is the leading token of the message when it is written by
/// [`prefixed`]: one or more lowercase ASCII words joined by `_`, terminated by
/// `": "`. A message that does not start with such a token (every error that
/// predates or ignores this convention) yields `None`, so callers can branch on
/// the code without having to pattern-match English prose.
///
/// # Example
///
/// ```
/// use ragent_agent::tool::param_schema::reason_in;
///
/// assert_eq!(
///     reason_in("limit_reached: 2 of 2 slots in use"),
///     Some("limit_reached")
/// );
/// assert_eq!(reason_in("Missing required parameter: agent"), None);
/// ```
#[must_use]
pub fn reason_in(message: &str) -> Option<&str> {
    // Accept both the tool-error envelope (`reason_code: <code>: ...`) and a
    // bare `<code>: ...` message, so the same extractor reads a registry
    // refusal and a tool error.
    let message = message.strip_prefix(REASON_PREFIX).unwrap_or(message);
    let (code, _) = message.split_once(": ")?;
    if code.is_empty()
        || !code
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        || code.starts_with('_')
        || code.ends_with('_')
    {
        return None;
    }
    Some(code)
}

/// Returns `message` with `code` prefixed so [`reason_in`] recovers it.
///
/// Used by a producing site (the `AgentManager`) that has no tool-output
/// channel of its own: the tool that called it re-reads the code with
/// [`reason_in`] and copies it into the tool result metadata (FR-010).
#[must_use]
pub fn prefixed(code: &str, message: &str) -> String {
    format!("{code}: {message}")
}

/// Builds a tool error carrying `code` as its machine-readable reason and
/// `message` as its human half (FR-010, NFR-005).
///
/// The returned error's `Display` is `reason_code: <message>`, so a caller that
/// only has the message can recover the code with [`reason_in`].
///
/// # Example
///
/// ```
/// use ragent_agent::tool::param_schema::{reason_error, reason_in};
///
/// let err = reason_error("limit_reached", "2 of 2 background slots are in use");
/// assert_eq!(reason_in(&err.to_string()), Some("limit_reached"));
/// ```
pub fn reason_error(code: &str, message: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{REASON_PREFIX}{}", prefixed(code, &message.to_string()))
}

/// Adds the machine-readable `reason` (and `reason_message`) fields to a tool
/// result's `metadata` (FR-010).
///
/// A tool whose failure is reported as a *result* rather than an error - the
/// shape `cancel_agent` uses for a refusal the caller can act on - attaches the
/// code here so the caller does not have to branch on English prose.
///
/// Returns the metadata unchanged when it is absent or is not a JSON object.
#[must_use]
pub fn with_reason(metadata: Option<Value>, code: &str) -> Value {
    match metadata {
        Some(Value::Object(mut map)) => {
            map.insert("reason".to_string(), Value::String(code.to_string()));
            Value::Object(map)
        }
        Some(other) => other,
        None => {
            let mut map = Map::new();
            map.insert("reason".to_string(), Value::String(code.to_string()));
            Value::Object(map)
        }
    }
}

#[cfg(test)]
#[path = "../../tests/inline/param_schema_tests.rs"]
mod tests;

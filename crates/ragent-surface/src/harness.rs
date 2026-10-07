//! Shared harness primitives for the `/plugins` and `/connectors` isolated
//! `/test` harnesses (spec `plugins` FR-013 / spec `connectors` FR-015).
//!
//! Both surfaces report a harness run as an ordered list of
//! [`HarnessStep`]s, generate schema-valid sample arguments with
//! [`sample_for_schema`], and truncate a captured result with [`truncate`].
//! Each had grown its own byte-identical copy of those items; the one
//! implementation lives here and both crates delegate.

use std::time::Duration;

use serde_json::Value as JsonValue;

/// Outcome of one harness step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// The step completed successfully.
    Pass,
    /// The step failed; the string is the cause (SPEC error-handling policy).
    Fail(String),
}

/// One reported harness step: a name, its outcome, the wall-clock time it took,
/// and an optional detail line (contributed names, generated sample arguments,
/// invocation result).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessStep {
    /// Step label (`discovery`, `manifest validation`, `version check`,
    /// `entry execution`, `sample invocation <tool>`).
    pub name: String,
    /// Pass/fail plus the cause on failure.
    pub outcome: StepOutcome,
    /// Wall-clock time spent in this step.
    pub elapsed: Duration,
    /// Optional human-readable detail appended to the rendered line.
    pub detail: Option<String>,
}

/// Build a [`HarnessStep`] record.
#[must_use]
pub fn step(
    name: impl Into<String>,
    outcome: StepOutcome,
    elapsed: Duration,
    detail: Option<String>,
) -> HarnessStep {
    HarnessStep {
        name: name.into(),
        outcome,
        elapsed,
        detail,
    }
}

/// Truncate `text` to at most `max` characters for a report.
///
/// A single char-iteration: `char_indices().nth(max)` is `Some` only when the
/// text is longer than `max`, so no separate length pass is needed.
#[must_use]
pub fn truncate(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((idx, _)) => format!("{}...", &text[..idx]),
        None => text.to_string(),
    }
}

/// Generate schema-valid sample arguments for a JSON-schema tool declaration.
///
/// Honours `const`/`default`, then the first of `examples`/`enum`, then the
/// declared `type` (recursing into `properties` and `items`); an untyped or
/// unknown node degrades to JSON `null`. The sample is only ever used as
/// harness input, never written back to a store.
#[must_use]
pub fn sample_for_schema(schema: &JsonValue) -> JsonValue {
    match schema {
        JsonValue::Object(map) => {
            for key in ["const", "default"] {
                if let Some(value) = map.get(key) {
                    return value.clone();
                }
            }
            for key in ["examples", "enum"] {
                if let Some(first) = map
                    .get(key)
                    .and_then(JsonValue::as_array)
                    .and_then(|values| values.first())
                {
                    return first.clone();
                }
            }
            match schema_type(map) {
                "object" => {
                    let mut out = serde_json::Map::new();
                    if let Some(properties) = map.get("properties").and_then(JsonValue::as_object) {
                        for (name, sub) in properties {
                            out.insert(name.clone(), sample_for_schema(sub));
                        }
                    }
                    JsonValue::Object(out)
                }
                "array" => match map.get("items") {
                    Some(items) => JsonValue::Array(vec![sample_for_schema(items)]),
                    None => JsonValue::Array(Vec::new()),
                },
                "string" => JsonValue::String("sample".to_string()),
                "integer" => JsonValue::from(0),
                "number" => JsonValue::from(0.0),
                "boolean" => JsonValue::Bool(false),
                "null" => JsonValue::Null,
                _ => JsonValue::Null,
            }
        }
        JsonValue::Bool(_) => JsonValue::Bool(false),
        JsonValue::Number(_) => JsonValue::from(0),
        JsonValue::String(_) => JsonValue::String("sample".to_string()),
        JsonValue::Array(_) => JsonValue::Array(Vec::new()),
        JsonValue::Null => JsonValue::Null,
    }
}

/// The `type` keyword of a JSON-schema object, tolerating the array form
/// (`"type": ["string", "null"]`) by taking the first string entry.
#[must_use]
pub fn schema_type(map: &serde_json::Map<String, JsonValue>) -> &str {
    match map.get("type") {
        Some(JsonValue::String(name)) => name.as_str(),
        Some(JsonValue::Array(names)) => {
            names.iter().find_map(JsonValue::as_str).unwrap_or("object")
        }
        _ => "object",
    }
}

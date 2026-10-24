//! The one decoder for accumulated tool-call argument text (spec
//! `uniformtools` FR-007).
//!
//! A provider streams a tool call's arguments as opaque fragments
//! ([`crate::llm::StreamEvent::ToolCallDelta`]) and the harness concatenates
//! them into a single string. What that string means is a wire-shape question
//! with exactly three acceptable answers:
//!
//! * a JSON object - the ordinary case, and the only thing a tool receives;
//! * a JSON *string* that itself holds an object - emitted by servers that
//!   double-encode the arguments (`"{\"path\":\"a.rs\"}"` as the value of a
//!   string field rather than as the field's object);
//! * empty or whitespace - a tool call with no arguments, which decodes to
//!   `{}` so a parameterless tool runs instead of failing.
//!
//! Anything else (a bare array, a number, `null`, or text that is not JSON at
//! all) is an error the caller turns into a corrective, model-visible message
//! (FR-008); it is never silently coerced to `{}`.
//!
//! This lives in `ragent-types` rather than in a provider crate because the
//! decode is a *harness* responsibility: the provider layer emits fragments
//! verbatim and **must not** decode them (FR-007), while the agent loop and the
//! TUI both need the same rule. One implementation keeps a model's arguments
//! from decoding one way in the loop and another way on screen.

use serde_json::Value;

/// Why accumulated tool-call argument text could not be decoded into the
/// argument object a tool receives (FR-007).
///
/// Every variant carries enough detail for the corrective error required by
/// FR-008 ("quotes the parse error"), so the caller never has to invent a
/// second explanation of the same failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolArgDecodeError {
    /// The text is not valid JSON at all. `detail` is the parser's own
    /// message, quoted verbatim by the corrective error.
    InvalidJson {
        /// The `serde_json` parse error text.
        detail: String,
    },
    /// The text is valid JSON, but not a JSON object (an array, a number, a
    /// boolean or `null`), and so cannot be a tool's argument object.
    NotAnObject {
        /// What was found instead, phrased for a sentence ("an array").
        found: &'static str,
    },
    /// The text is a JSON string, but the text that string holds is not a
    /// JSON object - either it does not parse, or it parses to a non-object.
    EncodedStringNotAnObject {
        /// The inner parse error, or a description of the non-object found.
        detail: String,
    },
}

impl std::fmt::Display for ToolArgDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson { detail } => {
                write!(f, "arguments are not valid JSON: {detail}")
            }
            Self::NotAnObject { found } => {
                write!(f, "arguments must be a single JSON object, got {found}")
            }
            Self::EncodedStringNotAnObject { detail } => write!(
                f,
                "arguments are a JSON string that does not decode to a JSON object: {detail}"
            ),
        }
    }
}

impl std::error::Error for ToolArgDecodeError {}

/// Decode the accumulated argument text of one completed tool call into the
/// JSON object the tool receives (FR-007).
///
/// Accepts a JSON object, a JSON string that decodes to one, or empty /
/// whitespace-only text (which is `{}` - a tool call with no arguments).
/// Surrounding whitespace is trimmed at each layer before parsing.
///
/// This is the **only** decode of tool-call arguments in the workspace: the
/// dispatch pipeline calls it once per call per step and reuses the result
/// through every later stage (schema validation, the loop restriction, hooks,
/// the repeat guard, the permission verdict and the backend call), and the TUI
/// calls it to render the same arguments. The provider layer never calls it -
/// providers emit `ToolCallDelta` fragments verbatim.
///
/// # Errors
///
/// Returns [`ToolArgDecodeError`] when the text is not JSON
/// ([`ToolArgDecodeError::InvalidJson`]), is JSON that is not an object
/// ([`ToolArgDecodeError::NotAnObject`]), or is a JSON string whose payload is
/// not an object ([`ToolArgDecodeError::EncodedStringNotAnObject`]). The caller
/// must not execute the tool in that case (FR-008).
///
/// # Examples
///
/// ```
/// use ragent_types::tool_args::decode_tool_arguments;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let object = decode_tool_arguments(r#"{"path":"a.rs"}"#)?;
/// assert_eq!(object["path"], "a.rs");
///
/// // A double-encoded payload decodes to the same object.
/// let encoded = decode_tool_arguments(r#""{\"path\":\"a.rs\"}""#)?;
/// assert_eq!(encoded, object);
///
/// // No arguments at all is an empty object, not an error.
/// assert_eq!(decode_tool_arguments("   ")?, serde_json::json!({}));
///
/// assert!(decode_tool_arguments("[1, 2]").is_err());
/// # Ok(())
/// # }
/// ```
pub fn decode_tool_arguments(raw: &str) -> Result<Value, ToolArgDecodeError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    let parsed: Value =
        serde_json::from_str(trimmed).map_err(|e| ToolArgDecodeError::InvalidJson {
            detail: e.to_string(),
        })?;
    match parsed {
        Value::Object(_) => Ok(parsed),
        Value::String(inner) => {
            let inner = inner.trim();
            if inner.is_empty() {
                return Ok(Value::Object(serde_json::Map::new()));
            }
            let nested: Value = serde_json::from_str(inner).map_err(|e| {
                ToolArgDecodeError::EncodedStringNotAnObject {
                    detail: e.to_string(),
                }
            })?;
            if nested.is_object() {
                Ok(nested)
            } else {
                Err(ToolArgDecodeError::EncodedStringNotAnObject {
                    detail: format!("the string holds {}, not an object", json_kind(&nested)),
                })
            }
        }
        other => Err(ToolArgDecodeError::NotAnObject {
            found: json_kind(&other),
        }),
    }
}

/// Name a JSON value's type for an error sentence ("an array", "a number").
fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

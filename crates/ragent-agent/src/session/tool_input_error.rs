//! The one construction site for a tool call refused *before* it ran (spec
//! `uniformtools` FR-008, FR-009, FR-014, FR-021).
//!
//! Three different mistakes reach the dispatch pipeline and have the same
//! consequence: the tool must not execute, and the model must be told why in
//! words it can act on.
//!
//! * The accumulated argument text would not decode
//!   ([`ragent_types::tool_args::decode_tool_arguments`]) - FR-008's trigger.
//! * The decoded arguments violate the tool's declared `parameters_schema()`
//!   ([`ragent_tools_core::schema::validate_required_args`]) - FR-009's trigger.
//! * The name is not registered - FR-021's trigger, which additionally offers
//!   the closest registered name when one is within the similarity threshold
//!   ([`crate::session::dispatch::suggest_tool_name`]).
//!
//! Before this module the two were built in two places, in two shapes: the
//! decode gate hand-assembled a nine-field outcome tuple around its own
//! sentence, while a schema failure left as a generic `anyhow` error whose text
//! the caller reformatted. Each site separately produced a `ToolCallStatus` and
//! separately decided the result's `is_error` flag, and two further sites
//! (`session::history`, `compaction::convert`) derived the flag again. A change
//! to any one sentence, status or flag reached only its own path.
//!
//! Here both causes become one [`ToolInputError`] value that owns its
//! model-facing sentence, the record status, and - through
//! [`ToolCallStatus::is_error`](ragent_types::message::ToolCallStatus::is_error) -
//! the `is_error` flag. The loop adapts that
//! value to the two shapes it needs (an outcome tuple for the pre-dispatch gate
//! and an `anyhow` error for the in-task gate), so the *content* of a refusal is
//! stated once and the adapters cannot drift from it.
//!
//! Ordering note: this module decides what a refusal says, not when it is
//! reached. Placing the schema check before the pre-tool hooks is the ordered
//! pipeline's job (FR-012, T-014).

use ragent_types::ToolArgDecodeError;
use ragent_types::message::ToolCallStatus;

/// Why a tool call was refused before execution, and what the model is told
/// (FR-008, FR-009).
///
/// Both variants carry the tool's name and the stage-specific detail, so the
/// sentence names the tool and quotes the underlying reason verbatim - a parse
/// error from `serde_json`, or the schema violation's own account of the
/// offending parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolInputError {
    /// The accumulated argument text is not a JSON object, so the tool cannot
    /// be called at all (FR-008).
    Undecodable {
        /// The tool the model tried to call.
        tool: String,
        /// The decoder's own account of the failure, quoted verbatim.
        detail: String,
        /// The schema's verdict on the `Value::Null` the failed decode stands
        /// for, when the tool is registered and has a schema to violate.
        ///
        /// A non-object payload is both undecodable *and* (as `null`) missing
        /// every required parameter; stating both reasons in the one sentence
        /// tells the model everything it needs to correct the call in a single
        /// retry.
        schema_reason: Option<String>,
    },
    /// The arguments decoded but do not satisfy the tool's declared schema
    /// (FR-009).
    SchemaViolation {
        /// The tool the model tried to call.
        tool: String,
        /// The validator's account of the offending parameter.
        detail: String,
    },
    /// The name the model sent is not registered, so nothing can run
    /// (FR-021).
    ///
    /// `suggestion` carries the closest registered name when one is within the
    /// similarity threshold, and is `None` when nothing is close enough - the
    /// refusal then names only the unknown tool rather than guessing.
    UnknownTool {
        /// The tool the model tried to call.
        tool: String,
        /// The closest registered name, when one is within the threshold.
        suggestion: Option<String>,
    },
}

impl ToolInputError {
    /// Build the FR-008 refusal for undecodable argument text.
    ///
    /// The decoder's [`Display`](std::fmt::Display) output is quoted verbatim,
    /// because FR-008 requires the corrective result to quote the parse error
    /// rather than paraphrase it.
    pub fn undecodable(tool: &str, error: &ToolArgDecodeError) -> Self {
        Self::Undecodable {
            tool: tool.to_string(),
            detail: error.to_string(),
            schema_reason: None,
        }
    }

    /// Build the FR-008 refusal with the schema's verdict on the `Value::Null`
    /// a failed decode stands for.
    ///
    /// A payload that is not a JSON object is also, as far as the tool's schema
    /// is concerned, missing every required parameter. Both reasons are stated in
    /// the one sentence so the model can correct the call in a single retry
    /// rather than discovering the second problem after fixing only the first.
    pub fn undecodable_with_reason(tool: &str, error: &ToolArgDecodeError, reason: &str) -> Self {
        Self::Undecodable {
            tool: tool.to_string(),
            detail: error.to_string(),
            schema_reason: Some(reason.to_string()),
        }
    }

    /// Build the FR-009 refusal for a schema violation.
    ///
    /// `detail` is
    /// [`validate_required_args`](ragent_tools_core::schema::validate_required_args)'s
    /// message, which already names the offending parameter and its declared
    /// type.
    pub fn schema_violation(tool: &str, detail: &str) -> Self {
        Self::SchemaViolation {
            tool: tool.to_string(),
            detail: detail.to_string(),
        }
    }

    /// Build the FR-021 refusal for an unregistered tool name.
    ///
    /// `suggestion` is the closest registered name from
    /// [`suggest_tool_name`](crate::session::dispatch::suggest_tool_name), or
    /// `None` when nothing is within the threshold. The caller resolves the
    /// similarity, so this constructor never consults a registry itself and the
    /// suggestion and the sentence cannot disagree.
    pub fn unknown_tool(tool: &str, suggestion: Option<&str>) -> Self {
        Self::UnknownTool {
            tool: tool.to_string(),
            suggestion: suggestion.map(str::to_string),
        }
    }

    /// The tool the model tried to call.
    pub fn tool(&self) -> &str {
        match self {
            Self::Undecodable { tool, .. }
            | Self::SchemaViolation { tool, .. }
            | Self::UnknownTool { tool, .. } => tool,
        }
    }

    /// The record status a refused call is stored with (FR-014).
    ///
    /// Always [`ToolCallStatus::Error`]: a refused call has not run and must not
    /// be indistinguishable from one that did, which is exactly what the status
    /// is for.
    pub fn status(&self) -> ToolCallStatus {
        ToolCallStatus::Error
    }

    /// The `is_error` flag the refusal's tool result carries (FR-014).
    ///
    /// Derived from [`Self::status`], never guessed from the message text, so
    /// the flag and the record cannot disagree.
    pub fn is_error(&self) -> bool {
        self.status().is_error()
    }

    /// The refusal as an `anyhow` error, for the in-task gate whose single exit
    /// path already reformats an error into the result's text and status.
    ///
    /// `Display` and the error's `{:#}` alternate form both render this same
    /// message, so the in-task route produces the identical text the pre-
    /// dispatch route does.
    pub fn into_anyhow(self) -> anyhow::Error {
        anyhow::anyhow!("{self}")
    }
}

impl std::fmt::Display for ToolInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Undecodable {
                tool,
                detail,
                schema_reason,
            } => match schema_reason {
                Some(reason) => write!(
                    f,
                    "Invalid arguments for tool '{tool}': {detail}. Arguments must be a \
                     single complete JSON object; resend the tool call with corrected \
                     JSON. The tool's schema also reports: {reason}. The tool was not \
                     executed."
                ),
                None => write!(
                    f,
                    "Invalid arguments for tool '{tool}': {detail}. Arguments must be a \
                     single complete JSON object; resend the tool call with corrected \
                     JSON. The tool was not executed."
                ),
            },
            Self::SchemaViolation { tool, detail } => write!(
                f,
                "Invalid arguments for tool '{tool}': {detail}. The tool was not \
                 executed; resend the call with arguments that satisfy the tool's \
                 declared schema."
            ),
            Self::UnknownTool { tool, suggestion } => match suggestion {
                Some(candidate) => write!(
                    f,
                    "Unknown tool: '{tool}'. Did you mean '{candidate}'? The tool \
                     was not executed; use a registered tool name."
                ),
                None => write!(
                    f,
                    "Unknown tool: '{tool}'. The tool was not executed; use a \
                     registered tool name."
                ),
            },
        }
    }
}

impl std::error::Error for ToolInputError {}

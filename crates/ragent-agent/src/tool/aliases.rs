//! Alias tools that map commonly hallucinated tool names to canonical implementations.
//!
//! Many LLMs emit tool names that differ from ragent's canonical names - either
//! because they have been trained on different coding-agent frameworks or because
//! they extrapolate plausible-sounding names from the task context.  Rather than
//! returning "Unknown tool" errors, each alias tool normalises its parameter names
//! and delegates to the canonical implementation.
//!
//! ## Aliases provided
//!
//! | Alias name          | Canonical tool | Notes                              |
//! |---------------------|----------------|------------------------------------|
//! | `update_file`       | `Write`        | `content` pass-through            |
//! | `run_code`          | `Bash`         | `code` -> `command`               |
//! | `AskUserQuestion`   | (self)         | free-text or multiple-choice user prompt, with a default/fallback option |

use anyhow::{Context, Result};
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use super::{Tool, ToolContext, ToolOutput};
// `Bash` and `Write` were agent-local dormant duplicates removed in
// DCREMOVALPLAN M3. The canonical `BashTool`/`WriteTool` implementations live
// in `ragent-tools-core` and implement that crate's `Tool` trait, so we bridge
// them back to the agent-local `Tool` trait via `ExtractedCoreToolAdapter`
// (the same adapter the runtime registry uses for every core tool).
use super::ExtractedCoreToolAdapter;
use crate::event::Event;
use ragent_tools_core::{bash, write};

// ---------------------------------------------------------------------------
// Helper: build a normalised input Value and delegate to a canonical tool
// ---------------------------------------------------------------------------

async fn delegate(
    tool: &(impl Tool + ?Sized),
    input: Value,
    ctx: &ToolContext,
) -> Result<ToolOutput> {
    tool.execute(input, ctx).await
}

/// Extract a shell command from an input Value, trying multiple common parameter names.
/// Models emit `command`, `code`, or `cmd` (sometimes as an array like `["bash","-c","..."]`).
fn extract_command(input: &mut Value) -> Option<String> {
    // Try `command` first (canonical)
    if let Some(s) = input["command"].as_str() {
        return Some(s.to_string());
    }
    // Then `code`
    if let Some(s) = input["code"].as_str().map(|s| s.to_string()) {
        input["command"] = Value::String(s.clone());
        return Some(s);
    }
    // Then `cmd` - may be a string or an array
    match &input["cmd"] {
        Value::String(s) => {
            let cmd = s.clone();
            input["command"] = Value::String(cmd.clone());
            return Some(cmd);
        }
        Value::Array(arr) => {
            // Join array elements as a shell command via `bash -c`
            let parts: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if !parts.is_empty() {
                let cmd = parts.join(" ");
                input["command"] = Value::String(cmd.clone());
                return Some(cmd);
            }
        }
        _ => {}
    }
    None
}

// ---------------------------------------------------------------------------
// update_file -> write
// ---------------------------------------------------------------------------

/// Alias for `Write`. Accepts `path` and `content`.
pub struct UpdateFileTool;

#[async_trait::async_trait]
impl Tool for UpdateFileTool {
    fn name(&self) -> &'static str {
        "update_file"
    }

    fn description(&self) -> &'static str {
        "Write new content to an existing file, replacing its current contents. \
         This is an alias for the canonical 'Write' tool and accepts exactly the same \
         parameters. REQUIRED parameters: 'path' (string, file to create or overwrite) and \
         'content' (string, new file contents). Common gotcha: 'path' must be relative to \
         the session working directory; missing either parameter will fail."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path":    { "type": "string", "description": "Path to the file to update" },
                "content": { "type": "string", "description": "New content to write" }
            },
            "required": ["path", "content"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    /// Writes `content` to `path` through the canonical `Write` implementation.
    ///
    /// The alias's whole contribution is its surface: the name `update_file`,
    /// the `update_file` row in the permission and loop-state tables, and the
    /// schema above, which advertises `path` and `content` and nothing else.
    /// The write itself is `ragent-tools-core`'s, because a second copy of the
    /// file-writing code is exactly the duplication `write::WriteTool` exists to
    /// prevent. The forward is reviewed by the NFR-008 guard (spec
    /// `uniformtools`): its per-line exemption table records this site and the
    /// reason it is not a wrapper standing in for a retired name.
    ///
    /// # Errors
    ///
    /// Returns whatever the canonical `Write` returns: a missing `path` or
    /// `content`, a parent directory that cannot be created, or a failed write.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        delegate(
            &ExtractedCoreToolAdapter::new(std::sync::Arc::new(write::WriteTool)),
            input,
            ctx,
        )
        .await
    }
}

// ---------------------------------------------------------------------------
// Bash execution aliases
// ---------------------------------------------------------------------------

/// Alias for `Bash`. Accepts `code` (maps to `command`).
pub struct RunCodeTool;

#[async_trait::async_trait]
impl Tool for RunCodeTool {
    fn name(&self) -> &'static str {
        "run_code"
    }

    fn description(&self) -> &'static str {
        "Run a shell command or code snippet as an alias for the canonical 'Bash' tool. \
         REQUIRED parameter: at least one of 'code' or 'command' (string, the shell command \
         or snippet to execute). The tool maps 'code' to the canonical 'command' parameter. \
         Optional: 'timeout' (integer seconds, default 120). Common gotcha: if neither 'code' \
         nor 'command' is provided the call will fail."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "code":    { "type": "string", "description": "Code or command to run" },
                "command": { "type": "string", "description": "Shell command (alternative to 'code')" },
                "timeout": { "type": "integer", "description": "Timeout in seconds (default: 120)" }
            },
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "bash:execute"
    }

    async fn execute(&self, mut input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        if extract_command(&mut input).is_none() {
            anyhow::bail!("Missing required 'command', 'code', or 'cmd' parameter");
        }
        delegate(
            &ExtractedCoreToolAdapter::new(std::sync::Arc::new(bash::BashTool)),
            input,
            ctx,
        )
        .await
    }
}

/// Prompts the user for input during a session, then blocks until they respond.
///
/// Some models emit `AskUserQuestion` when they want to prompt the user for input.
/// Supports two modes:
///
/// - **Free-text**: shows an editable text input area and returns the typed
///   response.
/// - **Multiple-choice**: pass the optional `options` array to render a
///   selectable list; the selected option is returned as the response.
pub struct AskUserTool;

#[async_trait::async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &'static str {
        "AskUserQuestion"
    }

    fn description(&self) -> &'static str {
        "Ask the user a question and wait for their typed response. Use this when you need \
         clarification, prioritisation help, or confirmation before proceeding. \
         REQUIRED parameter: 'question' (string, the prompt shown to the user). Optional: \
         'options' (array of strings) to render a multiple-choice dialog instead of free \
         text. Optional: 'default' (string) names the option pre-selected in the dialog and \
         used automatically if the user lets the dialog time out. A 'default' value that \
         matches no option is ignored, as is an option marked 'recommended'; with no usable \
         default the first option is pre-selected and used on timeout. Common gotcha: if \
         'options' is provided, the returned value will be one of the supplied strings; if \
         omitted, the user may type any free-text answer."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to ask the user"
                },
                "options": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional multiple-choice options. When provided, the user selects one instead of typing free text."
                },
                "default": {
                    "type": "string",
                    "description": "Optional. The option pre-selected in the dialog and used automatically if the user does not answer before the question times out. Must match one of 'options'. When absent, an option marked 'recommended' is used; otherwise the first option is used."
                }
            },
            "required": ["question"],
            "additionalProperties": false
        })
    }

    /// `ask_user`, not `ask_user_question` and not the tool's own name
    /// (FR-028).
    ///
    /// The family is permission vocabulary and predates the rename: the tool is
    /// called `AskUserQuestion` now, while the category a `"permission":
    /// "ask_user"` rule keys on is unchanged. Renaming it would silently
    /// un-govern every rule written against it, which is exactly the state the
    /// rename is required to preserve.
    fn permission_category(&self) -> &'static str {
        "ask_user"
    }

    /// Publishes a question event and blocks until the user submits a response.
    ///
    /// # Errors
    ///
    /// Returns an error if the `question` parameter is missing or the event
    /// bus closes while waiting for a reply.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let question = input["question"]
            .as_str()
            .context("Missing required 'question' parameter")?;

        let options: Vec<String> = input["options"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let default_index = resolve_default_index(options.as_slice(), input["default"].as_str());

        let request_id = uuid::Uuid::new_v4().to_string();

        // Subscribe before publishing so we don't miss the reply.
        let mut rx = ctx.event_bus.subscribe();

        ctx.event_bus.publish(Event::QuestionRequested {
            session_id: ctx.session_id.clone(),
            request_id: request_id.clone(),
            question: question.to_string(),
            options,
            default_index,
            // A pick-list question is never a credential prompt; the maskable
            // free-text answer belongs to the sudo askpass path only.
            secret: false,
        });

        // Wait for a matching QuestionAnswered event from the TUI.
        let response = loop {
            match rx.recv().await {
                Ok(Event::QuestionAnswered {
                    session_id: ref s,
                    request_id: ref rid,
                    response: ref r,
                    ..
                }) if s == &ctx.session_id && rid == &request_id => {
                    break r.clone();
                }
                Ok(_) => {
                    // Ignore unrelated events.
                }
                Err(RecvError::Lagged(_)) => {
                    // Some events were dropped; keep waiting.
                }
                Err(RecvError::Closed) => {
                    anyhow::bail!("Event bus closed while waiting for user response");
                }
            }
        };

        Ok(ToolOutput {
            content: response.clone(),
            metadata: Some(json!({
                "request_id": request_id,
                "question": question,
                "response": response,
            })),
        })
    }
}

/// Resolve which option index should be pre-selected (and used on timeout) for
/// a multiple-choice `AskUserQuestion` request.
///
/// Resolution order:
///
/// 1. An explicit option name matching `explicit_default` (trimmed,
///    case-insensitive).
/// 2. The first option marked `recommended` (the word appearing anywhere in
///    the option text, case-insensitive).
/// 3. The first option.
///
/// Returns `None` when there are no options at all (a free-text question).
fn resolve_default_index(options: &[String], explicit_default: Option<&str>) -> Option<usize> {
    if options.is_empty() {
        return None;
    }
    if let Some(name) = explicit_default {
        let name = name.trim();
        if !name.is_empty() {
            if let Some(i) = options.iter().position(|o| o.eq_ignore_ascii_case(name)) {
                return Some(i);
            }
        }
    }
    if let Some(i) = options
        .iter()
        .position(|o| o.to_ascii_lowercase().contains("recommended"))
    {
        return Some(i);
    }
    Some(0)
}

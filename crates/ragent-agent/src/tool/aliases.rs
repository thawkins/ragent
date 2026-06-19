//! Alias tools that map commonly hallucinated tool names to canonical implementations.
//!
//! Many LLMs emit tool names that differ from ragent's canonical names — either
//! because they have been trained on different coding-agent frameworks or because
//! they extrapolate plausible-sounding names from the task context.  Rather than
//! returning "Unknown tool" errors, each alias tool normalises its parameter names
//! and delegates to the canonical implementation.
//!
//! ## Aliases provided
//!
//! | Alias name          | Canonical tool | Notes                              |
//! |---------------------|----------------|------------------------------------|
//! | `update_file`       | `write`        | `content` pass-through            |
//! | `run_code`          | `bash`         | `code` → `command`                |
//! | `ask_user`          | (self)         | free-text or multiple-choice user prompt |

use anyhow::{Context, Result};
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use super::{Tool, ToolContext, ToolOutput};
use super::{bash, write};
use crate::event::Event;

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
    // Then `cmd` — may be a string or an array
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
// update_file → write
// ---------------------------------------------------------------------------

/// Alias for `write`. Accepts `path` and `content`.
pub struct UpdateFileTool;

#[async_trait::async_trait]
impl Tool for UpdateFileTool {
    fn name(&self) -> &'static str {
        "update_file"
    }

    fn description(&self) -> &'static str {
        "Write new content to an existing file, replacing its current contents. \
         Alias for 'write'. Use 'path' and 'content'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path":    { "type": "string", "description": "Path to the file to update" },
                "content": { "type": "string", "description": "New content to write" }
            },
            "required": ["path", "content"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        delegate(&write::WriteTool, input, ctx).await
    }
}

// ---------------------------------------------------------------------------
// Bash execution aliases
// ---------------------------------------------------------------------------

/// Alias for `bash`. Accepts `code` (maps to `command`).
pub struct RunCodeTool;

#[async_trait::async_trait]
impl Tool for RunCodeTool {
    fn name(&self) -> &'static str {
        "run_code"
    }

    fn description(&self) -> &'static str {
        "Run a code snippet. Alias for 'bash'. \
         Provide the snippet via 'code' or 'command'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "code":    { "type": "string", "description": "Code or command to run" },
                "command": { "type": "string", "description": "Shell command (alternative to 'code')" },
                "timeout": { "type": "integer", "description": "Timeout in seconds (default: 120)" }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "bash:execute"
    }

    async fn execute(&self, mut input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        if extract_command(&mut input).is_none() {
            anyhow::bail!("Missing required 'command', 'code', or 'cmd' parameter");
        }
        delegate(&bash::BashTool, input, ctx).await
    }
}

/// Prompts the user for input during a session, then blocks until they respond.
///
/// Some models emit `ask_user` when they want to prompt the user for input.
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
        "ask_user"
    }

    fn description(&self) -> &'static str {
        "Ask the user a question and wait for their typed response. \
         Use this when you need clarification, prioritisation help, or confirmation \
         before proceeding. \
         \
         When you need a choice from a fixed set, provide the optional `options` \
         parameter as an array of strings (e.g. [\"Yes\", \"No\", \"Skip\"]). \
         The user will see a multiple-choice dialog instead of a free-text input, \
         and their selection is returned as the result. \
         If `options` is omitted the user sees a plain text-input dialog."
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
                }
            },
            "required": ["question"]
        })
    }

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

        let request_id = uuid::Uuid::new_v4().to_string();

        // Subscribe before publishing so we don't miss the reply.
        let mut rx = ctx.event_bus.subscribe();

        ctx.event_bus.publish(Event::QuestionRequested {
            session_id: ctx.session_id.clone(),
            request_id: request_id.clone(),
            question: question.to_string(),
            options,
        });

        // Wait for a matching QuestionAnswered event from the TUI.
        let response = loop {
            match rx.recv().await {
                Ok(Event::QuestionAnswered {
                    session_id: ref s,
                    request_id: ref rid,
                    response: ref r,
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

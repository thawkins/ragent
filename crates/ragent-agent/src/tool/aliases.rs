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
//! | `ask_user`          | `question`     | free-text question to the user    |

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use super::{bash, question, write};

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

/// Alias: `ask_user` → [`question::QuestionTool`].
///
/// Some models emit `ask_user` when they want to prompt the user for input.
pub struct AskUserTool;

#[async_trait::async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &'static str {
        "ask_user"
    }

    fn description(&self) -> &'static str {
        "Ask the user a question and wait for their typed response. \
         Use this when you need clarification or prioritisation help before proceeding."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to ask the user"
                }
            },
            "required": ["question"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "question"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        delegate(&question::QuestionTool, input, ctx).await
    }
}

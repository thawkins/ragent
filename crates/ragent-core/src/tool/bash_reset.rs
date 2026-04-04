//! `bash_reset` — Reset the persistent shell state for this session.
//!
//! Removes the saved environment/cwd state file so that the next `bash`
//! command starts fresh from the agent's working directory.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use super::bash::state_file_path;

/// Resets the persistent shell state for the current session.
pub struct BashResetTool;

#[async_trait::async_trait]
impl Tool for BashResetTool {
    fn name(&self) -> &str {
        "bash_reset"
    }

    fn description(&self) -> &str {
        "Reset the persistent shell state (clears the saved working directory and environment \
         variables). Use when the shell is in a bad state or you want to start fresh from \
         the agent's working directory."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    fn permission_category(&self) -> &str {
        "bash:execute"
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let state_file = state_file_path(&ctx.session_id);
        if std::path::Path::new(&state_file).exists() {
            std::fs::remove_file(&state_file)?;
        }
        Ok(ToolOutput {
            content: format!(
                "Shell state reset. Next command will start from: {}",
                ctx.working_dir.display()
            ),
            metadata: None,
        })
    }
}

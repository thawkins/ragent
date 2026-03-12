//! Shell command execution tool.
//!
//! Provides [`BashTool`], which runs shell commands via `bash -c` in the
//! agent's working directory with configurable timeouts.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::time::Instant;
use tokio::process::Command;

use super::{Tool, ToolContext, ToolOutput};

/// Executes shell commands via `bash -c` and returns combined stdout/stderr output.
///
/// Output is truncated to 100 KB to avoid overwhelming the agent context.
/// Commands that exceed the configured timeout (default 120 s) are terminated.
pub struct BashTool;

const DEFAULT_TIMEOUT_SECS: u64 = 120;

const DENIED_PATTERNS: &[&str] = &["rm -rf /", "mkfs", "dd if=", ":(){ :|:&};:", "> /dev/sd"];

#[async_trait::async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return stdout and stderr. \
         Commands are run with bash -c in the working directory."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 120)"
                }
            },
            "required": ["command"]
        })
    }

    fn permission_category(&self) -> &str {
        "bash:execute"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let command = input["command"]
            .as_str()
            .context("Missing required 'command' parameter")?;
        let timeout_secs = input["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_SECS);

        tracing::info!(command = %command, working_dir = %ctx.working_dir.display(), "Executing bash command");

        for pattern in DENIED_PATTERNS {
            if command.contains(pattern) {
                bail!("Command rejected: contains dangerous pattern '{pattern}'. This pattern could cause irreversible damage to the system.");
            }
        }

        let start = Instant::now();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            Command::new("bash")
                .arg("-c")
                .arg(command)
                .current_dir(&ctx.working_dir)
                .output(),
        )
        .await;

        let elapsed_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let exit_code = output.status.code().unwrap_or(-1);

                let mut content = String::new();
                if !stdout.is_empty() {
                    content.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !content.is_empty() {
                        content.push('\n');
                    }
                    content.push_str("[stderr]\n");
                    content.push_str(&stderr);
                }
                if content.is_empty() {
                    content = "(no output)".to_string();
                }

                // Truncate very long output
                const MAX_OUTPUT: usize = 100_000;
                if content.len() > MAX_OUTPUT {
                    content.truncate(MAX_OUTPUT);
                    content.push_str("\n... (output truncated)");
                }

                Ok(ToolOutput {
                    content: format!(
                        "Exit code: {}\nDuration: {}ms\n\n{}",
                        exit_code, elapsed_ms, content
                    ),
                    metadata: Some(json!({
                        "exit_code": exit_code,
                        "duration_ms": elapsed_ms,
                    })),
                })
            }
            Ok(Err(e)) => Err(anyhow::anyhow!(
                "Failed to execute command: {}. Check that the command exists and is accessible.",
                e
            )),
            Err(_) => Ok(ToolOutput {
                content: format!("Command timed out after {} seconds", timeout_secs),
                metadata: Some(json!({
                    "timeout": true,
                    "timeout_secs": timeout_secs,
                })),
            }),
        }
    }
}

//! Python code execution tool.
//!
//! Provides [`ExecutePythonTool`], which writes a Python snippet to a temporary
//! file and runs it via `python3`, returning stdout and stderr.  The temporary
//! file is deleted after execution.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tokio::process::Command;

use super::{Tool, ToolContext, ToolOutput};

const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Execute a Python code snippet using the system `python3` interpreter.
pub struct ExecutePythonTool;

#[async_trait::async_trait]
impl Tool for ExecutePythonTool {
    fn name(&self) -> &'static str {
        "execute_python"
    }

    fn description(&self) -> &'static str {
        "Execute a Python code snippet using the system python3 interpreter. \
         The snippet is written to a temporary file and executed. Returns stdout \
         and stderr. For interactive or long-running scripts, use 'bash' instead."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "Python code to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 30)"
                }
            },
            "required": ["code"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "bash:execute"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let code = input["code"]
            .as_str()
            .context("Missing required 'code' parameter")?;
        let timeout_secs = input["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_SECS);

        // Write to a temporary file
        let tmp_path = ctx.working_dir.join(format!(
            ".ragent_py_{}.py",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));

        tokio::fs::write(&tmp_path, code)
            .await
            .with_context(|| "Failed to write Python snippet to temp file")?;

        let start = Instant::now();

        let result = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            Command::new("python3")
                .arg(&tmp_path)
                .current_dir(&ctx.working_dir)
                .output(),
        )
        .await;

        let elapsed_ms = start.elapsed().as_millis() as u64;

        // Always clean up
        let _ = tokio::fs::remove_file(&tmp_path).await;

        match result {
            Err(_) => Ok(ToolOutput {
                content: format!("Python execution timed out after {timeout_secs}s"),
                metadata: Some(json!({
                    "timed_out": true,
                    "timeout_secs": timeout_secs,
                })),
            }),
            Ok(Err(e)) => Err(anyhow::anyhow!(
                "Failed to launch python3: {e}. Check that python3 is installed and accessible."
            )),
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
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

                let line_count = content.lines().count();

                Ok(ToolOutput {
                    content: format!(
                        "Exit code: {exit_code}\nDuration: {elapsed_ms}ms\n\n{content}"
                    ),
                    metadata: Some(json!({
                        "exit_code": exit_code,
                        "duration_ms": elapsed_ms,
                        "line_count": line_count,
                    })),
                })
            }
        }
    }
}

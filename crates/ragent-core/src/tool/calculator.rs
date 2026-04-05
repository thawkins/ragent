//! Math expression calculator tool.
//!
//! Provides [`CalculatorTool`], which evaluates a mathematical expression by
//! delegating to `python3 -c "print(<expr>)"`.  This gives access to Python's
//! full numeric tower (integers, floats, complex, `math` module) without adding
//! a Rust expression-parser dependency.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::process::Command;

use super::{Tool, ToolContext, ToolOutput};

/// Evaluate a math expression and return the result.
pub struct CalculatorTool;

#[async_trait::async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &'static str {
        "calculator"
    }

    fn description(&self) -> &'static str {
        "Evaluate a mathematical expression and return the result. \
         Supports Python arithmetic, the 'math' module (e.g. math.sqrt, math.pi), \
         and integer/float/complex numbers. \
         Examples: '2 ** 32', 'math.factorial(20)', '(3+4j) * 2'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Mathematical expression to evaluate (Python syntax)"
                }
            },
            "required": ["expression"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "bash:execute"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let expr = input["expression"]
            .as_str()
            .context("Missing required 'expression' parameter")?;

        // Use python3 to evaluate — sandboxed to a single print statement.
        let code = format!("import math; print({expr})");

        let result = tokio::time::timeout(
            Duration::from_secs(10),
            Command::new("python3")
                .arg("-c")
                .arg(&code)
                .current_dir(&ctx.working_dir)
                .output(),
        )
        .await;

        match result {
            Err(_) => anyhow::bail!("Calculator timed out"),
            Ok(Err(e)) => anyhow::bail!("Failed to launch python3 for calculation: {e}"),
            Ok(Ok(output)) => {
                if !output.status.success() {
                    let err = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Expression error: {err}");
                }
                let answer = String::from_utf8_lossy(&output.stdout).trim().to_string();
                Ok(ToolOutput {
                    content: format!("{expr} = {answer}"),
                    metadata: Some(json!({
                        "expression": expr,
                        "result": answer,
                    })),
                })
            }
        }
    }
}

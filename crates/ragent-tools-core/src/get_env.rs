//! Environment variable read tool.
//!
//! Provides [`GetEnvTool`], which reads one or more environment variables and
//! returns their values.  Sensitive variables (containing `KEY`, `SECRET`,
//! `TOKEN`, `PASSWORD`, or `PASS`) are redacted to avoid leaking credentials.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Read environment variables.
pub struct GetEnvTool;

/// Variable names containing these substrings are redacted.
const SENSITIVE_PATTERNS: &[&str] = &["KEY", "SECRET", "TOKEN", "PASSWORD", "PASS", "CREDENTIAL"];

fn is_sensitive(name: &str) -> bool {
    let upper = name.to_uppercase();
    SENSITIVE_PATTERNS.iter().any(|p| upper.contains(p))
}

#[async_trait::async_trait]
impl Tool for GetEnvTool {
    fn name(&self) -> &'static str {
        "get_env"
    }

    fn description(&self) -> &'static str {
        "Read the value of one or more environment variables. \
         Sensitive variables (containing KEY, SECRET, TOKEN, PASSWORD, etc.) \
         are redacted. Use 'name' for a single variable or 'names' for a list."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of a single environment variable to read"
                },
                "names": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of environment variable names to read"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        // Collect the list of variable names to look up
        let mut names: Vec<String> = Vec::new();

        if let Some(n) = input["name"].as_str() {
            names.push(n.to_string());
        }
        if let Some(arr) = input["names"].as_array() {
            for v in arr {
                if let Some(s) = v.as_str() {
                    names.push(s.to_string());
                }
            }
        }

        if names.is_empty() {
            anyhow::bail!("Provide 'name' or 'names' parameter");
        }

        let mut lines: Vec<String> = Vec::new();
        let mut meta = serde_json::Map::new();

        for name in &names {
            match std::env::var(name) {
                Ok(val) => {
                    let display = if is_sensitive(name) {
                        "***REDACTED***".to_string()
                    } else {
                        val.clone()
                    };
                    lines.push(format!("{name}={display}"));
                    meta.insert(name.clone(), json!(display));
                }
                Err(_) => {
                    lines.push(format!("{name}=(not set)"));
                    meta.insert(name.clone(), Value::Null);
                }
            }
        }

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(Value::Object(meta)),
        })
    }
}

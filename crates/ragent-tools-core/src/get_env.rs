//! Environment variable read tool.
//!
//! Provides [`GetEnvTool`], which reads one or more environment variables and
//! returns their values.  Sensitive variables (containing `KEY`, `SECRET`,
//! `TOKEN`, `PASSWORD`, or `PASS`) are redacted to avoid leaking credentials.
//!
//! Name matching alone is not sufficient: a credential can sit in a variable
//! whose name carries none of those substrings. Values are therefore also run
//! through [`ragent_types::sanitize::redact_secrets`], which masks recognised
//! key shapes (`sk-...`, `ghp_...`, `Bearer ...`, registered secrets, ...)
//! regardless of the variable name (audit T-109).

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Read environment variables.
pub struct GetEnvTool;

/// Variable names containing these substrings are redacted.
const SENSITIVE_PATTERNS: &[&str] = &["KEY", "SECRET", "TOKEN", "PASSWORD", "PASS", "CREDENTIAL"];

fn is_sensitive(name: &str) -> bool {
    // Case-insensitive substring test that avoids allocating an uppercased
    // copy of the name on every call. `name` is an environment variable name,
    // so ASCII case folding is exact for every realistic input.
    SENSITIVE_PATTERNS
        .iter()
        .any(|p| contains_ignore_ascii_case_needle(name, p))
}

/// Whether `haystack` contains `needle` under ASCII-case-insensitive matching.
///
/// Scans byte windows of the same length as `needle`. Both the haystack and the
/// needle are compared with `eq_ignore_ascii_case`, which only folds ASCII
/// letters; a non-ASCII byte therefore only matches an identical byte, so a
/// UTF-8 multibyte sequence cannot spuriously match a different character.
fn contains_ignore_ascii_case_needle(haystack: &str, needle: &str) -> bool {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() {
        return true;
    }
    if h.len() < n.len() {
        return false;
    }
    h.windows(n.len())
        .any(|window| window.eq_ignore_ascii_case(n))
}

/// Redact a variable value for display.
///
/// Returns the masked value when the name looks sensitive *or* when the value
/// matches a known secret shape, so a credential in a differently-named
/// variable is not returned verbatim.
fn redact_value(name: &str, value: &str) -> String {
    if is_sensitive(name) {
        return "***REDACTED***".to_string();
    }
    let redacted = ragent_types::sanitize::redact_secrets(value);
    if redacted != value {
        redacted
    } else {
        value.to_string()
    }
}

#[async_trait::async_trait]
impl Tool for GetEnvTool {
    fn name(&self) -> &'static str {
        "get_env"
    }

    fn description(&self) -> &'static str {
        "Read the value of one or more environment variables. No parameters are \
         strictly required, but you must provide at least one of `name` (string) \
         for a single variable or `names` (array of strings) for multiple \
         variables. Variable names containing KEY, SECRET, TOKEN, PASSWORD, or \
         similar substrings are redacted, and any value matching a recognised \
         credential shape is masked even when the name is innocuous. The \
         result returns each requested variable's value or `(not set)` if it \
         is not defined."
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
            },
            "additionalProperties": false
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
            if let Ok(val) = std::env::var(name) {
                let display = redact_value(name, &val);
                lines.push(format!("{name}={display}"));
                meta.insert(name.clone(), json!(display));
            } else {
                lines.push(format!("{name}=(not set)"));
                meta.insert(name.clone(), Value::Null);
            }
        }

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(Value::Object(meta)),
        })
    }
}

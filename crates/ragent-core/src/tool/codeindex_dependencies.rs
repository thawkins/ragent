//! Codebase index dependency graph tool.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Query file-level dependency relationships from the code index.
pub struct CodeIndexDependenciesTool;

fn not_available() -> ToolOutput {
    ToolOutput {
        content: "Code index is not available. It may be disabled or not yet initialised. \
                  Use `grep` to search for import/use statements manually."
            .to_string(),
        metadata: Some(json!({
            "error": "codeindex_disabled",
            "fallback_tools": ["grep"]
        })),
    }
}

#[async_trait::async_trait]
impl Tool for CodeIndexDependenciesTool {
    fn name(&self) -> &'static str {
        "codeindex_dependencies"
    }

    fn description(&self) -> &'static str {
        "Query file-level dependencies from the code index. \
         Show what a file imports or what other files depend on it."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative file path to query dependencies for"
                },
                "direction": {
                    "type": "string",
                    "description": "Direction: 'imports' (what this file uses) or 'dependents' (what uses this file)",
                    "enum": ["imports", "dependents"],
                    "default": "imports"
                }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "codeindex:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let idx = match &ctx.code_index {
            Some(idx) => idx,
            None => return Ok(not_available()),
        };

        let path = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;

        let direction = match input["direction"].as_str() {
            Some("dependents") => ragent_code::types::DepDirection::Dependents,
            _ => ragent_code::types::DepDirection::Imports,
        };

        let deps = idx.dependencies(path, direction)?;

        if deps.is_empty() {
            let dir_label = match direction {
                ragent_code::types::DepDirection::Imports => "imports",
                ragent_code::types::DepDirection::Dependents => "dependents",
            };
            return Ok(ToolOutput {
                content: format!("No {dir_label} found for '{path}'."),
                metadata: Some(json!({"total_results": 0})),
            });
        }

        let dir_label = match direction {
            ragent_code::types::DepDirection::Imports => "Imports",
            ragent_code::types::DepDirection::Dependents => "Dependents",
        };

        let mut output = format!("{dir_label} for `{path}`:\n");
        for dep in &deps {
            output.push_str(&format!("  - {dep}\n"));
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({"total_results": deps.len()})),
        })
    }
}

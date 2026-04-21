//! AIWiki import tool for agents.
//!
//! Provides [`AiwikiImportTool`], which imports external markdown files
//! into the AIWiki knowledge base.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Import external markdown into the AIWiki knowledge base.
pub struct AiwikiImportTool;

/// Build a "not initialized" response.
fn not_initialized() -> ToolOutput {
    ToolOutput {
        content:
            "AIWiki is not initialized. Run `/aiwiki init` first to create the wiki structure."
                .to_string(),
        metadata: Some(json!({
            "error": "aiwiki_not_initialized",
            "initialized": false
        })),
    }
}

/// Build a "disabled" response.
fn disabled() -> ToolOutput {
    ToolOutput {
        content: "AIWiki is currently disabled. Run `/aiwiki on` to enable it.".to_string(),
        metadata: Some(json!({
            "error": "aiwiki_disabled",
            "enabled": false
        })),
    }
}

#[async_trait::async_trait]
impl Tool for AiwikiImportTool {
    fn name(&self) -> &'static str {
        "aiwiki_import"
    }

    fn description(&self) -> &'static str {
        "Import external markdown files into the AIWiki knowledge base. \
         Can import a single file, a directory recursively, or an entire \
         Obsidian vault. The imported files are placed in the wiki directory \
         and become part of the knowledge base."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to markdown file or directory to import"
                },
                "target_subdir": {
                    "type": "string",
                    "description": "Optional subdirectory in wiki/ to place imported files (e.g., 'imports', 'external')",
                    "default": "imports"
                }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "aiwiki:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Check if AIWiki exists
        if !ragent_aiwiki::Aiwiki::exists(&ctx.working_dir) {
            return Ok(not_initialized());
        }

        let wiki = match ragent_aiwiki::Aiwiki::new(&ctx.working_dir).await {
            Ok(w) => w,
            Err(e) => {
                return Ok(ToolOutput {
                    content: format!("Failed to load AIWiki: {}", e),
                    metadata: Some(json!({"error": e.to_string()})),
                });
            }
        };

        if !wiki.config.enabled {
            return Ok(disabled());
        }

        let source_path = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;
        let source_path = ctx.working_dir.join(source_path);

        if !source_path.exists() {
            return Ok(ToolOutput {
                content: format!("Path not found: {}", source_path.display()),
                metadata: Some(json!({"error": "path_not_found"})),
            });
        }

        let target_subdir = input["target_subdir"].as_str();

        match ragent_aiwiki::import_markdown(&wiki, &source_path, target_subdir).await {
            Ok(count) => {
                let target_desc = if let Some(subdir) = target_subdir {
                    format!("wiki/{}/", subdir)
                } else {
                    "wiki/".to_string()
                };

                Ok(ToolOutput {
                    content: format!(
                        "## Import Complete ✅\n\n\
                         Imported {} markdown file(s) from:\n\
                         `{}`\n\n\
                         Target: aiwiki/{}\n\n\
                         Run `/aiwiki sync` to process and cross-link the imported content.",
                        count,
                        source_path.display(),
                        target_desc
                    ),
                    metadata: Some(json!({
                        "imported_count": count,
                        "source_path": source_path.to_string_lossy().to_string(),
                        "target_subdir": target_subdir
                    })),
                })
            }
            Err(e) => Ok(ToolOutput {
                content: format!("Import failed: {}", e),
                metadata: Some(json!({"error": e.to_string()})),
            }),
        }
    }
}

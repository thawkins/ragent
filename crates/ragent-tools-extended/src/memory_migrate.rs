//! `memory_migrate` — Migrate a flat MEMORY.md into structured memory blocks.
//!
//! This tool analyses an existing MEMORY.md file, proposes a split into
//! named blocks based on markdown headings, and optionally executes the
//! migration. The original MEMORY.md is never deleted.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::memory::block::BlockScope;
use crate::memory::migrate::migrate_memory_md;
use crate::memory::storage::FileBlockStorage;

/// Tool for migrating a flat MEMORY.md into named memory blocks.
pub struct MemoryMigrateTool;

#[async_trait::async_trait]
impl Tool for MemoryMigrateTool {
    fn name(&self) -> &'static str {
        "memory_migrate"
    }

    fn description(&self) -> &'static str {
        "Analyse a flat MEMORY.md file and propose splitting it into named memory blocks \
         based on markdown headings. Set execute=true to perform the migration (original \
         MEMORY.md is preserved). Set execute=false (default) for a dry-run preview."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["user", "project", "global"],
                    "description": "Memory scope to migrate. Default: 'project'"
                },
                "execute": {
                    "type": "boolean",
                    "description": "Set true to actually create the block files. Default: false (dry-run only)."
                }
            },
            "required": []
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let scope_str = input["scope"].as_str().unwrap_or("project");
        let execute = input["execute"].as_bool().unwrap_or(false);
        let scope = BlockScope::from_param(scope_str).unwrap_or(BlockScope::Project);
        let storage = FileBlockStorage::new();

        // Read the MEMORY.md file.
        let dir = crate::memory::block::resolve_block_dir(&scope, &ctx.working_dir)?;
        let mem_path = dir.join("MEMORY.md");

        if !mem_path.exists() {
            return Ok(ToolOutput {
                content: format!(
                    "No MEMORY.md found at {} (scope: {scope})",
                    mem_path.display()
                ),
                metadata: Some(json!({
                    "scope": scope.as_str(),
                    "found": false
                })),
            });
        }

        let content = std::fs::read_to_string(&mem_path)
            .with_context(|| format!("Failed to read {}", mem_path.display()))?;

        if content.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!(
                    "MEMORY.md at {} is empty — nothing to migrate.",
                    mem_path.display()
                ),
                metadata: Some(json!({
                    "scope": scope.as_str(),
                    "found": true,
                    "empty": true
                })),
            });
        }

        let plan = migrate_memory_md(&content, &scope, &ctx.working_dir, &storage, execute)?;

        let mode = if execute { "executed" } else { "dry-run" };
        Ok(ToolOutput {
            content: format!("Migration {} ({scope} scope):\n\n{}", mode, plan.summary()),
            metadata: Some(json!({
                "scope": scope.as_str(),
                "execute": execute,
                "would_create": plan.would_create,
                "would_skip": plan.would_skip,
                "section_count": plan.sections.len()
            })),
        })
    }
}

//! `memory_write` / `memory_read` / `memory_replace` — Persistent memory tools.
//!
//! Agents call these tools to remember facts across sessions. Memory files are
//! automatically loaded into the system prompt at the start of each session.
//!
//! # Block-based memory (Milestone 1)
//!
//! In addition to the legacy flat `MEMORY.md` file, agents can now create
//! named memory blocks with YAML frontmatter. A block is a `.md` file in
//! the memory directory whose filename (minus `.md`) serves as the label.
//!
//! Backward compatibility: when `label` is not provided, the tools fall back
//! to the original `MEMORY.md` behaviour.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::memory::block::{BlockScope, MemoryBlock};
use crate::memory::storage::{BlockStorage, FileBlockStorage};

// ── MemoryWriteTool ───────────────────────────────────────────────────────────

/// Tool for persisting notes to user or project memory files.
///
/// Supports both legacy mode (writing to `MEMORY.md`) and block mode
/// (writing to `<label>.md` with YAML frontmatter).
pub struct MemoryWriteTool;

#[async_trait::async_trait]
impl Tool for MemoryWriteTool {
    fn name(&self) -> &'static str {
        "memory_write"
    }

    fn description(&self) -> &'static str {
        "Persist notes or learnings to memory files that are automatically loaded in future \
         sessions. Use scope='user' for global memory (~/.ragent/memory/) or \
         scope='project' for project-specific memory (.ragent/memory/ in the \
         working directory). When 'label' is provided, writes to a named memory block \
         (<label>.md) with optional YAML frontmatter; otherwise writes to the default \
         MEMORY.md file (backward compatible)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The content to write to the memory file"
                },
                "scope": {
                    "type": "string",
                    "enum": ["user", "project", "global"],
                    "description": "Memory scope: 'user' or 'global' for global (~/.ragent/memory/) or 'project' for project-level (.ragent/memory/). Default: 'project'"
                },
                "label": {
                    "type": "string",
                    "description": "Named memory block label (e.g. 'patterns', 'persona'). When provided, writes to <label>.md with YAML frontmatter. Must be lowercase with hyphens."
                },
                "description": {
                    "type": "string",
                    "description": "Short description of the memory block's purpose (only used when 'label' is provided)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum content size in bytes for this block (0 = no limit). Only used when 'label' is provided."
                },
                "mode": {
                    "type": "string",
                    "enum": ["append", "overwrite"],
                    "description": "Write mode: 'append' adds to the end (default), 'overwrite' replaces the entire content. Only used when 'label' is provided."
                },
                "path": {
                    "type": "string",
                    "description": "Filename within the memory directory (default: MEMORY.md). Legacy parameter — use 'label' instead for structured blocks."
                }
            },
            "required": ["content"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let content = input["content"]
            .as_str()
            .context("Missing required 'content' parameter")?;

        let scope_str = input["scope"].as_str().unwrap_or("project");
        let label = input["label"].as_str();
        let mode = input["mode"].as_str().unwrap_or("append");
        let block_description = input["description"].as_str().unwrap_or("");
        let limit = input["limit"].as_u64().map(|l| l as usize).unwrap_or(0);

        let scope = BlockScope::from_param(scope_str).unwrap_or(BlockScope::Project);
        let storage = FileBlockStorage::new();

        if let Some(label_str) = label {
            // Block-based write.
            MemoryBlock::validate_label(label_str).map_err(|e| anyhow::anyhow!("{e}"))?;

            let existing = storage.load(label_str, &scope, &ctx.working_dir)?;

            // Check read-only on existing block regardless of mode.
            if let Some(ref block) = existing {
                if block.read_only {
                    anyhow::bail!(
                        "Memory block '{}' is read-only and cannot be modified",
                        label_str
                    );
                }
            }

            let block = if mode == "overwrite" {
                MemoryBlock::new(label_str, scope.clone())
                    .with_description(block_description)
                    .with_limit(limit)
                    .with_content(content.to_string())
            } else {
                // Append mode.
                let mut block = existing.unwrap_or_else(|| {
                    MemoryBlock::new(label_str, scope.clone())
                        .with_description(block_description)
                        .with_limit(limit)
                });

                if !block_description.is_empty() && block.description.is_empty() {
                    block.description = block_description.to_string();
                }
                if limit > 0 && block.limit == 0 {
                    block.limit = limit;
                }
                if block.read_only {
                    anyhow::bail!(
                        "Memory block '{}' is read-only and cannot be modified",
                        label_str
                    );
                }
                // Append content with a timestamp separator.
                let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
                block
                    .content
                    .push_str(&format!("\n\n<!-- {now} -->\n{content}\n"));
                block.updated_at = chrono::Utc::now();
                block
            };

            // Check limit before saving.
            if let Err(e) = block.check_content_limit() {
                anyhow::bail!("{e}");
            }

            storage.save(&block, &ctx.working_dir)?;

            let dir = crate::memory::block::resolve_block_dir(&block.scope, &ctx.working_dir)?;
            let file_path = dir.join(block.filename());
            let content_line_count = content.lines().count();

            Ok(ToolOutput {
                content: format!(
                    "Memory written to {} (scope: {}, label: {}, mode: {})\n\n{content}",
                    file_path.display(),
                    block.scope,
                    label_str,
                    mode
                ),
                metadata: Some(json!({
                    "file": file_path.display().to_string(),
                    "scope": scope.as_str(),
                    "label": label_str,
                    "mode": mode,
                    "byte_count": content.len(),
                    "line_count": content_line_count
                })),
            })
        } else {
            // Legacy write — backward compatible with existing behaviour.
            let filename = input["path"].as_str().unwrap_or("MEMORY.md");
            legacy_write(content, scope_str, filename, &ctx.working_dir)
        }
    }
}

// ── MemoryReadTool ────────────────────────────────────────────────────────────

/// Tool for reading back content from user or project memory files.
///
/// Supports both legacy mode (reading `MEMORY.md`) and block mode
/// (reading `<label>.md` with YAML frontmatter).
pub struct MemoryReadTool;

#[async_trait::async_trait]
impl Tool for MemoryReadTool {
    fn name(&self) -> &'static str {
        "memory_read"
    }

    fn description(&self) -> &'static str {
        "Read the contents of a memory file. Use to recall facts persisted by memory_write. \
         When 'label' is provided, reads the named memory block (<label>.md); \
         otherwise reads the default MEMORY.md file (backward compatible)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["user", "project", "global"],
                    "description": "Memory scope: 'user' or 'global' for global (~/.ragent/memory/) or 'project' for project-level (.ragent/memory/)"
                },
                "label": {
                    "type": "string",
                    "description": "Named memory block label to read (e.g. 'patterns', 'persona'). When omitted, reads the default MEMORY.md."
                },
                "path": {
                    "type": "string",
                    "description": "Filename within the memory directory (default: MEMORY.md). Legacy parameter — use 'label' instead for structured blocks."
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "none"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let scope_str = input["scope"].as_str().unwrap_or("project");
        let label = input["label"].as_str();
        let scope = BlockScope::from_param(scope_str).unwrap_or(BlockScope::Project);
        let storage = FileBlockStorage::new();

        if let Some(label_str) = label {
            // Block-based read.
            MemoryBlock::validate_label(label_str).map_err(|e| anyhow::anyhow!("{e}"))?;

            // Try cross-project resolution when no explicit scope is given
            // or when the default scope is used.
            if scope == BlockScope::Project && scope_str == "project" {
                // Attempt cross-project resolution (global blocks accessible
                // from any project when cross_project is enabled).
                if let Some(resolved) = crate::memory::cross_project::resolve_block(
                    label_str,
                    &ctx.working_dir,
                    &crate::CrossProjectConfig::default(),
                    &storage,
                )? {
                    let dir = crate::memory::block::resolve_block_dir(
                        &resolved.winning_scope,
                        &ctx.working_dir,
                    )?;
                    let file_path = dir.join(resolved.block.filename());
                    let content_line_count = resolved.block.content.lines().count();
                    let byte_count = resolved.block.content.len();

                    let mut output = format!(
                        "Memory block '{}' (scope: {}, {}):\n\n{}",
                        resolved.block.label,
                        resolved.winning_scope,
                        file_path.display(),
                        resolved.block.content
                    );
                    if !resolved.block.description.is_empty() {
                        output = format!("Description: {}\n\n{output}", resolved.block.description);
                    }
                    if resolved.block.read_only {
                        output.push_str("\n\n[read-only]");
                    }
                    if resolved.block.limit > 0 {
                        output.push_str(&format!(
                            "\n[size: {}/{} bytes]",
                            byte_count, resolved.block.limit
                        ));
                    }
                    if resolved.shadowed {
                        output.push_str(
                            "\n\n[note: project block shadows a global block with the same label]",
                        );
                    }

                    return Ok(ToolOutput {
                        content: output,
                        metadata: Some(json!({
                            "file": file_path.display().to_string(),
                            "scope": resolved.winning_scope.as_str(),
                            "label": label_str,
                            "line_count": content_line_count,
                            "byte_count": byte_count,
                            "read_only": resolved.block.read_only,
                            "limit": resolved.block.limit,
                            "shadowed": resolved.shadowed,
                        })),
                    });
                }
            }

            // Fallback: direct scope-based lookup.
            match storage.load(label_str, &scope, &ctx.working_dir)? {
                Some(block) => {
                    let dir =
                        crate::memory::block::resolve_block_dir(&block.scope, &ctx.working_dir)?;
                    let file_path = dir.join(block.filename());
                    let content_line_count = block.content.lines().count();
                    let byte_count = block.content.len();

                    let mut output = format!(
                        "Memory block '{}' (scope: {}, {}):\n\n{}",
                        block.label,
                        block.scope,
                        file_path.display(),
                        block.content
                    );
                    if !block.description.is_empty() {
                        output = format!("Description: {}\n\n{output}", block.description);
                    }
                    if block.read_only {
                        output.push_str("\n\n[read-only]");
                    }
                    if block.limit > 0 {
                        output.push_str(&format!("\n[size: {}/{} bytes]", byte_count, block.limit));
                    }

                    Ok(ToolOutput {
                        content: output,
                        metadata: Some(json!({
                            "file": file_path.display().to_string(),
                            "scope": scope.as_str(),
                            "label": label_str,
                            "line_count": content_line_count,
                            "byte_count": byte_count,
                            "read_only": block.read_only,
                            "limit": block.limit
                        })),
                    })
                }
                None => Ok(ToolOutput {
                    content: format!("No memory block '{label_str}' found (scope: {scope})"),
                    metadata: Some(json!({
                        "scope": scope.as_str(),
                        "label": label_str,
                        "line_count": 0,
                        "byte_count": 0
                    })),
                }),
            }
        } else {
            // Legacy read — backward compatible.
            let filename = input["path"].as_str().unwrap_or("MEMORY.md");
            legacy_read(scope_str, filename, &ctx.working_dir)
        }
    }
}

// ── MemoryReplaceTool ────────────────────────────────────────────────────────

/// Tool for surgical edits within a memory block, analogous to the `edit` tool
/// for code files. Finds an exact string match within a block's content and
/// replaces it.
pub struct MemoryReplaceTool;

#[async_trait::async_trait]
impl Tool for MemoryReplaceTool {
    fn name(&self) -> &'static str {
        "memory_replace"
    }

    fn description(&self) -> &'static str {
        "Replace a specific string in a named memory block. Finds the exact 'old_str' \
         within the block content and replaces it with 'new_str'. Preserves YAML \
         frontmatter. The block must not be read-only."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "label": {
                    "type": "string",
                    "description": "Named memory block label (required). Must be lowercase with hyphens."
                },
                "old_str": {
                    "type": "string",
                    "description": "The exact text to find within the block content"
                },
                "new_str": {
                    "type": "string",
                    "description": "The replacement text"
                },
                "scope": {
                    "type": "string",
                    "enum": ["user", "project", "global"],
                    "description": "Memory scope: 'user' or 'global' for global, 'project' for project-level. Default: 'project'"
                }
            },
            "required": ["label", "old_str", "new_str"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let label = input["label"]
            .as_str()
            .context("Missing required 'label' parameter")?;
        let old_str = input["old_str"]
            .as_str()
            .context("Missing required 'old_str' parameter")?;
        let new_str = input["new_str"]
            .as_str()
            .context("Missing required 'new_str' parameter")?;
        let scope_str = input["scope"].as_str().unwrap_or("project");
        let scope = BlockScope::from_param(scope_str).unwrap_or(BlockScope::Project);
        let storage = FileBlockStorage::new();

        MemoryBlock::validate_label(label).map_err(|e| anyhow::anyhow!("{e}"))?;

        let mut block = storage
            .load(label, &scope, &ctx.working_dir)?
            .ok_or_else(|| {
                anyhow::anyhow!("Memory block '{}' not found (scope: {})", label, scope)
            })?;

        if block.read_only {
            anyhow::bail!(
                "Memory block '{}' is read-only and cannot be modified",
                label
            );
        }

        // Find and replace exact match.
        let count = block.content.matches(old_str).count();
        if count == 0 {
            anyhow::bail!(
                "old_str not found in memory block '{}'. No changes made.",
                label
            );
        }
        if count > 1 {
            anyhow::bail!(
                "old_str found {} times in memory block '{}'. Provide more context to make the match unique.",
                count,
                label
            );
        }

        block.content = block.content.replacen(old_str, new_str, 1);
        block.updated_at = chrono::Utc::now();

        // Check content limit after replacement.
        if let Err(e) = block.check_content_limit() {
            anyhow::bail!("{e}");
        }

        storage.save(&block, &ctx.working_dir)?;

        let dir = crate::memory::block::resolve_block_dir(&block.scope, &ctx.working_dir)?;
        let file_path = dir.join(block.filename());

        Ok(ToolOutput {
            content: format!(
                "Replaced 1 occurrence in memory block '{}' (scope: {})\n\nFile: {}",
                label,
                scope,
                file_path.display()
            ),
            metadata: Some(json!({
                "file": file_path.display().to_string(),
                "scope": scope.as_str(),
                "label": label,
                "replacements": 1
            })),
        })
    }
}

// ── Legacy helpers ───────────────────────────────────────────────────────────

/// Resolves the memory directory for the given scope (legacy string-based).
pub fn resolve_memory_dir(
    scope: &str,
    working_dir: &std::path::Path,
) -> Result<std::path::PathBuf> {
    match scope {
        "user" | "global" => {
            let home = dirs::home_dir()
                .context("Cannot determine home directory for user memory scope")?;
            Ok(home.join(".ragent").join("memory"))
        }
        _ => Ok(working_dir.join(".ragent").join("memory")),
    }
}

/// Legacy write implementation (backward compatible).
fn legacy_write(
    content: &str,
    scope: &str,
    filename: &str,
    working_dir: &std::path::Path,
) -> Result<ToolOutput> {
    let mem_dir = resolve_memory_dir(scope, working_dir)?;
    std::fs::create_dir_all(&mem_dir)
        .with_context(|| format!("Failed to create memory directory: {}", mem_dir.display()))?;

    let file_path = mem_dir.join(filename);
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let entry = format!("\n\n<!-- {now} -->\n{content}\n");

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .with_context(|| format!("Failed to open memory file: {}", file_path.display()))?;
    file.write_all(entry.as_bytes())
        .with_context(|| format!("Failed to write to memory file: {}", file_path.display()))?;

    let content_line_count = content.lines().count();
    Ok(ToolOutput {
        content: format!(
            "Memory written to {} (scope: {scope})\n\n{content}",
            file_path.display()
        ),
        metadata: Some(json!({
            "file": file_path.display().to_string(),
            "scope": scope,
            "byte_count": entry.len(),
            "line_count": content_line_count
        })),
    })
}

/// Legacy read implementation (backward compatible).
fn legacy_read(scope: &str, filename: &str, working_dir: &std::path::Path) -> Result<ToolOutput> {
    let mem_dir = resolve_memory_dir(scope, working_dir)?;
    let file_path = mem_dir.join(filename);

    if !file_path.exists() {
        return Ok(ToolOutput {
            content: format!(
                "No memory file found at {} (scope: {scope})",
                file_path.display()
            ),
            metadata: Some(json!({
                "file": file_path.display().to_string(),
                "scope": scope,
                "line_count": 0,
                "byte_count": 0
            })),
        });
    }

    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read memory file: {}", file_path.display()))?;

    let content_line_count = content.lines().count();
    let byte_count = content.len();
    Ok(ToolOutput {
        content: if content.trim().is_empty() {
            format!("Memory file {} is empty", file_path.display())
        } else {
            format!(
                "Memory ({scope} scope, {}):\n\n{content}",
                file_path.display()
            )
        },
        metadata: Some(json!({
            "file": file_path.display().to_string(),
            "scope": scope,
            "line_count": content_line_count,
            "byte_count": byte_count
        })),
    })
}

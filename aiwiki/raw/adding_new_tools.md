# Adding New Tools to Ragent

This guide provides step-by-step instructions for adding new tools to ragent while maintaining consistency with existing tools.

## Prerequisites

Before adding a new tool, review:
- [`docs/standards/tool_output.md`](./tool_output.md) - Output format standards
- [`docs/standards/tool_metadata_schema.md`](./tool_metadata_schema.md) - Metadata field definitions
- Existing tool implementations in `crates/ragent-core/src/tool/`

## Step-by-Step Guide

### 1. Define the Tool

First, determine:
- **Tool name** - Should be descriptive and use snake_case
- **Category** - File, search, execution, network, etc.
- **Output pattern** - A (Summary+Content), B (Summary Only), or C (Structured)

### 2. Create the Tool File

Create a new file in `crates/ragent-core/src/tool/`:

```rust
//! Brief description of what this tool does.
//!
//! Provides [`MyTool`], which ...

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use super::metadata::MetadataBuilder;
use super::format::{format_summary_content, format_line_count};

/// Brief description of the tool.
pub struct MyTool;

#[async_trait::async_trait]
impl Tool for MyTool {
    fn name(&self) -> &'static str {
        "my_tool"
    }

    fn description(&self) -> &'static str {
        "Clear description of what the tool does."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "param1": {
                    "type": "string",
                    "description": "Description of param1"
                }
            },
            "required": ["param1"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "my:category"
    }

    async fn execute(&self,
        input: Value,
        ctx: &ToolContext
    ) -> Result<ToolOutput> {
        // Implementation
        let param1 = input["param1"]
            .as_str()
            .context("Missing required 'param1' parameter")?;
        
        // Do work...
        let result = "Result content";
        let line_count = result.lines().count();
        
        // Build metadata using the builder
        let metadata = MetadataBuilder::new()
            .count(line_count)
            .custom("custom_field", "value")
            .build();
        
        Ok(ToolOutput {
            content: format_summary_content(
                format!("{} {}", format_line_count(line_count), "processed"),
                result
            ),
            metadata,
        })
    }
}
```

### 3. Register the Tool

Add to `crates/ragent-core/src/tool/mod.rs`:

```rust
/// My new tool.
pub mod my_tool;
```

Then register in the `ToolRegistry` (usually in `crates/ragent-core/src/agent/mod.rs` or where the registry is initialized):

```rust
registry.register(Box::new(my_tool::MyTool));
```

### 4. Add TUI Display Support

Update `crates/ragent-tui/src/widgets/message_widget.rs`:

#### Input Summary

Add a case in `tool_input_summary()`:

```rust
"my_tool" => {
    let param = get_str(&["param1", "alias_name"]).unwrap_or_default();
    format!("📄 {} {}", emoji_for_category, param)
}
```

Choose the appropriate emoji:
- 📄 File operations
- 📁 Directory operations
- 🔍 Search operations
- ⚡ Execution
- 🌐 Network
- ❓ User interaction
- 📋 Task management
- 💭 Reasoning
- 🔧 Environment

#### Result Summary

Add a case in `tool_result_summary()`:

```rust
"my_tool" => {
    let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let truncated = out.get("truncated").and_then(|v| v.as_bool()).unwrap_or(false);
    let trunc = if truncated { "+" } else { "" };
    Some(format!(
        "📄 {}{} processed",
        pluralize(count, "item", "items"),
        trunc
    ))
}
```

### 5. Add Tool Alias (Optional)

If the tool might be called by alternative names, add an alias in `aliases.rs`:

```rust
"alternative_name" => "my_tool",
```

### 6. Write Tests

Create a test file in `crates/ragent-core/tests/test_my_tool.rs`:

```rust
//! Tests for my_tool

use ragent_core::event::EventBus;
use ragent_core::tool::my_tool::MyTool;
use ragent_core::tool::{Tool, ToolContext};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn ctx() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
    }
}

#[tokio::test]
async fn test_my_tool_basic() {
    let result = MyTool
        .execute(json!({"param1": "value"}), &ctx())
        .await
        .unwrap();
    
    assert!(
        result.content.contains("expected content"),
        "Should contain expected content: {}",
        result.content
    );
}

#[tokio::test]
async fn test_my_tool_metadata() {
    let result = MyTool
        .execute(json!({"param1": "value"}), &ctx())
        .await
        .unwrap();
    
    let metadata = result.metadata.expect("should have metadata");
    assert!(
        metadata.get("count").is_some(),
        "should have 'count' field, got: {:?}",
        metadata
    );
}

#[tokio::test]
async fn test_my_tool_empty_result() {
    let result = MyTool
        .execute(json!({"param1": ""}), &ctx())
        .await
        .unwrap();
    
    assert!(
        result.content.contains("No results") || result.content.contains("0"),
        "Should handle empty results: {}",
        result.content
    );
}
```

### 7. Run Tests

```bash
cargo test --test test_my_tool -p ragent-core
```

Also run the full test suite to ensure no regressions:

```bash
cargo test -p ragent-core
cargo test -p ragent-tui
```

### 8. Update Documentation

Add the new tool to `docs/standards/tool_output.md` and `docs/standards/tool_metadata_schema.md` if it introduces new patterns or fields.

## Checklist

Before submitting a new tool:

### Implementation

- [ ] Tool implements the `Tool` trait correctly
- [ ] Tool has appropriate permission category
- [ ] Parameters schema is defined
- [ ] All errors are handled appropriately
- [ ] Tool uses standardized output patterns

### Metadata

- [ ] Uses `MetadataBuilder` for consistent field names
- [ ] Includes all relevant fields from schema
- [ ] No deprecated field names used
- [ ] Tool-specific fields documented

### Content

- [ ] Follows Pattern A, B, or C consistently
- [ ] Uses `format_*` helpers for formatting
- [ ] Proper pluralization (use `pluralize()`)
- [ ] Paths are relative in content, absolute in metadata
- [ ] Large outputs are truncated appropriately

### TUI Integration

- [ ] `tool_input_summary` case added
- [ ] `tool_result_summary` case added
- [ ] Appropriate emoji selected
- [ ] Summary format is concise

### Testing

- [ ] Test file created
- [ ] Basic functionality tested
- [ ] Metadata fields tested
- [ ] Edge cases handled (empty, single, large)
- [ ] All tests pass

### Documentation

- [ ] Module-level doc comments
- [ ] Function-level doc comments for public APIs
- [ ] Example usage in doc comments (if applicable)
- [ ] Added to standards documents if needed

## Common Patterns

### File Operations

```rust
// Pattern B: Summary Only
let metadata = MetadataBuilder::new()
    .path(&path)
    .line_count(lines)
    .byte_count(bytes)
    .build();

Ok(ToolOutput {
    content: format!(
        "Wrote {} ({} lines) to {}",
        format_bytes(bytes),
        format_line_count(lines),
        path
    ),
    metadata,
})
```

### Search Operations

```rust
// Pattern A: Summary + Content
let content = results.join("\n");
let metadata = MetadataBuilder::new()
    .count(results.len())
    .file_count(files.len())
    .truncated(was_truncated)
    .build();

Ok(ToolOutput {
    content: format_summary_content(
        format!("{} found", format_match_count(results.len())),
        content
    ),
    metadata,
})
```

### Execution Tools

```rust
// Pattern C: Structured
let metadata = MetadataBuilder::new()
    .exit_code(exit_code)
    .duration_ms(duration)
    .line_count(line_count)
    .timed_out(timed_out)
    .build();

Ok(ToolOutput {
    content: format_status_output(exit_code, stdout, stderr, duration, timed_out),
    metadata,
})
```

## Migration from Old Tools

If updating an existing tool to follow new standards:

1. Audit current metadata fields
2. Map to standard names using `MetadataBuilder`
3. Update content format to use helpers
4. Add TUI display cases
5. Update tests
6. Document the change in CHANGELOG.md

## Getting Help

If you're unsure about:
- **Which pattern to use** - Look at similar existing tools
- **Field names** - Check `tool_metadata_schema.md`
- **Formatting** - Use existing `format::*` helpers
- **TUI display** - Follow examples in `message_widget.rs`

## Example: Complete Tool Implementation

Here's a complete example of a tool that follows all standards:

```rust
//! A tool that counts words in files.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use super::metadata::MetadataBuilder;

/// Counts words in a file.
pub struct WordCountTool;

#[async_trait::async_trait]
impl Tool for WordCountTool {
    fn name(&self) -> &'static str {
        "word_count"
    }

    fn description(&self) -> &'static str {
        "Count words in a text file. Returns word count and optionally lists the words."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to analyze"
                },
                "list_words": {
                    "type": "boolean",
                    "description": "Whether to list all words found",
                    "default": false
                }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self,
        input: Value,
        ctx: &ToolContext
    ) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;
        
        let list_words = input["list_words"].as_bool().unwrap_or(false);
        
        let path = ctx.working_dir.join(path_str);
        let content = tokio::fs::read_to_string(&path).await
            .with_context(|| format!("Failed to read file: {}", path.display()))?;
        
        let words: Vec<&str> = content.split_whitespace().collect();
        let word_count = words.len();
        let line_count = content.lines().count();
        
        let output = if list_words {
            format_summary_content(
                format!("{} words found in {} lines", word_count, line_count),
                words.join("\n")
            )
        } else {
            format!("{} words found in {} lines", word_count, line_count)
        };
        
        let metadata = MetadataBuilder::new()
            .path(path_str)
            .count(word_count)
            .line_count(line_count)
            .custom("list_words", list_words)
            .build();
        
        Ok(ToolOutput {
            content: output,
            metadata,
        })
    }
}
```

TUI display additions:

```rust
// In tool_input_summary()
"word_count" => {
    let path = get_relative_path(&["path"]);
    format!("📄 {}", path)
}

// In tool_result_summary()
"word_count" => {
    let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let lines = out.get("lines").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    Some(format!(
        "📄 {} in {}",
        pluralize(count, "word", "words"),
        pluralize(lines, "line", "lines")
    ))
}
```
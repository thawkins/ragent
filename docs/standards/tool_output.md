# Tool Output Standards

This document defines the standardized patterns and conventions for tool output in ragent, ensuring consistent user experience across all tools.

## Overview

Tool output consists of two parts:
1. **`content`** - Human-readable text displayed to the user
2. **`metadata`** - Structured JSON data for programmatic access

Both parts follow standardized patterns to ensure consistency across the TUI and API.

## Content Format Patterns

### Pattern A: Summary + Content

**Use for**: Read operations, search results, query responses

**Format**:
```
Summary line (e.g., "42 lines read", "5 matches found")

Content line 1
Content line 2
...
```

**Examples**:
- `read` - "50 lines read\n\n[file content]"
- `grep` - "3 matches found in 2 files\n\n[matching lines]"
- `lsp_symbols` - "25 symbols found\n\n[symbol list]"

**Implementation**:
```rust
use ragent_core::tool::format::format_summary_content;

let output = format_summary_content(
    format!("{} {}", line_count, pluralize(line_count, "line", "lines")),
    content
);
```

### Pattern B: Summary Only

**Use for**: Write operations, simple state changes

**Format**:
```
Summary line (e.g., "Wrote 42 bytes to file.txt")
```

**Examples**:
- `write` - "10 lines written to src/main.rs"
- `create` - "5 lines created in new.txt"
- `todo_write` - "Task 'test' marked complete"

**Implementation**:
```rust
use ragent_core::tool::format::format_simple_summary;

let output = format_simple_summary(format!(
    "Wrote {} to {}",
    format_line_count(line_count),
    path
));
```

### Pattern C: Structured Output

**Use for**: Execution tools with exit codes and timing

**Format**:
```
Exit code: X
Duration: Yms (timed out)

STDOUT:
[stdout content]

STDERR:
[stderr content]
```

**Examples**:
- `bash` - "Exit code: 0\nDuration: 150ms\n\nSTDOUT:\nhello"
- `execute_python` - "Exit code: 1\nDuration: 200ms\n\nSTDERR:\nError"

**Implementation**:
```rust
use ragent_core::tool::format::format_status_output;

let output = format_status_output(
    exit_code,
    stdout,
    stderr,
    duration_ms,
    timed_out
);
```

## Metadata Schema

All tools should use standardized field names in metadata:

### Common Fields

| Field | Type | Description | Used By |
|-------|------|-------------|---------|
| `path` | string | Primary file/directory path | read, write, edit |
| `line_count` | integer | Number of lines in content | read, write, edit |
| `total_lines` | integer | Total lines before truncation | read, grep |
| `byte_count` | integer | Size in bytes | write, copy_file |
| `count` | integer | Generic count of items | grep, glob, list |
| `file_count` | integer | Number of files | grep, multiedit |
| `matches` | integer | Number of matches | grep (alias) |
| `entries` | integer | Directory entries | list (alias) |
| `exit_code` | integer | Process exit code | bash, execute_python |
| `duration_ms` | integer | Execution duration | bash, execute_python |
| `timed_out` | boolean | Whether execution timed out | bash, execute_python |
| `summarized` | boolean | Whether content was truncated | read |
| `truncated` | boolean | Alias for summarized | grep |
| `old_lines` | integer | Lines before edit | edit, multiedit |
| `new_lines` | integer | Lines after edit | edit, multiedit |

### Tool-Specific Fields

| Tool | Additional Fields |
|------|-------------------|
| `grep` | `pattern`, `file_count` |
| `glob` | `pattern` |
| `list` | `path` |
| `bash` | `command` (in input) |
| `edit` | `path` |
| `multiedit` | `edits`, `file_count` |

**Implementation**:
```rust
use ragent_core::tool::metadata::MetadataBuilder;

let metadata = MetadataBuilder::new()
    .path("src/main.rs")
    .line_count(42)
    .byte_count(1024)
    .build();

Ok(ToolOutput {
    content: output,
    metadata,
})
```

## TUI Display Guidelines

### Input Summary Format

Tool input should be summarized with an emoji indicator:

| Category | Emoji | Example |
|----------|-------|---------|
| File Operations | 📄 | "📄 src/main.rs" |
| Directory Operations | 📁 | "📁 src/" |
| Search Operations | 🔍 | "🔍 'pattern' in src/" |
| Execution | ⚡ | "⚡ $ cargo build" |
| Network | 🌐 | "🌐 https://example.com" |
| User Interaction | ❓ | "❓ What is your name?" |
| Task Management | 📋 | "📋 3 tasks" |

### Result Summary Format

Tool results should be summarized concisely:

| Tool Pattern | Example |
|--------------|---------|
| File read | "📄 42 lines read" |
| File write | "📄 10 lines written to file.txt" |
| Edit | "📄 Edited file.txt: 5 → 3 lines" |
| Search | "🔍 5 matches found in 3 files" |
| Execution | "⚡ 5 lines… (exit 0)" |

### Pluralization

Always use proper pluralization:

```rust
use ragent_core::tool::format::{format_line_count, format_file_count};

format_line_count(1);   // "1 line"
format_line_count(5);   // "5 lines"
format_file_count(1);   // "1 file"
format_file_count(5);   // "5 files"
```

## Content Truncation

Large outputs should be truncated with clear markers:

```rust
use ragent_core::tool::truncate::truncate_content;

let result = truncate_content(content, 50);
// If content has 100 lines, result will be:
// "line1
// line2
// ...
// line49
// ... (51 lines omitted) ..."
```

## Byte Formatting

Byte sizes should be formatted with appropriate units:

```rust
use ragent_core::tool::format::format_bytes;

format_bytes(512);              // "512 B"
format_bytes(1536);             // "1.5 KB"
format_bytes(1024 * 1024);      // "1.0 MB"
format_bytes(1024 * 1024 * 1024); // "1.0 GB"
```

## Path Display

Paths should be displayed relative to the working directory when possible:

```rust
use ragent_tui::widgets::message_widget::make_relative_path;

make_relative_path("/home/user/project/src/main.rs", "/home/user/project");
// Returns: "src/main.rs"
```

## Error Presentation

Tools should return `Err(...)` for execution failures, and `Ok(ToolOutput { content: "Error: ...", ... })` for expected error conditions:

```rust
// Execution error - returns Err
if !path.exists() {
    return Err(anyhow!("File not found: {}", path));
}

// Expected condition - returns Ok with error message
if matches.is_empty() {
    return Ok(ToolOutput {
        content: "No matches found".to_string(),
        metadata: Some(json!({ "count": 0 })),
    });
}
```

## Testing

All tool output should be tested for:

1. **Content format compliance** - Follows Pattern A, B, or C
2. **Metadata completeness** - Contains all required fields
3. **Edge cases** - Empty results, single items, large results
4. **Pluralization** - Correct singular/plural forms
5. **Truncation** - Proper markers when content is truncated

See the test files for examples:
- `crates/ragent-core/tests/test_tool_output_format.rs`
- `crates/ragent-core/tests/test_tool_metadata.rs`
- `crates/ragent-core/tests/test_tool_truncate.rs`
- `crates/ragent-core/tests/test_visual_regression.rs`
- `crates/ragent-tui/tests/test_tool_display.rs`

## Migration Checklist

When adding a new tool or updating an existing tool:

- [ ] Choose appropriate content pattern (A, B, or C)
- [ ] Include all relevant metadata fields using `MetadataBuilder`
- [ ] Use `format_*` helpers for consistent formatting
- [ ] Add `tool_input_summary` case in message_widget.rs
- [ ] Add `tool_result_summary` case in message_widget.rs
- [ ] Add tests for content format and metadata
- [ ] Verify TUI display renders correctly
- [ ] Document any tool-specific fields in metadata schema
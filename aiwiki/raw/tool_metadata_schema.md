# Tool Metadata Schema Documentation

This document defines the formal JSON schema for tool metadata in ragent, standardizing the field names, types, and usage patterns across all tools.

## Overview

Tool metadata provides structured information about tool execution results, separate from the human-readable `content` field. This enables:
- Consistent TUI display formatting
- Programmatic access to execution results
- Better filtering and analysis of tool outputs

## Metadata Structure

```json
{
  "type": "object",
  "description": "Tool execution metadata",
  "properties": {
    // Standard fields defined below
  }
}
```

## Standard Field Definitions

### File/Path Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `path` | string | Absolute or relative path to primary file | `"src/main.rs"` |
| `source` | string | Source path for copy/move operations | `"src/old.rs"` |
| `destination` | string | Destination path for copy/move operations | `"src/new.rs"` |

### Count Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `count` | integer | Generic count of items | `42` |
| `lines` | integer | Number of lines processed/written | `150` |
| `line_count` | integer | Number of lines (legacy, prefer `lines`) | `150` |
| `total_lines` | integer | Total lines in a file | `500` |
| `entries` | integer | Directory entries | `12` |
| `matches` | integer | Pattern matches found | `5` |
| `results` | integer | Search results returned | `10` |
| `files` | integer | Number of files affected | `3` |
| `edits` | integer | Number of edits applied | `5` |
| `hunks` | integer | Number of diff hunks applied | `2` |
| `pages` | integer | Number of PDF/Office pages | `5` |
| `sheets` | integer | Number of spreadsheet sheets | `3` |
| `slides` | integer | Number of presentation slides | `10` |
| `symbols` | integer | Number of code symbols found | `25` |
| `symbol_count` | integer | Legacy symbol count (prefer `symbols`) | `25` |

### Line Position Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `start_line` | integer | Starting line number (1-based) | `1` |
| `end_line` | integer | Ending line number (1-based, inclusive) | `100` |
| `line` | integer | Single line number | `42` |
| `column` | integer | Column position (1-based) | `15` |

### Size Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `bytes` | integer | Size in bytes | `1024` |
| `size_bytes` | integer | File size in bytes | `2048` |

### Status Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `exit_code` | integer | Process exit code | `0` |
| `status` | integer/string | HTTP status or operation status | `200` or `"ok"` |
| `success` | boolean | Whether operation succeeded | `true` |
| `timed_out` | boolean | Whether operation timed out | `false` |
| `deleted` | boolean | Whether file was deleted | `true` |
| `cancelled` | boolean | Whether task was cancelled | `true` |
| `claimed` | boolean | Whether task was claimed | `true` |
| `approved` | boolean | Whether plan was approved | `true` |
| `exists` | boolean | Whether file/resource exists | `true` |

### Content Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `truncated` | boolean | Whether content was truncated | `true` |
| `summarised` | boolean | Whether content is a summary | `true` |
| `message` | string | Additional informational message | `"File is large..."` |

### Time Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `duration_ms` | integer | Duration in milliseconds | `150` |
| `modified` | string (ISO 8601) | Last modified timestamp | `"2024-01-15T10:30:00Z"` |

### Diff/Change Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `old_lines` | integer | Lines before change | `10` |
| `new_lines` | integer | Lines after change | `15` |
| `lines_added` | integer | Lines added | `5` |
| `lines_removed` | integer | Lines removed | `3` |

### Collection Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `references` | array | Reference locations | `[{"path": "...", "line": 5}]` |
| `file_stats` | array | Per-file statistics | `[{"path": "...", "edits": 2}]` |
| `diagnostics` | array | Compiler diagnostics | `[{"severity": "error", ...}]` |

### Task/Agent Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `task_id` | string | Unique task identifier | `"task-001"` |
| `agent` | string | Agent type/name | `"explore"` |
| `agent_id` | string | Agent instance ID | `"tm-001"` |
| `background` | boolean | Whether task runs in background | `true` |
| `team_name` | string | Team identifier | `"my-team"` |
| `teammate` | string | Teammate name | `"security-reviewer"` |

### Result/Value Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `result` | any | Generic result value | `42` or `"ok"` |
| `value` | any | Configuration value | `"DEBUG"` |
| `key` | string | Key name | `"API_KEY"` |
| `action` | string | Action performed | `"add"` |

### GitHub Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `number` | integer | Issue/PR number | `42` |
| `url` | string | GitHub URL | `"https://github.com/..."` |
| `state` | string | Issue/PR state | `"open"` |

### Office/PDF Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `author` | string | Document author | `"John Doe"` |
| `title` | string | Document title | `"Report"` |
| `file_type` | string | File type | `"file"` or `"directory"` |

## Content Pattern Types

Tools should follow one of these standardized content patterns:

### Pattern A: Summary + Content

For tools that return raw content that the agent needs to see:

```json
{
  "content": "<actual content with line numbers or raw output>",
  "metadata": {
    "lines": 150,
    "start_line": 1,
    "end_line": 150
  }
}
```

**Tools using Pattern A:**
- `read` - File content with line numbers
- `list` - Directory tree
- `grep` - Matching lines with header
- `glob` - File paths
- `search` - Search results
- `webfetch` - Fetched content
- `websearch` - Search results
- `lsp_*` - Code intelligence results
- `team_memory_read` - Memory content
- `memory_read` - Memory content

### Pattern B: Summary Only

For tools that perform an action and only need to report success/failure:

```json
{
  "content": "Wrote 150 lines to src/main.rs",
  "metadata": {
    "path": "src/main.rs",
    "lines": 150,
    "bytes": 4096
  }
}
```

**Tools using Pattern B:**
- `write`, `create` - File write confirmation
- `edit` - Edit summary
- `multiedit` - Multi-file edit summary
- `patch` - Patch application summary
- `append_to_file` - Append confirmation
- `rm` - Deletion confirmation
- `copy_file` - Copy confirmation
- `move_file` - Move confirmation
- `mkdir` - Directory creation
- `bash` - Command execution summary
- `execute_python` - Script execution
- `calculator` - Calculation result
- `new_task` - Task spawn confirmation
- `cancel_task` - Cancellation result
- `wait_tasks` - Wait completion
- `team_*` - Team operation results
- `todo_*` - TODO management
- `github_*` - GitHub operation results

### Pattern C: Structured Data

For tools that return structured information:

```json
{
  "content": "Exit code: 0\nDuration: 150ms\n\n<output>",
  "metadata": {
    "exit_code": 0,
    "duration_ms": 150,
    "stdout_lines": 42
  }
}
```

**Tools using Pattern C:**
- `bash` - Exit code, duration, output
- `execute_python` - Exit code, duration, output
- `file_info` - File metadata
- `office_info` - Document metadata
- `team_status` - Complex team status

## Deprecated Field Names

The following field names are deprecated and should be migrated:

| Deprecated | Preferred | Reason |
|------------|-----------|--------|
| `line_count` | `lines` | Consistency |
| `symbol_count` | `symbols` | Consistency |
| `files_searched` | `files` | Simplicity |
| `stdout_lines` | `lines` | Genericity |

## Migration Guidelines

### When Creating New Tools

1. Choose the appropriate pattern (A, B, or C)
2. Use standard field names from this document
3. Include all relevant metadata for TUI display
4. Always provide relative paths in content, absolute in metadata

### When Migrating Existing Tools

1. Audit current metadata fields
2. Map to standard field names
3. Update TUI display functions to use standard names
4. Add backward compatibility if needed
5. Update tests

### TUI Display Integration

The TUI layer uses these standard metadata fields:

- `lines` / `line_count` - For showing "X lines read/written"
- `path` - For showing file paths in summaries
- `count` / `entries` / `matches` - For showing result counts
- `exit_code` / `timed_out` - For showing command status
- `old_lines` / `new_lines` - For showing edit diffs

## Examples by Tool Category

### File Operations

```json
// read
{
  "content": "1 | fn main() {\n2 |     println!(\"Hello\");\n...",
  "metadata": {
    "path": "src/main.rs",
    "start_line": 1,
    "end_line": 50,
    "total_lines": 200,
    "lines": 50,
    "summarised": false
  }
}

// write
{
  "content": "Wrote 1,024 bytes (50 lines) to src/main.rs",
  "metadata": {
    "path": "src/main.rs",
    "bytes": 1024,
    "lines": 50
  }
}

// edit
{
  "content": "Edited src/main.rs: replaced 10 line(s) with 15 line(s)",
  "metadata": {
    "path": "src/main.rs",
    "old_lines": 10,
    "new_lines": 15,
    "lines": 15
  }
}
```

### Search Tools

```json
// grep
{
  "content": "src/main.rs:10: fn main() {\nsrc/lib.rs:5: pub fn helper() {",
  "metadata": {
    "matches": 2,
    "files": 2,
    "truncated": false
  }
}

// glob
{
  "content": "src/main.rs\nsrc/lib.rs\n...",
  "metadata": {
    "count": 5,
    "pattern": "**/*.rs"
  }
}
```

### Execution Tools

```json
// bash
{
  "content": "Exit code: 0\nDuration: 150ms\n\nHello, World!",
  "metadata": {
    "exit_code": 0,
    "duration_ms": 150,
    "lines": 1,
    "timed_out": false
  }
}
```

## Validation

Metadata should be validated to ensure:

1. All required fields for the pattern are present
2. Field types match the schema
3. No deprecated field names are used
4. Values are reasonable (e.g., positive counts)

## Future Extensions

Potential future metadata fields:

- `checksum` - Content hash for verification
- `encoding` - Text encoding (UTF-8, etc.)
- `mime_type` - MIME type of content
- `created` - Creation timestamp
- `permissions` - File permissions

## References

- Tool implementation: `crates/ragent-core/src/tool/`
- TUI display: `crates/ragent-tui/src/widgets/message_widget.rs`
- Core types: `crates/ragent-core/src/tool/mod.rs`

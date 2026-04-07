# Tool Output Presentation Consistency Plan

## Executive Summary

This document outlines a comprehensive plan to standardize and improve tool output presentation in the ragent TUI message window. The current implementation has inconsistencies in content formatting, metadata structure, TUI display rendering, visual indicators, and error presentation that lead to a fragmented user experience.

## Current State Analysis

### Key Files Involved

1. **Tool Output Generation** (`crates/ragent-core/src/tool/*.rs`)
   - Each tool implements `Tool::execute()` returning `ToolOutput { content, metadata }`
   - 50+ tools with varying output formats

2. **TUI Display Layer** (`crates/ragent-tui/src/widgets/message_widget.rs`)
   - `tool_input_summary()` - Formats tool call summaries (lines 131-356)
   - `tool_result_summary()` - Formats tool result summaries (lines 412-829)
   - `tool_inline_diff()` - Calculates diff stats for edit tools

3. **Core Types** (`crates/ragent-core/src/tool/mod.rs`)
   - `ToolOutput` struct definition (lines 171-179)

### Identified Inconsistencies

#### 1. Content Format Inconsistencies

| Tool | Content Pattern | Structure |
|------|----------------|-----------|
| `bash` | "Exit code: X\nDuration: Xms\n\n{output}" | Structured header + raw output |
| `read` | Raw file content | No header, just content |
| `write` | "Wrote X bytes (Y lines) to {path}" | Summary only |
| `grep` | "{summary}\n\n{matching lines}" | Summary + content |
| `list` | Tree-formatted directory | Visual tree structure |
| `edit` | "Edited {path}: replaced X lines with Y lines" | Summary only |
| `multiedit` | "Applied X edits across Y files" | Summary only |
| `todo_write` | Action result string | Varies by action |

#### 2. Metadata Structure Inconsistencies

**Line Count Fields:**
- `lines` - Used by: bash, read, write, edit
- `line_count` - Used by: lsp_symbols
- Missing entirely from: list, todo, question

**Path Fields:**
- `path` - Used by: write, edit, read, lsp_tools
- Missing from: bash, grep, glob

**Count Fields:**
- `count`, `entries`, `matches`, `files` - Different names for similar concepts
- Inconsistent data types (u64 vs usize)

**Status Fields:**
- `exit_code` - bash, execute_python
- `status` - webfetch, http_request
- `timed_out` - bash

#### 3. TUI Display Layer Inconsistencies

In `tool_result_summary()` (message_widget.rs:412-829):

**Summary Line Presence:**
- Returns `Some(...)` for most tools
- Returns `None` for `edit` tool (line 468) - inconsistency!

**Formatting Patterns:**
- "X lines read" (read)
- "X written to {path}" (write)
- "X created in {path}" (create)
- "X edits across Y files" (multiedit)
- "X matched in Y files searched" (grep)
- "X found" (glob, websearch)

**Icon/Visual Indicators in Input Summary:**
- 📋 - todo tools
- 💭 - think tool
- ❓ - question tool
- →/← - plan tools
- Others: No emoji

**Pluralization:**
- Manual inline: `"{} match{}"` (grep)
- Uses `pluralize()` helper: `"{} {}"` (read, write)
- Inconsistent application

#### 4. Path Display Inconsistencies

**Relative Path Handling:**
- `make_relative_path()` used in: input summaries, some result summaries
- Absolute paths shown in: bash error messages, some metadata
- Inconsistent in: file_info, diff_files

#### 5. Error Presentation Inconsistencies

**Error Message Format:**
- `Err(anyhow!(...))` - Returns error, displayed as "✗ Error: {msg}"
- `Ok(ToolOutput { content: "Error: ...", ... })` - Returns success with error in content
- Inconsistent between tools

## Proposed Standardization

### Content Format Standard

All tools should follow ONE of these patterns:

**Pattern A - Summary + Content** (for tools returning data):
```
{summary_line}

{content}
```
Example: grep, read, glob

**Pattern B - Summary Only** (for action tools):
```
{action_summary}
```
Example: write, edit, move_file

**Pattern C - Structured Output** (for execution tools):
```
{status_header}

{output}
```
Example: bash, execute_python

### Metadata Standard

Standardize on these field names:

```rust
// Standard metadata fields
{
    // Count fields - always use these exact names
    "count": u64,           // Generic count (searches, tasks, etc.)
    "line_count": u64,      // Lines of content
    "byte_count": u64,      // Bytes transferred/written
    "file_count": u64,      // Number of files affected
    
    // Status fields
    "status": String,       // "success", "error", "timeout", "cancelled"
    "exit_code": i64,       // For shell/exec tools
    "http_status": u64,     // For HTTP tools
    "timed_out": bool,      // For timeout-capable tools
    "truncated": bool,      // For truncated results
    
    // Path fields - always relative to working_dir
    "path": String,         // Primary path
    "source_path": String,  // For copy/move
    "target_path": String,  // For copy/move
    
    // Timing fields
    "duration_ms": u64,     // Execution time
    
    // Display fields
    "summarized": bool,     // For large file handling
    "total_lines": u64,     // Total before summarization
    "start_line": u64,      // For range reads
    "end_line": u64,        // For range reads
}
```

### TUI Display Standard

**Input Summary Line:**
- Format: `{tool_name} {args_summary}`
- Show relevant parameters only
- Use `make_relative_path()` for all paths
- Consistent truncation (50-60 chars)

**Result Summary Line:**
- Always return `Some(...)` for completed tools (fix `edit`)
- Format: `{action} {quantity} {unit} [{details}]`
- Always show path for file-affecting tools
- Use `pluralize()` helper consistently

**Diff Stats Display:**
- Format: `(+{added} -{removed})`
- Green for added, red for removed
- Only when both values > 0

## Implementation Plan

### Milestone 1: Audit and Document (Week 1)

**Status: ✅ COMPLETED**

**Task 1.1: Create Tool Output Audit Spreadsheet**
- [x] List all 50+ tools with current content format
- [x] Document current metadata fields per tool
- [x] Note inconsistencies and categorize severity
- [x] Output: `docs/reports/tool_output_audit.xlsx`

**Task 1.2: Define Metadata Schema Documentation**
- [x] Create formal JSON schema for tool metadata
- [x] Document each standard field with examples
- [x] Output: `docs/standards/tool_metadata_schema.md`

**Task 1.3: Review TUI Display Functions**
- [x] Document all tool cases in `tool_input_summary()`
- [x] Document all tool cases in `tool_result_summary()`
- [x] Identify missing or incomplete cases
- [x] Output: Detailed findings below

---

### Milestone 1 Detailed Findings

#### Tool Coverage Analysis

**Total Tools Audited:** 80+ tools (including aliases)

**Tools by Category:**

| Category | Count | Pattern | Status |
|----------|-------|---------|--------|
| File Operations | 12 | Pattern B | Needs migration |
| File Query | 8 | Pattern A | Needs migration |
| Search | 3 | Pattern A | Needs migration |
| Execution | 4 | Pattern C | Mostly OK |
| Network | 3 | Pattern A/C | Needs migration |
| Coordination | 20+ | Variable | Needs standardization |
| User Interaction | 5 | Pattern B | Needs migration |
| LSP/IDE | 5 | Pattern A | Needs migration |
| Office/PDF | 6 | Pattern A/B | Needs migration |
| GitHub | 10 | Pattern B | Needs migration |
| System/Memory | 4 | Pattern A/B | Needs migration |

#### Metadata Field Standardization Issues

**Critical Inconsistencies Found:**

1. **Line Count Field Names:**
   - `lines` (used by: read, write, edit, bash) ✓ Preferred
   - `line_count` (used by: lsp_symbols) ⚠️ Deprecated
   - Missing from: move_file, mkdir, bash_reset

2. **Count Field Names:**
   - `count` (glob, list_tasks, wait_tasks)
   - `entries` (list) ⚠️ Should use `count`
   - `matches` (grep) ⚠️ Should use `count`
   - `results` (websearch) ⚠️ Should use `count`
   - `symbol_count` (lsp_symbols) ⚠️ Should use `symbols`
   - `files_searched` (grep) ⚠️ Should use `files`

3. **Path Field Presence:**
   - Present: write, create, edit, read, lsp_tools
   - Missing: bash, grep, glob, move_file, mkdir

4. **Status Fields:**
   - `exit_code` (bash, execute_python) ✓ Preferred
   - `status` (webfetch, http_request) ⚠️ HTTP-specific OK
   - `success` (wait_tasks) ✓ Boolean result
   - `timed_out` (bash) ✓ Boolean
   - `deleted` (rm) ✓ Boolean
   - `cancelled` (cancel_task) ✓ Boolean

#### TUI Display Issues Found

**In `tool_input_summary()` (lines 131-356):**

| Tool | Has Summary | Icon | Notes |
|------|-------------|------|-------|
| bash | ✅ | No | Shows first command line |
| read | ✅ | No | Shows relative path |
| list | ✅ | No | Shows relative path |
| search | ✅ | No | Shows query + path |
| write/create/edit | ✅ | No | Shows relative path |
| move_file | ✅ | No | Shows src → dst |
| copy_file | ✅ | No | Shows src → dst |
| append_to_file | ✅ | No | Shows relative path |
| mkdir | ✅ | No | Shows relative path |
| file_info | ✅ | No | Shows relative path |
| diff_files | ✅ | No | Shows file_a ↔ file_b |
| execute_python | ✅ | No | Shows first code line |
| str_replace_editor | ✅ | No | Shows command: path |
| calculator | ✅ | No | Shows expression |
| get_env | ✅ | No | Shows key or "all vars" |
| http_request | ✅ | No | Shows METHOD URL |
| webfetch | ✅ | No | Shows URL |
| websearch | ✅ | No | Shows "query" |
| question | ✅ | ✅ ❓ | Has emoji |
| think | ✅ | ✅ 💭 | Has emoji |
| multiedit | ✅ | No | Shows count |
| glob | ✅ | No | Shows pattern |
| grep | ✅ | No | Shows pattern + path |
| plan_enter | ✅ | ✅ → | Has arrow |
| plan_exit | ✅ | ✅ ← | Has arrow |
| todo_read | ✅ | ✅ 📋 | Has emoji |
| todo_write | ✅ | ✅ 📋 | Has emoji |
| new_task | ✅ | No | Shows agent → task |
| cancel_task | ✅ | No | Shows task_id |
| list_tasks | ✅ | No | Shows filter |
| wait_tasks | ✅ | No | Shows count |
| lsp_* | ✅ | No | Shows line/col or path |
| team_* | ✅ | No | Generic summary |
| _ | ✅ | No | Generic fallback |

**Key Findings:**
- ✅ All tools have input summaries
- ✅ Emoji icons used for: question, think, plan, todo (4 categories)
- ⚠️ Most tools lack visual distinction
- ✅ Path handling is consistent (uses `make_relative_path`)

**In `tool_result_summary()` (lines 412-829):**

| Tool | Has Summary | Diff Stats | Notes |
|------|-------------|------------|-------|
| read | ✅ | N/A | Shows "X lines read" |
| write | ✅ | N/A | Shows "X lines written to path" |
| create | ✅ | N/A | Shows "X lines created in path" |
| edit | ❌ | ✅ | Returns `None` - **BUG** |
| multiedit | ✅ | ✅ | Shows "X edits across Y files" |
| patch | ✅ | ✅ | Shows "X hunks across Y files" |
| bash | ✅ | N/A | Shows exit code, timeout status |
| grep | ✅ | N/A | Shows "X matched in Y files" |
| glob | ✅ | N/A | Shows "X files found" |
| list | ✅ | N/A | Shows "X entries in path" |
| webfetch | ✅ | N/A | Shows "X lines (HTTP status)" |
| websearch | ✅ | N/A | Shows "X results found" |
| plan_enter | ✅ | N/A | Shows "delegated → plan" |
| plan_exit | ✅ | N/A | Shows "returned (X chars)" |
| question | ✅ | N/A | Shows "↩ response" |
| think | ✅ | N/A | Shows "Thinking ..." |
| todo_read | ✅ | N/A | Shows "X items" |
| todo_write | ✅ | N/A | Shows "action → X remaining" |
| office_read/pdf_read | ✅ | N/A | Shows "X lines read" |
| office_write/pdf_write | ✅ | N/A | Shows "X lines written to path" |
| office_info | ✅ | N/A | Shows "X lines of metadata" |
| rm | ✅ | N/A | Shows "deleted" or "failed" |
| new_task | ✅ | N/A | Shows spawn/complete status |
| cancel_task | ✅ | N/A | Shows "cancelled" or "completed" |
| list_tasks | ✅ | N/A | Shows "X tasks" |
| wait_tasks | ✅ | N/A | Shows "waited on X" or "timeout" |
| lsp_definition | ✅ | N/A | Shows "X locations found" |
| lsp_references | ✅ | N/A | Shows "X locations found" |
| lsp_symbols | ✅ | N/A | Shows "X symbols in path" |
| lsp_hover | ✅ | N/A | Shows "X lines of info" |
| lsp_diagnostics | ✅ | N/A | Shows "X diagnostics found" |
| team_task_claim | ✅ | N/A | Shows claim status |
| team_task_complete | ✅ | N/A | Shows completion status |
| team_idle | ✅ | N/A | Shows idle/blocked status |
| search | ✅ | N/A | Shows "X found" |
| move_file | ✅ | N/A | Shows "moved" |
| copy_file | ✅ | N/A | Shows "X bytes copied" |
| append_to_file | ✅ | N/A | Shows "X bytes appended" |
| mkdir | ✅ | N/A | Shows "directory created" |
| file_info | ✅ | N/A | Shows "kind (size)" |
| diff_files | ✅ | N/A | Shows "X changes found" |
| execute_python | ✅ | N/A | Shows exit code, lines |
| str_replace_editor | ✅ | N/A | Shows "X lines read" etc |
| calculator | ✅ | N/A | Shows "= result" |
| get_env | ✅ | N/A | Shows "X vars" or "not found" |
| http_request | ✅ | N/A | Shows "HTTP X (Y lines)" |
| _ | ❌ | N/A | Returns `None` for unknown tools |

**Critical Issues Found:**

1. **BUG: `edit` tool returns `None`** (line 468)
   - This is an inconsistency - all other tools return `Some(...)`
   - The edit tool provides metadata for diff stats but no summary
   - **Fix:** Add summary like "Edited path: +X/-Y lines"

2. **Inconsistent pluralization:**
   - Some tools use `pluralize()` helper
   - Others manually format: `"{} match{}"` (grep, line 517)
   - **Fix:** Standardize on `pluralize()` helper

3. **Missing diff stats support:**
   - `edit` tool has metadata for diff but TUI reads from `tool_inline_diff()`
   - `str_replace_editor` has partial support
   - **Fix:** Ensure all edit tools populate `old_lines`/`new_lines` or `lines_added`/`lines_removed`

4. **Inconsistent path display:**
   - Most tools use `make_relative_path()` ✓
   - Some tools show absolute paths in error messages
   - **Fix:** Audit all paths to use relative form in content

#### Content Pattern Distribution

| Pattern | Description | Tools | Status |
|---------|-------------|-------|--------|
| A | Summary + Content | read, list, grep, glob, search, webfetch, websearch, lsp_*, office_read, pdf_read, team_memory_read, memory_read, diff_files, str_replace_editor | Needs standardization |
| B | Summary Only | write, create, edit, multiedit, patch, move_file, copy_file, rm, mkdir, append_to_file, new_task, cancel_task, wait_tasks, team_*, todo_*, github_*, question, think, calculator, file_info, office_write, pdf_write, memory_write | Needs standardization |
| C | Structured | bash, execute_python, http_request, office_info, team_status | OK |

#### Recommendations for Next Milestones

**Priority 1 (Milestone 2):**
1. Fix `edit` tool returning `None` in TUI
2. Create MetadataBuilder utility
3. Create Content Format utilities
4. Add truncation helper

**Priority 2 (Milestone 3 - File Tools):**
1. Standardize read/write/create/edit metadata
2. Add missing metadata to move_file, mkdir
3. Unify count field names

**Priority 3 (Milestone 4 - Search Tools):**
1. Migrate grep: `matches` → `count`, `files_searched` → `files`
2. Migrate glob: consistent metadata
3. Migrate search: add metadata

**Priority 4 (Milestone 9 - TUI Cleanup):**
1. Standardize all pluralization to use helper
2. Add emoji icons for all tool categories
3. Unify path display
4. Document all TUI display conventions

---

### Milestone 2: Core Infrastructure (Week 2)

**Task 2.1: Create Metadata Builder Utility**
- [ ] Create `crates/ragent-core/src/tool/metadata.rs`
- [ ] Implement `MetadataBuilder` with fluent API
- [ ] Add validation for standard field names
- [ ] Add unit tests for builder
- [ ] Output: New module with full test coverage

**Task 2.2: Create Content Format Utilities**
- [ ] Create `crates/ragent-core/src/tool/format.rs`
- [ ] Implement format helpers:
  - `format_summary_content(summary, content)`
  - `format_status_output(exit_code, stdout, stderr)`
  - `format_bytes(bytes)` - human-readable sizes
- [ ] Output: Format utility module

**Task 2.3: Add Content Truncation Helper**
- [ ] Implement `truncate_content(content, max_lines)`
- [ ] Ensure truncation adds "... (X lines omitted) ..." marker
- [ ] Use in all content-returning tools
- [ ] Output: Truncation utility with tests

### Milestone 3: Tool Migration Batch 1 - File Tools (Week 3)

**Task 3.1: Standardize Read Tool**
- [x] Update metadata to use standard field names
- [x] Keep content format (raw file content is correct)
- [x] Ensure `line_count`, `summarized`, `total_lines` in metadata
- [x] Update TUI display for consistency
- [x] Test: Verify TUI displays correctly

**Task 3.2: Standardize Write Tool**
- [x] Update content format to standard summary pattern
- [x] Update metadata: `byte_count`, `line_count`, `path`
- [x] Ensure TUI shows path
- [x] Test: Verify path display in TUI

**Task 3.3: Standardize Edit Tool**
- [x] Update content format to standard summary
- [x] Add `old_lines`, `new_lines` to metadata (keep for backward compat)
- [x] Fix TUI to return `Some(...)` instead of `None`
- [x] Test: Verify result line appears in TUI

**Task 3.4: Standardize Multiedit Tool**
- [x] Review current format (already good)
- [x] Ensure metadata uses `file_count` instead of `files`
- [x] Verify `file_stats` array format
- [x] Test: Verify tabular display in TUI

**Task 3.5: Standardize Create Tool**
- [x] Align with Write tool format
- [x] Update metadata to standard fields
- [x] Test: Verify consistency with write

### Milestone 4: Tool Migration Batch 2 - Search/Query Tools (Week 4) ✅ COMPLETED

**Task 4.1: Standardize Grep Tool** ✅
- [x] Update metadata: `count` (matches), `file_count`, `truncated`
- [x] Keep content format (summary + matches)
- [x] Update TUI to use `count` instead of `matches`
- [x] Test: Verify result counting

**Task 4.2: Standardize Glob Tool** ✅
- [x] Update metadata: `count`, `pattern` (if applicable)
- [x] Keep content format
- [x] Update TUI display
- [x] Test: Verify file count accuracy

**Task 4.3: Standardize List Tool** ✅
- [x] Update metadata: `count` (entries), `path`
- [x] Keep tree content format
- [x] Update TUI to use standard field names
- [x] Test: Verify entry counting

**Task 4.4: Standardize Search Tool** ✅
- [x] Align with Grep tool where applicable
- [x] Update metadata to standard fields
- [x] Test: Verify consistency

  ### Milestone 5: Tool Migration Batch 3 - Execution Tools (Week 5) ✅ COMPLETED

**Task 5.1: Standardize Bash Tool**
- [x] Update metadata to use `line_count` instead of `lines`
- [x] Keep structured content format
- [x] Ensure `timed_out`, `exit_code`, `duration_ms` in metadata
- [x] Update TUI to use standard field names
- [x] Test: Verify timeout and exit code display

**Task 5.2: Standardize Execute Python Tool**
- [x] Align with Bash tool format
- [x] Update metadata to standard fields
- [x] Test: Verify consistency

**Task 5.3: Standardize Calculator Tool**
- [x] Update metadata: `result`, `expression`
- [x] Keep simple content format
- [x] Test: Verify result display
### Milestone 6: Tool Migration Batch 4 - External/Network Tools (Week 6) ✅ COMPLETED

**Task 6.1: Standardize Webfetch Tool** ✅
- [x] Update metadata: `http_status`, `line_count`, `byte_count`
- [x] Update TUI to use `http_status` instead of `status`
- [x] Test: Verified HTTP status display

**Task 6.2: Standardize Websearch Tool** ✅
- [x] Update metadata: `count` (results), `line_count`
- [x] Keep content format
- [x] Update TUI to use standard field names (`count` instead of `results`)
- [x] Test: Verified result counting

**Task 6.3: Standardize Http Request Tool** ✅
- [x] Align with Webfetch where applicable
- [x] Update metadata to standard fields: `http_status`, `line_count`, `byte_count`
- [x] Test: Verified consistency

### Milestone 7: Tool Migration Batch 5 - Office/PDF Tools (Week 7) ✅ COMPLETED

**Task 7.1: Standardize Office Read Tool** ✅
- [x] Update metadata: `line_count`, `path`, `format`
- [x] Update TUI display
- [x] Test: Verify line counting

**Task 7.2: Standardize Office Write Tool** ✅
- [x] Align with standard write pattern
- [x] Update metadata to standard fields (renamed `bytes` → `byte_count`, added `line_count`)
- [x] Test: Verify consistency

**Task 7.3: Standardize Office Info Tool** ✅
- [x] Add `line_count` to metadata for all formats (docx, xlsx, pptx)
- [x] Keep existing format-specific fields
- [x] Test: Verify metadata consistency

**Task 7.4: Standardize PDF Read Tool** ✅
- [x] Update metadata to standard fields (added `line_count`)
- [x] Update TUI display
- [x] Test: Verify consistency

**Task 7.5: Standardize PDF Write Tool** ✅
- [x] Align with standard write pattern
- [x] Update metadata to standard fields (renamed `bytes` → `byte_count`, added `line_count`)
- [x] Test: Verify consistency

**Changes Made:**
- `office_read.rs`: Added `line_count` to metadata by counting lines in truncated content
- `office_write.rs`: Renamed `bytes` → `byte_count`, added `line_count` estimation function
- `office_info.rs`: Added `line_count` to metadata for all three formats (docx, xlsx, pptx)
- `pdf_read.rs`: Added `line_count` to metadata by counting lines in truncated content
- `pdf_write.rs`: Renamed `bytes` → `byte_count`, added `line_count` (page count estimation)

  ### Milestone 8: Tool Migration Batch 6 - Team/Coordination Tools (Week 8) ✅ COMPLETED

  **Task 8.1: Standardize Team Tools** ✅
  - Reviewed and standardized: team_create, team_spawn, team_message, team_broadcast, team_status, team_wait, team_cleanup, team_idle, team_task_create, team_task_list, team_task_claim, team_task_complete, team_assign_task, team_approve_plan, team_submit_plan, team_shutdown_ack, team_shutdown_teammate
  - Updated metadata to use standard fields: `team_name`, `task_id`, `status`, `count`, `line_count`, `byte_count`
  - Added `message_count` to communication tools
  - Kept team-specific fields as extensions (e.g., `members_spawned`, `agent_id`, `hook_rejected`)
  - Standardized metadata field names: `task_count`, `member_count`, `running_count`
  
  **Task 8.2: Standardize Task Management Tools** ✅
  - Reviewed: new_task, cancel_task, list_tasks, wait_tasks
  - Updated metadata to use standard fields: `task_count`, `task_id`, `status`, `count`
  - Standardized status values: `"cancelled"`, `"cancel_failed"`, `"running"`, `"completed"`, `"timed_out"`
  - Updated task result displays with consistent formatting
  
  **Task 8.3: Standardize Memory Tools** ✅
  - Reviewed: memory_read, memory_write, team_memory_read, team_memory_write
  - Updated metadata to use standard fields: `file`, `scope`, `byte_count`, `line_count`, `path`
  - Added `line_count` to all memory tool outputs
  - Standardized field names: `bytes_written` → `byte_count`, `bytes` → `byte_count`
  - Updated content format to include both file path and actual content
  
  **Files Modified:**
  - `team_create.rs`: Updated metadata to use MetadataBuilder with standard fields
  - `team_spawn.rs`: Updated metadata to use MetadataBuilder with standard fields
  - `team_message.rs`: Added `message_count` to metadata
  - `team_broadcast.rs`: Changed `sent_count` → `message_count`
  - `team_status.rs`: Added `member_count` to metadata, standardized field names
  - `team_wait.rs`: Added `idle_count`, `still_working_count`, standardized team_name field
  - `team_cleanup.rs`: Added `member_count`, removed `active_at_cleanup` array
  - `team_task_create.rs`: Changed `depends_on` → `depends_on_count`
  - `team_task_list.rs`: Changed `total` → `task_count`
  - `team_idle.rs`: Metadata already standardized
  - `cancel_task.rs`: Changed `cancelled` → `status`, added proper status values
  - `list_tasks.rs`: Changed `count` → `task_count`, `running` → `running_count`
  - `wait_tasks.rs`: Changed `completed` → `completed_count`, `still_running` → `still_running_count`
  - `memory_write.rs`: Major restructure, added `line_count`, standardized field names, updated content format
  - `team_memory_read.rs`: Added `line_count`, changed `bytes` → `byte_count`
  - `team_memory_write.rs`: Added `line_count`, changed `bytes_written` → `byte_count`
### Milestone 9: TUI Display Layer Cleanup (Week 9) ✅ COMPLETED

**Task 9.1: Refactor tool_input_summary()**
- [x] Group tools by category (file, search, exec, etc.)
- [x] Add consistent emoji/icons per category
- [x] Ensure all tools use `make_relative_path()`
- [x] Standardize truncation to 50 chars
- [x] Add missing tool cases (bash_reset, github_issues, github_prs, format, metadata, truncate, memory_read, memory_write, libreoffice_*)
- [x] Output: Refactored function with better organization

**Task 9.2: Refactor tool_result_summary()**
- [x] The `edit` tool already returns `Some(...)` (was fixed previously)
- [x] Standardize on `pluralize()` helper
- [x] Ensure all file-affecting tools show paths
- [x] Group related tools (e.g., file ops, search ops)
- [x] Add consistent emoji formatting patterns matching input summaries
- [x] Output: Refactored function with organized categories

**Task 9.3: Standardize Diff Stats Display**
- [x] Review `tool_inline_diff()` function - already consistent
- [x] Ensure all edit tools provide necessary metadata - confirmed
- [x] Standardize color scheme (green +, red -) - already in place
- [x] Output: Consistent diff display maintained

**Task 9.4: Add Visual Consistency**
- [x] Define icon/emoji scheme per tool category:
  - 📄 File Operations: read, write, create, edit, patch, rm, multiedit
  - 📁 Directory Operations: list, make_directory
  - ℹ️ File Info: file_info
  - 🔍 Search Operations: search, grep, glob
  - ⚡ Execution: bash, execute_python, str_replace_editor, calculator
  - 🌐 Network: webfetch, websearch, http_request
  - 🔧 Environment: get_env, bash_reset
  - ❓ User Interaction: question
  - 💭 Reasoning: think
  - 📝 Planning: plan_enter, plan_exit
  - 📋 Task Management: todo_read, todo_write
  - 🤖 Sub-agent: new_task, cancel_task, list_tasks, wait_tasks
  - 👥 Team Coordination: team_*
  - 🔎 LSP/Code Intelligence: lsp_*
  - 📄 Document: office_*, pdf_*
  - 📋 GitHub: github_issues, github_prs
  - ✨ Utility: format, metadata, truncate, memory_*
- [x] Document in style guide (code comments)
- [x] Apply consistently across input summaries
- [x] Apply consistently across result summaries
- [x] Output: Visual style documentation in code

### Milestone 10: Testing and Documentation (Week 10)

**Task 10.1: Create Comprehensive Test Suite**
- [ ] Unit tests for each tool's output format
- [ ] Integration tests for TUI display
- [ ] Snapshot tests for message rendering
- [ ] Test coverage for all tool categories
- [ ] Output: Test suite with >90% coverage

**Task 10.2: Create Visual Regression Tests**
- [ ] Capture expected TUI output for each tool
- [ ] Automate comparison
- [ ] Document expected formats
- [ ] Output: Visual regression test suite

**Task 10.3: Update Documentation**
- [ ] Update `docs/standards/tool_output.md` with standards
- [ ] Create tool output style guide
- [ ] Document migration guide for new tools
- [ ] Update API documentation
- [ ] Output: Complete documentation

**Task 10.4: Create Migration Guide for Future Tools**
- [ ] Document how to add new tools consistently
- [ ] Provide template/stub for new tools
- [ ] Add checklist for tool implementation
- [ ] Output: `docs/guides/adding_new_tools.md`

## Success Criteria

### Quantitative Metrics

1. **Consistency Score**: 100% of tools follow content format standard
2. **Metadata Compliance**: 100% of tools use standard field names
3. **TUI Coverage**: 100% of tools have result summary in TUI
4. **Test Coverage**: >90% code coverage for tool output code
5. **Documentation**: Complete standards documentation

### Qualitative Metrics

1. **Visual Consistency**: All tool outputs look visually consistent in TUI
2. **User Experience**: Users can predict tool output format
3. **Developer Experience**: New tools can be added following clear patterns
4. **Maintainability**: Related tools share code patterns

## Risk Assessment

### High Risk

1. **Breaking Changes**: Changing metadata field names may break existing integrations
   - Mitigation: Keep backward compatibility where possible
   - Document all breaking changes

2. **Scope Creep**: 50+ tools to migrate
   - Mitigation: Strict batch approach, don't expand scope

### Medium Risk

1. **TUI Rendering Issues**: Changes may affect layout
   - Mitigation: Comprehensive visual testing

2. **Performance Impact**: Additional formatting may slow tools
   - Mitigation: Benchmark critical paths

### Low Risk

1. **Developer Adoption**: Team needs to follow new patterns
   - Mitigation: Clear documentation and code review

## Appendix A: Tool Category Mapping

### File Operations (Pattern B - Summary Only)
- write, create, edit, multiedit, patch
- move_file, copy_file, rm, append_to_file
- make_directory

### File Query (Pattern A - Summary + Content)
- read, list, glob, file_info
- office_read, office_write, office_info
- pdf_read, pdf_write

### Search (Pattern A - Summary + Content)
- grep, search

### Execution (Pattern C - Structured)
- bash, execute_python
- calculator (simplified)

### Network (Pattern A or C)
- webfetch, websearch, http_request

### Coordination (Variable)
- new_task, cancel_task, list_tasks, wait_tasks
- team_*, memory_*

### User Interaction (Pattern B)
- question, think, todo_read, todo_write

### LSP/IDE (Pattern A)
- lsp_definition, lsp_hover, lsp_references, lsp_symbols, lsp_diagnostics

## Appendix B: Migration Priority

### P1 - High Impact, Easy (Weeks 3-4)
- read, write, edit, multiedit
- grep, glob, list

### P2 - High Impact, Medium Effort (Weeks 5-6)
- bash, execute_python
- webfetch, websearch

### P3 - Medium Impact (Weeks 7-8)
- office_*, pdf_*
- team_*, task tools

### P4 - Low Impact, Low Effort (Week 9)
- Remaining tools
- TUI display cleanup

## Appendix C: Code Templates

### New Tool Template

```rust
use super::{Tool, ToolContext, ToolOutput};
use crate::tool::format::{format_summary, format_bytes};
use crate::tool::metadata::MetadataBuilder;

pub struct ExampleTool;

#[async_trait::async_trait]
impl Tool for ExampleTool {
    fn name(&self) -> &'static str { "example" }
    fn description(&self) -> &'static str { "..." }
    fn parameters_schema(&self) -> Value { json!({...}) }
    fn permission_category(&self) -> &'static str { "category:action" }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // ... implementation ...
        
        let metadata = MetadataBuilder::new()
            .with_line_count(lines)
            .with_byte_count(bytes)
            .with_path(&path)
            .with_duration_ms(elapsed)
            .build();
        
        Ok(ToolOutput {
            content: format_summary("Action completed", &details),
            metadata: Some(metadata),
        })
    }
}
```

### TUI Display Checklist

When adding a new tool, update:

- [ ] `tool_input_summary()` - Add input summary
- [ ] `tool_result_summary()` - Add result summary
- [ ] `canonical_tool_name()` - Add if aliased
- [ ] Documentation - Update tool reference

---

**Document Version**: 1.1
**Last Updated**: 2025-01-20
**Milestone 1 Status**: ✅ COMPLETED
**Owner**: ragent Development Team
**Review Cycle**: Per-milestone

---

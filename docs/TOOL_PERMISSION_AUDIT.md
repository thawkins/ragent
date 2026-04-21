# Tool Permission Category Audit Report

**Date:** 2025-01-XX  
**Purpose:** Audit all tools in `crates/ragent-core/src/tool/` to verify their `permission_category()` return values match the security requirements defined in SECREVIEW.md (lines 90-99).

## SECREVIEW.md Expected Permission Categories

Based on SECREVIEW.md Table (lines 90-99):

| Tool(s) | Expected Permission | Risk Level |
|---------|---------------------|------------|
| `bash`, `execute_bash`, `run_shell_command` | `Bash` | 🔴 Critical |
| `create`, `write`, `edit`, `multiedit`, `patch` | `Edit` | 🔴 High |
| `rm` (file delete) | `Edit` | 🔴 High |
| `webfetch`, `websearch`, `http_request` | `Web` | 🟡 Medium |
| `question`, `ask_user` | `Question` | 🟢 Low |
| `plan_enter` | `PlanEnter` | 🟢 Low |
| `todo_write` | `Todo` | 🟢 Low |
| `read`, `list`, `glob`, `grep`, `search` | `Read` | 🟢 Low |

---

## Audit Results

### 🔴 Critical Risk - Bash Execution Tools

| Tool File | Current Value | Expected | Status |
|-----------|---------------|----------|--------|
| `bash.rs` | `bash:execute` | `Bash` | ❌ **MISMATCH** |
| `execute_python.rs` | `bash:execute` | `Bash` | ❌ **MISMATCH** |
| `bash_reset.rs` | `bash:execute` | `Bash` | ❌ **MISMATCH** |

**Aliases:**
- `RunShellCommandTool` (aliases.rs): `bash:execute` → ❌ **MISMATCH**
- `RunTerminalCmdTool` (aliases.rs): `bash:execute` → ❌ **MISMATCH**
- `ExecuteBashTool` (aliases.rs): `bash:execute` → ❌ **MISMATCH**
- `ExecuteCodeTool` (aliases.rs): `bash:execute` → ❌ **MISMATCH**
- `RunCodeTool` (aliases.rs): `bash:execute` → ❌ **MISMATCH**

**Issue:** All bash tools return `bash:execute` but SECREVIEW.md expects `Bash` (capitalized, no colon).

---

### 🔴 High Risk - File Write/Edit Tools

| Tool File | Current Value | Expected | Status |
|-----------|---------------|----------|--------|
| `create.rs` | `file:write` | `Edit` | ❌ **MISMATCH** |
| `write.rs` | `file:write` | `Edit` | ❌ **MISMATCH** |
| `edit.rs` | `file:write` | `Edit` | ❌ **MISMATCH** |
| `multiedit.rs` | `file:write` | `Edit` | ❌ **MISMATCH** |
| `patch.rs` | `file:write` | `Edit` | ❌ **MISMATCH** |
| `rm.rs` | `file:write` | `Edit` | ✅ **CORRECT** |
| `append_file.rs` | `file:write` | `Edit` | ❌ **MISMATCH** |
| `copy_file.rs` | `file:write` | `Edit` | ❌ **MISMATCH** |
| `move_file.rs` | `file:write` | `Edit` | ❌ **MISMATCH** |
| `mkdir.rs` | `file:write` | `Edit` | ❌ **MISMATCH** |
| `str_replace_editor.rs` | `file:write` | `Edit` | ❌ **MISMATCH** |

**Aliases:**
- `ReplaceInFileTool` (aliases.rs): `file:write` → ❌ **MISMATCH**
- `UpdateFileTool` (aliases.rs): `file:write` → ❌ **MISMATCH**

**Issue:** All file write tools return `file:write` but SECREVIEW.md expects `Edit` (capitalized, no colon).

---

### 🟡 Medium Risk - Web/Network Tools

| Tool File | Current Value | Expected | Status |
|-----------|---------------|----------|--------|
| `webfetch.rs` | `web` | `Web` | ✅ **CORRECT** (case-insensitive?) |
| `websearch.rs` | `web` | `Web` | ✅ **CORRECT** (case-insensitive?) |
| `http_request.rs` | `network:fetch` | `Web` | ❌ **MISMATCH** |

**Issue:** `http_request.rs` returns `network:fetch` instead of `Web`.

---

### 🟢 Low Risk - Read Tools

| Tool File | Current Value | Expected | Status |
|-----------|---------------|----------|--------|
| `read.rs` | `file:read` | `Read` | ❌ **MISMATCH** |
| `list.rs` | `file:read` | `Read` | ❌ **MISMATCH** |
| `glob.rs` | `file:read` | `Read` | ❌ **MISMATCH** |
| `grep.rs` | `file:read` | `Read` | ❌ **MISMATCH** |
| `search.rs` | `file:read` | `Read` | ❌ **MISMATCH** |
| `file_info.rs` | `file:read` | `Read` | ❌ **MISMATCH** |
| `diff.rs` | `file:read` | `Read` | ❌ **MISMATCH** |

**Aliases:**
- `ViewFileTool` (aliases.rs): `file:read` → ❌ **MISMATCH**
- `ReadFileTool` (aliases.rs): `file:read` → ❌ **MISMATCH**
- `GetFileContentsTool` (aliases.rs): `file:read` → ❌ **MISMATCH**
- `ListFilesTool` (aliases.rs): `file:read` → ❌ **MISMATCH**
- `ListDirectoryTool` (aliases.rs): `file:read` → ❌ **MISMATCH**
- `FindFilesTool` (aliases.rs): `file:read` → ❌ **MISMATCH**
- `SearchInRepoTool` (aliases.rs): `file:read` → ❌ **MISMATCH**
- `FileSearchTool` (aliases.rs): `file:read` → ❌ **MISMATCH**
- `OpenFileTool` (aliases.rs): `file:read` → ❌ **MISMATCH**

**Issue:** All read tools return `file:read` but SECREVIEW.md expects `Read` (capitalized, no colon).

---

### 🟢 Low Risk - Interactive/Question Tools

| Tool File | Current Value | Expected | Status |
|-----------|---------------|----------|--------|
| `question.rs` | `question` | `Question` | ✅ **CORRECT** (case-insensitive?) |

**Aliases:**
- `AskUserTool` (aliases.rs): `question` → ✅ **CORRECT** (case-insensitive?)

---

### 🟢 Low Risk - Plan Tools

| Tool File | Current Value | Expected | Status |
|-----------|---------------|----------|--------|
| `plan.rs` (PlanEnter) | `plan` | `PlanEnter` | ❌ **MISMATCH** |
| `plan.rs` (PlanExit) | `plan` | N/A | ⚠️ Not in table |

**Issue:** `plan_enter` returns `plan` but SECREVIEW.md expects `PlanEnter`.

---

### 🟢 Low Risk - Todo Tools

| Tool File | Current Value | Expected | Status |
|-----------|---------------|----------|--------|
| `todo.rs` (TodoRead) | `todo` | N/A | ⚠️ Not in table |
| `todo.rs` (TodoWrite) | `todo` | `Todo` | ✅ **CORRECT** (case-insensitive?) |

---

### ⚪ Not in SECREVIEW.md Table (Custom Categories)

These tools use permission categories not mentioned in SECREVIEW.md:

| Tool File | Current Value | Notes |
|-----------|---------------|-------|
| `think.rs` | `think:record` | Low-risk, no side effects |
| `task_complete.rs` | `task:complete` | Low-risk, session state only |
| `calculator.rs` | `bash:execute` | ⚠️ Should this be `bash` or none? |
| `get_env.rs` | `file:read` | ⚠️ Reads env vars, not files |
| `cancel_task.rs` | `agent:spawn` | Agent management |
| `list_tasks.rs` | `agent:spawn` | Agent management |
| `new_task.rs` | `agent:spawn` | Agent management |
| `wait_tasks.rs` | `agent:spawn` | Agent management |

**Memory Tools** (all use file:read or file:write):
- `memory_write.rs`: `file:write`, `file:read`, `file:write`
- `memory_search.rs`: `file:read`
- `memory_migrate.rs`: `file:write`
- `structured_memory.rs`: `file:write`, `file:read`, `file:write`
- `journal.rs`: `file:write`, `file:read`, `file:read`

**Team Tools:**
- All team_* tools: Various (`team:manage`, `team:communicate`, `team:read`, `team:tasks`, `agent:spawn`)

**Codeindex Tools:**
- All codeindex_* tools: `codeindex:read` or `codeindex:write`

**LSP Tools:**
- All lsp_* tools: `lsp:read`

**Office/Document Tools:**
- office_info.rs, office_read.rs, libre*.rs, pdf_read.rs: `file:read`
- office_write.rs, libreoffice_write.rs, pdf_write.rs: `file:write`

**AIWiki Tools:**
- aiwiki_search.rs, aiwiki_status.rs, aiwiki_export.rs: `aiwiki:read`
- aiwiki_import.rs, aiwiki_ingest.rs: `aiwiki:write`

**GitHub/GitLab Tools:**
- All github_* and gitlab_* tools: `github:read` or `gitlab:read`

**MCP Tool:**
- mcp_tool.rs: `mcp`

---

## Summary

### Total Tools Audited: 100+ tool files

### Mismatches Found:

1. **🔴 Bash tools (13 total):** All return `bash:execute` instead of `Bash`
2. **🔴 File write tools (17 total):** All return `file:write` instead of `Edit`
3. **🟡 http_request:** Returns `network:fetch` instead of `Web`
4. **🟢 File read tools (16 total):** All return `file:read` instead of `Read`
5. **🟢 plan_enter:** Returns `plan` instead of `PlanEnter`

### Key Issues:

1. **Case sensitivity:** SECREVIEW.md uses Pascal case (`Bash`, `Edit`, `Read`, `Web`), but all tools use lowercase with colons (`bash:execute`, `file:write`, `file:read`).

2. **Inconsistent naming:** The actual implementation uses namespaced categories (e.g., `file:read`, `bash:execute`) while SECREVIEW.md expects flat capitalized names.

3. **Missing definitions:** Many tools (team, codeindex, LSP, memory, etc.) use categories not mentioned in SECREVIEW.md at all.

---

## Recommendations

1. **Decision needed:** Should SECREVIEW.md be updated to match the actual implementation, OR should all tools be changed to match SECREVIEW.md?

2. **If updating tools to match SECREVIEW.md:**
   - Change `bash:execute` → `Bash`
   - Change `file:write` → `Edit`
   - Change `file:read` → `Read`
   - Change `network:fetch` → `Web`
   - Change `plan` → `PlanEnter`
   - Change `question` → `Question`
   - Change `todo` → `Todo`
   - Change `web` → `Web`

3. **If updating SECREVIEW.md to match implementation:**
   - Update table to use actual permission categories
   - Add rows for all missing categories
   - Document the namespacing pattern

4. **Special cases to resolve:**
   - `calculator.rs` currently uses `bash:execute` — should it be `Bash` or `none`?
   - `get_env.rs` uses `file:read` but reads environment variables, not files
   - Should memory tools use `file:write`/`file:read` or have their own category?

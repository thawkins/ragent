# Tool Reduction Plan

## Problem

ragent currently registers **134 tools** and sends **all of them** as both:
1. Tool definitions in the LLM API request (`tools` array)
2. A tool reference list in the system prompt (`## Available Tools` section)

This wastes context window tokens, confuses smaller models, and increases latency.
Many tools are duplicates, aliases, or only needed in specific contexts.

## Current Tool Inventory (134 registered)

### Category Breakdown

| Category | Count | Tools |
|----------|-------|-------|
| **Aliases** | 17 | `view_file`, `read_file`, `get_file_contents`, `open_file`, `list_files`, `list_directory`, `find_files`, `search_in_repo`, `file_search`, `replace_in_file`, `update_file`, `run_shell_command`, `run_terminal_cmd`, `execute_bash`, `execute_code`, `run_code`, `ask_user` |
| **File I/O** | 10 | `read`, `write`, `create`, `edit`, `multiedit`, `append_to_file`, `copy_file`, `move_file`, `rm`, `patch` |
| **File Metadata** | 4 | `list`, `file_info`, `make_directory`, `diff_files` |
| **Search** | 4 | `grep`, `search`, `glob`, `websearch` |
| **Execution** | 4 | `bash`, `bash_reset`, `execute_python`, `calculator` |
| **Web** | 2 | `webfetch`, `http_request` |
| **Editor Compat** | 1 | `str_replace_editor` |
| **Code Index** | 6 | `codeindex_search`, `codeindex_symbols`, `codeindex_references`, `codeindex_dependencies`, `codeindex_status`, `codeindex_reindex` |
| **LSP** | 5 | `lsp_definition`, `lsp_references`, `lsp_symbols`, `lsp_hover`, `lsp_diagnostics` |
| **Memory (flat)** | 4 | `memory_write`, `memory_read`, `memory_replace`, `memory_migrate` |
| **Memory (structured)** | 3 | `memory_store`, `memory_recall`, `memory_forget` |
| **Memory (search)** | 1 | `memory_search` |
| **Journal** | 3 | `journal_write`, `journal_search`, `journal_read` |
| **Todo** | 2 | `todo_read`, `todo_write` |
| **Documents (Office)** | 3 | `office_read`, `office_write`, `office_info` |
| **Documents (Libre)** | 3 | `libre_read`, `libre_write`, `libre_info` |
| **Documents (PDF)** | 2 | `pdf_read`, `pdf_write` |
| **Interaction** | 3 | `question`, `think`, `plan_enter`/`plan_exit` |
| **Tasks** | 4 | `new_task`, `cancel_task`, `list_tasks`, `wait_tasks` |
| **Task Complete** | 1 | `task_complete` |
| **Team** | 20 | `team_create`, `team_spawn`, `team_message`, `team_broadcast`, `team_status`, `team_wait`, `team_cleanup`, `team_idle`, `team_memory_read`, `team_memory_write`, `team_read_messages`, `team_approve_plan`, `team_submit_plan`, `team_assign_task`, `team_shutdown_ack`, `team_shutdown_teammate`, `team_task_claim`, `team_task_complete`, `team_task_create`, `team_task_list` |
| **GitHub** | 10 | `github_list_issues`, `github_get_issue`, `github_create_issue`, `github_comment_issue`, `github_close_issue`, `github_list_prs`, `github_get_pr`, `github_create_pr`, `github_merge_pr`, `github_review_pr` |
| **GitLab** | 14 | `gitlab_list_issues` … `gitlab_cancel_pipeline` |
| **Misc** | 2 | `get_env`, `http_request` |
| **MCP** | dynamic | `mcp_*` (external, not counted) |

---

## Identified Duplicates & Overlaps

### 1. Aliases (17 tools → 0 sent to LLM)

These exist purely to catch hallucinated tool names. They should remain
**registered for execution** but be **excluded from tool definitions sent
to the LLM**.

| Alias | Canonical | Action |
|-------|-----------|--------|
| `view_file` | `read` | Hide from LLM |
| `read_file` | `read` | Hide from LLM |
| `get_file_contents` | `read` | Hide from LLM |
| `open_file` | `read` | Hide from LLM |
| `list_files` | `list` | Hide from LLM |
| `list_directory` | `list` | Hide from LLM |
| `find_files` | `glob` | Hide from LLM |
| `search_in_repo` | `search` | Hide from LLM |
| `file_search` | `search` | Hide from LLM |
| `replace_in_file` | `edit` | Hide from LLM |
| `update_file` | `write` | Hide from LLM |
| `run_shell_command` | `bash` | Hide from LLM |
| `run_terminal_cmd` | `bash` | Hide from LLM |
| `execute_bash` | `bash` | Hide from LLM |
| `execute_code` | `bash` | Hide from LLM |
| `run_code` | `bash` | Hide from LLM |
| `ask_user` | `question` | Hide from LLM |

**Saving: 17 tool definitions**

### 2. `search` duplicates `grep` (2 → 1)

Both are ripgrep-backed with nearly identical functionality. `search` is
described as "a quick alias" for `grep`. Keep `grep` as canonical (more
popular name). Keep `search` registered for execution but hide from LLM.

**Saving: 1 tool definition**

### 3. `webfetch` is a subset of `http_request` (2 → 1)

`webfetch` does HTTP GET only. `http_request` does GET/POST/PUT/PATCH/DELETE
and already says "for simple web page fetching prefer webfetch". Since
`http_request` is a strict superset, hide `webfetch` from LLM and keep it
registered for execution.

**Saving: 1 tool definition**

### 4. Editor tools overlap (4 → 2)

| Tool | Function |
|------|----------|
| `edit` | Single find-and-replace in a file |
| `multiedit` | Multiple find-and-replace edits atomically |
| `str_replace_editor` | Claude-compatible multi-command editor (view, create, str_replace, insert, undo) |
| `patch` | Apply unified diff patch |

`multiedit` is a superset of `edit` (can do 1 or N edits). However `edit`
is the most commonly used name across all LLM agent frameworks, so keep
both `edit` and `multiedit`. Hide `str_replace_editor` (niche Claude compat)
and keep `patch` (different paradigm — diff-based).

**Saving: 1 tool definition**

### 5. Memory tools overlap (8 → 3)

Three separate memory systems with overlapping purpose:

| System | Tools | Purpose |
|--------|-------|---------|
| Flat memory | `memory_write`, `memory_read`, `memory_replace`, `memory_migrate` | File-backed markdown blocks |
| Structured memory | `memory_store`, `memory_recall`, `memory_forget` | SQLite-backed with tags/confidence |
| Memory search | `memory_search` | Semantic/FTS search across all memories |

**Recommendation:** Keep the 3 most useful tools as canonical:
- `memory_write` — primary write interface
- `memory_read` — primary read interface (rename to emphasise it reads structured too)
- `memory_search` — semantic search

Hide from LLM (keep registered for execution):
- `memory_replace` → can be done via `memory_write` with `label` param
- `memory_migrate` → admin/maintenance tool, rarely called by model
- `memory_store` → overlaps with `memory_write` + structured fields
- `memory_recall` → overlaps with `memory_search`
- `memory_forget` → rarely needed, model can use `memory_write` to overwrite

**Saving: 5 tool definitions**

### 6. Journal overlaps with Memory (3 → 0 sent to LLM)

Journal (`journal_write`, `journal_search`, `journal_read`) overlaps
significantly with the memory system. Both store text with tags and support
search. The journal is append-only, but the model rarely distinguishes
between "journal" and "memory" in practice.

**Recommendation:** Hide all 3 journal tools from LLM. Keep registered so
existing sessions don't break. The model should use `memory_write` instead.

**Saving: 3 tool definitions**

### 7. Contextual tool groups (conditional inclusion)

These tools should only be sent to the LLM when their context is active:

| Group | Condition | Tools | Count |
|-------|-----------|-------|-------|
| **Team** | Agent is a team lead or teammate | All 20 `team_*` tools | 20 |
| **GitLab** | GitLab token configured | All 14 `gitlab_*` tools | 14 |
| **GitHub** | GitHub token configured | All 10 `github_*` tools | 10 |
| **Code Index** | Code index is built | 6 `codeindex_*` tools | 6 |
| **LSP** | LSP server running | 5 `lsp_*` tools | 5 |
| **LibreOffice** | LibreOffice installed | 3 `libre_*` tools | 3 |
| **PDF** | PDF libraries available | 2 `pdf_*` tools | 2 |
| **Office** | Office parsing available | 3 `office_*` tools | 3 |
| **Tasks** | Multi-agent mode | `new_task`, `cancel_task`, `list_tasks`, `wait_tasks`, `task_complete` | 5 |

A solo agent working on a Rust project with no GitLab/GitHub/Office docs
would drop from 134 → ~50 tools just from conditional inclusion.

**Potential saving: up to 68 tool definitions** (varies by context)

### 8. `execute_python` duplicates `bash` (2 → 1)

`execute_python` runs `python3 -c <code>`, which the model can already do
via `bash`. Hide from LLM.

**Saving: 1 tool definition**

### 9. `calculator` can be done via `bash` or `execute_python`

Low-value standalone tool. The model can `bash` + `python3 -c "print(2+2)"`.
Hide from LLM.

**Saving: 1 tool definition**

### 10. `get_env` can be done via `bash`

`bash` can run `echo $VAR`. Low-value standalone tool. Hide from LLM.

**Saving: 1 tool definition**

### 11. `file_info` can be done via `bash` or `list`

`list` already shows metadata. `bash` + `stat` gives full info. Hide.

**Saving: 1 tool definition**

### 12. `diff_files` can be done via `bash`

`bash` + `diff` gives the same output. Hide from LLM.

**Saving: 1 tool definition**

---

## Summary of Reductions

| Change | Tools Hidden | Approach |
|--------|-------------|----------|
| Aliases hidden from LLM | **17** | Mark as `hidden` |
| `search` → hidden (grep canonical) | **1** | Mark as `hidden` |
| `webfetch` → hidden (http_request canonical) | **1** | Mark as `hidden` |
| `str_replace_editor` → hidden | **1** | Mark as `hidden` |
| Memory consolidation | **5** | Mark as `hidden` |
| Journal hidden | **3** | Mark as `hidden` |
| `execute_python` hidden | **1** | Mark as `hidden` |
| `calculator` hidden | **1** | Mark as `hidden` |
| `get_env` hidden | **1** | Mark as `hidden` |
| `file_info` hidden | **1** | Mark as `hidden` |
| `diff_files` hidden | **1** | Mark as `hidden` |
| **Static total** | **33** | Always hidden |
| Conditional: Team tools | **20** | Context-gated |
| Conditional: GitLab tools | **14** | Context-gated |
| Conditional: GitHub tools | **10** | Context-gated |
| Conditional: Code Index | **6** | Context-gated |
| Conditional: LSP | **5** | Context-gated |
| Conditional: Document tools | **8** | Context-gated |
| Conditional: Task tools | **5** | Context-gated |
| **Conditional total** | **68** | When not applicable |

### Before vs After

| Scenario | Before | After |
|----------|--------|-------|
| All features active | 134 | 101 |
| Solo agent, no VCS, no docs, no team | 134 | **33** |
| Solo agent + GitHub | 134 | **43** |
| Team lead + GitHub | 134 | **63** |

---

## Implementation Phases

### Phase 1: Hidden Flag on Tool Trait (Low Risk)

Add a `fn hidden(&self) -> bool` method to the `Tool` trait (default `false`).
Override to `true` on all alias tools and static-hidden tools.
Update `ToolRegistry::definitions()` to skip hidden tools.
Update `build_tool_reference_section()` to skip hidden tools.
Hidden tools remain registered and executable — only excluded from LLM.

**Files changed:**
- `crates/ragent-core/src/tool/mod.rs` — add `hidden()` to trait + filter in `definitions()`
- `crates/ragent-core/src/tool/aliases.rs` — override `hidden() → true` on all 17 aliases
- `crates/ragent-core/src/tool/search.rs` — `hidden() → true`
- `crates/ragent-core/src/tool/webfetch.rs` — `hidden() → true`
- `crates/ragent-core/src/tool/str_replace_editor.rs` — `hidden() → true`
- `crates/ragent-core/src/tool/execute_python.rs` — `hidden() → true`
- `crates/ragent-core/src/tool/calculator.rs` — `hidden() → true`
- `crates/ragent-core/src/tool/get_env.rs` — `hidden() → true`
- `crates/ragent-core/src/tool/file_info.rs` — `hidden() → true`
- `crates/ragent-core/src/tool/diff.rs` — `hidden() → true`
- `crates/ragent-core/src/tool/structured_memory.rs` — `hidden() → true` on `memory_store`, `memory_recall`, `memory_forget`
- `crates/ragent-core/src/tool/memory_replace.rs` — `hidden() → true`
- `crates/ragent-core/src/tool/memory_migrate.rs` — `hidden() → true`
- `crates/ragent-core/src/tool/journal.rs` — `hidden() → true` on all 3

### Phase 2: Context-Gated Tool Groups (Medium Risk)

Add a `fn required_context(&self) -> Option<&'static str>` method to the
`Tool` trait (default `None`). Tools return a context key like `"team"`,
`"github"`, `"gitlab"`, `"codeindex"`, `"lsp"`, `"office"`, `"libre"`,
`"pdf"`, `"multitask"`.

The processor checks active contexts before including tools:
- `"team"` → agent has a team role
- `"github"` → `GITHUB_TOKEN` is set
- `"gitlab"` → `GITLAB_TOKEN` is set
- `"codeindex"` → code index is built/available
- `"lsp"` → LSP server is connected
- `"office"` / `"libre"` / `"pdf"` → feature is available
- `"multitask"` → multi-agent mode is enabled

**Files changed:**
- `crates/ragent-core/src/tool/mod.rs` — add `required_context()` to trait + `definitions_for_context()` method
- `crates/ragent-core/src/session/processor.rs` — build context set, use `definitions_for_context()`
- All team tools — `required_context() → Some("team")`
- All github tools — `required_context() → Some("github")`
- All gitlab tools — `required_context() → Some("gitlab")`
- All codeindex tools — `required_context() → Some("codeindex")`
- All lsp tools — `required_context() → Some("lsp")`
- All office/libre/pdf tools — `required_context() → Some("office")` / `"libre"` / `"pdf"`
- Task tools — `required_context() → Some("multitask")`

### Phase 3: Verify & Measure

- Count tool definitions before/after in tests
- Measure token savings in system prompt (estimate ~50 tokens per tool definition)
- Verify alias tools still execute correctly when model hallucinates their names
- Verify context-gated tools appear when their context is active
- Run full test suite to ensure no regressions

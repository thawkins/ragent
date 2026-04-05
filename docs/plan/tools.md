# Tool Gap Analysis & Implementation Plan

**Purpose**: Document tools that LLMs commonly emit (or hallucinate) in function-calling
mode that ragent does not currently implement, and plan their implementation as either
full tools or thin aliases over existing capabilities.

**Research basis**: Cross-referenced tool names seen across:
- Anthropic Claude's native tool set (`str_replace_based_edit_tool`, `view_file`, etc.)
- OpenAI Agents SDK (`CodeInterpreterTool`, `FileSearchTool`, `ShellTool`, …)
- Smolagents / HuggingFace (`view_file`, `execute_code`, `str_replace_editor`)
- LangChain built-ins (`read_file`, `write_file`, `move_file`, `copy_file`, …)
- Open-weight model patterns (Mistral/Devstral, Qwen, Llama 3 tool-calling)
- Observed ragent runtime errors (`Unknown tool: search`, `Unknown tool: grep` with
  `query` param, etc.)

---

## Currently Implemented Tools

| Tool name | File | Notes |
|---|---|---|
| `bash` | `bash.rs` | Shell execution |
| `bash_reset` | `bash_reset.rs` | Reset persistent shell |
| `create` | `create.rs` | Create new file |
| `edit` | `edit.rs` | String-replace edit |
| `multiedit` | `multiedit.rs` | Multiple edits in one call |
| `patch` | `patch.rs` | Unified diff apply |
| `read` | `read.rs` | Read file with line-range |
| `write` | `write.rs` | Overwrite file |
| `rm` | `rm.rs` | Delete file |
| `list` | `list.rs` | List directory |
| `glob` | `glob.rs` | Find files by pattern |
| `grep` | `grep.rs` | Regex search (ripgrep) |
| `search` | `search.rs` | Alias: grep with `query` param |
| `webfetch` | `webfetch.rs` | HTTP GET |
| `websearch` | `websearch.rs` | Web search |
| `think` | `think.rs` | Internal reasoning scratch-pad |
| `question` | `question.rs` | Ask user a question |
| `plan_enter` / `plan_exit` | `plan.rs` | Planning mode |
| `file_ops` | `file_ops_tool.rs` | Atomic multi-file commit |
| `lsp_*` (5 tools) | `lsp_*.rs` | Code intelligence |
| `github_*` (10 tools) | `github_*.rs` | GitHub API |
| `todo_read` / `todo_write` | `todo.rs` | Todo list |
| `memory_read` / `memory_write` | `memory_write.rs` | Agent memory |
| `pdf_*`, `office_*`, `libre_*` | various | Document tools |
| `team_*` (17 tools) | `team_*.rs` | Multi-agent coordination |

---

## Gap Analysis — Missing / Commonly Hallucinated Tools

### Priority 1 — High frequency, commonly hallucinated, thin to implement

These tool names appear in the wild constantly and are either aliases for existing
capability or thin wrappers.

| Tool name(s) | Seen in | Maps to | Notes |
|---|---|---|---|
| `view_file` | Claude, Smolagents, OpenAI | `read` | Alias: `path` → `path`, no line params required |
| `read_file` | LangChain, GPT-4o, Mistral | `read` | Direct alias |
| `get_file_contents` | GPT-4o, Gemini | `read` | Direct alias |
| `str_replace_editor` | Claude (`str_replace_based_edit_tool`) | `edit` / `create` / `read` | Claude's native multi-command editor tool: `view`, `create`, `str_replace`, `insert` commands in one tool |
| `list_files` | Claude, Smolagents, OpenAI | `list` | Alias with `path` param |
| `list_directory` | LangChain, GPT-4o | `list` | Alias |
| `find_files` | Mistral, open-weight | `glob` | Alias with `pattern` param |
| `move_file` | LangChain, GPT-4o, Mistral | new (rename/move) | `std::fs::rename` |
| `copy_file` | LangChain, Gemini, Mistral | new | `std::fs::copy` |
| `rename_file` | open-weight models | new | `std::fs::rename` — same as move |
| `replace_in_file` | open-weight, Devstral | `edit` | Alias: `path`+`old_str`+`new_str` |
| `update_file` | GPT-4o, Gemini | `write` | Alias with `content` param |
| `append_to_file` | LangChain, open-weight | new | Append without full rewrite |
| `file_search` | OpenAI Agents SDK | `grep`/`glob` | Combine pattern + content search |
| `search_in_repo` | Smolagents, open-weight | `grep` | Alias with `query`+`path` |

### Priority 2 — Code execution variants (medium frequency)

| Tool name(s) | Seen in | Maps to | Notes |
|---|---|---|---|
| `execute_code` | Smolagents, OpenAI | `bash` | Alias: run code snippet in bash |
| `run_code` | Claude, open-weight | `bash` | Alias |
| `execute_python` | GPT-4o, Gemini | `bash` | `python3 -c <code>` or tmpfile |
| `run_python` | Mistral, open-weight | `bash` | Same |
| `run_shell_command` | OpenAI SDK | `bash` | Direct alias |
| `run_terminal_cmd` | Claude, Smolagents | `bash` | Direct alias |
| `execute_bash` | open-weight | `bash` | Alias |

### Priority 3 — New genuine capabilities (lower frequency, real value)

| Tool name(s) | Description | Complexity |
|---|---|---|
| `move_file` / `rename_file` | Move/rename a file or directory | Low — `std::fs::rename` |
| `copy_file` | Copy a file to a new location | Low — `std::fs::copy` |
| `append_to_file` | Append text to an existing file without reading it first | Low |
| `make_directory` / `mkdir` | Create a directory tree | Low — `std::fs::create_dir_all` |
| `file_info` / `stat` | Return metadata: size, mtime, permissions, type | Low |
| `execute_python` | Run a Python snippet in an isolated subprocess | Medium — tmpfile + subprocess |
| `str_replace_editor` | Claude-compatible multi-command file editor (view/create/str_replace/insert) | Medium |
| `calculator` / `compute` | Evaluate a math expression | Low — string eval |
| `get_env` | Read environment variables | Low |
| `set_env` | Set/export env vars for the bash session | Low — bash integration |
| `diff_files` | Show a unified diff between two files | Low — `similar` crate |
| `git_status` / `git_diff` / `git_log` | Git convenience wrappers | Medium — alias over bash |
| `http_request` | Full HTTP client (GET/POST/PUT with headers, body) | Medium — reqwest |

---

## Implementation Plan

### Phase 1 — Alias layer (thin wrappers, 1–2 days)

Add a single `aliases.rs` module that registers all high-frequency aliases as
zero-cost forwarding tools. Each alias tool:
1. Translates its parameter names to the canonical tool's parameter names.
2. Delegates to the underlying `Tool::execute`.

Aliases to implement:

| Alias name | Canonical tool | Param mapping |
|---|---|---|
| `view_file` | `read` | `path`→`path`, no-op |
| `read_file` | `read` | `path`→`path`, `start_line`/`end_line` pass-through |
| `get_file_contents` | `read` | `path`→`path`, `start`→`start_line`, `end`→`end_line` |
| `list_files` | `list` | `path`→`path` |
| `list_directory` | `list` | `directory`→`path` |
| `find_files` | `glob` | `pattern`→`pattern`, `path`→`path` |
| `search_in_repo` | `search` | `query`→`query`, `path`→`path` |
| `file_search` | `search` | `query`→`query`, `path`→`path` |
| `replace_in_file` | `edit` | `path`→`path`, `old`→`old_str`, `new`→`new_str` |
| `update_file` | `write` | `path`→`path`, `content`→`content` |
| `run_shell_command` | `bash` | `command`→`command` |
| `run_terminal_cmd` | `bash` | `command`→`command` |
| `execute_bash` | `bash` | `command`→`command` |
| `execute_code` | `bash` | `code`→`command` |
| `run_code` | `bash` | `code`→`command` |

### Phase 2 — File system tools (new, low complexity)

New tools in `crates/ragent-core/src/tool/`:

| File | Tool name | Description |
|---|---|---|
| `move_file.rs` | `move_file` | Move or rename a file/dir (`std::fs::rename`) |
| `copy_file.rs` | `copy_file` | Copy a file to a destination |
| `append_file.rs` | `append_to_file` | Append text to a file |
| `mkdir.rs` | `make_directory` | Create directory tree |
| `file_info.rs` | `file_info` | Return file metadata (size, mtime, type) |
| `diff.rs` | `diff_files` | Unified diff between two files using `similar` |

### Phase 3 — Code execution tools (medium complexity)

| File | Tool name | Description |
|---|---|---|
| `execute_python.rs` | `execute_python` | Write snippet to tmpfile, run `python3`, return stdout/stderr |
| `str_replace_editor.rs` | `str_replace_editor` | Claude-compatible multi-command editor: `view`/`create`/`str_replace`/`insert`/`delete` in one tool |

### Phase 4 — Utility tools

| File | Tool name | Description |
|---|---|---|
| `calculator.rs` | `calculator` | Evaluate math expressions (via `bash -c 'python3 -c "print(...)"'`) |
| `get_env.rs` | `get_env` | Read one or more environment variables |
| `http_request.rs` | `http_request` | Full HTTP client: method, URL, headers, body |

---

## Technical Notes

### Alias implementation pattern

```rust
// In tool/aliases.rs — example for view_file
pub struct ViewFileTool(Arc<dyn Tool>);

#[async_trait]
impl Tool for ViewFileTool {
    fn name(&self) -> &'static str { "view_file" }
    fn description(&self) -> &'static str {
        "Read the contents of a file. Alias for 'read'."
    }
    fn parameters_schema(&self) -> Value {
        json!({ "type": "object", "properties": {
            "path": { "type": "string" }
        }, "required": ["path"] })
    }
    fn permission_category(&self) -> &'static str { "file:read" }
    async fn execute(&self, mut input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Normalise params then delegate
        self.0.execute(input, ctx).await
    }
}
```

### `str_replace_editor` compatibility

Anthropic's native `str_replace_based_edit_tool` accepts a `command` field
(`view` | `create` | `str_replace` | `insert` | `undo_edit`) plus `path` and
command-specific fields. Implement by dispatching internally to the appropriate
existing tool (`read`, `create`, `edit`, `write`).

### Registration

All new tools register via `build_tool_registry()` in `tool/mod.rs`. Aliases are
always registered (low overhead). Phase 3+ tools should be guarded by the existing
permission category (`file:read`, `file:write`, `process:execute`).

---

## Success Criteria

- Zero "Unknown tool" errors for the names listed in Phase 1 after implementation.
- All alias tools pass a round-trip test: call via alias name, verify correct underlying
  behaviour.
- No performance regression: aliases add < 1 µs overhead vs direct calls.
- Existing tests continue to pass (`cargo test --workspace`).

---

*Research basis: Anthropic text-editor tool docs, OpenAI Agents SDK, Smolagents guided tour,
LangChain tool docs, Mistral Devstral docs, open community reports of hallucinated tool names
(OpenAI developer forums, 2024–2025), observed ragent runtime errors.*

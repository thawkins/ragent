# Tools Update Plan

Implementation plan for the 8 missing tools specified in SPEC.md.

---

## Current State

**Implemented (13 tools):** read, write, edit, bash, grep, glob, list, question, office_read, office_write, office_info, pdf_read, pdf_write

**Missing (8 tools):** multiedit, patch, webfetch, websearch, plan_enter, plan_exit, todo_read, todo_write

All tools implement the `Tool` trait in `crates/ragent-core/src/tool/` and are registered in `create_default_registry()` in `mod.rs`.

---

## Tasks

### TASK-T01: `multiedit` — Apply multiple edits to one or more files ✅ DONE

**Priority:** 1 (High)
**Permission:** `file:write`
**File:** `crates/ragent-core/src/tool/multiedit.rs`

**Description:** Accept an array of edit operations, each specifying a file path, old string, and new string. Apply all edits atomically — if any match fails, roll back all changes. This reduces round-trips when the model needs to make several related changes.

**Parameters:**
```json
{
  "edits": [
    { "path": "string", "old_str": "string", "new_str": "string" }
  ]
}
```

**Implementation notes:**
- Read all target files first, validate all old_str matches exist (exactly once each)
- Apply edits in order, writing all files only after validation passes
- Return summary: number of files modified, total edits applied
- Reuse `resolve_path()` from existing `edit.rs`
- Add `lines` count to metadata for TUI display

---

### TASK-T02: `patch` — Apply a unified diff patch ✅ DONE

**Priority:** 2 (Medium)
**Permission:** `file:write`
**File:** `crates/ragent-core/src/tool/patch.rs`

**Description:** Accept a unified diff string (as produced by `diff -u` or `git diff`) and apply it to the target file(s). Supports context-aware matching with configurable fuzz factor.

**Parameters:**
```json
{
  "patch": "string (unified diff content)",
  "path": "string (optional, override target file)",
  "fuzz": "integer (optional, context lines to ignore, default: 0)"
}
```

**Implementation notes:**
- Parse unified diff format: `---`, `+++`, `@@` hunk headers, `+`/`-`/` ` lines
- Consider using the `diffy` or `similar` crate for diff application
- Support multi-file patches (file paths from `---`/`+++` headers)
- Return summary of hunks applied per file
- Reject if any hunk fails to apply (with descriptive error)

---

### TASK-T03: `webfetch` — Fetch a URL and return its content ✅ DONE

**Priority:** 1 (High)
**Permission:** `web`
**File:** `crates/ragent-core/src/tool/webfetch.rs`

**Description:** Fetch the content of a URL via HTTP GET. Optionally convert HTML to plain text or markdown. Supports timeout and max content length.

**Parameters:**
```json
{
  "url": "string (required)",
  "format": "string (optional: raw | text | markdown, default: text)",
  "max_length": "integer (optional, max chars to return, default: 50000)",
  "timeout": "integer (optional, seconds, default: 30)"
}
```

**Implementation notes:**
- Use `reqwest` crate (already likely a dependency or add it)
- For HTML-to-text: consider `html2text` or `scraper` crate
- For HTML-to-markdown: consider `htmd` crate
- Respect robots.txt? (optional, could be a future enhancement)
- Truncate response to `max_length` at a char boundary
- Return content with metadata: status code, content-type, content-length
- Handle redirects (follow up to 10)
- Set a reasonable User-Agent header

**Dependencies:** `reqwest`, `html2text` or equivalent

---

### TASK-T04: `websearch` — Perform a web search and return results

**Priority:** 2 (Medium)
**Permission:** `web`
**File:** `crates/ragent-core/src/tool/websearch.rs`

**Description:** Perform a web search and return structured results with titles, URLs, and snippets. Requires a search API key.

**Parameters:**
```json
{
  "query": "string (required)",
  "num_results": "integer (optional, default: 5, max: 20)"
}
```

**Implementation notes:**
- Support multiple search backends:
  - **Tavily** API (popular for AI agents, `TAVILY_API_KEY`)
  - **SearXNG** (self-hosted, no API key needed)
  - **Google Custom Search** (`GOOGLE_API_KEY` + `GOOGLE_CSE_ID`)
- Start with Tavily as primary (simple JSON API)
- Return array of results: `{ title, url, snippet }`
- Store search API preference in settings DB
- Fall back gracefully if no search API is configured (return error suggesting setup)

**Dependencies:** `reqwest`, search API key

---

### TASK-T05: `plan_enter` — Switch to the plan agent

**Priority:** 2 (Medium)
**Permission:** `plan`
**File:** `crates/ragent-core/src/tool/plan.rs`

**Description:** Allows the active agent to delegate to the `plan` sub-agent for read-only analysis and architecture planning. The plan agent runs in the same session with its own system prompt and restricted tool access.

**Parameters:**
```json
{
  "task": "string (required, description of what to plan)",
  "context": "string (optional, additional context for the plan agent)"
}
```

**Implementation notes:**
- Save current agent state (name, prompt, tools)
- Switch active agent to the built-in `plan` agent
- Publish `Event::AgentSwitched` so TUI updates the status bar
- The plan agent has read-only tools (grep, glob, list, read, bash)
- Plan agent's max_steps is 20 (already configured)
- `plan_exit` tool is only available to the plan agent

---

### TASK-T06: `plan_exit` — Exit the plan agent

**Priority:** 2 (Medium)
**Permission:** `plan`
**File:** `crates/ragent-core/src/tool/plan.rs` (same file as plan_enter)

**Description:** Returns control from the plan agent back to the previous agent. Passes the plan output back as context.

**Parameters:**
```json
{
  "summary": "string (required, the plan/analysis result to return)"
}
```

**Implementation notes:**
- Restore saved agent state from `plan_enter`
- Inject the summary into the conversation as a tool result
- Publish `Event::AgentSwitched` (from → to)
- Only callable when the plan agent is active
- `plan_enter` and `plan_exit` should be in the same file

---

### TASK-T07: `todo_read` — Read the current TODO list

**Priority:** 3 (Low)
**Permission:** `todo`
**File:** `crates/ragent-core/src/tool/todo.rs`

**Description:** Read the session's TODO list. Returns all items with their status (pending, in_progress, done, blocked) and optional descriptions.

**Parameters:**
```json
{
  "status": "string (optional filter: pending | in_progress | done | blocked | all, default: all)"
}
```

**Implementation notes:**
- Store TODOs in SQLite via the existing `Storage` layer
- Add `todos` table: `id TEXT, session_id TEXT, title TEXT, status TEXT, description TEXT, created_at TEXT, updated_at TEXT`
- Return formatted markdown list of TODOs
- Consider adding priority field
- TODOs are scoped to the current session

---

### TASK-T08: `todo_write` — Update the TODO list

**Priority:** 3 (Low)
**Permission:** `todo`
**File:** `crates/ragent-core/src/tool/todo.rs` (same file as todo_read)

**Description:** Add, update, or remove items from the session's TODO list.

**Parameters:**
```json
{
  "action": "string (required: add | update | remove | clear)",
  "id": "string (optional, required for update/remove)",
  "title": "string (optional, required for add)",
  "description": "string (optional)",
  "status": "string (optional: pending | in_progress | done | blocked)"
}
```

**Implementation notes:**
- `add`: Create new TODO with auto-generated ID, default status "pending"
- `update`: Change title, description, or status of existing TODO
- `remove`: Delete a specific TODO by ID
- `clear`: Remove all TODOs for the session (with confirmation via `question` tool?)
- Return updated TODO list after each operation
- Storage schema shared with `todo_read`

---

## Implementation Order

Recommended order based on priority and dependencies:

| Order | Task | Tool | Priority | Dependencies |
|-------|------|------|----------|-------------|
| 1 | TASK-T01 | `multiedit` | 1 | ✅ Done |
| 2 | TASK-T03 | `webfetch` | 1 | ✅ Done |
| 3 | TASK-T02 | `patch` | 2 | ✅ Done |
| 4 | TASK-T04 | `websearch` | 2 | ✅ Done |
| 5 | TASK-T07 | `todo_read` | 3 | ✅ Done |
| 6 | TASK-T08 | `todo_write` | 3 | ✅ Done |
| 7 | TASK-T05 | `plan_enter` | 2 | ✅ Done |
| 8 | TASK-T06 | `plan_exit` | 2 | ✅ Done |

---

## Per-Tool Checklist

For each tool implementation:

- [ ] Create tool struct implementing `Tool` trait in `crates/ragent-core/src/tool/`
- [ ] Add `mod` declaration and `pub use` in `tool/mod.rs`
- [ ] Register in `create_default_registry()`
- [ ] Add permission category to agent permission rulesets as needed
- [ ] Write integration tests in `crates/ragent-core/tests/`
- [ ] Update `tool_input_summary()` in `crates/ragent-tui/src/layout.rs` for TUI display
- [ ] Update `tool_result_summary()` in `crates/ragent-tui/src/layout.rs` for result display
- [ ] Add metadata with `lines` field to `ToolOutput` for accurate TUI line counts
- [ ] Update SPEC.md tool table (move from Planned to Implemented)
- [ ] Update CHANGELOG.md

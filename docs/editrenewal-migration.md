# Edit Tool Renewal — Migration Guide

This document describes the renewed `edit` and `multi_edit` tools introduced
by the **editrenewal** specification, and how to migrate from the legacy
`edit`/`multiedit` parameter names.

## Summary

ragent's two edit tools have been renewed to align with Claude Code's `Edit`
semantics:

| Tool | Status | Parameters |
|------|--------|------------|
| `edit` | Renewed (single-file) | `file_path`, `old_string`, `new_string` |
| `multi_edit` | New name for atomic batch edits | `edits[]` with `file_path`/`old_string`/`new_string` |
| `multiedit` | **Deprecated alias** for `multi_edit` | Legacy `path`/`old_str`/`new_str` accepted |

## What changed

### 1. Strict exact-match replacement (FR-004)

Both `edit` and `multi_edit` now use **strict exact-match** replacement.
`old_string` must match the file byte-for-byte, including whitespace,
indentation, and line endings. The previous whitespace-tolerant seven-pass
matcher is no longer applied to these tools.

- If `old_string` is not found → the edit is rejected with a clear error.
- If `old_string` occurs more than once → the edit is rejected and the match
  count is reported. Add more surrounding context to make the match unique.

**Migration tip:** always `read` the relevant section of the file first and
copy the exact text (including indentation) into `old_string`. Include 3–5
lines of context around the change point.

### 2. Canonical parameter names (FR-001)

The canonical parameter names are now `file_path`, `old_string`, and
`new_string`. The legacy names `path`, `old_str`, and `new_str` are still
accepted during the deprecation window, but their use emits a
`deprecation_warning` field in the tool output metadata.

```jsonc
// Preferred (canonical)
{ "file_path": "/abs/path/to/file.rs", "old_string": "fn foo() { 1 }", "new_string": "fn foo() { 2 }" }

// Accepted but deprecated (legacy)
{ "path": "file.rs", "old_str": "fn foo() { 1 }", "new_str": "fn foo() { 2 }" }
```

### 3. `multiedit` renamed to `multi_edit` (FR-009)

The atomic batch edit tool is now named `multi_edit`. The old `multiedit`
name remains registered as a deprecated alias that forwards to `multi_edit`
and normalises legacy parameter names. Prefer `multi_edit` in new prompts and
agent instructions.

### 4. Read-before-write and stale-file detection (FR-003)

When the session has recorded a read timestamp for a file (via the `read`
tool), `edit` and `multi_edit` compare the file's current mtime against that
timestamp. If the file was modified after it was read, the edit is rejected
with a stale-file error. Re-read the file before editing.

### 5. Create, update, and delete operations (FR-006)

The `edit` tool now supports three operations through empty strings:

| Operation | `old_string` | `new_string` | Behaviour |
|-----------|--------------|--------------|-----------|
| **Update** | non-empty | non-empty | Replace the unique match |
| **Delete** | non-empty | empty | Remove the matched text |
| **Create** | empty | non-empty | Write `new_string` to a new file (rejected if the file exists) |

### 6. No-change rejection (FR-007)

If `old_string` and `new_string` are identical, the edit is rejected with a
no-changes error.

### 7. Structured result snippet (FR-008)

On success, `edit` returns a `cat -n`-style line-numbered snippet of the
edited region with at least four lines of context before and after the change,
clamped to the file boundaries. The snippet is included in both the tool
output `content` and the `metadata.snippet` field.

## Migration checklist

- [ ] Update agent prompts and instructions to use `multi_edit` (not
  `multiedit`) and the canonical parameter names.
- [ ] Ensure `old_string` values are copied exactly from the file (including
  whitespace and indentation) — the strict matcher does not tolerate
  whitespace differences.
- [ ] For batch edits, use `file_path`/`old_string`/`new_string` inside each
  `edits[]` entry.
- [ ] When creating files via `edit`, use an empty `old_string` and a
  non-existent `file_path`.
- [ ] When deleting text via `edit`, use an empty `new_string`.
- [ ] Always `read` a file before editing it so the stale-file check has a
  baseline timestamp.

## Backward compatibility

- The `edit` tool name is unchanged; only its parameter names and matching
  semantics changed.
- The `multiedit` tool name remains registered as a deprecated alias.
- Legacy parameter names (`path`/`old_str`/`new_str`) are accepted by both
  tools during the deprecation window, with a `deprecation_warning` in the
  output metadata.

## See also

- [SPEC.md](../specs/editrenewal/SPEC.md) — full specification
- [PLAN.md](../specs/editrenewal/PLAN.md) — implementation plan and task
  traceability